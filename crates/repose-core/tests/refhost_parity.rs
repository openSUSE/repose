//! Offline record/replay parity corpus for `list-products`.
//!
//! Each `tests/oracle/refhosts/<label>/` directory holds sanitized real
//! refhost discovery inputs (`products.d/*.prod`, `baseproduct-target`,
//! `os-release`, `transactional`) plus the byte goldens captured from the
//! Python `repose` oracle (`list-products.{text,json,yaml}`), with the real
//! hostname neutralized to `<label>`.
//!
//! The test replays each case fully offline: it feeds the recorded inputs
//! through the exact same pipeline the CLI uses — [`parse_system`] to build the
//! `System`, then the [`TextDisplay`] / [`JsonDisplay`] / [`list_products_yaml`]
//! renderers keyed by host `<label>:22` — and compares against the goldens.
//! This locks in the list-products byte-parity (notably the nested
//! `<codestream><name>` fix) with no network access.
//!
//! Python stores addons in a `frozenset`, so its addon *ordering* is not
//! reproducible; Rust emits a deterministic sorted order. Cases with >= 2
//! addons are therefore compared order-insensitively (sort the lines), while
//! the base-product structural line — the one the codestream bug corrupted —
//! is additionally asserted verbatim and in place. Single-addon cases are
//! compared byte-for-byte.

use std::fs;
use std::path::{Path, PathBuf};

use repose_core::display::{list_products_yaml, CommandDisplay, JsonDisplay, TextDisplay};
use repose_core::product_parse::{parse_system, ProdFile};
use repose_core::types::System;

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/oracle/refhosts")
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

/// The base-product structural line/block for a format — the specific bytes the
/// nested `<codestream><name>` bug corrupted. Checked verbatim even in the
/// order-insensitive path.
fn base_marker(fmt: &str, s: &str) -> String {
    match fmt {
        "text" => s
            .lines()
            .find(|l| l.starts_with("  Base product:"))
            .expect("text base-product line")
            .to_string(),
        "json" => s
            .lines()
            .find(|l| l.contains("\"kind\": \"base\""))
            .expect("json base object")
            .to_string(),
        "yaml" => {
            let lines: Vec<&str> = s.lines().collect();
            let start = lines
                .iter()
                .position(|l| *l == "product:")
                .expect("yaml product: block");
            let mut block = vec![lines[start]];
            for l in &lines[start + 1..] {
                if l.starts_with(' ') {
                    block.push(l);
                } else {
                    break;
                }
            }
            block.join("\n")
        }
        other => panic!("unknown format {other}"),
    }
}

fn sorted_lines(s: &str) -> Vec<&str> {
    let mut v: Vec<&str> = s.lines().collect();
    v.sort_unstable();
    v
}

fn fail(label: &str, fmt: &str, mode: &str, expected: &str, actual: &str) -> ! {
    let exp = sorted_lines(expected);
    let act = sorted_lines(actual);
    let only_expected: Vec<&str> = exp.iter().filter(|l| !act.contains(*l)).copied().collect();
    let only_actual: Vec<&str> = act.iter().filter(|l| !exp.contains(*l)).copied().collect();
    panic!(
        "\nrefhost parity MISMATCH [{label}] {fmt} ({mode})\n\
         --- lines only in golden (expected) ---\n{}\n\
         --- lines only in rendered (actual) ---\n{}\n\
         --- full expected ---\n{expected}\n--- full actual ---\n{actual}",
        only_expected.join("\n"),
        only_actual.join("\n"),
    );
}

fn check(label: &str, fmt: &str, golden: &str, actual: &str, addon_count: usize) {
    // >= 2 addons: Python frozenset order is non-reproducible, so compare the
    // set of lines, then pin the base-product line verbatim. 0/1 addon: the
    // output is deterministic, so require exact byte-equality.
    if addon_count >= 2 {
        if sorted_lines(golden) != sorted_lines(actual) {
            fail(label, fmt, "sorted", golden, actual);
        }
        let (g, a) = (base_marker(fmt, golden), base_marker(fmt, actual));
        if g != a {
            panic!(
                "\nrefhost parity BASE-PRODUCT MISMATCH [{label}] {fmt}\n\
                 expected: {g:?}\n  actual: {a:?}"
            );
        }
    } else if golden != actual {
        fail(label, fmt, "raw", golden, actual);
    }
}

#[test]
fn refhost_list_products_parity() {
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
        let addon_count = sys.get_addons().len();

        let text_golden = fs::read_to_string(dir.join("list-products.text")).unwrap();
        let json_golden = fs::read_to_string(dir.join("list-products.json")).unwrap();
        let yaml_golden = fs::read_to_string(dir.join("list-products.yaml")).unwrap();

        check(
            &label,
            "text",
            &text_golden,
            &render_text(hostname, port, &sys),
            addon_count,
        );
        check(
            &label,
            "json",
            &json_golden,
            &render_json(hostname, port, &sys),
            addon_count,
        );
        check(
            &label,
            "yaml",
            &yaml_golden,
            &render_yaml(hostname, &sys),
            addon_count,
        );
    }
}
