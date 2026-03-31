# Pipeline Authoring

This guide shows how to write useful Previa pipelines that are easy to execute, debug, and reuse.

## Pipeline Shape

Each pipeline is a direct object with:

- `id` optional
- `name` required
- `description` optional
- `steps` required

Each step typically includes:

- `id`
- `name`
- `method`
- `url`
- `headers`
- `body` optional
- `asserts`

## Minimal Example

```yaml
id: users-smoke
name: Users Smoke
description: Basic smoke coverage for the users API.
steps:
  - id: get_users
    name: List users
    method: GET
    url: https://api.example.com/users
    headers: {}
    asserts:
      - field: status
        operator: equals
        expected: "200"
```

## Supported Templates

Previa supports these template roots:

- `{{steps.<step_id>.<field>}}`
- `{{specs.<slug>.url.<name>}}`
- `{{helpers.uuid}}`
- `{{helpers.email}}`
- `{{helpers.name}}`
- `{{helpers.username}}`
- `{{helpers.number 10 99}}`
- `{{helpers.date}}`
- `{{helpers.boolean}}`
- `{{helpers.cpf}}`

Do not invent unsupported roots such as `{{env.*}}`, `{{project.*}}`, or `{{run.*}}`.

## Helpers and Template Variables

Template expressions are resolved inside string values across the pipeline, including:

- `url`
- `headers`
- `body`
- assertion `expected`

Use each root for a different job:

- `{{specs.<slug>.url.<name>}}` for reusable base URLs
- `{{steps.<step_id>.<field>}}` for values returned by earlier steps
- `{{helpers.*}}` for generated inline test data

### Use Specs for Base URLs

Use spec URLs when the same pipeline should run in more than one environment.

Example spec:

```json
{
  "slug": "users",
  "urls": [
    { "name": "local", "url": "http://127.0.0.1:3000" },
    { "name": "hml", "url": "https://hml.example.com" }
  ]
}
```

Example pipeline usage:

```yaml
steps:
  - id: list_users
    name: List users
    method: GET
    url: "{{specs.users.url.hml}}/users"
    headers: {}
    asserts:
      - field: status
        operator: equals
        expected: "200"
```

Prefer `{{specs.<slug>.url.<name>}}` over repeating full URLs in every step.
Legacy `{{url.<slug>.<name>}}` expressions are normalized internally, but new pipelines should use `specs.*`.

### Use Step Outputs as Variables

`steps.*` references values from the response body of a previous step. This is the main way to carry runtime data forward through a pipeline.

```yaml
steps:
  - id: create_user
    name: Create user
    method: POST
    url: "{{specs.users.url.hml}}/users"
    headers:
      content-type: application/json
    body:
      name: "{{helpers.name}}"
      email: "{{helpers.email}}"
    asserts:
      - field: status
        operator: equals
        expected: "201"

  - id: get_user
    name: Get created user
    method: GET
    url: "{{specs.users.url.hml}}/users/{{steps.create_user.id}}"
    headers: {}
    asserts:
      - field: body.email
        operator: equals
        expected: "{{steps.create_user.email}}"
```

If a value must appear again later, prefer reading it from `steps.*` instead of calling the same helper twice.

### Use Helpers for Inline Test Data

Helpers generate values at render time:

- `{{helpers.uuid}}`
- `{{helpers.email}}`
- `{{helpers.name}}`
- `{{helpers.username}}`
- `{{helpers.number 10 99}}`
- `{{helpers.date}}`
- `{{helpers.boolean}}`
- `{{helpers.cpf}}`

Common usage:

```yaml
headers:
  x-request-id: "{{helpers.uuid}}"
body:
  name: "{{helpers.name}}"
  email: "{{helpers.email}}"
```

Important notes:

- helpers generate inline values, not named variables
- each helper expression is resolved independently
- if the same generated value is needed later, capture it through a step response and reference it with `steps.*`
- helper substitutions are string-based, so write assertions and payloads with that behavior in mind

## Chaining Steps

Use outputs from earlier steps to build later requests:

```yaml
steps:
  - id: create_user
    name: Create user
    method: POST
    url: "{{specs.users.url.hml}}/users"
    headers:
      content-type: application/json
      x-request-id: "{{helpers.uuid}}"
    body:
      name: "{{helpers.name}}"
      email: "{{helpers.email}}"
    asserts:
      - field: status
        operator: equals
        expected: "201"

  - id: get_user
    name: Get user
    method: GET
    url: "{{specs.users.url.hml}}/users/{{steps.create_user.id}}"
    headers: {}
    asserts:
      - field: status
        operator: equals
        expected: "200"
      - field: body.email
        operator: equals
        expected: "{{steps.create_user.email}}"
```

## Assertions

Supported operators:

- `equals`
- `not_equals`
- `contains`
- `exists`
- `not_exists`
- `gt`
- `lt`

Common assertion fields:

- `status`
- `body.<field>`
- response-derived values exposed by a previous step

## Important Behavior Notes

- `step.url` must always resolve to an absolute URL
- `GET` and `HEAD` do not send request bodies
- `delay` is in milliseconds
- retries use `maxAttempts = retry + 1`
- assertion failures can trigger retry when configured

## Practical Authoring Tips

- keep step `id`s short and stable
- validate one important thing per assertion before adding deeper checks
- use `{{specs.<slug>.url.<name>}}` instead of repeating environment URLs
- include a cleanup step for mutation-heavy flows when possible
- start with a smoke path before building a full regression chain

## Common Mistakes

- using unsupported operators like `gte` or `lte`
- referencing `{{specs.<slug>.url.<name>}}` without creating the matching project spec
- depending on a field from a step that never produced it
- importing a file that is not a direct `Pipeline` object

## See Also

- [Spec-driven testing](./spec-driven-testing.md)
- [Examples cookbook](./examples-cookbook.md)
- [Pipeline import](./pipeline-import.md)
