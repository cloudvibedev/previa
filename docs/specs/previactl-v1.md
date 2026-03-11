# `previactl` v1 Specification

## Summary

`previactl` is the local operations CLI for Previa. Version 1 is Linux-first and
local-only: it runs and manages a local Previa stack, inspects the published
release manifest, exposes the `previactl` version, and cleans local Previa
artifacts under `PREVIA_HOME`.

This document is implementation-ready. Anything not defined here is out of
scope for v1 and must not be invented during implementation.

## Product Goals

- Bootstrap a local stack with one `previa-main` and multiple
  `previa-runner` processes.
- Allow attaching existing runner endpoints that are already running.
- Support foreground and detached execution modes.
- Persist all `previactl`-generated files under `PREVIA_HOME`.
- Reuse the current environment-variable contract already supported by the
  binaries.
- Expose the remote release manifest and the local `previactl` version.

## Non-Goals

- Installing binaries for Linux, macOS, or Windows in v1.
- Updating binaries in v1.
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

`previactl manifest show` only fetches and prints this document. It does not
download or install binaries in v1.

## Command Surface

The v1 CLI surface is fixed to the commands below:

```text
previactl up [--runners, -r <N>] [--attach-runner, -a <address|address:port|port> ...] [-d, --detach]
previactl down [--runner <address|address:port|port> ...]
previactl restart
previactl status [--main] [--runner <address|address:port|port>]
previactl version
previactl manifest show
```

No additional v1 commands are required.

### Command Semantics

#### `previactl manifest show`

- Fetches the remote manifest.
- Prints the parsed JSON in a human-readable format.
- Does not write to disk.

#### `previactl up [--runners, -r <N>] [--attach-runner, -a <address|address:port|port> ...]`

- Bootstraps a local stack on the current host.
- Executes exactly one `previa-main` process.
- Optionally spawns the number of local `previa-runner` processes declared by
  `--runners <N>` or `-r <N>`.
- Optionally attaches one or more existing runner targets provided through
  repeated `--attach-runner <selector>` or `-a <selector>` flags.
- `--attach-runner <selector>` accepts:
  - `port`, for example `55880`
  - `address:port`, for example `10.0.0.12:55880`
  - `address`, for example `10.0.0.12`
- `port` is normalized to `http://127.0.0.1:<port>`.
- `address:port` is normalized to `http://<address>:<port>`.
- `address` is normalized to `http://<address>:55880`.
- `up` must persist attached runners in normalized full-URL form.
- Requires at least one runner source overall: either `--runners <N>` greater
  than `0`, at least one `--attach-runner` / `-a`, or both.
- When `--runners` is omitted, it defaults to `1`.
- When present, `--runners <N>` must be an integer greater than or equal to
  `0`.
- Accepts `-d` and `--detach` to leave the spawned processes running in
  background.
- Uses port `55880` for the first local runner and increments sequentially for
  each additional local runner.
- Builds `RUNNER_ENDPOINTS` for `previa-main` by concatenating:
  - the local runner processes started by the command, in local port order
  - the attached runner endpoints provided via `--attach-runner` / `-a`, in
    CLI order after normalization
- Example:
  `http://127.0.0.1:55880,http://127.0.0.1:55881,http://10.0.0.12:55880`
- Starts `previa-main` after all local runner processes have been spawned.
- Without `-d` or `--detach`, runs all child processes in foreground and
  multiplexes their stdout and stderr to the current terminal session.
- Without `-d` or `--detach`, stops all child processes when the command
  receives `SIGINT` or `SIGTERM`.
- With `-d` or `--detach`, writes `PREVIA_HOME/run/up-state.json` and then
  exits successfully.
- Does not rewrite `PREVIA_HOME/config/main.env` or
  `PREVIA_HOME/config/runner.env`.

#### `previactl down [--runner <address|address:port|port> ...]`

- Stops a local detached stack started by `previactl up --detach`.
- Reads `PREVIA_HOME/run/up-state.json`.
- Without `--runner`, sends a termination signal to the recorded
  `previa-main` PID and to every recorded local `previa-runner` PID.
- Without `--runner`, waits for the recorded local processes to exit and
  removes `PREVIA_HOME/run/up-state.json` after shutdown completes.
- With one or more `--runner <selector>` flags, sends termination signals only
  to the matching recorded local runner PIDs.
- With one or more `--runner <selector>` flags, rewrites
  `PREVIA_HOME/run/up-state.json` after removing the stopped local runner
  entries and preserving the `previa-main` PID plus any remaining local runners
  and attached runner endpoints.
- `--runner <selector>` accepts:
  - `port`, for example `55880`
  - `address:port`, for example `127.0.0.1:55880`
  - `address`, for example `127.0.0.1`
- Matching rules:
  - `port` matches local runner entries with the same `port`
  - `address:port` matches local runner entries with both the same address and
    the same port
  - `address` matches all local runner entries with the same address
- The runtime file must store the local runner bind address for each runner
  entry so that selector matching is deterministic.
- Partial runner shutdown must fail if none of the requested selectors match a
  local runner entry in the runtime file.
- Partial runner shutdown must fail if it would leave the stack with zero
  runner sources overall, meaning no remaining local runners and no attached
  runner endpoints.
- Fails with a clear error if no detached runtime file exists.
- Does not send termination signals to attached runner endpoints because they
  are not child processes of `previactl`.

#### `previactl restart`

- Restarts a detached local stack previously started by `previactl up --detach`.
- Reads `PREVIA_HOME/run/up-state.json`.
- Stops the recorded local processes using the same behavior as `previactl down`.
- Starts a new detached stack using the same effective configuration recorded in
  the runtime file:
  - the local runner count from the recorded local runner entries
  - the attached runner endpoints from `attached_runners`
- Rewrites `PREVIA_HOME/run/up-state.json` with the new PIDs after the new stack
  starts successfully.
- Fails with a clear error if no detached runtime file exists.
- Does not send termination signals to attached runner endpoints.

#### `previactl status [--main] [--runner <address|address:port|port>]`

- Reports the status of the detached local stack managed by `previactl up`.
- Reads `PREVIA_HOME/run/up-state.json` when it exists.
- Without filters, checks whether the recorded `previa-main` PID and local
  `previa-runner` PIDs are still alive and reports the overall stack status.
- With `--main`, reports only the status of the recorded `previa-main` PID.
- With `--runner <selector>`, reports only the status of the recorded local
  runner or local runners that match the given selector.
- `--main` and `--runner <selector>` are mutually exclusive in v1.
- Without filters, prints `stopped` when the runtime file does not exist.
- Without filters, prints `running` with the recorded PIDs, ports, and attached
  runner endpoints when all recorded local processes are alive.
- Without filters, prints `degraded` when the runtime file exists but one or
  more recorded local PIDs are no longer alive.
- With `--main`, prints `running` or `stopped` for the `previa-main` process.
- With `--runner <selector>`, prints `running` or `stopped` for the selected
  local runner or runners.
- `--runner <selector>` accepts:
  - `port`, for example `55880`
  - `address:port`, for example `127.0.0.1:55880`
  - `address`, for example `127.0.0.1`
- `status --runner <selector>` must fail clearly when the requested selector
  does not match any local runner entry in the runtime file.
- Does not interact with native service managers.

#### `previactl version`

- Prints the `previactl` binary version.
- Does not fetch the manifest.
- Does not read `PREVIA_HOME/data/install-state.json`.
- Does not inspect running processes.

## Filesystem Layout

v1 uses `PREVIA_HOME` as the base directory for all `previactl`-generated
files.

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

Any `previactl` command that writes files must create parent directories as
needed.

## Runtime State

### Detached Runtime File

Path: `PREVIA_HOME/run/up-state.json`

Schema:

```json
{
  "mode": "detached",
  "started_at": "2026-03-11T16:25:00Z",
  "main": {
    "pid": 41021,
    "port": 5588
  },
  "attached_runners": ["http://10.0.0.12:55880"],
  "runners": [
    {
      "address": "127.0.0.1",
      "pid": 41022,
      "port": 55880
    },
    {
      "address": "127.0.0.1",
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
- `previactl down` reads this file, terminates the recorded local processes,
  waits for them to stop, and then removes the file when stopping the full
  stack.
- `previactl down --runner <selector>` rewrites this file after removing the
  selected local runner entries.
- `previactl restart` reads this file, stops the recorded local processes, and
  uses the recorded local runner count plus `attached_runners` to launch a new
  detached stack.
- `previactl status` reads this file and reports `running`, `degraded`, or
  `stopped` based on file presence and PID liveness.
- The runtime file must persist attached runner endpoints for status reporting
  and `RUNNER_ENDPOINTS` introspection in normalized full-URL form.
- If one or more recorded local PIDs no longer exist, `down` continues shutting
  down the remaining recorded local processes and still removes the runtime
  file.

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
- `up` reads this file when present and must not rewrite it.

### `runner.env`

Path: `PREVIA_HOME/config/runner.env`

Default content:

```dotenv
ADDRESS=0.0.0.0
PORT=55880
RUST_LOG=info
```

Notes:

- `up` reads this file when present and must not rewrite it.

## Runtime Rules

`previactl up` is the v1 bootstrap command for local development, single-host
evaluation, and hybrid local-plus-remote runner attachment.

Rules:

- It is local-only and does not provision remote hosts.
- It uses the installed binaries from `PREVIA_HOME/bin`.
- It always executes one `previa-main`.
- It executes exactly the local runner count declared by the operator in
  `--runners <N>` or `-r <N>`.
- It may attach existing runner targets declared through repeated
  `--attach-runner <selector>` or `-a <selector>` flags.
- It must reject `up` if `--runners 0` / `-r 0` is combined with no
  `--attach-runner` / `-a`.
- `previa-main` binds to the configured `ADDRESS` and `PORT` from
  `PREVIA_HOME/config/main.env` when present.
- Each local spawned runner binds to `127.0.0.1` and uses ports starting at
  `55880`.
- The command must override `RUNNER_ENDPOINTS` for the `previa-main` child
  process so that it points to all local spawned runners followed by all
  attached runner endpoints after selector normalization.
- Attached runner endpoints are treated as externally managed and are never
  spawned, restarted, or terminated by `previactl`.
- If any child process fails during startup, the command must terminate the
  remaining local children and exit with a non-zero status.

## Error Handling

The implementation must surface explicit user-facing errors for:

- Missing `PREVIA_HOME/bin/previa-main`.
- Missing `PREVIA_HOME/bin/previa-runner` when local runners are requested.
- Invalid `--attach-runner <selector>` / `-a <selector>` value.
- Existing detached runtime file during `up --detach`.
- Missing detached runtime file during `down`.
- Unknown local runner selector during `down --runner <selector>`.
- Attempted `down --runner <selector>` that would leave the stack with zero runner
  sources.
- Missing detached runtime file during `restart`.
- Mutually exclusive `status --main` and `status --runner <selector>`.
- Unknown local runner selector during `status --runner <selector>`.
- Permission failures when writing inside `PREVIA_HOME`.
- Failure to spawn `previa-main` or one of the local `previa-runner`
  processes.

## Test Plan

The implementation is complete only when these scenarios are covered:

1. `manifest show` fetches and prints the remote manifest without writing files.
2. `version` prints the `previactl` binary version without requiring network.
3. `up -r 3` starts one `previa-main`, three local runners, and injects
   `RUNNER_ENDPOINTS=http://127.0.0.1:55880,http://127.0.0.1:55881,http://127.0.0.1:55882`
   into the `previa-main` child process.
4. `up -r 1 -a 10.0.0.12:55880` injects
   `RUNNER_ENDPOINTS=http://127.0.0.1:55880,http://10.0.0.12:55880`
   into the `previa-main` child process.
5. `up -r 0 -a 10.0.0.12:55880` is valid and starts only `previa-main`
   locally while attaching the remote runner endpoint.
6. `up -r 0` with no attached runner fails validation before spawning any
   process.
7. `up -a 55880` normalizes the attached runner target to
   `http://127.0.0.1:55880`.
8. `up -a 10.0.0.12` normalizes the attached runner target to
   `http://10.0.0.12:55880`.
9. `up -a 10.0.0.12:55880` normalizes the attached runner target to
   `http://10.0.0.12:55880`.
10. `up -a bad:value:123` fails clearly because the attached runner selector is
    invalid.
11. `up -r 3 --detach` writes `PREVIA_HOME/run/up-state.json` with the
   `previa-main` PID and the three runner PIDs, then exits without stopping the
   spawned processes.
12. Detached runtime state persists attached runner endpoints when
   `--attach-runner` or `-a` is used.
13. `status` reports `running` when all PIDs in `PREVIA_HOME/run/up-state.json`
    are alive.
14. `status` reports `degraded` when the runtime file exists but one or more
    recorded local PIDs are no longer alive.
15. `status` reports `stopped` when no detached runtime file exists.
16. `status --main` reports only the status of the recorded `previa-main`
    process.
17. `status --runner 55880` reports the status of the recorded local runner on
    port `55880`.
18. `status --runner 127.0.0.1:55880` reports the status of the recorded local
    runner bound to `127.0.0.1:55880`.
19. `status --runner 127.0.0.1` reports the status of all recorded local
    runners bound to `127.0.0.1`.
20. `status --runner 55880` fails clearly when the selector does not match any
    local runner entry in the runtime file.
21. `status --main --runner 55880` fails clearly because the filters are
    mutually exclusive.
22. `down` reads `PREVIA_HOME/run/up-state.json`, terminates the recorded local
    processes, waits for shutdown, and removes the runtime file.
23. `down` fails clearly when no detached runtime file exists.
24. `down --runner 55880` stops only the recorded local runner on port `55880`
    and rewrites `PREVIA_HOME/run/up-state.json` with the remaining runner
    entries.
25. `down --runner 127.0.0.1:55880` stops only the recorded local runner bound
    to `127.0.0.1:55880`.
26. `down --runner 127.0.0.1` stops all recorded local runners bound to
    `127.0.0.1`.
27. `down --runner 55880 --runner 55881` stops only the selected local runners
    and preserves `previa-main` plus any remaining local runners and attached
    runner endpoints.
28. `down --runner 55880` fails clearly when the selector does not match any
    local runner entry in the runtime file.
29. `down --runner 55880` fails clearly if removing that runner would leave the
    stack with zero runner sources overall.
30. `down` does not attempt to terminate attached runner endpoints.
31. `restart` reads `PREVIA_HOME/run/up-state.json`, stops the detached local
    processes, starts a new detached stack with the same runner topology, and
    rewrites the runtime file with new PIDs.
32. `restart` fails clearly when no detached runtime file exists.
33. `up --detach` fails clearly when `PREVIA_HOME/run/up-state.json` already
    exists.
34. Any file generated by `previactl` is written under `PREVIA_HOME`.

## Rollback and Recovery

- Automatic rollback is out of scope for v1.
- If `up` fails before detached runtime state is written, the command must
  terminate already spawned child processes before exiting.
- If `down` encounters one or more missing local PIDs, it must continue
  processing the remaining recorded local processes and then remove the runtime
  file.
- If `down --runner <selector>` stops some requested local runners and then fails
  before rewriting the runtime file, the operator must reconcile the runtime
  file manually before the next `status`, `down`, or `restart`.
- If `restart` fails after stopping the previous detached stack but before the
  new detached stack is fully ready, the operator must rerun `previactl up` or
  `previactl restart` manually.

## Security and Known Risks

- The manifest is trusted as the release source of truth for `manifest show`.
- No checksum verification is available in v1 because the current manifest does
  not expose checksum fields.
- No signature verification is available in v1.
- Adding checksums and signed release verification is mandatory hardening work
  for the future installer/update workflows.

## Implementation Notes

- The future crate will be named `previactl`.
- It should remain separate from HTTP transport concerns and reuse dedicated
  modules for manifest fetching, runtime state persistence, process spawning,
  endpoint validation, and teardown behavior.
- The CLI must target the existing `previa-main` and `previa-runner` contracts
  without requiring changes to those binaries for v1.
