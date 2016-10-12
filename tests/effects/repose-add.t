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


test default behavior (adds default repos)::

  $ repose add -n fubar.example.org snafu.example.org -- sles:12
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ sles:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ sles:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/ sles:12::lt
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ sles:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ sles:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/ sles:12::lt


test adds only requested repos::

  $ repose add -n fubar.example.org snafu.example.org -- sles:12::{up,du}
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ sles:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/ sles:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ sles:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/ sles:12::du


test that `-t/--tag` provides defaults to tagless patterns::

  $ repose add -n -t gm -t dg fubar.example.org snafu.example.org -- sles:12 sled:12::at
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ sles:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/ sles:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::at http://www2.ati.com/suse/sle12/ sled:12::at
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ sles:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/ sles:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sled:12::at http://www2.ati.com/suse/sle12/ sled:12::at


test that tag negation means "all tags but these"::

  $ repose add -n fubar.example.org snafu.example.org -- sles:12::~gm,up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/ sles:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/ sles:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/ sles:12::lt
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::dl http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update_debug/ sles:12::dl
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/ sles:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/ sles:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/ sles:12::lt
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sles:12::dl http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update_debug/ sles:12::dl

  $ repose add -n fubar.example.org -- sles:12::~.
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/ sles:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/ sles:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/ sles:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/ sles:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/ sles:12::lt
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sles:12::dl http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update_debug/ sles:12::dl


no attempt to skip already-present repos::

  $ repose add -n fubar.example.org snafu.example.org -- sled:12
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
