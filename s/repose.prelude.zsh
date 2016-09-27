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

setopt extended_glob
setopt hist_subst_pattern
setopt err_return
setopt no_unset
setopt warn_create_global

. haveopt.sh || exit 2

function main-hosts-repas # {{{
{
  local -a options; options=(
    h help
    n print
  )
  local print
  local on oa
  local -i oi=0
  while haveopt oi on oa $=options -- "$@"; do
    case $on in
    h | help      ) display-help $on ;;
    n | print     ) print=print ;;
    *             ) reject-misuse -$oa ;;
    esac
  done; shift $oi

  (( $# )) || reject-misuse

  local -i seppos="$@[(i)--]"
  local -a hosts; hosts=("$@[1,$((seppos - 1))]")
  local -a repas; repas=("$@[$((seppos + 1)),-1]")

  (( $#hosts )) || reject-misuse
  (( $#repas )) || reject-misuse

  local fname=do-${${cmdname#*-}:-complain}
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
      o $fname $h $rn $ru $repas
    done
  done
} # }}}

function main-add-install # {{{
{
  local -a options; options=(
    h   help
    n   print
    t=  tag=
  )
  local print
  local -a tags; tags=(gm lt se up)
  local -i first_tag=1
  local on oa
  local -i oi=0
  while haveopt oi on oa $=options -- "$@"; do
    case $on in
    h | help      ) display-help $on ;;
    n | print     ) print=print ;;
    t | tag       ) (( first_tag )) && { first_tag=0; tags=() }
                    tags+=($oa)
                    ;;
    *             ) reject-misuse -$oa ;;
    esac
  done; shift $oi

  (( $# )) || reject-misuse

  local -i seppos="$@[(i)--]"
  local -a hosts; hosts=("$@[1,$((seppos - 1))]")
  local -a patts; patts=("$@[$((seppos + 1)),-1]")

  (( $#hosts )) || reject-misuse
  (( $#patts )) || reject-misuse

  local -a parts
  local arch basev arg h rn zcmd
  for h in $hosts; do
    o rh-get-arch-basev $h \
    | read arch basev

    for arg in $patts; do
      parts=("${(@s.:.)arg}" '' '' '')
      parts=("${(@)parts[1,4]}")
      [[ $parts[2] == (|'*') ]] \
      && parts[2]=$basev
      o repoq -A -a $arch ${(s: :)tags/#/-t }  "${${(@j.:.)parts}%%:##}" \
      | while read rn zcmd; do
          if test-online-repo $zcmd
          then
            run-in $h $zcmd
          fi
        done
      if (( DO_INSTALL )); then
        run-in $h "zypper -n --gpg-auto-import-keys in -l ${parts[1]}-release"
      fi
    done
  done
} # }}}


function test-online-repo # {{{
{
  local f_args=${1}
  local -a url
  url=(${(s: :)f_args})
  curl -fIs ${url[6]} > /dev/null
 } #}}}


function rh-list-products # {{{
{
  local h=$1 d
  d=$(mktemp -d)
  trap "o rm -rf $d" EXIT
  o scp -Bq $h:/etc/products.d/\*.prod $d

  local pf REPLY
  reply=()
  for pf in $d/*.prod(N); do
    o xml-get-product $pf | read REPLY
    o xform-product $REPLY
    reply+=($REPLY)
  done
} # }}}

function xform-product # {{{
{
  local -a r; r=("${(s.:.)1}")
  case $r[1] in
  SLED|SUSE_SLED) r[1]=(sled) ;;
  SLES|SUSE_SLES) r[1]=(sles) ;;
  esac
  REPLY="${(j.:.L)r}"
} # }}}


function rh-fetch-baseproduct # {{{
{
  local h=$1 f=$2
  o scp -Bq $h:/etc/products.d/baseproduct $f
} # }}}

function rh-get-arch-basev # {{{
{
  local h=$1 f=$(mktemp -u)
  trap "o rm -f $f" EXIT
  o rh-fetch-baseproduct $h $f
  o xml-get-arch-basev $f
} # }}}

function xml-get-arch-basev # {{{
{
  o xml sel -t \
    -m /product \
    -v arch \
    -o ' ' \
    -v baseversion \
    --if 'patchlevel!=0' \
      -o . -v patchlevel \
    --break \
    --nl \
    $1
} # }}}


function xml-get-product # {{{
{
  local pf=$1
  o xml sel -t \
    -m /product \
      -v ./name -o : \
      --if ./baseversion \
        -v ./baseversion \
        --if "./patchlevel!=0" \
          -o . -v ./patchlevel \
        --break \
      --else \
        -v ./version \
      --break \
      -o : \
      -v ./arch \
      --nl \
    $pf
} # }}}

function rh-list-repos # {{{
{
  local host=$1 lr_xml
  local -a rv
  reply=()
  lr_xml=$(mktemp -u)
  trap "o rm -f $lr_xml" EXIT
  o redir -1 $lr_xml rh-fetch-repos $host
  o xml-get-repos $lr_xml
} # }}}

function rh-fetch-repos # {{{
{
  print= run-in $1 zypper -x lr
} # }}}

function xml-get-repos # {{{
{
  local f=$1 line
  local -a rv
  o xml sel -t \
    -m /stream/repo-list/repo \
    -v @name \
    -o $'\001' \
    -v url \
    --nl \
    $f \
  | while IFS=$'\001' read -A line; do
      rv+=($line)
    done
  reply=($rv)
} # }}}


function fixup-repa # {{{
{
  if [[ $1 == *\* ]]; then
    REPLY=$1
    return
  fi
  local -a parts
  parts=("${(@s.:.)1}" '' '' '')
  parts=("${(@)parts[1,4]:/#%/*}")
  parts[3]='*' # arch
  local tags="${parts[4]//,/|}"
  if [[ $tags == [~^]* ]]; then
    tags="*~(${tags#?})"
  fi
  parts[4]="($tags)"
  REPLY=${(j.:.)parts}
} # }}}


function run-in # {{{
{
  local h=$1 print=${print-}; shift
  case $h in
  .) o $print "$@" ;;
  *) o $print ssh -n -o BatchMode=yes $h "$@" ;;
  esac
} # }}}


function display-help # {{{
{
  [[ $1 == h ]] && {
    local self=${cmdname/-/ }
    print -- ${${${cmdhelp//\#c/$self}//#[[:space:]]#/}//%[[:space:]]#/}
    exit
  }
  o exec man 1 $cmdname
  exit # we get here in tests
} # }}}

function reject-misuse # {{{
{
  local val=${1-} self=${cmdname/-/ } ex=1
  case $val in
  -?)  print -f "%s: unknown option '%s'\n" -- $self $val ;;
  -?*) print -f "%s: unknown option '%s'\n" -- $self -$val ;;
  ?*)  print -f "%s: unknown argument '%s'\n" -- $self $val ;;
  '')  print -f "%s: missing argument\n" -- $self ;;
  esac
  print -f $msg_run_for_usage $self
  exit $ex
} # }}}

function complain # {{{
{
  local ex=0 severity=warning
  if [[ $1 == <-> ]]; then
    ex=$1
    severity=error
  fi
  shift
  print -u 2 -f "%s: %s\n" $severity $1
  if (( $# > 1 )); then
    print -u 2 -f "%s\n" "$@[2,-1]"
  fi
  return $ex
} # }}}

function redir # {{{
{
  local -i o0=0 o1=1 o2=2
  local optname OPTARG OPTIND
  while getopts 0:1:2: optname; do
    case $optname in
    0) exec {o0}<$OPTARG ;;
    1) exec {o1}>$OPTARG ;;
    2) exec {o2}>$OPTARG ;;
    esac
  done; shift $((OPTIND - 1))
  o "$@" <&${o0} 1>&${o1} 2>&${o2}
} # }}}

function O o # {{{
{
  local chatty=REPOSE_CHATTY
  local dryrun=REPOSE_DRYRUN
  local -i do_dryrun=0
  if [[ $1 == -n ]]; then
    shift
    do_dryrun=1
  fi
  if (( ${(P)#chatty} )); then
    if [[ "${(@j,%,)@}" == ${(P)~chatty} ]]; then
      print -ru $logfd -- $0 "${(q-)@}"
    fi
  fi
  if [[ $0 == o ]] && (( ${(P)#dryrun} )); then
    if [[ "${(@j,%,)@}" == ${(P)~dryrun} ]]; then
      do_dryrun=1
    fi
  fi
  if (( do_dryrun )); then
    return 0
  fi
  "$@"
} # }}}

declare -Tgx REPOSE_CHATTY repose_chatty \|
declare -Tgx REPOSE_DRYRUN repose_dryrun \|

declare -gir logfd=2

declare -gr msg_run_for_usage="run '%s -h' for usage instructions\n"
