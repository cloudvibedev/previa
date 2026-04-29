# Previa — API Pipeline Testing Studio

**Previa** é uma ferramenta visual para criar, executar e monitorar testes de API organizados em **pipelines sequenciais**. Ela permite importar uma especificação OpenAPI, montar fluxos de requisições encadeadas e executá-los como testes de integração ou testes de carga — tudo direto no browser ou via um backend remoto.

---

## O que é uma Pipeline?

Uma pipeline é uma sequência ordenada de **steps** (passos), onde cada step representa uma requisição HTTP. Os steps são executados na ordem definida, e cada step pode **referenciar dados retornados por steps anteriores** usando variáveis de template.

### Estrutura de uma Pipeline

```yaml
name: User CRUD Flow
description: Fluxo completo de criação, leitura, atualização e exclusão de um usuário.
steps:
  - id: create_user
    name: Create User
    method: POST
    url: "{{specs.users-api.url.hml}}/users"
    headers:
      Content-Type: application/json
    body:
      name: "{{helpers.name}}"
      email: "{{helpers.email}}"

  - id: get_user
    name: Get User
    method: GET
    url: "{{specs.users-api.url.hml}}/users/{{steps.create_user.id}}"

  - id: delete_user
    name: Delete User
    method: DELETE
    url: "{{specs.users-api.url.hml}}/users/{{steps.create_user.id}}"
```

### Variáveis de Template

Dentro de URLs, headers e body, você pode usar templates `{{...}}`:

| Sintaxe | Descrição | Exemplo |
|---|---|---|
| `{{specs.<slug>.url.<env>}}` | URL do ambiente de uma spec OpenAPI | `{{specs.users-api.url.hml}}` |
| `{{steps.<id>.<campo>}}` | Acessa o body da resposta de um step anterior | `{{steps.create_user.id}}` |
| `{{helpers.<tipo>}}` | Gera dados fake automaticamente (Faker.js) | `{{helpers.email}}`, `{{helpers.uuid}}` |

Isso permite encadear fluxos complexos: criar um recurso → usar o ID retornado para buscá-lo → atualizá-lo → deletá-lo.

### Assertions (Validações)

Cada step pode ter um array `asserts` para validar a resposta:

```yaml
asserts:
  - field: status
    operator: equals
    expected: "201"
  - field: body.email
    operator: contains
    expected: "@"
  - field: body.id
    operator: exists
```

**Operadores disponíveis:** `equals`, `not_equals`, `contains`, `exists`, `not_exists`, `gt`, `lt`

**Campos suportados:**
- `status` — código HTTP da resposta
- `body.<path>` — acesso a campos do body via dot-notation (ex: `body.user.name`)
- `header.<name>` — valor de um header de resposta

Se alguma assertion falhar, o step é marcado como `error`.

### Múltiplos Ambientes

Os ambientes são definidos nos servers de cada spec OpenAPI do projeto. Use `{{specs.<slug>.url.<env>}}` para apontar para o ambiente desejado:

```yaml
# HML
url: "{{specs.users-api.url.hml}}/users"

# PRD
url: "{{specs.users-api.url.prd}}/users"
```

---

## Funcionalidades Principais

### 1. Projetos
- Cada projeto contém uma OpenAPI Spec e múltiplas pipelines
- Suporte a importação/exportação de projetos completos
- Dados persistidos em `localStorage`

### 2. OpenAPI Spec
- Importe uma spec OpenAPI (JSON ou YAML) para extrair rotas, parâmetros e schemas
- As rotas da spec alimentam o criador de pipelines com autocomplete de operações, headers e body

### 3. Integration Test
- Executa a pipeline step a step, mostrando o resultado de cada requisição em tempo real
- Exibe request enviado, response recebido, duração e resultado das assertions
- Histórico de execuções salvo em IndexedDB

### 4. Load Test
- Executa a pipeline inteira repetidamente com concorrência configurável
- Configurações: total de requests, concorrência simultânea e ramp-up gradual
- Métricas em tempo real: RPS, latência média, P95, P99
- Gráficos de latência e throughput ao longo do tempo
- Histórico de runs salvo em IndexedDB

### 5. Dashboard
- Visualização agregada dos resultados de testes por pipeline
- Mini-gráficos de tendência para integration e load tests

### 6. Execução Remota (Backend)
- Opcionalmente, configure uma URL de backend nas configurações do projeto (⚙️ no header)
- Quando configurado, os testes são enviados para o servidor externo e os resultados chegam em tempo real via **HTTP Streaming (SSE)**
- O contrato OpenAPI do backend está documentado em `docs/test-execution-api.yaml`
- Sem backend configurado, tudo roda localmente no browser via Fetch API

---

## Arquitetura

```
src/
├── components/          # Componentes React (UI)
├── lib/
│   ├── pipeline-executor.ts    # Executor local de pipelines (browser)
│   ├── load-test-executor.ts   # Executor local de load tests
│   ├── remote-executor.ts      # Executor remoto via SSE streaming
│   ├── template-helpers.ts     # Resolução de variáveis e Faker.js
│   ├── openapi-parser.ts       # Parser de specs OpenAPI
│   ├── execution-store.ts      # IndexedDB para histórico de integration tests
│   ├── load-test-store.ts      # IndexedDB para histórico de load tests
│   └── storage.ts              # localStorage para projetos e configurações
├── pages/               # Páginas da aplicação
├── types/               # Tipos TypeScript
└── docs/
    └── test-execution-api.yaml  # Contrato OpenAPI do backend de execução
```

## Tecnologias

- **React 18** + **TypeScript**
- **Vite** (build tool)
- **Tailwind CSS** + **shadcn/ui** (design system)
- **Monaco Editor** (editor de código embutido)
- **Recharts** (gráficos de métricas)
- **IndexedDB** (persistência de histórico)
- **Faker.js** (geração de dados fake)

## Como rodar

```bash
npm install
npm run dev
```

Acesse `http://localhost:5173` no browser.

---

## Backend (Runner + Orchestrator)

Este repositório tem dois serviços Rust:

- `services/runner`: executa pipelines e faz streaming SSE dos eventos.
- `services/main`: orquestrador que distribui execução para 1+ runners e persiste histórico em SQLite.

Comportamento de execução remota no `main`:

- Se o cliente SSE desconectar, a execução continua em background até concluir.
- Para reconectar ao stream de uma execução em andamento (ou obter stream finito de execução já concluída), use `GET /api/v1/projects/{projectId}/executions/{executionId}`.
- Para interromper manualmente uma execução ativa, use `POST /api/v1/executions/{executionId}/cancel`.
- A execução entra no histórico imediatamente com status `running` e é atualizada para `success`, `error` ou `cancelled` ao finalizar.

### Subindo dois runners (5000 e 5001) + main

```bash
# terminal 1
cd services/runner
RUST_LOG=debug PORT=5000 cargo run
```

```bash
# terminal 2
cd services/runner
RUST_LOG=debug PORT=5001 cargo run
```

```bash
# terminal 3
cd services/main
RUST_LOG=debug RUNNER_ENDPOINTS="http://localhost:5000,http://localhost:5001" cargo run
```

Com isso:

- Runner 1: `http://localhost:5000`
- Runner 2: `http://localhost:5001`
- Main (orchestrator): `http://localhost:3100` (porta default)

### Variáveis de ambiente

#### `services/runner`

| Variável | Default | Descrição |
|---|---|---|
| `ADDRESS` | `0.0.0.0` | Endereço de bind HTTP do runner |
| `PORT` | `3000` | Porta HTTP do runner |
| `RUST_LOG` | *(vazio)* | Filtro de logs do `tracing` (ex: `info`, `debug`, `previa_runner=debug`) |

#### `services/main`

| Variável | Default | Descrição |
|---|---|---|
| `RUNNER_ENDPOINTS` | *(vazio)* | Lista de runners separados por vírgula. Ex: `http://localhost:5000,http://localhost:5001` |
| `ORCHESTRATOR_DATABASE_URL` | `sqlite://orchestrator.db` | URL SQLite usada pelo orquestrador |
| `RUNNER_RPS_PER_NODE` | `1000` | Capacidade estimada por runner para cálculo de distribuição no load test |
| `ADDRESS` | `0.0.0.0` | Endereço de bind HTTP do orquestrador |
| `PORT` | `3100` | Porta HTTP do orquestrador |
| `RUST_LOG` | *(vazio)* | Filtro de logs do `tracing` (ex: `info`, `debug`, `previa_main=debug`) |

### Endpoints úteis

- Runner: `/health`, `/info`, `/openapi.json`, `/api/v1/tests/e2e`, `/api/v1/tests/load`
- Main: `/health`, `/info`, `/openapi.json`, `/api/v1/projects`, `/api/v1/projects/{projectId}/tests/e2e`, `/api/v1/projects/{projectId}/tests/load`, `/api/v1/projects/{projectId}/executions/{executionId}`, `/api/v1/executions/{executionId}/cancel`

---

## Delay e Retry por Step

No motor de pipeline do runner, cada step suporta:

- `delay` (opcional, ms): executado antes de cada tentativa. Máximo `300000`.
- `retry` (opcional): tentativas extras além da primeira. Máximo `10`.

Regras de retry:

- Retry acontece em falha de assertion.
- Retry acontece em erro de conexão/timeout da requisição.
- Erro HTTP por si só (ex: `404`, `500`) não faz retry, exceto se houver assertion falhando.
- Em sucesso, interrompe imediatamente novas tentativas.

Eventos/resultado incluem:

- `attempt`: tentativa atual
- `maxAttempts`: total de tentativas possíveis (`retry + 1`)
