"""Acceptance-shaped checks with explicit expectations from T01, T02 and NPG."""
from copy import deepcopy
import unittest

from verify_catalog import POC, inputs, verify


class CatalogTest(unittest.TestCase):
    def setUp(self):
        self.catalog, self.sources, self.spec = inputs()

    def verify(self):
        return verify(self.catalog, self.sources, self.spec)

    def test_scope_and_expected_trace(self):
        self.assertEqual(self.verify(), (6, 9))
        actual = {
            (entry["id"], entry["version"]):
                (entry["scope"], entry["readiness"], entry["requirements"][0]["item_id"])
            for entry in self.catalog["entries"]
        }
        self.assertEqual(actual, {
            ("CAT-FORM", "0.1.0"): ("common", "contract", "03-01.accept"),
            ("CAT-ROLE", "0.1.0"): ("common", "draft", "03-02.accept"),
            ("CAT-RECOVERY", "0.1.0"): ("common", "draft", "03-02.accept"),
            ("CAT-GITHUB", "0.1.0"): ("repository", "draft", "03-02.accept"),
            ("CAT-GO", "0.1.0"): ("go", "draft", "03-03.accept"),
            ("CAT-RUST", "0.1.0"): ("rust", "draft", "03-03.accept"),
        })
        mappings = {
            row["item_id"]: {link["source_id"]: link["relation"] for link in row["sources"]}
            for row in self.sources["mappings"]
        }
        self.assertEqual(mappings["CAT-FORM"]["spec-0301"], "requirement")
        self.assertEqual(mappings["CAT-ROLE"]["npg-role"], "candidate")
        self.assertEqual(mappings["CAT-RECOVERY"]["npg-recovery"], "candidate")
        self.assertEqual(mappings["CAT-RECOVERY"]["collector-judge"], "observation")
        self.assertEqual(mappings["CAT-GITHUB"]["npg-ci"], "candidate")
        self.assertEqual(mappings["CAT-GITHUB"]["npg-ci-run"], "observation")

    def test_required_fields(self):
        del self.catalog["entries"][0]["expected_evidence"]
        with self.assertRaisesRegex(ValueError, "missing expected_evidence"):
            self.verify()

    def test_wrong_profile(self):
        self.catalog["entries"][0]["requirements"][0]["profile_revision"] = 999
        with self.assertRaisesRegex(ValueError, "invalid requirement/acceptance binding"):
            self.verify()

    def test_mismatched_version_join(self):
        self.sources["mappings"][0]["item_version"] = "0.1.1"
        with self.assertRaisesRegex(ValueError, "identities differ"):
            self.verify()

    def test_duplicate_identity(self):
        self.catalog["entries"].append(deepcopy(self.catalog["entries"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate identity"):
            self.verify()

    def test_unpinned_repository_scope(self):
        entry = next(entry for entry in self.catalog["entries"] if entry["scope"] == "repository")
        del entry["repository"]
        with self.assertRaisesRegex(ValueError, "repository binding mismatch"):
            self.verify()

    def test_unresolved_source(self):
        self.sources["mappings"][0]["sources"][0]["source_id"] = "missing"
        with self.assertRaisesRegex(ValueError, "unresolved source"):
            self.verify()

    def test_historical_source_cannot_be_promoted_to_requirement(self):
        self.sources["mappings"][0]["sources"].append(
            {"source_id": "npg-role", "relation": "requirement"}
        )
        with self.assertRaisesRegex(ValueError, "unapproved requirement source"):
            self.verify()

    def test_approved_source_cannot_be_rebound_to_another_path(self):
        spec_source = self.sources["sources"][0]
        spec_source["path"] = self.sources["sources"][-1]["path"]
        with self.assertRaisesRegex(ValueError, "requirement source anchor mismatch"):
            self.verify()

    def test_approved_source_cannot_cite_another_clause_in_the_same_file(self):
        source = self.sources["sources"][0]
        spec_lines = (POC / source["path"]).read_text(encoding="utf-8").splitlines()
        line = next(index for index, text in enumerate(spec_lines, 1)
                    if '"03-02.accept"' in text)
        source.update(lines=[line, line], excerpt="03-02.accept")
        with self.assertRaisesRegex(ValueError, "requirement source anchor mismatch"):
            self.verify()

    def test_stale_digest(self):
        self.sources["sources"][0]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "digest mismatch"):
            self.verify()

    def test_unknown_revision(self):
        self.sources["sources"][0]["revision"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "revision mismatch"):
            self.verify()

    def test_wrong_excerpt(self):
        source = next(source for source in self.sources["sources"] if source["id"] == "npg-role")
        source["excerpt"] = "not present on these lines"
        with self.assertRaisesRegex(ValueError, "excerpt missing"):
            self.verify()


if __name__ == "__main__":
    unittest.main(verbosity=2)
