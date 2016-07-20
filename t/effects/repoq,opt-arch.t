option: --arch
==============

setup::

  $ . $TESTROOT/setup


test::

  $ repoq -a s390x sle-sdk:12
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update_debug/

  $ repoq -a s390x sle-sdk:12:
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update_debug/

  $ repoq -a s390x sle-sdk:12::
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/s390x/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/s390x/update_debug/

  $ repoq -a s390x sle-sdk:12:x86_64
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq -a s390x sle-sdk:12:x86_64:
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq -a s390x sle-sdk:12:x86_64 sles:12
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update_debug/
