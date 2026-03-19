# FAQ

## When should I use Docker vs `--bin`?

Use `previa up -d` for the general case. Use `previa up -d --bin` when you want local binary execution, especially during runtime development.

## Does `--bin` download missing binaries automatically?

Yes. If required runtime binaries are missing, `previa` downloads the exact `previa-main` and `previa-runner` version that matches the current CLI version and installs them under `PREVIA_HOME/bin`.

## Why does `--attach-runner` require `RUNNER_AUTH_KEY`?

Because `previa-main` must know the shared key in advance to authenticate against attached runner endpoints. Without that key, attached runners are not considered safe or usable.

## Does Previa generate `RUNNER_AUTH_KEY` automatically?

Yes for local-only stacks. When no key is configured and only local runners are used, `previa up` generates a UUID v4 and persists it into the context env files.

## Where does Previa store local data?

Under `PREVIA_HOME`, defaulting to `$HOME/.previa`, unless overridden with `--home <path>`.

## How do I isolate everything inside one repository?

Use:

```bash
previa --home ./.previa up -d
```

## How do I connect the hosted IDE to my local stack?

Run:

```bash
previa open
```

This opens `https://ide.previa.dev?add_context=<main-url>`.

## How do I connect an AI assistant through MCP?

Enable MCP on `previa-main` and point the assistant to:

```text
http://localhost:5588/mcp
```

## Can I use remote runners?

Yes. Start a `previa-runner` elsewhere, set the same `RUNNER_AUTH_KEY`, and attach it with `--attach-runner`.

## Can I import and export projects?

Yes, through the `previa-main` API. Project bundle import/export is available even though there are no dedicated `previa import` / `previa export` CLI commands today.

## See Also

- [Runtime modes](./runtime-modes.md)
- [Main and runner authentication](./main-runner-auth.md)
- [MCP integration](./mcp.md)
