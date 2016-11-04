repose reset
============

setup::

  $ . $TESTROOT/setup

  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost empty.example.org x86_64 sles:12

  $ fake-refhost extras.example.org x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::gm \
  >   sle-we:12::{gm,up}

  $ fake-refhost mixup.example.org x86_64 \
  >   sled:12 \
  >   -- \
  >   sled:12::gm \
  >   sle-sdk:12::gm

test::

  $ repose reset -n empty.example.org
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no empty.example.org zypper -n ar -cgkn sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ sles:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no empty.example.org zypper -n --gpg-auto-import-keys refresh sles:12::gm > /dev/null
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no empty.example.org zypper -n ar -cfgkn sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ sles:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no empty.example.org zypper -n --gpg-auto-import-keys refresh sles:12::up > /dev/null
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no empty.example.org zypper -n ar -cfgkn sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/ sles:12::lt
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no empty.example.org zypper -n --gpg-auto-import-keys refresh sles:12::lt > /dev/null

  $ repose reset -n extras.example.org
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no extras.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no extras.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no extras.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no extras.example.org zypper -n ar -cfgkn sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/ sles:12::lt
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no extras.example.org zypper -n --gpg-auto-import-keys refresh sles:12::lt > /dev/null

  $ repose reset -n mixup.example.org
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no mixup.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no mixup.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no mixup.example.org zypper -n --gpg-auto-import-keys refresh sled:12::up > /dev/null
