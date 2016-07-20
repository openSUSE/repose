repose remove
=============

setup::

  $ . $TESTROOT/setup

  $ repose_chatty+=('*')
  $ repose_dryrun=(${repose_dryrun:#ssh%*} "ssh%*%zypper* rr*")

  $ fake-refhost fubar.example.org x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::{gm,up}

  $ fake-refhost snafu.example.org x86_64 \
  >   sled:12 \
  >   sle-sdk:12 \
  >   -- \
  >   sled:12::{gm,up,nv} \
  >   sle-sdk:12::gm

test::

  $ repose remove -n fubar.example.org snafu.example.org -- '*'
  O find-cmd remove
  o run-cmd */repose-remove -n fubar.example.org snafu.example.org -- '*' (glob)
  o rh-list-repos fubar.example.org
  o redir -1 * rh-fetch-repos fubar.example.org (glob)
  o rh-fetch-repos fubar.example.org
  o ssh -n -o BatchMode=yes fubar.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  o do-remove fubar.example.org sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ '*'
  o print ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  o do-remove fubar.example.org sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ '*'
  o print ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  o do-remove fubar.example.org sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ '*'
  o print ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  o do-remove fubar.example.org sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ '*'
  o print ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  o rh-list-repos snafu.example.org
  o redir -1 * rh-fetch-repos snafu.example.org (glob)
  o rh-fetch-repos snafu.example.org
  o ssh -n -o BatchMode=yes snafu.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  o do-remove snafu.example.org sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ '*'
  o print ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/
  o do-remove snafu.example.org sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ '*'
  o print ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  o do-remove snafu.example.org sled:12::nv http://download.nvidia.com/novell/sle12/ '*'
  o print ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://download.nvidia.com/novell/sle12/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://download.nvidia.com/novell/sle12/
  o do-remove snafu.example.org sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ '*'
  o print ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/

  $ repose remove -n fubar.example.org snafu.example.org -- sle-sdk
  O find-cmd remove
  o run-cmd */repose-remove -n fubar.example.org snafu.example.org -- sle-sdk (glob)
  o rh-list-repos fubar.example.org
  o redir -1 * rh-fetch-repos fubar.example.org (glob)
  o rh-fetch-repos fubar.example.org
  o ssh -n -o BatchMode=yes fubar.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  o do-remove fubar.example.org sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ 'sle-sdk:*:*:(*)'
  o do-remove fubar.example.org sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ 'sle-sdk:*:*:(*)'
  o do-remove fubar.example.org sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ 'sle-sdk:*:*:(*)'
  o print ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  o do-remove fubar.example.org sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ 'sle-sdk:*:*:(*)'
  o print ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  o rh-list-repos snafu.example.org
  o redir -1 * rh-fetch-repos snafu.example.org (glob)
  o rh-fetch-repos snafu.example.org
  o ssh -n -o BatchMode=yes snafu.example.org zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  o do-remove snafu.example.org sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ 'sle-sdk:*:*:(*)'
  o do-remove snafu.example.org sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ 'sle-sdk:*:*:(*)'
  o do-remove snafu.example.org sled:12::nv http://download.nvidia.com/novell/sle12/ 'sle-sdk:*:*:(*)'
  o do-remove snafu.example.org sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ 'sle-sdk:*:*:(*)'
  o print ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
