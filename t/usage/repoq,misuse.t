basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repoq -x
  repoq: unknown option '-x'
  run 'repoq -h' for usage instructions
  [1]

  $ repoq --xeno
  repoq: unknown option '--xeno'
  run 'repoq -h' for usage instructions
  [1]


no arguments::

  $ repoq
  repoq: missing argument
  run 'repoq -h' for usage instructions
  [1]


incomplete request::

  $ repoq sles:12
  repoq: no architecture requested for 'sles:12'
  run 'repoq -h' for usage instructions
  [1]
