repose remove
=============

setup::

  $ . $TESTROOT/setup

  $ repose_dryrun=(${repose_dryrun:#ssh%*})

  $ fake-refhost fubar.example.org x86_64 \
  >   sles:12 \
  >   -- \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::{gm,up}

  $ fake-refhost snafu.example.org x86_64 \
  >   sled:12 \
  >   sle-sdk:12 \
  >   -- \
  >   sled:12::{gm,up,nv} \
  >   sle-sdk:12::gm


funky repository patterns
-------------------------

asterisk is what you'd expect::

  $ repose remove -n fubar.example.org -- \*
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ repose remove -n fubar.example.org -- 'sle*:12*'
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

other wildcard spellings::

  $ function rr { repose remove -n fubar.example.org -- "$@" }
  $ diff -u =(rr \*) =(rr   :)
  $ diff -u =(rr \*) =(rr  ::)
  $ diff -u =(rr \*) =(rr :::)

empty fields are treated as asterisks::

  $ repose remove -n fubar.example.org -- :::gm
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/

complementary set, simplified syntax::

  $ noglob repose remove -n snafu.example.org -- :::^gm
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://download.nvidia.com/novell/sle12/

  $ noglob repose remove -n snafu.example.org -- :::^gm,nv
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/

  $ noglob repose remove -n snafu.example.org -- :::~gm
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://download.nvidia.com/novell/sle12/

  $ noglob repose remove -n snafu.example.org -- :::~gm,nv
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/

complementary set, extended_glob syntax::

  $ noglob repose remove -n snafu.example.org -- :::(*~gm)
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://download.nvidia.com/novell/sle12/

  $ noglob repose remove -n snafu.example.org -- :::(*~(gm|nv))
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/

multiple repo patterns::

  $ noglob repose remove -n fubar.example.org snafu.example.org -- sles:12 sled:12::^nv,at sle-sdk
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  ssh -n -o BatchMode=yes fubar.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  ssh -n -o BatchMode=yes snafu.example.org zypper -n rr http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
