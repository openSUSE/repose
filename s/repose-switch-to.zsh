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
usage: #c -h | --help | [-n] HOST... -- REPA...
Enable matching repositories, disable their complementary set
  Options:
    -h                    Display this message
    --help                Display full help
    -n,--print            Display, do not perform destructive commands

  Operands:
    HOST                  Machine to operate on
    REPA                  Repository pattern
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
