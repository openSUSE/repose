repose list-products
====================

setup::

  $ . $TESTROOT/setup

  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost omg.example.org x86_64 \
  > sled:12 \
  > sle-module-web-scripting:12

  $ fake-refhost wtf.example.org x86_64 \
  > sles:12 \
  > sle-sdk:12

test::

  $ repose list-products {wtf,omg}.example.org
  wtf.example.org sles:12:x86_64
  wtf.example.org sle-sdk:12:x86_64
  omg.example.org sled:12:x86_64
  omg.example.org sle-module-web-scripting:12:x86_64
