"""Tests for ``repose.cli`` (Typer-based CLI).

Mirrors the structure of the retired ``tests/test_argparsing.py``:
version/help, global flags, per-subcommand parsing, probe flags, and
SSH host-key transport globals. Where the old tests inspected an
``argparse.Namespace`` directly, these tests monkeypatch the matching
``Command.registry`` entry with a ``MagicMock`` and assert against the
``Namespace`` the mock receives — same surface, just routed through
``CliRunner``.
"""

from pathlib import Path
from unittest.mock import MagicMock

import pytest
from typer.testing import CliRunner

from repose import __version__
from repose.cli import app
from repose.command import Command

runner = CliRunner()


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def mock_registry(monkeypatch):
    """Install a fresh MagicMock for every registered command.

    Each call to ``cls(ns)`` is captured on the mock; tests inspect
    ``mock_registry[name].call_args[0][0]`` to get the Namespace the
    Command would have received under the real CLI.
    """
    mocks: dict[str, MagicMock] = {}
    for name in list(Command.registry.keys()):
        instance = MagicMock()
        instance.run.return_value = 0
        cls = MagicMock(return_value=instance)
        monkeypatch.setitem(Command.registry, name, cls)
        mocks[name] = cls
    return mocks


def _ns(mocks: dict[str, MagicMock], name: str):
    """Return the Namespace the mocked Command class received."""
    cls = mocks[name]
    cls.assert_called_once()
    return cls.call_args[0][0]


# ---------------------------------------------------------------------------
# --version / bare invocation
# ---------------------------------------------------------------------------


def test_version_action():
    result = runner.invoke(app, ["--version"])
    assert result.exit_code == 0
    assert result.stdout == f"repose version: {__version__}\n"


def test_version_short_flag():
    result = runner.invoke(app, ["-V"])
    assert result.exit_code == 0
    assert result.stdout == f"repose version: {__version__}\n"


def test_bare_invocation_prints_help_exit_0():
    result = runner.invoke(app, [])
    assert result.exit_code == 0
    assert "Usage" in result.stdout
    # Every subcommand surfaces in the help block.
    for name in (
        "add",
        "remove",
        "reset",
        "install",
        "clear",
        "uninstall",
        "list-products",
        "list-repos",
        "known-products",
    ):
        assert name in result.stdout


# ---------------------------------------------------------------------------
# Global flags
# ---------------------------------------------------------------------------


def test_debug_flag_captured(mock_registry):
    result = runner.invoke(app, ["-d", "add", "-t", "dummy", "dummy"])
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    assert ns.debug is True
    assert ns.quiet is False


def test_quiet_flag_captured(mock_registry):
    result = runner.invoke(app, ["-q", "add", "-t", "dummy", "dummy"])
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    assert ns.quiet is True
    assert ns.debug is False


def test_debug_and_quiet_mutually_exclusive(mock_registry):
    result = runner.invoke(app, ["-d", "-q", "known-products"])
    assert result.exit_code != 0


def test_dry_default_is_false(mock_registry):
    result = runner.invoke(app, ["add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").dry is False


def test_dry_short_flag_sets_true(mock_registry):
    result = runner.invoke(app, ["-n", "add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").dry is True


def test_print_long_flag_sets_dry(mock_registry):
    result = runner.invoke(app, ["--print", "add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").dry is True


def test_config_default_path(mock_registry):
    result = runner.invoke(app, ["known-products"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "known-products").config == Path(
        "/etc/repose/products.yml"
    )


def test_config_override(mock_registry):
    result = runner.invoke(app, ["-c", "/tmp/my.yml", "known-products"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "known-products").config == Path("/tmp/my.yml")


def test_no_color_default_false(mock_registry):
    result = runner.invoke(app, ["add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").no_color is False


def test_no_color_sets_true(mock_registry):
    result = runner.invoke(app, ["--no-color", "add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").no_color is True


def test_format_default_text(mock_registry):
    result = runner.invoke(app, ["add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").format == "text"


def test_format_json_accepted(mock_registry):
    result = runner.invoke(app, ["--format", "json", "add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").format == "json"


def test_format_invalid_rejected(mock_registry):
    result = runner.invoke(app, ["--format", "xml", "add", "-t", "h", "x"])
    assert result.exit_code != 0


# ---------------------------------------------------------------------------
# Per-subcommand parsing
# ---------------------------------------------------------------------------


def test_target_required_for_add(mock_registry):
    result = runner.invoke(app, ["add", "x"])
    assert result.exit_code != 0


def test_repa_required_positional_for_add(mock_registry):
    result = runner.invoke(app, ["add", "-t", "h"])
    assert result.exit_code != 0


def test_multiple_targets(mock_registry):
    result = runner.invoke(app, ["add", "-t", "h1", "-t", "h2", "x"])
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    # Each target is a ParseHosts dict; the keys are the host strings.
    assert len(ns.target) == 2
    assert list(ns.target[0].keys()) == ["h1"]
    assert list(ns.target[1].keys()) == ["h2"]


def test_multiple_repa_args(mock_registry):
    result = runner.invoke(app, ["add", "-t", "h", "x", "y", "z"])
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    assert [r.product for r in ns.repa] == ["x", "y", "z"]


def test_list_products_yaml_flag(mock_registry):
    result = runner.invoke(app, ["list-products", "-t", "h", "--yaml"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "list-products").yaml is True


def test_list_products_yaml_default_false(mock_registry):
    result = runner.invoke(app, ["list-products", "-t", "h"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "list-products").yaml is False


# ---------------------------------------------------------------------------
# Registry dispatch (parametrized, regression test for mis-binding)
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "command, registry_key, cli_args",
    [
        ("add", "add", ["add", "-t", "dummy", "dummy"]),
        ("remove", "remove", ["remove", "-t", "dummy", "dummy"]),
        ("reset", "reset", ["reset", "-t", "dummy"]),
        ("install", "install", ["install", "-t", "dummy", "dummy"]),
        ("clear", "clear", ["clear", "-t", "dummy"]),
        ("uninstall", "uninstall", ["uninstall", "-t", "dummy", "dummy"]),
        ("list-products", "list-products", ["list-products", "-t", "dummy"]),
        ("list-repos", "list-repos", ["list-repos", "-t", "dummy"]),
        ("known-products", "known-products", ["known-products"]),
    ],
)
def test_command_dispatches_to_registry(mock_registry, command, registry_key, cli_args):
    """Each subcommand resolves its matching Command class via the registry."""
    result = runner.invoke(app, cli_args)
    assert result.exit_code == 0, result.stderr
    cls = mock_registry[registry_key]
    cls.assert_called_once()
    cls.return_value.run.assert_called_once()
    # Sister commands must NOT have been called.
    for other_name, other_cls in mock_registry.items():
        if other_name == registry_key:
            continue
        other_cls.assert_not_called()


def test_dispatch_uses_correct_command_per_subcommand(mock_registry):
    """Two invocations in the same session must each call their OWN
    command class (regression test for any closure late-binding trap)."""
    result_add = runner.invoke(app, ["add", "-t", "h", "x"])
    assert result_add.exit_code == 0, result_add.stderr
    mock_registry["add"].assert_called_once()
    mock_registry["clear"].assert_not_called()

    result_clear = runner.invoke(app, ["clear", "-t", "h"])
    assert result_clear.exit_code == 0, result_clear.stderr
    mock_registry["clear"].assert_called_once()
    # add still has exactly one call from earlier.
    assert mock_registry["add"].call_count == 1


# ---------------------------------------------------------------------------
# Exit-code propagation
# ---------------------------------------------------------------------------


def test_exit_code_from_command_run(monkeypatch):
    """Whatever ``Command.run`` returns becomes the process exit code."""
    instance = MagicMock()
    instance.run.return_value = 2
    fake = MagicMock(return_value=instance)
    monkeypatch.setitem(Command.registry, "known-products", fake)

    result = runner.invoke(app, ["known-products"])
    assert result.exit_code == 2


# ---------------------------------------------------------------------------
# Probe flags (--probe-timeout / --no-probe) — scoped to add/install/reset
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "subcmd, extra",
    [
        ("add", ["x"]),
        ("install", ["x"]),
        ("reset", []),
    ],
)
def test_probe_timeout_default_is_5(mock_registry, subcmd, extra):
    result = runner.invoke(app, [subcmd, "-t", "h", *extra])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, subcmd).probe_timeout == 5.0


@pytest.mark.parametrize(
    "subcmd, extra",
    [
        ("add", ["x"]),
        ("install", ["x"]),
        ("reset", []),
    ],
)
def test_probe_timeout_custom_value_is_float(mock_registry, subcmd, extra):
    result = runner.invoke(app, [subcmd, "-t", "h", "--probe-timeout", "0.25", *extra])
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, subcmd)
    assert ns.probe_timeout == 0.25
    assert isinstance(ns.probe_timeout, float)


@pytest.mark.parametrize(
    "subcmd, extra",
    [
        ("add", ["x"]),
        ("install", ["x"]),
        ("reset", []),
    ],
)
def test_no_probe_default_is_false(mock_registry, subcmd, extra):
    result = runner.invoke(app, [subcmd, "-t", "h", *extra])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, subcmd).no_probe is False


@pytest.mark.parametrize(
    "subcmd, extra",
    [
        ("add", ["x"]),
        ("install", ["x"]),
        ("reset", []),
    ],
)
def test_no_probe_flag_sets_true(mock_registry, subcmd, extra):
    result = runner.invoke(app, [subcmd, "-t", "h", "--no-probe", *extra])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, subcmd).no_probe is True


@pytest.mark.parametrize(
    "subcmd, cli_args",
    [
        ("clear", ["clear", "-t", "h"]),
        ("known-products", ["known-products"]),
        ("list-products", ["list-products", "-t", "h"]),
        ("list-repos", ["list-repos", "-t", "h"]),
        ("remove", ["remove", "-t", "h", "x"]),
        ("uninstall", ["uninstall", "-t", "h", "x"]),
    ],
)
def test_probe_flags_absent_from_non_probing_commands(mock_registry, subcmd, cli_args):
    """``--probe-timeout`` / ``--no-probe`` are scoped to add/reset/install
    only; passing them to other subcommands errors out, and the parsed
    namespace doesn't carry them."""
    result = runner.invoke(app, cli_args)
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, subcmd)
    assert not hasattr(ns, "probe_timeout")
    assert not hasattr(ns, "no_probe")

    result_fail = runner.invoke(app, [*cli_args, "--no-probe"])
    assert result_fail.exit_code != 0


# ---------------------------------------------------------------------------
# SSH host-key policy + --known-hosts (parity with the old two-pass test)
# ---------------------------------------------------------------------------


def test_strict_host_key_checking_default(mock_registry):
    result = runner.invoke(app, ["add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    assert ns.strict_host_key_checking == "accept-new"
    assert ns.known_hosts is None


@pytest.mark.parametrize("mode", ["yes", "accept-new", "no", "off"])
def test_strict_host_key_checking_accepts_all_modes(mock_registry, mode):
    result = runner.invoke(
        app, [f"--strict-host-key-checking={mode}", "add", "-t", "h", "x"]
    )
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").strict_host_key_checking == mode


def test_strict_host_key_checking_rejects_unknown_mode(mock_registry):
    result = runner.invoke(
        app, ["--strict-host-key-checking=maybe", "add", "-t", "h", "x"]
    )
    assert result.exit_code != 0


def test_known_hosts_flag_parses_to_path(mock_registry, tmp_path):
    kh = tmp_path / "kh"
    result = runner.invoke(app, ["--known-hosts", str(kh), "add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").known_hosts == Path(str(kh))


def test_host_key_policy_threads_into_target_config(mock_registry):
    """The two-pass parity check: ``--strict-host-key-checking=yes``
    must reach the ``Target`` objects built by ``-t`` parsing."""
    result = runner.invoke(
        app,
        [
            "--strict-host-key-checking=yes",
            "add",
            "-t",
            "example.com:2222",
            "SLES:15-SP3:x86_64:",
        ],
    )
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    assert ns.strict_host_key_checking == "yes"
    targets = ns.target
    assert len(targets) == 1
    target = next(iter(targets[0].values()))
    assert target.config.host_key_policy == "yes"


def test_host_key_policy_default_threads_into_target_config(mock_registry):
    """Without ``--strict-host-key-checking`` the default ``accept-new``
    config reaches each ``Target``."""
    result = runner.invoke(app, ["add", "-t", "h", "SLES:15-SP3:x86_64:"])
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    target = next(iter(ns.target[0].values()))
    assert target.config.host_key_policy == "accept-new"
    assert target.config.known_hosts is None


def test_target_parser_resolves_context_config(mock_registry, tmp_path):
    """``_target_parser`` must read the live ``ConnectionConfig`` off the
    active Typer context.

    Regression guard for the Typer ``get_current_context`` import path
    (cli.py): Typer >=0.26 vendors click under ``typer._click.globals``
    while Typer 0.16 (Leap 16) uses real click. If the resolver imports
    the wrong namespace — or fails outright — no live context is found
    and each ``Target`` silently falls back to a default
    ``ConnectionConfig`` with ``known_hosts is None``. Asserting that a
    context-supplied ``--known-hosts`` path reaches the parsed ``Target``
    proves the context lookup actually resolved.
    """
    kh = tmp_path / "kh"
    result = runner.invoke(
        app,
        ["--known-hosts", str(kh), "add", "-t", "h", "SLES:15-SP3:x86_64:"],
    )
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    target = next(iter(ns.target[0].values()))
    # Only reachable if _target_parser found the live context's conn_config.
    assert target.config.known_hosts == Path(str(kh))


# ---------------------------------------------------------------------------
# --ssh-backend  (PR 14: asyncssh rewrite)
# ---------------------------------------------------------------------------


def test_ssh_backend_default_is_asyncssh(mock_registry):
    result = runner.invoke(app, ["add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    assert ns.ssh_backend == "asyncssh"


@pytest.mark.parametrize("backend", ["asyncssh", "paramiko"])
def test_ssh_backend_accepts_both_modes(mock_registry, backend):
    result = runner.invoke(app, [f"--ssh-backend={backend}", "add", "-t", "h", "x"])
    assert result.exit_code == 0, result.stderr
    assert _ns(mock_registry, "add").ssh_backend == backend


def test_ssh_backend_rejects_unknown_mode(mock_registry):
    result = runner.invoke(app, ["--ssh-backend=netconf", "add", "-t", "h", "x"])
    assert result.exit_code != 0


def test_ssh_backend_threads_into_target_config(mock_registry):
    """``--ssh-backend=paramiko`` must reach each ``Target``'s ConnectionConfig."""
    result = runner.invoke(
        app,
        [
            "--ssh-backend=paramiko",
            "add",
            "-t",
            "h",
            "SLES:15-SP3:x86_64:",
        ],
    )
    assert result.exit_code == 0, result.stderr
    ns = _ns(mock_registry, "add")
    target = next(iter(ns.target[0].values()))
    assert target.config.ssh_backend == "paramiko"


# ---------------------------------------------------------------------------
# Friendly error handling (moved from old test_main.py — these exercise
# the ``_dispatch`` exception-translation block in ``repose.cli``)
# ---------------------------------------------------------------------------


def _install_raising_command(monkeypatch, name: str, exc: BaseException) -> None:
    instance = MagicMock()
    instance.run.side_effect = exc
    fake = MagicMock(return_value=instance)
    monkeypatch.setitem(Command.registry, name, fake)


def test_friendly_message_on_missing_config(monkeypatch, caplog):
    err = FileNotFoundError(2, "No such file or directory", "/etc/repose/products.yml")
    _install_raising_command(monkeypatch, "known-products", err)

    with caplog.at_level("ERROR", logger="repose.cli"):
        result = runner.invoke(app, ["known-products"])

    assert result.exit_code == 2
    assert "config file not found" in caplog.text
    assert "/etc/repose/products.yml" in caplog.text


def test_friendly_message_on_permission_error(monkeypatch, caplog):
    err = PermissionError(13, "Permission denied", "/etc/repose/products.yml")
    _install_raising_command(monkeypatch, "known-products", err)

    with caplog.at_level("ERROR", logger="repose.cli"):
        result = runner.invoke(app, ["known-products"])

    assert result.exit_code == 2
    assert "permission denied" in caplog.text.lower()


def test_friendly_message_on_is_a_directory_error(monkeypatch, caplog):
    err = IsADirectoryError(21, "Is a directory", "/etc/repose")
    _install_raising_command(monkeypatch, "known-products", err)

    with caplog.at_level("ERROR", logger="repose.cli"):
        result = runner.invoke(app, ["known-products"])

    assert result.exit_code == 2
    assert "is a directory" in caplog.text.lower()


def test_friendly_message_on_yaml_error(monkeypatch, caplog):
    from ruamel.yaml import YAMLError

    _install_raising_command(monkeypatch, "known-products", YAMLError("bad yaml"))

    with caplog.at_level("ERROR", logger="repose.cli"):
        result = runner.invoke(app, ["known-products"])

    assert result.exit_code == 2
    assert "invalid yaml" in caplog.text.lower()


def test_friendly_message_on_non_mapping_config(tmp_path, caplog):
    """A list-shaped products.yml gets the one-liner + exit 2, end to end.

    Regression: ``load_template`` raises ``TemplateError`` for a
    non-mapping top level, but ``_dispatch`` only translated
    ``FileNotFoundError``/``PermissionError``/``IsADirectoryError``/
    ``YAMLError`` — the new exception escaped every real command as a
    raw traceback (exit 1) while malformed YAML got the friendly
    treatment. Uses the real loader on a real file on purpose.
    """
    listy = tmp_path / "products.yml"
    listy.write_text("- SLES\n- openSUSE-Leap\n")

    with caplog.at_level("ERROR", logger="repose.cli"):
        result = runner.invoke(app, ["-c", str(listy), "known-products"])

    assert result.exit_code == 2
    assert not isinstance(result.exception, ValueError), (
        "the config error must be translated, not raised as a traceback"
    )
    assert "must be a YAML mapping" in caplog.text


def test_debug_propagates_traceback(monkeypatch):
    """With ``--debug``, the original exception is re-raised intact so
    contributors see the full stack instead of the friendly summary."""
    err = FileNotFoundError(2, "No such file or directory", "/nope")
    _install_raising_command(monkeypatch, "known-products", err)

    result = runner.invoke(app, ["--debug", "known-products"], catch_exceptions=True)
    assert result.exit_code != 0
    assert isinstance(result.exception, FileNotFoundError)


def test_keyboard_interrupt_returns_130(monkeypatch, caplog):
    _install_raising_command(monkeypatch, "known-products", KeyboardInterrupt())

    with caplog.at_level("ERROR", logger="repose.cli"):
        result = runner.invoke(app, ["known-products"])

    assert result.exit_code == 130
    assert "interrupted" in caplog.text.lower()


# ---------------------------------------------------------------------------
# Logger level wiring (moved from old test_main.py)
# ---------------------------------------------------------------------------


def test_debug_flag_sets_logger_level_debug(mock_registry):
    import logging as _logging

    log = _logging.getLogger("repose")
    saved = log.level
    try:
        result = runner.invoke(app, ["-d", "known-products"])
        assert result.exit_code == 0, result.stderr
        assert log.level == _logging.DEBUG
    finally:
        log.setLevel(saved)


def test_quiet_flag_sets_logger_level_warning(mock_registry):
    import logging as _logging

    log = _logging.getLogger("repose")
    saved = log.level
    try:
        result = runner.invoke(app, ["-q", "known-products"])
        assert result.exit_code == 0, result.stderr
        assert log.level == _logging.WARNING
    finally:
        log.setLevel(saved)
