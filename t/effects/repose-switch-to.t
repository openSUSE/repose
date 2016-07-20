repose list
===========

setup::

  $ . $TESTROOT/setup

  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost rofl.example.org x86_64 \
  >   sled:12 \
  >   sle-module-legacy:12 \
  >   -- \
  >   sled:12::{gm,up} \
  >   sle-module-legacy:12::{gm,up}

  $ fake-refhost lmao.example.org x86_64 \
  >   sles:12 \
  >   sle-sdk:12 \
  >   -- \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::{gm,up}

test::

  $ repose switch-to -n rofl.example.org lmao.example.org -- :::gm
  ssh -n -o BatchMode=yes rofl.example.org zypper -n mr -e http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/
  ssh -n -o BatchMode=yes rofl.example.org zypper -n mr -d http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  ssh -n -o BatchMode=yes rofl.example.org zypper -n mr -e http://dl.example.org/ibs/SUSE/Products/SLE-Module-Legacy/12/x86_64/product/
  ssh -n -o BatchMode=yes rofl.example.org zypper -n mr -d http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Legacy/12/x86_64/update/
  ssh -n -o BatchMode=yes lmao.example.org zypper -n mr -e http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  ssh -n -o BatchMode=yes lmao.example.org zypper -n mr -d http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  ssh -n -o BatchMode=yes lmao.example.org zypper -n mr -e http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes lmao.example.org zypper -n mr -d http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
