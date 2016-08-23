full spec
=========

setup::

  $ . $TESTROOT/setup


request matches no definition::

  $ repoq --no-name notthere:69:omg
  repoq: no rule matches 'notthere:69:omg'
  [1]

  $ repoq -N notthere:69:omg
  repoq: no rule matches 'notthere:69:omg'
  [1]


happy path::

  $ repoq --no-name sle-sdk:12:x86_64
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq -N sles:11.3:s390x sle-sdk:11.3:s390x
  http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-SERVER/11-SP3-POOL/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SP3/s390x/update/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SECURITY/s390x/update/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SP3-LTSS/s390x/update/
  http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-DEBUGINFO/11-SP3-POOL/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-DEBUGINFO/11-SP3/
  http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-SDK/11-SP3-POOL/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SDK/11-SP3/s390x/update/

  $ repoq --no-name sles:12:s390x sle-sdk:12:x86_64
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update_debug/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq -N sles:12:s390x sle-sdk:12.1:x86_64
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update_debug/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12-SP1/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12-SP1/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12-SP1/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12-SP1/x86_64/update_debug/

  $ repoq --no-name sles:11.4:s390x sle-sdk:12:ppc64le
  http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-SERVER/11-SP4-POOL/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SP4/s390x/update/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SECURITY/s390x/update/
  http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-DEBUGINFO/11-SP4-POOL/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-DEBUGINFO/11-SP4/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/ppc64le/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/ppc64le/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/ppc64le/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/ppc64le/update_debug/

mixed results (*currently*, repoq does a single pass over its operands)::

  $ repoq -N sles:12:x86_64 notthere:69:omg
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/
  repoq: no rule matches 'notthere:69:omg'
  [1]

  $ repoq --no-name sles:12:x86_64 notthere:69:omg sled:12:x86_64
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/
  repoq: no rule matches 'notthere:69:omg'
  [1]
