.. vim: ft=rst sw=2 sts=2 et

================
 **repose-list**
================

--------------------------
List matching repositories
--------------------------

:Authors:
:Copyright: GPL-3.0
:Version: @VERSION@
:Manual section: 1
:Manual group: User Commands
:Maintainer: openSUSE Project

SYNOPSIS
========

**repose list** **-h** \| **--help**

**repose list** [**-v** \| **--verbose**] *HOST*... [-- *REPA*...]

DESCRIPTION
===========

**repose list** displays information about package repositories installed in each *HOST*. Each line of output contains the appropriate *HOST* followed by a single space, followed by a repository URL.

OPTIONS
=======

:-h:
  Display usage instructions.

:--help:
  Display this manual page.

:-v, --verbose:
 Disable quiet mode for ssh and scp that suppresses most warning and diagnostic messages.

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

repoq(1), repose(1), repose-list-products(1), ssh(1), zypper(8).

REPOSE
======

**repose list** is part of repose(1).
