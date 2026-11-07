#!/usr/bin/env python3
"""Build the two reviewed optional mods against the exact offline distribution source.

Developer/CI tool only: players receive the compiled game in the launcher. No game assets
are read or packaged. The original checkout is restored after a temporary in-place build.
"""
from __future__ import annotations

import argparse
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import zipfile

MIXINS_REPO = "https://github.com/horizon-rust/horizon-mixins"
MIXINS_REVISION = "5a64af938ec86b00d53d16a408aabc0038fb3e57"
MODS = ("mousetricks", "treefall")


def run(words, *, cwd=None):
    subprocess.run(list(map(str, words)), cwd=cwd, check=True)


def stage_mods(source: Path, out: Path, revision: str):
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("The offline source revision must be an exact commit")
    out.mkdir(parents=True, exist_ok=True)
    licenses = []
    for name in MODS:
        vendor = source / "third_party/offline-native-mods" / name
        target = out / name
        shutil.copytree(vendor / "mixins" / name, target, dirs_exist_ok=True)
        metadata = json.loads((vendor / "horizon-native.json").read_text(encoding="utf-8"))
        manifest = target / "horizon-mixin.toml"
        text = manifest.read_text(encoding="utf-8")
        old = f'horizon_revision = "{metadata["horizon"]}"'
        if text.count(old) != 1:
            raise ValueError(f"{name}: cannot locate the declared upstream pin")
        manifest.write_text(text.replace(old, f'horizon_revision = "{revision}"'), encoding="utf-8")
        licenses.append(f"{name}\n{'=' * len(name)}\n\n" + (vendor / "LICENSE").read_text(encoding="utf-8"))
    (out / "LICENSES-native-mods.txt").write_text("\n\n".join(licenses), encoding="utf-8")


def pristine_source(source: Path, revision: str, out: Path):
    archive = subprocess.check_output(["git", "-C", str(source), "archive", "--format=zip", revision])
    out.mkdir(parents=True)
    with zipfile.ZipFile(io.BytesIO(archive)) as files:
        for entry in files.infolist():
            relative = Path(entry.filename)
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError("Invalid source archive path")
            if entry.is_dir() or (entry.external_attr >> 16) & 0o170000 == 0o120000:
                continue
            path = out / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(files.read(entry))
    (out / ".horizon-revision").write_text(revision, encoding="utf-8")


def build(args):
    source = args.source.resolve()
    revision = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if subprocess.run(["git", "-C", str(source), "diff", "--quiet", "HEAD", "--"], check=False).returncode:
        raise ValueError("Build from a clean checkout; the helper temporarily stages transformed files")
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="optional-mods-", dir=work)).resolve()
    assert scratch.is_relative_to(work)
    bundles, pristine, transformed = (scratch / name for name in ("bundles", "pristine", "transformed"))
    stage_mods(source, bundles, revision)
    pristine_source(source, revision, pristine)
    cargo = args.cargo
    if args.tool:
        tool = args.tool.resolve()
    else:
        tools = work / "tools"
        tool = tools / "bin" / ("horizon-mixins.exe" if os.name == "nt" else "horizon-mixins")
        if not tool.is_file():
            run([cargo, "install", "--locked", "--git", MIXINS_REPO, "--rev", MIXINS_REVISION,
                 "--root", tools, "--target-dir", source / "target/offline-mixins", "horizon-mixins-cli"])
    run([tool, "plan", "--workspace", pristine, "--mods", bundles, "--out", scratch / "plan.json"])
    run([tool, "apply", "--workspace", pristine, "--mods", bundles, "--out", transformed])

    # Reuse the vanilla build's crate paths and dependencies. Only transformed source is written;
    # a finally block restores exact original bytes, including after a compiler failure.
    originals = {}
    try:
        for path in (transformed / "crates/horizon-client").rglob("*"):
            if not path.is_file():
                continue
            relative = path.relative_to(transformed)
            target = source / relative
            original = target.read_bytes() if target.is_file() else None
            data = path.read_bytes()
            if original is not None and data.replace(b"\r\n", b"\n") == original.replace(b"\r\n", b"\n"):
                continue
            if data != original:
                originals[target] = original
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
        if not originals:
            raise ValueError("The mod transformation produced no changed game code")
        run([cargo, "build", "--profile", "fast", "--locked", "-p", "horizon-client", "--bin", "horizon-client"], cwd=source)
        if not args.skip_lints:
            run([cargo, "clippy", "--profile", "fast", "--locked", "-p", "horizon-client", "--lib", "--bins", "--", "-D", "warnings"], cwd=source)
        output = args.output.resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        binary = source / "target/fast" / ("horizon-client.exe" if os.name == "nt" else "horizon-client")
        shutil.copy2(binary, output)
        shutil.copy2(bundles / "LICENSES-native-mods.txt", output.parent.parent / "LICENSES-native-mods.txt")
        (output.parent / "source.json").write_text(json.dumps({
            "schema": 1, "source_commit": revision, "mixins_revision": MIXINS_REVISION,
            "mods": [{"id": name, "license": "MIT", "author": "d3grando"} for name in MODS],
        }, indent=2) + "\n", encoding="utf-8")
        print(f"Built optional mod profile: {output}")
    finally:
        for path, original in originals.items():
            if original is None:
                path.unlink(missing_ok=True)
            else:
                path.write_bytes(original)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("test", "stage", "build"))
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--work", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--revision")
    parser.add_argument("--tool", type=Path)
    parser.add_argument("--skip-lints", action="store_true", help="developer iteration only")
    args = parser.parse_args()
    if args.action == "test":
        for name in MODS:
            manifest = args.source / "third_party/offline-native-mods" / name / "Cargo.toml"
            run([args.cargo, "fmt", "--manifest-path", manifest, "--", "--check"])
            run(["rustfmt", "--edition", "2024", "--check", *sorted((manifest.parent / "mixins" / name).glob("*.rs"))])
            run([args.cargo, "test", "--locked", "--manifest-path", manifest])
            run([args.cargo, "clippy", "--locked", "--manifest-path", manifest, "--all-targets", "--", "-D", "warnings"])
    elif args.action == "stage":
        if not args.work or not args.revision:
            parser.error("stage requires --work and --revision")
        stage_mods(args.source, args.work, args.revision)
    else:
        if not args.work or not args.output:
            parser.error("build requires --work and --output")
        build(args)


if __name__ == "__main__":
    main()
