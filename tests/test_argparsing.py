import sys
from unittest.mock import MagicMock

import pytest

import repose.argparsing
from repose import __version__
from repose.argparsing import get_parser
from repose.command import Command


@pytest.fixture
def mock_types(monkeypatch):
    monkeypatch.setattr(repose.argparsing, "Repa", lambda x: x)
    monkeypatch.setattr(repose.argparsing, "ParseHosts", lambda x: x)


def test_version_action(monkeypatch, capsys):
    monkeypatch.setattr(sys, "argv", ["repose", "--version"])
    with pytest.raises(SystemExit):
        get_parser().parse_args()

    ret = capsys.readouterr()
    assert ret.out == f"repose version: {__version__}\n"


def test_debug_action():
    parser = get_parser()
    args = parser.parse_args(["-d", "add", "-t", "dummy", "dummy"])
    assert args.debug is True

    parser = get_parser()
    args = parser.parse_args(["-q", "add", "-t", "dummy", "dummy"])
    assert args.quiet is True


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
def test_command_dispatches_to_registry(
    monkeypatch, mock_types, command, registry_key, cli_args
):
    """args.func resolves the matching Command class via the registry."""
    instance = MagicMock()
    fake_cls = MagicMock(return_value=instance)
    monkeypatch.setitem(Command.registry, registry_key, fake_cls)

    args = get_parser().parse_args(cli_args)
    args.func(args)

    fake_cls.assert_called_once_with(args)
    instance.run.assert_called_once()


def test_dispatch_uses_late_binding_per_subcommand(monkeypatch, mock_types):
    """Each subparser dispatches to its OWN command, not the last one
    registered (regression test for the closure late-binding trap)."""
    add_instance = MagicMock()
    add_cls = MagicMock(return_value=add_instance)
    clear_instance = MagicMock()
    clear_cls = MagicMock(return_value=clear_instance)
    monkeypatch.setitem(Command.registry, "add", add_cls)
    monkeypatch.setitem(Command.registry, "clear", clear_cls)

    parser = get_parser()

    add_args = parser.parse_args(["add", "-t", "h", "x"])
    add_args.func(add_args)
    add_cls.assert_called_once_with(add_args)
    add_instance.run.assert_called_once()
    clear_cls.assert_not_called()

    clear_args = parser.parse_args(["clear", "-t", "h"])
    clear_args.func(clear_args)
    clear_cls.assert_called_once_with(clear_args)
    clear_instance.run.assert_called_once()


# ---------------------------------------------------------------------------
# Extended coverage: defaults, mutually exclusive group, missing args.
# ---------------------------------------------------------------------------


def test_dry_default_is_false(mock_types):
    args = get_parser().parse_args(["add", "-t", "h", "x"])
    assert args.dry is False


def test_dry_short_flag_sets_true(mock_types):
    args = get_parser().parse_args(["-n", "add", "-t", "h", "x"])
    assert args.dry is True


def test_print_long_flag_sets_dry(mock_types):
    args = get_parser().parse_args(["--print", "add", "-t", "h", "x"])
    assert args.dry is True


def test_config_default_path():
    from pathlib import Path

    parser = get_parser()
    args = parser.parse_args(["known-products"])
    assert args.config == Path("/etc/repose/products.yml")


def test_config_override():
    from pathlib import Path

    args = get_parser().parse_args(["-c", "/tmp/my.yml", "known-products"])
    assert args.config == Path("/tmp/my.yml")


def test_debug_and_quiet_mutually_exclusive():
    with pytest.raises(SystemExit):
        get_parser().parse_args(["-d", "-q", "known-products"])


def test_target_required_for_add():
    with pytest.raises(SystemExit):
        # missing -t
        get_parser().parse_args(["add", "x"])


def test_repa_required_positional_for_add(mock_types):
    with pytest.raises(SystemExit):
        get_parser().parse_args(["add", "-t", "h"])


def test_multiple_targets(mock_types):
    args = get_parser().parse_args(["add", "-t", "h1", "-t", "h2", "x"])
    assert args.target == ["h1", "h2"]


def test_multiple_repa_args(mock_types):
    args = get_parser().parse_args(["add", "-t", "h", "x", "y", "z"])
    assert args.repa == ["x", "y", "z"]


def test_list_products_yaml_flag():
    args = get_parser().parse_args(["list-products", "-t", "h", "--yaml"])
    assert args.yaml is True


def test_list_products_yaml_default_false():
    args = get_parser().parse_args(["list-products", "-t", "h"])
    assert args.yaml is False


def test_no_color_flag_default_false(mock_types):
    args = get_parser().parse_args(["add", "-t", "h", "x"])
    assert args.no_color is False


def test_no_color_flag_sets_true(mock_types):
    args = get_parser().parse_args(["--no-color", "add", "-t", "h", "x"])
    assert args.no_color is True


def test_format_default_text(mock_types):
    args = get_parser().parse_args(["add", "-t", "h", "x"])
    assert args.format == "text"


def test_format_json_accepted(mock_types):
    args = get_parser().parse_args(["--format", "json", "add", "-t", "h", "x"])
    assert args.format == "json"


def test_format_invalid_rejected(mock_types):
    with pytest.raises(SystemExit):
        get_parser().parse_args(["--format", "xml", "add", "-t", "h", "x"])


# ---------------------------------------------------------------------------
# Probe flags (PR 08): --probe-timeout / --no-probe
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "subcmd, extra",
    [
        ("add", ["x"]),
        ("install", ["x"]),
        ("reset", []),
    ],
)
def test_probe_timeout_default_is_5(mock_types, subcmd, extra):
    args = get_parser().parse_args([subcmd, "-t", "h", *extra])
    assert args.probe_timeout == 5.0


@pytest.mark.parametrize(
    "subcmd, extra",
    [
        ("add", ["x"]),
        ("install", ["x"]),
        ("reset", []),
    ],
)
def test_probe_timeout_custom_value_is_float(mock_types, subcmd, extra):
    args = get_parser().parse_args(
        [subcmd, "-t", "h", "--probe-timeout", "0.25", *extra]
    )
    assert args.probe_timeout == 0.25
    assert isinstance(args.probe_timeout, float)


@pytest.mark.parametrize(
    "subcmd, extra",
    [
        ("add", ["x"]),
        ("install", ["x"]),
        ("reset", []),
    ],
)
def test_no_probe_default_is_false(mock_types, subcmd, extra):
    args = get_parser().parse_args([subcmd, "-t", "h", *extra])
    assert args.no_probe is False


@pytest.mark.parametrize(
    "subcmd, extra",
    [
        ("add", ["x"]),
        ("install", ["x"]),
        ("reset", []),
    ],
)
def test_no_probe_flag_sets_true(mock_types, subcmd, extra):
    args = get_parser().parse_args([subcmd, "-t", "h", "--no-probe", *extra])
    assert args.no_probe is True


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
def test_probe_flags_absent_from_non_probing_commands(mock_types, subcmd, cli_args):
    """``--probe-timeout`` / ``--no-probe`` are scoped to add/reset/install
    only; passing them to other subcommands errors out, and the parsed
    namespace doesn't carry them."""
    args = get_parser().parse_args(cli_args)
    assert not hasattr(args, "probe_timeout")
    assert not hasattr(args, "no_probe")

    with pytest.raises(SystemExit):
        get_parser().parse_args([*cli_args, "--no-probe"])


# ---------------------------------------------------------------------------
# PR 12 — SSH host-key policy flags + two-pass ``parse()``.
# ---------------------------------------------------------------------------


def test_strict_host_key_checking_default(mock_types):
    args = get_parser().parse_args(["add", "-t", "h", "x"])
    assert args.strict_host_key_checking == "accept-new"
    assert args.known_hosts is None


@pytest.mark.parametrize("mode", ["yes", "accept-new", "no", "off"])
def test_strict_host_key_checking_accepts_all_modes(mock_types, mode):
    args = get_parser().parse_args(
        [f"--strict-host-key-checking={mode}", "add", "-t", "h", "x"]
    )
    assert args.strict_host_key_checking == mode


def test_strict_host_key_checking_rejects_unknown_mode(mock_types):
    with pytest.raises(SystemExit):
        get_parser().parse_args(
            ["--strict-host-key-checking=maybe", "add", "-t", "h", "x"]
        )


def test_known_hosts_flag_parses_to_path(mock_types, tmp_path):
    kh = tmp_path / "kh"
    args = get_parser().parse_args(["--known-hosts", str(kh), "add", "-t", "h", "x"])
    from pathlib import Path

    assert args.known_hosts == Path(str(kh))


def test_parse_two_pass_threads_config_into_target():
    """``parse()`` propagates the global flags into the ``Target``s built
    by ``ParseHosts`` during the second pass."""
    from repose.argparsing import parse

    args = parse(
        [
            "--strict-host-key-checking=yes",
            "add",
            "-t",
            "example.com:2222",
            "SLES:15-SP3:x86_64:",
        ]
    )
    assert args.strict_host_key_checking == "yes"
    # args.target is a list of ParseHosts dicts ({key: Target}).
    targets = args.target
    assert len(targets) == 1
    target = next(iter(targets[0].values()))
    assert target.config.host_key_policy == "yes"


def test_parse_two_pass_default_when_flag_absent():
    """Without ``--strict-host-key-checking`` the default ``accept-new``
    config reaches each ``Target``."""
    from repose.argparsing import parse

    args = parse(["add", "-t", "h", "SLES:15-SP3:x86_64:"])
    target = next(iter(args.target[0].values()))
    assert target.config.host_key_policy == "accept-new"
    assert target.config.known_hosts is None
