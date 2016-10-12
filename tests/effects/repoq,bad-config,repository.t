borked configuration: repository lines
======================================

setup::

  $ . $TESTROOT/setup


test traling garbage::

  $ cat > multival <<\EOF
  > # comment
  > foo
  >   bar baz qux
  > EOF

  $ repoq -F multival sles:12:x86_64
  repoq: syntax error in */multival line 3: (glob)
  repoq:   bar baz qux
  repoq: trailing garbage after repository url
  [1]


test duplicates::

  $ cat > duprepo <<\EOF
  > foo
  >   bar baz
  > omg
  >   this that
  >   this that
  > wtf
  >   mine yours
  > EOF

  $ repoq -F duprepo sles:12:x86_64
  repoq: syntax error in */duprepo line 5: (glob)
  repoq:   this that
  repoq: duplicate definition
  [1]
