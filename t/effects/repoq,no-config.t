missing configuration
=====================

setup::

  $ . $TESTROOT/setup


through env::

  $ REPOQ_RULES=~/nonexistent repoq sles:12:x86_64
  repoq: file not found: ~/nonexistent
  [1]


through option::

  $ repoq -F ~/nonexistent sles:12:x86_64
  repoq: file not found: ~/nonexistent
  [1]
