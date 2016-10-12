option: -R,--rr
===============

setup::

  $ . $TESTROOT/setup


test::

  $ repoq --rr sle-sdk:12:x86_64
  sle-sdk:12::gm zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  sle-sdk:12::up zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  sle-sdk:12::dg zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  sle-sdk:12::du zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq --rr sle-live-patching:12:x86_64 sle-sdk:12:x86_64
  sle-live-patching:12::gm zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product/
  sle-live-patching:12::up zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update/
  sle-live-patching:12::dg zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product_debug/
  sle-live-patching:12::du zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update_debug/
  sle-sdk:12::gm zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  sle-sdk:12::up zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  sle-sdk:12::dg zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  sle-sdk:12::du zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq --no-name --rr sle-sdk:12:x86_64
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq --no-name --rr sle-live-patching:12:x86_64 sle-sdk:12:x86_64
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product_debug/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update_debug/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/
