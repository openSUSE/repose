basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose issue-add -x
  repose issue-add: unknown option '-x'
  run 'repose issue-add -h' for usage instructions
  [1]

  $ repose issue-add --xeno
  repose issue-add: unknown option '--xeno'
  run 'repose issue-add -h' for usage instructions
  [1]


no arguments::

  $ repose issue-add
  repose issue-add: missing argument
  run 'repose issue-add -h' for usage instructions
  [1]


no issue arguments (only hosts)::

  $ repose issue-add fubar.example.org
  repose issue-add: missing argument
  run 'repose issue-add -h' for usage instructions
  [1]

  $ repose issue-add fubar.example.org --
  repose issue-add: missing argument
  run 'repose issue-add -h' for usage instructions
  [1]

  $ repose issue-add {fubar,snafu}.example.org
  repose issue-add: missing argument
  run 'repose issue-add -h' for usage instructions
  [1]

  $ repose issue-add {fubar,snafu}.example.org --
  repose issue-add: missing argument
  run 'repose issue-add -h' for usage instructions
  [1]
