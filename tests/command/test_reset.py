import concurrent.futures
from argparse import Namespace
from unittest.mock import AsyncMock, MagicMock

import pytest
from conftest import ImmediateExecutor

import repose.command._command
from repose.command.reset import Reset
from repose.messages import UnsuportedProductMessage


class MockRepo:
    def __init__(self, name, url, refresh=False):
        self.name = name
        self.url = url
        self.refresh = refresh


class MockRawRepo:
    def __init__(self, alias):
        self.alias = alias


class MockProduct:
    def __init__(self, name, version):
        self.name = name
        self.version = version


def _ok_out():
    return [["cmd", "", "", 0, 0]]


def _fail_out():
    return [["cmd", "", "boom", 1, 0]]


@pytest.fixture
def mock_args():
    """Fixture for command arguments."""
    return Namespace(
        dry=False,
        target=[{"user@host1": MagicMock()}],
        config="dummy_config",
        repa=None,
        yaml=False,
    )


def test_reset_command_run(monkeypatch, mock_args, mock_ssh_client):
    # Setup Mocks
    mock_repoq = MagicMock()
    mock_repoq.solve_product.return_value = {
        "product": [
            MockRepo("prod-repo1", "http://prod-repo1.url", refresh=True),
        ]
    }

    mock_target = MagicMock()
    mock_target.products = [MockProduct("SLES", "15-SP4")]
    mock_target.raw_repos = [
        MockRawRepo("existing-repo1"),
        MockRawRepo("existing-repo2"),
    ]
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
    monkeypatch.setattr(Reset, "repoq", mock_repoq)
    monkeypatch.setattr(
        repose.command._command, "check_repo_url", lambda url, *, timeout: True
    )

    # Instantiate and Run
    reset_command = Reset(mock_args)
    assert reset_command.run() == 0

    # Assertions
    mock_host_group_instance.read_products.assert_called_once()
    mock_host_group_instance.read_repos.assert_called_once()

    run_calls = mock_target.run.call_args_list
    assert len(run_calls) == 2

    rr_call_args, _ = run_calls[0]
    rr_cmd = rr_call_args[0]
    assert rr_cmd.startswith("zypper -n rr")
    assert "existing-repo1" in rr_cmd
    assert "existing-repo2" in rr_cmd

    ar_call_args, _ = run_calls[1]
    expected_ar_cmd = reset_command.addcmd.format(
        name="prod-repo1", url="http://prod-repo1.url", params="-cfkn"
    )
    assert ar_call_args[0] == expected_ar_cmd

    mock_host_group_instance.close.assert_called_once()


def _setup_reset(
    monkeypatch,
    args,
    repoq_solution=None,
    raw_repos=None,
    products=None,
    probe=True,
    solve_side_effect=None,
    out=None,
):
    mock_repoq = MagicMock()
    if solve_side_effect is not None:
        mock_repoq.solve_product.side_effect = solve_side_effect
    else:
        mock_repoq.solve_product.return_value = repoq_solution

    mock_target = MagicMock()
    mock_target.products = products or [MockProduct("SLES", "15-SP4")]
    # ``raw_repos=[]`` is meaningful (host without repos) — only ``None``
    # falls back to the default.
    if raw_repos is None:
        raw_repos = [MockRawRepo("existing-repo1")]
    mock_target.raw_repos = raw_repos
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
    monkeypatch.setattr(Reset, "repoq", mock_repoq)
    monkeypatch.setattr(
        repose.command._command,
        "check_repo_url",
        lambda url, *, timeout: probe,
    )
    return mock_target, mock_hg, mock_repoq


def test_reset_dryrun_does_not_run(monkeypatch, mock_args, capsys, mock_ssh_client):
    mock_args.dry = True
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        repoq_solution={"product": [MockRepo("repo1", "http://r1", refresh=True)]},
    )

    assert Reset(mock_args).run() == 0

    target.run.assert_not_called()
    out = capsys.readouterr().out
    assert "user@host1" in out
    assert "existing-repo1" in out


def test_reset_unsupported_product_logs_error(
    monkeypatch, mock_args, caplog, mock_ssh_client
):
    UnknownProd = type("P", (), {"name": "X", "version": "1"})
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        solve_side_effect=UnsuportedProductMessage(UnknownProd()),
    )

    with caplog.at_level("ERROR", logger="repose.command.reset"):
        # UnsuportedProductMessage on the only host → exit 2.
        assert Reset(mock_args).run() == 2

    assert any("Refhost" in r.message for r in caplog.records)
    # _add() raised before reaching the run() block — no commands executed.
    target.run.assert_not_called()


def test_reset_dead_probe_aborts_without_removal(
    monkeypatch, mock_args, caplog, mock_ssh_client
):
    """A transient probe failure must not wipe the host's repos.

    When every replacement URL fails the live probe the resolved
    command set is empty; ``rr`` must NOT run (otherwise the host is
    left with zero repositories) and the host is reported failed.
    """
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        repoq_solution={"product": [MockRepo("r1", "http://bad", refresh=False)]},
        probe=False,
    )

    with caplog.at_level("ERROR", logger="repose.command.reset"):
        # Single host, aborted before removal → all-fail → exit 2.
        assert Reset(mock_args).run() == 2

    # Nothing was executed: neither the destructive rr nor any ar.
    target.run.assert_not_called()
    assert any("Refhost" in r.message for r in caplog.records)


# ---------------------------------------------------------------------------
# Exit code propagation (PR 06)
# ---------------------------------------------------------------------------


def _setup_reset_multi(monkeypatch, hosts):
    mock_repoq = MagicMock()
    mock_repoq.solve_product.return_value = {
        "product": [MockRepo("repo1", "http://repo1.url", refresh=False)]
    }

    targets = {}
    for host, out in hosts.items():
        t = MagicMock()
        t.products = [MockProduct("SLES", "15-SP4")]
        t.raw_repos = [MockRawRepo(f"{host}-existing")]
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
    monkeypatch.setattr(Reset, "repoq", mock_repoq)
    monkeypatch.setattr(
        repose.command._command, "check_repo_url", lambda url, *, timeout: True
    )
    return targets, hg


def test_reset_run_returns_0_when_all_succeed(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        config="dummy_config",
        repa=None,
        yaml=False,
    )
    _setup_reset_multi(monkeypatch, {"h1": _ok_out(), "h2": _ok_out()})

    assert Reset(args).run() == 0


def test_reset_run_returns_1_on_partial_failure(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        config="dummy_config",
        repa=None,
        yaml=False,
    )
    _setup_reset_multi(monkeypatch, {"h1": _ok_out(), "h2": _fail_out()})

    assert Reset(args).run() == 1


def test_reset_run_returns_2_when_all_fail(monkeypatch, mock_ssh_client):
    args = Namespace(
        dry=False,
        target=[{"h1": MagicMock(), "h2": MagicMock()}],
        config="dummy_config",
        repa=None,
        yaml=False,
    )
    _setup_reset_multi(monkeypatch, {"h1": _fail_out(), "h2": _fail_out()})

    assert Reset(args).run() == 2


# ---------------------------------------------------------------------------
# Async path: abort-on-empty-replacement regression
# ---------------------------------------------------------------------------


async def test_arun_one_aborts_when_no_live_replacement(mock_args, caplog):
    """Async ``_arun_one`` must mirror the sync abort-on-empty guard.

    With no live replacement repos it must not run the destructive
    ``rr`` and must report the host as failed.
    """
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1")]
    target.out = _ok_out()
    target.run = AsyncMock()

    reset.targets = {"user@host1": target}
    # Empty replacement set == every probe failed.
    reset._aadd = AsyncMock(return_value=(set(), ["r1"]))

    with caplog.at_level("ERROR", logger="repose.command.reset"):
        ok = await reset._arun_one("user@host1", lambda host, msg: None)

    assert ok is False
    target.run.assert_not_awaited()
    assert any("Refhost" in r.message for r in caplog.records)


def test_reset_partial_probe_drop_aborts_without_removal(
    monkeypatch, mock_args, caplog, mock_ssh_client
):
    """A partial probe failure must not silently lose repos.

    When the live-URL probe drops a proper subset of the resolved
    replacement repositories the destructive ``rr`` must NOT run and
    the host is reported failed — otherwise the dropped repos are
    permanently gone after a transient mirror blip while the survivors
    are re-added and the host is still reported as success.
    """
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        repoq_solution={
            "product": [
                MockRepo("good", "http://good", refresh=False),
                MockRepo("bad", "http://bad", refresh=False),
            ]
        },
    )
    # One mirror is alive, the other fails the probe → partial drop.
    monkeypatch.setattr(
        repose.command._command,
        "check_repo_url",
        lambda url, *, timeout: url != "http://bad",
    )

    with caplog.at_level("ERROR", logger="repose.command.reset"):
        # Single host aborted before removal → all-fail → exit 2.
        assert Reset(mock_args).run() == 2

    # Neither the destructive rr nor any ar ran.
    target.run.assert_not_called()
    assert any("bad" in r.getMessage() for r in caplog.records)


def test_reset_dryrun_predicts_partial_drop_abort(
    monkeypatch, mock_args, caplog, mock_ssh_client
):
    """``--dry`` must predict the real abort on a partial drop, not show
    a destructive plan that never executes."""
    mock_args.dry = True
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        repoq_solution={
            "product": [
                MockRepo("good", "http://good", refresh=False),
                MockRepo("bad", "http://bad", refresh=False),
            ]
        },
    )
    monkeypatch.setattr(
        repose.command._command,
        "check_repo_url",
        lambda url, *, timeout: url != "http://bad",
    )

    with caplog.at_level("ERROR", logger="repose.command.reset"):
        # Dry-run aborts (exit 2), matching the real run.
        assert Reset(mock_args).run() == 2
    target.run.assert_not_called()
    assert any("bad" in r.getMessage() for r in caplog.records)


def test_reset_empty_current_repos_skips_rr_still_adds(
    monkeypatch, mock_args, caplog, mock_ssh_client
):
    """A host whose current repo list is empty must not issue a bare ``rr``.

    Pre-fix the removal step ran as ``zypper -n rr`` with no argument,
    which zypper rejects non-zero, so the exit-code classifier reported
    the no-op host as failed. The replacement repos must still be added
    and the host reported ok.
    """
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        repoq_solution={"product": [MockRepo("repo1", "http://r1", refresh=False)]},
        raw_repos=[],
    )

    with caplog.at_level("INFO", logger="repose.command.reset"):
        assert Reset(mock_args).run() == 0

    run_calls = target.run.call_args_list
    assert len(run_calls) == 1
    assert run_calls[0][0][0].startswith("zypper -n ar")
    assert any("No repositories to clear" in r.message for r in caplog.records)


def test_reset_dryrun_empty_current_repos_skips_rr_preview(
    monkeypatch, mock_args, capsys, mock_ssh_client
):
    """``--dry`` on a repo-less host must not preview a bare ``zypper -n rr``.

    The dry-run must predict the real run, which skips the removal step
    entirely and only adds the replacement repos.
    """
    mock_args.dry = True
    target, _, _ = _setup_reset(
        monkeypatch,
        mock_args,
        repoq_solution={"product": [MockRepo("repo1", "http://r1", refresh=False)]},
        raw_repos=[],
    )

    assert Reset(mock_args).run() == 0

    target.run.assert_not_called()
    out = capsys.readouterr().out
    assert "zypper -n rr" not in out
    assert "zypper -n ar" in out


async def test_reset_arun_one_empty_current_repos_skips_rr(mock_args, caplog):
    """Async ``_arun_one`` must mirror the sync no-current-repos guard."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = []
    target.out = _ok_out()
    target.run = AsyncMock()

    reset.targets = {"user@host1": target}
    reset._aadd = AsyncMock(
        return_value=({"zypper -n ar -ckn repo1 http://r1 repo1"}, [])
    )

    with caplog.at_level("INFO", logger="repose.command.reset"):
        ok = await reset._arun_one("user@host1", lambda host, msg: None)

    assert ok is True
    assert target.run.await_count == 1
    assert target.run.await_args_list[0][0][0].startswith("zypper -n ar")
    assert any("No repositories to clear" in r.message for r in caplog.records)


async def test_reset_arun_one_dryrun_empty_current_repos_skips_rr_preview(mock_args):
    """Async dry-run must mirror the sync guard: no bare ``rr`` preview."""
    mock_args.ssh_backend = "asyncssh"
    mock_args.dry = True
    reset = Reset(mock_args)
    reset.console = MagicMock()

    target = MagicMock()
    target.raw_repos = []
    target.out = _ok_out()
    target.run = AsyncMock()

    reset.targets = {"user@host1": target}
    ar_cmd = "zypper -n ar -ckn repo1 http://r1 repo1"
    reset._aadd = AsyncMock(return_value=({ar_cmd}, []))

    ok = await reset._arun_one("user@host1", lambda host, msg: None)

    assert ok is True
    target.run.assert_not_awaited()
    previews = [c.args[1] for c in reset.console.dry.call_args_list]
    assert previews == [ar_cmd]


async def test_arun_one_aborts_on_partial_probe_drop(mock_args, caplog):
    """Async ``_arun_one`` must mirror the sync partial-drop guard.

    With a survivor *and* a dropped candidate it must not run the
    destructive ``rr`` and must report the host as failed.
    """
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1")]
    target.out = _ok_out()
    target.run = AsyncMock()

    reset.targets = {"user@host1": target}
    # One survivor command, one candidate dropped by the probe.
    reset._aadd = AsyncMock(return_value=({"ar good"}, ["bad"]))

    with caplog.at_level("ERROR", logger="repose.command.reset"):
        ok = await reset._arun_one("user@host1", lambda host, msg: None)

    assert ok is False
    target.run.assert_not_awaited()
    assert any("bad" in r.getMessage() for r in caplog.records)


# ---------------------------------------------------------------------------
# Behaviour pinning: _add / _run / _arun_one command + argument wiring
# ---------------------------------------------------------------------------


def _rr_orderings(*aliases):
    """Both valid ``rr`` renderings (repoaliases come from an unordered set)."""
    a, b = aliases
    return {
        f"zypper -n rr {a} {b}",
        f"zypper -n rr {b} {a}",
    }


def test_add_solves_products_and_builds_add_cmds(monkeypatch, mock_args):
    """``_add`` must solve the host's own products and render ``-ckn`` for a
    non-refresh repo (pins the ``solve_product`` arg and the params literal)."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    repo = MockRepo("prod-repo1", "http://prod-repo1.url", refresh=False)
    products = [MockProduct("SLES", "15-SP4")]
    mock_repoq = MagicMock()
    mock_repoq.solve_product.return_value = {"product": [repo]}
    monkeypatch.setattr(Reset, "repoq", mock_repoq)

    target = MagicMock()
    target.products = products
    reset.targets = {"user@host1": target}
    reset._filter_live_urls = MagicMock(return_value=[repo])

    cmds, dropped = reset._add("user@host1")

    # Products of the addressed host, not None, drive the solve.
    mock_repoq.solve_product.assert_called_once_with(products)
    assert dropped == []
    expected = reset.addcmd.format(
        name="prod-repo1", url="http://prod-repo1.url", params="-ckn"
    )
    assert cmds == {expected}


def test_run_success_pins_updates_and_run_commands(mock_args):
    """Sync ``_run`` success path: progress messages carry the real host and
    exact text, ``rr`` joins aliases with a single space, and each add cmd is
    executed verbatim."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1"), MockRawRepo("existing-repo2")]
    target.out = _ok_out()
    reset.targets = {"user@host1": target}
    reset._add = MagicMock(return_value=({"ar cmd1"}, []))
    reset._report_target = MagicMock(return_value=True)

    updates = []
    ok = reset._run("user@host1", lambda host, msg: updates.append((host, msg)))

    assert ok is True
    assert ("user@host1", "clearing repos") in updates
    assert ("user@host1", "resolving new repos") in updates
    assert ("user@host1", "re-adding 1 repo(s)") in updates

    run_calls = target.run.call_args_list
    assert run_calls[0].args[0] in _rr_orderings("existing-repo1", "existing-repo2")
    assert run_calls[1].args == ("ar cmd1",)


def test_run_report_failure_sets_ok_false(mock_args):
    """Sync ``_run``: a failed ``_report_target`` after a real run must flip the
    aggregate result to ``False`` (pins the ``ok = False`` assignment)."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1")]
    target.out = _fail_out()
    reset.targets = {"user@host1": target}
    reset._add = MagicMock(return_value=({"ar cmd1"}, []))
    reset._report_target = MagicMock(return_value=False)

    ok = reset._run("user@host1", lambda host, msg: None)

    assert ok is False


def test_run_rr_report_failure_alone_sets_ok_false(mock_args):
    """Sync ``_run``: when the post-``rr`` report FAILS but the add-cmd
    report SUCCEEDS, the aggregate must still be ``False``. This isolates
    the first ``ok = False`` (after the destructive removal) so it is the
    sole determinant — a failed removal report must never be masked by a
    later successful add report."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1")]
    reset.targets = {"user@host1": target}
    reset._add = MagicMock(return_value=({"ar cmd1"}, []))
    # rr report fails first, the add-cmd report then succeeds.
    reset._report_target = MagicMock(side_effect=[False, True])

    ok = reset._run("user@host1", lambda host, msg: None)

    assert ok is False


def test_run_dryrun_pins_console_dry_calls(mock_args):
    """Sync ``_run`` dry path previews the exact ``rr`` (single-space join) and
    each add cmd against the real host via ``console.dry``."""
    mock_args.ssh_backend = "asyncssh"
    mock_args.dry = True
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1"), MockRawRepo("existing-repo2")]
    reset.targets = {"user@host1": target}
    reset._add = MagicMock(return_value=({"ar cmd1"}, []))
    reset.console = MagicMock()

    ok = reset._run("user@host1", lambda host, msg: None)

    assert ok is True
    target.run.assert_not_called()
    dry_calls = reset.console.dry.call_args_list
    assert dry_calls[0].args[0] == "user@host1"
    assert dry_calls[0].args[1] in _rr_orderings("existing-repo1", "existing-repo2")
    assert dry_calls[1].args == ("user@host1", "ar cmd1")


async def test_arun_one_success_pins_updates_and_run_commands(mock_args):
    """Async ``_arun_one`` success path mirrors ``_run``: correct progress
    messages, ``_aadd`` addressed to the host, a single-space ``rr`` join,
    each add cmd awaited verbatim, ``_report_target`` called with the host,
    and a ``True`` result when every report succeeds."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1"), MockRawRepo("existing-repo2")]
    target.out = _ok_out()
    target.run = AsyncMock()
    reset.targets = {"user@host1": target}
    reset._aadd = AsyncMock(return_value=({"ar cmd1"}, []))
    reset._report_target = MagicMock(return_value=True)

    updates = []
    ok = await reset._arun_one(
        "user@host1", lambda host, msg: updates.append((host, msg))
    )

    assert ok is True
    assert ("user@host1", "clearing repos") in updates
    assert ("user@host1", "resolving new repos") in updates
    assert ("user@host1", "re-adding 1 repo(s)") in updates

    reset._aadd.assert_awaited_once_with("user@host1")
    reset._report_target.assert_any_call("user@host1")

    await_calls = target.run.await_args_list
    assert await_calls[0].args[0] in _rr_orderings("existing-repo1", "existing-repo2")
    assert await_calls[1].args == ("ar cmd1",)


async def test_arun_one_report_failure_sets_ok_false(mock_args):
    """Async ``_arun_one``: a failed ``_report_target`` after a real run must
    flip the aggregate result to ``False``."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1")]
    target.out = _fail_out()
    target.run = AsyncMock()
    reset.targets = {"user@host1": target}
    reset._aadd = AsyncMock(return_value=({"ar cmd1"}, []))
    reset._report_target = MagicMock(return_value=False)

    ok = await reset._arun_one("user@host1", lambda host, msg: None)

    assert ok is False


async def test_arun_one_rr_report_failure_alone_sets_ok_false(mock_args):
    """Async ``_arun_one``: when the post-``rr`` report FAILS but the
    add-cmd report SUCCEEDS, the aggregate must still be ``False`` —
    isolates the first ``ok = False`` after the destructive removal so it
    is the sole determinant and cannot be masked by a later success."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1")]
    target.run = AsyncMock()
    reset.targets = {"user@host1": target}
    reset._aadd = AsyncMock(return_value=({"ar cmd1"}, []))
    # rr report fails first, the add-cmd report then succeeds.
    reset._report_target = MagicMock(side_effect=[False, True])

    ok = await reset._arun_one("user@host1", lambda host, msg: None)

    assert ok is False


async def test_arun_one_unsupported_product_sets_ok_false(mock_args, caplog):
    """Async ``_arun_one``: an ``UnsuportedProductMessage`` raised by
    ``_aadd`` must be caught, logged, and flip the result to ``False`` —
    the async sibling of ``test_reset_unsupported_product_logs_error``,
    exercising the ``except`` branch on the async path."""
    mock_args.ssh_backend = "asyncssh"
    reset = Reset(mock_args)

    unknown_prod = type("P", (), {"name": "X", "version": "1"})
    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1")]
    target.run = AsyncMock()
    reset.targets = {"user@host1": target}
    reset._aadd = AsyncMock(side_effect=UnsuportedProductMessage(unknown_prod()))

    with caplog.at_level("ERROR", logger="repose.command.reset"):
        ok = await reset._arun_one("user@host1", lambda host, msg: None)

    assert ok is False
    target.run.assert_not_awaited()
    assert any("Refhost" in r.message for r in caplog.records)


async def test_arun_one_dryrun_pins_console_dry_calls(mock_args):
    """Async ``_arun_one`` dry path previews the exact ``rr`` and add cmds via
    ``console.dry`` against the real host and does not touch the network."""
    mock_args.ssh_backend = "asyncssh"
    mock_args.dry = True
    reset = Reset(mock_args)

    target = MagicMock()
    target.raw_repos = [MockRawRepo("existing-repo1"), MockRawRepo("existing-repo2")]
    target.run = AsyncMock()
    reset.targets = {"user@host1": target}
    reset._aadd = AsyncMock(return_value=({"ar cmd1"}, []))
    reset.console = MagicMock()

    ok = await reset._arun_one("user@host1", lambda host, msg: None)

    assert ok is True
    target.run.assert_not_awaited()
    dry_calls = reset.console.dry.call_args_list
    assert dry_calls[0].args[0] == "user@host1"
    assert dry_calls[0].args[1] in _rr_orderings("existing-repo1", "existing-repo2")
    assert dry_calls[1].args == ("user@host1", "ar cmd1")
