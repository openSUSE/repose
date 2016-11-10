help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose clear -h
  usage: repose clear -h | --help | [-n] HOST...
  Remove all repositories
    Options:
      -h                     Display this message
      --help                 Display full help
      -n, --print            Display, do not perform destructive commands
      -v, --verbose          Enable verbose mode for scp,ssh commands
  
    Operands:
      HOST                   Machine to operate on

  $ repose clear --help
  o exec man 1 repose-clear
