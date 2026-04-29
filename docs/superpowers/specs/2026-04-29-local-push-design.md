# Previa Local Push Design

## Goal

Add a project-local push workflow that copies a local Previa project to a remote
`previa-main`.

## Command

```bash
previa local push --project my_app --to https://previa.example.com
previa local push --project my_app --to https://previa.example.com --overwrite
```

## Behavior

The command reads the project-local context from `./.previa`, resolves the local
project by ID or exact name, exports it through the local API, and imports it
into the remote API.

If no matching remote project exists, the remote project is created. If a
matching remote project exists, the command fails unless `--overwrite` is
provided.

With `--overwrite`, the remote project is deleted and the local export snapshot
is imported. This is replace behavior, not merge behavior.

Remote matching checks project ID first, then exact project name. If project
name matching is ambiguous, the user must pass `--remote-project-id`.

History is excluded by default. `--include-history` includes E2E and load
history in the export/import payload.

## Scope

This change is implemented in the CLI using existing project export, delete,
and import APIs. It does not add a new server endpoint.
