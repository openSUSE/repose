Special cases and other deviants
================================

setup::

  $ . $TESTROOT/setup


test::

  $ repoq -a x86_64 suse-openstack-cloud:6
  http://dl.example.org/ibs/SUSE/Products/OpenStack-Cloud/6/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/OpenStack-Cloud/6/x86_64/update/

  $ repoq -a x86_64 ses:1
  http://dl.example.org/ibs/SUSE/Products/Storage/1.0/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/Storage/1.0/x86_64/update/
  http://dl.example.org/ibs/SUSE/Products/Storage/1.0/x86_64/product_debug/
  http://dl.example.org/ibs/SUSE/Updates/Storage/1.0/x86_64/update_debug/

  $ repoq -a x86_64 suse-manager-server:2.1
  http://dl.example.org/update/zypp-patches.suse.de/x86_64/update/SUSE-MANAGER/2.1-POOL/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SUSE-MANAGER/2.1/x86_64/update/

  $ repoq -a x86_64 suse-manager-server:3.0
  http://dl.example.org/ibs/SUSE/Products/SUSE-Manager-Server/3.0/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SUSE-Manager-Server/3.0/x86_64/update/

  $ repoq -a x86_64 suse-manager-proxy:2.1
  http://dl.example.org/update/zypp-patches.suse.de/x86_64/update/SUSE-MANAGER-PROXY/2.1-POOL/
  http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SUSE-MANAGER-PROXY/2.1/x86_64/update/

  $ repoq -a x86_64 suse-manager-proxy:3.0
  http://dl.example.org/ibs/SUSE/Products/SUSE-Manager-Proxy/3.0/x86_64/product/
  http://dl.example.org/ibs/SUSE/Updates/SUSE-Manager-Proxy/3.0/x86_64/update/
