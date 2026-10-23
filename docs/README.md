# Documentação do kdenlive-mcp-plus

O kdenlive-mcp-plus é um servidor MCP (Model Context Protocol) que permite a um agente de IA editar vídeo de forma totalmente headless, usando o libmlt — o mesmo motor de render do Kdenlive — e salvando no formato nativo do Kdenlive (`.kdenlive`, XML do MLT), para que o responsável pelo projeto possa abrir e ajustar o mesmo arquivo à mão.

## Como ler

Se você está chegando agora, leia nesta ordem:

1. [01-visao.md](01-visao.md) — o problema, o objetivo e os princípios do projeto.
2. [02-arquitetura.md](02-arquitetura.md) — como as peças se encaixam.
3. Depois, o que for relevante para a sua tarefa: [04-ferramentas.md](04-ferramentas.md) para saber o que existe e o que ainda não existe, [06-testes.md](06-testes.md) para rodar a bateria de testes, [07-ambiente.md](07-ambiente.md) para configurar o ambiente, e assim por diante.

## Estado atual

| Fase | Status |
|---|---|
| Fase 0 — base, motor e servidor mínimo | Concluída |
| Fase 1 — base operacional | Concluída |
| Fase 2 — edição completa | Concluída (inclusive `clip_reframe_vertical` e `audio_duck`) |
| Fase 3 — integração com o Kdenlive aberto | Concluída para o uso principal (formato nativo + autosave + recarregamento automático pelo D-Bus, D-040/D-042); só painéis próprios dentro do Kdenlive (fork) ficam pendentes do responsável pelo projeto |

Ver [05-roadmap.md](05-roadmap.md) para os critérios de saída de cada fase.

## Como começar

Para compilar e usar o projeto, veja o [README.md](../README.md) na raiz. Para configurar o ambiente de desenvolvimento (dependências do sistema, versões verificadas), veja [07-ambiente.md](07-ambiente.md).

## Todos os documentos

- [README.md](README.md) — este índice.
- [01-visao.md](01-visao.md) — problema, objetivo, quem usa, princípios, escopo e critérios de sucesso.
- [02-arquitetura.md](02-arquitetura.md) — componentes, fluxo de uma chamada, modelo de dados, decisões de design.
- [03-concorrentes.md](03-concorrentes.md) — análise de projetos semelhantes e posicionamento.
- [04-ferramentas.md](04-ferramentas.md) — tabela completa de ferramentas (herdadas e novas), com origem, fase e status.
- [05-roadmap.md](05-roadmap.md) — as fases do projeto, critérios de saída, dependências e riscos.
- [06-testes.md](06-testes.md) — estratégia de testes e comandos para rodar a bateria completa.
- [07-ambiente.md](07-ambiente.md) — requisitos, comandos de instalação e versões verificadas.
- [08-licencas.md](08-licencas.md) — licenças do projeto e de cada dependência.
- [09-riscos-e-bugs.md](09-riscos-e-bugs.md) — bugs do projeto original, duplicações evitadas e riscos técnicos.
- [10-glossario.md](10-glossario.md) — termos técnicos usados no projeto.
- [decisoes.md](decisoes.md) — todas as decisões de projeto, numeradas, com motivo e alternativas avaliadas.
