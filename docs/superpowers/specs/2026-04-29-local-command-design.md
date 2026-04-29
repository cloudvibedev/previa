# Previa Local Command Design

## Goal

Add a repository-local CLI workflow that makes `previa --home ./.previa ...`
easy to discover and remember.

## User Interface

The CLI gains a top-level `local` command with subcommands that mirror common
runtime operations:

```bash
previa local up -d
previa local status
previa local open
previa local logs
previa local down
```

`local up` keeps the normal `up` semantics. It does not imply detached mode;
users still pass `-d` or `--detach` when they want a detached stack.

## Behavior

`previa local <command>` runs the matching existing command with the runtime
home set to `./.previa`, resolved relative to the current working directory.

Examples:

```bash
previa local up -d
```

is equivalent to:

```bash
previa --home ./.previa up -d
```

If the user explicitly passes global `--home`, the explicit value wins:

```bash
previa --home ./tmp-previa local status
```

uses `./tmp-previa`, not `./.previa`.

## Scope

This change is limited to the `previa` CLI and its documentation. It does not
change `previa-main`, `previa-runner`, `previa-engine`, storage layout,
runtime configuration, or API contracts.

## Testing

Tests should verify argument parsing and command dispatch behavior without
starting a real stack. The implementation should preserve existing command
arguments for `up`, `status`, `open`, `logs`, and `down`.
