repose list
===========

setup::

  $ . $TESTROOT/setup

  $ repose_chatty=('*')
  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost fubar.example.org x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::{gm,up}

  $ fake-refhost snafu.example.org x86_64

test::

  $ repose list fubar.example.org snafu.example.org
  O find-cmd list
  o run-cmd */repose-list fubar.example.org snafu.example.org (glob)
  o rh-list-repos fubar.example.org
  o redir -1 * rh-fetch-repos fubar.example.org (glob)
  o rh-fetch-repos fubar.example.org
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  o rh-list-repos snafu.example.org
  o redir -1 * rh-fetch-repos snafu.example.org (glob)
  o rh-fetch-repos snafu.example.org
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
