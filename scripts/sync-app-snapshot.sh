#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 /path/to/private/app" >&2
  exit 2
fi

source_dir="${1%/}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${repo_root}/app"

if [ ! -d "${source_dir}" ]; then
  echo "source app directory not found: ${source_dir}" >&2
  exit 1
fi

mkdir -p "${target_dir}"

rsync -a --delete \
  --exclude '.git' \
  --exclude 'node_modules' \
  --exclude 'dist' \
  --exclude '.env' \
  --exclude '.env.*' \
  "${source_dir}/" \
  "${target_dir}/"

echo "synced app snapshot from ${source_dir} to ${target_dir}"
