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
