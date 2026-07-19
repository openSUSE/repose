//! Offline record/replay parity corpus for `list-products`.
//!
//! Each `tests/vectors/refhosts/<label>/` directory holds sanitized real
//! refhost discovery inputs (`products.d/*.prod`, `baseproduct-target`,
//! `os-release`, `transactional`) plus the expected byte outputs
//! (`list-products.{text,json,yaml}`), with the real hostname neutralized
//! to `<label>`.
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
//! addons are therefore compared order-insensitively — as sorted lines for
//! text/json (self-contained lines), and as a set of complete `- name:`
//! addon blocks for yaml (so name/version cross-attribution between addons
//! still fails) — while the base-product structural line, the one the
//! codestream bug corrupted, is additionally asserted verbatim and at its
//! exact line position, and total byte length plus trailing newline must
//! always match. Zero/single-addon cases are compared byte-for-byte.

use std::fs;
use std::path::{Path, PathBuf};

use repose_core::display::{list_products_yaml, CommandDisplay, JsonDisplay, TextDisplay};
use repose_core::product_parse::{parse_system, ProdFile};
use repose_core::types::System;

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors/refhosts")
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

/// The base-product structural line/block for a format — the specific bytes
/// the nested `<codestream><name>` bug corrupted — plus its LINE POSITION, so
/// a rendering that emits the right bytes in the wrong place (e.g. base after
/// the addons in json) still fails. Checked even in the order-insensitive
/// path: addon reordering never moves the base marker (text/json put the base
/// first; in yaml every addon occupies the same number of lines regardless of
/// order, and `product:` follows the whole addons section).
fn base_marker(fmt: &str, s: &str) -> (usize, String) {
    match fmt {
        "text" => {
            let (pos, line) = s
                .lines()
                .enumerate()
                .find(|(_, l)| l.starts_with("  Base product:"))
                .expect("text base-product line");
            (pos, line.to_string())
        }
        "json" => {
            let (pos, line) = s
                .lines()
                .enumerate()
                .find(|(_, l)| l.contains("\"kind\": \"base\""))
                .expect("json base object");
            (pos, line.to_string())
        }
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
            (start, block.join("\n"))
        }
        other => panic!("unknown format {other}"),
    }
}

/// Split a rendered YAML document into (head, addon blocks, tail): head is
/// everything through the `addons:` key, each block is one complete
/// `- name:`-rooted addon entry (its lines in order), tail is everything from
/// the first following top-level key (`arch:`) onward.
fn split_yaml_addons(s: &str) -> (Vec<&str>, Vec<String>, Vec<&str>) {
    let lines: Vec<&str> = s.lines().collect();
    let Some(start) = lines.iter().position(|l| *l == "addons:") else {
        // "addons: []" (or no addons key at all): nothing to chunk.
        return (lines, vec![], vec![]);
    };
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut i = start + 1;
    while i < lines.len() {
        let l = lines[i];
        if l.starts_with("- ") {
            blocks.push(vec![l]);
        } else if l.starts_with(' ') {
            blocks
                .last_mut()
                .expect("addon continuation line before any '- name:' entry")
                .push(l);
        } else {
            break;
        }
        i += 1;
    }
    (
        lines[..=start].to_vec(),
        blocks.into_iter().map(|b| b.join("\n")).collect(),
        lines[i..].to_vec(),
    )
}

/// Order-insensitive yaml compare that keeps addon entries INTACT: the head
/// and tail must match verbatim, and the addons section must contain the same
/// SET of complete `- name:` blocks (each block's internal line order
/// preserved), so cross-attributing one addon's version to another's name
/// fails even though the flat line multiset would be identical.
fn yaml_addon_set_eq(golden: &str, actual: &str) -> bool {
    let (g_head, mut g_blocks, g_tail) = split_yaml_addons(golden);
    let (a_head, mut a_blocks, a_tail) = split_yaml_addons(actual);
    g_blocks.sort_unstable();
    a_blocks.sort_unstable();
    g_head == a_head && g_blocks == a_blocks && g_tail == a_tail
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
    // Whole-file shape first: byte length and trailing newline must always
    // match — `lines()`-based comparison alone would miss a lost final `\n`
    // or duplicated/merged whitespace.
    assert_eq!(
        golden.len(),
        actual.len(),
        "\nrefhost parity BYTE-LENGTH MISMATCH [{label}] {fmt}\n\
         --- full expected ---\n{golden}\n--- full actual ---\n{actual}"
    );
    assert_eq!(
        golden.ends_with('\n'),
        actual.ends_with('\n'),
        "refhost parity TRAILING-NEWLINE MISMATCH [{label}] {fmt}"
    );

    // >= 2 addons: Python frozenset order is non-reproducible, so compare
    // order-insensitively — whole addon blocks for yaml, lines for text/json
    // (whose lines are self-contained) — then pin the base-product marker
    // verbatim AND at its position. 0/1 addon: the output is deterministic,
    // so require exact byte-equality.
    if addon_count >= 2 {
        if fmt == "yaml" {
            if !yaml_addon_set_eq(golden, actual) {
                fail(label, fmt, "yaml-blocks", golden, actual);
            }
        } else if sorted_lines(golden) != sorted_lines(actual) {
            fail(label, fmt, "sorted", golden, actual);
        }
        let ((g_pos, g), (a_pos, a)) = (base_marker(fmt, golden), base_marker(fmt, actual));
        if g != a || g_pos != a_pos {
            panic!(
                "\nrefhost parity BASE-PRODUCT MISMATCH [{label}] {fmt}\n\
                 expected (line {g_pos}): {g:?}\n  actual (line {a_pos}): {a:?}"
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
