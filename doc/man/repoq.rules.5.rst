.. vim: ft=rst sw=2 sts=2 et

================
 **repoq.rules**
================

-----------------------------
Repoq repository table format
-----------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-2.0
:Version: 0.28
:Manual section: 1
:Manual group: BSD General Commands Manual


SYNOPSIS
========

*/usr/local/etc/repose/repoq.rules*

DESCRIPTION
===========

repoq(1) reads configuration data from */usr/local/etc/repose/repoq.rules* (or the file specified with **--file** on the command line). The file is lexed (tokenized) using the rules of Zsh (see “Z:opts:” in zshexpn(1), Parameter Expansion Flags); comments are ignored.

The result is a sequence of mappings from product name patterns to repository definitions. No indentation is allowed on product name pattern lines, repository definitions must be indented using two spaces. Each mapping consists of a line containing a product name pattern, followed by one or more repository definition lines.

The following variables can be used in repository URLs. See repoq(1) for description of their respective domains.

|
| **P**   Product name
| **V**   Version string (format: *M*-SP\ *m*)
| **v**   Version string (format: *M*.\ *m*)
| **A**   Architecture name

EXAMPLES
========

| sapaio\:12
|   gm http\://dl.example.org/SUSE/Products/SLE-SAP/$V/$A/product/
|   up http\://dl.example.org/SUSE/Updates/SLE-SAP/$V/$A/update/

| ses # SUSE Enterprise Storage
|   gm http\://dl.example.org/SUSE/Products/Storage/$V/$A/product/
|   up http\://dl.example.org/SUSE/Updates/Storage/$V/$A/update/

| sle-module-toolchain\:12
|   gm http\://dl.example.org/SUSE/Products/${${(C)P}/#Sle-/SLE-}/12/$A/product/
|   up http\://dl.example.org/SUSE/Updates/${${(C)P}/#Sle-/SLE-}/12/$A/update/

| sled\:12
|   gm http\://dl.example.org/SUSE/Products/SLE-DESKTOP/$V/$A/product/
|   up http\://dl.example.org/SUSE/Updates/SLE-DESKTOP/$V/$A/update/
|   nv http\://download.nvidia.com/novell/sle${(L)V/-}/
|   at http\://www2.ati.com/suse/sle${(L)V/-}/

SEE ALSO
========

repoq(1), repose(1), zshexpn(1).

REPOSE
======

**repoq.rules** is part of repose(1).
