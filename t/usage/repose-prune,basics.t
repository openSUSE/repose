help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose prune -h
  usage: repose prune -h | --help | [-n] HOST... [-- REPA...]
  Remove stray repositories
    Options:
      -h                    Display this message
      --help                Display full help
      -n,--print            Display, do not perform destructive commands
  
    Operands:
      HOST                  Machine to operate on
      REPA                  Repository to whitelist

  $ repose prune --help
  o exec man 1 repose-prune
