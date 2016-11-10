repose issue-add
================

setup::

  $ . $TESTROOT/setup

  $ repose_chatty+=('*')

  $ fake-refhost fubar.example.org x86_64 sles:12 --
  $ cp -r $FIXTURES/SUSE:Maintenance:1085:89320 .

test::

  $ repose issue-add -n fubar.example.org -- SUSE:Maintenance:1085:89320
  O find-cmd issue-add
  o run-cmd */repose-issue-add -n fubar.example.org -- SUSE:Maintenance:1085:89320 (glob)
  o rh-get-arch-basev fubar.example.org
  o rh-fetch-baseproduct fubar.example.org * (glob)
  o get-from fubar.example.org /etc/products.d/baseproduct * (glob)
  o scp -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org:/etc/products.d/baseproduct * (glob)
  o xml-get-arch-basev * (glob)
  o xml sel -t -m /product -v arch -o ' ' -v baseversion --if patchlevel!=0 -o . -v patchlevel --break --nl * (glob)
  o rm -f * (glob)
  o rh-list-products fubar.example.org
  o get-from fubar.example.org '/etc/products.d/*.prod' * (glob)
  o scp -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no 'fubar.example.org:/etc/products.d/*.prod' * (glob)
  o xml-get-product */SLES:12.prod (glob)
  o xml sel -t -m /product -v ./name -o : --if ./baseversion -v ./baseversion --if ./patchlevel!=0 -o . -v ./patchlevel --break --else -v ./version --break -o : -v ./arch --nl */SLES:12.prod (glob)
  o xform-product SLES:12:x86_64
  o rm -rf * (glob)
  o sumaxy * x86_64 sles:12 (glob)
  o print ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org 'zypper -n ar -cgkn sles:12::p=1085 http://dl.example.org/ibs/SUSE:/Maintenance:/1085/SUSE_Updates_SLE-SERVER_12_x86_64/ sles:12::p=1085'
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper -n ar -cgkn sles:12::p=1085 http://dl.example.org/ibs/SUSE:/Maintenance:/1085/SUSE_Updates_SLE-SERVER_12_x86_64/ sles:12::p=1085
  o print ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org 'zypper --gpg-auto-import-keys refresh sles:12::p=1085 > /dev/null'
  ssh -n -q -o BatchMode=yes -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no fubar.example.org zypper --gpg-auto-import-keys refresh sles:12::p=1085 > /dev/null
