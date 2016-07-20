tag handling
============

setup::

  $ . $TESTROOT/setup

4th segment gives tags to filter::

  $ repoq sles:12:x86_64:gm
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/

option -t TAG provides default value for three-segment SPECs::

  $ repoq -t gm sles:12:x86_64
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/

options -t TAG accumulate::

  $ repoq -t gm -t at -t nv sles:12:x86_64 sled:12.1:x86_64
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product/
  http://download.nvidia.com/novell/sle12sp1/
  http://www2.ati.com/suse/sle12sp1/

the 4th segment is a comma-delimited list of tags::

  $ repoq sles:12:x86_64:gm sled:12.1:x86_64:at,nv
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  http://download.nvidia.com/novell/sle12sp1/
  http://www2.ati.com/suse/sle12sp1/

  $ repoq -a x86_64 sles:12::gm sled:12.1::at,nv
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  http://download.nvidia.com/novell/sle12sp1/
  http://www2.ati.com/suse/sle12sp1/

complementary sets (negation)::

  $ repoq sles:12:x86_64:\^gm sled:12.1:x86_64:\^at,nv
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/
  http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12-SP1/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12-SP1/x86_64/update_debug/

  $ repoq sles:12:x86_64:~gm sled:12.1:x86_64:~at,nv
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update_debug/
  http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12-SP1/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12-SP1/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12-SP1/x86_64/update_debug/
