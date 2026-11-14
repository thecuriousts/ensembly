#!/bin/sh
# Validate a releases/latest Location header against a release repo root; print the CLI tag.
# Usage: resolve_release_location.sh ROOT LOCATION
# Exit 0 and print tag on success; exit 1 on refuse; exit 2 on usage error.
set -eu

usage() {
  printf 'usage: %s ROOT LOCATION\n' "$(basename "$0")" >&2
  exit 2
}

[ $# -eq 2 ] || usage

release_repo_canonical() {
  printf '%s' "$1" | sed 's#/*$##'
}

release_repo_path() {
  root=$1
  after_host=${root#*://}
  case $after_host in
    */*)
      repo_path="/${after_host#*/}"
      repo_path=${repo_path%/}
      printf '%s' "$repo_path"
      ;;
    *)
      printf ''
      ;;
  esac
}

release_repo_scheme() {
  case $1 in
    https://*) printf '%s' https ;;
    http://*) printf '%s' http ;;
    *) printf '%s' '' ;;
  esac
}

valid_cli_release_tag() {
  printf '%s' "$1" | grep -Eq '^ensembly-cli--v[0-9]+\.[0-9]+\.[0-9]+$'
}

root=$(release_repo_canonical "$1")
loc=$2
repo_path=$(release_repo_path "$root")

case $loc in
  *://*@*)
    exit 1
    ;;
esac

case $loc in
  http://*|https://*)
    root_scheme=$(release_repo_scheme "$root")
    loc_scheme=$(release_repo_scheme "$loc")
    if [ "$root_scheme" = https ] && [ "$loc_scheme" = http ]; then
      exit 1
    fi
    prefix="${root}/releases/tag/"
    case $loc in
      "${prefix}"*)
        tag="${loc#"$prefix"}"
        ;;
      *)
        exit 1
        ;;
    esac
    ;;
  /*)
    if [ -n "$repo_path" ]; then
      prefix="${repo_path}/releases/tag/"
      case $loc in
        "${prefix}"*)
          tag="${loc#"$prefix"}"
          ;;
        *)
          exit 1
          ;;
      esac
    else
      case $loc in
        /releases/tag/*)
          tag="${loc#/releases/tag/}"
          ;;
        *)
          exit 1
          ;;
      esac
    fi
    ;;
  *)
    exit 1
    ;;
esac

if [ -z "$tag" ] || ! valid_cli_release_tag "$tag"; then
  exit 1
fi

printf '%s' "$tag"