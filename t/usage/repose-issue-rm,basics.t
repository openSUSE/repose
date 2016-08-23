help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose issue-rm -h
  usage: repose issue-rm -h | --help | [-n] HOST... -- ISSUE...
  Remove issue-specific repositories
    Options:
      -h                    Display this message
      --help                Display full help
      -n,--print            Display, do not perform destructive commands
  
    Operands:
      HOST                  Machine to operate on
      ISSUE                 Issue to remove repositories for

  $ repose issue-rm --help
  o exec man 1 repose-issue-rm
