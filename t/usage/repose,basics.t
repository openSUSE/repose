help strings
============

setup::

  $ . $TESTROOT/setup


help::

  $ repose -h
  usage: repose -h | --help | COMMAND [options] [operands]
  Manipulate zypper repositories based on /etc/products.d
    Options:
      -h                    Display this message
      --help                Display full help
  
    Commands:
      add                   Add requested repositories
      clear                 Remove all repositories
      install               Install an addon, add its repositories
      issue-add             Add issue-specific repositories
      issue-rm              Remove issue-specific repositories
      list                  List matching repositories
      list-products         List matching products
      remove                Remove matching repositories
      reset                 Remove stray repositories
      switch-to             Enable requested repositories, disable their complementary set

  $ repose --help
  o exec man 1 repose
