repose remove
=============

setup::

  $ . $TESTROOT/setup

  $ repose_chatty+=('*')

  $ fake-refhost \$local x86_64 \
  >   sled:12 \
  >   sle-sdk:12 \
  >   -- \
  >   sled:12::{gm,up,nv} \
  >   sle-sdk:12::{gm,up}

test::

  $ repose remove -n . -- '*'
  O find-cmd remove
  o run-cmd */repose-remove -n . -- '*' (glob)
  o rh-list-repos .
  o redir -1 * rh-fetch-repos . (glob)
  o rh-fetch-repos .
  o zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  o do-remove . sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ '*'
  o print zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/
  o do-remove . sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ '*'
  o print zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  o do-remove . sled:12::nv http://download.nvidia.com/novell/sle12/ '*'
  o print zypper -n rr http://download.nvidia.com/novell/sle12/
  zypper -n rr http://download.nvidia.com/novell/sle12/
  o do-remove . sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ '*'
  o print zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  o do-remove . sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ '*'
  o print zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ repose remove -n . -- sle-sdk
  O find-cmd remove
  o run-cmd */repose-remove -n . -- sle-sdk (glob)
  o rh-list-repos .
  o redir -1 * rh-fetch-repos . (glob)
  o rh-fetch-repos .
  o zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  o do-remove . sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ 'sle-sdk:*:*:(*)'
  o do-remove . sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ 'sle-sdk:*:*:(*)'
  o do-remove . sled:12::nv http://download.nvidia.com/novell/sle12/ 'sle-sdk:*:*:(*)'
  o do-remove . sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ 'sle-sdk:*:*:(*)'
  o print zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  o do-remove . sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ 'sle-sdk:*:*:(*)'
  o print zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
