# Frontend Persistence Model

This frontend uses three different persistence buckets, each with a clear role.

## Remote source of truth

When an orchestrator/backend is configured, these entities must come from the API
and stay in memory on the client:

- projects
- pipelines
- specs
- execution history
- load test history
- queue state
- live execution state

Remote mode must not use browser persistence as a fallback for these entities.

## Browser-local by design

These values are intentionally local to the current browser/profile:

- orchestrator contexts and selected context
- UI preferences such as theme, palette and glass level
- step view and auto-scroll preferences
- editor/chat layout preferences
- temporary execution reconnect hints stored in `sessionStorage`
- AI chat conversations stored in IndexedDB

These values are product-level client preferences, not shared project state.

## Offline/local mode

When no backend is configured, the app can still persist local project state in
IndexedDB:

- projects and pipelines in `project-db.ts`
- execution history in `execution-store.ts`
- load history in `load-test-store.ts`

This mode is the only place where local persistence should act as the source of
truth for project data.

## Module map

- `src/lib/ui-preferences.ts`
  Browser-local UI preferences only
- `src/lib/project-db.ts`
  Offline/local project persistence only
- `src/lib/execution-store.ts`
  Offline/local E2E history only
- `src/lib/load-test-store.ts`
  Offline/local load history only
- `src/lib/chat-db.ts`
  Browser-local chat persistence only

## Guardrail

If a feature represents shared project/runtime data and a backend is available,
persist it remotely and keep only in-memory client state on the frontend.
