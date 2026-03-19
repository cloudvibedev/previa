# E2E Queues

Previa can execute multiple stored pipelines in sequence for the same project through an E2E queue.

## Create a Queue

```bash
curl -sS -D queue.headers \
  http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e/queue \
  -H 'content-type: application/json' \
  -d @- <<JSON
{
  "pipelineIds": [
    "$PIPELINE_ID",
    "another-pipeline-id"
  ],
  "selectedBaseUrlKey": "hml",
  "specs": []
}
JSON
```

The response is `202 Accepted` and includes:

- a queue snapshot body
- `x-queue-id`
- `Location: /api/v1/projects/<projectId>/tests/e2e/queue/<queueId>`

## Inspect the Active Queue

```bash
curl -sS http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e/queue
```

This returns the current active queue snapshot for the project.

## Follow a Queue

```bash
QUEUE_ID="<queue-id>"

curl -N http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e/queue/$QUEUE_ID
```

Behavior:

- while the queue is active, the endpoint returns SSE updates such as `queue:update`
- once the queue finishes, the same endpoint returns the final JSON snapshot

## Cancel a Queue

```bash
curl -X DELETE \
  http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e/queue/$QUEUE_ID
```

## Typical Use Cases

- execute a regression sequence in a fixed order
- run pre-check, mutation, verification, and cleanup as separate pipelines
- let an assistant orchestrate test batches through MCP

## See Also

- [MCP integration](./mcp.md)
- [Operations](./operations.md)
