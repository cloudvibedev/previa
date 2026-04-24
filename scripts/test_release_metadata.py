#!/usr/bin/env python3

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = REPO_ROOT / "scripts" / "generate_release_metadata.py"


class ReleaseMetadataTests(unittest.TestCase):
    def generate(self, release_scope: str) -> dict:
        with tempfile.TemporaryDirectory() as temp_dir:
            env = os.environ.copy()
            env.update(
                {
                    "PUBLISHED_AT": "2026-04-24T00:00:00Z",
                    "RELEASE_SCOPE": release_scope,
                    "REPOSITORY": "cloudvibedev/previa",
                    "VERSION": "1.2.3",
                }
            )

            subprocess.run(
                [sys.executable, str(SCRIPT)],
                cwd=temp_dir,
                env=env,
                check=True,
            )

            metadata_path = Path(temp_dir) / "release-metadata.json"
            return json.loads(metadata_path.read_text(encoding="utf-8"))

    def test_mac_scope_includes_amd64_and_arm64_control_binaries(self) -> None:
        metadata = self.generate("mac")

        self.assertEqual(
            metadata["links"],
            {
                "previa_macos_amd64": "https://github.com/cloudvibedev/previa/releases/download/v1.2.3/previa-macos-amd64",
                "previa_macos_arm64": "https://github.com/cloudvibedev/previa/releases/download/v1.2.3/previa-macos-arm64",
            },
        )

    def test_all_scope_includes_macos_arm64_control_binary(self) -> None:
        metadata = self.generate("all")

        self.assertIn("previa_macos_amd64", metadata["links"])
        self.assertIn("previa_macos_arm64", metadata["links"])


if __name__ == "__main__":
    unittest.main()
