from argparse import ArgumentTypeError
from urllib.parse import urlparse

from .target import Target
from .types.connection_config import ConnectionConfig


class HostParseError(ValueError, ArgumentTypeError):
    # Note: need to inherit ArgumentTypeError so the custom exception
    # messages get shown to the users properly
    # by L{argparse.ArgumentParser._get_value}

    def __init__(self, message: str):
        super().__init__("Target host: " + message)


class PortNotIntError(HostParseError):
    def __init__(self, hostname: str) -> None:
        super().__init__(f"Wrong port specification on Host: {hostname}")


def _parse_host_string(arg: str, config: ConnectionConfig) -> dict[str, Target]:
    """Parse a ``[user@]host[:port]`` string into a ``{key: Target}`` dict."""
    x = urlparse(f"//{arg}")
    hostname = x.hostname or ""
    try:
        if x.port:
            keyname = f"{hostname}:{x.port}"
            port = x.port
        else:
            keyname = hostname
            port = 22

        username = x.username if x.username else "root"

        return {keyname: Target(hostname, port, username, config=config)}
    except ValueError:
        raise PortNotIntError(hostname)


class ParseHosts(dict):
    """Argparse ``type=`` adapter for ``-t HOST`` arguments.

    Two construction modes:

    - ``ParseHosts("user@host:port")`` — historical one-shot mode. Parses
      immediately with default ``ConnectionConfig``. Kept for tests and
      ad-hoc programmatic use.
    - ``ParseHosts(ConnectionConfig(...))`` — factory mode. The returned
      instance is *callable* with a single string argument and yields a
      new ``ParseHosts`` (a plain ``dict`` subclass) populated with one
      ``Target`` built with the captured config. This is the form wired
      into argparse via ``type=ParseHosts(cfg)``.
    """

    def __init__(self, arg: "str | ConnectionConfig") -> None:
        if isinstance(arg, ConnectionConfig):
            # Factory mode: defer parsing until ``__call__``.
            self._config: ConnectionConfig = arg
            super().__init__()
            return
        # One-shot mode: parse with default config.
        self._config = ConnectionConfig()
        super().__init__(_parse_host_string(arg, self._config))

    def __call__(self, arg: str) -> "ParseHosts":
        # Build a fresh ParseHosts seeded with parsed targets; reuse the
        # captured config. We can't reuse ``self`` because argparse
        # appends each ``-t`` result to a list and may invoke the type
        # callable many times in one parse run.
        instance = ParseHosts.__new__(ParseHosts)
        instance._config = self._config
        dict.__init__(instance, _parse_host_string(arg, self._config))
        return instance
