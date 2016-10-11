repose clear
============

setup::

  $ . $TESTROOT/setup

  $ repose_chatty+=('*')
  $ repose_dryrun=(${repose_dryrun:#ssh%*} "ssh%*%zypper%-n%rr%*")

  $ fake-refhost fubar.example.org x86_64 \
  >   sles:12 \
  >   sle-sdk:12 \
  >   -- \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::{gm,up}

test::

  $ repose clear fubar.example.org
  O find-cmd clear
  o run-cmd */repose-clear fubar.example.org (glob)
  o rh-list-repos fubar.example.org
  o redir -1 * rh-fetch-repos fubar.example.org (glob)
  o rh-fetch-repos fubar.example.org
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
