# Previa Link Pipeline Sync Design

## Objetivo

Adicionar `previa link` como fluxo local para trabalhar com pipelines versionadas
em um diretorio do repositorio da aplicacao.

O comando deve subir um Previa local em modo anonimo, criar uma stack temporaria
a partir dos arquivos do diretorio, manter sincronizacao bidirecional enquanto o
link estiver ativo e remover a stack temporaria quando o link for encerrado.

Exemplo principal:

```bash
previa link tests/e2e
```

Em um repositorio `my-app`, esse comando representa o link entre:

```text
my-app/tests/e2e
```

e uma stack temporaria local derivada desse caminho, por exemplo:

```text
my-app-tests-e2e
```

## Decisoes

- `previa link` funciona somente em modo project-local.
- `previa link` usa `./.previa` como `PREVIA_HOME`, equivalente ao fluxo de
  `previa local`.
- `previa link` nao aceita URL remota e nao sincroniza com `previa-main`
  remoto.
- `previa link` sobe o runtime local automaticamente se ele nao estiver
  rodando.
- `previa up` e `previa link` sobem o app somente em modo anonimo.
- O CLI nao deve oferecer modo protected para `previa up` nem para
  `previa link`.
- O diretorio local e a fonte duravel. A stack no banco local e cache
  temporario.
- O sync roda em background por padrao e devolve o terminal.
- Em conflito, a ultima alteracao vence.
- Remover um arquivo local remove a pipeline correspondente do banco.
- Remover uma pipeline no Previa remove o arquivo correspondente do diretorio.
- Encerrar o link remove a stack temporaria do banco depois de um flush final.
- Se o processo for morto sem cleanup, o proximo `previa up` ou `previa link`
  deve detectar e remover stacks temporarias orfas.

## Nao Objetivos

- Nao sincronizar com `previa-main` remoto.
- Nao transformar `previa link` em backup de projeto completo.
- Nao sincronizar specs, env groups, historico, tokens, usuarios ou runners.
- Nao manter a stack criada pelo link como dado permanente.
- Nao suportar modo protected no fluxo do CLI local.
- Nao resolver merges semanticos dentro de uma mesma pipeline. A v1 usa
  "ultima alteracao vence".

## Comandos

### Criar Link

```bash
previa link <DIR> [--stack <NAME>]
```

Comportamento:

1. Resolve `<DIR>` relativo ao diretorio atual.
2. Garante que `./.previa` sera usado como home local.
3. Limpa links temporarios orfaos antes de iniciar.
4. Sobe o runtime local, se necessario, em modo anonimo.
5. Escolhe um nome de stack temporaria.
6. Importa arquivos de pipeline existentes no diretorio.
7. Persiste metadata do link.
8. Inicia um processo de sync em background.
9. Imprime um resumo com stack, path, PID e caminho de logs.

### Status

```bash
previa link status
```

Mostra links ativos no `./.previa` do diretorio atual:

- `linkId`
- stack temporaria
- path linkado
- PID do sync
- estado do processo
- ultima sincronizacao
- contadores de imports, exports, updates e deletes

### Logs

```bash
previa link logs [--follow]
```

Le logs do processo de sync do projeto local atual. Com `--follow`, acompanha
novas linhas.

### Stop

```bash
previa link stop [--all]
```

Sem `--all`, para os links do projeto local atual. Com `--all`, para todos os
links registrados no `./.previa`.

Para cada link:

1. Solicita encerramento do processo de sync.
2. Executa flush final das pipelines do banco para o diretorio.
3. Remove a stack temporaria do banco.
4. Remove metadata de link e arquivos de runtime do sync.

## Nome Da Stack Temporaria

Quando `--stack` e informado, o valor e usado como nome base.

```bash
previa link tests/e2e --stack smoke
```

Nomes resultantes:

```text
smoke
smoke-2
smoke-3
```

Quando `--stack` nao e informado, o nome base vem do diretorio atual mais o path
linkado.

Exemplo:

```text
cwd:  /work/my-app
dir:  tests/e2e
base: my-app-tests-e2e
```

O nome deve passar por normalizacao compativel com `parse_stack_name`:

- letras e numeros ASCII
- `.`
- `_`
- `-`

Caracteres fora desse conjunto viram `-`. Separadores repetidos colapsam. O
nome final nao pode iniciar com separador; se necessario, prefixar `link-`.

Se o nome ja existir, o CLI escolhe o proximo sufixo `-N` disponivel. O link
nao deve reutilizar uma stack permanente existente.

## Autenticacao

`previa up` e `previa link` devem sempre iniciar `previa-main` em modo anonimo.

Mudancas esperadas:

- remover ou deprecar `--protected` em `previa up`
- remover ou deprecar `--root-username`
- remover ou deprecar `--root-password-stdin`
- tornar `--anonymous` desnecessario
- reescrever ou remover `PREVIA_AUTH_ANONYMOUS=false` de `main.env` quando o
  runtime for iniciado pelo CLI

O binario `previa-main` ainda pode continuar suportando protected mode via env
manual para quem iniciar o backend diretamente. A restricao desta spec e sobre
o CLI local.

## Arquivos Suportados

O link observa e escreve arquivos de pipeline com os mesmos sufixos do import
atual:

- `.previa`
- `.previa.json`
- `.previa.yaml`
- `.previa.yml`

Novas pipelines criadas no Previa devem ser exportadas como YAML por padrao:

```text
<pipeline-id-ou-slug>.previa.yaml
```

Arquivos existentes preservam seu formato original. Se uma pipeline veio de
`.previa.json`, updates futuros mantem JSON. Se veio de YAML, updates futuros
mantem YAML.

## Estado Persistido

O CLI deve persistir estado do link dentro do home local:

```text
.previa/stacks/default/links/<link-id>.json
.previa/stacks/default/links/<link-id>.log
.previa/stacks/default/links/<link-id>.pid
```

Payload sugerido:

```json
{
  "linkId": "link_...",
  "path": "/work/my-app/tests/e2e",
  "stackName": "my-app-tests-e2e",
  "projectId": "project_...",
  "temporary": true,
  "createdBy": "previa link",
  "createdAt": "2026-05-26T12:00:00Z",
  "lastSyncAt": "2026-05-26T12:00:05Z",
  "files": {
    "/work/my-app/tests/e2e/smoke.previa.yaml": {
      "pipelineId": "smoke",
      "format": "yaml",
      "checksum": "sha256:...",
      "mtimeMs": 1779796805000
    }
  }
}
```

O projeto temporario tambem deve ter uma marca no banco para permitir cleanup
seguro mesmo quando arquivos de metadata forem perdidos. A primeira versao pode
usar tags internas se nao houver coluna dedicada:

```text
__previa_link_temporary
__previa_link_id:<link-id>
```

Se tags internas forem inadequadas para a UI, usar uma coluna dedicada em
`projects` em uma migracao posterior. A regra de produto e que o cleanup nao
deve depender somente do nome da stack.

## Fluxo Inicial

1. `previa link tests/e2e` resolve o path absoluto.
2. O CLI cria `./.previa` se nao existir.
3. O CLI limpa links temporarios orfaos.
4. O CLI sobe `previa-main` e runners locais em modo anonimo.
5. O CLI lista projetos existentes para escolher um nome temporario disponivel.
6. O CLI le arquivos de pipeline no diretorio.
7. O CLI cria a stack temporaria com esses arquivos.
8. O CLI registra metadata do link.
9. O CLI inicia o sync em background.
10. O terminal retorna ao usuario.

Se o diretorio estiver vazio, `previa link` ainda cria uma stack temporaria
vazia e passa a observar novas pipelines criadas no Previa ou no filesystem.

## Sync Do Filesystem Para O Banco

Eventos observados:

- arquivo criado
- arquivo alterado
- arquivo removido
- arquivo renomeado

Regras:

- criar arquivo cria pipeline no projeto temporario
- alterar arquivo atualiza pipeline
- remover arquivo remove pipeline
- renomear arquivo preserva pipeline quando o `id` interno for o mesmo
- erro de parse nao apaga a pipeline existente; registra erro no log e mantem
  o ultimo estado valido
- alteracoes geradas pelo proprio sync sao ignoradas via checksum/origem

O parse deve reaproveitar a logica de importacao existente sempre que possivel.
Validacoes de template e conflito de IDs devem continuar passando pelo backend.

## Sync Do Banco Para O Filesystem

O processo de link detecta alteracoes feitas no Previa local por polling
periodico da API de pipelines do projeto temporario.

Regras:

- pipeline criada no Previa gera arquivo novo
- pipeline alterada no Previa atualiza arquivo existente
- pipeline removida no Previa remove arquivo local
- se nao houver arquivo associado, o nome usa `pipeline.id` quando presente,
  senao slug do nome da pipeline
- colisoes de caminho recebem sufixo numerico

Polling e a escolha para a primeira versao. Eventos/SSE podem substituir o
polling depois, mas nao fazem parte deste design.

## Conflitos

Quando a mesma pipeline muda no filesystem e no Previa antes do sync estabilizar,
vence a ultima alteracao conhecida.

O sync deve registrar no log:

- origem vencedora (`filesystem` ou `previa`)
- pipeline afetada
- timestamp local
- arquivo afetado

Nao ha arquivo `.conflict` na primeira versao.

## Cleanup E Orfaos

`previa link stop` deve remover a stack temporaria depois de flush final.

`previa down` deve:

1. parar links ativos do contexto project-local
2. executar cleanup das stacks temporarias
3. encerrar runtime

Se o usuario matar o processo do link, matar o terminal, derrubar o computador
ou usar um comando que nao consiga finalizar o cleanup, o proximo comando local
que sobe ou gerencia runtime deve limpar orfaos antes de continuar.

Comandos que devem acionar cleanup de orfaos:

- `previa up`
- `previa local up`
- `previa link`
- `previa link status`
- `previa down`
- `previa local down`

Um link e considerado orfao quando:

- metadata indica `temporary: true`
- o PID do sync nao existe mais
- o projeto temporario ainda existe no banco

Se o path local ainda existir, ele permanece intacto. O cleanup remove somente a
materializacao temporaria no banco e os arquivos de metadata do link.

## APIs Necessarias

A implementacao deve preferir APIs existentes, mas o sync bidirecional precisa
de operacoes idempotentes para pipelines individuais:

- listar pipelines de um projeto
- criar pipeline em um projeto existente
- atualizar pipeline por ID
- remover pipeline por ID
- remover projeto temporario por ID

Se algum endpoint atual nao suportar esse fluxo com seguranca, adicionar rotas
em `main/src/server/handlers/pipelines.rs` e mover logica reutilizavel para
`services/`, mantendo handlers como transporte.

## Arquitetura CLI

Modulos sugeridos:

- `previa/src/link.rs`: comando principal, bootstrap, spawn do sync e cleanup
- `previa/src/link_state.rs`: leitura/escrita de metadata de link
- `previa/src/link_sync.rs`: loop de sync bidirecional
- `previa/src/pipeline_import.rs`: reutilizar descoberta e parse de arquivos
- `previa/src/export.rs`: reutilizar serializacao e nomeacao de arquivos

O processo em background pode ser o proprio binario `previa` invocado em um
subcomando interno oculto:

```bash
previa link run --link-id <id>
```

Esse subcomando nao precisa aparecer na documentacao publica.

## Erros E Mensagens

Casos esperados:

- diretorio inexistente: falhar com mensagem clara
- path nao e diretorio: falhar com mensagem clara
- `--stack` invalido: mostrar regra de nomes suportados
- projeto permanente com nome base ja existe: escolher sufixo numerico, nao
  reutilizar
- erro de parse em arquivo: logar e manter ultimo estado valido
- runtime local protegido antigo: reescrever para anonymous durante bootstrap
- tentativa de usar URL remota: falhar, porque link e somente project-local

## Testes

CLI:

- parse de `previa link <DIR>`
- parse de `previa link <DIR> --stack smoke`
- parse de `previa link status`
- parse de `previa link logs --follow`
- parse de `previa link stop --all`
- `previa up` nao aceita modo protected

Nomeacao:

- deriva `my-app-tests-e2e` de cwd `my-app` e dir `tests/e2e`
- aplica sufixo `-2` quando nome existe
- normaliza caracteres invalidos

Estado:

- escreve e le metadata de link
- detecta PID morto
- remove metadata depois de stop

Sync:

- arquivo criado cria pipeline
- arquivo alterado atualiza pipeline
- arquivo removido remove pipeline
- pipeline criada no Previa cria arquivo
- pipeline alterada no Previa atualiza arquivo
- pipeline removida no Previa remove arquivo
- ultima alteracao vence em conflito
- erro de parse nao deleta pipeline existente

Cleanup:

- `link stop` remove stack temporaria
- `down` remove links ativos antes de encerrar
- `up` remove stack temporaria orfa antes de subir
- cleanup nao remove arquivos do diretorio linkado

Build:

- `cargo build --release`

## Documentacao

Atualizar:

- `docs/previa/cli-commands.md`
- `docs/previa/import-export.md`
- `docs/previa/project-repository-workflow.md`
- `docs/previa/security.md`
- `docs/previa/access-management.md`

Documentar que `previa link` e temporario, project-local, anonimo e usa o
diretorio como fonte duravel.

## Riscos

- Sync bidirecional pode sobrescrever trabalho se o usuario editar o mesmo
  pipeline nos dois lugares quase ao mesmo tempo. A mitigacao v1 e log claro e
  regra simples de ultima alteracao vence.
- Cleanup por metadata pode falhar se o arquivo for apagado manualmente. Por
  isso a stack temporaria tambem precisa de marca no banco.
- Remover protected mode do CLI pode quebrar usuarios que dependem de
  `previa up --protected`. A mitigacao e manter protected mode disponivel para
  quem inicia `previa-main` diretamente via env manual.
- Watchers de filesystem variam por plataforma. A implementacao deve combinar
  watcher com reconciliacao periodica para nao depender somente de eventos.

## Criterios De Aceite

- `previa link tests/e2e` cria `./.previa`, sobe runtime anonimo e inicia sync
  em background.
- Sem `--stack`, a stack temporaria usa nome derivado de cwd + path.
- Com `--stack`, a stack temporaria usa o nome informado com sufixo numerico se
  necessario.
- Alteracoes em arquivos e no Previa sincronizam nos dois sentidos.
- Delecoes sincronizam nos dois sentidos.
- `previa link stop` remove a stack temporaria e preserva arquivos locais.
- `previa down` limpa links e stacks temporarias.
- `previa up` limpa stacks temporarias orfas.
- `previa up` e `previa link` nao sobem em modo protected.
- O repo passa em `cargo build --release`.
