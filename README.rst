.. vim: ft=rst sw=2 sts=2 et tw=72

########################################################################
                              REPOSE/REPOQ
########################################################################
========================================================================
                Manipulate repositories in QAM refhosts
========================================================================
.. image:: https://travis-ci.org/openSUSE/repose.svg?branch=master
    :target: https://travis-ci.org/openSUSE/repose

.. image:: https://coveralls.io/repos/github/openSUSE/repose/badge.svg?branch=master
    :target: https://coveralls.io/github/openSUSE/repose?branch=master

Big Picture
===========

*Repose/Repoq* is a pair of tools for querying and manipulation of
repositories in SUSE QA Maintenance reference machines.

*Repose* allows for manipulation of repositories in refhosts requiring
only a running sshd and zypper installed on them.

.. note:: Picture of the big picture:

  ::

        #=============#              #=============#
        #   refhost   #  1. query    #   repose    #
        #     <-------------------------------+    #
        #     |       #              #      #=============#
        #     |       #  2. product info    #      "      #
        #     +------------------------------->  repoq    #
        #             #              #      # |   or      #
        #             #  3. zypper commands # |  sumaxy   #
        #     <-------------------------------+    "      #
        #             #              #      #=============#
        #=============#              #=============#



Repoq
=====

*Repoq* maps strings like `sle-sdk:12.1:x86_64` to the corresponding
repository URLs, or to suitable `zypper addrepo` (`zypper removerepo`)
commands.
*Repoq* performs this mapping based on information collected from a file
it downloads from a configured location.

Repose
======

*Repose* reports or modifies the package repositories in one or more
refhosts based on installed products (/etc/products.d/), repository
configuration (/etc/zypp/repos.d), and user input; commands are sent via
ssh.
*Repose* uses *Repoq* and *sumaxy* internally.
