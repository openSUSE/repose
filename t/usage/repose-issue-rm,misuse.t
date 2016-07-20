basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose issue-rm -x
  repose issue-rm: unknown option '-x'
  run 'repose issue-rm -h' for usage instructions
  [1]

  $ repose issue-rm --xeno
  repose issue-rm: unknown option '--xeno'
  run 'repose issue-rm -h' for usage instructions
  [1]


no arguments::

  $ repose issue-rm
  repose issue-rm: missing argument
  run 'repose issue-rm -h' for usage instructions
  [1]
