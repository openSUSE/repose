"""Tests for ``repose.target.parsers.product.parse_system``."""

import logging
from contextlib import asynccontextmanager, contextmanager
from unittest.mock import AsyncMock, MagicMock

import pytest

from repose.target.parsers import Product
from repose.target.parsers.product import parse_system, parse_system_async
from repose.types.system import System, UnknownSystemError


def _prod_xml(name, baseversion=None, patchlevel=None, version=None, arch="x86_64"):
    parts = [f"<name>{name}</name>", f"<arch>{arch}</arch>"]
    if baseversion is not None:
        parts.append(f"<baseversion>{baseversion}</baseversion>")
    if patchlevel is not None:
        parts.append(f"<patchlevel>{patchlevel}</patchlevel>")
    if version is not None:
        parts.append(f"<version>{version}</version>")
    return f"<product>{''.join(parts)}</product>".encode()


@contextmanager
def _open_returning(content: bytes):
    """Context manager that yields an iterable mimicking sftp.open()."""
    mock_file = MagicMock()
    # parse_product uses ElementTree.fromstringlist — pass a single-element list
    mock_file.__iter__.return_value = iter([content])
    yield mock_file


def _make_connection(
    files, basefile, file_contents, osrelease=None, listdir_error=None
):
    """Build a mock Connection with /etc/products.d/* layout."""
    conn = MagicMock()
    if listdir_error is not None:
        conn.listdir.side_effect = listdir_error
    else:
        conn.listdir.return_value = files
    conn.readlink.return_value = basefile

    def _open(path, *args, **kwargs):
        if path in file_contents:
            return _open_returning(file_contents[path])
        raise FileNotFoundError(path)

    conn.open.side_effect = _open
    return conn


def test_suse_baseproduct_with_patchlevel():
    base = _prod_xml("SLES", baseversion="15", patchlevel="3", arch="x86_64")
    conn = _make_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": base},
    )

    system = parse_system(conn)

    assert isinstance(system, System)
    assert system.get_base() == Product("SLES", "15-SP3", "x86_64")
    assert system.get_addons() == set()


def test_patchlevel_zero_omits_sp_suffix():
    base = _prod_xml("SLES", baseversion="15", patchlevel="0", arch="x86_64")
    conn = _make_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": base},
    )

    system = parse_system(conn)
    assert system.get_base() == Product("SLES", "15", "x86_64")


def test_simple_version_fallback():
    """Product without baseversion/patchlevel falls back to <version>."""
    base = _prod_xml("openSUSE", version="15.5", arch="x86_64")
    conn = _make_connection(
        files=["openSUSE.prod"],
        basefile="openSUSE.prod",
        file_contents={"/etc/products.d/openSUSE.prod": base},
    )

    system = parse_system(conn)
    assert system.get_base() == Product("openSUSE", "15.5", "x86_64")


def test_caasp_version_overridden_to_all():
    base = _prod_xml("CAASP", baseversion="4", patchlevel="0", arch="x86_64")
    conn = _make_connection(
        files=["CAASP.prod"],
        basefile="CAASP.prod",
        file_contents={"/etc/products.d/CAASP.prod": base},
    )
    system = parse_system(conn)
    assert system.get_base() == Product("CAASP", "ALL", "x86_64")


def test_addons_collected_and_migration_filtered():
    base = _prod_xml("SLES", baseversion="15", patchlevel="3", arch="x86_64")
    addon = _prod_xml(
        "sle-module-basesystem",
        baseversion="15",
        patchlevel="3",
        arch="x86_64",
    )
    migration = _prod_xml(
        "SLES15-migration",
        baseversion="15",
        patchlevel="3",
        arch="x86_64",
    )
    conn = _make_connection(
        files=["SLES.prod", "module.prod", "migration.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": base,
            "/etc/products.d/module.prod": addon,
            "/etc/products.d/migration.prod": migration,
        },
    )

    system = parse_system(conn)
    addons = system.get_addons()
    names = {a.name for a in addons}
    assert "sle-module-basesystem" in names
    # migration product must be filtered out
    assert "SLES15-migration" not in names


def test_baseproduct_symlink_with_path_strips_dir():
    base = _prod_xml("SLES", baseversion="15", patchlevel="3", arch="x86_64")
    conn = _make_connection(
        files=["SLES.prod"],
        basefile="../SLES.prod",  # symlink target with leading path
        file_contents={"/etc/products.d/SLES.prod": base},
    )

    system = parse_system(conn)
    assert system.get_base().name == "SLES"


def test_transactional_host_detected_via_conf():
    """A transactional-update.conf marks the host transactional."""
    base = _prod_xml("SL-Micro", version="6.1", arch="x86_64")
    conn = _make_connection(
        files=["SL-Micro.prod"],
        basefile="SL-Micro.prod",
        file_contents={
            "/etc/products.d/SL-Micro.prod": base,
            "/usr/etc/transactional-update.conf": b"",
        },
    )

    system = parse_system(conn)
    assert system.is_transactional() is True


def test_transactional_conf_in_etc_also_detected():
    """SLE Micro 5.x keeps the conf in /etc, not /usr/etc."""
    base = _prod_xml("SLE-Micro", version="5.5", arch="x86_64")
    conn = _make_connection(
        files=["SLE-Micro.prod"],
        basefile="SLE-Micro.prod",
        file_contents={
            "/etc/products.d/SLE-Micro.prod": base,
            "/etc/transactional-update.conf": b"",
        },
    )

    system = parse_system(conn)
    assert system.is_transactional() is True


def test_non_transactional_host_not_detected():
    """A regular SLES host (no transactional-update.conf) is not flagged."""
    base = _prod_xml("SLES", baseversion="16", patchlevel="0", arch="x86_64")
    conn = _make_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": base},
    )

    system = parse_system(conn)
    assert system.is_transactional() is False


_UBUNTU_OS_RELEASE = (
    b'NAME="Ubuntu"\n'
    b'VERSION_ID="22.04"\n'
    b'VERSION="22.04.3 LTS (Jammy Jellyfish)"\n'
    b"ID=ubuntu\n"
    b"ID_LIKE=debian\n"
)


def test_non_suse_reflects_os_release_id_and_version():
    """A non-SUSE host's identity comes from os-release ID/VERSION_ID.

    Regression guard: the parser must reflect the file contents (ubuntu
    22.04) rather than fabricating a hardcoded rhel/7 tuple.
    """
    conn = MagicMock()
    conn.listdir.side_effect = OSError("No such directory")

    def _open(path, *args, **kwargs):
        if path == "/etc/os-release":
            return _open_returning(_UBUNTU_OS_RELEASE)
        raise FileNotFoundError(path)

    conn.open.side_effect = _open

    system = parse_system(conn)
    assert system.get_base() == Product("ubuntu", "22.04", "unknown")


def test_non_suse_os_release_uses_optional_architecture_field():
    """An optional ARCHITECTURE key overrides the arch placeholder."""
    conn = MagicMock()
    conn.listdir.side_effect = OSError("No such directory")

    def _open(path, *args, **kwargs):
        if path == "/etc/os-release":
            return _open_returning(b"ID=fedora\nVERSION_ID=40\nARCHITECTURE=aarch64\n")
        raise FileNotFoundError(path)

    conn.open.side_effect = _open

    system = parse_system(conn)
    assert system.get_base() == Product("fedora", "40", "aarch64")


def test_non_suse_os_release_bare_line_without_equals_is_skipped():
    """A non-comment line lacking ``=`` is skipped, not parsed as ``key=''``.

    Guards the ``or "=" not in line`` skip clause: a bare ``ARCHITECTURE``
    line must leave the arch default intact. If ``or`` were ``and`` the
    line would be split as key=whole-line, value='' and clobber the
    ``"unknown"`` arch placeholder with an empty string.
    """
    conn = MagicMock()
    conn.listdir.side_effect = OSError("No such directory")

    def _open(path, *args, **kwargs):
        if path == "/etc/os-release":
            return _open_returning(b"ID=fedora\nVERSION_ID=40\nARCHITECTURE\n")
        raise FileNotFoundError(path)

    conn.open.side_effect = _open

    system = parse_system(conn)
    assert system.get_base() == Product("fedora", "40", "unknown")


def test_non_suse_os_release_without_id_falls_back_to_linux(caplog):
    """A malformed os-release (no ID) warns and uses the spec default.

    os-release(5) defines ``ID``'s default as ``"linux"``, so the parser
    falls back to that identity with a warning naming the host instead of
    raising and knocking the host out of the scan.
    """
    conn = MagicMock()
    conn.hostname = "mystery.example.com"
    conn.listdir.side_effect = OSError("No such directory")

    def _open(path, *args, **kwargs):
        if path == "/etc/os-release":
            return _open_returning(b'PRETTY_NAME="Mystery OS"\n')
        raise FileNotFoundError(path)

    conn.open.side_effect = _open

    with caplog.at_level(logging.WARNING):
        system = parse_system(conn)

    assert system.get_base() == Product("linux", "", "unknown")
    warning = next(
        record.getMessage()
        for record in caplog.records
        if record.levelno == logging.WARNING
    )
    # Pin the host prefix (kills the hostname->None mutant) without
    # over-pinning the rest of the incidental warning phrasing.
    assert warning.startswith("mystery.example.com:")


def test_non_suse_without_osrelease_returns_rhel6_default():
    conn = MagicMock()
    conn.listdir.side_effect = OSError("No such directory")
    conn.open.side_effect = FileNotFoundError("/etc/os-release")

    system = parse_system(conn)
    assert system.get_base() == Product("rhel", "6", "x86_64")


# ---------------------------------------------------------------------------
# parse_system_async — mirrors the sync cases for the async backend.
# These guard the backend-parity contract: the async parser keys off the
# same OSError/FileNotFoundError surface, which only holds because
# AsyncConnection translates asyncssh's SFTP errors (see test_aiossh.py).
# ---------------------------------------------------------------------------


def _async_open_returning(content: bytes):
    @asynccontextmanager
    async def _cm(*args, **kwargs):
        mock_file = MagicMock()
        mock_file.__iter__.return_value = iter([content])
        yield mock_file

    return _cm()


def _make_async_connection(files, basefile, file_contents, listdir_error=None):
    conn = MagicMock()
    if listdir_error is not None:
        conn.listdir = AsyncMock(side_effect=listdir_error)
    else:
        conn.listdir = AsyncMock(return_value=files)
    conn.readlink = AsyncMock(return_value=basefile)

    def _open(path, *args, **kwargs):
        if path in file_contents:
            return _async_open_returning(file_contents[path])
        raise FileNotFoundError(path)

    conn.open.side_effect = _open
    return conn


async def test_async_suse_baseproduct_with_patchlevel():
    base = _prod_xml("SLES", baseversion="15", patchlevel="3", arch="x86_64")
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": base},
    )

    system = await parse_system_async(conn)
    assert system.get_base() == Product("SLES", "15-SP3", "x86_64")


async def test_async_transactional_host_detected_via_conf():
    base = _prod_xml("SL-Micro", version="6.1", arch="x86_64")
    conn = _make_async_connection(
        files=["SL-Micro.prod"],
        basefile="SL-Micro.prod",
        file_contents={
            "/etc/products.d/SL-Micro.prod": base,
            "/usr/etc/transactional-update.conf": b"",
        },
    )

    system = await parse_system_async(conn)
    assert system.is_transactional() is True


async def test_async_non_transactional_not_detected():
    base = _prod_xml("SLES", baseversion="16", patchlevel="0", arch="x86_64")
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": base},
    )

    system = await parse_system_async(conn)
    assert system.is_transactional() is False


async def test_async_non_suse_falls_back_to_os_release():
    """A missing /etc/products.d (FileNotFoundError) must not crash.

    Before the SFTP-error translation, asyncssh raised ``SFTPNoSuchFile``
    (not an ``OSError``) here, so the ``except OSError`` never fired and
    every non-SUSE host blew up on the default backend.
    """
    conn = MagicMock()
    conn.listdir = AsyncMock(side_effect=FileNotFoundError("No such directory"))

    def _open(path, *args, **kwargs):
        if path == "/etc/os-release":
            return _async_open_returning(_UBUNTU_OS_RELEASE)
        raise FileNotFoundError(path)

    conn.open.side_effect = _open

    system = await parse_system_async(conn)
    assert system.get_base() == Product("ubuntu", "22.04", "unknown")


async def test_async_non_suse_without_osrelease_returns_rhel6_default():
    conn = MagicMock()
    conn.listdir = AsyncMock(side_effect=FileNotFoundError("No such directory"))
    conn.open.side_effect = FileNotFoundError("/etc/os-release")

    system = await parse_system_async(conn)
    assert system.get_base() == Product("rhel", "6", "x86_64")


# ---------------------------------------------------------------------------
# Malformed product data must be skipped, not abort the whole host parse.
# Regressions for review findings N7 (missing <name>/<arch> raised
# AttributeError), N8 (files.remove(basefile) raised ValueError when the
# baseproduct target was not a listed .prod file) and v1 #24 (readlink
# returning None raised TypeError).
# ---------------------------------------------------------------------------

_BASE_XML = _prod_xml("SLES", baseversion="15", patchlevel="3")
_ADDON_XML = _prod_xml("sle-module-basesystem", version="15.3")

# Raw byte literals below: _prod_xml cannot omit <name>/<arch> or emit
# self-closing empty elements, which is exactly what these shapes need.
_MALFORMED_PRODS = [
    pytest.param(
        b"<product><arch>x86_64</arch><version>1.0</version></product>",
        id="missing-name",
    ),
    pytest.param(
        b"<product><name>bad</name><version>1.0</version></product>",
        id="missing-arch",
    ),
    pytest.param(
        b"<product><name/><arch>x86_64</arch><version>1.0</version></product>",
        id="empty-name",
    ),
    pytest.param(
        b"<product><name>bad</name><arch>x86_64</arch></product>",
        id="missing-version",
    ),
    pytest.param(
        b"<product><name>bad</name><arch>x86_64</arch>"
        b"<baseversion/><patchlevel>3</patchlevel></product>",
        id="empty-baseversion-with-patchlevel",
    ),
    pytest.param(
        b"<product><name>bad</name><arch>x86_64</arch>"
        b"<baseversion/><patchlevel>0</patchlevel></product>",
        id="empty-baseversion-patchlevel-zero",
    ),
]


@pytest.mark.parametrize("bad", _MALFORMED_PRODS)
def test_malformed_addon_prod_is_skipped(bad):
    """One malformed .prod skips that file only; siblings still parse."""
    conn = _make_connection(
        files=["SLES.prod", "module.prod", "bad.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": _BASE_XML,
            "/etc/products.d/module.prod": _ADDON_XML,
            "/etc/products.d/bad.prod": bad,
        },
    )

    system = parse_system(conn)

    assert system.get_base() == Product("SLES", "15-SP3", "x86_64")
    assert system.get_addons() == {Product("sle-module-basesystem", "15.3", "x86_64")}


def test_malformed_baseproduct_raises_unknown_system_error():
    """A malformed baseproduct cannot yield a System — specific error."""
    conn = _make_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": b"<product><arch>x86_64</arch></product>",
        },
    )

    with pytest.raises(UnknownSystemError):
        parse_system(conn)


def test_baseproduct_target_not_in_prod_files_is_tolerated():
    """A baseproduct target missing from the .prod list must not abort."""
    conn = _make_connection(
        files=["module.prod"],
        basefile="SLES.sav",  # filtered out by the .endswith('.prod') listing
        file_contents={
            "/etc/products.d/SLES.sav": _BASE_XML,
            "/etc/products.d/module.prod": _ADDON_XML,
        },
    )

    system = parse_system(conn)

    assert system.get_base() == Product("SLES", "15-SP3", "x86_64")
    assert system.get_addons() == {Product("sle-module-basesystem", "15.3", "x86_64")}


def test_dangling_baseproduct_symlink_raises_unknown_system_error():
    """A baseproduct symlink whose target file is gone fails clean.

    The open of the resolved target raises FileNotFoundError; that must
    surface as UnknownSystemError, not abort the parse with an opaque
    error.
    """
    conn = _make_connection(
        files=["module.prod"],
        basefile="SLES.prod.old",  # readlink resolves, but the file is gone
        file_contents={"/etc/products.d/module.prod": _ADDON_XML},
    )

    with pytest.raises(UnknownSystemError):
        parse_system(conn)


def test_malformed_addon_logs_single_warning_naming_file(caplog):
    """One malformed addon emits exactly one warning, with the filename."""
    conn = _make_connection(
        files=["SLES.prod", "bad.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": _BASE_XML,
            "/etc/products.d/bad.prod": b"<product><arch>x86_64</arch></product>",
        },
    )

    with caplog.at_level(logging.WARNING):
        parse_system(conn)

    warnings = [r for r in caplog.records if r.levelno == logging.WARNING]
    assert len(warnings) == 1
    assert "bad.prod" in warnings[0].getMessage()


def test_readlink_none_raises_unknown_system_error():
    """An unresolvable baseproduct symlink fails clean, not TypeError."""
    conn = _make_connection(
        files=["SLES.prod"],
        basefile=None,
        file_contents={"/etc/products.d/SLES.prod": _BASE_XML},
    )

    with pytest.raises(UnknownSystemError):
        parse_system(conn)


@pytest.mark.parametrize("bad", _MALFORMED_PRODS)
async def test_async_malformed_addon_prod_is_skipped(bad):
    conn = _make_async_connection(
        files=["SLES.prod", "module.prod", "bad.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": _BASE_XML,
            "/etc/products.d/module.prod": _ADDON_XML,
            "/etc/products.d/bad.prod": bad,
        },
    )

    system = await parse_system_async(conn)

    assert system.get_base() == Product("SLES", "15-SP3", "x86_64")
    assert system.get_addons() == {Product("sle-module-basesystem", "15.3", "x86_64")}


async def test_async_malformed_baseproduct_raises_unknown_system_error():
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": b"<product><arch>x86_64</arch></product>",
        },
    )

    with pytest.raises(UnknownSystemError):
        await parse_system_async(conn)


async def test_async_baseproduct_target_not_in_prod_files_is_tolerated():
    conn = _make_async_connection(
        files=["module.prod"],
        basefile="SLES.sav",
        file_contents={
            "/etc/products.d/SLES.sav": _BASE_XML,
            "/etc/products.d/module.prod": _ADDON_XML,
        },
    )

    system = await parse_system_async(conn)

    assert system.get_base() == Product("SLES", "15-SP3", "x86_64")
    assert system.get_addons() == {Product("sle-module-basesystem", "15.3", "x86_64")}


async def test_async_dangling_baseproduct_symlink_raises_unknown_system_error():
    conn = _make_async_connection(
        files=["module.prod"],
        basefile="SLES.prod.old",  # readlink resolves, but the file is gone
        file_contents={"/etc/products.d/module.prod": _ADDON_XML},
    )

    with pytest.raises(UnknownSystemError):
        await parse_system_async(conn)


async def test_async_readlink_none_raises_unknown_system_error():
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile=None,
        file_contents={"/etc/products.d/SLES.prod": _BASE_XML},
    )

    with pytest.raises(UnknownSystemError):
        await parse_system_async(conn)


# ---------------------------------------------------------------------------
# Mutation-kill tests: pin the exact probe paths, error/warning strings and
# filtering semantics that the happy-path assertions above leave unchecked.
# ---------------------------------------------------------------------------


def test_listdir_probed_with_products_d_path():
    """The product directory listed must be exactly /etc/products.d."""
    conn = _make_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": _BASE_XML},
    )

    parse_system(conn)

    conn.listdir.assert_called_once_with("/etc/products.d")


def test_readlink_probes_baseproduct_symlink_path():
    """The baseproduct symlink read must be the exact well-known path."""
    conn = _make_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": _BASE_XML},
    )

    parse_system(conn)

    conn.readlink.assert_called_once_with("/etc/products.d/baseproduct")


def test_readlink_none_error_message_names_symlink():
    """The unresolved-symlink error carries its specific message verbatim."""
    conn = _make_connection(
        files=["SLES.prod"],
        basefile=None,
        file_contents={"/etc/products.d/SLES.prod": _BASE_XML},
    )

    with pytest.raises(UnknownSystemError) as excinfo:
        parse_system(conn)

    assert str(excinfo.value) == ("/etc/products.d/baseproduct symlink did not resolve")


def test_unreadable_baseproduct_error_message_names_file():
    """The unreadable-baseproduct error names the offending file verbatim."""
    conn = _make_connection(
        files=["module.prod"],
        basefile="SLES.prod.old",
        file_contents={"/etc/products.d/module.prod": _ADDON_XML},
    )

    with pytest.raises(UnknownSystemError) as excinfo:
        parse_system(conn)

    assert str(excinfo.value) == "baseproduct file SLES.prod.old could not be read"


def test_malformed_baseproduct_error_and_warning_name_file(caplog):
    """A malformed base yields the exact error and a filename-tagged warning."""
    conn = _make_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": b"<product><arch>x86_64</arch></product>",
        },
    )

    with caplog.at_level(logging.WARNING), pytest.raises(UnknownSystemError) as excinfo:
        parse_system(conn)

    assert str(excinfo.value) == "baseproduct file SLES.prod is malformed"
    malformed = [
        r.getMessage()
        for r in caplog.records
        if "malformed product file" in r.getMessage()
    ]
    assert malformed
    assert "SLES.prod" in malformed[0]


def test_multi_dash_migration_addon_filtered():
    """A migration product ending in ``-migration`` is filtered by suffix.

    Uses a multi-dash name so ``rpartition('-')`` (last segment) differs
    from ``partition('-')`` (first split) — only the correct suffix match
    excludes it.
    """
    base = _prod_xml("SLES", baseversion="15", patchlevel="3")
    migration = _prod_xml("sle-we-migration", baseversion="15", patchlevel="3")
    conn = _make_connection(
        files=["SLES.prod", "mig.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": base,
            "/etc/products.d/mig.prod": migration,
        },
    )

    system = parse_system(conn)

    assert system.get_addons() == set()


def test_os_release_value_strips_only_quotes_not_x():
    """Value unquoting strips only quotes, leaving other characters intact."""
    conn = MagicMock()
    conn.listdir.side_effect = OSError("No such directory")

    def _open(path, *args, **kwargs):
        if path == "/etc/os-release":
            return _open_returning(b"ID=Xubuntu\nVERSION_ID=22.04\n")
        raise FileNotFoundError(path)

    conn.open.side_effect = _open

    system = parse_system(conn)
    assert system.get_base() == Product("Xubuntu", "22.04", "unknown")


async def test_async_listdir_probed_with_products_d_path():
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": _BASE_XML},
    )

    await parse_system_async(conn)

    conn.listdir.assert_awaited_once_with("/etc/products.d")


async def test_async_readlink_probes_baseproduct_symlink_path():
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": _BASE_XML},
    )

    await parse_system_async(conn)

    conn.readlink.assert_awaited_once_with("/etc/products.d/baseproduct")


async def test_async_readlink_none_error_message_names_symlink():
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile=None,
        file_contents={"/etc/products.d/SLES.prod": _BASE_XML},
    )

    with pytest.raises(UnknownSystemError) as excinfo:
        await parse_system_async(conn)

    assert str(excinfo.value) == ("/etc/products.d/baseproduct symlink did not resolve")


async def test_async_unreadable_baseproduct_error_message_names_file():
    conn = _make_async_connection(
        files=["module.prod"],
        basefile="SLES.prod.old",
        file_contents={"/etc/products.d/module.prod": _ADDON_XML},
    )

    with pytest.raises(UnknownSystemError) as excinfo:
        await parse_system_async(conn)

    assert str(excinfo.value) == "baseproduct file SLES.prod.old could not be read"


async def test_async_malformed_baseproduct_error_and_warning_name_file(caplog):
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": b"<product><arch>x86_64</arch></product>",
        },
    )

    with caplog.at_level(logging.WARNING), pytest.raises(UnknownSystemError) as excinfo:
        await parse_system_async(conn)

    assert str(excinfo.value) == "baseproduct file SLES.prod is malformed"
    malformed = [
        r.getMessage()
        for r in caplog.records
        if "malformed product file" in r.getMessage()
    ]
    assert malformed
    assert "SLES.prod" in malformed[0]


async def test_async_baseproduct_symlink_with_path_strips_dir():
    base = _prod_xml("SLES", baseversion="15", patchlevel="3", arch="x86_64")
    conn = _make_async_connection(
        files=["SLES.prod"],
        basefile="../SLES.prod",
        file_contents={"/etc/products.d/SLES.prod": base},
    )

    system = await parse_system_async(conn)
    assert system.get_base().name == "SLES"


async def test_async_multi_dash_migration_addon_filtered():
    base = _prod_xml("SLES", baseversion="15", patchlevel="3")
    migration = _prod_xml("sle-we-migration", baseversion="15", patchlevel="3")
    conn = _make_async_connection(
        files=["SLES.prod", "mig.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": base,
            "/etc/products.d/mig.prod": migration,
        },
    )

    system = await parse_system_async(conn)

    assert system.get_addons() == set()


async def test_async_malformed_addon_logs_warning_naming_file(caplog):
    conn = _make_async_connection(
        files=["SLES.prod", "bad.prod"],
        basefile="SLES.prod",
        file_contents={
            "/etc/products.d/SLES.prod": _BASE_XML,
            "/etc/products.d/bad.prod": b"<product><arch>x86_64</arch></product>",
        },
    )

    with caplog.at_level(logging.WARNING):
        await parse_system_async(conn)

    warnings = [r for r in caplog.records if r.levelno == logging.WARNING]
    assert len(warnings) == 1
    assert "bad.prod" in warnings[0].getMessage()


async def test_async_non_suse_os_release_without_id_warns_hostname(caplog):
    conn = MagicMock()
    conn.hostname = "asyncmystery.example.com"
    conn.listdir = AsyncMock(side_effect=FileNotFoundError("No such directory"))

    def _open(path, *args, **kwargs):
        if path == "/etc/os-release":
            return _async_open_returning(b'PRETTY_NAME="Mystery OS"\n')
        raise FileNotFoundError(path)

    conn.open.side_effect = _open

    with caplog.at_level(logging.WARNING):
        system = await parse_system_async(conn)

    assert system.get_base() == Product("linux", "", "unknown")
    warning = next(
        record.getMessage()
        for record in caplog.records
        if record.levelno == logging.WARNING
    )
    # Pin the host prefix (kills the hostname->None mutant) without
    # over-pinning the rest of the incidental warning phrasing.
    assert warning.startswith("asyncmystery.example.com:")
