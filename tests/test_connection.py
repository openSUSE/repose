"""Tests for ``repose.connection``."""

import logging
import threading
import logging
import os
from unittest.mock import MagicMock

import paramiko
import pytest

import repose.connection
from repose.connection import (
    _RECONNECT_FORCE_AFTER,
    _RECONNECT_MAX_ATTEMPTS,
    CommandTimeout,
    Connection,
)
from repose.connection_policy import AcceptNewPolicy
from repose.types.connection_config import ConnectionConfig


@pytest.fixture(autouse=True)
def _stub_sshconfig(monkeypatch):
    """Avoid touching the user's ~/.ssh/config in any test."""
    monkeypatch.setattr(paramiko.config.SSHConfig, "parse", lambda self, fd: None)


def test_connection_with_mock(mock_ssh_client):
    conn = Connection("dummy_host", "dummy_user", 22)
    conn.connect()
    mock_ssh_client.connect.assert_called_once_with(
        hostname="dummy_host",
        port=22,
        username="dummy_user",
        key_filename=None,
        sock=None,
    )
    conn.close()
    mock_ssh_client.close.assert_called_once()


def test_invalid_port_falls_back_to_22(mock_ssh_client):
    conn = Connection("h", "u", "not-a-port")
    assert conn.port == 22


def test_repr_contains_user_and_host(mock_ssh_client):
    conn = Connection("h", "u", 22)
    text = repr(conn)
    assert "u" in text and "h" in text and "22" in text


def test_auth_failure_falls_back_to_password(mock_ssh_client, monkeypatch):
    """First connect() raises AuthenticationException; password prompt
    triggers a second connect() with the typed password."""
    mock_ssh_client.connect.side_effect = [
        paramiko.AuthenticationException("no key"),
        None,  # success on second attempt
    ]
    monkeypatch.setattr("getpass.getpass", lambda: "secret")

    conn = Connection("h", "u", 22)
    conn.connect()

    # Two attempts; second uses password keyword
    assert mock_ssh_client.connect.call_count == 2
    second_call = mock_ssh_client.connect.call_args_list[1]
    assert second_call.kwargs["password"] == "secret"


def test_password_failure_reraises(mock_ssh_client, monkeypatch):
    mock_ssh_client.connect.side_effect = paramiko.AuthenticationException("wrong")
    monkeypatch.setattr("getpass.getpass", lambda: "bad")

    conn = Connection("h", "u", 22)
    with pytest.raises(paramiko.AuthenticationException):
        conn.connect()


def test_bad_host_key_refuses_without_password_prompt(
    mock_ssh_client, monkeypatch, caplog
):
    """A changed host key must fail fast: no password prompt, no retry,
    a clear error log, and the exception propagated."""
    mock_ssh_client.connect.side_effect = paramiko.BadHostKeyException(
        "h", MagicMock(), MagicMock()
    )
    prompt = MagicMock()
    monkeypatch.setattr("getpass.getpass", prompt)

    conn = Connection("h", "u", 22)
    with caplog.at_level(logging.ERROR, logger="repose.connection"):
        with pytest.raises(paramiko.BadHostKeyException):
            conn.connect()

    prompt.assert_not_called()
    mock_ssh_client.connect.assert_called_once()
    assert any(
        record.levelno == logging.ERROR
        and "host key verification failed for h" in record.getMessage()
        for record in caplog.records
    )


def test_ssh_exception_propagates(mock_ssh_client):
    mock_ssh_client.connect.side_effect = paramiko.SSHException("boom")
    conn = Connection("h", "u", 22)
    with pytest.raises(paramiko.SSHException):
        conn.connect()


def test_generic_exception_propagates(mock_ssh_client):
    mock_ssh_client.connect.side_effect = RuntimeError("network gone")
    conn = Connection("h", "u", 22)
    with pytest.raises(RuntimeError):
        conn.connect()


def test_command_timeout_str_returns_repr_of_command():
    t = CommandTimeout("ls -la")
    assert str(t) == repr("ls -la")


def test_is_active_when_no_transport(mock_ssh_client):
    mock_ssh_client._transport = None
    conn = Connection("h", "u", 22)
    assert conn.is_active() in (False, None)


def test_is_active_true_when_transport_active(mock_ssh_client):
    transport = MagicMock()
    transport.is_active.return_value = True
    mock_ssh_client._transport = transport

    conn = Connection("h", "u", 22)
    assert conn.is_active() is True


def test_listdir_uses_sftp(mock_ssh_client):
    sftp = MagicMock()
    sftp.listdir.return_value = ["a", "b"]
    mock_ssh_client.open_sftp.return_value = sftp

    conn = Connection("h", "u", 22)
    assert conn.listdir("/etc") == ["a", "b"]
    sftp.listdir.assert_called_once_with("/etc")
    sftp.close.assert_called_once()


def test_listdir_closes_sftp_when_op_raises(mock_ssh_client):
    """A failing listdir op must still close the SFTP client (channel leak)."""
    sftp = MagicMock()
    sftp.listdir.side_effect = FileNotFoundError("/missing")
    mock_ssh_client.open_sftp.return_value = sftp

    conn = Connection("h", "u", 22)
    with pytest.raises(FileNotFoundError):
        conn.listdir("/missing")
    sftp.close.assert_called_once()


def test_readlink_uses_sftp(mock_ssh_client):
    sftp = MagicMock()
    sftp.readlink.return_value = "/target"
    mock_ssh_client.open_sftp.return_value = sftp

    conn = Connection("h", "u", 22)
    assert conn.readlink("/link") == "/target"
    sftp.readlink.assert_called_once_with("/link")


def test_readlink_closes_sftp_when_op_raises(mock_ssh_client):
    """A failing readlink op must still close the SFTP client (channel leak)."""
    sftp = MagicMock()
    sftp.readlink.side_effect = PermissionError("denied")
    mock_ssh_client.open_sftp.return_value = sftp

    conn = Connection("h", "u", 22)
    with pytest.raises(PermissionError):
        conn.readlink("/protected/link")
    sftp.close.assert_called_once()


def test_open_returns_sftp_file(mock_ssh_client):
    sftp = MagicMock()
    handle = MagicMock()
    sftp.open.return_value = handle
    mock_ssh_client.open_sftp.return_value = sftp

    conn = Connection("h", "u", 22)
    result = conn.open("/path/to/file")
    assert result is handle


def test_open_raises_propagates_after_close(mock_ssh_client):
    sftp = MagicMock(spec=paramiko.SFTPClient)
    sftp.open.side_effect = OSError("nope")
    mock_ssh_client.open_sftp.return_value = sftp

    conn = Connection("h", "u", 22)
    with pytest.raises(OSError):
        conn.open("/missing")
    sftp.close.assert_called_once()


def test_close_session_static_handles_none():
    # Should not raise when given None
    Connection.close_session(None)


def test_close_session_static_swallows_errors():
    s = MagicMock()
    s.shutdown.side_effect = RuntimeError("already closed")
    # No exception should escape
    Connection.close_session(s)


def test_reconnect_when_active_does_nothing(mock_ssh_client):
    transport = MagicMock()
    transport.is_active.return_value = True
    mock_ssh_client._transport = transport

    conn = Connection("h", "u", 22)
    # Should be a no-op since is_active returns True
    conn.reconnect()
    # connect() not called via reconnect
    mock_ssh_client.connect.assert_not_called()


# ---------------------------------------------------------------------------
# Host-key policy (PR 12) — verify each policy mode wires the right
# paramiko policy object through ``set_missing_host_key_policy`` and the
# ``known_hosts`` override flips load_system_host_keys → load_host_keys.
# ---------------------------------------------------------------------------


def test_host_key_policy_yes_uses_reject_policy(mock_ssh_client):
    """``host_key_policy='yes'`` installs ``paramiko.RejectPolicy``."""
    cfg = ConnectionConfig(host_key_policy="yes")
    conn = Connection("h", "u", 22, config=cfg)
    conn.connect()

    mock_ssh_client.set_missing_host_key_policy.assert_called_once()
    policy = mock_ssh_client.set_missing_host_key_policy.call_args.args[0]
    assert isinstance(policy, paramiko.RejectPolicy)


def test_host_key_policy_accept_new_uses_accept_new_policy(mock_ssh_client):
    """``host_key_policy='accept-new'`` (default) installs ``AcceptNewPolicy``."""
    cfg = ConnectionConfig(host_key_policy="accept-new")
    conn = Connection("h", "u", 22, config=cfg)
    conn.connect()

    mock_ssh_client.set_missing_host_key_policy.assert_called_once()
    policy = mock_ssh_client.set_missing_host_key_policy.call_args.args[0]
    assert isinstance(policy, AcceptNewPolicy)


def test_host_key_policy_off_uses_auto_add(mock_ssh_client):
    """``host_key_policy='off'`` preserves pre-PR-12 ``AutoAddPolicy``."""
    cfg = ConnectionConfig(host_key_policy="off")
    conn = Connection("h", "u", 22, config=cfg)
    conn.connect()

    mock_ssh_client.set_missing_host_key_policy.assert_called_once()
    policy = mock_ssh_client.set_missing_host_key_policy.call_args.args[0]
    assert isinstance(policy, paramiko.AutoAddPolicy)


def test_known_hosts_loads_custom_file(mock_ssh_client, tmp_path):
    """``known_hosts=<path>`` calls ``load_host_keys`` (not system)."""
    kh = tmp_path / "kh"
    kh.write_text("")  # empty but readable
    cfg = ConnectionConfig(known_hosts=kh)
    conn = Connection("h", "u", 22, config=cfg)
    conn.connect()

    mock_ssh_client.load_host_keys.assert_called_once_with(str(kh))
    mock_ssh_client.load_system_host_keys.assert_not_called()


def test_accept_new_policy_adds_unknown_key():
    """``AcceptNewPolicy.missing_host_key`` records the key in-memory."""
    client = MagicMock()
    key = MagicMock()
    key.get_name.return_value = "ssh-ed25519"

    policy = AcceptNewPolicy()  # no path → no persistence attempt
    policy.missing_host_key(client, "newhost.example.com", key)

    client.get_host_keys.return_value.add.assert_called_once_with(
        "newhost.example.com", "ssh-ed25519", key
    )
    client.save_host_keys.assert_not_called()


def test_accept_new_defaults_known_hosts_to_user_file(mock_ssh_client, monkeypatch):
    """Without a config override, accept-new persists to ~/.ssh/known_hosts."""
    monkeypatch.setattr(os.path, "expanduser", lambda p: p.replace("~", "/home/fake"))
    cfg = ConnectionConfig(host_key_policy="accept-new")  # known_hosts=None
    conn = Connection("h", "u", 22, config=cfg)
    conn.connect()

    # The default path still goes through the system loader (missing
    # file tolerated); only the *persist* target is resolved eagerly.
    mock_ssh_client.load_system_host_keys.assert_called_once_with()
    policy = mock_ssh_client.set_missing_host_key_policy.call_args.args[0]
    assert isinstance(policy, AcceptNewPolicy)
    assert policy.known_hosts_path == "/home/fake/.ssh/known_hosts"


def test_accept_new_uses_configured_known_hosts(mock_ssh_client, tmp_path):
    """A ``known_hosts`` config override is also the persist target."""
    kh = tmp_path / "kh"
    kh.write_text("")
    cfg = ConnectionConfig(host_key_policy="accept-new", known_hosts=kh)
    conn = Connection("h", "u", 22, config=cfg)
    conn.connect()

    mock_ssh_client.load_host_keys.assert_called_once_with(str(kh))
    policy = mock_ssh_client.set_missing_host_key_policy.call_args.args[0]
    assert isinstance(policy, AcceptNewPolicy)
    assert policy.known_hosts_path == str(kh)


def test_accept_new_first_contact_creates_known_hosts(tmp_path):
    """First contact ever: no known_hosts file (nor parent directory).

    The policy must create both with safe permissions and write exactly
    one well-formed line, while also trusting the key in-memory.
    """
    known_hosts = tmp_path / ".ssh" / "known_hosts"  # parent missing too
    client = paramiko.SSHClient()
    key = paramiko.ECDSAKey.generate()

    policy = AcceptNewPolicy(known_hosts_path=str(known_hosts))
    policy.missing_host_key(client, "newhost.example.com", key)

    assert client.get_host_keys().lookup("newhost.example.com") is not None
    expected = f"newhost.example.com {key.get_name()} {key.get_base64()}\n"
    assert known_hosts.read_text() == expected
    # 0o600/0o700 modulo umask: never group/other accessible.
    assert known_hosts.stat().st_mode & 0o077 == 0
    assert known_hosts.parent.stat().st_mode & 0o077 == 0


def test_accept_new_appends_without_rewriting_existing_content(tmp_path):
    """Persisting must append, byte-preserving prior file content.

    ``SSHClient.save_host_keys`` would truncate-rewrite the file and
    drop the comment and blank line below; the append path keeps them.
    """
    known_hosts = tmp_path / "known_hosts"
    existing = (
        "# trusted fleet hosts — hand-maintained comment\n"
        "\n"
        "old.example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakeFakeFake\n"
    )
    known_hosts.write_text(existing)
    client = paramiko.SSHClient()
    key = paramiko.ECDSAKey.generate()

    policy = AcceptNewPolicy(known_hosts_path=str(known_hosts))
    policy.missing_host_key(client, "new.example.com", key)

    new_line = f"new.example.com {key.get_name()} {key.get_base64()}\n"
    assert known_hosts.read_text() == existing + new_line


def test_accept_new_persist_failure_warns_and_still_accepts(tmp_path, caplog):
    """An OSError during persist is a visible soft failure.

    The warning names the path and the error, and the key stays trusted
    in-memory so the connection is not aborted.
    """
    blocker = tmp_path / "blocker"
    blocker.write_text("")  # regular file where a directory is needed
    known_hosts = blocker / ".ssh" / "known_hosts"
    client = paramiko.SSHClient()
    key = paramiko.ECDSAKey.generate()

    policy = AcceptNewPolicy(known_hosts_path=str(known_hosts))
    with caplog.at_level(logging.WARNING, logger="repose.connection_policy"):
        # Must not raise: persist failure must not kill the session.
        policy.missing_host_key(client, "h.example.com", key)

    assert client.get_host_keys().lookup("h.example.com") is not None
    assert "could not persist host key" in caplog.text
    assert str(known_hosts) in caplog.text


def test_run_timeout_non_tty_raises_without_prompt(mock_ssh_client, monkeypatch):
    """A timed-out command on non-interactive stdin yields CommandTimeout.

    Under multi-host fan-out every worker passes ``lock=None`` and shares
    stdin; calling ``input()`` there interleaves prompts and (on non-TTY
    stdin) raises EOFError. The non-TTY guard must instead raise
    CommandTimeout without ever touching ``input()``.
    """
    conn = Connection("h", "u", 22, timeout=0)
    conn.connect()

    # Force select() to report a timeout on every poll.
    monkeypatch.setattr("repose.connection.select.select", lambda *a, **k: ([], [], []))
    # Non-interactive stdin: nobody can answer the wait/cancel prompt.
    monkeypatch.setattr("sys.stdin.isatty", lambda: False)

    # input() must never be reached; fail loudly if it is.
    def _forbidden(*args, **kwargs):
        raise AssertionError("input() called under non-TTY stdin")

    monkeypatch.setattr("builtins.input", _forbidden)

    with pytest.raises(CommandTimeout):
        conn.run("sleep 100")


def test_run_timeout_tty_prompt_holds_module_lock(mock_ssh_client, monkeypatch):
    """The interactive timeout prompt runs entirely under _PROMPT_LOCK.

    With a TTY, timed-out fan-out workers used to call ``input()``
    concurrently (no call site passes ``lock``), interleaving prompts on
    shared stdin. The prompt/answer exchange must own the module-level
    prompt lock so at most one thread prompts at a time.
    """
    conn = Connection("h", "u", 22, timeout=0)
    conn.connect()

    # Force select() to report a timeout on every poll.
    monkeypatch.setattr("repose.connection.select.select", lambda *a, **k: ([], [], []))
    # Interactive stdin: the wait/cancel prompt is reachable.
    monkeypatch.setattr("sys.stdin.isatty", lambda: True)

    prompts = []

    def _fake_input(prompt):
        # The entire exchange must be serialized under the module lock.
        assert repose.connection._PROMPT_LOCK.locked()
        prompts.append(prompt)
        return "abort"

    monkeypatch.setattr("builtins.input", _fake_input)

    with pytest.raises(CommandTimeout):
        conn.run("sleep 100")

    assert len(prompts) == 1
    assert 'command "sleep 100" timed out on h' in prompts[0]
    # The lock is released again once the exchange is over.
    assert not repose.connection._PROMPT_LOCK.locked()


def test_run_timeout_tty_prompts_do_not_interleave(mock_ssh_client, monkeypatch):
    """Two concurrently timed-out workers get strictly sequential prompts.

    Worker A is parked inside its prompt on an event gate while worker B
    is already running; B's prompt must not start until A's whole
    prompt/answer exchange has finished.
    """
    monkeypatch.setattr("repose.connection.select.select", lambda *a, **k: ([], [], []))
    monkeypatch.setattr("sys.stdin.isatty", lambda: True)

    events = []
    a_in_prompt = threading.Event()
    release_a = threading.Event()

    def _fake_input(prompt):
        assert repose.connection._PROMPT_LOCK.locked()
        name = threading.current_thread().name
        events.append(f"{name}-enter")
        if name == "worker-a":
            a_in_prompt.set()
            # Hold the prompt open until the main thread releases it.
            assert release_a.wait(5)
        events.append(f"{name}-exit")
        return "n"

    monkeypatch.setattr("builtins.input", _fake_input)

    results = {}

    def _worker():
        conn = Connection("h", "u", 22, timeout=0)
        conn.connect()
        try:
            conn.run("sleep 100")
        except CommandTimeout:
            results[threading.current_thread().name] = "timeout"

    thread_a = threading.Thread(target=_worker, name="worker-a")
    thread_b = threading.Thread(target=_worker, name="worker-b")

    thread_a.start()
    assert a_in_prompt.wait(5)  # A owns the prompt and is parked inside it.
    thread_b.start()  # B times out too and must queue behind A.
    release_a.set()

    thread_a.join(5)
    thread_b.join(5)
    assert not thread_a.is_alive() and not thread_b.is_alive()

    # Each worker got its own complete, non-interleaved exchange, and B's
    # prompt only started after A's finished.
    assert events == [
        "worker-a-enter",
        "worker-a-exit",
        "worker-b-enter",
        "worker-b-exit",
    ]
    assert results == {"worker-a": "timeout", "worker-b": "timeout"}


# ---------------------------------------------------------------------------
# run() — output decoding
# ---------------------------------------------------------------------------


def test_run_tolerates_non_utf8_output(mock_ssh_client, monkeypatch):
    """Non-UTF-8 bytes in command output are replaced, not fatal.

    Regression test: the final decode used strict UTF-8, so a single
    non-UTF-8 byte from the remote command raised UnicodeDecodeError
    out of run(), killing the host worker mid-run.
    """
    session = mock_ssh_client.get_transport.return_value.open_session.return_value
    # First loop iteration delivers the payloads, second sees EOF.
    session.recv_ready.side_effect = [True, False]
    session.recv.return_value = b"ok\xff\xfe"
    session.recv_stderr_ready.side_effect = [True, False]
    session.recv_stderr.return_value = b"err\xfd"
    session.recv_exit_status.return_value = 0
    # Pretend data is always ready so the timeout prompt never triggers.
    monkeypatch.setattr(
        "repose.connection.select.select", lambda *a, **k: ([session], [], [])
    )

    conn = Connection("h", "u", 22)
    stdout, stderr, exitcode = conn.run("zypper lr")

    assert (stdout, stderr, exitcode) == ("ok\ufffd\ufffd", "err\ufffd", 0)


# ---------------------------------------------------------------------------
# Transactional reboot support: fire_and_forget / boot_id / wait_reconnect
# ---------------------------------------------------------------------------


def test_fire_and_forget_dispatches_then_closes(mock_ssh_client):
    """The command is exec'd on a fresh session and the link is closed."""
    conn = Connection("h", "u", 22)
    conn.connect()

    conn.fire_and_forget("systemctl reboot")

    session = mock_ssh_client.get_transport.return_value.open_session.return_value
    session.exec_command.assert_called_once_with("systemctl reboot")
    mock_ssh_client.close.assert_called()


def test_fire_and_forget_raises_when_session_never_opens(mock_ssh_client):
    """A failed session open must surface instead of dropping the command.

    Regression test for the silent-drop defect: with the session-open
    failure swallowed, the reboot is never dispatched, yet the caller
    proceeds to ``wait_reconnect`` which instantly "succeeds" against a
    host that never went down.
    """
    conn = Connection("h", "u", 22)
    conn.connect()
    transport = mock_ssh_client.get_transport.return_value
    transport.open_session.side_effect = paramiko.SSHException("no session")

    with pytest.raises(paramiko.SSHException, match="never dispatched"):
        conn.fire_and_forget("systemctl reboot")

    # The link is still torn down; the command was never exec'd.
    mock_ssh_client.close.assert_called()


def test_boot_id_returns_stripped_output(mock_ssh_client, monkeypatch):
    conn = Connection("h", "u", 22)
    monkeypatch.setattr(conn, "run", lambda *a, **k: ("abc-123\n", "", 0))
    assert conn.boot_id() == "abc-123"


def test_boot_id_empty_on_error(mock_ssh_client, monkeypatch):
    conn = Connection("h", "u", 22)

    def _boom(*a, **k):
        raise RuntimeError("down")

    monkeypatch.setattr(conn, "run", _boom)
    assert conn.boot_id() == ""


def test_wait_reconnect_succeeds_after_retries(mock_ssh_client):
    """Reconnect keeps retrying while the host is down, then succeeds."""
    conn = Connection("h", "u", 22)
    # is_active(): down, down, then up (with a spare for the final check).
    mock_ssh_client._transport.is_active.side_effect = [False, False, True, True]

    ok = conn.wait_reconnect(retry=5, timeout=0, backoff=False)

    assert ok is True
    assert mock_ssh_client.connect.call_count == 2


def test_wait_reconnect_gives_up_after_retry_budget(mock_ssh_client):
    """A host that never comes back gets exactly ``retry`` attempts."""
    conn = Connection("h", "u", 22)
    mock_ssh_client._transport.is_active.return_value = False

    ok = conn.wait_reconnect(retry=2, timeout=0, backoff=False)

    assert ok is False
    assert mock_ssh_client.connect.call_count == 2


# ---------------------------------------------------------------------------
# Bounded reconnect loops: a transport that stays active but keeps
# refusing new sessions/channels must fail loudly after a capped number
# of attempts instead of spinning forever (concurrency hazard).
# ---------------------------------------------------------------------------


@pytest.fixture
def _no_backoff(monkeypatch):
    """Neutralise the reconnect backoff so the bounded loops run fast.

    The stub reports the channel as readable so that, when a session does
    open, ``run``'s read loop skips its interactive timeout prompt.
    """
    monkeypatch.setattr(
        repose.connection.select, "select", lambda *a, **k: (["ready"], [], [])
    )


def _active_transport(mock_ssh_client):
    """Wire the mock so ``is_active()`` reports a live transport."""
    transport = MagicMock()
    transport.is_active.return_value = True
    mock_ssh_client._transport = transport
    mock_ssh_client.get_transport.return_value = transport
    return transport


def test_run_raises_after_attempt_cap_on_active_transport(mock_ssh_client, _no_backoff):
    """``run`` must not spin forever when the transport stays active but
    ``open_session`` keeps failing; it raises after the attempt cap."""
    transport = _active_transport(mock_ssh_client)

    calls = {"n": 0}

    def _open_session(*args, **kwargs):
        calls["n"] += 1
        # Guard against the pre-fix unbounded loop turning this test
        # into a hang: surface a distinct failure well past the cap.
        if calls["n"] > _RECONNECT_MAX_ATTEMPTS + 50:
            raise AssertionError("run() looped past the attempt cap")
        raise paramiko.SSHException("no session")

    transport.open_session.side_effect = _open_session

    conn = Connection("h", "u", 22)
    with pytest.raises(paramiko.SSHException):
        conn.run("true")

    # One initial attempt plus one per bounded retry, then it raises.
    assert calls["n"] == _RECONNECT_MAX_ATTEMPTS + 1


def test_sftp_reconnect_raises_after_attempt_cap_on_active_transport(
    mock_ssh_client, _no_backoff
):
    """``__sftp_reconnect`` (via ``listdir``) must also bound its retries
    when the SFTP channel open keeps failing on an active transport."""
    _active_transport(mock_ssh_client)

    calls = {"n": 0}

    def _open_sftp(*args, **kwargs):
        calls["n"] += 1
        if calls["n"] > _RECONNECT_MAX_ATTEMPTS + 50:
            raise AssertionError("__sftp_reconnect looped past the cap")
        raise paramiko.SSHException("no channel")

    mock_ssh_client.open_sftp.side_effect = _open_sftp

    conn = Connection("h", "u", 22)
    with pytest.raises(paramiko.SSHException):
        conn.listdir("/etc")

    assert calls["n"] == _RECONNECT_MAX_ATTEMPTS + 1


def test_run_recovers_after_forced_transport_teardown(mock_ssh_client, _no_backoff):
    """A degraded-but-active transport is recycled at the force threshold:
    ``close()`` tears it down, ``connect()`` brings up a fresh transport,
    and ``run`` then completes normally."""
    broken = _active_transport(mock_ssh_client)
    broken.open_session.side_effect = paramiko.SSHException("no session")

    good_session = MagicMock()
    good_session.recv_ready.return_value = False
    good_session.recv_stderr_ready.return_value = False
    good_session.recv_exit_status.return_value = 0

    def _teardown(*args, **kwargs):
        # A real close() leaves no live transport behind.
        mock_ssh_client._transport = None

    def _fresh_transport(*args, **kwargs):
        # A real connect() installs a new, working transport.
        fresh = MagicMock()
        fresh.is_active.return_value = True
        fresh.open_session.return_value = good_session
        mock_ssh_client._transport = fresh
        mock_ssh_client.get_transport.return_value = fresh

    mock_ssh_client.close.side_effect = _teardown
    mock_ssh_client.connect.side_effect = _fresh_transport

    conn = Connection("h", "u", 22)
    assert conn.run("true") == ("", "", 0)

    # The broken transport was retried up to the force threshold, then
    # recycled: close() tore it down and connect() replaced it.
    assert broken.open_session.call_count == _RECONNECT_FORCE_AFTER
    assert mock_ssh_client.close.called
    assert mock_ssh_client.connect.called


def test_run_reconnect_failures_consume_budget_then_raise(mock_ssh_client, _no_backoff):
    """When reconnect() keeps failing after the forced teardown, the loop
    must consume the remaining attempt budget and raise the cap-exhausted
    ``SSHException`` — not leak the raw connect error."""
    broken = _active_transport(mock_ssh_client)
    broken.open_session.side_effect = paramiko.SSHException("no session")

    def _teardown(*args, **kwargs):
        mock_ssh_client._transport = None

    mock_ssh_client.close.side_effect = _teardown
    mock_ssh_client.connect.side_effect = RuntimeError("network down")

    conn = Connection("h", "u", 22)
    with pytest.raises(paramiko.SSHException, match="Unable to open a session"):
        conn.run("true")

    # Every attempt from the forced teardown onwards tries to reconnect.
    expected_connects = _RECONNECT_MAX_ATTEMPTS - _RECONNECT_FORCE_AFTER + 1
    assert mock_ssh_client.connect.call_count == expected_connects


# ---------------------------------------------------------------------------
# Command-timeout handling: aborting a timed-out command must not leak the
# channel on the shared transport.
# ---------------------------------------------------------------------------


def test_run_closes_session_when_timeout_aborted(mock_ssh_client, monkeypatch):
    """Declining the wait prompt on a command timeout still closes the channel.

    ``run`` raises ``CommandTimeout`` when the user declines to keep waiting.
    The channel must be torn down on that path, otherwise sessions accumulate
    on the shared transport until ``MaxSessions`` is hit.
    """
    session = mock_ssh_client.get_transport.return_value.open_session.return_value

    # Force select() to report a timeout (no data ready), triggering the
    # interactive wait prompt.
    monkeypatch.setattr("repose.connection.select.select", lambda *a, **k: ([], [], []))
    # The user declines to keep waiting → run() raises CommandTimeout.
    monkeypatch.setattr("builtins.input", lambda *a, **k: "n")

    conn = Connection("h", "u", 22)
    conn.connect()

    with pytest.raises(CommandTimeout):
        conn.run("sleep 999")

    # close_session() shuts down and closes the channel on the abort path.
    session.shutdown.assert_called_once_with(2)
    session.close.assert_called_once()
