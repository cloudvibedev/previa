# Agents

- Document and adjust each agent as provider workflows change.
- **Architecture**: keep transport concerns inside routes/, move reusable logic
  into dedicated modules, and align data contracts with openapi.yaml and models/
  structs.
- **Project structure**: routes/ handles HTTP transport and wiring, services/
  owns reusable business logic and integrations, models/ defines data contracts
  and DB-facing structs; keep modules small and focused.
- **Separation rule**: always split routes from services and models; do not mix
  request/response handling with business logic or data structs.
- **Persistence**: prefer SQLx query macros with bound parameters, reuse
  migrations/, and avoid inline schema drift.
- **Processes**: discuss changes via PROJECT.md conventions, open pull requests
  with review context, and keep agents.md current when workflows shift.
- **Release build & push**: after finishing any change run
  `cargo build --release`; if the release build succeeds, commit the changes and
  push to the remote.
- **Agent execution behavior**: `POST /v1/agents/{agent_id}` supports recursive
  tool execution until the model stops returning tool calls; in streaming mode,
  responses must keep the `{assistantMessage, providerPayload}` contract and
  stream only model output chunks.
