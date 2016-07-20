option: -R,--rr
===============

setup::

  $ . $TESTROOT/setup


test::

  $ repoq --rr sle-sdk:12:x86_64
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq --rr sle-live-patching:12:x86_64 sle-sdk:12:x86_64
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-Live-Patching/12/x86_64/product_debug/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-Live-Patching/12/x86_64/update_debug/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/
