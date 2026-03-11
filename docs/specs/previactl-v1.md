# `previactl` v1 Specification

## Summary

`previactl` is the local operations CLI for Previa. Version 1 is Linux-first and
local-only: it installs, updates, uninstalls, runs, and manages
`previa-main` and `previa-runner` on the current host.

This document is implementation-ready. Anything not defined here is out of
scope for v1 and must not be invented during implementation.

## Product Goals

- Install `previa-main` and `previa-runner` from `https://files.previa.dev/manifest.json`.
- Detect the current operating system and CPU architecture and pick the correct
  artifact links from the manifest.
- Persist installation state without depending on `previa-main --version` or
  `previa-runner --version`.
- Bootstrap a local stack in foreground with one `previa-main` and multiple
  `previa-runner` processes.
- Run `previa-main` and `previa-runner` in foreground for local operations.
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
    "previa_main_linux_amd64": "https://files.previa.dev/0.0.5/files/previa-main-linux-amd64",
    "previa_runner_linux_amd64": "https://files.previa.dev/0.0.5/files/previa-runner-linux-amd64"
  }
}
```

### Platform Mapping

The CLI must map the detected platform to manifest keys exactly as follows:

| OS | Architecture | `previa-main` key | `previa-runner` key |
| --- | --- | --- | --- |
| `linux` | `x86_64` | `previa_main_linux_amd64` | `previa_runner_linux_amd64` |
| `linux` | `aarch64` | `previa_main_linux_arm64` | `previa_runner_linux_arm64` |
| `macos` | `x86_64` | `previa_main_macos_amd64` | `previa_runner_macos_amd64` |
| `macos` | `aarch64` | `previa_main_macos_arm64` | `previa_runner_macos_arm64` |
| `windows` | `x86_64` | `previa_main_windows_amd64` | `previa_runner_windows_amd64` |

If the current `(os, arch)` pair is not in this table, `previactl` must fail
with an explicit unsupported-platform error before any download begins.

If the pair exists in the table but either manifest key is missing,
`previactl` must fail with an explicit missing-artifact error and print the
missing key name.

## Command Surface

The v1 CLI surface is fixed to the commands below:

```text
previactl install [--force-config]
previactl update
previactl uninstall [--purge]
previactl up --runners <N> [-d, --detach]
previactl down
previactl status
previactl run main
previactl run runner
previactl manifest show
```

No additional v1 commands are required.

### Command Semantics

#### `previactl manifest show`

- Fetches the remote manifest.
- Prints the parsed JSON in a human-readable format.
- Does not write to disk.

#### `previactl install [--force-config]`

- Fetches the remote manifest.
- Resolves the current platform.
- Downloads both `previa-main` and `previa-runner`.
- Installs the binaries into the configured installation paths.
- Creates base config files only if they do not already exist.
- If `--force-config` is provided, rewrites base config files even when they
  already exist.
- Writes installation metadata to `install-state.json`.
- Does not create or start system services automatically.

#### `previactl update`

- Fetches the remote manifest.
- Reads `install-state.json`.
- Compares the remote manifest version to the installed version string.
- If versions are equal, prints `already up to date` and exits successfully.
- If the remote version is newer, downloads both binaries and replaces them
  atomically.
- Does not overwrite existing config files.
- Updates `install-state.json` after both binaries are replaced successfully.
- Does not restart processes that are already running.

#### `previactl uninstall [--purge]`

- Stops the detached local stack if `/tmp/previactl-up-state.json` exists.
- Removes installed binaries and `install-state.json`.
- Without `--purge`, preserves `/etc/previa` and `/var/lib/previa`.
- With `--purge`, also removes `/etc/previa` and `/var/lib/previa`.

#### `previactl up --runners <N>`

- Bootstraps a local stack in foreground on the current host.
- Executes exactly one `previa-main` process and exactly the number of
  `previa-runner` processes declared by `--runners <N>`.
- Requires `<N>` to be an integer greater than or equal to `1`.
- Accepts `-d` and `--detach` to leave the spawned processes running in
  background.
- Uses port `55880` for the first runner and increments sequentially for each
  additional runner.
- Builds `RUNNER_ENDPOINTS` for `previa-main` from the runner processes started
  by the command, for example
  `http://127.0.0.1:55880,http://127.0.0.1:55881,http://127.0.0.1:55882`.
- Starts `previa-main` after all runner processes have been spawned.
- Without `-d` or `--detach`, runs all processes in foreground and multiplexes
  their stdout and stderr to the current terminal session.
- Without `-d` or `--detach`, stops all child processes when the command
  receives `SIGINT` or `SIGTERM`.
- With `-d` or `--detach`, writes a temporary runtime file containing the PIDs
  of the spawned processes and then exits successfully.
- Does not rewrite `/etc/previa/main.env` or `/etc/previa/runner.env`.

#### `previactl down`

- Stops a local detached stack started by `previactl up --runners <N> --detach`.
- Reads the temporary runtime file created by detached `up`.
- Sends a termination signal to the recorded `previa-main` PID and to every
  recorded `previa-runner` PID.
- Waits for the recorded processes to exit.
- Removes the temporary runtime file after shutdown completes.
- Fails with a clear error if no detached stack runtime file exists.

#### `previactl status`

- Reports the status of the detached local stack managed by `previactl up`.
- Reads `/tmp/previactl-up-state.json` when it exists.
- Checks whether the recorded `previa-main` PID and `previa-runner` PIDs are
  still alive.
- Prints `stopped` when the runtime file does not exist.
- Prints `running` with the recorded PIDs and ports when all recorded processes
  are alive.
- Prints `degraded` when the runtime file exists but one or more recorded PIDs
  are no longer alive.
- Does not interact with native service managers.

#### `previactl run main`

- Loads `/etc/previa/main.env`.
- Starts `/opt/previa/bin/previa-main` in foreground.
- Inherits stdout and stderr in the current terminal session.

#### `previactl run runner`

- Loads `/etc/previa/runner.env`.
- Starts `/opt/previa/bin/previa-runner` in foreground.
- Inherits stdout and stderr in the current terminal session.

## Installation Layout

The Linux installation layout is fixed for v1:

- Binaries:
  - `/opt/previa/bin/previa-main`
  - `/opt/previa/bin/previa-runner`
- Config:
  - `/etc/previa/main.env`
  - `/etc/previa/runner.env`
- State:
  - `/var/lib/previa/install-state.json`
- Default orchestrator database:
  - `/var/lib/previa/main/orchestrator.db`

The installer must create parent directories as needed.

## Persistent State

### Install State File

Path: `/var/lib/previa/install-state.json`

Schema:

```json
{
  "name": "previa",
  "version": "0.0.5",
  "platform": {
    "os": "linux",
    "arch": "x86_64"
  },
  "installed_at": "2026-03-11T16:10:00Z",
  "artifacts": {
    "main": {
      "source_url": "https://files.previa.dev/0.0.5/files/previa-main-linux-amd64",
      "path": "/opt/previa/bin/previa-main"
    },
    "runner": {
      "source_url": "https://files.previa.dev/0.0.5/files/previa-runner-linux-amd64",
      "path": "/opt/previa/bin/previa-runner"
    }
  }
}
```

Rules:

- `version` is the source of truth for installed version checks.
- The file is written only after both binaries are installed successfully.
- The file is removed by `uninstall`.
- Partial writes must be avoided by writing to a temporary file in the same
  directory and renaming it into place.

## Configuration Model

`previactl` must reuse the environment variables already supported by the
existing binaries.

### `main.env`

Path: `/etc/previa/main.env`

Default content:

```dotenv
ADDRESS=0.0.0.0
PORT=5588
ORCHESTRATOR_DATABASE_URL=sqlite:///var/lib/previa/main/orchestrator.db
RUNNER_ENDPOINTS=http://127.0.0.1:55880
RUST_LOG=info
```

Notes:

- `ORCHESTRATOR_DATABASE_URL` must use the absolute path shown above.
- `RUNNER_ENDPOINTS` defaults to the local runner process started by the same
  host.
- If the file already exists, `install` and `update` must leave it unchanged
  unless `install --force-config` is used.

### `runner.env`

Path: `/etc/previa/runner.env`

Default content:

```dotenv
ADDRESS=0.0.0.0
PORT=55880
RUST_LOG=info
```

Notes:

- If the file already exists, `install` and `update` must leave it unchanged
  unless `install --force-config` is used.

## Local Bootstrap Rules

`previactl up --runners <N>` is the v1 bootstrap command for local development
or single-host evaluation.

Rules:

- It is local-only and does not provision remote hosts.
- It uses the installed binaries from `/opt/previa/bin`.
- It always executes one `previa-main`.
- It always executes exactly the runner count declared by the operator in
  `--runners <N>`.
- `previa-main` binds to the configured `ADDRESS` and `PORT` from
  `/etc/previa/main.env` when present.
- Each runner binds to `127.0.0.1` and uses ports starting at `55880`.
- The command must override `RUNNER_ENDPOINTS` for the `previa-main` child
  process so that it points to the runners spawned by the same command.
- The runner count is explicit; there is no default implicit runner fan-out in
  v1.
- If any child process fails during startup, the command must terminate the
  remaining children and exit with a non-zero status.

## Detached Bootstrap Rules

Detached local bootstrap uses a single temporary runtime file:

- Path: `/tmp/previactl-up-state.json`
- Ownership: the user who launched `previactl up --detach`
- Multiplicity: only one detached `previactl up` stack is supported per host in
  v1

Runtime file schema:

```json
{
  "mode": "detached",
  "started_at": "2026-03-11T16:25:00Z",
  "main": {
    "pid": 41021,
    "port": 5588
  },
  "runners": [
    {
      "pid": 41022,
      "port": 55880
    },
    {
      "pid": 41023,
      "port": 55881
    }
  ]
}
```

Rules:

- `previactl up --detach` must fail if `/tmp/previactl-up-state.json` already
  exists.
- The runtime file is written only after all child processes have been spawned
  successfully.
- The runtime file must be written atomically by writing a temporary file in
  `/tmp` and renaming it into place.
- `previactl down` reads this file, terminates the recorded processes, waits for
  them to stop, and then removes the file.
- `previactl status` reads this file and reports `running`, `degraded`, or
  `stopped` based on file presence and PID liveness.
- If one or more recorded PIDs no longer exist, `down` continues shutting down
  the remaining recorded processes and still removes the runtime file.
- Detached mode must not create unit files or call native service managers.

## Installation and Update Flow

### Download Rules

- Downloads must use the URLs from the manifest without rewriting them.
- Each binary is first downloaded to a temporary file in the destination
  directory.
- Temporary files are marked executable before the final rename.
- Final replacement uses atomic rename within the same filesystem.
- If any download or rename fails, existing installed binaries must remain in
  place.

### Install Flow

1. Fetch and parse the manifest.
2. Validate `name == "previa"`.
3. Detect platform and resolve the two artifact URLs.
4. Create required directories.
5. Download and atomically install both binaries.
6. Create default config files if absent, or rewrite them only with
   `--force-config`.
7. Write `install-state.json`.
8. Exit without touching already-running processes.

### Update Flow

1. Fetch and parse the manifest.
2. Read `install-state.json`; if missing, fail with `not installed`.
3. Compare remote `version` to installed `version`.
4. If equal, print `already up to date` and exit with code `0`.
5. Resolve current platform URLs from the remote manifest.
6. Download and atomically replace both binaries.
7. Rewrite `install-state.json` with the new version and artifact URLs.
8. Exit without restarting already-running processes.

## Error Handling

The implementation must surface explicit user-facing errors for:

- Unsupported operating system.
- Unsupported CPU architecture.
- Missing manifest keys for the current platform.
- Invalid or incomplete manifest schema.
- HTTP download failures.
- Missing installation state during `update`.
- Missing installed binary during `run`.
- Existing detached runtime file during `up --detach`.
- Missing detached runtime file during `down`.
- Permission failures when writing to `/opt`, `/etc`, `/var/lib`, or `/tmp`.

## Test Plan

The implementation is complete only when these scenarios are covered:

1. Clean install on Linux `x86_64` with a valid manifest installs both binaries
   and writes `install-state.json`.
2. Clean install on Linux `aarch64` resolves the `_arm64` manifest keys.
3. `update` with equal local and remote versions prints `already up to date`.
4. `update` with a newer remote version replaces both binaries and updates
   `install-state.json`.
5. Missing manifest key for the detected platform fails before any binary is
   replaced.
6. Failed download of either binary leaves the existing installation untouched.
7. `run main` starts `previa-main` with
   `ORCHESTRATOR_DATABASE_URL=sqlite:///var/lib/previa/main/orchestrator.db`
   when the default config is used.
8. `run runner` starts `previa-runner` with default `ADDRESS=0.0.0.0` and
   `PORT=55880` when the default config is used.
9. `up --runners 3` starts one `previa-main`, three local runners, and injects
    `RUNNER_ENDPOINTS=http://127.0.0.1:55880,http://127.0.0.1:55881,http://127.0.0.1:55882`
    into the `previa-main` child process.
10. `up --runners 0` fails validation before spawning any process.
11. `up --runners 3 --detach` writes `/tmp/previactl-up-state.json` with the
    `previa-main` PID and the three runner PIDs, then exits without stopping
    the spawned processes.
12. `status` reports `running` when all PIDs in
    `/tmp/previactl-up-state.json` are alive.
13. `status` reports `degraded` when the runtime file exists but one or more
    recorded PIDs are no longer alive.
14. `status` reports `stopped` when no detached runtime file exists.
15. `down` reads `/tmp/previactl-up-state.json`, terminates the recorded
    processes, waits for shutdown, and removes the runtime file.
16. `down` fails clearly when no detached runtime file exists.
17. `up --detach` fails clearly when `/tmp/previactl-up-state.json` already
    exists.
18. `uninstall` without `--purge` removes binaries and runtime state but preserves
    `/etc/previa` and `/var/lib/previa`.
19. Reinstall after non-purge uninstall reuses the preserved config files.

## Rollback and Recovery

- Automatic rollback is out of scope for v1.
- If `update` fails before atomic rename, the previous installation remains
  authoritative.
- If `update` fails after one binary swap but before the second swap, the
  operator must recover manually by rerunning `previactl update` or restoring
  the previous binaries.

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
  modules for manifest fetching, platform detection, installation state,
  detached stack state, and process execution.
- The CLI must target the existing `previa-main` and `previa-runner` contracts
  without requiring changes to those binaries for v1.
