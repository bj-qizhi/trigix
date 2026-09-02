#!/usr/bin/env python3
"""Validate deterministic, non-secret release inputs and emit bounded evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LOCK_FILES = (
    "Cargo.lock",
    "apps/web/package-lock.json",
    "services/browser-runtime/package-lock.json",
    "services/ai-runtime/requirements.lock",
    "services/ai-runtime/requirements-test.lock",
    "sdk/python/requirements.lock",
)


def fail(message: str) -> None:
    raise SystemExit(f"release input verification failed: {message}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def chart_app_version() -> str:
    chart = (ROOT / "charts/trigix/Chart.yaml").read_text(encoding="utf-8")
    match = re.search(r'^appVersion:\s*["\']?([^"\'\s]+)', chart, re.MULTILINE)
    if not match:
        fail("charts/trigix/Chart.yaml has no appVersion")
    return match.group(1)


def verify() -> dict[str, object]:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    workspace_version = cargo["workspace"]["package"]["version"]
    web_package = json.loads((ROOT / "apps/web/package.json").read_text(encoding="utf-8"))
    web_lock = json.loads((ROOT / "apps/web/package-lock.json").read_text(encoding="utf-8"))
    browser_package = json.loads(
        (ROOT / "services/browser-runtime/package.json").read_text(encoding="utf-8")
    )
    browser_lock = json.loads(
        (ROOT / "services/browser-runtime/package-lock.json").read_text(encoding="utf-8")
    )
    desktop_config = json.loads(
        (ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8")
    )
    versions = {
        "rust_workspace": workspace_version,
        "web": web_package["version"],
        "web_lock": web_lock["packages"][""]["version"],
        "browser_runtime": browser_package["version"],
        "browser_runtime_lock": browser_lock["packages"][""]["version"],
        "desktop": desktop_config["version"],
        "helm_app": chart_app_version(),
    }
    if len(set(versions.values())) != 1:
        fail(f"product versions differ: {versions}")

    values = (ROOT / "charts/trigix/values.yaml").read_text(encoding="utf-8")
    if re.search(r'^\s*tag:\s*["\']?latest["\']?\s*$', values, re.MULTILINE):
        fail("Helm application images must not default to latest")

    templates = (
        "charts/trigix/templates/deployment.yaml",
        "charts/trigix/templates/ai-runtime-deployment.yaml",
        "charts/trigix/templates/executor-deployment.yaml",
    )
    for relative in templates:
        content = (ROOT / relative).read_text(encoding="utf-8")
        if "default .Chart.AppVersion" not in content:
            fail(f"{relative} does not default its image to Chart.appVersion")

    publish_workflow = (ROOT / ".github/workflows/publish-helm-chart.yml").read_text(
        encoding="utf-8"
    )
    if re.search(r"deploys app \d+\.\d+\.\d+", publish_workflow):
        fail("Helm release notes contain a hard-coded application version")

    lock_digests: dict[str, str] = {}
    for relative in LOCK_FILES:
        path = ROOT / relative
        if not path.is_file():
            fail(f"missing lock file: {relative}")
        if relative.endswith("requirements.lock"):
            locked = path.read_text(encoding="utf-8")
            if "--hash=sha256:" not in locked:
                fail(f"Python lock has no package hashes: {relative}")
        lock_digests[relative] = sha256(path)

    return {
        "schema": "trigix.release-quality-inputs.v1",
        "product_version": workspace_version,
        "lock_sha256": lock_digests,
        "helm_application_tag_default": workspace_version,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    parser.add_argument("--expected-version")
    args = parser.parse_args()
    evidence = verify()
    if args.expected_version and evidence["product_version"] != args.expected_version:
        fail(
            "expected product version "
            f"{args.expected_version}, found {evidence['product_version']}"
        )
    encoded = json.dumps(evidence, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")


if __name__ == "__main__":
    main()
