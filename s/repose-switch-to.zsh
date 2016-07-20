#!/usr/bin/zsh -f
# vim: ft=zsh sw=2 sts=2 et fdm=marker cms=\ #\ %s

declare -gr cmdname=$0:t

declare -gr cmdhelp=$'
usage: #c -h | --help | [-n] HOST... -- REPA...
Enable requested repositories, disable their complementary set
  Options:
    -h                    Display this message
    --help                Display full help
    -n,--print            Display, do not perform destructive commands

  Operands:
    HOST                  Machine to operate on
    REPA                  Repository to enable
'

. ${REPOSE_PRELUDE:-@preludedir@/repose.prelude.zsh} || exit 2


function do-switch-to # {{{
{
  local h=$1 rn=$2 ru=$3; shift 3
  local -a repas; repas=("$@")
  local state=d
  if [[ $rn == ${(j:|:)~repas} ]]; then
    state=e
  fi
  o $print ssh -n -o BatchMode=yes $h "zypper -n mr -$state $ru"
} # }}}

main-hosts-repas "$@"
