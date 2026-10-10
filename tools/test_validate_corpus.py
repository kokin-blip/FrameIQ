import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("validator", Path(__file__).with_name("validate-corpus.py"))
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


def record(**changes):
    return dict(dict(videoId="v1", sha256="a"*64, subjectGroup="person1", license="owned",
                     consentRef="signed-1", split="calibration", ordinal="0", origin="permissioned_real",
                     qualityRelevance=2, acceptableRecommendation=True, regions=[], duplicateGroup=None), **changes)


class CorpusTests(unittest.TestCase):
    def test_partial_corpus_is_valid_but_not_ready(self):
        result = validator.validate([record()])
        self.assertTrue(result["valid"])
        self.assertFalse(result["annotationMinimumMet"])

    def test_subject_and_identical_content_cannot_leak(self):
        for changes in (dict(sha256="b"*64), dict(subjectGroup="person2")):
            result = validator.validate([record(), record(videoId="v2", split="held_out", ordinal="1", **changes)])
            self.assertFalse(result["valid"])
            self.assertTrue(any("leaks" in e for e in result["errors"]))

    def test_duplicate_frames_and_invalid_labels_rejected(self):
        self.assertFalse(validator.validate([record(), record()])["valid"])
        for changes in (dict(ordinal="1.2"), dict(qualityRelevance=True), dict(acceptableRecommendation=0),
                        dict(origin="synthetic"), dict(sha256="no"), dict(ordinal=str(2**63))):
            self.assertFalse(validator.validate([record(**changes)])["valid"])

    def test_region_must_fit_and_be_finite(self):
        for width in (2, float("nan"), float("inf")):
            r = record(regions=[dict(category="face", box=dict(x=0, y=0, width=width, height=.2))])
            self.assertFalse(validator.validate([r])["valid"])

    def test_media_path_and_checksum_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/"clip").write_bytes(b"test")
            import hashlib
            r = record(path="clip", sha256=hashlib.sha256(b"test").hexdigest())
            self.assertEqual(validator.validate([r], root)["verifiedFiles"], 1)
            self.assertFalse(validator.validate([record(path="clip")], root)["valid"])
            self.assertFalse(validator.validate([record(path="../clip")], root)["valid"])

    def test_minimum_counts_unique_sources(self):
        records = [record(videoId=f"v{i}", sha256=f"{i:064x}", subjectGroup=f"p{i}",
                          split="calibration" if i<15 else "held_out", ordinal=str(j))
                   for i in range(30) for j in range(10)]
        self.assertTrue(validator.validate(records)["annotationMinimumMet"])


if __name__ == "__main__":
    unittest.main()
