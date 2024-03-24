#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "$0")"

if ! which bindgen >/dev/null; then
  cargo install bindgen
fi

bindgen lfs_sys.h --use-core --no-layout-tests --rust-target nightly -- \
  -DLFS_THREADSAFE \
  -DLFS_NO_DEBUG \
  -DLFS_NO_WARN \
  -DLFS_NO_ERROR \
  -DLFS_NO_ASSERT \
  -I../lfs \
  -I../wasi-sdk/share/wasi-sysroot/include \
  -I../wasi-sdk/lib/clang/17/include \
  >../fs/bindings.rs
