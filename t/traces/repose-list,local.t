repose list
===========

setup::

  $ . $TESTROOT/setup

  $ repose_chatty=('*')

  $ fake-refhost \$local x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::{gm,up}

test::

  $ repose list .
  O find-cmd list
  o run-cmd */repose-list . (glob)
  o rh-list-repos .
  o redir -1 * rh-fetch-repos . (glob)
  o rh-fetch-repos .
  o zypper -x lr
  o xml-get-repos * (glob)
  o xml sel -t -m /stream/repo-list/repo -v @name -o \x01 -v url --nl * (glob)
  o rm -f * (glob)
  . http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
