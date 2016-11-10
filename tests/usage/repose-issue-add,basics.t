help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose issue-add -h
  usage: repose issue-add -h | --help | [-n] HOST... -- ISSUEDIR...
  Add issue-specific repositories
    Options:
      -h                     Display this message
      --help                 Display full help
      -n, --print            Display, do not perform destructive commands
      -v, --verbose          Enable verbose mode for scp,ssh commands
  
    Operands:
      HOST                   Machine to operate on
      ISSUEDIR               Directory with metadata for issue to install repositories for

  $ repose issue-add --help
  o exec man 1 repose-issue-add
