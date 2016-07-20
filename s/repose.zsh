#!/usr/bin/zsh -f
# vim: ft=zsh sw=2 sts=2 et fdm=marker cms=\ #\ %s

declare -gr cmdname=$0:t

declare -gr cmdhelp="
usage: #c -h | --help | COMMAND [options] [operands]
Manipulate zypper repositories based on /etc/products.d
  Options:
    -h                    Display this message
    --help                Display full help

  Commands:
    add                   Add requested repositories
    clear                 Remove all repositories
    install               Install an addon, add its repositories
    issue-add             Add issue-specific repositories
    issue-rm              Remove issue-specific repositories
    list                  List matching repositories
    list-products         List matching products
    remove                Remove matching repositories
    reset                 Remove stray repositories
    switch-to             Enable requested repositories, disable their complementary set
"

. ${REPOSE_PRELUDE:-@preludedir@/repose.prelude.zsh} || exit 2

function $cmdname # {{{
{
  local -a options; options=(
    h help
  )
  local -i oi
  local on oa
  while haveopt oi on oa $=options -- "$@"; do
  case $on in
    h|help) display-help $on ;;
    *)      reject-misuse -$oa ;;
  esac
  done; shift $oi

  local REPLY
  local -a reply parts

  (( $# )) || reject-misuse

  O find-cmd $1
  o run-cmd $REPLY $@[2,-1]
} # }}}

function find-cmd # {{{
{
  local name=$cmdname-$1 impl=

  for impl in $name $bindir/$name; do
    if impl=$(whence $impl); then
      REPLY=$impl
      return
    fi
  done

  complain 1 "unknown command ${(qq)1}"
} # }}}

function run-cmd # {{{
{
  "$@"
} # }}}

bindir=$0:h $cmdname "$@"
