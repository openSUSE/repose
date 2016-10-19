Special cases and other deviants
================================

setup::

  $ . $TESTROOT/setup


test case-mismatch::

  $ repoq sle-module-Web-Scripting:12:x86_64:gm
  repoq: no rule matches 'sle-module-Web-Scripting:12:x86_64:gm'
  [1]

  $ repoq sle-module-web-scripting:12:x86_64:gm
  sle-module-web-scripting:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Web-Scripting/12/x86_64/product/


test special-snowflake products::

  $ repoq -a x86_64 suse-openstack-cloud:6
  suse-openstack-cloud:6::gm http://dl.example.org/ibs/SUSE/Products/OpenStack-Cloud/6/x86_64/product/
  suse-openstack-cloud:6::up http://dl.example.org/ibs/SUSE/Updates/OpenStack-Cloud/6/x86_64/update/

  $ repoq -a x86_64 ses:1
  ses:1::gm http://dl.example.org/ibs/SUSE/Products/Storage/1.0/x86_64/product/
  ses:1::up http://dl.example.org/ibs/SUSE/Updates/Storage/1.0/x86_64/update/
  ses:1::dg http://dl.example.org/ibs/SUSE/Products/Storage/1.0/x86_64/product_debug/
  ses:1::du http://dl.example.org/ibs/SUSE/Updates/Storage/1.0/x86_64/update_debug/

  $ repoq -a x86_64 suse-manager-server:2.1
  suse-manager-server:2.1::gm http://dl.example.org/update/zypp-patches.suse.de/x86_64/update/SUSE-MANAGER/2.1-POOL/
  suse-manager-server:2.1::up http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SUSE-MANAGER/2.1/x86_64/update/

  $ repoq -a x86_64 suse-manager-server:3.0
  suse-manager-server:3.0::gm http://dl.example.org/ibs/SUSE/Products/SUSE-Manager-Server/3.0/x86_64/product/
  suse-manager-server:3.0::up http://dl.example.org/ibs/SUSE/Updates/SUSE-Manager-Server/3.0/x86_64/update/

  $ repoq -a x86_64 suse-manager-proxy:2.1
  suse-manager-proxy:2.1::gm http://dl.example.org/update/zypp-patches.suse.de/x86_64/update/SUSE-MANAGER-PROXY/2.1-POOL/
  suse-manager-proxy:2.1::up http://dl.example.org/update/build-ncc.suse.de/SUSE/Updates/SUSE-MANAGER-PROXY/2.1/x86_64/update/

  $ repoq -a x86_64 suse-manager-proxy:3.0
  suse-manager-proxy:3.0::gm http://dl.example.org/ibs/SUSE/Products/SUSE-Manager-Proxy/3.0/x86_64/product/
  suse-manager-proxy:3.0::up http://dl.example.org/ibs/SUSE/Updates/SUSE-Manager-Proxy/3.0/x86_64/update/
