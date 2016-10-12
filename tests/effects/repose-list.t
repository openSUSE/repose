repose list
===========

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

  $ repose list fubar.example.org -- \*
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ repose list fubar.example.org -- 'sle*:12*'
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ repose list fubar.example.org -- :::gm
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/

other wildcard spellings::

  $ function rr { repose list fubar.example.org -- "$@" }
  $ diff -u =(rr \*) =(rr   :)
  $ diff -u =(rr \*) =(rr  ::)
  $ diff -u =(rr \*) =(rr :::)

complementary set, simplified syntax::

  $ noglob repose list snafu.example.org -- :::^gm
  snafu.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  snafu.example.org http://download.nvidia.com/novell/sle12/

  $ noglob repose list snafu.example.org -- :::^gm,nv
  snafu.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/

  $ noglob repose list snafu.example.org -- :::~gm
  snafu.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  snafu.example.org http://download.nvidia.com/novell/sle12/

  $ noglob repose list snafu.example.org -- :::~gm,nv
  snafu.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/

complementary set, extended_glob syntax::

  $ noglob repose list snafu.example.org -- :::(*~gm)
  snafu.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  snafu.example.org http://download.nvidia.com/novell/sle12/

  $ noglob repose list snafu.example.org -- :::(*~(gm|nv))
  snafu.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/

multiple repo patterns::

  $ noglob repose list fubar.example.org snafu.example.org -- sles:12 sled:12::^nv,at sle-sdk
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  fubar.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  fubar.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
  snafu.example.org http://dl.example.org/ibs/SUSE/Products/SLE-DESKTOP/12/x86_64/product/
  snafu.example.org http://dl.example.org/ibs/SUSE/Updates/SLE-DESKTOP/12/x86_64/update/
  snafu.example.org http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
