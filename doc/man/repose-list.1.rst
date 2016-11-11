.. vim: ft=rst sw=2 sts=2 et

================
 **repose-list**
================

--------------------------
List matching repositories
--------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-2.0
:Version: 0.28
:Manual section: 1
:Manual group: BSD General Commands Manual

SYNOPSIS
========

**repose list** **-h** \| **--help**

**repose list** *HOST*... [-- *REPA*...]

DESCRIPTION
===========

**repose list** displays information about package repositories installed in each *HOST*. Each line of output contains the appropriate *HOST* followed by a single space, followed by a repository URL.

OPTIONS
=======

:-h:
  Display usage instructions.

:--help:
  Display this manual page.

OPERANDS
========

*HOST*
  Machine to operate on (see repose(1)).

*REPA*
  Repository pattern, see repose(1). Report only repositories whose names match this pattern.

EXAMPLES
========

Exclude repositories whose names have the **gm** or **up** tag.

::

    $ repose list root@{fubar,snafu}.example.org -- :::~gm,up

SEE ALSO
========

repoq(1), repose(1), repose-list-products(1), smrt(1), ssh(1), zypper(8).

REPOSE
======

**repose list** is part of repose(1).

BSD Feb 04, 2016 BSD
