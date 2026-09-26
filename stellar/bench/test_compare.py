#!/usr/bin/env python3

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPARE = HERE / "compare.py"
COMMITTED_BASELINE = HERE / "baseline.json"

TARGET = (
    "stealth-sender",
    "batch_send",
    "batch_size=100",
)


def key_for(row):
    return (
        row["contract"],
        row["function"],
        row["params"],
    )


class CompareBaselineAuthorityTest(unittest.TestCase):
    def test_supplied_baseline_is_authoritative(self):
        committed = json.loads(COMMITTED_BASELINE.read_text())

        selected = copy.deepcopy(committed)
        current = copy.deepcopy(committed)

        target_selected = next(
            row for row in selected["results"]
            if key_for(row) == TARGET
        )
        target_current = next(
            row for row in current["results"]
            if key_for(row) == TARGET
        )

        # Simulate a downloaded develop artifact with a much lower limit.
        target_selected["mem_bytes"] = 100

        # A 6% regression against the downloaded baseline must fail.
        target_current["mem_bytes"] = 106

        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            selected_path = tmp_path / "develop-baseline.json"
            current_path = tmp_path / "current.json"

            selected_path.write_text(
                json.dumps(selected, indent=2) + "\n"
            )
            current_path.write_text(
                json.dumps(current, indent=2) + "\n"
            )

            result = subprocess.run(
                [
                    sys.executable,
                    str(COMPARE),
                    str(selected_path),
                    str(current_path),
                    "--threshold-pct",
                    "5",
                ],
                capture_output=True,
                text=True,
            )

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("batch_size=100", result.stdout)
        self.assertIn("mem_bytes", result.stdout)
        self.assertIn("+6.00%", result.stdout)


if __name__ == "__main__":
    unittest.main()
