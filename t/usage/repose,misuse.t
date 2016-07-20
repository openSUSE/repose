basic misuse
============

setup::

  $ . $TESTROOT/setup


unknown option::

  $ repose -x
  repose: unknown option '-x'
  run 'repose -h' for usage instructions
  [1]

  $ repose --xeno
  repose: unknown option '--xeno'
  run 'repose -h' for usage instructions
  [1]


no arguments::

  $ repose
  repose: missing argument
  run 'repose -h' for usage instructions
  [1]


unknown subcommand::

  $ repose blabla
  error: unknown command 'blabla'
  [1]

  $ REPOSE_CHATTY=\* repose blabla
  O find-cmd blabla
  error: unknown command 'blabla'
  [1]

  $ REPOSE_CHATTY=\* REPOSE_DRYRUN=\* repose blabla
  O find-cmd blabla
  error: unknown command 'blabla'
  [1]
