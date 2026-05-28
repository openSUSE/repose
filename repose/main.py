import sys

from .argparsing import get_parser, parse
from .colorlog import create_logger


def main():
    logger = create_logger("repose")
    args = parse(sys.argv[1:])

    if not hasattr(args, "func"):
        # ``parse`` already executed a successful parse_args, so the
        # only way to land here is a bare ``repose`` invocation; rebuild
        # the parser cheaply just to render usage.
        get_parser().print_usage()
        sys.exit(0)

    if args.debug:
        logger.setLevel("DEBUG")
    elif args.quiet:
        logger.setLevel("WARNING")

    sys.exit(args.func(args))
