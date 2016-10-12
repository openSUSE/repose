basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose add -x
  repose add: unknown option '-x'
  run 'repose add -h' for usage instructions
  [1]

  $ repose add --xeno
  repose add: unknown option '--xeno'
  run 'repose add -h' for usage instructions
  [1]


no arguments::

  $ repose add
  repose add: missing argument
  run 'repose add -h' for usage instructions
  [1]


just the separator::

  $ repose add --
  repose add: missing argument
  run 'repose add -h' for usage instructions
  [1]


no repository patterns::

  $ repose add fubar.example.org --
  repose add: missing argument
  run 'repose add -h' for usage instructions
  [1]


no repository patterns::

  $ repose add -- foo
  repose add: missing argument
  run 'repose add -h' for usage instructions
  [1]
