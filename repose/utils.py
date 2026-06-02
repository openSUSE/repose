import os
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import urlopen


def timestamp() -> str:
    # remove fractional part
    return str(int(time.time()))


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
    ``repomd.xml`` payload, with a GET fallback because some mirrors
    return 405 on HEAD even though the resource exists.

    A short-lived ``httpx.AsyncClient`` is created per call; the
    cohort-level batching is the caller's job
    (:meth:`Command._afilter_live_urls`).
    """
    import httpx

    async with httpx.AsyncClient(timeout=timeout) as client:
        for suffix in ("repodata/repomd.xml", "suse/repodata/repomd.xml"):
            target = url + suffix
            try:
                resp = await client.head(target, follow_redirects=True)
                if resp.status_code < 400:
                    return True
                if resp.status_code == 405:
                    # Some mirrors disallow HEAD; retry with GET.
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


def green(xs) -> str:
    s = str(xs)
    return "\033[1;32m{}\033[1;m".format(s) if _color_enabled() else s


def red(xs) -> str:
    s = str(xs)
    return "\033[1;31m{}\033[1;m".format(s) if _color_enabled() else s


def yellow(xs) -> str:
    s = str(xs)
    return "\033[1;33m{}\033[1;m".format(s) if _color_enabled() else s


def blue(xs) -> str:
    s = str(xs)
    return "\033[1;34m{}\033[1;m".format(s) if _color_enabled() else s
