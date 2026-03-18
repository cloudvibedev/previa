# Issue: controle de concorrencia de execucoes

## Contexto

Hoje o Previa aceita execucoes simultaneas de testes E2E e load, tanto no orchestrator quanto nos runners. Isso permite paralelismo, mas nao existe um controle explicito de capacidade por runner, por projeto ou por ambiente.

Na pratica:

- multiplas execucoes podem ser disparadas ao mesmo tempo
- multiplos testes podem cair no mesmo runner
- nao existe fila para quando a capacidade estiver esgotada
- nao existe lock por projeto ou ambiente compartilhado

Isso aumenta o risco de flakiness, disputa por recursos e sobrecarga local, principalmente em pipelines E2E que usam o mesmo banco, as mesmas credenciais ou o mesmo ambiente externo.

## Problema

O modelo atual usa concorrencia livre. Ele funciona para throughput, mas nao protege contra:

- duas pipelines E2E alterando o mesmo estado ao mesmo tempo
- varias execucoes sendo roteadas para o mesmo runner sem limite
- degradacao de performance por excesso de execucoes simultaneas
- resultados inconsistentes quando testes compartilham fixtures ou dados

## Proposta

Implementar controle de concorrencia em tres niveis.

### 1. Limite por runner

Cada runner deve expor ou receber uma capacidade maxima de execucoes simultaneas.

Recomendacao inicial:

- E2E: `1` execucao simultanea por runner, ou um valor baixo e configuravel
- Load: limite configuravel por runner, separado de E2E

Objetivo:

- evitar sobrecarga local
- impedir concentracao descontrolada de execucoes em um unico node
- permitir que o orchestrator escolha apenas runners com slot disponivel

### 2. Lock por projeto ou ambiente

Pipelines que compartilham estado nao devem rodar ao mesmo tempo quando isso comprometer isolamento.

Recomendacao inicial:

- lock por `project_id` para execucoes E2E
- extensao futura para `environment_key` ou identificador equivalente

Objetivo:

- evitar conflito entre pipelines do mesmo projeto
- reduzir flakiness causada por estado compartilhado

### 3. Fila explicita

Quando nao houver capacidade disponivel, a execucao nao deve competir imediatamente por recurso.

Ela deve entrar em fila com estado `queued`, aguardando liberacao de slot.

Objetivo:

- previsibilidade operacional
- evitar falhas aleatorias por disputa
- permitir feedback claro no SSE, no historico e na UI

## Comportamento sugerido

### E2E

- aplicar lock por `project_id`
- usar apenas runners com slot livre
- se nao houver slot disponivel, enfileirar a execucao

### Load

- distribuir carga apenas entre runners com capacidade disponivel
- aplicar limite maximo configuravel por runner
- opcionalmente bloquear cargas concorrentes grandes no mesmo conjunto de runners

## Desenho tecnico sugerido

Adicionar um scheduler no orchestrator para controlar reservas e fila.

Estruturas sugeridas:

- `runner_slots`
- `project_locks`
- `queued_executions`

Fluxo sugerido:

1. request chega ao orchestrator
2. orchestrator tenta reservar lock e capacidade
3. se houver recursos, inicia a execucao
4. se nao houver recursos, marca como `queued`
5. ao finalizar ou cancelar, libera lock e slots
6. scheduler promove a proxima execucao elegivel da fila

## Regras iniciais recomendadas

- E2E:
  - `1` execucao por `project_id`
  - `1` execucao por runner por padrao
- Load:
  - limite configuravel por runner
  - sem lock por projeto por padrao

## Estado e observabilidade

O estado da execucao deve refletir a fase real do agendamento:

- `queued`
- `running`
- `completed`
- `error`
- `cancelled`

Pontos de visibilidade desejados:

- SSE com evento inicial indicando `queued` ou `running`
- historico persistido com timestamps de fila e inicio real
- endpoints de status/listagem mostrando ocupacao por runner

## Fases de implementacao

### Fase 1

- lock por `project_id` para E2E
- limite simples por runner
- rejeitar ou enfileirar quando nao houver capacidade

### Fase 2

- fila real com promocao automatica
- persistencia de estado `queued`
- visibilidade no historico e SSE

### Fase 3

- locks por ambiente
- politicas separadas para E2E e load
- configuracao dinamica de capacidade por runner

## Resultado esperado

Com esse modelo, o Previa continua permitindo execucoes simultaneas, mas com isolamento e capacidade controlados. Isso reduz instabilidade, melhora previsibilidade e prepara o produto para ambientes com mais de um runner e mais de um projeto ativo ao mesmo tempo.
