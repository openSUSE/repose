#!/usr/bin/zsh -f
# vim: ft=zsh sw=2 sts=2 et fdm=marker cms=\ #\ %s

declare -gr cmdname=$0:t

declare -gr cmdhelp=$'
usage: #c -h | --help | HOST...
List matching products
  Options:
    -h                    Display this message
    --help                Display full help

  Operands:
    HOST                  Machine to operate on
'

. ${REPOSE_PRELUDE:-@preludedir@/repose.prelude.zsh} || exit 2

function $cmdname-main # {{{
{
  local -a options; options=(
    h help
  )
  local on oa
  local -i oi=0
  while haveopt oi on oa $=options -- "$@"; do
    case $on in
    h | help      ) display-help $on ;;
    *             ) reject-misuse -$oa ;;
    esac
  done; shift $oi

  (( $# )) || reject-misuse

  local -a hosts; hosts=("$@")

  local REPLY r
  local -a reply
  local h
  for h in $hosts; do
    o rh-list-products $h
    for r in $reply; do
      print -f "%s %q\n" -- $h $r
    done
  done
} # }}}

$cmdname-main "$@"
