#!/usr/bin/zsh -f
# vim: ft=zsh sw=2 sts=2 et fdm=marker cms=\ #\ %s

declare -gr cmdname=$0:t

declare -gr cmdhelp=$'
usage: #c -h | --help | [-n] HOST... -- ADDON...
Install a product, add its repositories
  Options:
    -h                    Display this message
    --help                Display full help
    -n,--print            Display, do not perform destructive commands

  Operands:
    HOST                  Machine to operate on
    ADDON                 Product to install
'

. ${REPOSE_PRELUDE:-@preludedir@/repose.prelude.zsh} || exit 2


DO_INSTALL=1 main-add-install "$@"
