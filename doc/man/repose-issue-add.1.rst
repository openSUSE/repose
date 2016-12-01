.. vim: ft=rst sw=2 sts=2 et

=====================
 **repose-issue-add**
=====================

-------------------------------
Add issue-specific repositories
-------------------------------

:Authors:
:Copyright: GPL-3.0
:Version: @VERSION@
:Manual section: 1
:Manual group: User Commands
:Maintainer: openSUSE Project

SYNOPSIS
========

**repose issue-add** **-h** \| **--help**

**repose issue-add** [**-v** \| **--verbose**] [**-n** \| **--print**] *HOST*... -- *ISSUEDIR*...

DESCRIPTION
===========

**repose issue-add** adds, in each *HOST*, any number of package repositories for one or more maintenance updates.

**repose issue-add** queries each *HOST* for its architecture and installed products and uses sumaxy(1) to generate appropriate **zypper addrepo** commands.

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

*ISSUEDIR*
  Directory containing metadata for the maintenance issue to add repositories for. See sumaxy(1).

EXAMPLES
========

Install whatever issue repositories are appropriate (as identified by the maintenance update metadata in the current directory) for each of the two hosts:

::

    $ cd SUSE:Maintenance:1234:56789
    $ repose issue-add root@{fubar,snafu}.example.org -- .

SEE ALSO
========

repoq(1), repose(1), repose-issue-rm(1), ssh(1), sumaxy(1), zypper(8).

REPOSE
======

**repose issue-add** is part of repose(1).
