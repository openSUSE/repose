"""Tests for ``repose.aiossh.AsyncConnection``.

Two flavours of test live here:

- **Unit tests** that monkeypatch ``asyncssh.connect`` (and the SFTP
  surface) so we exercise our wiring — option resolution, host-key
  policy translation, password fallback, the ``CommandTimeout``
  translation — without spinning a server. These are fast and cover
  the bulk of the branches.

- **One in-process integration test** that boots a real asyncssh
  server on an ephemeral port and runs an actual command through it.
  This guards against API drift in asyncssh (e.g. a future ``run``
  signature change) that mocks would silently miss.
"""

from __future__ import annotations

import asyncio
from typing import Any
from unittest.mock import AsyncMock, MagicMock

import asyncssh
import pytest

from repose.aiossh import AsyncConnection, CommandTimeout
from repose.types.connection_config import ConnectionConfig


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _stub_ssh_config(monkeypatch):
    """Force the optional ``~/.ssh/config`` lookup to return an empty dict.

    Otherwise tests would non-deterministically pick up the developer's
    real SSH config and bleed it into the connect kwargs assertions.
    """
    monkeypatch.setattr("repose.aiossh._parse_openssh_config", lambda host: {})


@pytest.fixture
def fake_conn():
    """A MagicMock standing in for an :class:`asyncssh.SSHClientConnection`."""
    conn = MagicMock()
    conn.is_closed.return_value = False
    conn.close = MagicMock()
    conn.wait_closed = AsyncMock(return_value=None)
    return conn


# ---------------------------------------------------------------------------
# Construction
# ---------------------------------------------------------------------------


def test_repr_contains_user_host_port():
    c = AsyncConnection("h", "u", 22)
    text = repr(c)
    assert "u" in text and "h" in text and "22" in text


def test_invalid_port_falls_back_to_22():
    c = AsyncConnection("h", "u", "not-a-port")
    assert c.port == 22


def test_timeout_from_config_when_not_passed():
    cfg = ConnectionConfig(timeout=42.0)
    c = AsyncConnection("h", "u", 22, config=cfg)
    assert c.timeout == 42.0


def test_explicit_positional_timeout_wins_over_config():
    cfg = ConnectionConfig(timeout=42.0)
    c = AsyncConnection("h", "u", 22, 9.0, config=cfg)
    assert c.timeout == 9.0


# ---------------------------------------------------------------------------
# connect — host-key policy translation
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "policy, expected_known_hosts",
    [
        ("no", None),
        ("off", None),
        ("yes", ()),  # asyncssh default = ~/.ssh/known_hosts
        # accept-new is file-dependent (the real path when the file is
        # present, else the empty trust set), so it is covered by the
        # dedicated tests below rather than this static table.
    ],
)
async def test_known_hosts_arg_translation(
    monkeypatch, fake_conn, policy, expected_known_hosts
):
    cfg = ConnectionConfig(host_key_policy=policy)
    captured: dict[str, Any] = {}

    async def fake_connect(**kwargs):
        captured.update(kwargs)
        return fake_conn

    monkeypatch.setattr(asyncssh, "connect", fake_connect)

    c = AsyncConnection("h", "u", 22, config=cfg)
    await c.connect()

    assert captured["known_hosts"] == expected_known_hosts


async def test_accept_new_engages_validator(monkeypatch, fake_conn, tmp_path):
    """Regression: with no known_hosts file yet, ``accept-new`` still
    engages the validator instead of disabling checking.

    asyncssh calls ``SSHClient.validate_host_public_key`` only when
    ``_trusted_host_keys`` is non-None, and substitutes the system
    ``~/.ssh/known_hosts`` for any *falsy* value (so ``b""``/``None``
    would bypass the validator). With the file absent the arg is the
    truthy empty trust set, so every offered key defers to
    ``_AcceptNewClient``.
    """
    kh = tmp_path / "known_hosts"  # not created: missing-file case
    cfg = ConnectionConfig(host_key_policy="accept-new", known_hosts=kh)
    captured: dict[str, Any] = {}

    async def fake_connect(**kwargs):
        captured.update(kwargs)
        return fake_conn

    monkeypatch.setattr(asyncssh, "connect", fake_connect)

    c = AsyncConnection("h", "u", 22, config=cfg)
    await c.connect()

    kh_arg = captured["known_hosts"]
    assert kh_arg  # truthy: asyncssh will not load system known_hosts
    trusted, _ca, _revoked, *_ = asyncssh.match_known_hosts(kh_arg, "h", "0.0.0.0", 22)
    assert list(trusted) == []  # empty trust -> validator sees every key
    assert issubclass(captured["client_factory"], asyncssh.SSHClient)


async def test_accept_new_existing_file_passed_to_asyncssh(
    monkeypatch, fake_conn, tmp_path
):
    """When known_hosts exists, asyncssh receives the file path so it
    does the native matching / algorithm negotiation."""
    kh = tmp_path / "known_hosts"
    key = asyncssh.generate_private_key("ssh-ed25519")
    kh.write_text(f"h {key.export_public_key().decode().strip()}\n")
    cfg = ConnectionConfig(host_key_policy="accept-new", known_hosts=kh)
    captured: dict[str, Any] = {}

    async def fake_connect(**kwargs):
        captured.update(kwargs)
        return fake_conn

    monkeypatch.setattr(asyncssh, "connect", fake_connect)

    c = AsyncConnection("h", "u", 22, config=cfg)
    await c.connect()

    assert captured["known_hosts"] == str(kh)  # asyncssh reads it natively
    assert issubclass(captured["client_factory"], asyncssh.SSHClient)


async def test_known_hosts_path_threaded_through(monkeypatch, fake_conn, tmp_path):
    kh = tmp_path / "known_hosts"
    kh.write_text("")
    cfg = ConnectionConfig(host_key_policy="yes", known_hosts=kh)

    captured: dict[str, Any] = {}

    async def fake_connect(**kwargs):
        captured.update(kwargs)
        return fake_conn

    monkeypatch.setattr(asyncssh, "connect", fake_connect)

    c = AsyncConnection("h", "u", 22, config=cfg)
    await c.connect()

    assert captured["known_hosts"] == str(kh)


# ---------------------------------------------------------------------------
# connect — auth fallback
# ---------------------------------------------------------------------------


async def test_password_fallback_on_permission_denied(monkeypatch, fake_conn):
    """First connect raises ``PermissionDenied``; getpass + retry succeeds."""
    calls: list[dict[str, Any]] = []

    async def fake_connect(**kwargs):
        calls.append(kwargs)
        if len(calls) == 1:
            raise asyncssh.PermissionDenied(reason="no key")
        return fake_conn

    monkeypatch.setattr(asyncssh, "connect", fake_connect)
    monkeypatch.setattr("getpass.getpass", lambda: "secret")

    c = AsyncConnection("h", "u", 22)
    await c.connect()

    assert len(calls) == 2
    assert calls[1]["password"] == "secret"
    # The second attempt explicitly disables key-auth so asyncssh
    # doesn't bounce off the agent before reaching the password path.
    assert calls[1]["client_keys"] == ()


async def test_password_fallback_wrong_password_reraises(monkeypatch, fake_conn):
    async def fake_connect(**kwargs):
        raise asyncssh.PermissionDenied(reason="no key/password")

    monkeypatch.setattr(asyncssh, "connect", fake_connect)
    monkeypatch.setattr("getpass.getpass", lambda: "bad")

    c = AsyncConnection("h", "u", 22)
    with pytest.raises(asyncssh.PermissionDenied):
        await c.connect()


async def test_disconnect_error_propagates(monkeypatch):
    async def fake_connect(**kwargs):
        raise asyncssh.DisconnectError(
            reason="boom", code=asyncssh.DISC_CONNECTION_LOST
        )

    monkeypatch.setattr(asyncssh, "connect", fake_connect)
    c = AsyncConnection("h", "u", 22)
    with pytest.raises(asyncssh.DisconnectError):
        await c.connect()


async def test_oserror_propagates(monkeypatch):
    async def fake_connect(**kwargs):
        raise OSError("network unreachable")

    monkeypatch.setattr(asyncssh, "connect", fake_connect)
    c = AsyncConnection("h", "u", 22)
    with pytest.raises(OSError):
        await c.connect()


# ---------------------------------------------------------------------------
# connect — accept-new persists key on first contact
# ---------------------------------------------------------------------------


async def test_accept_new_installs_client_factory(monkeypatch, fake_conn, tmp_path):
    """``accept-new`` wires a custom ``SSHClient`` via ``client_factory=``.

    Validation lives in that subclass: asyncssh does the native matching
    and defers any not-already-trusted key to the subclass, which is the
    effective arbiter for accept-new.
    """
    kh = tmp_path / "known_hosts"
    kh.write_text("")
    cfg = ConnectionConfig(host_key_policy="accept-new", known_hosts=kh)

    captured: dict[str, Any] = {}

    async def fake_connect(**kwargs):
        captured.update(kwargs)
        return fake_conn

    monkeypatch.setattr(asyncssh, "connect", fake_connect)

    c = AsyncConnection("h", "u", 22, config=cfg)
    await c.connect()

    assert captured["known_hosts"]  # truthy: no system-file substitution
    assert "client_factory" in captured
    assert issubclass(captured["client_factory"], asyncssh.SSHClient)


async def test_accept_new_client_persists_unknown_key(tmp_path):
    """Drive the ``_AcceptNewClient`` validation callback directly.

    No network needed — ``validate_host_public_key`` is a synchronous
    method we can call against a real ``asyncssh`` key on an empty
    known_hosts file.
    """
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    kh.write_text("")  # empty: first-contact path
    key = asyncssh.generate_private_key("ssh-rsa")

    factory = _make_accept_new_client(kh, expected_host="h")
    client = factory()
    accepted = client.validate_host_public_key(
        host="h", addr="127.0.0.1", port=22, key=key
    )
    assert accepted is True

    body = kh.read_text()
    assert body.startswith("h ")
    assert "ssh-rsa" in body


async def test_accept_new_client_rejects_changed_key(tmp_path):
    """A different key for an already-pinned host must be rejected."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    pinned_key = asyncssh.generate_private_key("ssh-rsa")
    pinned_pub = pinned_key.export_public_key().decode().strip()
    kh.write_text(f"h {pinned_pub}\n")

    factory = _make_accept_new_client(kh, expected_host="h")
    client = factory()

    # A *new* key for the same host: changed-key → reject.
    other_key = asyncssh.generate_private_key("ssh-rsa")
    accepted = client.validate_host_public_key(
        host="h", addr="127.0.0.1", port=22, key=other_key
    )
    assert accepted is False

    # The same pinned key is still accepted.
    accepted = client.validate_host_public_key(
        host="h", addr="127.0.0.1", port=22, key=pinned_key
    )
    assert accepted is True


async def test_accept_new_persist_preserves_trailing_newline(tmp_path):
    """A known_hosts file without a trailing newline must not have its
    last entry merged with the newly persisted one."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    # Existing entry for a *different* host, no trailing newline.
    other = asyncssh.generate_private_key("ssh-ed25519")
    other_line = f"other {other.export_public_key().decode().strip()}"
    kh.write_bytes(other_line.encode())

    key = asyncssh.generate_private_key("ssh-rsa")
    factory = _make_accept_new_client(kh, expected_host="h")
    accepted = factory().validate_host_public_key(
        host="h", addr="127.0.0.1", port=22, key=key
    )
    assert accepted is True

    lines = kh.read_text().splitlines()
    assert len(lines) == 2  # not merged into one corrupt line
    assert lines[0] == other_line
    assert lines[1].startswith("h ")


async def test_accept_new_no_duplicate_on_factory_reuse(tmp_path):
    """The password-auth fallback re-runs the validator with the same
    factory; a first-contact key must be persisted only once."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    kh.write_text("")
    key = asyncssh.generate_private_key("ssh-rsa")

    factory = _make_accept_new_client(kh, expected_host="h")
    # Two instances from the same factory mirror the key-auth attempt
    # followed by the password fallback (same client_factory reused).
    for _ in range(2):
        accepted = factory().validate_host_public_key(
            host="h", addr="127.0.0.1", port=22, key=key
        )
        assert accepted is True

    host_lines = [ln for ln in kh.read_text().splitlines() if ln.startswith("h ")]
    assert len(host_lines) == 1  # no duplicate entry


async def test_accept_new_matches_pin_with_trailing_comment(tmp_path):
    """A pinned entry carrying a trailing comment still matches the
    comment-less key the server offers (key equality ignores comments)."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    key = asyncssh.generate_private_key("ssh-rsa")
    pub = key.export_public_key().decode().strip()
    kh.write_text(f"h {pub} admin@ops\n")

    factory = _make_accept_new_client(kh, expected_host="h")
    accepted = factory().validate_host_public_key(
        host="h", addr="127.0.0.1", port=22, key=key
    )
    assert accepted is True


async def test_accept_new_honours_non_default_port_pin(tmp_path):
    """A host pinned as ``[host]:port`` is matched on that port, so a
    changed key is rejected instead of being taken as first contact."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    pinned = asyncssh.generate_private_key("ssh-rsa")
    pinned_pub = pinned.export_public_key().decode().strip()
    kh.write_text(f"[h]:2222 {pinned_pub}\n")

    factory = _make_accept_new_client(kh, expected_host="h")
    client = factory()

    other = asyncssh.generate_private_key("ssh-rsa")
    changed = client.validate_host_public_key(
        host="h", addr="127.0.0.1", port=2222, key=other
    )
    assert changed is False  # changed key on the pinned port → reject

    same = client.validate_host_public_key(
        host="h", addr="127.0.0.1", port=2222, key=pinned
    )
    assert same is True


async def test_accept_new_persists_non_default_port_in_bracket_form(tmp_path):
    """First contact on a non-default port persists an ``[host]:port``
    entry so it matches on the next connect."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    kh.write_text("")
    key = asyncssh.generate_private_key("ssh-rsa")

    factory = _make_accept_new_client(kh, expected_host="h")
    accepted = factory().validate_host_public_key(
        host="h", addr="127.0.0.1", port=2222, key=key
    )
    assert accepted is True
    assert kh.read_text().startswith("[h]:2222 ")


async def test_accept_new_rejects_revoked_key(tmp_path):
    """A key explicitly revoked in known_hosts is refused, not taken as
    first contact and re-persisted."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    revoked = asyncssh.generate_private_key("ssh-rsa")
    revoked_pub = revoked.export_public_key().decode().strip()
    kh.write_text(f"@revoked h {revoked_pub}\n")
    before = kh.read_text()

    factory = _make_accept_new_client(kh, expected_host="h")
    accepted = factory().validate_host_public_key(
        host="h", addr="127.0.0.1", port=22, key=revoked
    )
    assert accepted is False
    assert kh.read_text() == before  # revoked key not persisted back


async def test_accept_new_matches_pin_under_configured_alias(tmp_path):
    """A key pinned under the configured alias is accepted even when
    asyncssh resolves and reports a different HostName; a changed key for
    that aliased host is still refused rather than taken as first
    contact."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    key = asyncssh.generate_private_key("ssh-rsa")
    pub = key.export_public_key().decode().strip()
    kh.write_text(f"rh1 {pub}\n")  # pinned under the alias

    # expected_host is the alias; asyncssh calls back with the resolved
    # HostName, which is not what the pin is stored under.
    factory = _make_accept_new_client(kh, expected_host="rh1")
    client = factory()

    accepted = client.validate_host_public_key(
        host="10.0.0.5", addr="10.0.0.5", port=22, key=key
    )
    assert accepted is True

    other = asyncssh.generate_private_key("ssh-rsa")
    changed = client.validate_host_public_key(
        host="10.0.0.5", addr="10.0.0.5", port=22, key=other
    )
    assert changed is False


async def test_accept_new_malformed_known_hosts_raises(tmp_path):
    """We use asyncssh's native parser and deliberately do NOT skip bad
    lines the way OpenSSH does: a malformed known_hosts surfaces as an
    error instead of being silently ignored."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    kh.write_text("@badmarker garbage line\n")

    with pytest.raises(ValueError):
        _make_accept_new_client(kh, expected_host="h")


async def test_accept_new_accepts_host_certificate_first_contact(tmp_path):
    """accept-new trusts a host certificate on first contact (TOFU), so
    the CA-key hook accepts rather than hard-rejecting cert hosts."""
    from repose.aiossh import _make_accept_new_client

    kh = tmp_path / "known_hosts"
    kh.write_text("")
    ca = asyncssh.generate_private_key("ssh-rsa")

    factory = _make_accept_new_client(kh, expected_host="h")
    accepted = factory().validate_host_ca_key(
        host="h", addr="127.0.0.1", port=22, key=ca
    )
    assert accepted is True


async def test_strict_policy_passes_known_hosts_to_asyncssh(
    monkeypatch, fake_conn, tmp_path
):
    """``yes`` lets asyncssh's native checker reject unknowns."""
    kh = tmp_path / "known_hosts"
    kh.write_text("")
    cfg = ConnectionConfig(host_key_policy="yes", known_hosts=kh)

    captured: dict[str, Any] = {}

    async def fake_connect(**kwargs):
        captured.update(kwargs)
        return fake_conn

    monkeypatch.setattr(asyncssh, "connect", fake_connect)

    c = AsyncConnection("h", "u", 22, config=cfg)
    await c.connect()

    assert captured["known_hosts"] == str(kh)
    assert "client_factory" not in captured


# ---------------------------------------------------------------------------
# run / timeout
# ---------------------------------------------------------------------------


async def test_run_returns_stdout_stderr_exitcode(fake_conn):
    result = MagicMock()
    result.stdout = "hello\n"
    result.stderr = ""
    result.exit_status = 0
    fake_conn.run = AsyncMock(return_value=result)

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    stdout, stderr, exitcode = await c.run("echo hello")

    assert (stdout, stderr, exitcode) == ("hello\n", "", 0)
    fake_conn.run.assert_awaited_once()
    # Verify timeout is plumbed from the connection.
    args, kwargs = fake_conn.run.call_args
    assert kwargs["timeout"] == c.timeout
    assert kwargs["check"] is False


async def test_run_translates_timeout_to_command_timeout(fake_conn):
    fake_conn.run = AsyncMock(side_effect=asyncio.TimeoutError())

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    with pytest.raises(CommandTimeout):
        await c.run("sleep 1000")


async def test_run_handles_none_exit_status(fake_conn):
    """A torn-down session reports ``exit_status=None``; we translate to -1."""
    result = MagicMock()
    result.stdout = ""
    result.stderr = "killed"
    result.exit_status = None
    fake_conn.run = AsyncMock(return_value=result)

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    _, _, exitcode = await c.run("kill 1")
    assert exitcode == -1


# ---------------------------------------------------------------------------
# sftp
# ---------------------------------------------------------------------------


async def test_listdir_filters_dot_entries(fake_conn):
    sftp = MagicMock()
    sftp.listdir = AsyncMock(return_value=[".", "..", "a.prod", "b.prod"])
    fake_conn.start_sftp_client = AsyncMock(return_value=sftp)

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    result = await c.listdir("/etc/products.d")
    assert result == ["a.prod", "b.prod"]


async def test_sftp_client_is_cached(fake_conn):
    """Multiple SFTP calls share one subsystem (paramiko parity)."""
    sftp = MagicMock()
    sftp.listdir = AsyncMock(return_value=[])
    sftp.readlink = AsyncMock(return_value="/etc/products.d/SLES.prod")
    fake_conn.start_sftp_client = AsyncMock(return_value=sftp)

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    await c.listdir(".")
    await c.readlink("/etc/products.d/baseproduct")
    await c.listdir(".")

    assert fake_conn.start_sftp_client.await_count == 1


async def test_open_yields_iterable_with_content(fake_conn):
    """``async with open() as f`` exposes ``for line in f`` semantics."""

    class FakeFile:
        def __init__(self, data: str) -> None:
            self._data = data

        async def __aenter__(self) -> "FakeFile":
            return self

        async def __aexit__(self, *exc):
            pass

        async def read(self):
            return self._data

    sftp = MagicMock()
    sftp.open = MagicMock(return_value=FakeFile("line1\nline2\n"))
    fake_conn.start_sftp_client = AsyncMock(return_value=sftp)

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    async with c.open("/etc/os-release") as f:
        lines = list(f)

    assert lines == ["line1\n", "line2\n"]


async def test_listdir_translates_missing_dir_to_filenotfound(fake_conn):
    """A missing remote directory surfaces as ``FileNotFoundError``.

    asyncssh's ``SFTPNoSuchFile`` does not derive from ``OSError``;
    backend-neutral callers (``parse_system_async``) ``except OSError``,
    so the translation is what lets non-SUSE hosts be detected.
    """
    sftp = MagicMock()
    sftp.listdir = AsyncMock(side_effect=asyncssh.SFTPNoSuchFile("no dir"))
    fake_conn.start_sftp_client = AsyncMock(return_value=sftp)

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    with pytest.raises(FileNotFoundError):
        await c.listdir("/etc/products.d")


async def test_open_translates_missing_file_to_filenotfound(fake_conn):
    """A missing remote file surfaces as ``FileNotFoundError`` on open."""
    sftp = MagicMock()
    sftp.open = MagicMock(side_effect=asyncssh.SFTPNoSuchFile("no file"))
    fake_conn.start_sftp_client = AsyncMock(return_value=sftp)

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    with pytest.raises(FileNotFoundError):
        async with c.open("/etc/os-release"):
            pass


async def test_sftp_permission_denied_translates_to_permissionerror(fake_conn):
    sftp = MagicMock()
    sftp.listdir = AsyncMock(side_effect=asyncssh.SFTPPermissionDenied("nope"))
    fake_conn.start_sftp_client = AsyncMock(return_value=sftp)

    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    with pytest.raises(PermissionError):
        await c.listdir("/etc/products.d")


# ---------------------------------------------------------------------------
# close
# ---------------------------------------------------------------------------


async def test_close_is_idempotent(fake_conn):
    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn
    await c.close()
    await c.close()  # second call must not raise
    # Underlying ``close`` should only fire once because the second
    # ``close()`` sees ``self._conn is None``.
    assert fake_conn.close.call_count == 1


def test_is_active_false_before_connect():
    c = AsyncConnection("h", "u", 22)
    assert c.is_active() is False


# ---------------------------------------------------------------------------
# Integration: real asyncssh server on an ephemeral port
# ---------------------------------------------------------------------------


class _EchoServer(asyncssh.SSHServer):
    """Accepts any client and any command; trivial echo session."""

    def begin_auth(self, username: str) -> bool:
        # ``False`` here means "no auth required"; matches the
        # ``--ssh-backend asyncssh`` smoke-test against a dev host.
        return False

    def password_auth_supported(self) -> bool:
        return True

    def public_key_auth_supported(self) -> bool:
        return True


async def _handle_session(process: asyncssh.SSHServerProcess) -> None:
    cmd = process.command or ""
    if cmd == "fail":
        process.exit(7)
        return
    process.stdout.write(f"echo:{cmd}\n")
    process.exit(0)


@pytest.mark.integration
async def test_integration_real_server_run_and_exit(tmp_path, monkeypatch):
    """End-to-end smoke test against an in-process asyncssh server.

    Verifies that the entire stack — connect → run → stdout/exit_code
    → close — produces the expected shape against a real asyncssh
    server, not just our mock surface. The test generates a throwaway
    server-host key, disables host-key checking on the client side,
    and runs two commands (one OK, one failing) to confirm exit-code
    propagation.
    """
    monkeypatch.setattr("repose.aiossh._parse_openssh_config", lambda host: {})

    server_key = asyncssh.generate_private_key("ssh-rsa")
    server = await asyncssh.create_server(
        _EchoServer,
        "127.0.0.1",
        0,
        server_host_keys=[server_key],
        process_factory=_handle_session,
    )
    try:
        port = server.sockets[0].getsockname()[1]
        cfg = ConnectionConfig(host_key_policy="off")
        c = AsyncConnection("127.0.0.1", "u", port, config=cfg)
        await c.connect()

        out, err, rc = await c.run("hello")
        assert rc == 0
        assert "echo:hello" in out

        out, err, rc = await c.run("fail")
        assert rc == 7

        await c.close()
    finally:
        server.close()
        await server.wait_closed()


# ---------------------------------------------------------------------------
# Transactional reboot support (async mirror of the paramiko tests)
# ---------------------------------------------------------------------------


async def test_async_fire_and_forget_dispatches_then_closes(fake_conn):
    fake_conn.create_process = AsyncMock()
    c = AsyncConnection("h", "u", 22)
    c._conn = fake_conn

    await c.fire_and_forget("systemctl reboot")

    fake_conn.create_process.assert_awaited_once_with("systemctl reboot")
    fake_conn.close.assert_called()
    assert c._conn is None  # closed


async def test_async_boot_id_returns_stripped(monkeypatch):
    c = AsyncConnection("h", "u", 22)
    monkeypatch.setattr(c, "run", AsyncMock(return_value=("abc-123\n", "", 0)))
    assert await c.boot_id() == "abc-123"


async def test_async_wait_reconnect_succeeds_after_retries(monkeypatch, fake_conn):
    c = AsyncConnection("h", "u", 22)
    calls = {"n": 0}

    async def fake_connect():
        calls["n"] += 1
        if calls["n"] >= 2:
            c._conn = fake_conn  # host back up on the 2nd attempt

    monkeypatch.setattr(c, "connect", fake_connect)

    ok = await c.wait_reconnect(retry=5, timeout=0, backoff=False)

    assert ok is True
    assert calls["n"] == 2


async def test_async_wait_reconnect_gives_up(monkeypatch):
    c = AsyncConnection("h", "u", 22)
    calls = {"n": 0}

    async def fake_connect():
        calls["n"] += 1  # never becomes active

    monkeypatch.setattr(c, "connect", fake_connect)

    ok = await c.wait_reconnect(retry=2, timeout=0, backoff=False)

    assert ok is False
    assert calls["n"] == 3  # retry=N → N+1 attempts
