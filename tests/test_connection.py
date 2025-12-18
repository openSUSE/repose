import paramiko

from repose.connection import Connection


def test_connection_with_mock(mock_ssh_client, monkeypatch):
    monkeypatch.setattr(paramiko.config.SSHConfig, "parse", lambda self, fd: None)
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
