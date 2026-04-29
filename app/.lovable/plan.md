

## Exibir data/hora de atualização na pipeline

### Problema
O tipo `Pipeline` não possui campo `updatedAt`. O `PipelineRow` no IndexedDB tem, mas é descartado na desserialização (`fromPipelineRow` retorna só o `pipelineJson`).

### Plano

1. **Adicionar `updatedAt` ao tipo `Pipeline`** (`src/types/pipeline.ts`)
   - Campo opcional `updatedAt?: string` na interface `Pipeline`

2. **Preservar `updatedAt` na desserialização** (`src/lib/project-db.ts`)
   - Em `fromPipelineRow`, injetar `row.updatedAt` no objeto Pipeline retornado
   - Em `toPipelineRows`, preservar `updatedAt` existente da pipeline se houver

3. **Exibir no `PipelineListItem`** (`src/components/PipelineListItem.tsx`)
   - Ao lado do número de steps, mostrar a data/hora formatada (ex: "12/03 14:30") em texto `text-[10px] text-muted-foreground`
   - Usar `toLocaleString` para formatar de forma compacta

### Arquivos modificados
- `src/types/pipeline.ts` — adicionar campo `updatedAt`
- `src/lib/project-db.ts` — preservar `updatedAt` na conversão
- `src/components/PipelineListItem.tsx` — exibir data/hora

