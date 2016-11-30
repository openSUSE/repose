.. vim: ft=rst sw=2 sts=2 et

==================
 **repose-remove**
==================

----------------------------
Remove matching repositories
----------------------------

:Authors:
:Date: Feb 04, 2016
:Copyright: GPL-3.0
:Version: @VERSION@
:Manual section: 1
:Manual group: User Commands
:Maintainer: openSUSE Project

SYNOPSIS
========

**repose remove** **-h** \| **--help**

**repose remove** [**-v** \| **--verbose**] [**-n** \| **--print**] *HOST*... -- *REPA*...

DESCRIPTION
===========

**repose remove** removes, from each *HOST*, any number of package repositories.

**repose remove** uses repoq(1) to generate **zypper removerepo** commands.

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

*HOST*
  Machine to operate on (see repose(1)).

*REPA*
  Repository pattern (see repose(1)). Matching repositories will be removed.

EXAMPLES
========

Remove repositories whose names have the **gm** or **up** tag.

::

     $ repose remove root@fubar.example.org -- :::at,nv

Remove, from both hosts, repositories for **sle-sdk** and **sle-we** products.

::

     $ repose remove root@{fubar,snafu}.example.org -- sle-sdk sle-we

SEE ALSO
========

repose(1), repose-add(1), ssh(1), zypper(8).

REPOSE
======

**repose remove** is part of repose(1).
