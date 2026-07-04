import os
import ssl
import sys
import time
from functools import lru_cache
from urllib.error import HTTPError, URLError
from urllib.request import urlopen


def timestamp() -> str:
    # remove fractional part
    return str(int(time.time()))


@lru_cache(maxsize=1)
def _system_ssl_context() -> ssl.SSLContext:
    """Return an SSL context that trusts the OS certificate store.

    ``httpx`` defaults to the bundled ``certifi`` CA list, which does
    not include internal/enterprise CAs (e.g. the SUSE trust root that
    ``download.suse.de`` presents). ``urllib`` — and therefore the sync
    probe :func:`check_repo_url` — validates against the system trust
    store instead, so an internal repository reachable by the sync
    backend was silently rejected by the async backend. Building the
    context from the system defaults restores backend parity.

    The context is cached: it is read-only after construction and
    rebuilding it per probe would re-read the CA bundle from disk.
    """
    return ssl.create_default_context()


def check_repo_url(url: str, *, timeout: float = 5.0) -> bool:
    """Check whether a repository URL exposes a valid ``repomd.xml``.

    Tries ``<url>repodata/repomd.xml`` first and falls back to
    ``<url>suse/repodata/repomd.xml`` (used by SUSE-style layouts).

    The ``timeout`` keyword bounds each individual ``urlopen`` call in
    seconds; without it ``urlopen`` would honour
    ``socket.getdefaulttimeout()``, which is typically unset and hangs
    forever on a black-holed IP.

    Returns ``True`` if either probe succeeds, ``False`` otherwise.
    """
    for suffix in ("repodata/repomd.xml", "suse/repodata/repomd.xml"):
        try:
            urlopen(url + suffix, timeout=timeout)
            return True
        except (HTTPError, URLError, TimeoutError, OSError):
            continue
    return False


async def check_repo_url_async(url: str, *, timeout: float = 5.0) -> bool:
    """Async equivalent of :func:`check_repo_url` using ``httpx``.

    Same two-suffix probe order as the sync variant
    (``repodata/repomd.xml`` then ``suse/repodata/repomd.xml``) so a
    repository considered live by one backend is also considered live
    by the other — important for the backend-parity tests. A 2xx-or-3xx
    response counts as live; anything else (4xx/5xx, timeout, network
    error) counts as dead, mirroring the sync ``urlopen``-throws
    semantic.

    HEAD is used in preference to GET to avoid pulling the full
    ``repomd.xml`` payload, with a GET fallback on *any* non-success
    HEAD status. Some servers reject HEAD outright — 405, but also
    400/401/403 from nginx ``limit_except``, WAFs, or S3/proxy layers —
    while serving GET fine; without the broad fallback such a repo,
    judged live by the sync ``urlopen`` (GET) probe, would be reported
    dead here and break backend parity.

    TLS is verified against the system trust store (see
    :func:`_system_ssl_context`) rather than ``httpx``'s bundled
    ``certifi`` list, so internal CAs trusted by the sync probe are
    trusted here too.

    A short-lived ``httpx.AsyncClient`` is created per call; the
    cohort-level batching is the caller's job
    (:meth:`Command._afilter_live_urls`).
    """
    import httpx

    async with httpx.AsyncClient(
        timeout=timeout, verify=_system_ssl_context()
    ) as client:
        for suffix in ("repodata/repomd.xml", "suse/repodata/repomd.xml"):
            target = url + suffix
            try:
                resp = await client.head(target, follow_redirects=True)
                if resp.status_code < 400:
                    return True
                # Any non-success HEAD (405, but also 400/401/403 from
                # servers that reject HEAD yet serve GET) retries with
                # GET, matching the sync urllib (GET) probe's semantics.
                resp = await client.get(target, follow_redirects=True)
                if resp.status_code < 400:
                    return True
            except (httpx.HTTPError, OSError):
                continue
    return False


def _color_enabled() -> bool:
    """Decide whether ANSI color sequences should be emitted.

    Precedence (highest first):

    1. ``COLOR=always`` -> on.
    2. ``COLOR=never`` -> off.
    3. ``NO_COLOR`` set (any value) -> off (https://no-color.org).
    4. ``sys.stdout.isatty()`` -> on for TTYs, off otherwise.
    """
    v = os.environ.get("COLOR")
    if v == "always":
        return True
    if v == "never":
        return False
    if os.environ.get("NO_COLOR") is not None:
        return False
    isatty = getattr(sys.stdout, "isatty", None)
    return bool(isatty and isatty())


def green(xs: str) -> str:
    """Wraps a string in ANSI escape codes to make it green.

    Honours the runtime colour mode (see :mod:`mtui.colorctl`); returns
    the input unchanged when colour is disabled.
    """
    if not _color_enabled():
        return str(xs)
    return f"\033[1;32m{xs!s}\033[1;m\033[0m"


def red(xs: str) -> str:
    """Wraps a string in ANSI escape codes to make it red.

    Honours the runtime colour mode (see :mod:`mtui.colorctl`); returns
    the input unchanged when colour is disabled.
    """
    if not _color_enabled():
        return str(xs)
    return f"\033[1;31m{xs!s}\033[1;m\033[0m"


def yellow(xs: str) -> str:
    """Wraps a string in ANSI escape codes to make it yellow.

    Honours the runtime colour mode (see :mod:`mtui.colorctl`); returns
    the input unchanged when colour is disabled.
    """
    if not _color_enabled():
        return str(xs)
    return f"\033[1;33m{xs!s}\033[1;m\033[0m"


def blue(xs: str) -> str:
    """Wraps a string in ANSI escape codes to make it blue.

    Honours the runtime colour mode (see :mod:`mtui.colorctl`); returns
    the input unchanged when colour is disabled.
    """
    if not _color_enabled():
        return str(xs)
    return f"\033[1;34m{xs!s}\033[1;m\033[0m"
