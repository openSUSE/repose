# Refhost `list-products` record/replay corpus

Offline regression fixtures for `repose list-products`. Each `<label>/`
directory is a **recorded** real reference host: its raw discovery inputs plus
the expected `repose` outputs. The Rust test
[`crates/repose-core/tests/refhost_parity.rs`](../../../crates/repose-core/tests/refhost_parity.rs)
replays every case **with no network** and asserts the rendered output matches.

This locks in the exact list-products byte output — in particular the nested
`<codestream><name>` fix, where the friendly codestream name must not clobber
the canonical product `<name>` — so that regression is caught in CI.

## What each case contains

```
<label>/
  products.d/*.prod      # sanitized /etc/products.d/*.prod (base + addons)
  baseproduct-target     # basename the /etc/products.d/baseproduct symlink resolves to
  os-release             # sanitized /etc/os-release
  transactional          # "true" / "false" (transactional-update.conf probe)
  list-products.text     # expected output: `repose list-products`
  list-products.json     # expected output: `repose --format json list-products`
  list-products.yaml     # expected output: `repose list-products --yaml`
```

The replay feeds the inputs through `repose_core::parse_system` (products.d
present => SUSE path; base chosen by `baseproduct-target`; `transactional`
honored) and the exact CLI display functions, keyed by host `<label>:22`.

## Sanitization (this repo is public)

The inputs are **real** refhost data, scrubbed so no internal infrastructure
leaks:

- Every refhost hostname is replaced by the neutral `<label>` (the `:22` port is
  kept). The label is the only host identity in the expected outputs.
- No `zypper -x lr` data is committed — it carries internal IBS URLs. This
  corpus is scoped to `list-products` only.
- Every `http(s)://` URL and internal host (the QA refhost domains, the
  download/build-service hosts, `obs*://…` URIs, and internal build-service
  paths) inside `*.prod` and `os-release` is replaced with
  `https://example.invalid/...`. Only **non-structural** bytes are
  touched: the elements the parser reads — depth-1 `<name>`, `<arch>`,
  `<version>`, `<baseversion>`, `<patchlevel>`, and the nested
  `<codestream><name>` — are left verbatim, because the product identity and
  codestream structure are the whole point of the fixture (and are public distro
  content). Scrubbing is verified not to change `parse_system` output.

A grep for those internal-infrastructure identifiers (the QA/build domains and
build-service paths the sanitizer strips) must find nothing under this
directory.

## Addon-order caveat

The historical Python 2.1.0 implementation stored addons in a `frozenset`, so
the addon **ordering** in vectors recorded from it is not reproducible (it
varied with `PYTHONHASHSEED`); Rust emits a deterministic sorted order. The
test therefore:

- compares **single-addon** cases (`<= 1` addon) byte-for-byte, and
- compares **multi-addon** cases order-**insensitively** (sort the lines),
  while additionally asserting the base-product structural line (text
  `Base product:`, json `"kind": "base"`, yaml `product:` block) verbatim and
  in place — that is the specific line the codestream bug corrupted.

The committed vectors keep whatever addon order the original recording run
emitted; only the comparison is order-insensitive.

## How to add a new case

Every future dogfood finding should become a recorded case:

1. Create `tests/vectors/refhosts/<label>/` with `products.d/*.prod`,
   `baseproduct-target`, `os-release`, and `transactional` captured from the
   host (read-only; see `capture_fixtures.sh` in the original snapshot).
2. Record the three expected outputs and substitute the real hostname with
   `<label>` (keep `:22`):
   - `list-products.text`  = `repose list-products`
   - `list-products.json`  = `repose --format json list-products`
   - `list-products.yaml`  = `repose list-products --yaml`
3. Sanitize per the rules above and confirm the `git grep` finds nothing.
4. `cargo test -p repose-core --test refhost_parity` — the test auto-discovers
   the new directory; no code change needed.
