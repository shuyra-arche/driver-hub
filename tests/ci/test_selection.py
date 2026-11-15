"""Focused checks for the PR handoff path classifier."""

import unittest

from selection import is_docs_only


class SelectionTests(unittest.TestCase):
    def test_non_empty_markdown_paths_skip_handoff(self):
        self.assertTrue(is_docs_only(b"README.md\0docs/ci.md\0"))

    def test_empty_diff_runs_handoff(self):
        self.assertFalse(is_docs_only(b""))

    def test_deleted_or_renamed_source_runs_handoff(self):
        self.assertFalse(is_docs_only(b"src/lib.rs\0README.md\0"))

    def test_unknown_extension_runs_handoff(self):
        self.assertFalse(is_docs_only(b"notes.MD\0"))

    def test_incomplete_nul_list_runs_handoff(self):
        self.assertFalse(is_docs_only(b"README.md"))

    def test_empty_path_runs_handoff(self):
        self.assertFalse(is_docs_only(b"README.md\0\0"))


if __name__ == "__main__":
    unittest.main()
