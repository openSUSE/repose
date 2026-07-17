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

from repose.template import load_template
from repose.template.resolver import Repoq
from repose.target.parsers import Product
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
                k: [
                    {"name": x.name, "url": x.url, "refresh": x.refresh} for x in v
                ]
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
    print(f"oracle written under {ORACLE}")


if __name__ == "__main__":
    main()
