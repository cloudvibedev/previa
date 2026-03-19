# Runtime Modes

Previa supports two main local runtime modes.

## Docker-Backed Mode

Use this for the general case:

```bash
previa up -d
```

This mode:

- uses published container images
- works well as the default operator path
- is the best fit when Docker is already available

## Binary-Backed Mode

Use this for local binary execution:

```bash
previa up -d --bin
```

This mode:

- starts local `previa-main` and `previa-runner` binaries instead of containers
- is useful for local development and debugging
- currently targets Linux for published runtime binaries

If a required runtime binary is missing, `previa` can fetch it from:

```text
https://downloads.previa.dev/latest.json
```

and install it under `PREVIA_HOME/bin`.

## Which One Should You Use?

Use Docker-backed mode when:

- you want the simplest operator experience
- you want the published stack layout
- you are not actively developing the runtime locally

Use `--bin` when:

- you are developing `previa-main` or `previa-runner`
- you want to avoid Docker
- you are on Linux and want local binary execution

## Notes

- `-d` is the short form of `--detach`
- `--version` applies to compose-backed runtimes, not `--bin`
- `--bin` resolves binaries from `PREVIA_HOME/bin` before workspace targets

## See Also

- [Minimal happy path](./minimal-happy-path.md)
- [Troubleshooting](./troubleshooting.md)
