# Ferramentas

Ver também: [visão](01-visao.md) · [arquitetura](02-arquitetura.md) · [roadmap](05-roadmap.md) · [riscos e bugs](09-riscos-e-bugs.md) · [decisões](decisoes.md)

Regra do projeto (princípio 3 em [docs/01-visao.md](01-visao.md)): uma ferramenta só aparece no `tools/list` do servidor MCP quando está **implementada e testada** (status `testada` nesta tabela). Toda ferramenta `planejada` existe só nesta documentação.

## Ferramentas desta execução (Fase 0)

| Nome | Categoria | Descrição | Origem | Fase | Status |
|---|---|---|---|---|---|
| `server_info` | Servidor | Sem parâmetros. Retorna nome e versão do servidor, versão do libmlt e a lista de presets de render disponíveis. | nova (Fase 0) | 0 | testada |
| `render_project` | Render | Renderiza um projeto MLT XML/`.kdenlive` para um arquivo final via libmlt, em processo isolado. Parâmetros: `project_path`, `output_path`, `preset` (opcional), `overwrite` (opcional). Será renomeada para `export_render` na Fase 1 (ver [decisões](decisoes.md) e [roadmap](05-roadmap.md)); essa renomeação ainda não aconteceu. | nova (Fase 0) | 0 | testada |

## Ferramentas herdadas (73, do `kdenlive-mcp-server` 0.1.3 — ver [concorrentes](03-concorrentes.md))

Lista de checklist de funcionalidade (decisão D-007); não é código copiado (decisão D-010). Todas com status `planejada` nesta fase.

### Projeto

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `project_new` | Cria uma nova sessão/projeto do Kdenlive. | 1 | planejada |
| `project_open` | Carrega um projeto existente na sessão. No original, só aceita `.kdenlive-cli.json`; no novo, deve abrir o `.kdenlive`/MLT XML nativo (ver "Problemas a corrigir" abaixo). | 1 | planejada |
| `project_save` | Persiste o estado atual do projeto num arquivo. | 1 | planejada |
| `project_get_info` | Retorna metadados do projeto: FPS, resolução, layout de trilhas, contagem de clipes na bin. | 1 | planejada |
| `project_list_profiles` | Enumera os perfis de vídeo de saída disponíveis (hd1080p30, 4k60, sd_pal etc.). | 2 | planejada |
| `project_get_profile` | Obtém o perfil de vídeo atual do projeto (resolução, FPS, proporção). | 2 | planejada |
| `project_set_profile` | Define ou atualiza o perfil de vídeo do projeto. | 2 | planejada |
| `project_get_render_profiles` | Alias de `project_list_profiles`; nome enganoso (perfil de projeto não é preset de render). **A remover** (ver "Problemas a corrigir"). | 2 | planejada |

### Bin

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `bin_import_clip` | Importa um arquivo de mídia para a bin do projeto. | 1 | planejada |
| `bin_list_clips` | Lista todos os itens da bin (ID, nome, tipo, duração). | 1 | planejada |
| `bin_get_clip_details` | Propriedades detalhadas de um clipe da bin (duração, proporção, info de mídia). | 1 | planejada |
| `bin_remove_clip` | Remove um clipe da bin pelo ID. | 1 | planejada |
| `bin_create_folder` | Cria uma pasta na bin. | 2 | planejada |
| `bin_move_clip` | Move um clipe para uma pasta (ou para a raiz). | 2 | planejada |
| `bin_rename` | Renomeia um clipe da bin. | 2 | planejada |

### Timeline

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `timeline_add_track` | Adiciona uma trilha de vídeo ou áudio à timeline. | 1 | planejada |
| `timeline_add_clip` | Posiciona um clipe da bin numa trilha, num tempo específico. | 1 | planejada |
| `timeline_list` | Lista as trilhas com contagem de clipes, IDs, estado de mudo/trava. Sobrepõe-se a `timeline_get_info`/`track_get_info` (ver "Problemas a corrigir"). | 1 | planejada |
| `timeline_get_info` | Informação completa: trilhas+clipes, duração, posição, zoom, guias, marcadores, perfil. | 1 | planejada |
| `timeline_remove_track` | Remove uma trilha e todos os seus clipes. | 2 | planejada |
| `timeline_move_clip` | Reposiciona um clipe na mesma trilha, para um novo tempo. | 2 | planejada |
| `timeline_trim_clip` | Ajusta as alças de entrada/saída de um clipe (não é o filtro de recorte). | 2 | planejada |
| `timeline_split_clip` | Corta um clipe em duas partes independentes, num ponto exato. | 2 | planejada |
| `timeline_remove_clip` | Remove um clipe da timeline. | 2 | planejada |
| `timeline_get_duration` | Duração total da timeline (maior tempo de término). | 2 | planejada |
| `timeline_get_position` | Posição atual do cursor de reprodução. | 2 | planejada |
| `timeline_seek` | Move o cursor para uma posição (segundos). | 2 | planejada |
| `timeline_get_zoom` | Nível de zoom atual da timeline. | 2 | planejada |
| `timeline_set_zoom` | Define o nível de zoom (0.1 a 10.0). | 2 | planejada |

### Filtros

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `filter_add` | Aplica um efeito de vídeo/áudio a um clipe da timeline. | 2 | planejada |
| `filter_remove` | Remove um efeito de um clipe. | 2 | planejada |
| `filter_set_param` | Atualiza um único parâmetro de um filtro ativo. | 2 | planejada |
| `filter_list` | Lista os filtros ativos de um clipe específico. | 2 | planejada |
| `filter_list_available` | Lista todos os filtros que o runtime do libmlt expõe. | 2 | planejada |

### Transições

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `transition_add` | Cria uma transição de mescla (crossfade, wipe etc.) entre duas camadas de trilha. | 2 | planejada |
| `transition_remove` | Remove uma transição pelo ID numérico. | 2 | planejada |
| `transition_set` | Altera um parâmetro de uma transição existente. | 2 | planejada |
| `transition_list` | Lista todas as transições da timeline. | 2 | planejada |

### Guias

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `guide_add` | Adiciona uma guia de navegação na timeline. Sobrepõe-se conceitualmente a `marker_*` (ver "Problemas a corrigir"). | 2 | planejada |
| `guide_remove` | Remove uma guia pelo ID. | 2 | planejada |
| `guide_list` | Lista todas as guias da timeline. | 2 | planejada |
| `guide_get` | Obtém uma guia específica pelo ID. | 2 | planejada |
| `guide_update` | Atualiza propriedades de uma guia. | 2 | planejada |

### Marcadores

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `marker_add` | Adiciona um marcador de timeline ou de clipe (conceito separado de guia, segundo o original). | 2 | planejada |
| `marker_list` | Lista todos os marcadores. | 2 | planejada |
| `marker_remove` | Remove um marcador pelo ID. | 2 | planejada |

### Exportação

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `export_xml` | Gera o XML do Kdenlive/MLT do projeto atual. | 1 | planejada |
| `export_render` | Renderiza o projeto para um arquivo via `melt`. No original, não aplica os presets de render; no novo, aplica (decisão D-005). Receberá o código hoje em `render_project` (ver nota acima e [decisões](decisoes.md)). | 1 | planejada |
| `export_list_presets` | Lista todos os presets de render disponíveis para a exportação final. | 1 | planejada |

### Sessão

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `session_undo` | Desfaz a mudança de estado mais recente (até 50 entradas de histórico). No novo, ganha um parâmetro `steps` e absorve `session_undo_step` (ver "Problemas a corrigir"). | 2 | planejada |
| `session_redo` | Refaz a última operação desfeita. Absorve `session_redo_step` do mesmo jeito. | 2 | planejada |
| `session_status` | Estado da sessão: projeto carregado, flag de modificado, profundidade do histórico. | 2 | planejada |
| `session_history` | Lista todas as entradas de histórico de undo/redo, com descrições. | 2 | planejada |
| `session_undo_step` | Desfaz N passos (granular). **A unificar em `session_undo(steps)`** (ver "Problemas a corrigir"). | 2 | planejada |
| `session_redo_step` | Refaz N passos (granular). **A unificar em `session_redo(steps)`**. | 2 | planejada |

### Clipe

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `clip_get_properties` | Propriedades do clipe na timeline: entrada/saída, duração, velocidade, opacidade, reversão. | 2 | planejada |
| `clip_set_speed` | Define a velocidade de reprodução de um clipe (1.0 normal, 2.0 dobro, 0.5 metade, negativo reverso). | 2 | planejada |
| `clip_reverse` | Inverte a direção de reprodução de um clipe (alterna). | 2 | planejada |
| `clip_set_opacity` | Define a opacidade de um clipe (0.0 transparente a 1.0 opaco). | 2 | planejada |
| `clip_set_color` | Define o rótulo de cor de um clipe da bin (hex ou nome). | 2 | planejada |
| `clip_add_note` | Adiciona ou atualiza uma nota de um clipe da bin. | 2 | planejada |

### Trilha

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `track_get_info` | Informações detalhadas de uma trilha, incluindo seus clipes. Sobrepõe-se a `timeline_list`/`timeline_get_info` (ver "Problemas a corrigir"). | 2 | planejada |
| `track_set_name` | Renomeia uma trilha. | 2 | planejada |
| `track_set_locked` | Trava/destrava uma trilha. | 2 | planejada |
| `track_set_hidden` | Oculta/mostra uma trilha (visibilidade de vídeo). | 2 | planejada |
| `track_set_muted` | Muta/desmuta uma trilha (áudio). | 2 | planejada |
| `track_move_up` | Move a trilha uma posição acima (troca com a anterior). | 2 | planejada |
| `track_move_down` | Move a trilha uma posição abaixo (troca com a próxima). | 2 | planejada |

### Fila de render

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `render_queue_add` | Adiciona um trabalho à fila de render. | 2 | planejada |
| `render_queue_list` | Lista os trabalhos da fila de render. | 2 | planejada |
| `render_queue_status` | Status da fila de render. | 2 | planejada |
| `render_queue_start` | Inicia o processamento da fila (renderiza em sequência via `melt`). | 2 | planejada |
| `render_queue_stop` | Para a fila de render. No original, só marca o estado como parado, sem encerrar o processo (ver [docs/09-riscos-e-bugs.md](09-riscos-e-bugs.md)); o novo `render_cancel` (abaixo) resolve isso de verdade. | 2 | planejada |

## Ferramentas novas (propostas)

Vindas da seção 6.2 do briefing original. Nota de proveniência: a página do Glama descreve, para a versão *atual* do repositório `Va1bhav512/kdenlive-mcp-server`, montagem em lote por intervalos e reenquadramento vertical — funções que não estavam na versão 0.1.3 analisada originalmente. **A verificar**: se essas duas funções já existem na versão atual do repositório de referência sob outro nome; não foi possível confirmar isso nesta execução. Elas continuam listadas aqui como novas, por cautela.

| Nome | Categoria | Descrição | Fase | Status |
|---|---|---|---|---|
| `title_add` | Títulos | Cria um clipe de título com texto, fonte, cor, posição e duração. | 2 | planejada |
| `title_update` | Títulos | Altera o texto ou o estilo de um título. | 2 | planejada |
| `keyframe_set` | Keyframes | Define o valor de um parâmetro de filtro ou transição num instante, com interpolação. | 2 | planejada |
| `keyframe_list` | Keyframes | Lista os keyframes de um parâmetro. | 2 | planejada |
| `keyframe_remove` | Keyframes | Remove um keyframe. | 2 | planejada |
| `project_import_kdenlive` | Projeto | Importa um `.kdenlive` nativo para o modelo interno do servidor. | 1 | planejada |
| `subtitle_import_srt` | Legendas | Importa um SRT e cria títulos sincronizados. | 2 | planejada |
| `subtitle_export_srt` | Legendas | Exporta as legendas da timeline em SRT. | 2 | planejada |
| `audio_set_levels` | Áudio | Define o nível em dB de um clipe ou trilha. | 2 | planejada |
| `audio_normalize` | Áudio | Normaliza o loudness de um clipe ou trilha. | 2 | planejada |
| `audio_duck` | Áudio | Reduz o volume de uma trilha enquanto há fala (ducking). | 3 (avaliar) | planejada |
| `clip_set_transform` | Transformação | Define posição, escala, rotação e recorte de um clipe. | 2 | planejada |
| `clip_reframe_vertical` | Transformação | Reenquadra vídeo horizontal para vertical. | 2 (avaliar) | planejada |
| `preview_frame` | Pré-visualização | Exporta um PNG de um instante do projeto. | 1 | planejada |
| `export_thumbnail` | Pré-visualização | Gera uma miniatura do vídeo renderizado. | 2 | planejada |
| `project_validate` | Validação | Confere mídia faltando, sobreposições de clipes, duração zero e perfil incompatível. | 1 | planejada |
| `timeline_assemble_ranges` | Lote | Monta uma sequência a partir de intervalos de um clipe, numa única chamada. | 2 | planejada |
| `bin_import_folder` | Lote | Importa uma pasta inteira de mídia para a bin. | 2 | planejada |
| `render_cancel` | Render | Cancela o render em andamento, encerrando de verdade o processo filho (diferente de `render_queue_stop`, ver acima). | 1 | planejada |
| `render_progress` | Render | Consulta o progresso do render em andamento. | 1 | planejada |

## Nomes

Todas as ferramentas (herdadas e novas) usam `snake_case`, no formato `grupo_acao`.

## Problemas a corrigir antes de portar (do briefing original, §6.1)

- **`session_undo`/`session_undo_step`** fazem a mesma coisa com granularidade diferente: unificar em `session_undo` com parâmetro `steps`. O mesmo vale para `session_redo`/`session_redo_step`.
- **`guide_*`/`marker_*`** têm conceitos sobrepostos: definir um modelo único e documentar a diferença entre os dois, se de fato houver uma.
- **`project_get_render_profiles`** é alias de `project_list_profiles`, com nome enganoso (perfil de projeto não é preset de render): remover.
- **`timeline_list`, `timeline_get_info` e `track_get_info`** se sobrepõem: consolidar.
- **`render_queue_stop`** só marca o estado da fila como parado; não encerra o processo de render. No novo projeto, `render_cancel` cancela de verdade, encerrando o processo filho.
- **`project_open`** aceita só `.kdenlive-cli.json` no original. No novo, deve abrir o `.kdenlive` nativo.
- **`export_render`** no original não aplica os presets de render (daí o bug de segmentation fault, ver [docs/09-riscos-e-bugs.md](09-riscos-e-bugs.md)). No novo, aplica (decisão D-005).
