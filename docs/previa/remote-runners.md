# Remote Runners

Remote runners let one `previa-main` orchestrate execution against runner processes outside the local stack.

## Start a Runner

Example:

```bash
RUNNER_AUTH_KEY=shared-secret \
ADDRESS=0.0.0.0 \
PORT=55880 \
cargo run -p previa-runner
```

You can also run a downloaded `previa-runner` binary with the same environment variables.

## Attach It to a Local Stack

Use `--attach-runner` with the same shared key:

```bash
RUNNER_AUTH_KEY=shared-secret previa up -d --attach-runner 10.0.0.12:55880
```

Accepted attached runner formats:

- `55880` -> `http://127.0.0.1:55880`
- `10.0.0.12:55880` -> `http://10.0.0.12:55880`
- `10.0.0.12` -> `http://10.0.0.12:55880`

## Mixed Topologies

You can combine local and attached runners:

```bash
RUNNER_AUTH_KEY=shared-secret previa up -d --runners 1 --attach-runner 10.0.0.12:55880
```

In that case:

- local runners inherit the same `RUNNER_AUTH_KEY`
- `previa-main` sends that key to all runners in `RUNNER_ENDPOINTS`

## Important Rule

Attached runners always require `RUNNER_AUTH_KEY`.

If the key on `previa-main` does not match the key on the remote runner, the runner appears unhealthy and execution cannot start successfully.

## See Also

- [Main and runner authentication](./main-runner-auth.md)
- [Operations](./operations.md)
