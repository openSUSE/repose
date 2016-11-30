.. vim: ft=rst sw=2 sts=2 et

========================
**repose-list-products**
========================

----------------------
List matching products
----------------------

:Authors:
:Date: Feb 04, 2016
:Copyright: GPL-3.0
:Version: @VERSION@
:Manual section: 1
:Manual group: User Commands
:Maintainer: openSUSE Project

SYNOPSIS
========

**repose list-products** **-h** \| **--help**

**repose list-products** [**-v** \| **--verbose**] *HOST*...

DESCRIPTION
===========

**repose list-products** displays information about products installed in each *HOST*. Each line of output contains the appropriate *HOST* followed by a single space, followed by **P**:**V**:**A**, where **P** is a **Product name**, **V** is a **Version string**, and **A** is an **Architecture name**. See repoq(1) for details.

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

EXAMPLES
========

::

  $ repose list-products root@{fubar,snafu}.example.org

SEE ALSO
========

repose-list(1), ssh(1), zypper(8).

REPOSE
======

**repose list-products** is part of repose(1).
