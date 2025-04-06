import os
import time


def timestamp() -> str:
    # remove fractional part
    return str(int(time.time()))


if os.getenv("COLOR", "always") == "always":

    def green(xs) -> str:
        return "\033[1;32m{!s}\033[1;m".format(xs)

    def red(xs) -> str:
        return "\033[1;31m{!s}\033[1;m".format(xs)

    def yellow(xs) -> str:
        return "\033[1;33m{!s}\033[1;m".format(xs)

    def blue(xs) -> str:
        return "\033[1;34m{!s}\033[1;m".format(xs)

else:
    green = red = yellow = blue = lambda xs: str(xs)
