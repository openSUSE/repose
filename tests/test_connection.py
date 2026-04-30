"""Tests for ``repose.connection``."""

from unittest.mock import MagicMock

import paramiko
import pytest

from repose.connection import CommandTimeout, Connection


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


def test_readlink_uses_sftp(mock_ssh_client):
    sftp = MagicMock()
    sftp.readlink.return_value = "/target"
    mock_ssh_client.open_sftp.return_value = sftp

    conn = Connection("h", "u", 22)
    assert conn.readlink("/link") == "/target"
    sftp.readlink.assert_called_once_with("/link")


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
