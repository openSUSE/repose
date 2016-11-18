.. vim: ft=rst sw=2 sts=2 et

=================
 **repose-reset**
=================

-------------------------------------------
Remove stray repositories, add missing ones
-------------------------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-2.0
:Version: 0.28
:Manual section: 1
:Manual group: BSD General Commands Manual

SYNOPSIS
========

**repose reset** **-h** \| **--help**

**repose reset** [**-n** \| **--print**] [**-t** *TAG*]... *HOST*...

DESCRIPTION
===========

**repose reset** removes all package repositories except those that belong to an installed product, and adds missing repositories for installed products.

OPTIONS
=======

:-h:
  Display usage instructions.

:--help:
  Display this manual page.

:-n, --print:
  Write destructive operations to standard output, do not actually perform them.

:-t, --tag=\ *TAG*:
  See **-t**, **--tag** in repose-add(1).

OPERANDS
========

*HOST*
  Machine to operate on (see repose(1)).

EXAMPLES
========

::

$ repose reset root@{fubar,snafu}.example.org

SEE ALSO
========

repoq(1), repose(1), ssh(1), zypper(8).

REPOSE
======

**repose reset** is part of repose(1).
