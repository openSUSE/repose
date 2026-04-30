"""Tests for ``repose.target.parsers.product.parse_system``."""

from contextlib import contextmanager
from unittest.mock import MagicMock

from repose.target.parsers import Product
from repose.target.parsers.product import parse_system
from repose.types.system import System


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


def test_non_suse_falls_back_to_os_release():
    """When /etc/products.d is absent, parser opens /etc/os-release."""
    conn = MagicMock()
    conn.listdir.side_effect = OSError("No such directory")

    def _open(path, *args, **kwargs):
        if path == "/etc/os-release":
            return _open_returning(b"")
        raise FileNotFoundError(path)

    conn.open.side_effect = _open

    system = parse_system(conn)
    # Stub returns ("rhel", "7", "x86_64")
    assert system.get_base() == Product("rhel", "7", "x86_64")


def test_non_suse_without_osrelease_returns_rhel6_default():
    conn = MagicMock()
    conn.listdir.side_effect = OSError("No such directory")
    conn.open.side_effect = FileNotFoundError("/etc/os-release")

    system = parse_system(conn)
    assert system.get_base() == Product("rhel", "6", "x86_64")
