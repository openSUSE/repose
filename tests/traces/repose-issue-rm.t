repose issue-rm
===============

setup::

  $ . $TESTROOT/setup

  $ repose_chatty+=('*')

  $ fake-refhost fubar.example.org x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::p=1085

test::

  $ repose issue-rm -n fubar.example.org -- SUSE:Maintenance:1085
  O find-cmd issue-rm
  o run-cmd */repose-issue-rm -n fubar.example.org -- SUSE:Maintenance:1085 (glob)
  o rh-list-repos fubar.example.org
  o redir -1 * rh-fetch-repos fubar.example.org (glob)
  o rh-fetch-repos fubar.example.org
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (esc) (glob)
  o rm -f * (glob)
  o print ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org 'zypper -n rr http://dl.example.org/ibs/SUSE:/Maintenance:/1085/SUSE_Updates_SLE-SERVER_12_x86_64/'
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE:/Maintenance:/1085/SUSE_Updates_SLE-SERVER_12_x86_64/

test verbose::

  $ repose issue-rm -n -v fubar.example.org -- SUSE:Maintenance:1085
  O find-cmd issue-rm
  o run-cmd */repose-issue-rm -n -v fubar.example.org -- SUSE:Maintenance:1085 (glob)
  o rh-list-repos fubar.example.org
  o redir -1 * rh-fetch-repos fubar.example.org (glob)
  o rh-fetch-repos fubar.example.org
  o ssh -n -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (esc) (glob)
  o rm -f * (glob)
  o print ssh -n -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org 'zypper -n rr http://dl.example.org/ibs/SUSE:/Maintenance:/1085/SUSE_Updates_SLE-SERVER_12_x86_64/'
  ssh -n -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE:/Maintenance:/1085/SUSE_Updates_SLE-SERVER_12_x86_64/
