#!/usr/bin/env python3
"""Generate the public dependency license inventory from locked inputs."""

from __future__ import annotations

import argparse
import importlib.metadata
import json
import pathlib
import re
import subprocess
from dataclasses import dataclass


ROOT = pathlib.Path(__file__).resolve().parents[2]
PYTHON_LOCKS = (
    ROOT / "services/ai-runtime/requirements.lock",
    ROOT / "services/ai-runtime/requirements-test.lock",
    ROOT / "sdk/python/requirements.lock",
)


@dataclass(frozen=True, order=True)
class Dependency:
    ecosystem: str
    name: str
    version: str
    license: str
    source: str


def safe(value: object, fallback: str = "Not declared") -> str:
    text = str(value or "").strip().replace("|", "\\|").replace("\n", " ")
    return text or fallback


def source_value(value: object, fallback: str) -> str:
    if isinstance(value, dict):
        value = value.get("url")
    text = str(value or "").strip()
    if ", " in text and text.split(", ", 1)[1].startswith("http"):
        text = text.split(", ", 1)[1]
    return safe(text, fallback)


def rust_dependencies() -> set[Dependency]:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(result.stdout)
    dependencies: set[Dependency] = set()
    for package in metadata["packages"]:
        if package.get("source") is None:
            continue
        source = package.get("repository") or package.get("homepage") or package["source"]
        dependencies.add(
            Dependency("Rust", package["name"], package["version"], safe(package.get("license")), safe(source))
        )
    return dependencies


def node_dependencies() -> set[Dependency]:
    lock_path = ROOT / "apps/web/package-lock.json"
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    dependencies: set[Dependency] = set()
    missing: list[str] = []
    for relative, locked in lock["packages"].items():
        if not relative.startswith("node_modules/"):
            continue
        package_path = ROOT / "apps/web" / relative / "package.json"
        if not package_path.is_file():
            if locked.get("optional"):
                continue
            missing.append(relative)
            continue
        package = json.loads(package_path.read_text(encoding="utf-8"))
        license_value = package.get("license")
        if isinstance(license_value, dict):
            license_value = license_value.get("type")
        dependencies.add(
            Dependency(
                "Node.js",
                package.get("name") or relative.removeprefix("node_modules/"),
                locked["version"],
                safe(license_value),
                source_value(
                    package.get("repository") or package.get("homepage") or locked.get("resolved"),
                    f"https://www.npmjs.com/package/{package.get('name') or relative.removeprefix('node_modules/')}/v/{locked['version']}",
                ),
            )
        )
    if missing:
        sample = "\n  ".join(missing[:20])
        raise SystemExit(f"Run `npm ci` in apps/web before generating notices. Missing:\n  {sample}")
    return dependencies


def locked_python_packages() -> set[tuple[str, str]]:
    packages: set[tuple[str, str]] = set()
    pattern = re.compile(r"^([A-Za-z0-9_.-]+)==([^\s\\]+)")
    for lock_path in PYTHON_LOCKS:
        for line in lock_path.read_text(encoding="utf-8").splitlines():
            match = pattern.match(line)
            if match:
                packages.add((match.group(1), match.group(2)))
    return packages


def python_dependencies() -> set[Dependency]:
    dependencies: set[Dependency] = set()
    missing: list[str] = []
    for name, version in sorted(locked_python_packages()):
        try:
            metadata = importlib.metadata.metadata(name)
            installed_version = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            missing.append(f"{name}=={version}")
            continue
        if installed_version != version:
            missing.append(f"{name}=={version} (installed {installed_version})")
            continue
        license_value = metadata.get("License-Expression") or metadata.get("License")
        source = metadata.get("Project-URL") or metadata.get("Home-page")
        dependencies.add(
            Dependency(
                "Python",
                name,
                version,
                safe(license_value),
                source_value(source, f"https://pypi.org/project/{name}/{version}/"),
            )
        )
    if missing:
        joined = "\n  ".join(missing)
        raise SystemExit(f"Install the exact Python lock sets before generating notices:\n  {joined}")
    return dependencies


def render(dependencies: set[Dependency]) -> str:
    sections: list[str] = [
        "# Third-party notices",
        "",
        "Generated from the locked Rust, Node.js, and Python dependency graphs. This inventory is provided for attribution and review. Each component remains subject to its own license text and copyright notices. The source link identifies where the authoritative package metadata and license text can be obtained.",
        "",
        "Regenerate with `python3 scripts/release/generate_third_party_notices.py --output THIRD_PARTY_NOTICES.md` after installing the exact Node.js and Python lock sets. Release review must resolve every `Not declared` entry before distribution when the component is included in the shipped artifact.",
        "",
    ]
    for ecosystem in ("Rust", "Node.js", "Python"):
        entries = sorted(item for item in dependencies if item.ecosystem == ecosystem)
        sections.extend(
            [
                f"## {ecosystem} dependencies",
                "",
                "| Package | Version | Declared license | Source |",
                "| --- | --- | --- | --- |",
            ]
        )
        for item in entries:
            sections.append(f"| {safe(item.name)} | {safe(item.version)} | {safe(item.license)} | {safe(item.source)} |")
        sections.append("")
    return "\n".join(sections)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "THIRD_PARTY_NOTICES.md")
    args = parser.parse_args()
    dependencies = rust_dependencies() | node_dependencies() | python_dependencies()
    args.output.write_text(render(dependencies), encoding="utf-8")
    print(f"wrote {len(dependencies)} dependency records to {args.output}")


if __name__ == "__main__":
    main()
