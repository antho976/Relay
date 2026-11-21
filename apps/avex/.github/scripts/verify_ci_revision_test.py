#!/usr/bin/env python3
import unittest
from verify_ci_revision import contains_pr_head


class RevisionTest(unittest.TestCase):
    def test_accepts_the_run_merge_with_current_head_parent(self):
        self.assertTrue(contains_pr_head("new", "merge", "tree t\nparent base\nparent new\n\nMerge"))

    def test_rejects_previous_revision_even_if_message_mentions_current_head(self):
        self.assertFalse(contains_pr_head("new", "merge", "tree t\nparent base\nparent old\n\nparent new"))

    def test_accepts_exact_head(self):
        self.assertTrue(contains_pr_head("new", "new", "tree t\nparent old\n\nCommit"))

    def test_rejects_missing_expected_revision(self):
        self.assertFalse(contains_pr_head("", "new", "tree t\nparent old\n\nCommit"))


if __name__ == "__main__":
    unittest.main()
