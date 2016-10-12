borked configuration: product lines
===================================

setup::

  $ . $TESTROOT/setup


test traling garbage::

  $ cat > multival <<\EOF
  > # comment
  > foo
  >   bar baz
  > omg wtf
  >   rofl lmao
  > EOF

  $ repoq -F multival sles:12:x86_64
  repoq: syntax error in */multival line 4: (glob)
  repoq: omg wtf
  repoq: trailing garbage after product pattern
  [1]


test duplicates::

  $ cat > dupprj <<\EOF
  > foo
  >   bar baz
  > omg
  >   this that
  > qux
  >   rofl lmao
  > omg
  >   mine yours
  > EOF

  $ repoq -F dupprj sles:12:x86_64
  repoq: syntax error in */dupprj line 7: (glob)
  repoq: omg
  repoq: duplicate definition
  [1]
