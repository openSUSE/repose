help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose reset -h
  usage: repose reset -h | --help | [-n] HOST...
  Remove stray repositories, add missing ones
    Options:
      -h                    Display this message
      --help                Display full help
      -n,--print            Display, do not perform destructive commands
  
    Operands:
      HOST                  Machine to operate on

  $ repose reset --help
  o exec man 1 repose-reset
