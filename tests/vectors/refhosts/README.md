# Refhost record/replay regression vectors

Offline regression fixtures for `repose list-products` and `repose
list-repos`. Each `<label>/` directory is a **recorded** real reference host:
its raw discovery inputs plus the expected `repose` outputs. The Rust test
[`crates/repose-core/tests/refhost_vectors.rs`](../../../crates/repose-core/tests/refhost_vectors.rs)
replays every case **with no network** and asserts the rendered output matches.

This locks in the exact byte output — in particular the nested
`<codestream><name>` fix, where the friendly codestream name must not clobber
the canonical product `<name>` — so that regression is caught in CI.

## What each case contains

```
<label>/
  products.d/*.prod      # sanitized /etc/products.d/*.prod (base + addons)
  baseproduct-target     # basename the /etc/products.d/baseproduct symlink resolves to
  os-release             # sanitized /etc/os-release
  transactional          # "true" / "false" (transactional-update.conf probe)
  zypper-x-lr.xml        # OPTIONAL: sanitized `zypper -x lr` XML
  list-products.text     # expected output: `repose list-products`
  list-products.json     # expected output: `repose --format json list-products`
  list-products.yaml     # expected output: `repose list-products --yaml`
  list-repos.text        # expected output: `repose list-repos`        (iff zypper-x-lr.xml)
  list-repos.json        # expected output: `repose --format json list-repos` (ditto)
```

The replay feeds the inputs through `repose_core::parse_system` (products.d
present => SUSE path; base chosen by `baseproduct-target`; `transactional`
honored) and the exact CLI display functions, keyed by host `<label>:22`.

When `zypper-x-lr.xml` is present it additionally goes through
`repose_core::repo_parse::parse_repositories` — the same pure fn the live
`repose-ssh::host::read_repos` path calls — and the `list_repos` renderers,
compared byte-for-byte against `list-repos.{text,json}` (repo order in the XML
is deterministic, so no order caveat applies). Cases without the XML are
list-products-only; the list-repos half is skipped silently.

## Regenerating goldens: `UPDATE_VECTORS=1`

```
UPDATE_VECTORS=1 cargo test -p repose-core --test refhost_vectors
```

Instead of asserting, the test **writes** the freshly rendered outputs as the
expected files (`list-products.{text,json,yaml}` and, when `zypper-x-lr.xml`
exists, `list-repos.{text,json}`), printing one `updated <label>/<file>` line
per write, then asserts (trivially green). This is both the capture workflow
for new cases (a bare inputs-only `<label>/` gets its goldens generated) and
the intentional-change workflow (review the `git diff` before committing).

`REFHOST_VECTORS_ROOT=<dir>` overrides the corpus root, e.g. to trial a
capture in a scratch directory before adding it here.

## Capturing a new case: `scripts/capture-vector.sh`

```
scripts/capture-vector.sh <host> <label>
```

SSHes to `<host>` as root (BatchMode — your key must already be authorized)
and records all inputs into `tests/vectors/refhosts/<label>/`: `os-release`,
the `baseproduct` symlink-target basename, every `/etc/products.d/*.prod`,
the transactional-update.conf probe result, and the `zypper -x lr` XML. It
then sanitizes everything in place (rules below), verifies nothing internal
survived, and prints the `UPDATE_VECTORS=1` step to generate the goldens.
The script is idempotent: re-running a capture overwrites and re-sanitizes.

Full workflow:

1. `scripts/capture-vector.sh <host> <label>`
2. `UPDATE_VECTORS=1 cargo test -p repose-core --test refhost_vectors`
3. Review `git diff tests/vectors/refhosts/<label>` (inputs *and* generated
   goldens), confirm the sanitization grep below finds nothing.
4. Commit. The test auto-discovers the new directory; no code change needed.

## Sanitization (this repo is public)

The inputs are **real** refhost data, scrubbed so no internal infrastructure
leaks. `capture-vector.sh` applies these rules to `os-release`, every
`*.prod`, and `zypper-x-lr.xml`:

- Every refhost hostname is replaced by the neutral `<label>` (the `:22` port
  is kept). The label is the only host identity in the expected outputs.
- Every `http(s)://` URL keeps its **path** but has scheme+host replaced by
  `https://example.invalid` — path structure is preserved so distinct repos
  stay distinct (e.g. two update repos still differ in their `/ibs/...`
  suffix).
- `repoid="obsrepository://..."` becomes
  `obsrepository://example.invalid/scrubbed` (its path is internal
  build-service data, so it is fully scrubbed, not preserved).
- Any leftover internal hostname (`*.suse.cz`, `*.suse.de`, `qam.suse*`) —
  including inside URL paths — becomes `example.invalid`.
- Only **non-structural** bytes are touched: product-identity elements —
  depth-1 `<name>`, `<arch>`, `<version>`, `<baseversion>`, `<patchlevel>`,
  and the nested `<codestream><name>` — plus repo `alias`, `name`, `enabled`,
  and `autorefresh` attributes are left verbatim, because product identity and
  repo flags are the whole point of the fixture (and are public distro
  content). Exception: an alias that itself embeds an internal hostname is
  scrubbed — public-repo safety wins.

A grep for those internal-infrastructure identifiers must find nothing under
this directory (the capture script enforces this):

```
grep -rEn 'suse\.(cz|de)|qam\.suse' tests/vectors/refhosts/
```

## Addon-order caveat (list-products only)

The historical Python 2.1.0 implementation stored addons in a `frozenset`, so
the addon **ordering** in vectors recorded from it is not reproducible (it
varied with `PYTHONHASHSEED`); Rust emits a deterministic sorted order. The
test therefore:

- compares **single-addon** cases (`<= 1` addon) byte-for-byte, and
- compares **multi-addon** cases order-**insensitively** (sort the lines;
  whole `- name:` blocks for yaml), while additionally asserting the
  base-product structural line (text `Base product:`, json `"kind": "base"`,
  yaml `product:` block) verbatim and in place — that is the specific line the
  codestream bug corrupted.

The committed vectors keep whatever addon order the original recording run
emitted; only the comparison is order-insensitive. `list-repos` outputs have
no such caveat and are always compared byte-for-byte.
