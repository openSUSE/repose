repose prune
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
  >   sled:12::{gm,up} \
  >   sle-sdk:12::gm

test::

  $ repose prune -n omg.example.org wtf.example.org -- :

  $ repose prune -n omg.example.org wtf.example.org -- \*

  $ repose prune -n omg.example.org wtf.example.org -- notthere
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/
  ssh -n -o BatchMode=yes wtf.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/

  $ repose prune -n omg.example.org wtf.example.org -- \*we
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes wtf.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/

  $ repose prune -n omg.example.org wtf.example.org -- :::gm
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/

  $ repose prune -n omg.example.org wtf.example.org
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/
  ssh -n -o BatchMode=yes wtf.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/

  $ repose prune -n omg.example.org wtf.example.org -- sle-we
  ssh -n -o BatchMode=yes omg.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes wtf.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
