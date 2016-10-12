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

declare -gr cmdhelp="
usage: #c -h | --help | COMMAND [options] [operands]
Manipulate products and repositories
  Options:
    -h                    Display this message
    --help                Display full help

  Commands:
    add                   Add matching repositories
    clear                 Remove all repositories
    install               Install a product, add its repositories
    issue-add             Add issue-specific repositories
    issue-rm              Remove issue-specific repositories
    list                  List matching repositories
    list-products         List matching products
    prune                 Remove stray repositories
    remove                Remove matching repositories
    reset                 Remove stray repositories, add missing ones
    switch-to             Enable matching repositories, disable their complementary set
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
