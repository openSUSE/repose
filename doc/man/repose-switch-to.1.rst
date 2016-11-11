REPOSE-SWITCH-TO(1) BSD General Commands Manual REPOSE-SWITCH-TO(1)

NAME repose switch-to — Enable matching repositories, disable their
complementary set

SYNOPSIS repose switch-to -h \| --help repose switch-to [-n \| --print]
HOST... -- REPA...

DESCRIPTION repose switch-to enables specified package repositories,
disables all other repositories.

OPTIONS -h Display usage instructions.

::

     --help
         Display this manual page.

     -n, --print
         Write destructive operations to standard output, do not actually perform them.

OPERANDS HOST Machine to operate on (see repose(1)).

::

     REPA
         Repository pattern.  See repose(1).

EXAMPLES Enable sled repositories, disable everything else.

::

     $ repose switch-to root@{fubar,snafu}.example.org -- sled

SEE ALSO repoq(1), repose-remove(1), smrt(1), ssh(1), zypper(8).

REPOSE repose switch-to is part of repose(1).

BSD Feb 04, 2016 BSD
