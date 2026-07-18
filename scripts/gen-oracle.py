#!/usr/bin/env python3
"""Regenerate tests/oracle/** goldens from the Python repose reference.

Run from repository root with the Python package importable:

    uv run python scripts/gen-oracle.py
"""

from __future__ import annotations

import json
import shlex
import tempfile
from pathlib import Path
from urllib.parse import urlparse

from types import SimpleNamespace

from repose.template import load_template
from repose.template.resolver import Repoq
from repose.target.parsers import Product
import repose.target.parsers.product as _product_mod
from repose.target.parsers.repository import parse_repositories
from repose.command.remove import Remove
from repose.types.repa import Repa
from repose.types.refhost.transformations import transform_version_partialy

ROOT = Path(__file__).resolve().parents[1]
ORACLE = ROOT / "tests" / "oracle"


def main() -> None:
    ORACLE.mkdir(parents=True, exist_ok=True)
    for sub in (
        "repa",
        "shell",
        "repoq",
        "template",
        "transform",
        "hostparse",
        "ndjson",
        "product",
        "zypper_lr",
        "remove_match",
    ):
        (ORACLE / sub).mkdir(exist_ok=True)

    # REPA
    repa_cases = []
    for s in [
        "SLES",
        "SLES:15-SP3",
        "SLES:15-SP3:x86_64",
        "SLES:15-SP3:x86_64:update",
        "SLES:15",
        "SLES::x86_64",
        "prod:",
        ":ver",
        "a:b:c:d",
        "",
        "a:b:c:d:e",
    ]:
        try:
            r = Repa(s)
            repa_cases.append(
                {
                    "input": s,
                    "ok": True,
                    "product": r.product,
                    "version": r.version,
                    "arch": r.arch,
                    "repo": r.repo,
                    "baseversion": r.baseversion,
                    "smallver": r.smallver,
                }
            )
        except ValueError as e:
            repa_cases.append({"input": s, "ok": False, "error": str(e)})
    (ORACLE / "repa" / "parse.json").write_text(
        json.dumps(repa_cases, indent=2) + "\n", encoding="utf-8"
    )

    # shell
    shell_inputs = [
        "",
        "simple",
        "evil repo's name",
        "http://mirror.example.com/dist path/?foo=1&bar=2",
        "evil alias's",
        "prod uct's",
        "a b",
        "a'b",
        "$HOME",
        "foo;bar",
        "x\ny",
        "*",
    ]
    (ORACLE / "shell" / "quote.json").write_text(
        json.dumps(
            [{"input": s, "quoted": shlex.quote(s)} for s in shell_inputs],
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    join_parts = ["zypper", "-n", "rr", "evil alias's", "other"]
    (ORACLE / "shell" / "join.json").write_text(
        json.dumps({"parts": join_parts, "joined": shlex.join(join_parts)}, indent=2)
        + "\n",
        encoding="utf-8",
    )
    evil_name = "evil repo's name"
    evil_url = "http://mirror.example.com/dist path/?foo=1&bar=2"
    evil_alias = "evil alias's"
    evil_product = "prod uct's"
    cmds = {
        "add_ckn": [
            "zypper",
            "-n",
            "ar",
            "-ckn",
            evil_name,
            evil_url,
            evil_name,
        ],
        "add_cfkn": [
            "zypper",
            "-n",
            "ar",
            "-cfkn",
            evil_name,
            evil_url,
            evil_name,
        ],
        "rr": ["zypper", "-n", "rr", evil_alias],
        "in": ["zypper", "-n", "in", "-t", "product", "-l", "-f", evil_product],
        "in_t": [
            "transactional-update",
            "-n",
            "pkg",
            "in",
            "-t",
            "product",
            "-l",
            "-f",
            evil_product,
        ],
        "rm": ["zypper", "-n", "rm", "-t", "product", evil_product],
        "rm_t": [
            "transactional-update",
            "-n",
            "pkg",
            "rm",
            "-t",
            "product",
            "-l",
            "-f",
            evil_product,
        ],
    }
    (ORACLE / "shell" / "command_templates.json").write_text(
        json.dumps({k: shlex.join(v) for k, v in cmds.items()}, indent=2) + "\n",
        encoding="utf-8",
    )

    # transform
    tf = [
        {"input": v, "output": transform_version_partialy(v)}
        for v in ["15-SP3", "15.3", "15", "ALL", "tumbleweed", "3.19.1", ""]
    ]
    (ORACLE / "transform" / "version.json").write_text(
        json.dumps(tf, indent=2) + "\n", encoding="utf-8"
    )

    yaml_text = """
SLES:
  "15-SP3":
    pool:
      url: http://example.com/$version/$arch/pool/
    update:
      url: http://example.com/${version}/$arch/update/
      enabled: true
    default_repos:
      - update
      - pool
  "15":
    pool:
      url: http://example.com/$shortver/$arch/
    default_repos:
      - pool
QA:
  "15-SP3":
    tools:
      url: http://qa/$version/
    default_repos:
      - tools
"""
    (ORACLE / "template" / "sample.yml").write_text(yaml_text, encoding="utf-8")
    td = Path(tempfile.mkdtemp())
    p = td / "products.yml"
    p.write_text(yaml_text, encoding="utf-8")
    tpl = load_template(p)
    rq = Repoq(tpl)
    base = Product("SLES", "15-SP3", "x86_64")
    repoq_cases = []
    for repa_s in [
        "SLES:15-SP3:x86_64:update",
        "SLES:15-SP3:x86_64",
        "SLES:15-SP9:x86_64",
        "SLES:15-SP3:x86_64:missing",
        "NOPE:1",
    ]:
        try:
            r = rq.solve_repa(Repa(repa_s), base)
            ser = {
                k: [{"name": x.name, "url": x.url, "refresh": x.refresh} for x in v]
                for k, v in r.items()
            }
            repoq_cases.append({"repa": repa_s, "ok": True, "result": ser})
        except Exception as e:  # noqa: BLE001 — oracle capture
            repoq_cases.append({"repa": repa_s, "ok": False, "error": str(e)})
    (ORACLE / "repoq" / "solve_repa.json").write_text(
        json.dumps({"template": yaml_text, "cases": repoq_cases}, indent=2) + "\n",
        encoding="utf-8",
    )

    host_cases = []
    for arg in [
        "example.com",
        "alice@example.com",
        "admin@example.com:2222",
        "root@host",
        "bad:port",
        "host:99999",
    ]:
        try:
            x = urlparse(f"//{arg}")
            hostname = x.hostname or ""
            if x.port:
                keyname = f"{hostname}:{x.port}"
                port = x.port
            else:
                keyname = hostname
                port = 22
            username = x.username if x.username else "root"
            host_cases.append(
                {
                    "input": arg,
                    "ok": True,
                    "key": keyname,
                    "hostname": hostname,
                    "port": port,
                    "username": username,
                }
            )
        except ValueError:
            host_cases.append({"input": arg, "ok": False})
    (ORACLE / "hostparse" / "hosts.json").write_text(
        json.dumps(host_cases, indent=2) + "\n", encoding="utf-8"
    )

    ndjson = [
        {"event": "dry", "level": "info", "host": "h", "cmd": "zypper -n lr"},
        {
            "event": "report",
            "level": "error",
            "host": "h",
            "line": "fail",
            "ok": False,
        },
        {"event": "known_product", "name": "SLES"},
    ]
    (ORACLE / "ndjson" / "events.jsonl").write_text(
        "\n".join(json.dumps(x) for x in ndjson) + "\n", encoding="utf-8"
    )

    # product parsers (pure .prod XML + os-release). Module-level dunder names
    # are not class-mangled, so getattr fetches the private parsers directly.
    parse_product = getattr(_product_mod, "__parse_product")
    parse_os_release = getattr(_product_mod, "__parse_os_release")
    # Only realistic cases where the Rust pure parser matches Python. The
    # baseversion-without-patchlevel edge diverges (documented in
    # product_parse.rs) and is intentionally omitted.
    prod_xml_cases = [
        (
            "base_sp",
            "SLES.prod",
            "<product><name>SLES</name><arch>x86_64</arch><baseversion>15</baseversion><patchlevel>3</patchlevel></product>",
        ),
        (
            "patchlevel_0",
            "X.prod",
            "<product><name>SLES</name><arch>x86_64</arch><baseversion>15</baseversion><patchlevel>0</patchlevel></product>",
        ),
        (
            "simple_version",
            "os.prod",
            "<product><name>openSUSE</name><arch>x86_64</arch><version>15.5</version></product>",
        ),
        (
            "caasp_all",
            "CAASP.prod",
            "<product><name>CAASP</name><arch>x86_64</arch><version>4.0</version></product>",
        ),
        (
            "addon_module",
            "m.prod",
            "<product><name>sle-module-basesystem</name><arch>x86_64</arch><baseversion>15</baseversion><patchlevel>3</patchlevel></product>",
        ),
        (
            "missing_name",
            "b.prod",
            "<product><arch>x86_64</arch><version>1.0</version></product>",
        ),
        (
            "missing_arch",
            "b.prod",
            "<product><name>bad</name><version>1.0</version></product>",
        ),
        (
            "empty_name",
            "b.prod",
            "<product><name/><arch>x86_64</arch><version>1.0</version></product>",
        ),
        (
            "missing_version",
            "b.prod",
            "<product><name>bad</name><arch>x86_64</arch></product>",
        ),
        (
            "empty_baseversion",
            "b.prod",
            "<product><name>bad</name><arch>x86_64</arch><baseversion/><patchlevel>3</patchlevel></product>",
        ),
    ]
    prod_cases = []
    for name, fname, xml in prod_xml_cases:
        res = parse_product([xml.encode()], fname)
        expected = (
            None if res is None else {"name": res[0], "version": res[1], "arch": res[2]}
        )
        prod_cases.append(
            {
                "name": name,
                "input": {"filename": fname, "xml": xml},
                "expected": expected,
            }
        )
    (ORACLE / "product" / "parse_prod.json").write_text(
        json.dumps(prod_cases, indent=2) + "\n", encoding="utf-8"
    )

    os_cases_in = [
        ("ubuntu", 'NAME="Ubuntu"\nVERSION_ID="22.04"\nID=ubuntu\nID_LIKE=debian\n'),
        ("fedora_arch", "ID=fedora\nVERSION_ID=40\nARCHITECTURE=aarch64\n"),
        ("bare_line_skipped", "ID=fedora\nVERSION_ID=40\nARCHITECTURE\n"),
        ("no_id_defaults_linux", 'PRETTY_NAME="Mystery OS"\n'),
        ("quote_strip_keeps_value", "ID=Xubuntu\nVERSION_ID=22.04\n"),
        ("sles_full", 'ID="sles"\nVERSION_ID="15-SP6"\nARCHITECTURE="x86_64"\n'),
    ]
    os_cases = []
    for name, text in os_cases_in:
        n, v, a = parse_os_release([text.encode()], "h")
        os_cases.append(
            {
                "name": name,
                "input": {"text": text},
                "expected": {"name": n, "version": v, "arch": a},
            }
        )
    (ORACLE / "product" / "os_release.json").write_text(
        json.dumps(os_cases, indent=2) + "\n", encoding="utf-8"
    )

    # zypper -x lr XML parser. Realistic cases match the Rust parser; malformed
    # XML is a documented delta (Python raises, Rust tolerates → empty).
    repo_xml_cases = [
        (
            "two_repos_mixed_state",
            '<?xml version="1.0"?><stream><repo-list><repo alias="repo-one" name="Repo One" enabled="1" autorefresh="0" gpgcheck="1"><url>http://example.com/one/</url></repo><repo alias="repo-two" name="Repo Two" enabled="0" autorefresh="0" gpgcheck="1"><url>http://example.com/two/</url></repo></repo-list></stream>',
        ),
        (
            "enabled_two_is_false",
            '<stream><repo-list><repo alias="a" name="A" enabled="2"><url>http://e</url></repo></repo-list></stream>',
        ),
        (
            "missing_optional_attrs",
            '<stream><repo-list><repo alias="min" name="Min" enabled="1"><url>http://min/</url></repo></repo-list></stream>',
        ),
        (
            "missing_alias_skipped",
            '<stream><repo-list><repo name="Bad" enabled="1"><url>http://e</url></repo><repo alias="good" name="Good" enabled="1"><url>http://example.com/good/</url></repo></repo-list></stream>',
        ),
        ("empty_repo_list", "<stream><repo-list></repo-list></stream>"),
        (
            "url_attr_ignored_child_used",
            '<stream><repo-list><repo alias="a" name="A" enabled="1" url="http://attr-ignored/"><url>http://child/</url></repo></repo-list></stream>',
        ),
        (
            "empty_and_missing_url_skipped",
            '<stream><repo-list><repo alias="e" name="E" enabled="1"><url/></repo><repo alias="nou" name="NoUrl" enabled="1"></repo><repo alias="good" name="Good" enabled="1"><url>http://example.com/good/</url></repo></repo-list></stream>',
        ),
        ("malformed_xml", "not <xml"),
    ]
    repo_cases = []
    for name, xml in repo_xml_cases:
        try:
            repos = parse_repositories(xml)
        except Exception:  # noqa: BLE001 — ET.ParseError; Rust tolerates → []
            repo_cases.append({"name": name, "xml": xml, "raises": True})
            continue
        expected = sorted(
            (
                {"alias": r.alias, "name": r.name, "url": r.url, "state": r.state}
                for r in repos
            ),
            key=lambda d: (d["alias"], d["name"], d["url"], d["state"]),
        )
        repo_cases.append({"name": name, "xml": xml, "expected": expected})
    (ORACLE / "zypper_lr" / "parse.json").write_text(
        json.dumps(repo_cases, indent=2) + "\n", encoding="utf-8"
    )

    # remove alias-match (`_calculate_repolist`): call the real oracle method
    # with a fake self so it can never drift from the ported logic.
    def _repolist(patterns, aliases):
        fake = SimpleNamespace(
            targets={
                "h": SimpleNamespace(repos=SimpleNamespace(keys=lambda: list(aliases)))
            }
        )
        return sorted(Remove._calculate_repolist(fake, "h", set(patterns)))

    remove_cases_in = [
        (
            "exact_match_single",
            ["SLES:15-SP4::repo1"],
            ["SLES:15-SP4::repo1", "SLES:15-SP4::repo2", "other:repo"],
        ),
        (
            "exact_not_prefix_repo10",
            ["SLES:15-SP3::repo1"],
            [
                "SLES:15-SP3::repo1",
                "SLES:15-SP3::repo10",
                "SLES:15-SP3::repo1-debuginfo",
            ],
        ),
        (
            "double_colon_matches_product",
            ["SLES:15-SP4::"],
            ["SLES:15-SP4::repo1", "SLES:15-SP4::repo10", "other:repo"],
        ),
        (
            "double_colon_no_match",
            ["QA:15-SP4::"],
            ["SLES:15-SP4::repo1", "other:repo"],
        ),
        ("exact_no_match", ["SLES:15-SP4::repo-missing"], ["SLES:15-SP4::repo-other"]),
        (
            "multiple_patterns_union",
            ["SLES:15-SP4::repo1", "SLES:15-SP4::repo2"],
            ["SLES:15-SP4::repo1", "SLES:15-SP4::repo2", "SLES:15-SP4::repo3"],
        ),
        (
            "mixed_exact_and_double_colon",
            ["SLES:15-SP4::", "QA:15-SP4::tools"],
            [
                "SLES:15-SP4::repo1",
                "SLES:15-SP4::repo2",
                "QA:15-SP4::tools",
                "QA:15-SP4::tools-extra",
            ],
        ),
        (
            "substring_is_contains_not_prefix",
            ["SLES:15-SP4::"],
            ["mirror-SLES:15-SP4::repo", "SLES:15-SP4::repo1"],
        ),
        ("empty_patterns", [], ["SLES:15-SP4::repo1"]),
        ("empty_aliases", ["SLES:15-SP4::repo1"], []),
    ]
    remove_cases = [
        {
            "name": name,
            "patterns": patterns,
            "aliases": aliases,
            "expected": _repolist(patterns, aliases),
        }
        for name, patterns, aliases in remove_cases_in
    ]
    (ORACLE / "remove_match" / "repolist.json").write_text(
        json.dumps(remove_cases, indent=2) + "\n", encoding="utf-8"
    )

    print(f"oracle written under {ORACLE}")


if __name__ == "__main__":
    main()
