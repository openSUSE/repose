repose add
==========

setup::

  $ . $TESTROOT/setup

  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost fubar.example.org x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::{gm,up}

  $ fake-refhost snafu.example.org x86_64 \
  >   sled:12 \
  >   -- \
  >   sled:12::{gm,up,nv} \
  >   sle-sdk:12:::gm


no attempt to skip already-present repos::

  $ noglob repose add -n fubar.example.org snafu.example.org -- sles:12 sled:12::^nv,at sle-sdk
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkn sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ sles:12::gm
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkfn sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ sles:12::up
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkfn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkfn sled:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product_debug/ sled:12::dg
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkfn sled:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update_debug/ sled:12::du
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkfn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkn sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ sles:12::gm
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkfn sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ sles:12::up
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkfn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkfn sled:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product_debug/ sled:12::dg
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkfn sled:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update_debug/ sled:12::du
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkfn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
