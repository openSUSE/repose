import sys

from .colorlog import create_logger
from .argparsing import get_parser


def main():
    parser = get_parser()
    logger = create_logger("repose")
    args = parser.parse_args(sys.argv[1:])

    if not hasattr(args, "func"):
        parser.print_usage()
        sys.exit(0)

    if args.debug:
        logger.setLevel("DEBUG")
    elif args.quiet:
        logger.setLevel("WARNING")

    sys.exit(args.func(args))
