help strings
============

setup::

  $ . $TESTROOT/setup


short::

  $ repoq -h
  usage: repoq -h | --help | [-F RULES][-A|-R][-N][-a ARCH][-t TAG]... SPEC...
  Output repository information for given products
    Options:
      -h                    Display this message
      --help                Display full help
      -A,--addrepo          Output `zypper addrepo` commands
      -F,--file=RULES       Use product/repository information from RULES
      -N,--no-name          Omit repository names
      -R,--removerepo       Output `zypper removerepo` commands
      -a,--arch=ARCH        Default architecture of requested repositories
      -t,--tag=TAG          Imply TAG for SPECs with no tagset
  
    Operands:
      SPEC                  P:V[:A[:T]


long::

  $ repoq --help
  o exec man 1 repoq
