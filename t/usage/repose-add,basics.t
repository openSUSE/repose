help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose add -h
  usage: repose add -h | --help | [-n] HOST... -- REPA...
  Add matching repositories
    Options:
      -h                    Display this message
      --help                Display full help
      -n,--print            Display, do not perform destructive commands
  
    Operands:
      HOST                  Machine to operate on
      REPA                  Repository pattern

  $ repose add --help
  o exec man 1 repose-add
