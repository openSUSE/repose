.. vim: ft=rst sw=2 sts=2 et tw=72

########################################################################
                              REPOSE/REPOQ
########################################################################
========================================================================
                             Hacking  Guide
========================================================================

Design and implementation of *repose* and its companions are based on
these factors:

* the primary user is SUSE QA Maintenance (QAM)
* QAM people run mostly openSUSE and SUSE products in recent versions
* QAM manages a crazy spectrum of product/version combinations
* there are many more managed machines than managing ones
* staying out of managed machines avoids bootstrapping issues
