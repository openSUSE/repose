"""Async SSH backend on top of :mod:`asyncssh`.

Mirrors the public surface of :class:`repose.connection.Connection`
(``connect``, ``run``, ``listdir``, ``open``, ``readlink``, ``close``,
``is_active``) so :class:`repose.target.async_target.AsyncTarget` and
the rest of the codebase can swap backends behind a single CLI flag.

The intent is byte-for-byte behavioural parity with the paramiko path,
not a re-imagining: same exit-code surface, same exception types
(``CommandTimeout`` is re-exported here so callers ``except`` it
regardless of backend), same host-key policy semantics
(``yes``/``accept-new``/``no``/``off``).

Everything I/O bound is ``async``; CPU work (parsing, formatting)
stays sync and lives in the parsers as it did before.
"""

from __future__ import annotations

import asyncio
import errno
import getpass
import logging
import os
import socket
from pathlib import Path
from typing import Any

import asyncssh

from .connection import CommandTimeout  # re-export for backend-neutral except clauses
from .types.connection_config import ConnectionConfig


__all__ = ["AsyncConnection", "CommandTimeout"]


logger = logging.getLogger("repose.aiossh")


def _translate_sftp_error(exc: asyncssh.SFTPError) -> OSError:
    """Map an asyncssh ``SFTPError`` onto the matching ``OSError`` subclass.

    asyncssh's SFTP exceptions derive from ``asyncssh.Error`` — *not*
    from ``OSError`` — so backend-neutral callers that ``except
    OSError``/``except FileNotFoundError`` (e.g. the shared
    :func:`repose.target.parsers.product.parse_system_async`) would
    otherwise never catch a missing ``/etc/products.d`` or
    ``/etc/os-release``. paramiko's SFTP client raises ``OSError``
    subclasses for the same conditions, so translating here keeps the
    two backends behaviourally identical.
    """
    if isinstance(exc, asyncssh.SFTPNoSuchFile):
        return FileNotFoundError(str(exc))
    if isinstance(exc, asyncssh.SFTPPermissionDenied):
        return PermissionError(str(exc))
    return OSError(str(exc))


def _parse_openssh_config(hostname: str) -> dict[str, Any]:
    """Look up ``hostname`` in the user's ``~/.ssh/config``.

    Returns the same dict-of-lowercased-keys shape that paramiko's
    ``SSHConfig.lookup`` returns, so the call sites that consult
    ``opts.get("hostname", ...)`` / ``opts.get("port", ...)`` /
    ``opts.get("user", ...)`` / ``opts.get("identityfile", ...)`` /
    ``opts.get("proxycommand", ...)`` keep working unchanged.

    Missing config file → empty dict (no overrides). Permission errors
    or malformed entries are logged at WARNING and treated as empty.
    """
    cfg_path = os.path.expanduser("~/.ssh/config")
    try:
        # Local import: paramiko stays a runtime dep during the
        # dual-backend window. Reusing its parser keeps the asyncssh
        # path 100% behaviour-compatible with the paramiko path for
        # the long tail of ``~/.ssh/config`` directives.
        import paramiko

        cfg = paramiko.config.SSHConfig()
        with open(cfg_path) as fd:
            cfg.parse(fd)
        return cfg.lookup(hostname)
    except OSError as exc:
        if exc.errno != errno.ENOENT:
            logger.warning("could not read %s: %s", cfg_path, exc)
        return {}
    except Exception as exc:
        # Defensive: a malformed ssh_config should not take down the
        # entire run. Match paramiko-side behaviour where a parse
        # error logs and continues.
        logger.warning("could not parse %s: %s", cfg_path, exc)
        return {}


def _default_known_hosts_path() -> Path:
    return Path(os.path.expanduser("~/.ssh/known_hosts"))


def _build_known_hosts_arg(policy: str, known_hosts: Path | None) -> Any:
    """Translate ConnectionConfig host-key knobs to asyncssh's
    ``known_hosts`` argument.

    - ``yes``        → the configured file (or asyncssh's default
      ``~/.ssh/known_hosts``); asyncssh refuses unknown hosts and
      mismatches natively.
    - ``accept-new`` → the configured file (or ``~/.ssh/known_hosts``)
      when it exists, so asyncssh does the native matching, host-key
      algorithm negotiation, revocation and certificate validation; the
      custom ``_AcceptNewClient`` (see :func:`_make_accept_new_client`)
      only decides first-contact vs changed-key. An unparseable line
      makes asyncssh reject the whole file -- unlike OpenSSH we do not
      skip bad lines. When the file does not exist an empty *but truthy*
      trust set keeps the validator engaged with every host as first
      contact.
    - ``no``/``off`` → ``None``: asyncssh disables host-key validation
      entirely (no ``client_factory`` is installed for these policies).
      Matches the historical pre-PR-12 behaviour.
    """
    if policy == "accept-new":
        resolved = known_hosts or _default_known_hosts_path()
        return str(resolved) if resolved.exists() else ([], [], [])
    if policy in ("no", "off"):
        return None
    if known_hosts is None:
        return ()  # asyncssh default: ``~/.ssh/known_hosts``
    return str(known_hosts)


_DEFAULT_SSH_PORT = 22


def _append_known_host(path: Path, host: str, port: int, key: bytes) -> None:
    """Append a first-contact host key, keeping ``path`` newline-safe.

    An existing known_hosts file whose final line lacks a trailing
    newline would otherwise have its last entry merged with the new one,
    corrupting both. Non-default ports are written in OpenSSH's
    ``[host]:port`` form so the entry matches on the next connect.
    """
    entry_host = host if port == _DEFAULT_SSH_PORT else f"[{host}]:{port}"
    needs_newline = False
    if path.exists() and path.stat().st_size > 0:
        with open(path, "rb") as fh:
            fh.seek(-1, os.SEEK_END)
            needs_newline = fh.read(1) != b"\n"
    with open(path, "a", encoding="utf-8") as fh:
        if needs_newline:
            fh.write("\n")
        fh.write(f"{entry_host} {key.decode().strip()}\n")


def _make_accept_new_client(
    known_hosts_path: Path, expected_host: str
) -> type[asyncssh.SSHClient]:
    """Return an ``SSHClient`` subclass enforcing ``accept-new`` semantics.

    asyncssh does the native known_hosts matching (see
    :func:`_build_known_hosts_arg`) and only calls
    :meth:`validate_host_public_key` for a key it does not already
    trust. This client then implements the accept-new decision:

    - if the key is revoked for this host, refuse it;
    - if the host has pins (under its resolved name or the configured
      alias) but the offered key matches none, refuse it as a changed
      key;
    - if the offered key matches a pin stored under the alias -- which
      asyncssh, matching only the resolved name, would have missed --
      accept it;
    - otherwise this is first contact: accept and persist the key.

    The known_hosts file is read once at factory time (via asyncssh's
    own parser) for that first-contact-vs-changed decision; asyncssh
    reads the same file for its native matching.

    Host *certificates* are accepted on first contact under the same
    trust-on-first-use rule (see :meth:`validate_host_ca_key`), but
    asyncssh still enforces the certificate's own validity (principals,
    expiry): an expired or wrong-principal host cert is refused. Pinning
    a certificate against a specific CA is out of scope.
    """
    known_hosts = (
        asyncssh.read_known_hosts(str(known_hosts_path))
        if known_hosts_path.exists()
        else None
    )
    persisted: set[bytes] = set()

    def _pins(host: str, addr: str, port: int) -> tuple[set[bytes], set[bytes]]:
        """Trusted and revoked key blobs for ``host`` and the alias.

        Known limitation: asyncssh's ``SSHKnownHosts.match`` drops the
        revoked list when a port-qualified lookup finds no trusted key,
        so a key revoked via a ``@revoked [host]:port`` entry is not
        detected (this also defeats asyncssh's own native ``yes``
        validation, not just accept-new). Plain ``@revoked host``
        entries are honoured.
        """
        trusted: set[bytes] = set()
        revoked: set[bytes] = set()
        if known_hosts is not None:
            # A pin may be stored under the resolved name or the
            # configured alias; consult both so an alias pin is honoured.
            for name in {host, expected_host}:
                matched = known_hosts.match(name, addr, port)
                trusted |= {k.public_data for k in matched[0]}
                revoked |= {k.public_data for k in matched[2]}
        return trusted, revoked

    class _AcceptNewClient(asyncssh.SSHClient):
        def validate_host_public_key(
            self,
            host: str,
            addr: str,
            port: int,
            key: asyncssh.SSHKey,
        ) -> bool:
            offered = key.public_data
            trusted, revoked = _pins(host, addr, port)
            if offered in revoked:
                # A key explicitly revoked for this host is never
                # accepted, even when no positive pin remains.
                logger.error(
                    "accept-new: host key for %s is revoked; refusing",
                    expected_host,
                )
                return False
            if offered in trusted:
                # Matches a pin -- possibly one stored under the
                # configured alias that asyncssh did not match on.
                return True
            if trusted:
                # Host is known but the offered key is not one of its
                # pins: the canonical changed-key scenario.
                logger.error(
                    "accept-new: host key for %s changed; refusing",
                    expected_host,
                )
                return False
            # First contact: accept and persist once. The password-auth
            # fallback re-runs this validator with the same factory, so
            # guard against writing a duplicate entry.
            blob = key.export_public_key()
            if blob not in persisted:
                try:
                    _append_known_host(known_hosts_path, host, port, blob)
                    persisted.add(blob)
                    logger.info(
                        "accept-new: persisted host key for %s to %s",
                        expected_host,
                        known_hosts_path,
                    )
                except OSError as werr:
                    logger.warning(
                        "accept-new: could not persist host key for %s to %s: %s",
                        expected_host,
                        known_hosts_path,
                        werr,
                    )
            return True

        def validate_host_ca_key(
            self,
            host: str,
            addr: str,
            port: int,
            key: asyncssh.SSHKey,
        ) -> bool:
            # accept-new is trust-on-first-use: accept a host certificate
            # the same way an unknown host key is accepted on first
            # contact. Passing a non-None (empty) known_hosts engages
            # asyncssh's CA check, whose default would reject every cert
            # host outright -- a connectivity regression versus leaving
            # host-key validation off. Certificate *pinning* / change
            # detection is out of scope (repose targets plain host-key
            # refhosts); this only restores reachability.
            logger.info(
                "accept-new: trusting host certificate for %s (first-contact)",
                expected_host,
            )
            return True

    return _AcceptNewClient


class AsyncConnection:
    """Async equivalent of :class:`repose.connection.Connection`.

    Public surface mirrors the paramiko backend exactly: all I/O
    methods are coroutines, but the same arguments and the same return
    shapes apply. ``close()`` is idempotent.
    """

    def __init__(
        self,
        hostname: str,
        username: str,
        port: str | int,
        timeout: float | None = None,
        *,
        config: ConnectionConfig | None = None,
    ) -> None:
        self.username = username
        self.hostname = hostname
        try:
            self.port = int(port)
        except Exception:
            self.port = 22

        self.config: ConnectionConfig = config or ConnectionConfig()
        # Explicit positional ``timeout`` wins over ``config.timeout``;
        # mirror the paramiko ``Connection`` semantics so the two
        # backends accept the exact same call shape.
        self.timeout: float = (
            float(timeout) if timeout is not None else self.config.timeout
        )

        self._conn: asyncssh.SSHClientConnection | None = None
        self._sftp: asyncssh.SFTPClient | None = None

    def __repr__(self) -> str:
        return (
            f"<{self.__class__.__name__} object username={self.username} "
            f"hostname={self.hostname} port={self.port}>"
        )

    # ------------------------------------------------------------------
    # connect / auth
    # ------------------------------------------------------------------

    async def _do_connect(
        self,
        *,
        known_hosts: Any,
        client_factory: type[asyncssh.SSHClient] | None = None,
        password: str | None = None,
    ) -> asyncssh.SSHClientConnection:
        """Single ``asyncssh.connect`` call honoring ``~/.ssh/config``.

        Centralised so the key-auth and password-auth attempts share
        the exact same option-resolution logic (host alias, port,
        username override, identityfile, proxycommand).
        """
        opts = _parse_openssh_config(self.hostname)

        # When a ProxyCommand is configured, paramiko explicitly passes
        # the *original* hostname (because the proxy resolves it on the
        # other side). Mirror that here.
        target_host: str = (
            opts.get("hostname", self.hostname)
            if "proxycommand" not in opts
            else self.hostname
        )
        target_port = int(opts.get("port", self.port))
        target_user = opts.get("user", self.username)
        client_keys = opts.get("identityfile") or ()
        tunnel = opts.get("proxycommand")

        kwargs: dict[str, Any] = {
            "host": target_host,
            "port": target_port,
            "username": target_user,
            "known_hosts": known_hosts,
        }
        if client_factory is not None:
            kwargs["client_factory"] = client_factory
        if password is not None:
            kwargs["password"] = password
            # Disable public-key auth when we already know we need the
            # password path; otherwise asyncssh may try the agent first
            # and fail before getting to the password.
            kwargs["client_keys"] = ()
        elif client_keys:
            kwargs["client_keys"] = client_keys
        if tunnel:
            # asyncssh supports proxy_command directly (no tunnel= dance
            # like the paramiko ProxyCommand class).
            kwargs["proxy_command"] = tunnel

        return await asyncssh.connect(**kwargs)

    async def connect(self) -> None:
        """Open the SSH session with key-auth → password-auth fallback.

        Mirrors :meth:`Connection.connect`: tries public key first,
        falls back to a single interactive password prompt on
        authentication failure, and re-raises everything else. The
        ``accept-new`` policy is implemented by an ``SSHClient``
        subclass (``_make_accept_new_client``) installed via
        ``client_factory=``.
        """
        policy = self.config.host_key_policy
        kh_arg = _build_known_hosts_arg(policy, self.config.known_hosts)
        client_factory: type[asyncssh.SSHClient] | None = None
        if policy == "accept-new":
            kh_path = self.config.known_hosts or _default_known_hosts_path()
            client_factory = _make_accept_new_client(kh_path, self.hostname)

        try:
            logger.debug("connecting to %s:%s", self.hostname, self.port)
            self._conn = await self._do_connect(
                known_hosts=kh_arg, client_factory=client_factory
            )
            return
        except asyncssh.HostKeyNotVerifiable:
            # ``yes`` reaches here for an unknown or mismatched key;
            # ``accept-new`` reaches here when its validator rejects a
            # *changed* key (``validate_host_public_key`` returned
            # False). ``no``/``off`` disable the native checker and never
            # reach here. Bubble up like paramiko's
            # ``BadHostKeyException``/``RejectPolicy``.
            logger.error(
                "host key verification failed for %s (policy=%s)",
                self.hostname,
                policy,
            )
            raise
        except asyncssh.PermissionDenied:
            logger.warning(
                "Authentication failed on %s: AuthKey missing.", self.hostname
            )
            logger.warning("Trying manually, please enter the root password")
        except asyncssh.DisconnectError as exc:
            # asyncssh raises DisconnectError for misc transport errors
            # (closed socket, banner mismatch, ...). Treat as the
            # paramiko ``SSHException`` equivalent: log + re-raise.
            logger.error("SSH error while connecting to %s: %s", self.hostname, exc)
            raise
        except (OSError, socket.gaierror) as exc:
            # DNS / connect-refused / network unreachable. Mirror
            # paramiko's "generic Exception" branch.
            logger.error("%s: %s", self.hostname, exc)
            raise

        # Password fallback path. Reached only via the
        # ``PermissionDenied`` branch above.
        password = await asyncio.to_thread(getpass.getpass)
        try:
            self._conn = await self._do_connect(
                known_hosts=kh_arg,
                client_factory=client_factory,
                password=password,
            )
        except asyncssh.PermissionDenied:
            logger.error("Authentication failed on %s: wrong password", self.hostname)
            raise

    # ------------------------------------------------------------------
    # exec / sftp
    # ------------------------------------------------------------------

    async def run(self, command: str, lock: Any = None) -> tuple[str, str, int]:
        """Execute ``command`` over the established session.

        Returns ``(stdout, stderr, exitcode)``. Raises ``CommandTimeout``
        when the per-command timeout fires, matching the paramiko
        backend's exception type so callers can ``except CommandTimeout``
        without caring which backend ran.

        ``lock`` is accepted for signature parity with
        :meth:`Connection.run` and ignored — asyncssh's read loop never
        prompts the user mid-command (the paramiko backend's
        "wait? (y/N)" UX is intentionally dropped; a hard timeout is
        clearer and scripts-friendly).
        """
        assert self._conn is not None, "connect() must run first"
        try:
            # errors="replace": asyncssh's default strict UTF-8 decode
            # raises ProtocolError (from UnicodeDecodeError) as soon as
            # a command emits a non-UTF-8 byte, killing the whole
            # connection mid-command. Replacing undecodable bytes with
            # U+FFFD matches the paramiko backend's tolerant decode.
            result = await self._conn.run(
                command, check=False, timeout=self.timeout, errors="replace"
            )
        except asyncio.TimeoutError as exc:
            # asyncssh raises asyncio.TimeoutError when its own timeout
            # fires; translate to the project-wide exception type.
            raise CommandTimeout(command) from exc

        # asyncssh decodes by default (encoding='utf-8'); stdout/stderr
        # are str. exit_status may be None for a session torn down
        # without a normal exit (mirror paramiko's ``-1`` sentinel).
        stdout = (
            result.stdout
            if isinstance(result.stdout, str)
            else (result.stdout.decode("utf-8", "ignore") if result.stdout else "")
        )
        stderr = (
            result.stderr
            if isinstance(result.stderr, str)
            else (result.stderr.decode("utf-8", "ignore") if result.stderr else "")
        )
        exitcode = result.exit_status if result.exit_status is not None else -1
        return stdout, stderr, exitcode

    async def _sftp_open(self) -> asyncssh.SFTPClient:
        """Open (or reuse) a single SFTP subsystem for this connection.

        Caching mirrors paramiko's ``__sftp_reconnect`` reuse pattern,
        avoiding the per-call subsystem-setup cost when a command does
        a flurry of ``listdir`` + multiple ``open`` (see
        :func:`parse_system`).
        """
        assert self._conn is not None, "connect() must run first"
        if self._sftp is None:
            self._sftp = await self._conn.start_sftp_client()
        return self._sftp

    async def listdir(self, path: str = ".") -> list[str]:
        logger.debug("getting %s:%s:%s listing", self.hostname, self.port, path)
        sftp = await self._sftp_open()
        # asyncssh returns ``Sequence[str|bytes]`` depending on the
        # input encoding; we always pass str, so we always get str.
        # Filter out ``.`` and ``..`` for paramiko parity (paramiko's
        # SFTPClient.listdir does so by default).
        try:
            entries = await sftp.listdir(path)
        except asyncssh.SFTPError as exc:
            raise _translate_sftp_error(exc) from exc
        return [e for e in entries if e not in (".", "..")]

    async def readlink(self, path: str) -> str | None:
        logger.debug("read link %s:%s:%s", self.hostname, self.port, path)
        sftp = await self._sftp_open()
        try:
            link = await sftp.readlink(path)
        except asyncssh.SFTPError as exc:
            raise _translate_sftp_error(exc) from exc
        return link if isinstance(link, str) or link is None else link.decode()

    def open(self, filename: str, mode: str = "r") -> "_AsyncSFTPFileCtx":
        """Return an async context manager yielding a file-like proxy.

        Call sites in :mod:`repose.target.parsers.product` use the
        async variant via ``async with await target_open(...) as f:``
        on the async side; the sync paramiko backend still exposes the
        synchronous version. The proxy returned here implements
        ``async with`` plus ``for line in f`` (sync iteration over
        cached lines), and an awaitable ``.read()``. The files we
        touch (``/etc/os-release``, ``/etc/products.d/*.prod``) are
        tiny (< 8 KiB), so the eager read on ``__aenter__`` is fine
        and lets the async parser body stay structurally identical to
        the sync paramiko parser.
        """
        return _AsyncSFTPFileCtx(self, filename, mode)

    def is_active(self) -> bool:
        return self._conn is not None and not self._conn.is_closed()

    async def fire_and_forget(self, command: str) -> None:
        """Dispatch a command without waiting (mirror of the sync backend).

        Starts ``command`` on a new channel and closes the connection;
        a dropped link (e.g. from a reboot) is expected. Follow up with
        :meth:`wait_reconnect`.
        """
        logger.debug("fire-and-forget %r on %s:%s", command, self.hostname, self.port)
        if self._conn is not None:
            try:
                # create_process sends the exec request without waiting
                # for the command to finish — right for a reboot.
                await self._conn.create_process(command)
            except Exception:  # noqa: BLE001 - link teardown is expected
                logger.debug("fire-and-forget dispatch raised", exc_info=True)
        await self.close()

    async def boot_id(self) -> str:
        """Return the host's current boot id, or "" if unreadable."""
        try:
            stdout, _, _ = await self.run("cat /proc/sys/kernel/random/boot_id")
        except Exception:  # noqa: BLE001
            return ""
        return stdout.strip()

    async def wait_reconnect(
        self, retry: int = 10, timeout: int = 10, backoff: bool = True
    ) -> bool:
        """Reconnect after a reboot, retrying while the host is down.

        Returns True once the connection is active again, else False
        after exhausting ``retry``.
        """
        self._conn = None
        self._sftp = None
        count = 0
        rtimeout = timeout
        while not self.is_active() and count < retry:
            count += 1
            await asyncio.sleep(rtimeout)
            if backoff:
                rtimeout = 2 * (timeout + 5 * count)
            try:
                await self.connect()
            except Exception:  # noqa: BLE001 - host still rebooting
                logger.debug(
                    "reconnect attempt %d/%d to %s:%s failed",
                    count,
                    retry,
                    self.hostname,
                    self.port,
                    exc_info=True,
                )
        return self.is_active()

    async def close(self) -> None:
        """Close SFTP (if open) and the SSH session. Idempotent."""
        logger.debug("closing connection to %s:%s", self.hostname, self.port)
        if self._sftp is not None:
            try:
                self._sftp.exit()
            except Exception:  # noqa: BLE001
                pass
            self._sftp = None
        if self._conn is not None:
            self._conn.close()
            try:
                await self._conn.wait_closed()
            except Exception:  # noqa: BLE001
                pass
            self._conn = None


class _AsyncSFTPFileCtx:
    """Async context manager backing :meth:`AsyncConnection.open`.

    Designed for two consumption shapes used by the parsers:

    - ``for line in f`` (iterating a small text file)
    - ``await f.read()`` (slurping the whole file)

    The file is pre-read into memory on ``__aenter__`` so the *async*
    parser body can stay a near-mirror of the sync paramiko parser.
    The files we touch are tiny (< 8 KiB) so this is cheap.
    """

    def __init__(self, conn: AsyncConnection, filename: str, mode: str) -> None:
        self._conn = conn
        self._filename = filename
        self._mode = mode
        self._content: str | bytes | None = None

    async def __aenter__(self) -> "_AsyncSFTPFileCtx":
        sftp = await self._conn._sftp_open()
        try:
            async with sftp.open(self._filename, self._mode) as f:
                data = await f.read()
        except asyncssh.SFTPError as exc:
            raise _translate_sftp_error(exc) from exc
        self._content = data
        return self

    async def __aexit__(self, *exc_info: Any) -> None:
        self._content = None

    def __iter__(self):
        if self._content is None:
            return iter(())
        if isinstance(self._content, bytes):
            return iter(
                self._content.decode("utf-8", "ignore").splitlines(keepends=True)
            )
        return iter(self._content.splitlines(keepends=True))

    async def read(self) -> str | bytes:
        return self._content if self._content is not None else ""
