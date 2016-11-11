.. vim: ft=rst sw=2 sts=2 et

===============
 **repose-add**
===============

-------------------------
Add matching repositories
-------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-2.0
:Version: 0.28
:Manual section: 1
:Manual group: BSD General Commands Manual

SYNOPSIS
========

**repose add** **-h** \| **--help**

**repose add** [**-n** \| **--print**] [**-t** *TAG*]... *HOST*... -- *REPA*...

DESCRIPTION
===========

**repose add** adds, in each *HOST*, any number of package repositories. **repose add** uses repoq(1) to generate **zypper addrepo** commands.

OPTIONS
=======

:-h: Display usage instructions.

:--help:
 Display this manual page.

:-n, --print:
 Write destructive operations to standard output, do not actually perform them.

:-t, --tag=\ *TAG*:
 Override default *TAG*\ s for tagless *REPA*\ s. See *REPA* in **OPERANDS** below and **-t**, **--tag** in repoq(1).

OPERANDS
========

*HOST*
 Machine to operate on (see repose(1)).

*REPA*
 | Repository pattern (see repose(1)) with these extra requirements:
  · **P** cannot be empty
  · **V** can be empty, in which case version string of the *baseproduct* is used
  · **A** can be empty, in which case architecture name of the *baseproduct* is used
  · **T** defaults to **gm**,\  **lt**,\  **se**,\  **up**.

EXAMPLES
========

Add the **at**- and **nv**-tagged repositories for whatever **sled** version and architecture **fubar.example.org** is on.

::

  $ repose add root@fubar.example.org -- sled:::at,nv

Add default-tagged repositories for **sle-sdk**, **sle-we** addons. As in the previous example, queries both hosts for their baseproduct versions and architectures.

::

  $ repose add root@{fubar,snafu}.example.org -- sle-sdk sle-we

SEE ALSO
========

repoq(1), repose-remove(1), smrt(1), ssh(1), zypper(8).

REPOSE
======

**repose add** is part of repose(1).
