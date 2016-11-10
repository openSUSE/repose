help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose list -h
  usage: repose list -h | --help | HOST... [-- REPA...]
  List matching repositories
    Options:
      -h                     Display this message
      --help                 Display full help
      -v, --verbose          Enable verbose mode for scp,ssh commands
  
    Operands:
      HOST                   Machine to operate on
      REPA                   Repository to list

  $ repose list --help
  o exec man 1 repose-list
