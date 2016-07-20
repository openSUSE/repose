#!/usr/bin/zsh -f
# vim: ft=zsh sw=2 sts=2 et fdm=marker cms=\ #\ %s

declare -gr cmdname=$0:t

declare -gr cmdhelp=$'
usage: #c -h | --help | [-n] HOST... [-- REPA...]
Remove matching repositories
  Options:
    -h                    Display this message
    --help                Display full help
    -n,--print            Display, do not perform destructive commands

  Operands:
    HOST                  Machine to operate on
    REPA                  Repository to remove
'

. ${REPOSE_PRELUDE:-@preludedir@/repose.prelude.zsh} || exit 2


function do-remove # {{{
{
  local h=$1 rn=$2 ru=$3; shift 3
  local -a repas; repas=("$@")
  [[ $rn == ${(j:|:)~repas} ]] || return 0
  rh-repo-remove $h $rn $ru
} # }}}

function rh-repo-remove # {{{
{
  run-in $1 zypper -n rr $3
} # }}}

main-hosts-repas "$@"
