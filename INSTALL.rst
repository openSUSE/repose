.. vim: ft=rst sw=2 sts=2 et tw=72

=======================================================================
                        Installing Repose/Repoq
=======================================================================


SUSE Linux Enterprise
=====================

::

  zypper ar -f http://download.suse.de/ibs/home:/rneuhauser/SLE_12/home:rneuhauser.repo


openSUSE Tumbleweed
===================

::

  zypper ar -f http://download.suse.de/ibs/home:/rneuhauser/openSUSE_Tumbleweed/home:rneuhauser.repo


Common Steps
============

::

  zypper in cram curl haveopt make openssh sumaxy xmlstarlet zsh
  git clone gitlab@gitlab.suse.de:rneuhauser/repose.git
  cd repose
  make check
  sudo make install
