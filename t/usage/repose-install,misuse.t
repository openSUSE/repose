basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose install -x
  repose install: unknown option '-x'
  run 'repose install -h' for usage instructions
  [1]

  $ repose install --xeno
  repose install: unknown option '--xeno'
  run 'repose install -h' for usage instructions
  [1]


no arguments::

  $ repose install
  repose install: missing argument
  run 'repose install -h' for usage instructions
  [1]
