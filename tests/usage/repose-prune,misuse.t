basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose prune -x
  repose prune: unknown option '-x'
  run 'repose prune -h' for usage instructions
  [1]

  $ repose prune --xeno
  repose prune: unknown option '--xeno'
  run 'repose prune -h' for usage instructions
  [1]


no arguments::

  $ repose prune
  repose prune: missing argument
  run 'repose prune -h' for usage instructions
  [1]
