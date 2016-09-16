repose install
==============

setup::

  $ . $TESTROOT/setup

  $ fake-refhost fubar.example.org x86_64 sles:12 --
  $ fake-refhost snafu.example.org x86_64 sles:12 --

  $ fake-refhost rofl.example.org x86_64 sles:12 sle-we:12 --
  $ fake-refhost lmao.example.org x86_64 sles:12 --

test::

  $ repose install -n {snafu,fubar}.example.org -- sle-module-legacy:12 sle-sdk:12
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkn sle-module-legacy:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Legacy/12/x86_64/product/ sle-module-legacy:12::gm
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgknf sle-module-legacy:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Legacy/12/x86_64/update/ sle-module-legacy:12::up
  ssh -n -o BatchMode=yes snafu.example.org zypper -n --gpg-auto-import-keys in -l sle-module-legacy-release
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  ssh -n -o BatchMode=yes snafu.example.org zypper -n ar -cgknf sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  ssh -n -o BatchMode=yes snafu.example.org zypper -n --gpg-auto-import-keys in -l sle-sdk-release
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkn sle-module-legacy:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Legacy/12/x86_64/product/ sle-module-legacy:12::gm
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgknf sle-module-legacy:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Legacy/12/x86_64/update/ sle-module-legacy:12::up
  ssh -n -o BatchMode=yes fubar.example.org zypper -n --gpg-auto-import-keys in -l sle-module-legacy-release
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  ssh -n -o BatchMode=yes fubar.example.org zypper -n ar -cgknf sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  ssh -n -o BatchMode=yes fubar.example.org zypper -n --gpg-auto-import-keys in -l sle-sdk-release

FIXME: installs already present products::

  $ repose install -n {rofl,lmao}.example.org -- sle-we
  ssh -n -o BatchMode=yes rofl.example.org zypper -n ar -cgkn sle-we:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/ sle-we:12::gm
  ssh -n -o BatchMode=yes rofl.example.org zypper -n ar -cgknf sle-we:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/ sle-we:12::up
  ssh -n -o BatchMode=yes rofl.example.org zypper -n --gpg-auto-import-keys in -l sle-we-release
  ssh -n -o BatchMode=yes lmao.example.org zypper -n ar -cgkn sle-we:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-WE/12/x86_64/product/ sle-we:12::gm
  ssh -n -o BatchMode=yes lmao.example.org zypper -n ar -cgknf sle-we:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-WE/12/x86_64/update/ sle-we:12::up
  ssh -n -o BatchMode=yes lmao.example.org zypper -n --gpg-auto-import-keys in -l sle-we-release
