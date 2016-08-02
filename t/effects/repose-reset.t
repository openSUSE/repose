repose reset
============

setup::

  $ . $TESTROOT/setup

  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost omg.example.org x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::gm \
  >   sle-we:12::{gm,up}

  $ fake-refhost wtf.example.org x86_64 \
  >   sled:12 \
  >   -- \
  >   sled:12::gm \
  >   sle-sdk:12::gm

test::

  $ repose reset -n omg.example.org
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/

  $ repose reset -n wtf.example.org
  ssh -n -o BatchMode=yes wtf.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes wtf.example.org zypper -n ar -cgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
