//! Offline record/replay regression vectors for `list-products` and
//! `list-repos`.
//!
//! Each `tests/vectors/refhosts/<label>/` directory holds sanitized real
//! refhost discovery inputs (`products.d/*.prod`, `baseproduct-target`,
//! `os-release`, `transactional`, and optionally `zypper-x-lr.xml`) plus the
//! expected byte outputs (`list-products.{text,json,yaml}` and, when the
//! zypper XML input is present, `list-repos.{text,json}`), with the real
//! hostname neutralized to `<label>`.
//!
//! The test replays each case fully offline: it feeds the recorded inputs
//! through the exact same pipeline the CLI uses — [`parse_system`] to build the
//! `System`, then the [`TextDisplay`] / [`JsonDisplay`] / [`list_products_yaml`]
//! renderers keyed by host `<label>:22`; and, for cases that recorded a
//! `zypper -x lr` capture, [`parse_repositories`] (the same pure fn the live
//! `read_repos` path in `repose-ssh::host` calls) into the `list_repos`
//! renderers — and compares against the goldens. This locks in the
//! list-products byte-parity (notably the nested `<codestream><name>` fix)
//! and the list-repos rendering with no network access. Cases without
//! `zypper-x-lr.xml` simply skip the list-repos half.
//!
//! Golden regeneration (the capture workflow): run with `UPDATE_VECTORS=1`
//! and the test WRITES the freshly rendered outputs as the expected files
//! (printing one `updated <label>/<file>` line per write) before asserting —
//! trivially green. Drop sanitized inputs into a new `<label>/` directory,
//! run `UPDATE_VECTORS=1 cargo test -p repose-core --test refhost_vectors`,
//! review the diff, commit. `REFHOST_VECTORS_ROOT` overrides the corpus root
//! (useful for exercising regeneration on a scratch copy).
//!
//! Output ordering is deterministic (addons sorted by name/version/arch),
//! so every format is compared by exact byte equality.
//! (historical note: the old order-insensitive compare — sorted lines for
//! text/json (self-contained lines), and as a set of complete `- name:`
//! addon blocks for yaml — was only needed while goldens carried Python's
//! random frozenset order; goldens are now recorded in repose's own order.)
//! (so name/version cross-attribution between addons
//! still fails) — while the base-product structural line, the one the
//! codestream bug corrupted, is additionally asserted verbatim and at its
//! exact line position, and total byte length plus trailing newline must
//! always match. Zero/single-addon cases are compared byte-for-byte.

use std::fs;
use std::path::{Path, PathBuf};

use repose_core::display::{list_products_yaml, CommandDisplay, JsonDisplay, TextDisplay};
use repose_core::product_parse::{parse_system, ProdFile};
use repose_core::repo_parse::parse_repositories;
use repose_core::types::{Repository, System};

fn corpus_root() -> PathBuf {
    match std::env::var_os("REFHOST_VECTORS_ROOT") {
        Some(root) => PathBuf::from(root),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors/refhosts"),
    }
}

/// `UPDATE_VECTORS=1` switches the run from asserting to regenerating goldens.
fn update_mode() -> bool {
    std::env::var("UPDATE_VECTORS").as_deref() == Ok("1")
}

/// Read the golden `file` for a case. In UPDATE mode, first overwrite it with
/// the freshly `rendered` bytes (announcing the write), so the subsequent
/// comparison is trivially green and a brand-new `<label>/` needs no
/// hand-written goldens.
fn golden(dir: &Path, label: &str, file: &str, rendered: &str) -> String {
    let path = dir.join(file);
    if update_mode() {
        fs::write(&path, rendered).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        println!("updated {label}/{file}");
    }
    fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "golden {}: {e}\n(hint: UPDATE_VECTORS=1 regenerates expected outputs)",
            path.display()
        )
    })
}

/// Replicates `commands::list_cmd::split_key`: `host:port`, defaulting to 22.
fn split_key(key: &str) -> (&str, u16) {
    if let Some((h, p)) = key.rsplit_once(':') {
        if let Ok(port) = p.parse() {
            return (h, port);
        }
    }
    (key, 22)
}

/// Load one recorded case and run it through `parse_system`, mirroring the SSH
/// path in `repose-ssh::host::discover_system` (products.d present => SUSE path,
/// base chosen by the `baseproduct` symlink target, transactional honored).
fn replay(dir: &Path) -> System {
    let base_link = fs::read_to_string(dir.join("baseproduct-target"))
        .expect("baseproduct-target")
        .trim()
        .to_string();
    let base_file = base_link
        .rsplit_once('/')
        .map_or(base_link.as_str(), |(_, t)| t);
    let base_xml = fs::read_to_string(dir.join("products.d").join(base_file))
        .unwrap_or_else(|_| panic!("base .prod {base_file} in {}", dir.display()));
    let transactional = fs::read_to_string(dir.join("transactional"))
        .expect("transactional")
        .trim()
        == "true";

    let mut paths: Vec<PathBuf> = fs::read_dir(dir.join("products.d"))
        .expect("products.d")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "prod"))
        .collect();
    paths.sort();
    let prod_files: Vec<ProdFile> = paths
        .iter()
        .map(|p| ProdFile {
            filename: p.file_name().unwrap().to_string_lossy().into_owned(),
            xml: fs::read_to_string(p).ok(),
        })
        .collect();

    parse_system(
        Some(&prod_files),
        Some(&base_link),
        Some(&base_xml),
        None,
        transactional,
    )
    .expect("parse_system on recorded refhost inputs")
}

fn render_text(hostname: &str, port: u16, sys: &System) -> String {
    let mut out = Vec::new();
    TextDisplay { output: &mut out }
        .list_products(hostname, port, sys)
        .unwrap();
    String::from_utf8(out).unwrap()
}

fn render_json(hostname: &str, port: u16, sys: &System) -> String {
    let mut out = Vec::new();
    JsonDisplay { output: &mut out }
        .list_products(hostname, port, sys)
        .unwrap();
    String::from_utf8(out).unwrap()
}

fn render_yaml(hostname: &str, sys: &System) -> String {
    let mut out = Vec::new();
    list_products_yaml(&mut out, hostname, sys).unwrap();
    String::from_utf8(out).unwrap()
}

fn render_repos_text(hostname: &str, port: u16, repos: &[Repository]) -> String {
    let mut out = Vec::new();
    TextDisplay { output: &mut out }
        .list_repos(hostname, port, repos)
        .unwrap();
    String::from_utf8(out).unwrap()
}

fn render_repos_json(hostname: &str, port: u16, repos: &[Repository]) -> String {
    let mut out = Vec::new();
    JsonDisplay { output: &mut out }
        .list_repos(hostname, port, repos)
        .unwrap();
    String::from_utf8(out).unwrap()
}

/// The base-product structural line/block for a format — the specific bytes
/// the nested `<codestream><name>` bug corrupted — plus its LINE POSITION, so
/// a rendering that emits the right bytes in the wrong place (e.g. base after
/// the addons in json) still fails. Checked even in the order-insensitive
/// path: addon reordering never moves the base marker (text/json put the base
/// first; in yaml every addon occupies the same number of lines regardless of
/// order, and `product:` follows the whole addons section).
fn fail(label: &str, fmt: &str, expected: &str, actual: &str) -> ! {
    panic!(
        "\nrefhost vector MISMATCH [{label}] {fmt}\n\
         --- expected ---\n{expected}\n--- actual ---\n{actual}"
    );
}

fn check(label: &str, fmt: &str, golden: &str, actual: &str) {
    // Deterministic output is the contract (addons are emitted in sorted
    // order), so every format is compared by exact byte equality.
    if golden != actual {
        fail(label, fmt, golden, actual);
    }
}

#[test]
fn refhost_vectors_parity() {
    let root = corpus_root();
    let mut labels: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap_or_else(|_| panic!("corpus root {}", root.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    labels.sort();
    assert!(
        !labels.is_empty(),
        "no refhost cases under {}",
        root.display()
    );

    for dir in &labels {
        let label = dir.file_name().unwrap().to_string_lossy().into_owned();
        let key = format!("{label}:22");
        let (hostname, port) = split_key(&key);

        let sys = replay(dir);

        let text = render_text(hostname, port, &sys);
        let json = render_json(hostname, port, &sys);
        let yaml = render_yaml(hostname, &sys);

        let text_golden = golden(dir, &label, "list-products.text", &text);
        let json_golden = golden(dir, &label, "list-products.json", &json);
        let yaml_golden = golden(dir, &label, "list-products.yaml", &yaml);

        check(&label, "text", &text_golden, &text);
        check(&label, "json", &json_golden, &json);
        check(&label, "yaml", &yaml_golden, &yaml);

        // list-repos half: only for cases that recorded `zypper -x lr` XML
        // (older cases are list-products-only and skip this silently). The
        // XML goes through `parse_repositories` — the same pure fn the live
        // `repose-ssh::host::read_repos` path feeds — and repo order from the
        // XML is deterministic, so the compare is raw byte-equality
        let lr_path = dir.join("zypper-x-lr.xml");
        if lr_path.exists() {
            let lr_xml = fs::read_to_string(&lr_path)
                .unwrap_or_else(|e| panic!("read {}: {e}", lr_path.display()));
            let repos = parse_repositories(&lr_xml);

            let repos_text = render_repos_text(hostname, port, &repos);
            let repos_json = render_repos_json(hostname, port, &repos);

            let repos_text_golden = golden(dir, &label, "list-repos.text", &repos_text);
            let repos_json_golden = golden(dir, &label, "list-repos.json", &repos_json);

            check(&label, "repos-text", &repos_text_golden, &repos_text);
            check(&label, "repos-json", &repos_json_golden, &repos_json);
        }
    }
}
