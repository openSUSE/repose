.. vim: ft=rst sw=2 sts=2 et

==========
 **repoq**
==========

------------------------------------------------
Output repository information for given products
------------------------------------------------

:Author: Roman Neuhauser <rneuhauser+repose@sigpipe.cz>
:Date: Feb 04, 2016
:Copyright: GPL-2.0
:Version: 0.26
:Manual section: 1
:Manual group: User Commands

SYNOPSIS
========

**repoq** **-h** \| **--help**

**repoq** [**-F** *RULES*] [**-A** \| **-R**] [**-N**] [**-a** *ARCH*] [**-t** *TAG*]... *REPA*...

DESCRIPTION
===========

**repoq** writes to standard output information about repositories named in REPAs. By default, each line of output consist of two fields: **Repository name** and **Repository URL**, separated by a space. **-N** suppresses the first column, and **-A**, **-R** output zypper(8) commands in the second column.

OPTIONS
=======

:-h:
   Display usage instructions.

:--help:
   Display full help.

:-A, --addrepo:
   Output **zypper addrepo** commands.

:-F, --file=\ *RULES*:
   Use product/repository information from *RULES*.  This option overrides the $REPOQ_RULES environment variable.

:-N, --no-name:
   Omit the first column (repository names) from output.

:-R, --removerepo:
   Output **zypper removerepo** commands.

:-a, --arch=\ *ARCH*:
   Imply *ARCH* for *REPA*\ s without an explicit **Architecture name**.

:-t, --tag=\ *TAG*:
   Imply *TAG* for *REPA*\ s without an explicit **Tagset**.

OPERANDS
========

*REPA*
 Repository pattern, see below.

EXTENDED DESCRIPTION
====================

**Repository patterns** accepted by **repoq** use the following grammar:
  
  |
  | **P**:**V**\[:\[**A**\]\[:\[**T**\]\]\]

  where,

     |
     | **P**   is Product name
     | **V**   is Version string
     | **A**   is Architecture name
     | **T**   is Tagset

  That is, **repoq** accepts these formats:

     |
     | **P**:**V**:**A**:**T**
     | **P**:**V**:**A**
     | **P**:**V**
     | **P**:**V**::**T**

  Empty **A** requires **--arch**.  Empty **T** matches all tags (but see **--tag**).

  **Repository names** output by **repoq** use the following grammar:

      |
      | **P**:**V**::**T**

  where,

      |
      | **P**   is Product name
      | **V**   is Version string
      | **T**   is Tag

  **Product names** accepted and emitted are the variety found in */etc/products.d/\*.prod files* (XPath: */product/name/text()*). Exceptions: "SUSE_SLES", "SLES", and "sles" are equivalent; same with "SUSE_SLED", "SLED", and "sled".  **repoq** emits only the lowercase variants. **For use with openSUSE** product is "openSUSE" and "openSUSE-Addons-NonOss"

  **Version strings** use the **MAJOR**\[.\ **MINOR**\] format, where **MAJOR** is the value of */product/baseversion/text()*, and **MINOR** is the value of */product/patchlevel/text()* (omitted if it is empty or 0).

  **Architecture names** are the variety found in */etc/products.d/\*.prod* files (XPath: /product/arch/text()).

  **Tagsets** are comma-delimited lists of tags. Complement (negation) is expressed by prepending the tagset with ~ or ^ (tilde or caret).

  Empty tagset matches all tags (but see **--tag**).

  **Tags** are arbitrary words used to label repositories in repoq.rules(5).  Currently used tags are:

    |
    | at          ATI/AMD driver repository
    | dg          release repository with -debuginfo, -debugsource packages
    | du          update repository with -debuginfo, -debugsource packages
    | gm          release repository
    | nv          Nvidia driver repository
    | up          update repository
    | se          SECURITY Module repository
    | lt          LTS update repository

ENVIRONMENT
===========

*REPOQ\_RULES*
 This variable overrides the builtin default path to the configuration file, see repoq.rules(5).

*REPOQ_CHATTY*, *REPOQ_DRYRUN*
 Development and testing aids.

FILES
=====

*/usr/local/etc/repose/repoq.rules*
 See repoq.rules(5). Default location of the database used by **repoq** to map repository patterns to repository name and url pairs.

EXIT STATUS
===========

The **repoq** utility exits 0 on success, and >0 if an error occurs.

EXAMPLES
========

Show all repositories for SLE-SERVER and SLE-SDK 12-SP1 on x86\_64:

::

  $ repoq -a x86_64 sles:12.1 sle-sdk:12.1

Show product and update repositories for SLE-SERVER 12-SP1, and all but the product repository for SUSE Enterprise Storage 2.0, both on s390x:

:: 

  $ repoq -a s390x sles:12.1::gm,up ses:2.0::~gm

Show product, update repositories for SLE-SERVER 12-SP1 and SLE-DESKTOP 12, product repository for SLE-SDK 12-SP1:

::

  $ repoq -a ppc64le -t gm -t up sles:12.1 sle-sdk:12.1::gm sled:12

Show all but the GPU-specific repositories for SUSE-DESKTOP 12:

::

  $ repoq sled:12:x86_64:~at,nv

SEE ALSO
========
refdb(1), repose(1), zshexpn(1), repoq.rules(5), zypper(8).

REPOSE
======

**repoq** is part of repose(1).
