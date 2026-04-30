from argparse import Namespace
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


# ---------------------------------------------------------------------------
# Shared factories used across the test-suite.
# ---------------------------------------------------------------------------


@pytest.fixture
def make_args():
    """Factory that builds an argparse-style ``Namespace`` with sane defaults.

    Tests can override any field via keyword arguments, e.g.::

        args = make_args(dry=True, repa=[Repa("SLES:15-SP3:x86_64:")])
    """

    def _factory(**overrides) -> Namespace:
        defaults = {
            "dry": False,
            "target": [{"user@host1": MagicMock()}],
            "repa": [],
            "config": "dummy_config",
            "yaml": False,
        }
        defaults.update(overrides)
        return Namespace(**defaults)

    return _factory


@pytest.fixture
def mock_target_factory():
    """Factory producing a ``Target``-shaped ``MagicMock``."""

    def _factory(base="dummy_base", **kwargs):
        target = MagicMock()
        target.products.get_base.return_value = base
        for k, v in kwargs.items():
            setattr(target, k, v)
        return target

    return _factory


@pytest.fixture
def mock_host_group_factory(mock_target_factory):
    """Factory producing a ``HostGroup``-shaped ``MagicMock``."""

    def _factory(hosts=None, target=None):
        if hosts is None:
            hosts = ["user@host1"]
        if target is None:
            target = mock_target_factory()
        hg = MagicMock()
        hg.keys.return_value = hosts
        hg.__getitem__.return_value = target
        hg.__iter__ = lambda self: iter(hosts)
        return hg, target

    return _factory


@pytest.fixture
def patch_command_executor(monkeypatch):
    """Swap ``ThreadPoolExecutor`` → ``ImmediateExecutor`` and patch
    ``HostGroup`` so command-level tests run synchronously.

    Returns the (mocked) ``HostGroup`` factory for further assertions.
    """
    import concurrent.futures

    import repose.command._command

    monkeypatch.setattr(concurrent.futures, "ThreadPoolExecutor", ImmediateExecutor)

    def _apply(host_group_instance):
        hg_class = MagicMock(return_value=host_group_instance)
        monkeypatch.setattr(repose.command._command, "HostGroup", hg_class)
        return hg_class

    return _apply


@pytest.fixture
def sample_repos_xml():
    """Small ``zypper -x lr`` style XML fixture for parse_repositories."""
    return (
        '<?xml version="1.0"?>'
        "<stream>"
        "<repo-list>"
        '<repo alias="repo-one" name="Repo One" enabled="1" '
        'autorefresh="0" gpgcheck="1">'
        "<url>http://example.com/one/</url>"
        "</repo>"
        '<repo alias="repo-two" name="Repo Two" enabled="0" '
        'autorefresh="0" gpgcheck="1">'
        "<url>http://example.com/two/</url>"
        "</repo>"
        "</repo-list>"
        "</stream>"
    )
