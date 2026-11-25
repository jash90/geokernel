"""Ensure both corpus globs contain only inputs with the shared cases schema."""

import json
from pathlib import Path
import unittest


class CorpusLayout(unittest.TestCase):
    def test_full_corpus_inputs_have_cases(self):
        directory = Path(__file__).parent
        for path in sorted([*directory.glob("*.json"), *directory.glob("reference/*.json")]):
            with self.subTest(path=path.name):
                self.assertIsInstance(json.loads(path.read_text(encoding="utf-8")).get("cases"), list)


if __name__ == "__main__":
    unittest.main()
