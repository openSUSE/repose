repose list-products
====================

setup::

  $ . $TESTROOT/setup

  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost rofl.example.org x86_64 \
  > sled:12 \
  > suse-manager-server:3.0


  $ fake-refhost lmao.example.org x86_64 \
  > sled:12 \
  > suse-manager-server:2.1

test::

  $ repose list-products rofl.example.org
  rofl.example.org sled:12:x86_64
  rofl.example.org suse-manager-server:3.0:x86_64

  $ repose list-products lmao.example.org
  lmao.example.org sled:12:x86_64
  lmao.example.org suse-manager-server:2.1:x86_64
