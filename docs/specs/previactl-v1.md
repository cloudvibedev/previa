# `previactl` v1 Specification

## Summary

`previactl` is the local operations CLI for Previa. Version 1 is Linux-first and
local-only: it updates, uninstalls, and manages the `previa-main`,
`previa-runner`, and `previactl` binary version lifecycle on the current host.

This document is implementation-ready. Anything not defined here is out of
scope for v1 and must not be invented during implementation.

## Product Goals

- Detect the current operating system and CPU architecture and pick the correct
  artifact links from the manifest.
- Persist installation state without depending on `previa-main --version` or
  `previa-runner --version`.
- Keep `previa-main`, `previa-runner`, and `previactl` aligned with the latest
  available release during `update`.
- Persist all `previactl`-generated files under `PREVIA_HOME`.
- Reuse the current environment-variable contract already supported by the
  binaries.

## Non-Goals

- Remote provisioning over SSH.
- Fleet or cluster management across multiple hosts.
- Automatic runner registration in external control planes.
- Native service managers such as `systemd`, `launchd`, or Windows Service
  Manager.
- Checksum or signature verification before the manifest exposes those fields.

## External Contract

### Manifest Endpoint

- URL: `GET https://files.previa.dev/manifest.json`
- Content-Type: JSON
- Expected top-level schema:
  - `name: string`
  - `version: string`
  - `create_at: string` in RFC 3339 UTC format
  - `links: object<string, string>`

Example:

```json
{
  "name": "previa",
  "version": "0.0.5",
  "create_at": "2026-03-11T15:53:30Z",
  "links": {
    "previactl_linux_amd64": "https://files.previa.dev/0.0.5/files/previactl-linux-amd64",
    "previa_main_linux_amd64": "https://files.previa.dev/0.0.5/files/previa-main-linux-amd64",
    "previa_runner_linux_amd64": "https://files.previa.dev/0.0.5/files/previa-runner-linux-amd64"
  }
}
```

### Platform Mapping

The CLI must map the detected platform to manifest keys exactly as follows:

| OS | Architecture | `previactl` key | `previa-main` key | `previa-runner` key |
| --- | --- | --- | --- | --- |
| `linux` | `x86_64` | `previactl_linux_amd64` | `previa_main_linux_amd64` | `previa_runner_linux_amd64` |
| `linux` | `aarch64` | `previactl_linux_arm64` | `previa_main_linux_arm64` | `previa_runner_linux_arm64` |
| `macos` | `x86_64` | `previactl_macos_amd64` | `previa_main_macos_amd64` | `previa_runner_macos_amd64` |
| `macos` | `aarch64` | `previactl_macos_arm64` | `previa_main_macos_arm64` | `previa_runner_macos_arm64` |
| `windows` | `x86_64` | `previactl_windows_amd64` | `previa_main_windows_amd64` | `previa_runner_windows_amd64` |

If the current `(os, arch)` pair is not in this table, `previactl` must fail
with an explicit unsupported-platform error before any download begins.

If the pair exists in the table but any required manifest key is missing,
`previactl` must fail with an explicit missing-artifact error and print the
missing key name.

### Versioned Artifact URL Pattern

When `update` needs artifact URLs, it must derive them from the canonical
release layout or the remote manifest:

- `https://files.previa.dev/<version>/files/previactl-<os>-<arch>`
- `https://files.previa.dev/<version>/files/previa-main-<os>-<arch>`
- `https://files.previa.dev/<version>/files/previa-runner-<os>-<arch>`

On Windows, `.exe` is appended to the filename.

## Command Surface

The v1 CLI surface is fixed to the commands below:

```text
previactl update
previactl uninstall [--purge]
previactl version
previactl manifest show
```

No additional v1 commands are required.

### Command Semantics

#### `previactl manifest show`

- Fetches the remote manifest.
- Prints the parsed JSON in a human-readable format.
- Does not write to disk.

#### `previactl update`

- Fetches the remote manifest.
- Reads `PREVIA_HOME/data/install-state.json`.
- Compares the remote manifest version with:
  - the installed `previa-main` version from `PREVIA_HOME/data/install-state.json`
  - the installed `previa-runner` version from `PREVIA_HOME/data/install-state.json`
  - the running `previactl` binary version from compile-time metadata
- If all three already match the remote manifest version, prints
  `already up to date` and exits successfully.
- If one or more components differ, prints a component-by-component summary
  showing the current version and the target version for each differing binary.
- Prompts the user for confirmation before downloading anything.
- Aborts with no changes if the user does not confirm.
- Downloads and atomically replaces each differing component.
- Includes the `previactl` binary itself in the update set when its version is
  behind the remote manifest version.
- Replaces the running `previactl` binary by downloading a temporary file,
  marking it executable, and atomically swapping it into place as the final
  update step.
- Does not overwrite existing config files.
- Updates `PREVIA_HOME/data/install-state.json` after all selected components
  are replaced successfully.
- Does not restart processes that are already running.

#### `previactl uninstall [--purge]`

- Removes installed binaries and `PREVIA_HOME/data/install-state.json`.
- Without `--purge`, preserves `PREVIA_HOME/config` and `PREVIA_HOME/data`.
- With `--purge`, removes the entire `PREVIA_HOME` tree.

#### `previactl version`

- Prints the `previactl` binary version.
- Does not fetch the manifest.
- Does not read `PREVIA_HOME/data/install-state.json`.
- Does not inspect running processes.
- The printed version is the same value used by `update` for
  `previactl` version comparisons.

## Installation Layout

v1 uses `PREVIA_HOME` as the installation base directory across supported
operating systems.

- Environment variable:
  - `PREVIA_HOME`
- Default value when `PREVIA_HOME` is not set:
  - `$HOME/.previa`
- Directory layout:
  - `PREVIA_HOME/bin/previa-main`
  - `PREVIA_HOME/bin/previa-runner`
  - `PREVIA_HOME/config/main.env`
  - `PREVIA_HOME/config/runner.env`
  - `PREVIA_HOME/data/install-state.json`
  - `PREVIA_HOME/data/main/orchestrator.db`
  - `PREVIA_HOME/run/`

Any `previactl` command that writes files must create parent directories as
needed.

## Persistent State

### Install State File

Path: `PREVIA_HOME/data/install-state.json`

Schema:

```json
{
  "name": "previa",
  "platform": {
    "os": "linux",
    "arch": "x86_64"
  },
  "installed_at": "2026-03-11T16:10:00Z",
  "components": {
    "main": {
      "version": "0.0.5",
      "source_url": "https://files.previa.dev/0.0.5/files/previa-main-linux-amd64",
      "path": "/home/assis/.previa/bin/previa-main"
    },
    "runner": {
      "version": "0.0.5",
      "source_url": "https://files.previa.dev/0.0.5/files/previa-runner-linux-amd64",
      "path": "/home/assis/.previa/bin/previa-runner"
    }
  }
}
```

Rules:

- `components.main.version` and `components.runner.version` are the source of
  truth for installed binary version checks.
- `previactl` version is not persisted in `install-state.json`; it is read from
  the running binary metadata.
- The file is rewritten only after all selected update components are replaced
  successfully.
- The file is removed by `uninstall`.
- Partial writes must be avoided by writing to a temporary file in the same
  directory and renaming it into place.

## Configuration Model

`previactl` must reuse the environment variables already supported by the
existing binaries.

### `main.env`

Path: `PREVIA_HOME/config/main.env`

Default content:

```dotenv
ADDRESS=0.0.0.0
PORT=5588
ORCHESTRATOR_DATABASE_URL=sqlite://$HOME/.previa/data/main/orchestrator.db
RUNNER_ENDPOINTS=http://127.0.0.1:55880
RUST_LOG=info
```

Notes:

- `ORCHESTRATOR_DATABASE_URL` must use an absolute path inside
  `PREVIA_HOME/data/main/orchestrator.db`.
- Configuration files are managed outside `previactl` v1 install workflows.
- `update` must not rewrite this file.

### `runner.env`

Path: `PREVIA_HOME/config/runner.env`

Default content:

```dotenv
ADDRESS=0.0.0.0
PORT=55880
RUST_LOG=info
```

Notes:

- `update` must not rewrite this file.

## Installation and Update Flow

### Download Rules

- `update` must use the component URLs resolved from the remote manifest for the
  target version.
- Each binary is first downloaded to a temporary file in the destination
  directory.
- Temporary files are marked executable before the final rename.
- Final replacement uses atomic rename within the same filesystem.
- If any download or rename fails, existing installed binaries must remain in
  place.

### Update Flow

1. Fetch and parse the manifest.
2. Read `PREVIA_HOME/data/install-state.json`; if missing, fail with `not installed`.
3. Read the running `previactl` version.
4. Compare the remote manifest version against `main`, `runner`, and
   `previactl`.
5. If all three already match, print `already up to date` and exit with code
   `0`.
6. Print the list of components with version differences and prompt for user
   confirmation.
7. If the user declines, exit with no changes.
8. Resolve the artifact URLs for each differing component.
9. Download and atomically replace the selected components, updating
   `previactl` last.
10. Rewrite `PREVIA_HOME/data/install-state.json` with the new `main` and
    `runner` version and artifact URLs.
11. Exit without restarting already-running processes.

## Error Handling

The implementation must surface explicit user-facing errors for:

- Unsupported operating system.
- Unsupported CPU architecture.
- Missing manifest keys for the current platform.
- Invalid or incomplete manifest schema.
- HTTP download failures.
- Missing installation state during `update`.
- Failed `previactl` self-replacement during `update`.
- User declined confirmation during `update`.
- Permission failures when writing inside `PREVIA_HOME`.

## Test Plan

The implementation is complete only when these scenarios are covered:

1. `update` with equal local and remote versions for `main`, `runner`, and
   `previactl` prints `already up to date`.
2. `update` with a newer remote version lists the differing components and asks
   the user for confirmation before downloading anything.
3. `update` exits without changes when the user declines the confirmation
   prompt.
4. `update` with a newer remote version replaces every differing component and
   updates `PREVIA_HOME/data/install-state.json`.
5. `update` replaces `previactl` only after updating `main` and `runner`
   successfully.
6. Missing manifest key for the detected platform fails before any binary is
   replaced.
7. Failed download of any selected component leaves the existing installation
   untouched.
8. `version` prints the `previactl` binary version without requiring network or
    installed Previa binaries.
9. `uninstall` without `--purge` removes binaries and runtime state but preserves
    `PREVIA_HOME/config` and `PREVIA_HOME/data`.
10. Any file generated by `previactl` is written under `PREVIA_HOME`.

## Rollback and Recovery

- Automatic rollback is out of scope for v1.
- If `update` fails before atomic rename, the previous installation remains
  authoritative.
- If `update` fails after one component swap but before all selected
  components are replaced, the operator must recover manually by rerunning
  `previactl update` or restoring the previous binaries.

## Security and Known Risks

- The manifest is trusted as the release source of truth.
- No checksum verification is available in v1 because the current manifest does
  not expose checksum fields.
- No signature verification is available in v1.
- Adding checksums and signed release verification is mandatory hardening work
  for v2.

## Implementation Notes

- The future crate will be named `previactl`.
- It should remain separate from HTTP transport concerns and reuse dedicated
  modules for manifest fetching, platform detection, installation state, and
  self-update/process replacement behavior.
- The CLI must target the existing `previa-main` and `previa-runner` contracts
  without requiring changes to those binaries for v1.
