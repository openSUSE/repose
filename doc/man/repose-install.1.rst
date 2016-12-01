.. vim: ft=rst sw=2 sts=2 et

===================
 **repose-install**
===================

---------------------------------------
Install a product, add its repositories
---------------------------------------

:Authors:
:Copyright: GPL-3.0
:Version: @VERSION@
:Manual section: 1
:Manual group: User Commands
:Maintainer: openSUSE Project

SYNOPSIS
========

**repose install** **-h** \| **--help**

**repose install** [**-v** \| **--verbose**] [**-n** \| **--print**] [**-t** *TAG*]... *HOST*... -- *REPA*...

DESCRIPTION
===========

**repose install** adds, in each *HOST*, any number of package repositories for one or more addon products and, for each given product **P**, installs the **P**-release package. **repose install** uses repoq(1) to generate **zypper addrepo** commands.

OPTIONS
=======

:-h: Display usage instructions.

:--help:
 Display this manual page.

:-n, --print:
 Write destructive operations to standard output, do not actually perform them.

:-t, --tag=\ *TAG*:
 See **-t**, **--tag** in repose-add(1).

:-v, --verbose:
 Disable quiet mode for ssh and scp that suppresses most warning and diagnostic messages.

OPERANDS
========

*HOST*
  Machine to operate on (see repose(1)).

*REPA*
  | Repository pattern (see repose(1)) with these extra requirements:
     · **P** cannot be empty, and should name an addon product (in other words, there should be a package named **P**-release).
     · **V** can be empty, in which case version string of the *baseproduct* is used.
     · **A** can be empty, in which case architecture name of the *baseproduct* is used.

EXAMPLES
========

Add repositories for **Storage** and **SLE-HA** to **fubar.example.org** and **snafu.example.org**, then install ses-release and sle-ha-release packages in both. **SLE-HA** versions are copied from the respective */etc/products.d/baseproduct* files.

::

        $ repose install root@{fubar,snafu}.example.org -- ses:2.1 sle-ha

SEE ALSO
========

repoq(1), repose(1), ssh(1), zypper(8).

REPOSE
======

**repose install** is part of repose(1).
