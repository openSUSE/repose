.. vim: ft=rst sw=2 sts=2 et

=================
 **repose-clear**
=================

-----------------------
Remove all repositories
-----------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-3.0
:Version: @VERSION@
:Manual section: 1
:Manual group: User Commands
:Maintainer: openSUSE Project

SYNOPSIS
========

**repose clear** **-h** \| **--help**

**repose clear** [**-v** \| **--verbose**] [**-n** \| **--print**] *HOST*...

DESCRIPTION
===========

**repose clear** removes all package repositories from each *HOST*.

OPTIONS
=======

:-h:
 Display usage instructions.

:--help:
 Display this manual page.

:-n, --print:
 Write destructive operations to standard output, do not actually perform them.

:-v, --verbose:
 Disable quiet mode for ssh and scp that suppresses most warning and diagnostic messages.

OPERANDS
========

*HOST* Machine to operate on (see repose(1)).

EXAMPLES
========

::

$ repose clear root@{fubar,snafu}.example.org

SEE ALSO
========

repoq(1), repose-remove(1), ssh(1), zypper(8).

REPOSE
======

**repose clear** is part of repose(1).
