help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose switch-to -h
  usage: repose switch-to -h | --help | [-n] HOST... -- REPA...
  Enable matching repositories, disable their complementary set
    Options:
      -h                    Display this message
      --help                Display full help
      -n,--print            Display, do not perform destructive commands
  
    Operands:
      HOST                  Machine to operate on
      REPA                  Repository pattern

  $ repose switch-to --help
  o exec man 1 repose-switch-to
