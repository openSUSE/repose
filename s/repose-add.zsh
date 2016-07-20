#!/usr/bin/zsh -f
# vim: ft=zsh sw=2 sts=2 et fdm=marker cms=\ #\ %s

declare -gr cmdname=$0:t

declare -gr cmdhelp=$'
usage: #c -h | --help | [-n] HOST... [-- REPA...]
Add requested repositories
  Options:
    -h                    Display this message
    --help                Display full help
    -n,--print            Display, do not perform destructive commands

  Operands:
    HOST                  Machine to operate on
    REPA                  Repository to add
'

. ${REPOSE_PRELUDE:-@preludedir@/repose.prelude.zsh} || exit 2


DO_INSTALL=0 main-add-install "$@"
