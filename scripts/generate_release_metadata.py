#!/usr/bin/env python3

import json
import os
from pathlib import Path


def main() -> None:
    version = os.environ["VERSION"]
    repository = os.environ["REPOSITORY"]
    release_scope = os.environ["RELEASE_SCOPE"]
    published_at = os.environ["PUBLISHED_AT"]

    base_url = f"https://github.com/{repository}/releases/download/v{version}"
    links: dict[str, str] = {}

    if release_scope in {"linux", "all"}:
        links.update(
            {
                "previa_linux_amd64": f"{base_url}/previa-linux-amd64",
                "previa_linux_arm64": f"{base_url}/previa-linux-arm64",
                "previa_main_linux_amd64": f"{base_url}/previa-main-linux-amd64",
                "previa_main_linux_arm64": f"{base_url}/previa-main-linux-arm64",
                "previa_runner_linux_amd64": f"{base_url}/previa-runner-linux-amd64",
                "previa_runner_linux_arm64": f"{base_url}/previa-runner-linux-arm64",
            }
        )

    if release_scope in {"mac", "all"}:
        links["previa_macos_amd64"] = f"{base_url}/previa-macos-amd64"
        links["previa_macos_arm64"] = f"{base_url}/previa-macos-arm64"

    if release_scope in {"windows", "all"}:
        links["previa_windows_amd64"] = f"{base_url}/previa-windows-amd64.exe"

    payload = {
        "name": "previa",
        "version": version,
        "tag": f"v{version}",
        "published_at": published_at,
        "links": links,
    }

    Path("release-metadata.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
