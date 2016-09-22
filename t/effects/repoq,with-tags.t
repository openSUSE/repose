tag handling
============

setup::

  $ . $TESTROOT/setup

4th segment gives tags to filter::

  $ repoq sles:12:x86_64:gm
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/

option -t TAG provides default value for three-segment SPECs::

  $ repoq -t gm sles:12:x86_64
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/

options -t TAG accumulate::

  $ repoq -t gm -t at -t nv sles:12:x86_64 sled:12.1:x86_64
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  sled:12.1::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product/
  sled:12.1::nv http://download.nvidia.com/novell/sle12sp1/
  sled:12.1::at http://www2.ati.com/suse/sle12sp1/

the 4th segment is a comma-delimited list of tags::

  $ repoq sles:12:x86_64:gm sled:12.1:x86_64:at,nv
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  sled:12.1::nv http://download.nvidia.com/novell/sle12sp1/
  sled:12.1::at http://www2.ati.com/suse/sle12sp1/

  $ repoq -a x86_64 sles:12::gm sled:12.1::at,nv
  sles:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  sled:12.1::nv http://download.nvidia.com/novell/sle12sp1/
  sled:12.1::at http://www2.ati.com/suse/sle12sp1/

complementary sets (negation)::

  $ repoq sles:12:x86_64:\^gm sled:12.1:x86_64:\^at,nv
  sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/
  sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/
  sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/
  sles:12::dl http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update_debug/
  sled:12.1::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product/
  sled:12.1::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12-SP1/x86_64/update/
  sled:12.1::dg http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product_debug/
  sled:12.1::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12-SP1/x86_64/update_debug/

  $ repoq sles:12:x86_64:~gm sled:12.1:x86_64:~at,nv
  sles:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  sles:12::dg http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/
  sles:12::du http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/
  sles:12::lt http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update/
  sles:12::dl http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12-LTSS/x86_64/update_debug/
  sled:12.1::gm http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product/
  sled:12.1::up http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12-SP1/x86_64/update/
  sled:12.1::dg http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product_debug/
  sled:12.1::du http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12-SP1/x86_64/update_debug/
