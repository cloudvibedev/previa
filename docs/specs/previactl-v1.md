# `previactl` v1 Specification

## Summary

`previactl` is the local operations CLI for Previa. Version 1 is Linux-first and
local-only: it installs, updates, uninstalls, and manages `previa-main`,
`previa-runner`, and the `previactl` binary version lifecycle on the current
host.

This document is implementation-ready. Anything not defined here is out of
scope for v1 and must not be invented during implementation.

## Product Goals

- Install `previa-main` and `previa-runner` from `https://files.previa.dev/manifest.json`.
- Detect the current operating system and CPU architecture and pick the correct
  artifact links from the manifest.
- Persist installation state without depending on `previa-main --version` or
  `previa-runner --version`.
- Keep `previa-main`, `previa-runner`, and `previactl` aligned with the latest
  available release during `update`.
- Bootstrap a local stack in foreground with one `previa-main` and multiple
  `previa-runner` processes.
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

When `install` needs the release that matches the running `previactl` version,
it must derive artifact URLs from the canonical release layout:

- `https://files.previa.dev/<version>/files/previactl-<os>-<arch>`
- `https://files.previa.dev/<version>/files/previa-main-<os>-<arch>`
- `https://files.previa.dev/<version>/files/previa-runner-<os>-<arch>`

On Windows, `.exe` is appended to the filename.

## Command Surface

The v1 CLI surface is fixed to the commands below:

```text
previactl install [--force-config]
previactl update
previactl uninstall [--purge]
previactl up [--runners, -r <N>] [--attach-runner, -a <endpoint> ...] [-d, --detach]
previactl down
previactl status
previactl version
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
- Resolves the current `previactl` binary version from compile-time metadata.
- Installs only `previa-main` and `previa-runner` at the exact same version as
  the running `previactl`.
- Does not upgrade or downgrade the running `previactl` binary.
- Fails if the release matching the running `previactl` version is not
  available in the remote artifact layout.
- Resolves release URLs for `previa-main` and `previa-runner` by combining the
  running `previactl` version with the canonical versioned artifact URL pattern.
- Installs the binaries into the configured installation paths.
- Creates base config files only if they do not already exist.
- If `--force-config` is provided, rewrites base config files even when they
  already exist.
- Writes installation metadata to `install-state.json`.

#### `previactl update`

- Fetches the remote manifest.
- Reads `install-state.json`.
- Compares the remote manifest version with:
  - the installed `previa-main` version from `install-state.json`
  - the installed `previa-runner` version from `install-state.json`
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
- Updates `install-state.json` after all selected components are replaced
  successfully.
- Does not restart processes that are already running.

#### `previactl uninstall [--purge]`

- Stops the detached local stack if `PREVIA_HOME/run/up-state.json` exists.
- Removes installed binaries and `install-state.json`.
- Without `--purge`, preserves `PREVIA_HOME/config` and `PREVIA_HOME/data`.
- With `--purge`, removes the entire `PREVIA_HOME` tree.

#### `previactl up [--runners, -r <N>] [--attach-runner, -a <endpoint> ...]`

- Bootstraps a local stack in foreground on the current host.
- Executes exactly one `previa-main` process.
- Optionally spawns the number of local `previa-runner` processes declared by
  `--runners <N>` or `-r <N>`.
- Optionally attaches one or more existing runner endpoints provided through
  repeated `--attach-runner <endpoint>` or `-a <endpoint>` flags.
- Accepts `<endpoint>` values as full HTTP base URLs such as
  `http://10.0.0.12:55880`.
- Requires at least one runner source overall: either `--runners <N>` greater
  than `0`, at least one `--attach-runner` / `-a`, or both.
- When `--runners` is omitted, it defaults to `1`.
- When present, `--runners <N>` must be an integer greater than or equal to
  `0`.
- Accepts `-d` and `--detach` to leave the spawned processes running in
  background.
- Uses port `55880` for the first runner and increments sequentially for each
  additional local runner.
- Builds `RUNNER_ENDPOINTS` for `previa-main` by concatenating:
  - the local runner processes started by the command, in local port order
  - the attached runner endpoints provided via `--attach-runner` / `-a`, in
    CLI order
- Example:
  `http://127.0.0.1:55880,http://127.0.0.1:55881,http://10.0.0.12:55880`.
- Starts `previa-main` after all runner processes have been spawned.
- Without `-d` or `--detach`, runs all processes in foreground and multiplexes
  their stdout and stderr to the current terminal session.
- Without `-d` or `--detach`, stops all child processes when the command
  receives `SIGINT` or `SIGTERM`.
- With `-d` or `--detach`, writes a temporary runtime file containing the PIDs
  of the spawned processes and then exits successfully.
- Does not rewrite `PREVIA_HOME/config/main.env` or
  `PREVIA_HOME/config/runner.env`.

#### `previactl down`

- Stops a local detached stack started by `previactl up --detach`.
- Reads the temporary runtime file created by detached `up`.
- Sends a termination signal to the recorded `previa-main` PID and to every
  recorded `previa-runner` PID.
- Waits for the recorded processes to exit.
- Removes the temporary runtime file after shutdown completes.
- Fails with a clear error if no detached stack runtime file exists.
- Does not send termination signals to attached runner endpoints because they
  are not child processes of `previactl`.

#### `previactl status`

- Reports the status of the detached local stack managed by `previactl up`.
- Reads `PREVIA_HOME/run/up-state.json` when it exists.
- Checks whether the recorded `previa-main` PID and `previa-runner` PIDs are
  still alive.
- Prints `stopped` when the runtime file does not exist.
- Prints `running` with the recorded PIDs and ports when all recorded processes
  are alive.
- Prints `degraded` when the runtime file exists but one or more recorded PIDs
  are no longer alive.
- Does not interact with native service managers.

#### `previactl version`

- Prints the `previactl` binary version.
- Does not fetch the manifest.
- Does not read `install-state.json`.
- Does not inspect running processes.
- The printed version is the same value used by `install` and `update` for
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
  - `PREVIA_HOME/run/up-state.json`

The installer must create parent directories as needed.

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
- The file is written only after both binaries are installed successfully.
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
- `RUNNER_ENDPOINTS` defaults to the local runner process started by the same
  host.
- If the file already exists, `install` and `update` must leave it unchanged
  unless `install --force-config` is used.

### `runner.env`

Path: `PREVIA_HOME/config/runner.env`

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

`previactl up` is the v1 bootstrap command for local development, single-host
evaluation, and hybrid local-plus-remote runner attachment.

Rules:

- It is local-only and does not provision remote hosts.
- It uses the installed binaries from `PREVIA_HOME/bin`.
- It always executes one `previa-main`.
- It executes exactly the local runner count declared by the operator in
  `--runners <N>` or `-r <N>`.
- It may attach existing runner endpoints declared through repeated
  `--attach-runner <endpoint>` or `-a <endpoint>` flags.
- It must reject `up` if `--runners 0` / `-r 0` is combined with no
  `--attach-runner` / `-a`.
- `previa-main` binds to the configured `ADDRESS` and `PORT` from
  `PREVIA_HOME/config/main.env` when present.
- Each local spawned runner binds to `127.0.0.1` and uses ports starting at
  `55880`.
- The command must override `RUNNER_ENDPOINTS` for the `previa-main` child
  process so that it points to all local spawned runners followed by all
  attached runner endpoints.
- Attached runner endpoints are treated as externally managed and are never
  spawned, restarted, or terminated by `previactl`.
- If any child process fails during startup, the command must terminate the
  remaining children and exit with a non-zero status.

## Detached Bootstrap Rules

Detached local bootstrap uses a single temporary runtime file:

- Path: `PREVIA_HOME/run/up-state.json`
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
  "attached_runners": [
    "http://10.0.0.12:55880"
  ],
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

- `previactl up --detach` must fail if `PREVIA_HOME/run/up-state.json` already
  exists.
- The runtime file is written only after all child processes have been spawned
  successfully.
- The runtime file must be written atomically by writing a temporary file in
  `PREVIA_HOME/run` and renaming it into place.
- `previactl down` reads this file, terminates the recorded processes, waits for
  them to stop, and then removes the file.
- `previactl status` reads this file and reports `running`, `degraded`, or
  `stopped` based on file presence and PID liveness.
- The runtime file must persist attached runner endpoints for status reporting
  and `RUNNER_ENDPOINTS` introspection.
- If one or more recorded PIDs no longer exist, `down` continues shutting down
  the remaining recorded processes and still removes the runtime file.
- Detached mode must not create unit files or call native service managers.

## Installation and Update Flow

### Download Rules

- `update` must use the component URLs resolved from the remote manifest for the
  target version.
- `install` must use the canonical versioned artifact URL pattern for the
  running `previactl` version.
- Each binary is first downloaded to a temporary file in the destination
  directory.
- Temporary files are marked executable before the final rename.
- Final replacement uses atomic rename within the same filesystem.
- If any download or rename fails, existing installed binaries must remain in
  place.

### Install Flow

1. Fetch and parse the manifest.
2. Validate `name == "previa"`.
3. Resolve the running `previactl` version.
4. Detect platform and resolve the `previa-main` and `previa-runner` artifact
   URLs for that exact `previactl` version using the canonical versioned
   artifact URL pattern.
5. Create required directories.
6. Download and atomically install both binaries.
7. Create default config files if absent, or rewrite them only with
   `--force-config`.
8. Write `install-state.json`.
9. Exit without touching already-running processes.

### Update Flow

1. Fetch and parse the manifest.
2. Read `install-state.json`; if missing, fail with `not installed`.
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
10. Rewrite `install-state.json` with the new `main` and `runner` version and
    artifact URLs.
11. Exit without restarting already-running processes.

## Error Handling

The implementation must surface explicit user-facing errors for:

- Unsupported operating system.
- Unsupported CPU architecture.
- Missing manifest keys for the current platform.
- Invalid or incomplete manifest schema.
- HTTP download failures.
- Missing release artifacts for the current `previactl` version during
  `install`.
- Missing installation state during `update`.
- Failed `previactl` self-replacement during `update`.
- User declined confirmation during `update`.
- Invalid `--attach-runner <endpoint>` / `-a <endpoint>` value.
- Existing detached runtime file during `up --detach`.
- Missing detached runtime file during `down`.
- Permission failures when writing inside `PREVIA_HOME`.

## Test Plan

The implementation is complete only when these scenarios are covered:

1. Clean install on Linux `x86_64` with a valid manifest installs both binaries
   at the same version as the running `previactl` and writes
   `install-state.json`.
2. Clean install on Linux `aarch64` resolves the `_arm64` manifest keys.
3. `install` fails clearly when the release matching the running `previactl`
   version is not available remotely.
4. `update` with equal local and remote versions for `main`, `runner`, and
   `previactl` prints `already up to date`.
5. `update` with a newer remote version lists the differing components and asks
   the user for confirmation before downloading anything.
6. `update` exits without changes when the user declines the confirmation
   prompt.
7. `update` with a newer remote version replaces every differing component and
   updates `install-state.json`.
8. `update` replaces `previactl` only after updating `main` and `runner`
   successfully.
9. Missing manifest key for the detected platform fails before any binary is
   replaced.
10. Failed download of any selected component leaves the existing installation
    untouched.
11. `version` prints the `previactl` binary version without requiring network or
    installed Previa binaries.
12. `up --runners 3` starts one `previa-main`, three local runners, and injects
    `RUNNER_ENDPOINTS=http://127.0.0.1:55880,http://127.0.0.1:55881,http://127.0.0.1:55882`
    into the `previa-main` child process.
13. `up -r 1 -a http://10.0.0.12:55880` injects
    `RUNNER_ENDPOINTS=http://127.0.0.1:55880,http://10.0.0.12:55880`
    into the `previa-main` child process.
14. `up -r 0 -a http://10.0.0.12:55880` is valid and starts
    only `previa-main` locally while attaching the remote runner endpoint.
15. `up -r 0` with no attached runner fails validation before spawning
    any process.
16. `up -a 10.0.0.12:55880` fails clearly because the attached
    runner endpoint is not a full HTTP base URL.
17. `up -r 3 --detach` writes `PREVIA_HOME/run/up-state.json` with the
    `previa-main` PID and the three runner PIDs, then exits without stopping
    the spawned processes.
18. Detached runtime state persists attached runner endpoints when
    `--attach-runner` or `-a` is used.
19. `status` reports `running` when all PIDs in
    `PREVIA_HOME/run/up-state.json` are alive.
20. `status` reports `degraded` when the runtime file exists but one or more
    recorded PIDs are no longer alive.
21. `status` reports `stopped` when no detached runtime file exists.
22. `down` reads `PREVIA_HOME/run/up-state.json`, terminates the recorded
    processes, waits for shutdown, and removes the runtime file.
23. `down` fails clearly when no detached runtime file exists.
24. `down` does not attempt to terminate attached runner endpoints.
25. `up --detach` fails clearly when `PREVIA_HOME/run/up-state.json` already
    exists.
26. `uninstall` without `--purge` removes binaries and runtime state but preserves
    `PREVIA_HOME/config` and `PREVIA_HOME/data`.
27. Reinstall after non-purge uninstall reuses the preserved config files.

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
  modules for manifest fetching, platform detection, installation state,
  detached stack state, and process execution.
- The CLI must target the existing `previa-main` and `previa-runner` contracts
  without requiring changes to those binaries for v1.
