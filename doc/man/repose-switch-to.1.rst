.. vim: ft=rst sw=2 sts=2 et

====================
**repose-switch-to**
====================

-------------------------------------------------------------
Enable matching repositories, disable their complementary set
-------------------------------------------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-3.0
:Version: @VERSION@
:Manual section: 1
:Manual group: User Commands
:Maintainer: openSUSE Project

SYNOPSIS
========

**repose switch-to** **-h** \| **--help**

**repose switch-to** [**-v** \| **--verbose**] [**-n** \| **--print**] *HOST*... -- *REPA*...

DESCRIPTION
===========

**repose switch-to** enables specified package repositories, disables all other repositories.

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
 Repository pattern. See repose(1).

EXAMPLES
========

Enable **sled** repositories, disable everything else.

::

  $ repose switch-to root@{fubar,snafu}.example.org -- sled

SEE ALSO
========

repoq(1), repose-remove(1), ssh(1), zypper(8).

REPOSE
======

**repose switch-to** is part of repose(1).
