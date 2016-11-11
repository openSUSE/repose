REPOSE-PRUNE(1) BSD General Commands Manual REPOSE-PRUNE(1)

NAME repose prune — Remove stray repositories

SYNOPSIS repose prune -h \| --help repose prune [-n \| --print] HOST...
[-- REPA...]

DESCRIPTION repose prune removes all package repositories except those
that belong to an installed product or are whitelisted by the operands.

OPTIONS -h Display usage instructions.

::

     --help
         Display this manual page.

     -n, --print
         Write destructive operations to standard output, do not actually perform them.

OPERANDS HOST Machine to operate on (see repose(1)).

::

     REPA
         Repository pattern (see repose(1)). Matching repositories will be kept.

EXAMPLES Whitelist repositories for sled.

::

     $ repose prune root@{fubar,snafu}.example.org -- sled

SEE ALSO repoq(1), repose-remove(1), smrt(1), ssh(1), zypper(8).

REPOSE repose prune is part of repose(1).

BSD Feb 04, 2016 BSD
