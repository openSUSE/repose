"""Tests for ``repose.command._command.Command`` base class."""

from concurrent.futures import Future
from unittest.mock import MagicMock, call

import pytest

import repose.command._command as cmdmod
from repose.command._command import Command


class _Concrete(Command):
    command = True

    def run(self):
        return 0


def _make(monkeypatch, args_overrides=None, host_group=None):
    """Build a concrete Command instance with mocked HostGroup."""
    if host_group is None:
        host_group = MagicMock()
        host_group.keys.return_value = []
    monkeypatch.setattr(cmdmod, "HostGroup", MagicMock(return_value=host_group))

    from argparse import Namespace

    defaults = {
        "dry": False,
        "target": [{"h1": MagicMock()}],
        "config": "p",
        "yaml": False,
        "repa": [],
    }
    if args_overrides:
        defaults.update(args_overrides)
    return _Concrete(Namespace(**defaults)), host_group


# ---------------------------------------------------------------------------
# _filter_live_urls
# ---------------------------------------------------------------------------


class _StubRepo:
    """Minimal stand-in matching the ``.url`` attribute consumed by the
    helper. Real ``Repository`` instances carry more state, but the
    filter doesn't touch any of it."""

    def __init__(self, url: str) -> None:
        self.url = url


def test_filter_live_urls_returns_only_live(monkeypatch):
    """Each repo gets probed once; only those returning True survive,
    and the relative ordering is preserved."""
    calls: list[str] = []

    def _probe(url, *, timeout):
        calls.append(url)
        return url.endswith("/live/")

    monkeypatch.setattr(cmdmod, "check_repo_url", _probe)
    cmd, _ = _make(monkeypatch)

    repos = [
        _StubRepo("http://a/dead/"),
        _StubRepo("http://b/live/"),
        _StubRepo("http://c/dead/"),
        _StubRepo("http://d/live/"),
    ]
    live = cmd._filter_live_urls(repos)

    # One probe per repo (parallelism doesn't change call count).
    assert sorted(calls) == sorted(r.url for r in repos)
    # Order preserved among survivors.
    assert [r.url for r in live] == ["http://b/live/", "http://d/live/"]


def test_filter_live_urls_short_circuits_when_no_probe(monkeypatch):
    """``--no-probe`` returns the list unchanged and never calls the
    probe helper."""
    calls: list[str] = []

    def _probe(url, *, timeout):
        calls.append(url)
        return False

    monkeypatch.setattr(cmdmod, "check_repo_url", _probe)
    cmd, _ = _make(monkeypatch, args_overrides={"no_probe": True})

    repos = [_StubRepo("http://a/"), _StubRepo("http://b/")]
    assert cmd._filter_live_urls(repos) == repos
    assert calls == []


def test_filter_live_urls_forwards_probe_timeout(monkeypatch):
    """Custom ``--probe-timeout`` reaches the probe helper kwarg."""
    seen: list[float] = []

    def _probe(url, *, timeout):
        seen.append(timeout)
        return True

    monkeypatch.setattr(cmdmod, "check_repo_url", _probe)
    cmd, _ = _make(monkeypatch, args_overrides={"probe_timeout": 0.25})

    cmd._filter_live_urls([_StubRepo("http://a/"), _StubRepo("http://b/")])
    assert seen == [0.25, 0.25]


def test_filter_live_urls_default_timeout_is_5_seconds(monkeypatch):
    """Without an explicit flag the default 5s budget is used."""
    seen: list[float] = []

    def _probe(url, *, timeout):
        seen.append(timeout)
        return True

    monkeypatch.setattr(cmdmod, "check_repo_url", _probe)
    cmd, _ = _make(monkeypatch)

    cmd._filter_live_urls([_StubRepo("http://a/")])
    assert seen == [5.0]


def test_filter_live_urls_empty_input_is_noop(monkeypatch):
    """Empty input never spawns a pool or calls the probe helper."""
    calls: list[str] = []

    def _probe(url, *, timeout):
        calls.append(url)
        return True

    monkeypatch.setattr(cmdmod, "check_repo_url", _probe)
    cmd, _ = _make(monkeypatch)

    assert cmd._filter_live_urls([]) == []
    assert calls == []


# ---------------------------------------------------------------------------
# Filtering of unconnected targets
# ---------------------------------------------------------------------------


def test_unconnected_targets_are_dropped(monkeypatch):
    # HostGroup behaves like a dict with two hosts; one is "connected"
    # (truthy mock) and the other is falsy (not connected).
    connected = MagicMock()
    connected.__bool__ = lambda self: True
    disconnected = MagicMock()
    disconnected.__bool__ = lambda self: False

    hg = MagicMock()
    hg.keys.return_value = ["a", "b"]
    hg.__getitem__.side_effect = lambda k: {
        "a": connected,
        "b": disconnected,
    }[k]
    deletions = []
    hg.__delitem__.side_effect = lambda k: deletions.append(k)

    cmd, _ = _make(monkeypatch, host_group=hg)
    assert "b" in deletions
    assert "a" not in deletions


# ---------------------------------------------------------------------------
# _report_target
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "exitcode,expected_stream,expected_ok",
    [
        (0, "stdout", True),  # logger.info on stdout, success
        (4, "stdout", True),  # logger.warning on stdout, benign success
        (1, "stderr", False),  # logger.warning on stderr, failure
        (-1, "stderr", False),
    ],
)
def test_report_target_routes_by_exitcode(
    monkeypatch, exitcode, expected_stream, expected_ok
):
    """_report_target picks the right output stream depending on exitcode
    and returns ``True`` for success/benign exits, ``False`` for failures."""
    target = MagicMock()
    # out is list of [cmd, stdout, stderr, exitcode, runtime]
    target.out = [["cmd", "out-line", "err-line", exitcode, 0]]

    hg = MagicMock()
    hg.keys.return_value = ["host"]
    hg.__getitem__.return_value = target

    cmd, _ = _make(monkeypatch, host_group=hg)
    assert cmd._report_target("host") is expected_ok


def test_repoq_loads_template(monkeypatch, tmp_path):
    yml = tmp_path / "products.yml"
    yml.write_text("SLES:\n  '15':\n    default_repos:\n      - update\n")

    cmd, _ = _make(
        monkeypatch,
        args_overrides={"config": yml},
    )
    assert "SLES" in cmd.repoq.template


# ---------------------------------------------------------------------------
# _run_parallel
# ---------------------------------------------------------------------------


def test_run_parallel_invokes_fn_per_host(monkeypatch):
    """Helper fans ``fn(host)`` across every live target and returns
    one completed ``Future`` per host."""
    hg = MagicMock()
    hg.keys.return_value = ["h1", "h2", "h3"]
    hg.__getitem__.side_effect = lambda k: MagicMock()

    cmd, _ = _make(monkeypatch, host_group=hg)

    fn = MagicMock(return_value=None)
    futures = cmd._run_parallel(fn)

    # Worker threads may interleave, so compare unordered.
    assert sorted(fn.call_args_list) == sorted([call("h1"), call("h2"), call("h3")])
    assert len(futures) == 3
    assert all(isinstance(f, Future) for f in futures)
    assert all(f.done() for f in futures)


def test_run_parallel_passes_extra_args_after_host(monkeypatch):
    """Extra positional args are forwarded after ``host`` so callers
    like ``Uninstall`` keep ``_run(host, *args)`` ergonomics."""
    hg = MagicMock()
    hg.keys.return_value = ["h1", "h2"]
    hg.__getitem__.side_effect = lambda k: MagicMock()

    cmd, _ = _make(monkeypatch, host_group=hg)

    fn = MagicMock(return_value=None)
    sentinel = object()
    futures = cmd._run_parallel(fn, sentinel, "extra")

    # Worker threads may interleave, so compare unordered.
    assert sorted(fn.call_args_list) == sorted(
        [
            call("h1", sentinel, "extra"),
            call("h2", sentinel, "extra"),
        ]
    )
    assert [f.result() for f in futures] == [None, None]


# ---------------------------------------------------------------------------
# _aggregate
# ---------------------------------------------------------------------------


def _ok_future(value: bool = True) -> Future:
    """Return a completed Future whose result is ``value``."""
    f: Future = Future()
    f.set_result(value)
    return f


def _failed_future(exc: BaseException | None = None) -> Future:
    """Return a completed Future that holds an exception."""
    f: Future = Future()
    f.set_exception(exc or RuntimeError("boom"))
    return f


def test_aggregate_returns_0_when_no_futures(monkeypatch):
    """No targets at all is treated as success — there's nothing that
    could have failed."""
    cmd, _ = _make(monkeypatch)
    assert cmd._aggregate([]) == 0


def test_aggregate_returns_0_when_all_succeed(monkeypatch):
    cmd, _ = _make(monkeypatch)
    futures = [_ok_future(True), _ok_future(True), _ok_future(True)]
    assert cmd._aggregate(futures) == 0


def test_aggregate_returns_2_when_all_fail_via_exception(monkeypatch):
    cmd, _ = _make(monkeypatch)
    futures = [_failed_future(), _failed_future()]
    assert cmd._aggregate(futures) == 2


def test_aggregate_returns_2_when_all_fail_via_false_result(monkeypatch):
    cmd, _ = _make(monkeypatch)
    futures = [_ok_future(False), _ok_future(False)]
    assert cmd._aggregate(futures) == 2


def test_aggregate_returns_2_for_single_host_failure(monkeypatch):
    """Degenerate all-failed case: 1 of 1 failed → still exit 2."""
    cmd, _ = _make(monkeypatch)
    assert cmd._aggregate([_ok_future(False)]) == 2
    assert cmd._aggregate([_failed_future()]) == 2


def test_aggregate_returns_1_on_partial_failure(monkeypatch):
    cmd, _ = _make(monkeypatch)
    futures = [_ok_future(True), _ok_future(False)]
    assert cmd._aggregate(futures) == 1


def test_aggregate_returns_1_when_some_raise_and_some_succeed(monkeypatch):
    cmd, _ = _make(monkeypatch)
    futures = [_ok_future(True), _failed_future(), _ok_future(True)]
    assert cmd._aggregate(futures) == 1


def test_aggregate_treats_none_result_as_success(monkeypatch):
    """Forward-compatibility: any non-False result counts as success
    so legacy callers returning ``None`` don't accidentally fail."""
    cmd, _ = _make(monkeypatch)
    futures = [_ok_future(None), _ok_future(True)]  # type: ignore[arg-type]
    assert cmd._aggregate(futures) == 0
