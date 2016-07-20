basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose list-products -x
  repose list-products: unknown option '-x'
  run 'repose list-products -h' for usage instructions
  [1]

  $ repose list-products --xeno
  repose list-products: unknown option '--xeno'
  run 'repose list-products -h' for usage instructions
  [1]


no arguments::

  $ repose list-products
  repose list-products: missing argument
  run 'repose list-products -h' for usage instructions
  [1]
