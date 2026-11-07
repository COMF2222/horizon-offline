#!/usr/bin/env python3
"""Prepare a tested fork update without running upstream code or changing main.

The scheduled workflow creates a candidate, then dispatches the trusted release workflow on
main. Only that workflow may promote the candidate after its tests and signed build succeed.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

UPSTREAM = "horizon-rust/horizon"
STATE = "packaging/offline/upstream-state.json"
# A vendor update must not introduce workflows that use the original team's private infrastructure,
# replace our publisher, or replace the pinned key that installed launchers already trust.
PROTECTED = (".github/workflows", "tools/offline_upstream.py", "tools/ci/sign_release.sh",
             "packaging/offline/release-keys.txt")
SHA = re.compile(r"^[0-9a-f]{40}$")
RELEASE = re.compile(r"^offline-v(\d+)\.(\d+)\.(\d+)$")
LAUNCHER = re.compile(r"^launcher@(\d+)\.(\d+)\.(\d+)$")
SAFE_GIT = ["git", "-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false",
            "-c", "commit.gpgSign=false"]


class Refused(RuntimeError):
    pass


def command(args, *, data=None, env=None, check=True):
    result = subprocess.run(args, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    if check and result.returncode:
        raise Refused(result.stderr.decode(errors="replace").strip() or f"Command failed: {args[0]}")
    return result


def git(*args, data=None, env=None, check=True):
    return command(SAFE_GIT + list(args), data=data, env=env, check=check)


def api(path):
    return json.loads(command(["gh", "api", path]).stdout)


def released(items, pattern):
    matches = [(tuple(map(int, match.groups())), item)
               for item in items if not item.get("draft") and not item.get("prerelease")
               if (match := pattern.fullmatch(item.get("tag_name", "")))]
    return max(matches, key=lambda pair: pair[0])[1] if matches else None


def tag_commit(tag):
    target = api(f"repos/{UPSTREAM}/git/ref/tags/{tag}")["object"]
    while target["type"] == "tag":
        target = api(f"repos/{UPSTREAM}/git/tags/{target['sha']}")["object"]
    if target["type"] != "commit" or not SHA.fullmatch(target["sha"]):
        raise Refused(f"Upstream {tag} does not identify a source commit")
    return target["sha"]


def snapshot():
    launcher = released(api(f"repos/{UPSTREAM}/releases?per_page=100"), LAUNCHER)
    if launcher is None:
        raise Refused("Upstream has no stable launcher release")
    nightly = api(f"repos/{UPSTREAM}/releases/tags/nightly")
    upstream = git("ls-remote", f"https://github.com/{UPSTREAM}.git", "refs/heads/main").stdout.decode().split()
    if not upstream or not SHA.fullmatch(upstream[0]):
        raise Refused("Cannot resolve upstream main")
    return {"schema": 1, "repository": UPSTREAM, "launcher_tag": launcher["tag_name"],
            "launcher_commit": tag_commit(launcher["tag_name"]),
            "game_tag": "nightly", "game_commit": tag_commit("nightly"),
            "game_name": nightly.get("name", ""), "upstream_main": upstream[0],
            "observed_main": upstream[0]}


def signal(state):
    return tuple((state or {}).get(key) for key in ("launcher_tag", "game_commit", "game_name"))


def require_release_ancestry(source, state):
    """Never record an official update that the compiled source does not actually contain."""
    for key in ("launcher_commit", "game_commit"):
        commit = state.get(key, "")
        if not SHA.fullmatch(commit) or git(
                "merge-base", "--is-ancestor", commit, source, check=False).returncode:
            raise Refused(f"Source does not yet contain the released {key}; retry after upstream catches up")


def next_tag(items):
    current = released(items, RELEASE)
    major, minor, patch = map(int, RELEASE.fullmatch(current["tag_name"]).groups()) if current else (0, 2, -1)
    return f"offline-v{major}.{minor}.{patch + 1}"


def emit(values):
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as stream:
            for key, value in values.items():
                stream.write(f"{key}={value}\n")


def resolve():
    """Validate dispatch input in the trusted workflow before building a source ref."""
    source = os.environ.get("SOURCE_REF") or os.environ["GITHUB_SHA"]
    if not SHA.fullmatch(source):
        if source.startswith("-") or git("check-ref-format", "--branch", source, check=False).returncode:
            raise Refused("Invalid source branch")
    git("fetch", "--no-tags", "origin", source)
    source_sha = git("rev-parse", "FETCH_HEAD^{commit}").stdout.decode().strip()
    if not SHA.fullmatch(source_sha):
        raise Refused("Source does not identify a commit")
    tag = os.environ.get("PUBLISH_TAG", "")
    if not tag and os.environ.get("GITHUB_REF_TYPE") == "tag":
        tag = os.environ.get("GITHUB_REF_NAME", "")
    if tag and not RELEASE.fullmatch(tag):
        raise Refused("Release tags must be offline-vX.Y.Z")
    publish = os.environ.get("PUBLISH", "false") == "true"
    promote = os.environ.get("PROMOTE", "false") == "true"
    base = os.environ.get("BASE_SHA", "")
    if publish and not tag:
        raise Refused("Publishing requires an explicit release tag")
    if promote:
        if not publish or not source.startswith("offline-sync/") or not SHA.fullmatch(base):
            raise Refused("Promotion requires a validated sync candidate and base commit")
        if git("merge-base", "--is-ancestor", base, source_sha, check=False).returncode:
            raise Refused("The sync candidate does not contain the fork's base")
    emit({"source_sha": source_sha, "source_ref": source, "release_tag": tag,
          "offline_version": tag.removeprefix("offline-v") if tag else "0.2.0-dev",
          "publish": str(publish).lower(), "promote": str(promote).lower(), "base_sha": base})


def merged_candidate(base, upstream, state):
    """Use git objects and an isolated index, never checkout or execute vendor files."""
    merge = git("merge-tree", "--write-tree", "--name-only", "--no-messages", base, upstream, check=False)
    lines = merge.stdout.decode().splitlines()
    conflicts = lines[1:]
    if merge.returncode not in (0, 1) or not lines or not SHA.fullmatch(lines[0]):
        raise Refused("Cannot construct the upstream merge")
    unresolved = [path for path in conflicts if not protected(path)
                  and not re.fullmatch(r"launcher/lang/[a-zA-Z_-]+\.json", path)]
    if unresolved:
        raise Refused("Upstream merge conflicts; the existing release remains unchanged.\n"
                      + "\n".join(unresolved)[:4000])
    locales = {}
    for path in conflicts:
        if not protected(path):
            common = git("merge-base", base, upstream).stdout.decode().strip()
            locales[path] = merged_locale(common, base, upstream, path)
    tree = lines[0]
    with tempfile.TemporaryDirectory(prefix="horizon-offline-index-") as directory:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(directory) / "index"))
        git("read-tree", tree, env=env)
        present = git("ls-files", "-z", "--", *PROTECTED, env=env).stdout
        if present:
            git("update-index", "--force-remove", "-z", "--stdin", data=present, env=env)
        entries = git("ls-tree", "-rz", base, "--", *PROTECTED).stdout
        restored = []
        for entry in entries.split(b"\0"):
            if entry:
                meta, path = entry.split(b"\t", 1)
                mode, _, object_id = meta.split()
                restored.append(mode + b" " + object_id + b"\t" + path + b"\0")
        if restored:
            git("update-index", "-z", "--index-info", data=b"".join(restored), env=env)
        for path, contents in locales.items():
            blob = git("hash-object", "-w", "--stdin", data=contents).stdout.decode().strip()
            git("update-index", "--add", "--cacheinfo", "100644", blob, path, env=env)
        blob = git("hash-object", "-w", "--stdin", data=(json.dumps(state, indent=2) + "\n").encode()).stdout.strip()
        git("update-index", "--add", "--cacheinfo", "100644", blob.decode(), STATE, env=env)
        tree = git("write-tree", env=env).stdout.decode().strip()
    parents = ["-p", base]
    if git("merge-base", "--is-ancestor", upstream, base, check=False).returncode:
        parents += ["-p", upstream]
    identity = dict(os.environ, GIT_AUTHOR_NAME="github-actions[bot]", GIT_COMMITTER_NAME="github-actions[bot]",
                    GIT_AUTHOR_EMAIL="41898283+github-actions[bot]@users.noreply.github.com",
                    GIT_COMMITTER_EMAIL="41898283+github-actions[bot]@users.noreply.github.com")
    message = (f"Sync Horizon Offline with {state['launcher_tag']} and {state['game_name']}\n\n"
               "Generated by the Horizon Offline upstream synchronization workflow.\n")
    return git("commit-tree", tree, *parents, data=message.encode(), env=identity).stdout.decode().strip()


def protected(path):
    return any(path == prefix or path.startswith(prefix + "/") for prefix in PROTECTED)


def merged_locale(common, base, upstream, path):
    """Merge independent message keys, refusing two different edits of the same translation."""
    tables = []
    for revision in (common, base, upstream):
        result = git("show", f"{revision}:{path}", check=False)
        try:
            table = json.loads(result.stdout) if result.returncode == 0 else {}
        except (ValueError, UnicodeError) as error:
            raise Refused(f"Cannot merge translation table {path}") from error
        if not isinstance(table, dict) or not all(isinstance(value, str) for value in table.values()):
            raise Refused(f"Cannot merge translation table {path}")
        tables.append(table)
    old, ours, theirs = tables
    absent = object()
    merged = {}
    for key in dict.fromkeys([*theirs, *ours, *old]):
        before, left, right = (table.get(key, absent) for table in tables)
        if left == before:
            value = right
        elif right == before or left == right:
            value = left
        elif left is absent:
            # The fork replaced a message; a new translation of its old text cannot restore it.
            value = absent
        else:
            raise Refused(f"Upstream translation merge conflicts in {path}: {key}")
        if value is not absent:
            merged[key] = value
    return (json.dumps(merged, ensure_ascii=False, indent=2) + "\n").encode()


def prepare(force=False):
    repo = os.environ.get("GITHUB_REPOSITORY", "COMF2222/horizon-offline")
    if repo != "COMF2222/horizon-offline":
        raise Refused("This update channel only publishes COMF2222/horizon-offline")
    state = snapshot()
    own = api(f"repos/{repo}/releases?per_page=100")
    current = released(own, RELEASE)
    previous = None
    if current:
        git("fetch", "--no-tags", "origin", f"+refs/tags/{current['tag_name']}:refs/remotes/offline-published")
        old = git("show", f"refs/remotes/offline-published^{{commit}}:{STATE}", check=False)
        if old.returncode == 0:
            previous = json.loads(old.stdout)
    if not force and signal(previous) == signal(state):
        print("No new upstream launcher or game release; no build started.")
        emit({"changed": "false"})
        return
    git("fetch", "--no-tags", "origin", "main")
    base = git("rev-parse", "FETCH_HEAD^{commit}").stdout.decode().strip()
    git("fetch", "--no-tags", f"https://github.com/{UPSTREAM}.git", state["upstream_main"])
    git("fetch", "--no-tags", f"https://github.com/{UPSTREAM}.git",
        state["launcher_commit"], state["game_commit"])
    require_release_ancestry(state["upstream_main"], state)
    branch = f"offline-sync/{state['upstream_main'][:12]}-{base[:8]}"
    existing = git("ls-remote", "--exit-code", "origin", f"refs/heads/{branch}", check=False)
    if existing.returncode == 0 and not force:
        print(f"Candidate {branch} was already attempted; existing release remains active. Use force to retry.")
        emit({"changed": "false"})
        return
    if existing.returncode not in (0, 2):
        raise Refused("Cannot check the existing sync candidate")
    tag = next_tag(own)
    if existing.returncode == 0:
        candidate = existing.stdout.decode().split()[0]
        git("fetch", "--no-tags", "origin", branch)
        if git("merge-base", "--is-ancestor", base, candidate, check=False).returncode:
            raise Refused("Existing candidate is not based on current main")
    else:
        candidate = merged_candidate(base, state["upstream_main"], state)
        command(["gh", "auth", "setup-git"])
        git("push", "origin", f"{candidate}:refs/heads/{branch}")
    command(["gh", "workflow", "run", "offline-release.yml", "--repo", repo, "--ref", "main",
             "-f", f"source_ref={branch}", "-f", f"publish_tag={tag}", "-f", "publish=true",
             "-f", "promote=true", "-f", f"base_sha={base}"])
    print(f"Queued {tag} from {branch}; main and the published release are unchanged until all checks pass.")
    emit({"changed": "true", "candidate": branch, "release_tag": tag})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "resolve", "snapshot"))
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--merged-source", help="Record an upstream commit already merged into this source")
    args = parser.parse_args()
    if args.action == "resolve":
        resolve()
    elif args.action == "snapshot":
        state = snapshot()
        if args.merged_source:
            if not SHA.fullmatch(args.merged_source) or git(
                    "merge-base", "--is-ancestor", args.merged_source, "HEAD", check=False).returncode:
                raise Refused("The recorded upstream source is not an ancestor of this checkout")
            state["upstream_main"] = args.merged_source
            require_release_ancestry(args.merged_source, state)
        text = json.dumps(state, indent=2) + "\n"
        if args.out:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(text, encoding="utf-8")
        else:
            print(text, end="")
    else:
        prepare(args.force)


if __name__ == "__main__":
    try:
        main()
    except Refused as error:
        raise SystemExit(str(error))
