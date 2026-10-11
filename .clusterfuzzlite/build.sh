#!/bin/bash -eu
# oss-fuzz/ClusterFuzzLite build contract: build every cargo-fuzz target and
# place the resulting binaries in $OUT. cargo-fuzz comes preinstalled in the
# base-builder-rust image and picks up fuzz/ automatically.
# jitterentropy requires -O0; the CFLAGS ClusterFuzzLite injects end in -O1 and win.
AWS_LC_SYS_NO_JITTER_ENTROPY=1 cargo fuzz build
find fuzz/target/x86_64-unknown-linux-gnu/release -maxdepth 1 -type f -executable ! -name '*.d' -exec cp -t "$OUT" {} +

# libFuzzer loads $OUT/<target>_seed_corpus.zip and $OUT/<target>.dict. The two
# file-format targets seed from the pinned refhost captures instead of a second
# copy, so their seeds cannot drift from the real inputs.
pack() {
  local target=$1
  shift
  python3 .clusterfuzzlite/pack_seeds.py "$OUT/${target}_seed_corpus.zip" "$@"
}

pack repo_xml tests/vectors/refhosts/*/zypper-x-lr.xml
pack prod_parse tests/vectors/refhosts/*/os-release \
  tests/vectors/refhosts/*/products.d
pack host_spec fuzz/seeds/host_spec
pack transform fuzz/seeds/transform

cp fuzz/dicts/*.dict "$OUT/"
