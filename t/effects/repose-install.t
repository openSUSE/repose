repose install
==============

setup::

  $ . $TESTROOT/setup

  $ fake-refhost fubar.example.org x86_64 sles:12 --
  $ fake-refhost snafu.example.org x86_64 sles:12 --


test default behavior (adds default repos)::

  $ repose install -n fubar.example.org snafu.example.org -- sle-module-legacy:12 sle-sdk:12
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sle-module-legacy:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Legacy/12/x86_64/product/ sle-module-legacy:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sle-module-legacy:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Legacy/12/x86_64/update/ sle-module-legacy:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sle-module-legacy-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sle-sdk-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sle-module-legacy:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Legacy/12/x86_64/product/ sle-module-legacy:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sle-module-legacy:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Legacy/12/x86_64/update/ sle-module-legacy:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sle-module-legacy-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sle-sdk-release


test adds only requested repos::

  $ repose install -n fubar.example.org snafu.example.org -- sled:12::{up,du}
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sled-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update_debug/ sled:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sled-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sled-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sled:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update_debug/ sled:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sled-release


test that `-t/--tag` provides defaults to tagless patterns::

  $ repose install -n -t gm -t dg fubar.example.org snafu.example.org -- sle-we:12 sled:12::gm,up,nv
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sle-we:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/ sle-we:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sle-we:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product_debug/ sle-we:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sle-we-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::nv http://download.nvidia.com/novell/sle12/ sled:12::nv
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sled-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sle-we:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/ sle-we:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sle-we:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product_debug/ sle-we:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sle-we-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sled:12::nv http://download.nvidia.com/novell/sle12/ sled:12::nv
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sled-release


test that tag negation means "all tags but these"::

  $ repose install -n {snafu,fubar}.example.org -- sled:12::~nv
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sled:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product_debug/ sled:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sled:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update_debug/ sled:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sled-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sled:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product_debug/ sled:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update_debug/ sled:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sled-release

  $ repose install -n fubar.example.org -- sled:12::~.
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sled:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/ sled:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/ sled:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sled:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product_debug/ sled:12::dg
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update_debug/ sled:12::du
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sled:12::nv http://download.nvidia.com/novell/sle12/ sled:12::nv
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sled-release

test::

  $ repose install -n {snafu,fubar}.example.org -- sle-module-legacy:12 sle-sdk:12
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sle-module-legacy:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Legacy/12/x86_64/product/ sle-module-legacy:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sle-module-legacy:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Legacy/12/x86_64/update/ sle-module-legacy:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sle-module-legacy-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sle-sdk-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sle-module-legacy:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Legacy/12/x86_64/product/ sle-module-legacy:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sle-module-legacy:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Legacy/12/x86_64/update/ sle-module-legacy:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sle-module-legacy-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sle-sdk-release


FIXME: installs already present products::

  $ repose install -n {snafu,fubar}.example.org -- sle-we
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cgkn sle-we:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/ sle-we:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n ar -cfgkn sle-we:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/ sle-we:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no snafu.example.org zypper -n --gpg-auto-import-keys in -l sle-we-release
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sle-we:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/ sle-we:12::gm
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cfgkn sle-we:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/ sle-we:12::up
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n --gpg-auto-import-keys in -l sle-we-release
