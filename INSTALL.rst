.. vim: ft=rst sw=2 sts=2 et tw=72

=======================================================================
                        Installing Repose/Repoq
=======================================================================


SUSE, openSUSE
==============

::

  zypper install cram curl haveopt make openssh sumaxy xmlstarlet \
                 zsh python-docutils
  git clone https://github.com/openSUSE/repose.git
  mkdir repose-build
  cd repose-build
  ../repose/configure
  make check
  sudo make install
