.. vim: ft=rst sw=2 sts=2 et

=====================
 **repose-issue-add**
=====================

-------------------------------
Add issue-specific repositories
-------------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-2.0
:Version: 0.28
:Manual section: 1
:Manual group: BSD General Commands Manual

SYNOPSIS
========

**repose issue-add** **-h** \| **--help**

**repose issue-add** [**-n** \| **--print**] *HOST*... -- *ISSUEDIR*...

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
