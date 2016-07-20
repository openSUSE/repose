#!/usr/bin/zsh -f
# vim: ft=zsh sw=2 sts=2 et fdm=marker cms=\ #\ %s

# Copyright (C) 2016 SUSE LLC
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <http://www.gnu.org/licenses/>.

declare -gr cmdname=$0:t

declare -gr cmdhelp=$'
usage: #c -h | --help | HOST... [-- REPA...]
List matching repositories
  Options:
    -h                    Display this message
    --help                Display full help

  Operands:
    HOST                  Machine to operate on
    REPA                  Repository to list
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

  local -i seppos="$@[(i)--]"
  local -a hosts; hosts=("$@[1,$((seppos - 1))]")
  local -a repas; repas=("$@[$((seppos + 1)),-1]")

  (( $#hosts )) || reject-misuse
  (( $#repas )) || repas=('*')

  local REPLY
  local -a reply
  local -i i

  for ((i = 1; i <= $#repas; ++i)); do
    fixup-repa $repas[$i]
    repas[$i]=$REPLY
  done

  local h rn ru
  for h in $hosts; do
    o rh-list-repos $h
    for rn ru in $reply; do
      [[ $rn == ${(j:|:)~repas} ]] || continue
      print $h $ru
    done
  done
} # }}}

$cmdname-main "$@"
