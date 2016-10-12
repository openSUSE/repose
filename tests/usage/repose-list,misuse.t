basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose list -x
  repose list: unknown option '-x'
  run 'repose list -h' for usage instructions
  [1]

  $ repose list --xeno
  repose list: unknown option '--xeno'
  run 'repose list -h' for usage instructions
  [1]


no arguments::

  $ repose list
  repose list: missing argument
  run 'repose list -h' for usage instructions
  [1]
