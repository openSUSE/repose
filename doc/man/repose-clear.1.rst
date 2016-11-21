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
:Version: 0.26
:Manual section: 1
:Manual group: User Commands

SYNOPSIS
========

**repose clear** **-h** \| **--help**

**repose clear** [**-n** \| **--print**] *HOST*...

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
