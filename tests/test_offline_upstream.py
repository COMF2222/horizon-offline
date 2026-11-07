"""The fork's release selection and conservative, object-only upstream merge."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools import offline_upstream as sync


class ReleaseSelection(unittest.TestCase):
    def test_latest_release_uses_numeric_stable_versions(self):
        items = [dict(tag_name="launcher@0.2.9"), dict(tag_name="launcher@0.2.10"),
                 dict(tag_name="launcher@99.0.0", draft=True),
                 dict(tag_name="launcher@1.0.0", prerelease=True), dict(tag_name="unrelated")]
        self.assertEqual(sync.released(items, sync.LAUNCHER)["tag_name"], "launcher@0.2.10")

    def test_next_fork_version_does_not_follow_upstream_number(self):
        self.assertEqual(sync.next_tag([dict(tag_name="offline-v0.2.9"),
                                       dict(tag_name="launcher@9.9.9")]), "offline-v0.2.10")
        self.assertEqual(sync.next_tag([]), "offline-v0.2.0")

    def test_unreleased_main_changes_do_not_start_rebuilds(self):
        before = dict(launcher_tag="launcher@0.2.48", game_commit="a" * 40,
                      game_name="Nightly 2026-11-05", upstream_main="b" * 40)
        after = dict(before, upstream_main="c" * 40)
        self.assertEqual(sync.signal(before), sync.signal(after))
        self.assertNotEqual(sync.signal(before), sync.signal(dict(after, game_commit="d" * 40)))

    def test_a_new_nightly_on_the_same_main_gets_its_own_candidate(self):
        state = dict(launcher_tag="launcher@0.2.49", launcher_commit="a" * 40,
                     game_commit="b" * 40, game_name="Nightly fixture", upstream_main="c" * 40)
        base = "d" * 40
        first = sync.candidate_branch(base, state)
        self.assertEqual(first, sync.candidate_branch(base, dict(state)))
        self.assertNotEqual(first, sync.candidate_branch(base, dict(state, game_commit="e" * 40)))


class CandidateMerge(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="offline-upstream-test-")
        self.previous = Path.cwd()
        os.chdir(self.scratch.name)
        self.addCleanup(self.scratch.cleanup)
        self.addCleanup(os.chdir, self.previous)
        self.run_git("init", "-b", "main")
        self.run_git("config", "user.name", "Test fixture")
        self.run_git("config", "user.email", "test@example.invalid")
        self.write("game.txt", "original\n")
        self.write(".github/workflows/offline-release.yml", "our publisher\n")
        self.write("tools/ci/sign_release.sh", "our signer\n")
        self.write("packaging/offline/release-keys.txt", "our key\n")
        self.common = self.commit("common")

    def run_git(self, *args):
        return subprocess.run(["git", "-c", "core.hooksPath=/dev/null", *args], check=True,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.decode().strip()

    def write(self, path, text):
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def commit(self, message):
        self.run_git("add", ".")
        self.run_git("commit", "-m", message)
        return self.run_git("rev-parse", "HEAD")

    def test_clean_merge_keeps_fork_patch_and_publisher_but_imports_game_fix(self):
        self.write("offline.txt", "our offline feature\n")
        base = self.commit("fork")
        self.run_git("checkout", "-b", "vendor", self.common)
        self.write("game.txt", "upstream game fix\n")
        self.write(".github/workflows/vendor-private.yml", "must not become active\n")
        self.write("tools/ci/sign_release.sh", "different upstream signer\n")
        upstream = self.commit("upstream")
        self.run_git("checkout", "main")
        state = {"launcher_tag": "launcher@0.2.48", "game_name": "Nightly fixture"}
        candidate = sync.merged_candidate(base, upstream, state)
        self.assertEqual(self.run_git("rev-parse", "HEAD"), base)
        self.assertEqual(Path("game.txt").read_text(), "original\n")
        self.assertEqual(self.run_git("show", f"{candidate}:game.txt"), "upstream game fix")
        self.assertEqual(self.run_git("show", f"{candidate}:offline.txt"), "our offline feature")
        self.assertEqual(self.run_git("show", f"{candidate}:tools/ci/sign_release.sh"), "our signer")
        names = self.run_git("ls-tree", "-r", "--name-only", candidate).splitlines()
        self.assertNotIn(".github/workflows/vendor-private.yml", names)
        self.assertIn(".github/workflows/offline-release.yml", names)
        self.assertEqual(json.loads(self.run_git("show", f"{candidate}:{sync.STATE}")), state)
        self.assertEqual(self.run_git("status", "--porcelain"), "")

    def test_conflict_leaves_head_index_and_files_unchanged(self):
        self.write("game.txt", "our skin integration\n")
        base = self.commit("fork")
        self.run_git("checkout", "-b", "vendor", self.common)
        self.write("game.txt", "incompatible upstream change\n")
        upstream = self.commit("upstream")
        self.run_git("checkout", "main")
        with self.assertRaisesRegex(sync.Refused, "merge conflicts"):
            sync.merged_candidate(base, upstream, {})
        self.assertEqual(self.run_git("rev-parse", "HEAD"), base)
        self.assertEqual(Path("game.txt").read_text(), "our skin integration\n")
        self.assertEqual(self.run_git("status", "--porcelain"), "")

    def test_recorded_releases_must_actually_be_in_the_compiled_source(self):
        self.write("game.txt", "new official release\n")
        released_commit = self.commit("official release")
        state = dict(launcher_commit=self.common, game_commit=released_commit)
        with self.assertRaisesRegex(sync.Refused, "released game_commit"):
            sync.require_release_ancestry(self.common, state)
        sync.require_release_ancestry(released_commit, state)

    def separate_releases(self, *, conflicting=False):
        self.write("offline.txt", "our offline profile\n")
        if conflicting:
            self.write("game.txt", "our skin integration\n")
        base = self.commit("fork")
        self.run_git("checkout", "-b", "vendor-main", self.common)
        self.write("launcher.txt", "latest launcher\n")
        main = self.commit("launcher release")
        self.run_git("checkout", "-b", "vendor-nightly", self.common)
        self.write("game.txt", "released nightly game\n")
        self.write(".github/workflows/vendor-private.yml", "upstream-only infrastructure\n")
        nightly = self.commit("nightly with an independently picked PR")
        self.run_git("checkout", "main")
        state = dict(upstream_main=main, launcher_commit=main, game_commit=nightly,
                     launcher_tag="launcher@0.2.49", game_name="Nightly fixture")
        return base, main, nightly, state

    def test_picked_nightly_and_launcher_are_both_present_without_enabling_vendor_workflows(self):
        base, main, nightly, state = self.separate_releases()
        candidate = sync.released_candidate(base, state)
        for source in (base, main, nightly):
            self.run_git("merge-base", "--is-ancestor", source, candidate)
        self.assertEqual(self.run_git("show", f"{candidate}:launcher.txt"), "latest launcher")
        self.assertEqual(self.run_git("show", f"{candidate}:game.txt"), "released nightly game")
        self.assertEqual(self.run_git("show", f"{candidate}:offline.txt"), "our offline profile")
        self.assertEqual(self.run_git("show", f"{candidate}:.github/workflows/offline-release.yml"),
                         "our publisher")
        self.assertNotIn(".github/workflows/vendor-private.yml",
                         self.run_git("ls-tree", "-r", "--name-only", candidate).splitlines())
        self.assertEqual(json.loads(self.run_git("show", f"{candidate}:{sync.STATE}")), state)
        self.assertEqual(self.run_git("rev-parse", "HEAD"), base)
        self.assertEqual(self.run_git("status", "--porcelain"), "")

    def test_a_conflicting_picked_nightly_keeps_the_working_fork_unchanged(self):
        base, _, _, state = self.separate_releases(conflicting=True)
        with self.assertRaisesRegex(sync.Refused, "merge conflicts"):
            sync.released_candidate(base, state)
        self.assertEqual(self.run_git("rev-parse", "HEAD"), base)
        self.assertEqual(Path("game.txt").read_text(), "our skin integration\n")
        self.assertEqual(self.run_git("status", "--porcelain"), "")

    def locale_branches(self, original, ours, theirs):
        path = "launcher/lang/ru.json"
        self.write(path, json.dumps(original, indent=2) + "\n")
        common = self.commit("shared translations")
        self.write(path, json.dumps(ours, indent=2) + "\n")
        base = self.commit("fork translations")
        self.run_git("checkout", "-b", "vendor", common)
        self.write(path, json.dumps(theirs, indent=2) + "\n")
        upstream = self.commit("upstream translations")
        self.run_git("checkout", "main")
        return path, base, upstream

    def test_independent_translation_additions_do_not_block_updates(self):
        path, base, upstream = self.locale_branches(
            {"shared": "original"},
            {"shared": "original", "offline": "our local profile"},
            {"shared": "original", "new upstream": "new feature"})
        state = dict(launcher_tag="launcher@0.2.48", game_name="fixture")
        candidate = sync.merged_candidate(base, upstream, state)
        self.assertEqual(json.loads(self.run_git("show", f"{candidate}:{path}")),
                         {"shared": "original", "offline": "our local profile", "new upstream": "new feature"})
        self.assertEqual(self.run_git("status", "--porcelain"), "")

    def test_upstream_translation_does_not_restore_a_message_replaced_by_the_fork(self):
        path, base, upstream = self.locale_branches(
            {"old caption": "old", "shared": "old shared"},
            {"our caption": "LAN skins", "shared": "old shared"},
            {"old caption": "new translation", "shared": "improved shared"})
        candidate = sync.merged_candidate(base, upstream,
                                          dict(launcher_tag="launcher@0.2.48", game_name="fixture"))
        self.assertEqual(json.loads(self.run_git("show", f"{candidate}:{path}")),
                         {"our caption": "LAN skins", "shared": "improved shared"})

    def test_two_different_edits_of_the_same_translation_require_review(self):
        _, base, upstream = self.locale_branches(
            {"caption": "old"}, {"caption": "our translation"}, {"caption": "their translation"})
        with self.assertRaisesRegex(sync.Refused, "translation merge conflicts"):
            sync.merged_candidate(base, upstream, {})
        self.assertEqual(self.run_git("status", "--porcelain"), "")

    def test_conflicting_publisher_changes_keep_our_publisher(self):
        path = ".github/workflows/offline-release.yml"
        self.write(path, "updated fork publisher\n")
        base = self.commit("fork publisher")
        self.run_git("checkout", "-b", "vendor", self.common)
        self.write(path, "different vendor publisher\n")
        upstream = self.commit("vendor publisher")
        self.run_git("checkout", "main")
        candidate = sync.merged_candidate(base, upstream,
                                          dict(launcher_tag="launcher@0.2.48", game_name="fixture"))
        self.assertEqual(self.run_git("show", f"{candidate}:{path}"), "updated fork publisher")
        self.assertEqual(self.run_git("status", "--porcelain"), "")


class DispatchValidation(unittest.TestCase):
    def test_invalid_source_never_reaches_fetch(self):
        env = dict(SOURCE_REF="--upload-pack=anything", GITHUB_SHA="a" * 40)
        with patch.dict(os.environ, env):
            with self.assertRaises(sync.Refused):
                sync.resolve()


if __name__ == "__main__":
    unittest.main()
