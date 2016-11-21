.. vim: ft=rst sw=2 sts=2 et

========================
**repose-list-products**
========================

----------------------
List matching products
----------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-2.0
:Version: 0.26
:Manual section: 1
:Manual group: User Commands

SYNOPSIS
========

**repose list-products** **-h** \| **--help**

**repose list-products** *HOST*...

DESCRIPTION
===========

**repose list-products** displays information about products installed in each *HOST*. Each line of output contains the appropriate *HOST* followed by a single space, followed by **P**:**V**:**A**, where **P** is a **Product name**, **V** is a **Version string**, and **A** is an **Architecture name**. See repoq(1) for details.

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
