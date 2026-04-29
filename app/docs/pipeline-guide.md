# Pipeline Guide

A **pipeline** is a sequence of HTTP steps executed in order, where each step can reference data from previous steps. Pipelines let you define complete API test flows — from creating a resource to verifying and cleaning it up — in a single declarative JSON or YAML file.

---

## Table of Contents

- [Structure](#structure)
- [URL Resolution](#url-resolution)
- [Steps](#steps)
- [Variable Interpolation](#variable-interpolation)
- [Assertions](#assertions)
- [Delay & Retry](#delay--retry)
- [OpenAPI Integration](#openapi-integration)
- [API Features](#api-features)
- [Examples](#examples)

---

## Structure

A pipeline has three top-level fields:

| Field         | Type                       | Required | Description                                      |
|---------------|----------------------------|----------|--------------------------------------------------|
| `name`        | `string`                   | ✅       | Pipeline display name                            |
| `description` | `string`                   | ✅       | Brief description of the pipeline's purpose      |
| `steps`       | `PipelineStep[]`           | ✅       | Ordered list of HTTP steps (min 1)               |

---

## URL Resolution

Each OpenAPI spec in the project has a **slug** (unique identifier, e.g., `auth-api` or `auth_api`) and a list of **servers** (environment → URL mappings).

Use `{{specs.<spec-slug>.url.<environment>}}` in step URLs to reference the correct server:

```json
{
  "url": "{{specs.auth-api.url.hml}}/users"
}
```

This resolves to the `hml` server URL configured in the `auth-api` spec (e.g., `http://localhost:3000/users`).

### Example Spec Configuration

| Spec Slug     | Environment | URL                              |
|---------------|-------------|----------------------------------|
| `auth-api`    | `hml`       | `http://localhost:3000`          |
| `auth-api`    | `prd`       | `https://auth.api.example.com`  |
| `payments-api`| `hml`       | `http://localhost:3001`          |
| `payments-api`| `prd`       | `https://payments.api.example.com` |

---

## Steps

Each step represents an HTTP request. Steps are executed **sequentially**, and each step can access the response data of all previous steps.

| Field         | Type                          | Required | Description                                    |
|---------------|-------------------------------|----------|------------------------------------------------|
| `id`          | `string`                      | ✅       | Unique identifier used for referencing          |
| `name`        | `string`                      | ✅       | Display name                                    |
| `description` | `string`                      | ✅       | What this step does                             |
| `method`      | `GET\|POST\|PUT\|PATCH\|DELETE` | ✅       | HTTP method                                     |
| `url`         | `string`                      | ✅       | Request URL (supports interpolation)            |
| `headers`     | `Record<string, string>`      | ✅       | Request headers                                 |
| `body`        | `object`                      | ❌       | Request body (for POST/PUT/PATCH)               |
| `operationId` | `string`                      | ❌       | Links step to an OpenAPI operation              |
| `asserts`     | `StepAssertion[]`             | ❌       | Response assertions                             |
| `delay`       | `number` (0–300000)           | ❌       | Delay in ms before executing step               |
| `retry`       | `number` (0–10)               | ❌       | Retry attempts on failure                       |

---

## Variable Interpolation

Use `{{...}}` syntax to inject dynamic values anywhere in URLs, headers, and body fields.

### Available Variables

| Pattern                                    | Description                                          | Example                                          |
|--------------------------------------------|------------------------------------------------------|--------------------------------------------------|
| `{{specs.<slug>.url.<env>}}`               | Server URL from a spec's environment                 | `{{specs.auth-api.url.hml}}/users`               |
| `{{steps.<id>.status}}`                    | HTTP status code from a previous step                | `{{steps.create_user.status}}`                   |
| `{{steps.<id>.body.<field>}}`              | Response body field from a previous step             | `{{steps.create_user.body.id}}`                  |
| `{{steps.<id>.headers.<name>}}`            | Response header from a previous step                 | `{{steps.login.headers.authorization}}`          |
| `{{steps.<id>.<field>}}`                   | Shorthand for body fields                            | `{{steps.create_user.id}}`                       |
| `{{helpers.name}}`                         | Random full name (Faker.js)                          | `"John Doe"`                                     |
| `{{helpers.email}}`                        | Random email address                                 | `"john@example.com"`                             |
| `{{helpers.username}}`                     | Random username                                      | `"cool_user42"`                                  |
| `{{helpers.uuid}}`                         | Random UUID                                          | `"a1b2c3d4-..."`                                 |
| `{{helpers.number}}`                       | Random number                                        | `42`                                             |
| `{{helpers.date}}`                         | Random date string                                   | `"2024-03-15"`                                   |
| `{{helpers.boolean}}`                      | Random boolean                                       | `true`                                           |
| `{{helpers.cpf}}`                          | Random Brazilian CPF                                 | `"123.456.789-09"`                               |
| `{{helpers.cnpj}}`                         | Random Brazilian CNPJ                                | `"12.345.678/0001-90"`                           |
| `{{helpers.phone}}`                        | Random phone number                                  | `"(11) 98765-4321"`                              |

### Interpolation in URLs

```json
{
  "url": "{{specs.users-api.url.hml}}/users/{{steps.create_user.body.id}}"
}
```

### Interpolation in Body

```json
{
  "body": {
    "name": "{{helpers.name}}",
    "manager_id": "{{steps.create_manager.body.id}}"
  }
}
```

### Interpolation in Headers

```json
{
  "headers": {
    "Authorization": "Bearer {{steps.login.body.token}}"
  }
}
```

---

## Assertions

Each step can define an `asserts` array to validate the response. If any assertion fails, the step is marked as **error**.

### Assertion Structure

| Field      | Type     | Required | Description                                       |
|------------|----------|----------|---------------------------------------------------|
| `field`    | `string` | ✅       | The field to check (`status`, `body.field`, `headers.name`) |
| `operator` | `string` | ✅       | Comparison operator                               |
| `expected` | `string` | ❌       | Expected value (supports interpolation)           |

### Operators

| Operator      | Description                          | Requires `expected` |
|---------------|--------------------------------------|---------------------|
| `equals`      | Exact match                          | ✅                  |
| `not_equals`  | Must not match                       | ✅                  |
| `contains`    | String contains substring            | ✅                  |
| `exists`      | Field is present and not null        | ❌                  |
| `not_exists`  | Field is absent or null              | ❌                  |
| `gt`          | Greater than (numeric)               | ✅                  |
| `lt`          | Less than (numeric)                  | ✅                  |

### Assertion Examples

```json
{
  "asserts": [
    { "field": "status", "operator": "equals", "expected": "201" },
    { "field": "body.id", "operator": "exists" },
    { "field": "body.email", "operator": "contains", "expected": "@" },
    { "field": "body.age", "operator": "gt", "expected": "18" }
  ]
}
```

---

## Delay & Retry

Steps can be configured with delays and automatic retries for handling async processing or transient failures.

### Delay

The `delay` field specifies milliseconds to wait before executing the step:

```json
{
  "id": "check_status",
  "name": "Check Processing Status",
  "delay": 5000,
  "method": "GET",
  "url": "{{specs.jobs-api.url.hml}}/jobs/{{steps.create_job.body.id}}/status"
}
```

**Range**: 0 to 300000 ms (5 minutes)

### Retry

The `retry` field defines additional attempts on failure:

```json
{
  "id": "create_webhook",
  "name": "Create Webhook",
  "retry": 3,
  "method": "POST",
  "url": "{{specs.webhooks-api.url.hml}}/hooks"
}
```

**Range**: 0 to 10 retries (total attempts = retry + 1)

### Combined Example

```json
{
  "id": "poll_completion",
  "name": "Poll for Completion",
  "description": "Poll until job is complete",
  "delay": 2000,
  "retry": 5,
  "method": "GET",
  "url": "{{specs.jobs-api.url.hml}}/jobs/{{steps.create_job.body.id}}/status",
  "asserts": [
    { "field": "body.status", "operator": "equals", "expected": "completed" }
  ]
}
```

---

## OpenAPI Integration

When an OpenAPI spec is loaded, pipelines gain additional capabilities:

- **Contract Validation**: Each step is validated against the spec identified by the `{{specs.<slug>.url...}}` in the URL. Mismatches appear as yellow warning markers.
- **Auto-fill from Routes**: Select an OpenAPI operation in the visual editor to auto-populate method, URL, headers, and body schema.
- **Response Field Autocomplete**: `{{steps.<id>.body.` triggers contextual autocomplete with actual fields from the spec's response schema.
- **operationId Linking**: Setting `operationId` on a step binds it to a specific OpenAPI operation.
- **Live Spec Sync**: The UI monitors spec files for changes and notifies when they diverge from the loaded version.

---

## API Features

### Execution Cancellation (v0.0.7)

Active E2E test executions can be cancelled:

```http
POST /api/v1/executions/{executionId}/cancel
```

**Response Codes**:
- `202` — Cancellation requested
- `400` — Invalid parameter
- `404` — Execution not found or already finished

### Execution Queue (v0.0.7)

Queue pipelines for sequential execution:

```http
POST /api/v1/tests/e2e/queue
```

Queue a single pipeline or multiple pipelines for sequential processing.

### Project Import/Export (v0.0.7)

Import projects with optional history preservation:

```http
POST /api/v1/projects/import?includeHistory=true
```

Export project data for backup or migration:

```http
GET /api/v1/projects/{projectId}/export
```

### Execution History

Query execution history with filtering:

```http
GET /api/v1/projects/{projectId}/tests/e2e?pipelineIndex=0&limit=10&order=desc
```

**Query Parameters**:
- `pipelineIndex` — Filter by specific pipeline
- `limit` — Max records (default 100, max 500)
- `offset` — Pagination offset
- `order` — Sort by update time: `asc` | `desc`

### Load Testing

Execute distributed load tests:

```http
POST /api/v1/tests/load
```

Configure runners, request counts, concurrency, and ramp-up time.

---

## Examples

### Basic CRUD Flow (JSON)

```json
{
  "name": "User CRUD Flow",
  "description": "Create, read, update, and delete a user.",
  "steps": [
    {
      "id": "create_user",
      "name": "Create User",
      "description": "Create a new user with random data.",
      "method": "POST",
      "url": "{{specs.users-api.url.hml}}/users",
      "headers": { "Content-Type": "application/json" },
      "body": {
        "name": "{{helpers.name}}",
        "email": "{{helpers.email}}"
      },
      "asserts": [
        { "field": "status", "operator": "equals", "expected": "201" },
        { "field": "body.id", "operator": "exists" }
      ]
    },
    {
      "id": "get_user",
      "name": "Get User",
      "description": "Retrieve the created user by ID.",
      "method": "GET",
      "url": "{{specs.users-api.url.hml}}/users/{{steps.create_user.body.id}}",
      "headers": { "Content-Type": "application/json" },
      "asserts": [
        { "field": "status", "operator": "equals", "expected": "200" }
      ]
    }
  ]
}
```

### Authentication Flow (YAML)

```yaml
name: Auth Flow
description: Login and access a protected resource.
steps:
  - id: login
    name: Login
    description: Authenticate with credentials.
    method: POST
    url: "{{specs.auth-api.url.hml}}/auth/login"
    headers:
      Content-Type: application/json
    body:
      email: admin@example.com
      password: secret123
    asserts:
      - field: status
        operator: equals
        expected: "200"
      - field: body.token
        operator: exists

  - id: get_profile
    name: Get Profile
    description: Fetch the authenticated user's profile.
    method: GET
    url: "{{specs.auth-api.url.hml}}/users/me"
    headers:
      Content-Type: application/json
      Authorization: "Bearer {{steps.login.body.token}}"
    asserts:
      - field: status
        operator: equals
        expected: "200"
```

### Multi-Step Pipeline with Retry & Delay

```json
{
  "name": "Async Job Processing",
  "description": "Submit job and poll until completion.",
  "steps": [
    {
      "id": "submit_job",
      "name": "Submit Background Job",
      "method": "POST",
      "url": "{{specs.jobs-api.url.hml}}/jobs",
      "headers": { "Content-Type": "application/json" },
      "body": {
        "type": "data_processing",
        "payload": { "user_id": "{{helpers.uuid}}" }
      },
      "asserts": [
        { "field": "status", "operator": "equals", "expected": "202" },
        { "field": "body.job_id", "operator": "exists" }
      ]
    },
    {
      "id": "check_status",
      "name": "Poll Job Status",
      "description": "Wait and retry until job completes",
      "delay": 3000,
      "retry": 10,
      "method": "GET",
      "url": "{{specs.jobs-api.url.hml}}/jobs/{{steps.submit_job.body.job_id}}/status",
      "asserts": [
        { "field": "body.status", "operator": "equals", "expected": "completed" }
      ]
    },
    {
      "id": "fetch_result",
      "name": "Fetch Results",
      "method": "GET",
      "url": "{{specs.jobs-api.url.hml}}/jobs/{{steps.submit_job.body.job_id}}/results",
      "asserts": [
        { "field": "status", "operator": "equals", "expected": "200" },
        { "field": "body.data", "operator": "exists" }
      ]
    }
  ]
}
```
