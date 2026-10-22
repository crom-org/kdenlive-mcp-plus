# Roadmap

[Índice](README.md) · [Visão](01-visao.md) · [Ferramentas](04-ferramentas.md) · [Decisões](decisoes.md)

Quatro fases. Cada uma só começa depois que a anterior atingiu seu critério de saída.

## Fase 0 — base do motor e do servidor (esta execução, concluída)

**Entregue nesta execução:** motor de render sobre o libmlt via FFI, render headless isolado em processo separado, servidor MCP mínimo com `server_info` e `render_project`, testes automatizados e toda a documentação do projeto.

**Critério de saída:** os testes descritos em [docs/06-testes.md](06-testes.md) passam, e os critérios de sucesso listados em [docs/01-visao.md](01-visao.md) foram atingidos. Ambos se confirmam nesta execução — a Fase 0 está concluída.

**Dependências:** nenhuma; é o ponto de partida.

**Riscos que ficaram:** ver [docs/09-riscos-e-bugs.md](09-riscos-e-bugs.md) (avisos conhecidos do MLT/FFmpeg, teste manual de abertura no Kdenlive ainda pendente por falta de tela neste ambiente).

## Fase 1 — base operacional (próxima)

Implementar e testar, uma a uma (cada ferramenta só entra em `tools/list` quando testada — princípio 3 de [docs/01-visao.md](01-visao.md)):

`project_new`, `project_open` (lendo o `.kdenlive` nativo — diferente do projeto de referência, que só lia um `.kdenlive-cli.json` próprio), `project_save`, `project_get_info`, `bin_import_clip`, `bin_list_clips`, `bin_get_clip_details`, `bin_remove_clip`, `timeline_add_track`, `timeline_add_clip`, `timeline_list`, `timeline_get_info`, `export_xml`, `export_render`, `export_list_presets`, `render_cancel`, `render_progress`, `project_validate`, `preview_frame` e `project_import_kdenlive`.

Duas observações importantes sobre essa lista:

- **`export_render` é a futura `render_project`.** A ferramenta `render_project`, construída nesta Fase 0, será renomeada para `export_render` quando a Fase 1 começar a implementar as demais ferramentas de `export_*`, para manter a nomenclatura `grupo_ação` consistente (ver [docs/04-ferramentas.md](04-ferramentas.md), seção de convenções de nomes). Essa renomeação ainda não aconteceu — é só o plano. Quando acontecer, deve ser registrada como uma nova decisão em [docs/decisoes.md](decisoes.md).
- **`render_cancel` de verdade encerra o processo filho.** Diferente do `render_queue_stop` do projeto de referência, que só marcava um estado interno sem realmente interromper nada (ver o bug correspondente em [docs/09-riscos-e-bugs.md](09-riscos-e-bugs.md)).

**Critério de saída:** um agente consegue criar um projeto com dois clipes, renderizar, e o `.kdenlive` resultante abre no Kdenlive atual — teste manual, documentado em [docs/06-testes.md](06-testes.md).

**Risco principal:** ainda não se sabe o esquema XML interno completo que o Kdenlive exige para bins e efeitos — só o MLT XML "puro" (sem metadados específicos do Kdenlive) foi validado na Fase 0, via render direto. O quanto o Kdenlive aceita de volta nesse formato simplificado é algo a confirmar nesta fase.

**Dependência:** nenhuma ferramenta desta fase depende de código fora do motor libmlt já construído na Fase 0.

## Fase 2 — edição completa

As demais ferramentas herdadas do projeto de referência, mais as ferramentas novas marcadas como fase 2 — títulos, keyframes, legendas, áudio (níveis, normalização), transformação de clipe, miniaturas, montagem em lote por intervalos e importação de pasta inteira para a bin. A lista completa, com origem e status de cada uma, está em [docs/04-ferramentas.md](04-ferramentas.md).

**Critério de saída:** toda a tabela de [docs/04-ferramentas.md](04-ferramentas.md) com origem "herdada" ou fase ≤ 2 está com status `testada`.

**Risco:** volume grande de ferramentas de uma vez. Recomenda-se dividir em sub-lotes por categoria (bin, timeline, filtros, transições, guias/marcadores, fila de render, etc.), em vez de tentar entregar tudo de uma só implementação.

## Fase 3 — integração com o Kdenlive aberto

Painéis extras e controle do Kdenlive já aberto pelo usuário, para permitir revisão em tempo real do que o agente está fazendo. Depende de uma escolha entre fazer fork do Kdenlive em C++/Qt ou controlar o Kdenlive por D-Bus (como faz o projeto de referência `mcp-kdenlive`, que usa um fork com patch próprio mais D-Bus — ver [docs/03-concorrentes.md](03-concorrentes.md)).

A decisão D-006 (ver [docs/decisoes.md](decisoes.md)) já definiu que **não há fork do Kdenlive** nas fases anteriores: o motor headless precisa estar estável primeiro, porque o custo de manter um fork é alto.

**Critério de saída:** ainda não definido. Esta fase só começa depois que as Fases 0 a 2 estiverem concluídas e o motor estiver estável em uso real.

---
[Voltar ao índice](README.md)
