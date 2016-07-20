repose list
===========

setup::

  $ . $TESTROOT/setup

  $ fake-refhost \$local x86_64 \
  >   sles:12 \
  >   sle-sdk:12 \
  >   -- \
  >   sled:12::{nv,at} \
  >   sles:12::{gm,up} \
  >   sle-sdk:12::{gm,up}


funky repository patterns
-------------------------

asterisk is what you'd expect::

  $ repose list . -- \*
  . http://download.nvidia.com/novell/sle12/
  . http://www2.ati.com/suse/sle12/
  . http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ repose list . -- 'sle*:12*'
  . http://download.nvidia.com/novell/sle12/
  . http://www2.ati.com/suse/sle12/
  . http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ repose list . -- :::gm
  . http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/

other wildcard spellings::

  $ function rr { repose list . -- "$@" }
  $ diff -u =(rr \*) =(rr   :)
  $ diff -u =(rr \*) =(rr  ::)
  $ diff -u =(rr \*) =(rr :::)

complementary set, simplified syntax::

  $ noglob repose list . -- :::^gm
  . http://download.nvidia.com/novell/sle12/
  . http://www2.ati.com/suse/sle12/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ noglob repose list . -- :::^gm,nv
  . http://www2.ati.com/suse/sle12/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ noglob repose list . -- :::~gm
  . http://download.nvidia.com/novell/sle12/
  . http://www2.ati.com/suse/sle12/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ noglob repose list . -- :::~gm,nv
  . http://www2.ati.com/suse/sle12/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

complementary set, extended_glob syntax::

  $ noglob repose list . -- :::(*~gm)
  . http://download.nvidia.com/novell/sle12/
  . http://www2.ati.com/suse/sle12/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

  $ noglob repose list . -- :::(*~(gm|nv))
  . http://www2.ati.com/suse/sle12/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/

multiple repo patterns::

  $ noglob repose list . -- sles:12 sled:12::^nv,at sle-sdk
  . http://dl.example.org/ibs/SUSE/Products/SLE-SERVER/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SERVER/12/x86_64/update/
  . http://dl.example.org/ibs/SUSE/Products/SLE-SDK/12/x86_64/product/
  . http://dl.example.org/ibs/SUSE/Updates/SLE-SDK/12/x86_64/update/
