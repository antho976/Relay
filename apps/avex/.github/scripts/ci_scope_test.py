#!/usr/bin/env python3
import unittest
from ci_scope import is_ignored, needs_build


class ScopeTest(unittest.TestCase):
    def test_docs_only_skips_the_build(self):
        self.assertFalse(needs_build(["docs/audits/2026-09-26/README.md", "README.md"]))

    def test_any_source_file_builds(self):
        self.assertTrue(needs_build(["docs/ROADMAP.md", "forge-android/app/build.gradle.kts"]))

    def test_nested_goldens_build(self):
        self.assertFalse(is_ignored("forge-android/app/src/test/snapshots/home.png"))

    def test_root_media_is_ignored(self):
        self.assertTrue(is_ignored("hero.png"))

    def test_remotion_folders_are_ignored(self):
        self.assertTrue(is_ignored("remotion-promo/src/index.ts"))

    def test_design_doctrine_builds(self):
        # The doctrine parity suite reads .claude/DESIGN.md.
        self.assertTrue(needs_build([".claude/DESIGN.md"]))

    def test_workflow_changes_build(self):
        self.assertTrue(needs_build([".github/workflows/ci.yml"]))

    def test_nested_readme_builds(self):
        self.assertFalse(is_ignored("forge-android/README.md"))

    def test_empty_diff_builds(self):
        self.assertTrue(needs_build([]))


if __name__ == "__main__":
    unittest.main()
