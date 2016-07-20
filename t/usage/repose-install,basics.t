help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose install -h
  usage: repose install -h | --help | [-n] HOST... -- ADDON...
  Install a product, add its repositories
    Options:
      -h                    Display this message
      --help                Display full help
      -n,--print            Display, do not perform destructive commands
  
    Operands:
      HOST                  Machine to operate on
      ADDON                 Product to install

  $ repose install --help
  o exec man 1 repose-install
