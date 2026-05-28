import sys

from ruamel.yaml import YAMLError

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

    # Translate the most common config-load failures into a one-line
    # user-facing message instead of a raw traceback. ``--debug`` still
    # propagates the original exception so contributors can see the
    # full stack. Exit code ``2`` matches the "hard failure" slot in
    # ``ExitCode`` (Literal[0, 1, 2]) used by the command layer.
    try:
        sys.exit(args.func(args))
    except FileNotFoundError as e:
        if args.debug:
            raise
        logger.error(
            "config file not found: %s (use -c PATH to point at another)",
            e.filename or "<unknown>",
        )
        sys.exit(2)
    except PermissionError as e:
        if args.debug:
            raise
        logger.error("permission denied reading config: %s", e.filename or "<unknown>")
        sys.exit(2)
    except IsADirectoryError as e:
        if args.debug:
            raise
        logger.error(
            "config path is a directory, not a file: %s", e.filename or "<unknown>"
        )
        sys.exit(2)
    except YAMLError as e:
        if args.debug:
            raise
        logger.error("invalid YAML in config: %s", e)
        sys.exit(2)
    except KeyboardInterrupt:
        # Standard convention: 128 + SIGINT (2) = 130.
        logger.error("interrupted")
        sys.exit(130)
