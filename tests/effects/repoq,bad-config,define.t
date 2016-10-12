borked configuration: config lines
==================================

setup::

  $ . $TESTROOT/setup


test var-less define::

  $ cat > varless <<\EOF
  > # comment
  > 
  > define
  > EOF

  $ repoq -F varless sles:12:x86_64
  repoq: syntax error in */varless line 3: (glob)
  repoq: define
  repoq: missing variable name
  [1]


test value-less define::

  $ cat > valueless <<\EOF
  > # comment
  > 
  > define FOO
  > EOF

  $ repoq -F valueless sles:12:x86_64
  repoq: syntax error in */valueless line 3: (glob)
  repoq: define FOO
  repoq: missing value
  [1]


test define with multiple values::

  $ cat > multival <<\EOF
  > # comment
  > define FOO BAR BAZ
  > EOF

  $ repoq -F multival sles:12:x86_64
  repoq: syntax error in */multival line 2: (glob)
  repoq: define FOO BAR BAZ
  repoq: trailing garbage after value
  [1]

  $ cat > multival <<\EOF
  > # comment
  > define FOO "BAR BAZ" QUX
  > EOF

  $ repoq -F multival sles:12:x86_64
  repoq: syntax error in */multival line 2: (glob)
  repoq: define FOO "BAR BAZ" QUX
  repoq: trailing garbage after value
  [1]

  $ cat > multival <<\EOF
  > # comment
  > define FOO 'BAR BAZ' QUX
  > EOF

  $ repoq -F multival sles:12:x86_64
  repoq: syntax error in */multival line 2: (glob)
  repoq: define FOO 'BAR BAZ' QUX
  repoq: trailing garbage after value
  [1]


test overwrite::

  $ cat > cfgfile <<\EOF
  > define FOO bar
  > # is this a comment?
  > # it's definitely a comment!
  > define FOO qux
  > EOF

  $ repoq -F cfgfile sles:12:x86_64
  repoq: syntax error in */cfgfile line 4: (glob)
  repoq: define FOO qux
  repoq: duplicate definition
  [1]
