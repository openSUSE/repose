option: --arch
==============

setup::

  $ . $TESTROOT/setup


test::

  $ repoq -a s390x sle-sdk:12
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update_debug/

  $ repoq -a s390x sle-sdk:12:
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update_debug/

  $ repoq -a s390x sle-sdk:12::
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update_debug/

  $ repoq -a s390x sle-sdk:12:x86_64
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq -a s390x sle-sdk:12:x86_64:
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq -a s390x sle-sdk:12:x86_64 sles:12
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product/
  sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update/
  sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product_debug/
  sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update_debug/
  sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/s390x/update/
  sles:12::dl http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/s390x/update_debug/
