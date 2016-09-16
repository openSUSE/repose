full spec
=========

setup::

  $ . $TESTROOT/setup


request matches no definition::

  $ repoq notthere:69:omg
  repoq: no rule matches 'notthere:69:omg'
  [1]


happy path::

  $ repoq sle-sdk:12:x86_64
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq sles:11.3:s390x sle-sdk:11.3:s390x
  sles:11.3::gm http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-SERVER/11-SP3-POOL/
  sles:11.3::up http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SP3/s390x/update/
  sles:11.3::se http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SECURITY/s390x/update/
  sles:11.3::lt http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SP3-LTSS/s390x/update/
  sles:11.3::dg http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-DEBUGINFO/11-SP3-POOL/
  sles:11.3::du http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-DEBUGINFO/11-SP3/
  sle-sdk:11.3::gm http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-SDK/11-SP3-POOL/
  sle-sdk:11.3::up http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SDK/11-SP3/s390x/update/

  $ repoq sles:12:s390x sle-sdk:12:x86_64
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product/
  sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update/
  sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product_debug/
  sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update_debug/
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update_debug/

  $ repoq sles:12:s390x sle-sdk:12.1:x86_64
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product/
  sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update/
  sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/s390x/product_debug/
  sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/s390x/update_debug/
  sle-sdk:12.1::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12-SP1/x86_64/product/
  sle-sdk:12.1::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12-SP1/x86_64/update/
  sle-sdk:12.1::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12-SP1/x86_64/product_debug/
  sle-sdk:12.1::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12-SP1/x86_64/update_debug/

  $ repoq sles:11.4:s390x sle-sdk:12:ppc64le
  sles:11.4::gm http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-SERVER/11-SP4-POOL/
  sles:11.4::up http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SP4/s390x/update/
  sles:11.4::se http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-SERVER/11-SECURITY/s390x/update/
  sles:11.4::dg http://dl.example.org/update/zypp-patches.suse.de/s390x/update/SLE-DEBUGINFO/11-SP4-POOL/
  sles:11.4::du http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SLE-DEBUGINFO/11-SP4/
  sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/ppc64le/product/
  sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/ppc64le/update/
  sle-sdk:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/ppc64le/product_debug/
  sle-sdk:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/ppc64le/update_debug/

mixed results (*currently*, repoq does a single pass over its operands)::

  $ repoq sles:12:x86_64 notthere:69:omg
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/
  sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/
  repoq: no rule matches 'notthere:69:omg'
  [1]

  $ repoq notthere:69:omg sles:12:x86_64
  repoq: no rule matches 'notthere:69:omg'
  [1]

  $ repoq sles:12:x86_64 notthere:69:omg sled:12:x86_64
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/
  sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/
  repoq: no rule matches 'notthere:69:omg'
  [1]
