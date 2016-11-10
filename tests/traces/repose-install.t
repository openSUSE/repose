repose install
==============

setup::

  $ . $TESTROOT/setup

  $ repose_chatty+=('repoq%*' 'scp%*' 'ssh%*')
  $ repose_dryrun+=('ssh%*')

  $ fake-refhost root@omg.example.org x86_64 \
  >   sles:12 \
  >   --

  $ fake-refhost root@wtf.example.org x86_64 \
  >   sles:12 \
  >   --

  $ fake-refhost root@osuse.example.org x86_64 \
  >   openSUSE:42.2 \
  >   --
test::

  $ repose install -f root@{omg,wtf}.example.org -- sle-{module-toolchain,sdk}:12
  o scp -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@omg.example.org:/etc/products.d/baseproduct * (glob)
  o repoq -A -a x86_64 -t gm -t lt -t se -t nv -t at -t up sle-module-toolchain:12
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@omg.example.org 'zypper -n ar -cgkn sle-module-toolchain:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Toolchain/12/x86_64/product/ sle-module-toolchain:12::gm'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@omg.example.org 'zypper -n ar -cfgkn sle-module-toolchain:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Toolchain/12/x86_64/update/ sle-module-toolchain:12::up'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@omg.example.org 'zypper -n --gpg-auto-import-keys in --force -l sle-module-toolchain-release'
  o repoq -A -a x86_64 -t gm -t lt -t se -t nv -t at -t up sle-sdk:12
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@omg.example.org 'zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@omg.example.org 'zypper -n ar -cfgkn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@omg.example.org 'zypper -n --gpg-auto-import-keys in --force -l sle-sdk-release'
  o scp -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@wtf.example.org:/etc/products.d/baseproduct * (glob)
  o repoq -A -a x86_64 -t gm -t lt -t se -t nv -t at -t up sle-module-toolchain:12
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@wtf.example.org 'zypper -n ar -cgkn sle-module-toolchain:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-Module-Toolchain/12/x86_64/product/ sle-module-toolchain:12::gm'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@wtf.example.org 'zypper -n ar -cfgkn sle-module-toolchain:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-Module-Toolchain/12/x86_64/update/ sle-module-toolchain:12::up'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@wtf.example.org 'zypper -n --gpg-auto-import-keys in --force -l sle-module-toolchain-release'
  o repoq -A -a x86_64 -t gm -t lt -t se -t nv -t at -t up sle-sdk:12
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@wtf.example.org 'zypper -n ar -cgkn sle-sdk:12::gm http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@wtf.example.org 'zypper -n ar -cfgkn sle-sdk:12::up http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/ sle-sdk:12::up'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@wtf.example.org 'zypper -n --gpg-auto-import-keys in --force -l sle-sdk-release'

  $ repose install -f root@osuse.example.org  -- openSUSE-Addon-NonOss:42.2
  o scp -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org:/etc/products.d/baseproduct * (glob)
  o repoq -A -a x86_64 -t gm -t lt -t se -t nv -t at -t up openSUSE-Addon-NonOss:42.2
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org 'zypper -n ar -cgkn openSUSE-Addon-NonOss:42.2::gm http://download.opensuse.org/distribution/leap/42.2/repo/non-oss/ openSUSE-Addon-NonOss:42.2::gm'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org 'zypper -n ar -cfgkn openSUSE-Addon-NonOss:42.2::up http://download.opensuse.org/update/leap/42.2/non-oss/ openSUSE-Addon-NonOss:42.2::up'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org 'zypper -n ar -cfgkn openSUSE-Addon-NonOss:42.2::nv http://http.download.nvidia.com/opensuse/leap/42.2/ openSUSE-Addon-NonOss:42.2::nv'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org 'zypper -n --gpg-auto-import-keys in --force -l openSUSE-Addon-NonOss-release'

test without force flag::
  $ repose install root@osuse.example.org  -- openSUSE-Addon-NonOss:42.2
  o scp -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org:/etc/products.d/baseproduct * (glob)
  o repoq -A -a x86_64 -t gm -t lt -t se -t nv -t at -t up openSUSE-Addon-NonOss:42.2
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org 'zypper -n ar -cgkn openSUSE-Addon-NonOss:42.2::gm http://download.opensuse.org/distribution/leap/42.2/repo/non-oss/ openSUSE-Addon-NonOss:42.2::gm'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org 'zypper -n ar -cfgkn openSUSE-Addon-NonOss:42.2::up http://download.opensuse.org/update/leap/42.2/non-oss/ openSUSE-Addon-NonOss:42.2::up'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org 'zypper -n ar -cfgkn openSUSE-Addon-NonOss:42.2::nv http://http.download.nvidia.com/opensuse/leap/42.2/ openSUSE-Addon-NonOss:42.2::nv'
  o ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no root@osuse.example.org 'zypper -n --gpg-auto-import-keys in -l openSUSE-Addon-NonOss-release'

