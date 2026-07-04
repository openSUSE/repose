import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock, call

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.install import Install
from repose.types.repa import Repa


class MockRepo:
    def __init__(self, name, url, refresh=False):
        self.name = name
        self.url = url
        self.refresh = refresh


def _ok_out():
    return [["cmd", "", "", 0, 0]]


def _fail_out():
    return [["cmd", "", "boom", 1, 0]]


@pytest.fixture
def mock_args_and_repa():
    repa_instance = Repa("dummy-repa")
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[repa_instance],
        config="dummy_config",
        yaml=False,
    )
    return args, repa_instance


def test_install_command_run(monkeypatch, mock_args_and_repa, mock_ssh_client):
    mock_args, repa_instance = mock_args_and_repa

    # Mocks
    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = {
        "product-to-install": [MockRepo("repo1", "http://repo1.url", refresh=True)]
    }

    mock_target = MagicMock()
    mock_target.products.get_base.return_value = "dummy_base"
    mock_target.products.is_transactional.return_value = False
    mock_target.out = _ok_out()
    mock_host_group_instance = MagicMock()
    mock_host_group_instance.keys.return_value = ["user@host1"]
    mock_host_group_instance.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_host_group_instance),
    )
    monkeypatch.setattr(Install, "repoq", mock_repoq)
    monkeypatch.setattr(
        repose.command._command, "check_repo_url", lambda url, *, timeout: True
    )

    # Run
    install_command = Install(mock_args)
    assert install_command.run() == 0

    # Assertions
    mock_host_group_instance.read_products.assert_called_once()
    mock_host_group_instance.read_repos.assert_called_once()
    mock_repoq.solve_repa.assert_called_once_with(repa_instance, "dummy_base")

    expected_ar_cmd = install_command.addcmd.format(
        name="repo1", url="http://repo1.url", params="-cfkn"
    )
    expected_in_cmd = install_command.ipdcmd.format(products="product-to-install")

    mock_target.run.assert_has_calls(
        [
            call(expected_ar_cmd),
            call(install_command.refcmd),
            call(expected_in_cmd),
        ],
        any_order=False,
    )

    mock_host_group_instance.close.assert_called_once()


def _setup_install(monkeypatch, args, repoq_solution, out=None, probe=True):
    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = repoq_solution

    mock_target = MagicMock()
    mock_target.products.get_base.return_value = "dummy_base"
    mock_target.products.is_transactional.return_value = False
    mock_target.out = out if out is not None else _ok_out()
    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["user@host1"]
    mock_hg.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )
    monkeypatch.setattr(Install, "repoq", mock_repoq)
    monkeypatch.setattr(
        repose.command._command,
        "check_repo_url",
        lambda url, *, timeout: probe,
    )
    return mock_target, mock_hg, mock_repoq


def test_install_command_dryrun_skips_run(
    monkeypatch, mock_args_and_repa, capsys, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.dry = True

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"product": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )

    assert Install(args).run() == 0

    target.run.assert_not_called()
    out = capsys.readouterr().out
    assert "repo1" in out
    assert "user@host1" in out


def _make_transactional(target, *, installed_after):
    """Configure a mock target as a transactional host.

    ``installed_after`` is the set of product names the post-reboot
    product re-read should report (so the verify step can pass/fail).
    """
    target.products.is_transactional.return_value = True
    target.reboot.return_value = True
    after = set()
    for name in installed_after:
        p = MagicMock()
        p.name = name
        after.add(p)
    target.products.flatten.return_value = after


def test_install_on_transactional_host_uses_transactional_and_reboots(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    """Any product on a transactional host → transactional-update + reboot,
    regardless of the product name (the host, not the product, decides)."""
    args, _ = mock_args_and_repa

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"qa": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )
    # A non-"SL-Micro" product on a transactional host must still go
    # through transactional-update — this is the core fix.
    _make_transactional(target, installed_after={"qa"})

    assert Install(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    assert any("transactional-update -n pkg in" in cmd for cmd in issued)
    # -n is mandatory: it makes the inner zypper non-interactive, otherwise
    # it hits "Continue? [y/n]" -> EOF -> exit 4 under a non-tty SSH exec.
    assert not any("transactional-update pkg in" in cmd for cmd in issued), (
        "transactional-update must be invoked with -n (non-interactive)"
    )
    assert not any(cmd.startswith("zypper -n in") for cmd in issued)
    target.reboot.assert_called_once()


def test_install_transactional_imports_keys_before_install(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    """On a transactional host the repo signing keys must be imported into
    the snapshot keyring (reftcmd) *before* the product install, otherwise
    the inner zypper rejects the repo signature (exit 4)."""
    args, _ = mock_args_and_repa

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"qa": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )
    _make_transactional(target, installed_after={"qa"})

    assert Install(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    reftcmd = "transactional-update -n run zypper -n --gpg-auto-import-keys ref -f"
    assert reftcmd in issued, "transactional key-import refresh must be issued"
    install_idx = next(
        i for i, c in enumerate(issued) if "transactional-update -n pkg in" in c
    )
    assert issued.index(reftcmd) < install_idx, (
        "key-import refresh must run before the transactional install"
    )


def test_install_non_transactional_skips_key_import_refresh(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    """The transactional key-import refresh is only for transactional hosts."""
    args, _ = mock_args_and_repa

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"qa": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )
    # default _setup_install leaves is_transactional() -> False

    assert Install(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    assert not any("transactional-update run" in cmd for cmd in issued)


def test_install_transactional_verify_fails_when_product_absent(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    """If the product is not present after the reboot, the host fails."""
    args, _ = mock_args_and_repa

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"qa": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )
    _make_transactional(target, installed_after=set())  # qa missing after reboot

    # Single host, verify fails → all hosts failed → exit 2.
    assert Install(args).run() == 2
    target.reboot.assert_called_once()


def test_install_transactional_preserves_earlier_failure(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    """A passing reboot/verify must not mask an earlier failure.

    When one repa fails to resolve (``solve_repa`` raises) but another
    resolves, the product install and a passing reboot/verify must not
    clobber the accumulated ``ok=False`` — the host still fails.
    """
    args, repa_instance = mock_args_and_repa
    other_repa = Repa("other-repa")
    args.repa = [repa_instance, other_repa]

    target, _, repoq = _setup_install(
        monkeypatch,
        args,
        {"qa": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )
    # First repa fails to resolve, second resolves to a product so the
    # transactional install branch is still reached.
    repoq.solve_repa.side_effect = [
        ValueError("Unknow product: X"),
        {"qa": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    ]
    _make_transactional(target, installed_after={"qa"})
    # Reboot/verify passes — it must not reset the earlier failure.
    monkeypatch.setattr(Install, "_reboot_and_verify", lambda self, *a, **k: True)

    # Single host, earlier failure preserved → all hosts failed → exit 2.
    assert Install(args).run() == 2


def test_install_transactional_no_reboot_stages_only(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    """--no-reboot on a transactional host installs but skips reboot/verify."""
    args, _ = mock_args_and_repa
    args.no_reboot = True

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"qa": [MockRepo("repo1", "http://repo1.url", refresh=True)]},
    )
    _make_transactional(target, installed_after=set())

    assert Install(args).run() == 0
    issued = [c.args[0] for c in target.run.call_args_list]
    assert any("transactional-update -n pkg in" in cmd for cmd in issued)
    target.reboot.assert_not_called()


def test_install_command_no_products_logs_error(
    monkeypatch, mock_args_and_repa, caplog, mock_ssh_client
):
    args, _ = mock_args_and_repa

    target, _, repoq = _setup_install(
        monkeypatch,
        args,
        {},  # empty solution
    )

    with caplog.at_level("ERROR", logger="repose.command.install"):
        # Empty solution → "No products to install" error → all hosts
        # failed → exit 2.
        assert Install(args).run() == 2

    assert any("No products to install" in r.message for r in caplog.records)


def test_install_command_dead_probe_skips_ar_but_runs_product_install(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    """New in PR 08: probes that fail filter out the ``ar`` command,
    but the per-product ``in -t product`` step still runs (so a host
    can still install the product from whatever repos zypper already
    knows about)."""
    args, _ = mock_args_and_repa

    target, _, _ = _setup_install(
        monkeypatch,
        args,
        {"product-to-install": [MockRepo("repo1", "http://dead", refresh=False)]},
        probe=False,
    )

    assert Install(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    # ar suppressed by probe filter.
    assert not any(cmd.startswith("zypper -n ar") for cmd in issued)
    # Product install still issued.
    assert any("in -t product" in cmd for cmd in issued)


def test_install_command_solve_repa_value_error_logged(
    monkeypatch, mock_args_and_repa, caplog, mock_ssh_client
):
    args, _ = mock_args_and_repa

    mock_repoq = MagicMock()
    mock_repoq.solve_repa.side_effect = ValueError("Unknow product: X")

    mock_target = MagicMock()
    mock_target.products.get_base.return_value = "dummy_base"
    mock_hg = MagicMock()
    mock_hg.keys.return_value = ["user@host1"]
    mock_hg.__getitem__.return_value = mock_target

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )
    monkeypatch.setattr(Install, "repoq", mock_repoq)
    monkeypatch.setattr(
        repose.command._command, "check_repo_url", lambda url, *, timeout: True
    )

    with caplog.at_level("ERROR", logger="repose.command.install"):
        # solve_repa raises AND no products → exit 2.
        assert Install(args).run() == 2

    assert any("Unknow product" in r.message for r in caplog.records)


# ---------------------------------------------------------------------------
# Exit code propagation (PR 06)
# ---------------------------------------------------------------------------


def _setup_install_multi(monkeypatch, hosts):
    """Multi-host variant: one target per host, each with its own ``out``.

    A single-product repoq solution is shared so all hosts attempt the
    same add+install commands.
    """
    mock_repoq = MagicMock()
    mock_repoq.solve_repa.return_value = {
        "product-to-install": [MockRepo("repo1", "http://repo1.url", refresh=False)]
    }

    targets = {}
    for host, out in hosts.items():
        t = MagicMock()
        t.products.get_base.return_value = "dummy_base"
        t.products.is_transactional.return_value = False
        t.out = out
        targets[host] = t

    hg = MagicMock()
    hg.keys.return_value = list(hosts.keys())
    hg.__getitem__.side_effect = lambda k: targets[k]

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=hg),
    )
    monkeypatch.setattr(Install, "repoq", mock_repoq)
    monkeypatch.setattr(
        repose.command._command, "check_repo_url", lambda url, *, timeout: True
    )
    return targets, hg


def test_install_run_returns_0_when_all_succeed(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.target = [{"h1": MagicMock(), "h2": MagicMock()}]
    _setup_install_multi(monkeypatch, {"h1": _ok_out(), "h2": _ok_out()})

    assert Install(args).run() == 0


def test_install_run_returns_1_on_partial_failure(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.target = [{"h1": MagicMock(), "h2": MagicMock()}]
    _setup_install_multi(monkeypatch, {"h1": _ok_out(), "h2": _fail_out()})

    assert Install(args).run() == 1


def test_install_run_returns_2_when_all_fail(
    monkeypatch, mock_args_and_repa, mock_ssh_client
):
    args, _ = mock_args_and_repa
    args.target = [{"h1": MagicMock(), "h2": MagicMock()}]
    _setup_install_multi(monkeypatch, {"h1": _fail_out(), "h2": _fail_out()})

    assert Install(args).run() == 2
