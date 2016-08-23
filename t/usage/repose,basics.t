help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose -h
  usage: repose -h | --help | COMMAND [options] [operands]
  Manipulate products and repositories
    Options:
      -h                    Display this message
      --help                Display full help
  
    Commands:
      add                   Add matching repositories
      clear                 Remove all repositories
      install               Install a product, add its repositories
      issue-add             Add issue-specific repositories
      issue-rm              Remove issue-specific repositories
      list                  List matching repositories
      list-products         List matching products
      prune                 Remove stray repositories
      remove                Remove matching repositories
      reset                 Remove stray repositories, add missing ones
      switch-to             Enable matching repositories, disable their complementary set

  $ repose --help
  o exec man 1 repose
