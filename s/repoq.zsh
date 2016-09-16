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

# help strings {{{
declare -gr cmdusage=$'
usage: #c -h | --help | [-F RULES][-A|-R][-N][-a ARCH][-t TAG]... SPEC...
Output repository information for given products
  Options:
    -h                    Display this message
    --help                Display full help
    -A,--addrepo          Output `zypper addrepo` commands
    -F,--file=RULES       Use product/repository information from RULES
    -N,--no-name          Omit repository names
    -R,--removerepo       Output `zypper removerepo` commands
    -a,--arch=ARCH        Default architecture of requested repositories
    -t,--tag=TAG          Imply TAG for SPECs with no tagset

  Operands:
    SPEC                  P:V[:A[:T]
'
# }}}

. haveopt.sh

setopt extended_glob
setopt hist_subst_pattern
setopt err_return
setopt no_unset
setopt warn_create_global

declare -r cfgdir=${REPOQ_CFGDIR:-@etcdir@}

function repoq-main # {{{
{
  # argument handling {{{
  local -a options; options=(
    h  help
    A  ar
    F= file=
    N  no-name
    R  rr
    a= arch=
    t= tag=
  )

  local o_rules=${REPOQ_RULES:-$cfgdir/repoq.rules}
  local o_arch
  local -a o_tags
  local o_named=x
  local o_zypper

  local -i oi=0
  local on oa
  while haveopt oi on oa $=options -- "$@"; do
  case $on in
  h | help    ) display-help $on ;;
  A | ar      ) o_zypper=ar ;;
  F | file    ) o_rules=$oa ;;
  N | no-name ) o_named= ;;
  R | rr      ) o_zypper=rr ;;
  a | arch    ) o_arch=$oa ;;
  t | tag     ) o_tags+=($oa) ;;
  *           ) reject-misuse -$oa ;;
  esac
  done; shift $oi

  (( $# )) || reject-misuse

  local arg
  for arg in "$@"; do
    :; [[ $arg == [^:]##:[^:]##:[^:]##* ]] \
    || (( $#o_arch )) \
    || o reject-misuse $arg
  done
  # }}}

  [[ -e $o_rules ]] \
  || o complain 1 "file not found: ${(D)o_rules}"

  o run-query "$@"
} # }}}

function run-query # {{{
{
  local -a reply
  o read-rules $o_rules
  local -A rules vars
  local k v
  for k v in $reply; do
    case $k in
    define\|*) vars+=(${k#*\|} $v) ;;
    *)         rules+=($k $v) ;;
    esac
  done
  local arg
  for arg in "$@"; do
    :; o handle-arg $arg \
    || o complain 1 "no rule matches ${(qq)arg}"
  done
} # }}}

function handle-arg # {{{
{
  local arg=$1
  local -i rv=1
  local -a reply
  o fixup-request $arg
  local req=$reply[1]
  local tags=$reply[2]
  local pat=
  for pat in ${(k)rules}; do
    [[ $req == $~pat* ]] || continue
    rv=0
    local spec=
    for spec in ${(s: :)rules[$pat]}; do
      local tag=${spec%%:*}
      local url=${spec#*:}
      [[ $tag == $~tags ]] || continue
      o display-match "${(@s.:.)req}" $tag $url
    done
  done
  return $rv
} # }}}

function fixup-request # {{{
{
  local -a parts; parts=("${(@s.:.)1}" '' '' '')
  local tags="${${parts[4]:-${(j:,:)o_tags:-*}}//,/|}"
  [[ $tags == [~^]?* ]] \
  && tags="*~(${tags#?})"
  tags="($tags)"
  parts=(
    "$parts[1]"
    "$parts[2]"
    "${parts[3]:-$o_arch}"
  )
  local req="${(@j.:.)parts}"
  reply=($req $tags)
} # }}}

function display-match # {{{
{
  local H=$vars[H]
  local P=${(U)1} V=${2/./-SP} v=$2 A=$3 tag=$4 url=$5
  local rname=$1:$v::$tag
  [[ -n $A ]] \
  || o complain 1 'no architecture requested'
  case $o_zypper in
  ar)
    if [[ $tag == 'gm' ]]
    then
      print ${o_named:+$rname} zypper -n $o_zypper -cgkn $rname ${(e)url} $rname
    else
      print ${o_named:+$rname} zypper -n $o_zypper -cgknf $rname ${(e)url} $rname
    fi
  ;;
  rr) print ${o_named:+$rname} zypper -n $o_zypper ${(e)url}
  ;;
  '') print ${o_named:+$rname} ${(e)url}
  ;;
  * ) complain 2 "internal error: invalid \$o_zypper value (${(qq)o_zypper})"
  ;;
  esac
} # }}}

function read-rules # {{{
{
  local rules=$1
  local -i lino
  local s
  local patt
  local -A rv
  ; cat $rules \
  | while IFS='\n' read s; do
      (( ++lino ))
      [[ -n $s ]] || {
        continue
      }

      local -a words

      case $s in
      # comment
      \#*) continue ;;

      # define VAR VAL
      define[[:space:]]#)
        o complain 1 \
          "syntax error in ${1:a} line $lino:" \
          "$s" \
          "missing variable name"
      ;;
      define([[:space:]]##[[:IDENT:]]##*)#*)
        words=(${(Z:C:)s})
        local -a messages; messages=(
          "missing varname"
          "missing value"
          --
          "trailing garbage after value"
        )

        (( $#words == 3 )) \
        || o complain 1 \
            "syntax error in ${1:a} line $lino:" \
            "$s" \
            $messages[$#words]

        patt="define|$words[2]"
        (( ${+rv[$patt]} == 0 )) \
        || o complain 1 "syntax error in ${1:a} line $lino:" \
            "$s" \
            "duplicate definition"
        rv[$patt]=$words[3]
      ;;

      # product
      [![:space:]]*)
        words=(${(Z:C:)s})
        (( $#words == 1 )) \
        || o complain 1 "syntax error in ${1:a} line $lino:" \
            "$s" \
            "trailing garbage after product pattern"

        patt=$words
        (( ${+rv[$patt]} == 0 )) \
        || o complain 1 "syntax error in ${1:a} line $lino:" \
            "$s" \
            "duplicate definition"
        rv[$patt]=''
      ;;

      # repository
      [[:space:]]##[![:space:]]*)
        words=(${(Z:C:)s})
        local -a messages; messages=(
          --
          "missing repository url"
          "trailing garbage after repository url"
        )
        (( $#words == 2 )) \
        || o complain 1 "syntax error in ${1:a} line $lino:" \
            "$s" \
            $messages[$#words]

        [[ ${rv[$patt]-} != (* ${(j.:.)words}*) ]] \
        || o complain 1 "syntax error in ${1:a} line $lino:" \
            "$s" \
            "duplicate definition"
        rv[$patt]+=" ${(j.:.)words}"
      ;;
      esac
    done
    reply=("${(@kv)rv}")
} # }}}

function display-help # {{{
{
  [[ $1 == h ]] && {
    o display-helpstring cmdusage
    exit
  }
  o exec man 1 $cmdname
  exit # we get here in tests
} # }}}

function display-helpstring # {{{
{
  local v=$1 self=${cmdname/-/ }
  print -- ${${${${(P)v}//\#c/$self}//#[[:space:]]#/}//%[[:space:]]#/}
} # }}}

function reject-misuse # {{{
{
  local val=${1-} self=${cmdname/-/ } ex=1
  case $val in
  -?)  print -f "%s: unknown option '%s'\n" -- $self $val ;;
  -?*) print -f "%s: unknown option '%s'\n" -- $self -$val ;;
  ?*)  print -f "%s: no architecture requested for '%s'\n" -- $self $val ;;
  '')  print -f "%s: missing argument\n" -- $self ;;
  esac
  print -u 2 -f $msg_run_for_usage $self
  exit $ex
} # }}}

function complain # {{{
{
  local ex=0
  if [[ $1 == <-> ]]; then
    ex=$1
  fi
  shift
  print -u 2 -f "$cmdname: %s\n" "$@"
  exit $ex
} # }}}

function o # {{{
{
  declare -i dryrun=0
  if [[ $1 == -n ]]; then
    shift
    dryrun=1
  fi
  if (( $#REPOQ_CHATTY )); then
    if [[ "${(@j,%,)@}" == $~REPOQ_CHATTY ]]; then
      print -ru $logfd -- o "${(q-)@}"
    fi
  fi
  if (( $#REPOQ_DRYRUN )); then
    if [[ "${(@j,%,)@}" == $~REPOQ_DRYRUN ]]; then
      dryrun=1
    fi
  fi
  if (( dryrun )); then
    return 0
  fi
  "$@" || return $?
} # }}}

declare -Tgx REPOQ_CHATTY repoq_chatty \|
declare -Tgx REPOQ_DRYRUN repoq_dryrun \|

declare -gir logfd=2

declare -gr msg_run_for_usage="run '%s -h' for usage instructions\n"

bindir=$0:h repoq-main "$@"
