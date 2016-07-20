.. vim: ft=rst sw=2 sts=2 et tw=72

########################################################################
                              REPOSE/REPOQ
########################################################################
========================================================================
                      Comparison to `rep-clean.sh`
========================================================================


Add Repositories For Installed Products
=======================================

::

  $ ssh <HOST> rep-clean.sh -a
  $ repose update <HOST>


Remove Repositories
===================

common functionality::

  $ ssh <HOST> rep-clean.sh -r
  $ repose clear <HOST>
  $ repose remove <HOST> -- :

repose extras::

  $ repose remove <HOST> -- :::gm

  $ repose remove <HOST> -- ::i386

  $ repose remove <HOST> -- :11.3::at,nv,sc


Replace Installed Repositories
==============================

::

  $ ssh <HOST> rep-clean.sh -f
  $ repose reset <HOST>


Install Addons, Add Repositories
================================

::

  $ ssh <HOST> rep-clean.sh -A <ADDON>
  $ repose install <HOST> -- <ADDON>


Add Repositories For a Maintenance Update
=========================================

::

  $ ssh <HOST> rep-clean.sh -i <ISSUE>
  $ repose issue-add <HOST> -- <ISSUE>


Remove Repositories For a Maintenance Update
============================================

::

  $ ssh <HOST> rep-clean.sh -z
  $ repose issue-rm <HOST> -- <ISSUE>


Paths Less Traveled
===================

disable update and testing repositories::

  $ ssh <HOST> rep-clean.sh -g
  $ repose switch-to <HOST> -- :::^up,ts

disable update repositories::

  $ ssh <HOST> rep-clean.sh -t
  $ repose switch-to <HOST> -- :::^up

disable testing repositories::

  $ ssh <HOST> rep-clean.sh -n
  $ repose switch-to <HOST> -- :::^ts

ignore QAM-specific repos::

  $ ssh <HOST> rep-clean.sh -p ...
  $ repose ... <HOST> -- :::^qa,ts

