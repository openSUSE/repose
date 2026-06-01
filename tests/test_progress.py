"""Tests for ``repose.progress.Progress``."""

import io
import logging
import threading

from rich.console import Console as RichConsole

from repose.progress import Progress


# ---------------------------------------------------------------------------
# enabled=False: no-op context manager, state still mutates
# ---------------------------------------------------------------------------


def test_disabled_progress_is_noop_context_manager():
    prog = Progress(["h1", "h2"], enabled=False)
    with prog:
        prog.update("h1", "doing")
    # state still updates so future consumers can read it
    assert prog.state == {"h1": "doing", "h2": "pending"}


def test_disabled_progress_leaves_logging_untouched():
    lg = logging.getLogger("repose")
    before = list(lg.handlers)
    before_level = lg.level
    with Progress(["h1"], enabled=False):
        pass
    assert list(lg.handlers) == before
    assert lg.level == before_level


# ---------------------------------------------------------------------------
# update() basics
# ---------------------------------------------------------------------------


def test_update_mutates_state():
    prog = Progress(["h1"], enabled=False)
    prog.update("h1", "running")
    assert prog.state["h1"] == "running"


def test_update_thread_safe():
    """16 threads bashing the same host's cell — no exceptions, final
    state holds one of the posted values."""
    prog = Progress(["h"], enabled=False)
    barrier = threading.Barrier(16)

    def _hammer(i):
        barrier.wait()
        prog.update("h", str(i))

    threads = [threading.Thread(target=_hammer, args=(i,)) for i in range(16)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    final = prog.state["h"]
    assert final in {str(i) for i in range(16)}


# ---------------------------------------------------------------------------
# enabled=True: Live renders rows, logging swap restores
# ---------------------------------------------------------------------------


def _capture_console() -> RichConsole:
    """A Rich Console writing to an in-memory buffer with TTY emulation
    so Live actually renders."""
    return RichConsole(
        file=io.StringIO(),
        force_terminal=True,
        width=80,
    )


def test_enabled_progress_renders_rows():
    rc = _capture_console()
    prog = Progress(["beta", "alpha"], enabled=True, console=rc)
    with prog:
        prog.update("alpha", "working")
        prog.update("beta", "done")
    output = rc.file.getvalue()
    # transient=True clears the live region on exit, but the row text
    # passes through the buffer at least once.
    assert "alpha" in output
    assert "beta" in output


def test_enabled_progress_renders_status_text():
    rc = _capture_console()
    prog = Progress(["h"], enabled=True, console=rc)
    with prog:
        prog.update("h", "milestone-text-XYZ")
    assert "milestone-text-XYZ" in rc.file.getvalue()


def test_enabled_progress_swaps_and_restores_logging():
    """While Live is active a RichHandler is installed on ``repose``;
    after exit the original handler set + level come back verbatim."""
    lg = logging.getLogger("repose")
    # Snapshot then install a sentinel handler so we can prove restoration.
    original_handlers = list(lg.handlers)
    original_level = lg.level
    sentinel = logging.NullHandler()
    lg.addHandler(sentinel)
    lg.setLevel(logging.WARNING)
    try:
        rc = _capture_console()
        with Progress(["h"], enabled=True, console=rc):
            # During Live the only handler should be a RichHandler.
            handler_types = {type(h).__name__ for h in lg.handlers}
            assert "RichHandler" in handler_types
            assert "NullHandler" not in handler_types

        # After exit our sentinel + the prior handlers are back.
        assert sentinel in lg.handlers
        assert lg.level == logging.WARNING
    finally:
        # Tidy up the sentinel regardless.
        if sentinel in lg.handlers:
            lg.removeHandler(sentinel)
        # Restore pre-test state.
        for h in list(lg.handlers):
            if h not in original_handlers:
                lg.removeHandler(h)
        for h in original_handlers:
            if h not in lg.handlers:
                lg.addHandler(h)
        lg.setLevel(original_level)


def test_logging_restored_on_exception():
    """Exceptions inside the ``with`` block must not leak the
    RichHandler swap."""
    lg = logging.getLogger("repose")
    original_handlers = list(lg.handlers)
    original_level = lg.level
    rc = _capture_console()
    try:
        with Progress(["h"], enabled=True, console=rc):
            raise RuntimeError("boom")
    except RuntimeError:
        pass

    handler_types = {type(h).__name__ for h in lg.handlers}
    assert "RichHandler" not in handler_types
    assert list(lg.handlers) == original_handlers
    assert lg.level == original_level
