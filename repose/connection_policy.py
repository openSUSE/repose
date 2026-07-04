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
when a ``known_hosts`` path is available, persist it.

Persistence deliberately avoids ``SSHClient.save_host_keys``: it
truncate-rewrites the whole file, silently dropping comments, blank
lines and any entries paramiko cannot round-trip — unacceptable for the
user's real ``~/.ssh/known_hosts``. Instead a single well-formed line is
*appended*, exactly like OpenSSH itself records first contacts.
"""

import logging
import os
import threading

import paramiko


logger = logging.getLogger("repose.connection_policy")

# ``HostGroup`` fans connections out over a thread pool, so several
# first contacts may try to persist to the same known_hosts file at
# once. One process-wide lock serializes the appends so concurrent
# writes cannot interleave or drop lines.
_known_hosts_lock = threading.Lock()


class AcceptNewPolicy(paramiko.MissingHostKeyPolicy):
    """Add unknown host keys; rely on paramiko to reject changed ones.

    Matches OpenSSH's ``StrictHostKeyChecking=accept-new``. A newly
    accepted key is recorded in the client's in-memory host-key store
    (trusting it for the current process) and, when ``known_hosts_path``
    is set, persisted by appending one ``known_hosts`` line so
    subsequent runs can detect a changed key. Existing file content is
    never rewritten; the file (and its parent directory) is created on
    first contact if missing.
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
        self, client: paramiko.SSHClient, hostname: str, key: paramiko.PKey
    ) -> None:
        """Trust ``key`` for this run and persist it to known_hosts.

        Args:
            client: The connecting ``SSHClient`` (its in-memory host-key
                store receives the new entry).
            hostname: Hostname as presented by paramiko (already in
                ``[host]:port`` form for non-standard ports).
            key: The server's host key.

        A persistence failure must not kill the session: the connection
        proceeds on the in-memory entry alone and a warning names the
        path and the error.
        """
        logger.info(
            "accept-new: recording unknown host key for %s (%s)",
            hostname,
            key.get_name(),
        )
        client.get_host_keys().add(hostname, key.get_name(), key)
        if self.known_hosts_path is None:
            return
        try:
            _append_known_hosts_line(self.known_hosts_path, hostname, key)
        except OSError as exc:
            # Read-only known_hosts (e.g. system-wide file, or a path
            # the user can't write to) is a soft failure: the key is
            # still trusted in-memory for this run, but won't survive a
            # restart.
            logger.warning(
                "accept-new: could not persist host key for %s to %s: %s",
                hostname,
                self.known_hosts_path,
                exc,
            )


def _append_known_hosts_line(path: str, hostname: str, key: paramiko.PKey) -> None:
    """Append one ``known_hosts`` entry for ``hostname`` to ``path``.

    Creates the parent directory (mode ``0o700``) and the file (mode
    ``0o600``) if missing — matching OpenSSH conventions — and never
    truncates or rewrites existing content. All appends in the process
    share one lock, so concurrent first contacts cannot corrupt the
    file.

    Raises:
        OSError: If the directory or file cannot be created or written.
    """
    line = f"{hostname} {key.get_name()} {key.get_base64()}\n"
    with _known_hosts_lock:
        parent = os.path.dirname(path)
        if parent:
            os.makedirs(parent, mode=0o700, exist_ok=True)
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
        with os.fdopen(fd, "w") as fobj:
            fobj.write(line)
