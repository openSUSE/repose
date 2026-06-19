import concurrent.futures
from argparse import Namespace
from unittest.mock import MagicMock, call

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.uninstall import Uninstall
from repose.types.repa import Repa


class MockRepo:
    def __init__(self, name):
        self.name = name


class MockProduct:
    def __init__(self, name, version):
        self.name = name
        self.version = version


def _ok_out():
    return [["cmd", "", "", 0, 0]]


def _fail_out():
    return [["cmd", "", "boom", 1, 0]]


@pytest.fixture
def mock_args_and_repa():
    repa_instance = Repa("SLES:15-SP4")
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[repa_instance],
        config="dummy_config",
        yaml=False,
    )
    return args, repa_instance


def test_uninstall_command_run(monkeypatch, mock_args_and_repa, mock_ssh_client):
    mock_args, repa_instance = mock_args_and_repa

    # Mocks
    mock_target = MagicMock()
    mock_target.products.flatten.return_value = [MockProduct("SLES", "15-SP4")]
    mock_target.products.is_transactional.return_value = False
    mock_target.repos = {
        "SLES:15-SP4::repo1": MockRepo(name="SLES"),
        "SLES:15-SP4::repo2": MockRepo(name="SLES"),
        "other:repo": MockRepo(name="other"),
    }
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

    # Run
    uninstall_command = Uninstall(mock_args)
    assert uninstall_command.run() == 0

    # Assertions
    mock_host_group_instance.read_repos.assert_called_once()
    mock_host_group_instance.parse_repos.assert_called_once()

    rm_cmd = uninstall_command.rrpcmd.format(products="SLES")

    run_calls = mock_target.run.call_args_list
    # The order of the repository names in the rr command is not guaranteed.
    first_call_args, _ = run_calls[0]
    first_cmd = first_call_args[0]
    assert first_cmd.startswith("zypper -n rr")
    assert "SLES:15-SP4::repo1" in first_cmd
    assert "SLES:15-SP4::repo2" in first_cmd

    # The second call should be the rm command
    assert run_calls[1] == call(rm_cmd)

    mock_host_group_instance.close.assert_called_once()


def _setup_uninstall(monkeypatch, args, products, repos, out=None, hosts=None):
    if hosts is None:
        hosts = ["user@host1"]
    mock_target = MagicMock()
    mock_target.products.flatten.return_value = products
    mock_target.products.is_transactional.return_value = False
    mock_target.repos = repos
    mock_target.out = out if out is not None else _ok_out()
    mock_hg = MagicMock()
    mock_hg.keys.return_value = hosts
    mock_hg.__getitem__.return_value = mock_target
    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)
    monkeypatch.setattr(
        repose.command._command,
        "HostGroup",
        MagicMock(return_value=mock_hg),
    )
    return mock_target, mock_hg


def test_uninstall_dryrun_does_not_run(monkeypatch, capsys, mock_ssh_client):
    args = Namespace(
        dry=True,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("SLES", "15-SP4")],
        {"SLES:15-SP4::repo1": MockRepo("SLES")},
    )

    assert Uninstall(args).run() == 0

    target.run.assert_not_called()
    out = capsys.readouterr().out
    assert "user@host1" in out


def test_uninstall_no_patterns_logs(monkeypatch, caplog, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("OTHER:99")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("SLES", "15-SP4")],
        {"SLES:15-SP4::repo1": MockRepo("SLES")},
    )

    with caplog.at_level("INFO", logger="repose.command.uninstall"):
        # No matching pattern → INFO no-op → exit 0.
        assert Uninstall(args).run() == 0

    target.run.assert_not_called()
    assert any("no products for remove" in r.message for r in caplog.records)


def test_uninstall_no_matching_repos_runs_only_pdcmd(monkeypatch, mock_ssh_client):
    """Patterns match but no repos in dict → rrcmd skipped, only pdcmd runs."""
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("SLES", "15-SP4")],
        {},  # no repositories at all
    )

    assert Uninstall(args).run() == 0

    # Only one command issued: rrpcmd (no rrcmd because no rdict)
    assert target.run.call_count == 1
    assert "rm -t product" in target.run.call_args[0][0]


def test_uninstall_skips_repo_with_unparseable_name(monkeypatch, mock_ssh_client):
    """A repo whose name isn't a 4-part product must not crash uninstall.

    ``Repositories`` stores a ``(None, None)`` sentinel for such repos.
    When its alias still matches a removal pattern, ``_calculate_repodict``
    used to dereference ``.name`` on the sentinel and raise
    ``AttributeError``. It must instead skip the unmappable repo and
    remove only the genuine product repos.
    """
    from collections import namedtuple

    from repose.types.repositories import Repositories

    RawRepo = namedtuple("RawRepo", "alias name")
    repos = Repositories(
        [
            # 4-part name -> parsed into a Product
            RawRepo("SLES:15-SP4::repo1", "SLES:15-SP4:x86_64:pool"),
            # alias matches the pattern, but the name isn't a product
            # -> stored as the (None, None) sentinel
            RawRepo("SLES:15-SP4::weird", "SLES-15-SP4-weird"),
        ],
        "x86_64",
    )

    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("SLES", "15-SP4")],
        repos,
    )

    assert Uninstall(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    rr = next(cmd for cmd in issued if cmd.startswith("zypper -n rr"))
    # Only the product-mapped repo is removed; the sentinel one is skipped.
    assert "SLES:15-SP4::repo1" in rr
    assert "SLES:15-SP4::weird" not in rr


def test_uninstall_on_transactional_host_uses_transactional_and_reboots(
    monkeypatch, mock_ssh_client
):
    """Uninstall on a transactional host → transactional-update rm + reboot,
    then verify the product is gone. Decided by the host, not the product."""
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("qa:6.0")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("qa", "6.0")],
        {"qa:6.0::repo1": MockRepo("qa")},
    )
    target.products.is_transactional.return_value = True
    target.reboot.return_value = True
    # First flatten() drives pattern calc (product present); the second,
    # after the reboot, is the verify and must show the product gone.
    target.products.flatten.side_effect = [
        [MockProduct("qa", "6.0")],
        [],
    ]

    assert Uninstall(args).run() == 0

    issued = [c.args[0] for c in target.run.call_args_list]
    assert any(cmd.startswith("zypper -n rr") for cmd in issued)
    assert any(
        "transactional-update pkg rm -t product" in cmd and "qa" in cmd
        for cmd in issued
    )
    target.reboot.assert_called_once()


def test_uninstall_transactional_verify_fails_when_still_present(
    monkeypatch, mock_ssh_client
):
    """If the product is still present after the reboot, the host fails."""
    args = Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        repa=[Repa("qa:6.0")],
        config="dummy",
        yaml=False,
    )
    target, _ = _setup_uninstall(
        monkeypatch,
        args,
        [MockProduct("qa", "6.0")],
        {"qa:6.0::repo1": MockRepo("qa")},
    )
    target.products.is_transactional.return_value = True
    target.reboot.return_value = True
    # Still present after reboot → verify fails.
    target.products.flatten.side_effect = [
        [MockProduct("qa", "6.0")],
        [MockProduct("qa", "6.0")],
    ]

    assert Uninstall(args).run() == 2
    target.reboot.assert_called_once()


# ---------------------------------------------------------------------------
# Exit code propagation (PR 06)
# ---------------------------------------------------------------------------


def _setup_uninstall_multi(monkeypatch, hosts):
    targets = {}
    for host, out in hosts.items():
        t = MagicMock()
        t.products.flatten.return_value = [MockProduct("SLES", "15-SP4")]
        t.products.is_transactional.return_value = False
        t.repos = {"SLES:15-SP4::repo1": MockRepo("SLES")}
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
    return targets, hg


def test_uninstall_run_returns_0_when_all_succeed(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    _setup_uninstall_multi(monkeypatch, {"h1": _ok_out(), "h2": _ok_out()})

    assert Uninstall(args).run() == 0


def test_uninstall_run_returns_1_on_partial_failure(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    _setup_uninstall_multi(monkeypatch, {"h1": _ok_out(), "h2": _fail_out()})

    assert Uninstall(args).run() == 1


def test_uninstall_run_returns_2_when_all_fail(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        repa=[Repa("SLES:15-SP4")],
        config="dummy",
        yaml=False,
    )
    _setup_uninstall_multi(monkeypatch, {"h1": _fail_out(), "h2": _fail_out()})

    assert Uninstall(args).run() == 2
