REPOSE-REMOVE(1) BSD General Commands Manual REPOSE-REMOVE(1)

NAME repose remove — Remove matching repositories

SYNOPSIS repose remove -h \| --help repose remove [-n \| --print]
HOST... -- REPA...

DESCRIPTION repose remove removes, from each HOST, any number of package
repositories. repose remove uses repoq(1) to gen- erate zypper
removerepo commands.

OPTIONS -h Display usage instructions.

::

     --help
         Display this manual page.

     -n, --print
         Write destructive operations to standard output, do not actually perform them.

OPERANDS HOST Machine to operate on (see repose(1)).

::

     REPA
         Repository pattern (see repose(1)). Matching repositories will be removed.

EXAMPLES Remove repositories whose names have the gm or up tag.

::

        $ repose remove root@fubar.example.org -- :::at,nv
     Remove, from both hosts, repositories for sle-sdk and sle-we products.

     $ repose remove root@{fubar,snafu}.example.org -- sle-sdk sle-we

SEE ALSO repose(1), repose-add(1), smrt(1), ssh(1), zypper(8).

REPOSE repose remove is part of repose(1).

BSD Feb 04, 2016 BSD
