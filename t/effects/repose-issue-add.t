repose issue-add
================

setup::

  $ . $TESTROOT/setup

  $ fake-refhost fubar.example.org x86_64 sles:12 --
  $ cp -r $FIXTURES/SUSE:Maintenance:1085:89320 .

test::

  $ repose issue-add -n fubar.example.org -- SUSE:Maintenance:1085:89320
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sles:12::p=1085 http://dl.example.org/ibs/SUSE:/Maintenance:/1085/SUSE_Updates_SLE-SERVER_12_x86_64/ sles:12::p=1085
