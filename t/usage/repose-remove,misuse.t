basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose remove -x
  repose remove: unknown option '-x'
  run 'repose remove -h' for usage instructions
  [1]

  $ repose remove --xeno
  repose remove: unknown option '--xeno'
  run 'repose remove -h' for usage instructions
  [1]


no arguments::

  $ repose remove
  repose remove: missing argument
  run 'repose remove -h' for usage instructions
  [1]


just the separator::

  $ repose remove --
  repose remove: missing argument
  run 'repose remove -h' for usage instructions
  [1]


no repository patterns::

  $ repose remove fubar.example.org --
  repose remove: missing argument
  run 'repose remove -h' for usage instructions
  [1]


no repository patterns::

  $ repose remove -- foo
  repose remove: missing argument
  run 'repose remove -h' for usage instructions
  [1]
