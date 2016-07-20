help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose reset -h
  usage: repose reset -h | --help | [-n] HOST... [-- REPA...]
  Remove stray repositories
    Options:
      -h                    Display this message
      --help                Display full help
      -n,--print            Display, do not perform destructive commands
  
    Operands:
      HOST                  Machine to operate on
      REPA                  Repository to whitelist

  $ repose reset --help
  o exec man 1 repose-reset
