from abc import ABC, abstractmethod
from argparse import Namespace
import asyncio
import concurrent.futures
from concurrent.futures import Future
import functools
import logging
import sys
from pathlib import Path
from typing import Any, Awaitable, Callable, ClassVar, Iterable, cast

from ..console import Console
from ..display import CommandDisplay, JsonCommandDisplay
from ..progress import Progress
from ..target.async_hostgroup import AsyncHostGroup
from ..target.hostgroup import HostGroup
from ..template import load_template
from ..template.resolver import Repoq
from ..types import ExitCode
from ..types.repa import Repa
from ..utils import check_repo_url, check_repo_url_async

logger = logging.getLogger("repose.command")

# zypper exit codes that still mean the operation completed. Beyond 0,
# these are the informational codes where zypper did the work and only
# reports a follow-up condition: patches available (100/101), a reboot
# (102) or package-manager restart (103) required after a *successful*
# install, some repositories skipped on refresh (106), or a post-install
# %post script that failed after the transaction already committed (107).
# See zypper(8) EXIT CODES. Every other non-zero code -- the 1-8 error
# range (incl. 4 ERR_ZYPP and 6 NO_REPOS), plus 104 (capability not
# found) and 105 (interrupted) -- is a genuine failure.
ZYPPER_EXIT_OK = 0
ZYPPER_SUCCESS_EXIT_CODES = frozenset({0, 100, 101, 102, 103, 106, 107})

# Per-host progress updater signature. ``_run_parallel`` binds
# ``Progress.update`` and passes it as the second positional argument
# to every worker ``_run(host, update, *extra)``. Defined here so the
# six concrete commands share one canonical alias instead of redefining
# it locally.
UpdateFn = Callable[[str, str], None]


class Command(ABC):
    registry: ClassVar[dict[str, type["Command"]]] = {}

    def __init_subclass__(cls, *, name: str | None = None, **kwargs: Any) -> None:
        super().__init_subclass__(**kwargs)
        # ``name`` is optional so abstract intermediates (e.g. a future
        # base shared by several concrete commands) can inherit from
        # ``Command`` without polluting the CLI registry. Concrete
        # commands MUST pass ``name=``.
        if name is None:
            return
        if name in Command.registry:
            raise RuntimeError(f"Duplicate command name: {name!r}")
        Command.registry[name] = cls

    addcmd: str = "zypper -n ar {params} {name} {url} {name}"
    rrcmd: str = "zypper -n rr {repos}"
    refcmd: str = "zypper -n --gpg-auto-import-keys ref -f"
    # Transactional refresh: import repo signing keys into the *snapshot*
    # keyring before a transactional product install. On a transactional
    # host the plain ``refcmd`` runs on the live system, where the rpmdb is
    # read-only, so a freshly added repo's GPG key never lands where the
    # ``transactional-update`` inner ``zypper -R <snapshot>`` looks for it —
    # the inner zypper then rejects the repo's ``repomd.xml`` signature and
    # the install aborts (exit 4). Running the auto-importing refresh through
    # ``transactional-update run`` writes the key into a snapshot the
    # subsequent ``pkg in`` builds upon.
    reftcmd: str = "transactional-update -n run zypper -n --gpg-auto-import-keys ref -f"
    ipdcmd: str = "zypper -n in -t product -l -f {products}"
    rrpcmd: str = "zypper -n rm -t product {products}"
    # ``-n`` makes transactional-update non-interactive, which in turn runs
    # the inner ``zypper`` with ``--non-interactive``. Without it the inner
    # zypper prompts ``Continue? [y/n]`` and, having no terminal under an SSH
    # exec, dies with ``Cannot read input: bad stream or EOF`` (exit 4) — the
    # snapshot is then discarded and the product never installs. The
    # non-transactional twins above already use ``zypper -n``.
    ipdtcmd: str = "transactional-update -n pkg in -t product -l -f {products}"
    rrpdtcmd: str = "transactional-update -n pkg rm -t product -l -f {products}"
    reboot: str = "systemctl reboot"

    # Runtime backend selection produces one of two concrete dict-like
    # host groups. Commands' ``_srun``/``_arun`` bodies pick the
    # appropriate branch (see :meth:`run`); the union is declared here
    # so callers narrow it with ``isinstance`` instead of leaning on
    # blanket type-checker suppressions.
    targets: AsyncHostGroup | HostGroup

    def __init__(self, args: Namespace) -> None:
        __dtargets: dict = {}

        if "target" in args:
            for x in args.target:
                __dtargets.update(x)

        # Backend selection. ``ssh_backend`` is set unconditionally by
        # the typer CLI; tests that build a Namespace directly may
        # omit it — default to "paramiko" there so the sync path
        # remains the unchanged baseline for legacy fixtures.
        self.ssh_backend: str = getattr(args, "ssh_backend", "paramiko")
        self._is_async: bool = self.ssh_backend == "asyncssh"

        if self._is_async:
            # Do NOT call ``asyncio.run(targets_a.connect())`` here.
            # That would bind the underlying asyncssh ``SSHClientConnection``
            # objects (and their internal Futures) to a loop that is torn
            # down the moment ``asyncio.run`` returns. The later
            # ``asyncio.run(self._arun())`` in :meth:`run` would then touch
            # those Futures from a *different* loop, raising
            # ``got Future <...> attached to a different loop``. Defer
            # the connect-and-prune step into :meth:`run` so it shares the
            # one loop that also drives ``_arun``.
            self.targets = AsyncHostGroup(__dtargets)
        else:
            targets_s: HostGroup = HostGroup(__dtargets)
            targets_s.connect()
            # cann't  use dict comprehension - custom dict for hostgroup:(
            for target in list(targets_s.keys()):
                if not targets_s[target]:
                    del targets_s[target]
            self.targets = targets_s

        self.dryrun: bool = args.dry
        # ``args.config`` is already a ``Path`` (argparse ``type=Path``);
        # the prior ``str`` annotation was a lie.
        self.template_path: Path = args.config
        # ``repa`` is None for commands that don't accept a REPA argument
        # (list, known, clear, reset). Commands that *do* iterate over it
        # (add, install, remove, uninstall) gate on truthiness first.
        self.repa: list[Repa] = args.repa if "repa" in args else []
        self.yaml: bool = args.yaml if "yaml" in args else False
        output_format = "json" if getattr(args, "format", "text") == "json" else "text"
        self.console = Console(
            format=output_format,
            color="never" if getattr(args, "no_color", False) else "auto",
        )
        # Payload-emitting commands (list-products, list-repos,
        # known-products) write through ``self.display``. The JSON
        # variant mirrors the Console envelope so ``--format=json``
        # produces a uniform NDJSON stream across every subcommand.
        if output_format == "json":
            self.display = JsonCommandDisplay(sys.stdout)
        else:
            self.display = CommandDisplay(sys.stdout)
        # Probe knobs. ``getattr`` is defensive: commands without the
        # ``probe`` argparse group (list-products, clear, known-products,
        # ...) construct cleanly with the same defaults that apply when
        # the flag is absent for probing commands.
        self.probe_timeout: float = getattr(args, "probe_timeout", 5.0)
        self.no_probe: bool = getattr(args, "no_probe", False)
        # ``quiet`` is read by ``_make_progress`` to suppress the live
        # progress overlay even on a TTY. The same flag also drives the
        # logger-level cap in ``repose.cli``; the two effects are
        # intentionally independent (a quiet user still wants warnings,
        # just not the live table).
        self.quiet: bool = getattr(args, "quiet", False)
        # Transactional hosts must reboot for a staged package change to
        # take effect; repose reboots + reconnects + verifies by default.
        # ``--no-reboot`` stages the change and only prints a reminder.
        # ``getattr`` keeps direct-``Namespace`` tests (and non-install/
        # uninstall commands) constructing cleanly.
        self.no_reboot: bool = getattr(args, "no_reboot", False)

    @functools.cached_property
    def repoq(self) -> Repoq:
        """Shared ``Repoq`` resolver built once per ``Command`` instance.

        Thread-safety: ``cached_property`` is safe for read-after-write,
        but the *first* read materialises the value. Callers MUST ensure
        the first access happens on the main thread before any worker
        thread (e.g. ``_run_parallel``) is spawned, otherwise two
        workers may race and each construct a ``Repoq``. ``_run_parallel``
        consumers in this codebase satisfy the invariant by touching
        ``self.repoq`` only from within per-host worker functions that
        all observe the same instance once one of them populates the
        cache (a wasted re-construct, not a correctness bug, in the
        unlikely tied case).
        """
        return Repoq(load_template(self.template_path))

    def _report_target(self, target: str) -> bool:
        """Report the last command's output for ``target`` via Console.

        Returns ``True`` for zypper exit 0 and the informational success
        codes in :data:`ZYPPER_SUCCESS_EXIT_CODES` -- e.g. 102 ("reboot
        needed") / 103 ("restart needed") after a *successful* install,
        which repose otherwise handles out of band. Every other non-zero
        exit -- the 1-8 error range (including 4 ``ERR_ZYPP`` and 6
        ``NO_REPOS``), plus 104 (capability not found) and 105
        (interrupted) -- is a failure: the call returns ``False`` so the
        caller propagates the per-host failure into ``_aggregate`` rather
        than masking it as success.
        """
        out = self.targets[target].out[-1]
        exitcode = out[3]
        if exitcode == ZYPPER_EXIT_OK:
            for line in out[1].splitlines():
                self.console.report(target, line, ok=True, level="info")
            return True
        if exitcode in ZYPPER_SUCCESS_EXIT_CODES:
            # The work completed but a non-zero code carries a follow-up
            # note (e.g. 106 "repository skipped", 107 "%post failed")
            # that zypper may write to *either* stream, so surface both
            # at warning level rather than dropping the explanation.
            for stream in (out[1], out[2]):
                for line in stream.splitlines():
                    self.console.report(target, line, ok=True, level="warning")
            return True
        # Surface whatever zypper emitted, on either stream: some
        # diagnostics (e.g. "repository already exists") go to stdout, so
        # reporting stderr alone would leave a non-zero exit unexplained.
        for stream in (out[1], out[2]):
            for line in stream.splitlines():
                self.console.report(target, line, ok=False, level="error")
        return False

    def _check_products(self, host: str, products: list[str], present: bool) -> bool:
        """Verify ``products`` are present/absent in the host's re-read state.

        ``present=True`` (install) requires each product to now be
        installed; ``present=False`` (uninstall) requires each to be
        gone. Caller must have refreshed ``targets[host].products``.
        """
        system = self.targets[host].products
        installed = {p.name for p in system.flatten()} if system else set()
        ok = True
        for product in products:
            if present and product not in installed:
                logger.error("%s: product %s not installed after reboot", host, product)
                ok = False
            elif not present and product in installed:
                logger.error("%s: product %s still present after reboot", host, product)
                ok = False
        if ok:
            logger.info(
                "%s: verified product(s) %s after reboot",
                host,
                ", ".join(products),
            )
        return ok

    def _reboot_and_verify(
        self, host: str, products: list[str], present: bool = True
    ) -> bool:
        """Reboot a transactional host, then verify the change took (sync).

        With ``--no-reboot`` the change is left staged and only a reminder
        is logged (returns True). Otherwise the host is rebooted +
        reconnected and its products are re-read and checked.
        """
        if self.no_reboot:
            logger.info(
                "Reboot %s to activate the staged snapshot (--no-reboot set)",
                host,
            )
            return True
        if not self.targets[host].reboot(self.reboot):
            return False
        try:
            self.targets[host].read_products()
        except Exception:
            logger.error("%s: could not re-read products after reboot", host)
            logger.debug("re-read failure", exc_info=True)
            return False
        return self._check_products(host, products, present)

    async def _areboot_and_verify(
        self, host: str, products: list[str], present: bool = True
    ) -> bool:
        """Async mirror of :meth:`_reboot_and_verify`."""
        if self.no_reboot:
            logger.info(
                "Reboot %s to activate the staged snapshot (--no-reboot set)",
                host,
            )
            return True
        if not await self.targets[host].reboot(self.reboot):
            return False
        try:
            await self.targets[host].read_products()
        except Exception:
            logger.error("%s: could not re-read products after reboot", host)
            logger.debug("re-read failure", exc_info=True)
            return False
        return self._check_products(host, products, present)

    def _make_progress(self) -> Progress:
        """Construct the live-progress overlay for this command run.

        Auto-disables on non-TTY stdout, ``--format=json`` (the
        machine-readable consumer is the only thing on stdout) and
        ``--quiet``. When disabled the returned ``Progress`` is a
        no-op context manager — state still mutates internally but
        nothing renders and logging is left untouched.
        """
        enabled = (
            sys.stdout.isatty() and self.console.format != "json" and not self.quiet
        )
        return Progress(list(self.targets.keys()), enabled=enabled)

    def _worker(
        self,
        fn: Callable[..., bool],
        host: str,
        prog: Progress,
        extra: tuple[Any, ...],
    ) -> bool:
        """Wrap ``fn(host, update, *extra)`` with progress bookkeeping.

        Posts ``"running"`` before invoking ``fn``, ``"[green]done"`` /
        ``"[red]failed"`` after, and ``"[red]failed"`` if ``fn`` raises
        (then re-raises so ``_aggregate`` sees the exception via the
        future).
        """
        prog.update(host, "running")
        try:
            ok = fn(host, prog.update, *extra)
        except Exception:
            prog.update(host, "[red]failed")
            raise
        prog.update(host, "[green]done" if ok else "[red]failed")
        return ok

    def _run_parallel(
        self,
        fn: Callable[..., bool],
        *extra_args: Any,
    ) -> list[Future[bool]]:
        """Fan ``fn(host, update, *extra_args)`` across all live targets.

        ``fn`` receives a per-host ``update(host, status)`` callable
        as its second positional argument; commands call it at
        meaningful milestones to keep the live overlay informative.

        Returns the futures so callers can inspect ``.exception()``
        and ``.result()`` (consumed by ``_aggregate`` for exit-code
        propagation).
        """
        with self._make_progress() as prog:
            with concurrent.futures.ThreadPoolExecutor() as ex:
                futures = [
                    ex.submit(self._worker, fn, host, prog, extra_args)
                    for host in self.targets.keys()
                ]
                concurrent.futures.wait(futures)
                return futures

    def _aggregate(self, futures: list[Future[bool]]) -> ExitCode:
        """Collapse per-target futures into a process exit code.

        A future counts as failed when it raised (``f.exception() is not
        None``) or when its result is explicitly ``False``. A result of
        ``True`` (or any non-``False`` truthy value) counts as success
        for forward compatibility with callers that may still return
        ``None`` during transition.

        Returns ``0`` when every future succeeded, ``2`` when every
        future failed (including the degenerate single-host all-failed
        case), and ``1`` otherwise.
        """
        total = len(futures)
        if total == 0:
            return 0
        failed = 0
        for f in futures:
            if f.exception() is not None:
                failed += 1
                continue
            if f.result() is False:
                failed += 1
        if failed == 0:
            return 0
        if failed == total:
            return 2
        return 1

    async def _aworker(
        self,
        fn: Callable[..., Awaitable[bool]],
        host: str,
        prog: Progress,
        extra: tuple[Any, ...],
    ) -> bool:
        """Async sibling of :meth:`_worker`.

        Same progress-bookkeeping contract; awaits the coroutine
        produced by ``fn`` instead of calling a sync function. Used
        by :meth:`_arun_parallel` on the asyncssh backend.
        """
        prog.update(host, "running")
        try:
            ok = await fn(host, prog.update, *extra)
        except Exception:
            prog.update(host, "[red]failed")
            raise
        prog.update(host, "[green]done" if ok else "[red]failed")
        return ok

    async def _arun_parallel(
        self,
        fn: Callable[..., Awaitable[bool]],
        *extra_args: Any,
    ) -> list[asyncio.Task[bool | BaseException]]:
        """Async fan-out via :class:`asyncio.TaskGroup`.

        Mirror of :meth:`_run_parallel` but every worker is a
        coroutine. Per-host exceptions are *not* swallowed here — they
        surface on the returned tasks and feed :meth:`_aggregate_tasks`
        for exit-code propagation, exactly like the sync path's
        ``Future.exception()`` flow.

        The progress overlay (rich ``Live``) is started on the main
        thread before any coroutine runs; this is the same invariant
        the sync path relies on and keeps the asyncssh path free of
        thread-vs-event-loop renderer interactions.
        """

        # Trap exceptions inside the per-task wrapper so the TaskGroup
        # default cancel-siblings semantic doesn't tear down hosts
        # that are still making forward progress when one fails.
        async def _trap(coro: Awaitable[bool]) -> bool | BaseException:
            try:
                return await coro
            except BaseException as exc:  # noqa: BLE001
                return exc

        with self._make_progress() as prog:
            async with asyncio.TaskGroup() as tg:
                tasks = [
                    tg.create_task(
                        _trap(self._aworker(fn, host, prog, extra_args)),
                        name=f"arun:{host}",
                    )
                    for host in self.targets.keys()
                ]
        return tasks

    def _aggregate_tasks(self, tasks: list[asyncio.Task[Any]]) -> ExitCode:
        """Collapse async task results into a process exit code.

        Mirror of :meth:`_aggregate` but consumes ``asyncio.Task`` —
        plus the ``_trap`` wrapper in :meth:`_arun_parallel` may have
        returned a ``BaseException`` instance instead of raising;
        treat such returns as failures.

        Returns the same 0/1/2 triplet as the sync aggregator.
        """
        total = len(tasks)
        if total == 0:
            return 0
        failed = 0
        for t in tasks:
            # Tasks always finished by the time TaskGroup exited.
            if t.exception() is not None:
                failed += 1
                continue
            result = t.result()
            if isinstance(result, BaseException):
                failed += 1
                continue
            if result is False:
                failed += 1
        if failed == 0:
            return 0
        if failed == total:
            return 2
        return 1

    def _filter_live_urls(self, repos: Iterable[Any]) -> list[Any]:
        """Return only the repos whose URL passes ``check_repo_url``.

        Probes run in parallel via ``ThreadPoolExecutor`` so wall time
        scales with ``max(latency)`` rather than ``sum(latency)``.
        Pool size is capped at 16 to keep concurrent connections to a
        single mirror reasonable; smaller batches use a correspondingly
        smaller pool.

        Honors ``self.no_probe`` (short-circuits, returning every repo
        unchanged) and ``self.probe_timeout`` (per-probe socket
        timeout). Order of the returned list mirrors the input order.
        """
        repos = list(repos)
        if self.no_probe or not repos:
            return repos
        max_workers = min(16, len(repos))
        with concurrent.futures.ThreadPoolExecutor(max_workers=max_workers) as ex:
            alive = list(
                ex.map(
                    lambda r: check_repo_url(r.url, timeout=self.probe_timeout),
                    repos,
                )
            )
        return [r for r, ok in zip(repos, alive) if ok]

    async def _afilter_live_urls(self, repos: Iterable[Any]) -> list[Any]:
        """Async equivalent of :meth:`_filter_live_urls` using ``httpx``.

        Same contract — ``no_probe`` short-circuits, ``probe_timeout``
        bounds each probe, the returned list preserves input order —
        and the same effective concurrency cap of 16 (via a
        ``asyncio.Semaphore``) so a single mirror doesn't see a
        thundering herd of connections from one cohort.
        """
        repos = list(repos)
        if self.no_probe or not repos:
            return repos
        sem = asyncio.Semaphore(min(16, len(repos)))

        async def _gated(repo: Any) -> bool:
            async with sem:
                return await check_repo_url_async(repo.url, timeout=self.probe_timeout)

        alive = await asyncio.gather(*(_gated(r) for r in repos))
        return [r for r, ok in zip(repos, alive) if ok]

    # ------------------------------------------------------------------
    # Public entry point
    # ------------------------------------------------------------------

    def run(self) -> ExitCode:
        """Dispatch to the sync or async command body.

        Concrete subclasses implement :meth:`_srun` (sync, paramiko
        backend) and optionally :meth:`_arun` (async, asyncssh
        backend). When a subclass omits ``_arun`` we fall through to
        ``_srun`` even on the async backend — this is the safety net
        for read-only commands whose work is in-memory only (list,
        known-products) and don't benefit from an async rewrite.

        On the async backend we drive ``connect`` *and* ``_arun``
        inside a single ``asyncio.run`` so the asyncssh connections
        (and their internal Futures) stay bound to one loop for the
        entire command lifetime — see the matching note in
        :meth:`__init__`.
        """
        if self._is_async and type(self)._arun is not Command._arun:
            return asyncio.run(self._aentry())
        if self._is_async:
            # Async backend but no ``_arun``: still need to bring the
            # connections up (and prune dead hosts) before falling
            # back to the sync body, which only inspects in-memory
            # state populated by the sync HostGroup. The sync body
            # here is a read-only helper (list, known-products) that
            # doesn't touch ``self.targets`` for I/O, so no further
            # bridging is required.
            asyncio.run(self._aconnect_and_prune())
            return self._srun()
        return self._srun()

    async def _aconnect_and_prune(self) -> None:
        """Open every host and drop the ones that failed to connect.

        Mirrors the per-host pruning the sync branch does in
        ``__init__``: ``connect()`` populates ``is_connected`` and the
        falsy targets get removed so subsequent fan-outs only iterate
        the live set. Reached only via :meth:`run` after the
        ``_is_async`` gate, so ``self.targets`` is always an
        ``AsyncHostGroup`` here — the ``cast`` keeps the type-checker
        happy without a runtime ``isinstance`` (which would also reject
        the ``MagicMock`` replacement used in unit tests).
        """
        targets = cast(AsyncHostGroup, self.targets)
        await targets.connect()
        for target in list(targets.keys()):
            if not targets[target]:
                del targets[target]

    async def _aentry(self) -> ExitCode:
        """Connect, prune, then run the subclass ``_arun`` body.

        Single coroutine so :meth:`run` only invokes ``asyncio.run``
        once per command. Keeping the connect step here (and not in
        ``__init__``) is what keeps asyncssh's per-connection Futures
        bound to the loop that ``_arun`` later awaits on.
        """
        await self._aconnect_and_prune()
        return await self._arun()

    @abstractmethod
    def _srun(self) -> ExitCode:
        """Sync command body — runs on the paramiko backend."""
        return 0

    async def _arun(self) -> ExitCode:
        """Async command body — overridden by subclasses that need it.

        The default raises ``NotImplementedError``; commands that don't
        override it must still work because :meth:`run` only routes
        here when ``_arun`` is overridden (the ``Command._arun`` identity
        check above). The body remains so subclasses have a canonical
        signature to override against.
        """
        raise NotImplementedError(f"{type(self).__name__}._arun() is not implemented")
