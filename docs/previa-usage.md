# `previa`: guia de uso

`previa` e o CLI local do Previa para subir, inspecionar, parar e abrir um context local via Docker Compose, com um `previa-main`, zero ou mais `previa-runner` locais e runners anexados por URL.

Este guia cobre o uso operacional da CLI como ela existe hoje no codigo.

## Visao geral

Comandos disponiveis:

```text
previa --home <path> <COMMAND>
previa up [OPTIONS] [SOURCE]
previa pull [main|runner|all] [--version <version>]
previa down [OPTIONS]
previa restart [OPTIONS]
previa status [OPTIONS]
previa list [OPTIONS]
previa ps [OPTIONS]
previa logs [OPTIONS]
previa open [OPTIONS]
previa version
previa --version
```

Ajuda rapida:

```bash
previa --help
previa --home ./minha-previa status
previa up --help
previa logs --help
```

## Conceitos

### Context

Um `context` e um ambiente local isolado gerenciado pelo `previa`.

Cada context tem:

- um nome, como `default`, `other` ou `staging-local`
- um `previa-main`
- zero ou mais runners locais
- zero ou mais runners anexados por URL
- logs proprios
- arquivos de configuracao proprios
- runtime state proprio
- banco SQLite proprio do `main`

Se voce nao informar `--context`, o valor padrao e `default`.

Exemplo:

```bash
previa up --context default
previa up --context other -p 6688 -P 56880:56889
```

### PREVIA_HOME e `--home`

O `previa` grava estado, configuracao e o compose gerado sob `PREVIA_HOME`.

Voce pode sobrescrever isso por execucao com `--home <path>`.

Precedencia:

1. `--home <path>`
2. `PREVIA_HOME`
3. `$HOME/.previa`

Se `PREVIA_HOME` nao estiver definido, o padrao e:

```text
$HOME/.previa
```

Layout por context:

```text
$PREVIA_HOME/
  bin/
    previa
  stacks/
    <context>/
      config/
        main.env
        runner.env
      data/
        main/
          orchestrator.db
      run/
        docker-compose.generated.yaml
        lock
        state.json
```

## Resumo rapido

Subir um context local padrao com 1 runner:

```bash
previa up
```

Usar um home local so nessa execucao:

```bash
previa --home ./minha-previa up --detach
previa --home ./minha-previa status
```

Subir em detached mode:

```bash
previa up --detach
```

Ver status:

```bash
previa status
```

Ver processos:

```bash
previa ps
```

Abrir logs:

```bash
previa logs --follow
```

Abrir a UI com esse context:

```bash
previa open
```

Parar o context:

```bash
previa down
```

Baixar imagens publicadas:

```bash
previa pull
previa pull main
previa pull runner --version 0.0.7
```

## Como o `up` resolve a configuracao

O comando `up` combina configuracao de quatro fontes, nesta ordem de precedencia:

1. flags da CLI
2. arquivo compose informado em `SOURCE`
3. arquivos `main.env` e `runner.env` do context
4. defaults internos

Defaults relevantes:

- `main.address = 0.0.0.0`
- `main.port = 5588`
- `runner.address = 127.0.0.1`
- `runner port range = 55880:55979`
- `runners = 1`

O `up` tambem injeta:

- `PREVIA_CONTEXT` no `previa-main`
- `RUNNER_ENDPOINTS` com os runners locais e anexados
- `ORCHESTRATOR_DATABASE_URL` apontando para o SQLite do context

## `previa up`

Uso:

```text
previa up [--context <context>] [SOURCE] [--main-address <addr>] [-p, --main-port <port>] [--runner-address <addr>] [-P, --runner-port-range <start:end>] [-r, --runners <N>] [-a, --attach-runner <selector> ...] [--dry-run] [-d, --detach] [--version <tag>]
```

### O que faz

- gera um `docker-compose.generated.yaml` por context
- sobe exatamente um `previa-main`
- pode subir runners locais
- pode anexar runners ja existentes por endpoint HTTP
- pode rodar em foreground ou detached mode via `docker compose`

### Exemplos

Subir o context padrao:

```bash
previa up
```

Subir 3 runners locais:

```bash
previa up -r 3
```

Subir outro context com portas customizadas:

```bash
previa up --context other -p 6688 -P 56880:56889 -r 2
```

Subir usando um compose:

```bash
previa up .
previa up ./ambientes/dev
previa up ./previa-compose.yaml
```

Subir com runners anexados:

```bash
previa up -a 55880 -a 10.0.0.12:55880
```

Dry run:

```bash
previa up --dry-run
```

Detached:

```bash
previa up --detach --version latest
```

### `SOURCE`

O argumento opcional `SOURCE` pode ser:

- `.`
- um diretorio
- um arquivo `previa-compose.yaml`
- um arquivo `previa-compose.yml`
- um arquivo `previa-compose.json`

Quando `SOURCE` e `.` ou um diretorio, a busca acontece nesta ordem:

1. `previa-compose.yaml`
2. `previa-compose.yml`
3. `previa-compose.json`

### Seletor de runner anexado

`--attach-runner` aceita:

- `55880` -> `http://127.0.0.1:55880`
- `10.0.0.12:55880` -> `http://10.0.0.12:55880`
- `10.0.0.12` -> `http://10.0.0.12:55880`

Voce precisa ter pelo menos uma fonte de runner:

- `--runners > 0`
- pelo menos um `--attach-runner`
- ou ambos

### Dry run

`--dry-run` valida a configuracao sem subir containers.

Ele:

- resolve compose
- valida enderecos e portas
- valida capacidade da faixa de runners
- valida disponibilidade de bind local
- imprime o plano efetivo

Saida tipica:

```text
context: default
main: 0.0.0.0:5588
local runners: 1 (55880-55979)
attached runners:
```

### Detached mode

Com `--detach`, o `previa`:

- gera `run/docker-compose.generated.yaml`
- executa `docker compose up -d`
- grava `run/state.json`

Mensagem tipica:

```text
context 'default' started in detached mode (main: 0.0.0.0:5588)
```

### Regras e validacoes importantes

- `--dry-run` nao pode ser combinado com `--detach`
- `main.port` precisa estar entre `1` e `65535`
- a faixa de runners precisa ter portas suficientes para `-r`
- o context nao pode ja estar em execucao
- `up` falha antes de subir qualquer processo se o context ja estiver rodando
- `up` falha cedo se algum bind local planejado ja estiver ocupado

### Prompt de conflito de porta

Quando a porta local do `main` ou a faixa local de runners estiver ocupada, o `up` pergunta se pode continuar usando um deslocamento de `+100` portas.

Comportamento:

- para `main`, sugere `-p <porta+100>`
- para runners, sugere `-P <inicio+100:fim+100>`
- apertar Enter equivale a `Y`
- responder `n` aborta o comando

## `previa pull`

Uso:

```text
previa pull [main|runner|all] [--version <version>]
```

### O que faz

- executa `docker pull` para imagens publicadas do Previa no GHCR
- aceita `main`, `runner` ou `all`
- quando omitido, o alvo padrao e `all`
- quando `--version` e omitido, usa `latest`

### Repositorios

- `main` -> `ghcr.io/cloudvibedev/main`
- `runner` -> `ghcr.io/cloudvibedev/runner`

### Exemplos

```bash
previa pull
previa pull main
previa pull runner --version 0.0.7
previa pull all --version latest
```

## `previa down`

Uso:

```text
previa down [--context <context>] [--all-contexts] [--runner <selector> ...]
```

### O que faz

- encerra um context detached
- ou encerra runners locais especificos desse context
- ou encerra todos os contexts detached

### Exemplos

Parar o context atual:

```bash
previa down
```

Parar outro context:

```bash
previa down --context other
```

Parar apenas um runner local:

```bash
previa down --runner 55880
```

Parar todos os contexts:

```bash
previa down --all-contexts
```

### Regras

- `--all-contexts` e `--runner` sao mutuamente exclusivos
- `--runner` so atua em runners locais gravados no runtime
- attached runners nunca sao encerrados pelo `previa`
- parar os ultimos runners locais falha se nao houver attached runners restantes

## `previa restart`

Uso:

```text
previa restart [--context <context>]
```

Reinicia um context detached reaproveitando a configuracao gravada no runtime:

- `main.address`
- `main.port`
- faixa de portas dos runners
- runners locais
- attached runners
- `source`, quando houver

Exemplo:

```bash
previa restart --context other
```

## `previa status`

Uso:

```text
previa status [--context <context>] [--main] [--runner <selector>] [--json]
```

### O que faz

Lê o runtime do context e calcula estado a partir de:

- liveness do PID
- `GET /health` em cada processo local

Um processo local so e considerado healthy quando `/health` retorna `200 OK`.

### Exemplos

Status geral:

```bash
previa status
```

Status so do main:

```bash
previa status --main
```

Status de um runner:

```bash
previa status --runner 55880
```

JSON:

```bash
previa status --json
```

### Estados

- `running`: todos os processos locais vivos e healthy
- `degraded`: runtime existe, mas algum processo caiu ou falhou no health
- `stopped`: runtime ausente

### Saida humana

Exemplo:

```text
default  running
main     running  12345  0.0.0.0:5588
runner   running  12346  127.0.0.1:55880
attached http://10.0.0.12:55880
```

### Saida JSON

Estrutura:

```json
{
  "name": "default",
  "state": "running",
  "runtime_file": "/home/user/.previa/stacks/default/run/state.json",
  "main": {
    "state": "running",
    "pid": 12345,
    "address": "0.0.0.0",
    "port": 5588,
    "health_url": "http://0.0.0.0:5588/health",
    "log_path": "/home/user/.previa/stacks/default/logs/main.log"
  },
  "runners": [
    {
      "state": "running",
      "pid": 12346,
      "address": "127.0.0.1",
      "port": 55880,
      "health_url": "http://127.0.0.1:55880/health",
      "log_path": "/home/user/.previa/stacks/default/logs/runners/55880.log"
    }
  ],
  "attached_runners": [
    "http://10.0.0.12:55880"
  ]
}
```

## `previa list`

Uso:

```text
previa list [--json]
```

Lista todos os contexts conhecidos sob `PREVIA_HOME/stacks`.

Exemplo:

```bash
previa list
previa list --json
```

Saida humana:

```text
default  running
other    stopped
```

Saida JSON:

```json
[
  {
    "name": "default",
    "state": "running",
    "runtime_file": "/home/user/.previa/stacks/default/run/state.json"
  }
]
```

## `previa ps`

Uso:

```text
previa ps [--context <context>] [--json]
```

Mostra os processos locais registrados no runtime do context.

Exemplo:

```bash
previa ps
previa ps --context other --json
```

Saida humana:

```text
main    running  12345  0.0.0.0:5588       http://0.0.0.0:5588/health       /home/user/.previa/stacks/default/logs/main.log
runner  running  12346  127.0.0.1:55880    http://127.0.0.1:55880/health    /home/user/.previa/stacks/default/logs/runners/55880.log
```

Saida JSON:

```json
[
  {
    "role": "main",
    "state": "running",
    "pid": 12345,
    "address": "0.0.0.0",
    "port": 5588,
    "health_url": "http://0.0.0.0:5588/health",
    "log_path": "/home/user/.previa/stacks/default/logs/main.log"
  },
  {
    "role": "runner",
    "state": "running",
    "pid": 12346,
    "address": "127.0.0.1",
    "port": 55880,
    "health_url": "http://127.0.0.1:55880/health",
    "log_path": "/home/user/.previa/stacks/default/logs/runners/55880.log"
  }
]
```

## `previa logs`

Uso:

```text
previa logs [--context <context>] [--main] [--runner <selector>] [--follow] [-t, --tail [<lines>]]
```

### O que faz

Le os logs do runtime detached.

Sem filtro, imprime:

- `main.log`
- logs de todos os runners locais em ordem de porta

### Exemplos

Logs completos:

```bash
previa logs
```

So do `main`:

```bash
previa logs --main
```

So de um runner:

```bash
previa logs --runner 55880
```

Seguir logs:

```bash
previa logs --follow
```

Ultimas 20 linhas:

```bash
previa logs --tail 20
```

Atalho com default de 10 linhas:

```bash
previa logs -t
```

Follow + tail:

```bash
previa logs --follow -t 50
```

### Regras

- `--main` e `--runner` sao mutuamente exclusivos
- `-t` sem valor usa `10`
- `-t 0` falha
- o comando depende de runtime detached existente

## `previa open`

Uso:

```text
previa open [--context <context>]
```

Abre o navegador padrao com:

```text
https://app.previa.dev?add_context=<url-do-main>
```

Exemplo:

```bash
previa open
previa open --context other
```

Se o `main` estiver gravado como `0.0.0.0` ou `::`, o `previa` normaliza para loopback antes de montar a URL.

Exemplo de URL final:

```text
https://app.previa.dev?add_context=http%3A%2F%2F127.0.0.1%3A5588
```

Voce pode sobrescrever o comando que abre o navegador definindo `PREVIA_OPEN_BROWSER`.

## `previa version`

Uso:

```bash
previa version
previa --version
```

Saida:

```text
<version>
```

O valor exibido e a versao do pacote `previa` compilado.

## Arquivos de ambiente por context

Quando voce usa `previa up` sem `--dry-run`, o CLI garante a existencia destes arquivos:

`main.env`:

```dotenv
ADDRESS=0.0.0.0
PORT=5588
ORCHESTRATOR_DATABASE_URL=sqlite:///.../orchestrator.db
RUNNER_ENDPOINTS=http://127.0.0.1:55880
RUST_LOG=info
```

`runner.env`:

```dotenv
ADDRESS=127.0.0.1
PORT=55880
RUST_LOG=info
```

Caminhos:

```text
$PREVIA_HOME/stacks/<context>/config/main.env
$PREVIA_HOME/stacks/<context>/config/runner.env
```

## Exemplo de compose

Exemplo valido de `previa-compose.yaml`:

```yaml
version: 1
main:
  address: 0.0.0.0
  port: 5588
  env:
    RUST_LOG: info
runners:
  local:
    address: 127.0.0.1
    count: 2
    port_range:
      start: 55880
      end: 55889
    env:
      RUST_LOG: info
  attach:
    - 10.0.0.12:55880
```

## Fluxos comuns

### Subir um ambiente local simples

```bash
previa up --detach
previa status
previa open
```

### Operar varios contexts

```bash
previa up --context default --detach
previa up --context other --detach -p 6688 -P 56880:56889
previa list
previa status --context other
```

### Encerrar tudo

```bash
previa down --all-contexts
```

## Erros comuns

`context '<name>' is already running`

- o context selecionado ja tem processos ativos registrados
- use `previa status --context <name>`
- ou finalize com `previa down --context <name>`

`no detached runtime exists for context '<name>'`

- o context nao foi iniciado com `--detach`
- ou ja foi encerrado

`runner selector '<value>' did not match any local runner`

- o seletor informado em `status`, `logs` ou `down --runner` nao bate com nenhum runner local do runtime

`requested local runner count exceeds the configured port range`

- a faixa `-P` nao comporta a quantidade `-r`

## Referencias

- [spec v1](./specs/previa-v1.md)
- [README da workspace](../README.md)
