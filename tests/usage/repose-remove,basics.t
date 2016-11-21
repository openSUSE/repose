help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose remove -h
  usage: repose remove -h | --help | [-n] HOST... -- REPA...
  Remove matching repositories
    Options:
      -h                    Display this message
      --help                Display full help
      -n, --print           Display, do not perform destructive commands
      -v, --verbose         Enable verbose mode for ssh|scp commands
  
    Operands:
      HOST                  Machine to operate on
      REPA                  Repository pattern

  $ repose remove --help
  o exec man 1 repose-remove
