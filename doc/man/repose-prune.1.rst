.. vim: ft=rst sw=2 sts=2 et

================
**repose-prune**
================

-------------------------
Remove stray repositories
-------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-2.0
:Version: 0.26
:Manual section: 1
:Manual group: User Commands

SYNOPSIS
========

**repose prune** **-h** \| **--help**

**repose prune** [**-n** \| **--print**] *HOST*... [-- *REPA*...]

DESCRIPTION
===========

**repose prune** removes all package repositories except those that belong to an installed product or are whitelisted by the operands.

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

*HOST*
 Machine to operate on (see repose(1)).

*REPA*
 Repository pattern (see repose(1)). Matching repositories will be kept.

EXAMPLES
========

Whitelist repositories for **sled**.

::

  $ repose prune root@{fubar,snafu}.example.org -- sled

SEE ALSO
========

repoq(1), repose-remove(1), ssh(1), zypper(8).

REPOSE
======

**repose prune** is part of repose(1).
