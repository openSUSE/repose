from argparse import ArgumentTypeError
from urllib.parse import urlparse

from .target import Target


class HostParseError(ValueError, ArgumentTypeError):
    # Note: need to inherit ArgumentTypeError so the custom exception
    # messages get shown to the users properly
    # by L{argparse.ArgumentParser._get_value}

    def __init__(self, message: str):
        super().__init__("Target host: " + message)


class PortNotIntError(HostParseError):
    def __init__(self, hostname: str) -> None:
        super().__init__(f"Wrong port specification on Host: {hostname}")


class ParseHosts(dict):
    def __init__(self, arg: str) -> None:
        """
        arg is string with hosts in socket format username@host:port
        """
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

            host = [(keyname, Target(hostname, port, username))]
        except ValueError:
            raise PortNotIntError(hostname)
        super().__init__(host)
