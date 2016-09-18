option: -A,--ar
===============

setup::

  $ . $TESTROOT/setup


test::

  $ repoq --ar sle-sdk:12:x86_64
  sle-sdk:12::gm zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  sle-sdk:12::up zypper -n ar -cgkfn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  sle-sdk:12::dg zypper -n ar -cgkfn sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/ sle-sdk:12::dg
  sle-sdk:12::du zypper -n ar -cgkfn sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/ sle-sdk:12::du

  $ repoq --ar sle-live-patching:12:x86_64 sle-sdk:12:x86_64
  sle-live-patching:12::gm zypper -n ar -cgkn sle-live-patching:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product/ sle-live-patching:12::gm
  sle-live-patching:12::up zypper -n ar -cgkfn sle-live-patching:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update/ sle-live-patching:12::up
  sle-live-patching:12::dg zypper -n ar -cgkfn sle-live-patching:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product_debug/ sle-live-patching:12::dg
  sle-live-patching:12::du zypper -n ar -cgkfn sle-live-patching:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update_debug/ sle-live-patching:12::du
  sle-sdk:12::gm zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  sle-sdk:12::up zypper -n ar -cgkfn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  sle-sdk:12::dg zypper -n ar -cgkfn sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/ sle-sdk:12::dg
  sle-sdk:12::du zypper -n ar -cgkfn sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/ sle-sdk:12::du

  $ repoq --no-name --ar sle-sdk:12:x86_64
  zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  zypper -n ar -cgkfn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  zypper -n ar -cgkfn sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/ sle-sdk:12::dg
  zypper -n ar -cgkfn sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/ sle-sdk:12::du

  $ repoq --no-name --ar sle-live-patching:12:x86_64 sle-sdk:12:x86_64
  zypper -n ar -cgkn sle-live-patching:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product/ sle-live-patching:12::gm
  zypper -n ar -cgkfn sle-live-patching:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update/ sle-live-patching:12::up
  zypper -n ar -cgkfn sle-live-patching:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product_debug/ sle-live-patching:12::dg
  zypper -n ar -cgkfn sle-live-patching:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update_debug/ sle-live-patching:12::du
  zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm
  zypper -n ar -cgkfn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up
  zypper -n ar -cgkfn sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/ sle-sdk:12::dg
  zypper -n ar -cgkfn sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/ sle-sdk:12::du
