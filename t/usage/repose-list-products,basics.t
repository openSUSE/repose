help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose list-products -h
  usage: repose list-products -h | --help | HOST...
  List matching products
    Options:
      -h                    Display this message
      --help                Display full help
  
    Operands:
      HOST                  Machine to operate on

  $ repose list-products --help
  o exec man 1 repose-list-products
