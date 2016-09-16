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

test::

  $ repose install root@{omg,wtf}.example.org -- sle-{module-toolchain,sdk}:12
  o scp -Bq root@omg.example.org:/etc/products.d/baseproduct * (glob)
  o repoq -A -a x86_64 -t gm -t up -t se -t lt sle-module-toolchain:12
  o ssh -n -o BatchMode=yes root@omg.example.org 'zypper -n ar -cgkn sle-module-toolchain:12::gm http://*/SLE-Module-Toolchain/12/x86_64/product/ sle-module-toolchain:12::gm' (glob)
  o ssh -n -o BatchMode=yes root@omg.example.org 'zypper -n ar -cgknf sle-module-toolchain:12::up http://*/SLE-Module-Toolchain/12/x86_64/update/ sle-module-toolchain:12::up' (glob)
  o ssh -n -o BatchMode=yes root@omg.example.org 'zypper -n --gpg-auto-import-keys in -l sle-module-toolchain-release'
  o repoq -A -a x86_64 -t gm -t up -t se -t lt sle-sdk:12
  o ssh -n -o BatchMode=yes root@omg.example.org 'zypper -n ar -cgkn sle-sdk:12::gm http://*/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm' (glob)
  o ssh -n -o BatchMode=yes root@omg.example.org 'zypper -n ar -cgknf sle-sdk:12::up http://*/SLE-SDK/12/x86_64/update/ sle-sdk:12::up' (glob)
  o ssh -n -o BatchMode=yes root@omg.example.org 'zypper -n --gpg-auto-import-keys in -l sle-sdk-release'
  o scp -Bq root@wtf.example.org:/etc/products.d/baseproduct * (glob)
  o repoq -A -a x86_64 -t gm -t up -t se -t lt sle-module-toolchain:12
  o ssh -n -o BatchMode=yes root@wtf.example.org 'zypper -n ar -cgkn sle-module-toolchain:12::gm http://*/SLE-Module-Toolchain/12/x86_64/product/ sle-module-toolchain:12::gm' (glob)
  o ssh -n -o BatchMode=yes root@wtf.example.org 'zypper -n ar -cgknf sle-module-toolchain:12::up http://*/SLE-Module-Toolchain/12/x86_64/update/ sle-module-toolchain:12::up' (glob)
  o ssh -n -o BatchMode=yes root@wtf.example.org 'zypper -n --gpg-auto-import-keys in -l sle-module-toolchain-release'
  o repoq -A -a x86_64 -t gm -t up -t se -t lt sle-sdk:12
  o ssh -n -o BatchMode=yes root@wtf.example.org 'zypper -n ar -cgkn sle-sdk:12::gm http://*/SLE-SDK/12/x86_64/product/ sle-sdk:12::gm' (glob)
  o ssh -n -o BatchMode=yes root@wtf.example.org 'zypper -n ar -cgknf sle-sdk:12::up http://*/SLE-SDK/12/x86_64/update/ sle-sdk:12::up' (glob)
  o ssh -n -o BatchMode=yes root@wtf.example.org 'zypper -n --gpg-auto-import-keys in -l sle-sdk-release'
