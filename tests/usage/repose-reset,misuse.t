basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose reset -x
  repose reset: unknown option '-x'
  run 'repose reset -h' for usage instructions
  [1]

  $ repose reset --xeno
  repose reset: unknown option '--xeno'
  run 'repose reset -h' for usage instructions
  [1]


no arguments::

  $ repose reset
  repose reset: missing argument
  run 'repose reset -h' for usage instructions
  [1]
