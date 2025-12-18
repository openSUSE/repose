from concurrent.futures import Future
from unittest.mock import MagicMock

import paramiko
import pytest


@pytest.fixture
def mock_ssh_client(monkeypatch):
    mock_ssh_class = MagicMock()
    mock_ssh_instance = MagicMock()
    mock_ssh_class.return_value = mock_ssh_instance
    monkeypatch.setattr(paramiko, "SSHClient", mock_ssh_class)
    return mock_ssh_instance


# This executor runs tasks sequentially and returns a completed Future.
class ImmediateExecutor:
    __name__ = "ImmediateExecutor"

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        pass

    def submit(self, fn, *args, **kwargs):
        future = Future()
        try:
            result = fn(*args, **kwargs)
            future.set_result(result)
        except Exception as e:
            future.set_exception(e)
        return future

    @staticmethod
    def wait(futures):
        pass
