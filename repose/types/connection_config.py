"""Configuration carrier for ``Connection`` host-key + timeout knobs.

A single immutable record threaded from CLI argument parsing through
``ParseHosts`` → ``Target`` → ``Connection``. Keeping the policy in one
place avoids growing the ``Connection.__init__`` signature every time a
new transport flag lands.
"""

from dataclasses import dataclass
from pathlib import Path
from typing import Literal


HostKeyPolicy = Literal["yes", "accept-new", "no", "off"]
SSHBackend = Literal["asyncssh", "paramiko"]


@dataclass(frozen=True, slots=True)
class ConnectionConfig:
    """Immutable transport configuration shared across all ``Target``s.

    Attributes:
        host_key_policy: OpenSSH-style ``StrictHostKeyChecking`` mode.

            - ``"yes"`` — refuse unknown hosts (``RejectPolicy``).
            - ``"accept-new"`` — add unknown, refuse changed (default).
            - ``"no"`` / ``"off"`` — add unknown, tolerate changed
              (matches pre-PR-12 behaviour; use only on trusted nets).
        known_hosts: Optional path to a custom ``known_hosts`` file. When
            ``None`` the user's system known_hosts (``~/.ssh/known_hosts``
            via ``paramiko.SSHClient.load_system_host_keys``) is used.
        timeout: Per-command SSH read timeout in seconds.
        ssh_backend: Which SSH library to drive.

            - ``"asyncssh"`` (default) — native async backend; scales
              to hundreds of hosts without threadpool pressure.
            - ``"paramiko"`` — legacy threaded backend; available as
              a one-release safety net while ``asyncssh`` settles in
              real-world deployments. To be removed.
    """

    host_key_policy: HostKeyPolicy = "accept-new"
    known_hosts: Path | None = None
    timeout: float = 120.0
    ssh_backend: SSHBackend = "asyncssh"
