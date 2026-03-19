# Version Compatibility

This guide explains how version alignment works between `previa`, `previa-main`, and `previa-runner`.

## Default Behavior

By default, Previa keeps the local runtime aligned with the CLI version:

```text
previa == previa-main == previa-runner
```

That is the default path for:

- `previa up -d`
- `previa pull`
- `previa up -d --bin`

## Docker-Backed Runtime

When you run:

```bash
previa up -d
```

the CLI uses its own version as the default image tag for:

- `ghcr.io/cloudvibedev/main`
- `ghcr.io/cloudvibedev/runner`

You can override that explicitly:

```bash
previa up -d --version 1.0.0-alpha.4
```

## Binary-Backed Runtime

When you run:

```bash
previa up -d --bin
```

the CLI ensures that `previa-main` and `previa-runner` match the exact current CLI version.

If a binary is:

- missing, Previa downloads the matching version
- present but on a different version, Previa replaces it with the matching version
- already present on the same version, Previa reuses it

## `previa pull`

When you run:

```bash
previa pull
```

the CLI pulls the current CLI version tag by default.

You can still request another tag explicitly:

```bash
previa pull all --version 1.0.0-alpha.4
```

## Why This Matters

Keeping these versions aligned reduces runtime drift and avoids a class of bugs where:

- the CLI expects newer behavior than the runtime provides
- the runtime exposes older or different semantics than the CLI assumes
- a local `--bin` environment silently reuses stale binaries

## Recommendation

For the best experience, keep the default aligned path unless you are intentionally testing a specific compose-backed runtime version.

## See Also

- [Release and install](./release-install.md)
- [Runtime modes](./runtime-modes.md)
- [CLI commands](./cli-commands.md)
