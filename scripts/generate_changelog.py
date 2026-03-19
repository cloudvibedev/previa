#!/usr/bin/env python3

from __future__ import annotations

import argparse
import datetime as dt
import re
import subprocess
from pathlib import Path


HEADER = """# Changelog

All notable changes to Previa are documented in this file.
"""

GROUP_ORDER = [
    "Breaking Changes",
    "Features",
    "Bug Fixes",
    "Documentation",
    "Performance",
    "Refactors",
    "Testing",
    "Maintenance",
    "Other Changes",
]


def run_git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return result.stdout.strip()


def current_tag_exists(tag: str) -> bool:
    result = subprocess.run(
        ["git", "rev-parse", "-q", "--verify", f"refs/tags/{tag}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0


def previous_distinct_tag(current_tag: str) -> str | None:
    tags = run_git("tag", "--sort=-creatordate").splitlines()
    for tag in tags:
        tag = tag.strip()
        if tag and tag != current_tag:
            return tag
    return None


def repo_https_url() -> str | None:
    try:
        remote = run_git("remote", "get-url", "origin")
    except subprocess.CalledProcessError:
        return None

    remote = remote.strip()
    if remote.startswith("git@github.com:"):
        path = remote.split(":", 1)[1]
        if path.endswith(".git"):
            path = path[:-4]
        return f"https://github.com/{path}"
    if remote.startswith("https://github.com/"):
        if remote.endswith(".git"):
            remote = remote[:-4]
        return remote
    return None


def git_log_range(previous_tag: str | None, current_tag: str, target_ref: str) -> str:
    if previous_tag:
        if target_ref == current_tag:
            return f"{previous_tag}..{current_tag}"
        return f"{previous_tag}..{target_ref}"
    return target_ref


def collect_commits(log_range: str) -> list[dict[str, str | bool]]:
    raw = run_git("log", "--format=%H%x01%s%x01%b", log_range)
    if not raw:
        return []

    commits: list[dict[str, str | bool]] = []
    for line in raw.splitlines():
        commit_hash, subject, body = (line.split("\x01", 2) + ["", ""])[:3]
        subject = subject.strip()
        body = body.strip()

        if not subject or subject.startswith("Merge "):
            continue

        group, message = classify_commit(subject, body)
        if not message:
            continue

        commits.append(
            {
                "hash": commit_hash,
                "short_hash": commit_hash[:7],
                "group": group,
                "message": message,
                "breaking": group == "Breaking Changes",
            }
        )

    return commits


def classify_commit(subject: str, body: str) -> tuple[str, str]:
    normalized = subject.strip()
    breaking = "BREAKING CHANGE" in body or re.match(r"^[a-zA-Z0-9_-]+(?:\([^)]+\))?!:", normalized)

    conventional = re.match(
        r"^(?P<type>[a-zA-Z0-9_-]+)(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?:\s*(?P<message>.+)$",
        normalized,
    )

    if conventional:
        commit_type = conventional.group("type").lower()
        message = conventional.group("message").strip()
        scope = conventional.group("scope")
        if scope:
            message = f"{scope}: {message}"
        if breaking or conventional.group("breaking"):
            return "Breaking Changes", sentence_case(message)

        group_map = {
            "feat": "Features",
            "fix": "Bug Fixes",
            "docs": "Documentation",
            "doc": "Documentation",
            "perf": "Performance",
            "refactor": "Refactors",
            "test": "Testing",
            "chore": "Maintenance",
            "build": "Maintenance",
            "ci": "Maintenance",
            "style": "Maintenance",
        }
        return group_map.get(commit_type, "Other Changes"), sentence_case(message)

    if breaking:
        return "Breaking Changes", sentence_case(normalized)

    return "Other Changes", sentence_case(normalized)


def sentence_case(message: str) -> str:
    if not message:
        return message
    return message[0].upper() + message[1:]


def render_section(version: str, previous_tag: str | None, current_tag: str, commits: list[dict[str, str | bool]]) -> str:
    today = dt.date.today().isoformat()
    lines: list[str] = [f"## [{current_tag}] - {today}", ""]

    grouped: dict[str, list[dict[str, str | bool]]] = {name: [] for name in GROUP_ORDER}
    for commit in commits:
        grouped[str(commit["group"])].append(commit)

    for group in GROUP_ORDER:
        entries = grouped[group]
        if not entries:
            continue
        lines.append(f"### {group}")
        for commit in entries:
            lines.append(f"- {commit['message']} ({commit['short_hash']})")
        lines.append("")

    repo_url = repo_https_url()
    if repo_url and previous_tag:
        lines.append(f"Full Changelog: {repo_url}/compare/{previous_tag}...{current_tag}")
        lines.append("")

    return "\n".join(lines).strip() + "\n"


def upsert_section(changelog_path: Path, current_tag: str, section: str) -> None:
    if changelog_path.exists():
        contents = changelog_path.read_text(encoding="utf-8").strip()
    else:
        contents = HEADER.strip()

    if not contents.startswith("# Changelog"):
        contents = HEADER.strip() + "\n\n" + contents

    header = HEADER.strip()
    body = contents[len(header):].strip()

    pattern = re.compile(rf"(?ms)^## \[{re.escape(current_tag)}\] - .*?(?=^## \[|\Z)")
    body = pattern.sub("", body).strip()

    if body:
        updated = f"{header}\n\n{section.rstrip()}\n\n{body}\n"
    else:
        updated = f"{header}\n\n{section.rstrip()}\n"

    changelog_path.write_text(updated, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--release-notes", default="RELEASE_NOTES.md")
    args = parser.parse_args()

    version = args.version
    current_tag = version if version.startswith("v") else f"v{version}"
    target_ref = current_tag if current_tag_exists(current_tag) else "HEAD"
    previous_tag = previous_distinct_tag(current_tag)
    log_range = git_log_range(previous_tag, current_tag, target_ref)
    commits = collect_commits(log_range)

    if not commits:
        commits = [
            {
                "hash": "",
                "short_hash": "",
                "group": "Other Changes",
                "message": "Release published with no categorized changes.",
                "breaking": False,
            }
        ]

    section = render_section(version, previous_tag, current_tag, commits)

    changelog_path = Path(args.changelog)
    upsert_section(changelog_path, current_tag, section)
    Path(args.release_notes).write_text(section, encoding="utf-8")


if __name__ == "__main__":
    main()
