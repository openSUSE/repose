import os
import sys
import time


def timestamp() -> str:
    # remove fractional part
    return str(int(time.time()))


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
