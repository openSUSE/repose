basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose switch-to -x
  repose switch-to: unknown option '-x'
  run 'repose switch-to -h' for usage instructions
  [1]

  $ repose switch-to --xeno
  repose switch-to: unknown option '--xeno'
  run 'repose switch-to -h' for usage instructions
  [1]


no arguments::

  $ repose switch-to
  repose switch-to: missing argument
  run 'repose switch-to -h' for usage instructions
  [1]
