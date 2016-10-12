basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose clear -x
  repose clear: unknown option '-x'
  run 'repose clear -h' for usage instructions
  [1]

  $ repose clear --xeno
  repose clear: unknown option '--xeno'
  run 'repose clear -h' for usage instructions
  [1]


no arguments::

  $ repose clear
  repose clear: missing argument
  run 'repose clear -h' for usage instructions
  [1]
