#!/usr/bin/env bash
# Run in the approved Go source scope. Retain gofmt stderr and its exit status.
set -u
status=0
files=$(gofmt -l .) || status=$?
if ((status != 0)); then
  exit "$status"
fi

if [[ -n "$files" ]]; then
  printf '%s\n' "$files"
  exit 1
fi

printf '%s\n' 'format clean'
