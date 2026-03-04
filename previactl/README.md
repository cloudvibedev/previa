# `previactl`

CLI Linux para gerenciar binarios do Previa.

## Comandos iniciais

- `previactl install`: baixa a release mais recente e instala sem trocar a versao ativa atual.
- `previactl update`: compara versao atual com a release mais recente e pede confirmacao para atualizar.
- `previactl uninstall`: remove binarios gerenciados (`previa-main` e `previa-runner`).

## Layout de instalacao

- Versoes baixadas: `~/.local/share/previa/versions/<tag>`
- Link da versao ativa: `~/.local/share/previa/current`
- Links gerenciados: `~/.local/share/previa/bin/`
- Links de usuario: `~/.local/bin/previa-main` e `~/.local/bin/previa-runner`

## Variaveis de ambiente

- `PREVIA_REPO`: repo GitHub para buscar releases (padrao: `cloudvibedev/previa`)
- `GITHUB_TOKEN`: token opcional para evitar rate-limit/acessar repositorios privados

