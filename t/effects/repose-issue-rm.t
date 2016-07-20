repose issue-rm
===============

setup::

  $ . $TESTROOT/setup

  $ exit 80

  $ fake-refhost fubar.example.org x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::p=1085

test::

  $ repose issue-rm -n fubar.example.org -- SUSE:Maintenance:1085
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE:/Maintenance:/1085/SUSE_Updates_SLE-SERVER_12_x86_64/
