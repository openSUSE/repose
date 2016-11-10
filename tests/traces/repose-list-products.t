repose list-products
====================

setup::

  $ . $TESTROOT/setup

  $ repose_chatty=('*')
  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost fubar.example.org x86_64 \
  > sles:12 \
  > sle-sdk:12

  $ fake-refhost osuse.example.org x86_64 \
  > openSUSE:42.2

test::

  $ repose list-products fubar.example.org osuse.example.org
  O find-cmd list-products
  o run-cmd */repose-list-products fubar.example.org osuse.example.org (glob)
  o rh-list-products fubar.example.org
  o get-from fubar.example.org '/etc/products.d/*.prod' * (glob)
  o scp -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no 'fubar.example.org:/etc/products.d/*.prod' * (glob)
  o xml-get-product */SLES:12.prod (glob)
  o xml sel -t -m /product -v ./name -o : --if ./baseversion -v ./baseversion --if ./patchlevel!=0 -o . -v ./patchlevel --break --else -v ./version --break -o : -v ./arch --nl */SLES:12.prod (glob)
  o xform-product SLES:12:x86_64
  o xml-get-product */sle-sdk:12.prod (glob)
  o xml sel -t -m /product -v ./name -o : --if ./baseversion -v ./baseversion --if ./patchlevel!=0 -o . -v ./patchlevel --break --else -v ./version --break -o : -v ./arch --nl */sle-sdk:12.prod (glob)
  o xform-product sle-sdk:12:x86_64
  o rm -rf * (glob)
  fubar.example.org sles:12:x86_64
  fubar.example.org sle-sdk:12:x86_64
  o rh-list-products osuse.example.org
  o get-from osuse.example.org '/etc/products.d/*.prod' * (glob)
  o scp -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no 'osuse.example.org:/etc/products.d/*.prod' * (glob)
  o xml-get-product */openSUSE:42.2.prod (glob)
  o xml sel -t -m /product -v ./name -o : --if ./baseversion -v ./baseversion --if ./patchlevel!=0 -o . -v ./patchlevel --break --else -v ./version --break -o : -v ./arch --nl */openSUSE:42.2.prod (glob)
  o xform-product openSUSE:42.2:x86_64
  o rm -rf * (glob)
  osuse.example.org openSUSE:42.2:x86_64
