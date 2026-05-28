"""Custom ``paramiko.MissingHostKeyPolicy`` implementations.

paramiko ships ``AutoAddPolicy`` (add anything, trust anything) and
``RejectPolicy`` (refuse anything not in known_hosts), but no native
equivalent of OpenSSH's ``StrictHostKeyChecking=accept-new`` (add
*unknown* hosts on first contact, but refuse a host whose key has
*changed* since it was first recorded).

paramiko's transport already raises ``BadHostKeyException`` for keys
that exist in ``client.get_host_keys()`` but mismatch the server's
presented key — that check fires *before* the missing-host-key policy
is consulted. So a true accept-new implementation only needs to handle
the *missing* (i.e. genuinely unknown) case: add the key in-memory and,
when a writable ``known_hosts`` path is available, persist it.
"""

import logging
import os
from typing import Any


import paramiko


logger = logging.getLogger("repose.connection_policy")


class AcceptNewPolicy(paramiko.MissingHostKeyPolicy):
    """Add unknown host keys; rely on paramiko to reject changed ones.

    Matches OpenSSH's ``StrictHostKeyChecking=accept-new``. Optionally
    persists newly-accepted keys to ``known_hosts_path`` so subsequent
    runs can detect a changed key.
    """

    def __init__(self, known_hosts_path: str | None = None) -> None:
        super().__init__()
        # Expanded eagerly so the same path string ("~/.ssh/known_hosts")
        # works whether the policy is constructed before or after the
        # process changes its working directory.
        self.known_hosts_path: str | None = (
            os.path.expanduser(known_hosts_path) if known_hosts_path else None
        )

    def missing_host_key(
        self, client: paramiko.SSHClient, hostname: str, key: Any
    ) -> None:
        logger.info(
            "accept-new: recording unknown host key for %s (%s)",
            hostname,
            key.get_name(),
        )
        client.get_host_keys().add(hostname, key.get_name(), key)
        if self.known_hosts_path is None:
            return
        try:
            client.save_host_keys(self.known_hosts_path)
        except OSError as exc:
            # Read-only known_hosts (e.g. system-wide file, or a path
            # the user can't write to) is a soft failure: the key is
            # still in-memory for this run, but won't survive a restart.
            logger.warning(
                "accept-new: could not persist host key for %s to %s: %s",
                hostname,
                self.known_hosts_path,
                exc,
            )
