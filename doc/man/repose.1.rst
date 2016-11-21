.. vim: ft=rst sw=2 sts=2 et

==========
**repose**
==========

------------------------------------
Manipulate products and repositories
------------------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-3.0
:Version: 0.26
:Manual section: 1
:Manual group: User Commands

SYNOPSIS
========

**repose** **-h** \| **--help**

**repose** **COMMAND** [*options*] [*operands*]

DESCRIPTION
===========

**repose** combines repoq(1), sumaxy(1), ssh(1) and zypper(8) to query and manipulate multiple products and package repositories in one or more machines at once.

**COMMAND**\ s are listed below, *options* and *operands* are **COMMAND**-specific and documented in their respective man-pages.

OPTIONS
=======

:-h:
 Display usage instructions.

:--help:
 Display full help.

COMMANDS
========

:add:
 Add matching repositories. See repose-add(1).

:clear:
 Remove all repositories. See repose-clear(1).

:install:
 Install a product, add its repositories. See repose-install(1).

:issue-add:
 Add issue-specific repositories. See repose-issue-add(1).

:issue-rm:
 Remove issue-specific repositories. See repose-issue-rm(1).

:list:
 List matching repositories. See repose-list(1).

:list-products:
 List matching products. See repose-list-products(1).

:prune:
 Remove stray repositories. See repose-prune(1).

:remove:
 Remove matching repositories. See repose-remove(1).

:reset:
 Remove stray repositories, add missing ones. See repose-reset(1).

:switch-to:
 Enable matching repositories, disable their complementary set. See repose-switch-to(1).

HOSTS
=====

*HOST* operands have the following syntax: [*user*\ @]\ *hostname*

**repose** uses ssh(1) to access remote machines. "ssh user@hostname" must work without prompting for any passwords or passphrases.

PRODUCT NAMES
=============

**repose** assumes product names as specified in repoq(1).

REPOSITORY NAMES
================

**repose** assumes repository names as defined in repoq(1).

REPOSITORY PATTERNS
===================

**Repository patterns** accepted by **repose** are a variation on repoq(1) repository patterns:
 
 |
 | [**P**][:[**V**][:[**A**][:[**T**]]]]

 Empty segments are treated as wildcards or default to information gleaned from */etc/products.d/baseproduct*, depending on context. Some useful variants:

 |
 | ·  **P**:**V**:**A**:**T**
 | ·  **P**:**V**:**A**
 | ·  **P**:**V**
 | ·  **P**:**V**::**T**
 | ·  **P**:::**T**
 | ·  :::**T**

SEE ALSO
========

repoq(1), scp(1), ssh(1), ssh-copy-id(1), zypper(8).

BUGS
====

No doubt plentiful. Please report them at https://github.com/openSUSE/repose/issues
