#!/bin/sh
set -eu

backend_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
repository_root="$(CDPATH= cd -- "$backend_root/.." && pwd)"
cd "$backend_root"

if ! command -v buf >/dev/null 2>&1; then
  printf '%s\n' 'Buf CLI 1.50.0 is required to verify protobuf compatibility.' >&2
  exit 1
fi

buf_version="$(buf --version)"
if [ "$buf_version" != '1.50.0' ]; then
  printf '%s\n' 'The protobuf contract gate requires exactly Buf CLI 1.50.0.' >&2
  exit 1
fi

if ! git -C "$repository_root" show-ref --verify --quiet refs/heads/main; then
  printf '%s\n' 'The local main branch is required as the protobuf compatibility baseline.' >&2
  exit 1
fi

buf build
buf breaking --against "$repository_root/.git#branch=main,subdir=backend/platform/proto"
printf '%s\n' 'Protobuf compilation and FILE compatibility check passed against main.'
