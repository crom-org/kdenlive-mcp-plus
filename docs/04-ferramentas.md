# Ferramentas

Ver também: [visão](01-visao.md) · [arquitetura](02-arquitetura.md) · [roadmap](05-roadmap.md) · [riscos e bugs](09-riscos-e-bugs.md) · [decisões](decisoes.md)

Regra do projeto (princípio 3 em [docs/01-visao.md](01-visao.md)): uma ferramenta só aparece no `tools/list` do servidor MCP quando está **implementada e testada** (status `testada` nesta tabela). Toda ferramenta `planejada` existe só nesta documentação.

**Estado atual: catálogo completo.** 93 ferramentas no `tools/list`, todas `testada`, cobertas por 103 testes automatizados (72 unitários, 24 de integração via protocolo MCP real, 3 de presets e 4 de render). As duas que tinham ficado `planejada` (`clip_reframe_vertical` e `audio_duck`) foram implementadas e verificadas medindo o resultado (D-045 e D-047 em [decisões](decisoes.md)).

## Ferramentas desta execução, fora dos catálogos herdado/novo (Fase 0)

| Nome | Categoria | Descrição | Origem | Fase | Status |
|---|---|---|---|---|---|
| `server_info` | Servidor | Sem parâmetros. Retorna nome e versão do servidor, versão do libmlt e a lista de presets de render disponíveis. | nova (Fase 0) | 0 | testada |

## Ferramentas herdadas (do `kdenlive-mcp-server` 0.1.3 — ver [concorrentes](03-concorrentes.md))

Lista de checklist de funcionalidade (decisão D-007); não é código copiado (decisão D-010). Das 73 ferramentas originais, 70 foram portadas como ferramentas distintas (algumas absorvendo mais de um nome original — ver "Sessão", abaixo) e uma foi removida de propósito (`project_get_render_profiles`, ver "Problemas a corrigir").

### Projeto

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `project_new` | Cria uma nova sessão/projeto do Kdenlive, já com uma trilha de vídeo e uma de áudio (`default_tracks=false` para vazio; o Kdenlive 26 fecha ao abrir projeto sem trilhas, D-044). | 1 | testada |
| `project_open` | Carrega um projeto existente na sessão. Abre o `.kdenlive`/MLT XML nativo diretamente (diferente do original, que só aceitava `.kdenlive-cli.json`). | 1 | testada |
| `project_save` | Persiste o estado atual do projeto num arquivo. | 1 | testada |
| `project_get_info` | Retorna metadados do projeto: perfil, contagem de trilhas, contagem de clipes na bin, duração. | 1 | testada |
| `project_list_profiles` | Enumera os perfis de vídeo de saída disponíveis — nomes reais do libmlt (ex. `atsc_1080p_25`, `uhd_2160p_30`), verificados neste ambiente, não nomes "amigáveis" inventados. | 2 | testada |
| `project_get_profile` | Obtém o perfil de vídeo atual do projeto (resolução, FPS, proporção). | 2 | testada |
| `project_set_profile` | Troca o perfil de vídeo do projeto. Só altera os metadados do `<profile>`; não recalcula nem reposiciona nada já existente na timeline. | 2 | testada |

**`project_get_render_profiles`** foi removida do catálogo (não implementada de propósito): era um alias enganoso de `project_list_profiles` — ver "Problemas a corrigir".

### Bin

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `bin_import_clip` | Importa um arquivo de mídia para a bin do projeto. Sonda a duração com `ffprobe` (best-effort; nunca chama o libmlt para isso). | 1 | testada |
| `bin_list_clips` | Lista todos os itens da bin (ID, nome, caminho, duração, cor, nota). | 1 | testada |
| `bin_get_clip_details` | Propriedades detalhadas de um clipe da bin, incluindo se o arquivo de mídia ainda existe no disco. | 1 | testada |
| `bin_remove_clip` | Remove um clipe da bin pelo ID. Se o produtor ainda estiver referenciado numa trilha, só a entrada da bin é removida — o produtor continua existindo. | 1 | testada |
| `bin_create_folder` | Cria uma pasta na bin, via a propriedade `kdenlive:folder.<pai>.<id>` — confirmada no `dev-docs/fileformat.md` do próprio repositório do Kdenlive, não inventada. | 2 | testada |
| `bin_move_clip` | Move um clipe para uma pasta (ou para a raiz), via `kdenlive:folderid` — também confirmada na mesma fonte. | 2 | testada |
| `bin_rename` | Renomeia um clipe da bin (`kdenlive:clipname`). | 2 | testada |

### Timeline

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `timeline_add_track` | Adiciona uma trilha de vídeo ou áudio à timeline. | 1 | testada |
| `timeline_add_clip` | Posiciona um clipe da bin numa trilha, num tempo específico. Sustentado por uma inserção genérica por posição (nova na Fase 2): posições exatamente no início de um clipe existente empurram esse clipe e os seguintes para depois (usando a posição implícita do MLT); posições dentro de um espaço vazio o dividem; posições estritamente dentro de um clipe já ocupado continuam erro (sem "ripple" parcial). | 1 | testada |
| `timeline_list` | Lista as trilhas com contagem de clipes, IDs, estado de mudo/trava. | 1 | testada |
| `timeline_get_info` | Informação completa: trilhas+clipes, duração, perfil. | 1 | testada |
| `timeline_remove_track` | Remove uma trilha e todos os seus clipes. Os produtores não são apagados. | 1 | testada |
| `timeline_move_clip` | Reposiciona um clipe na mesma trilha, para um novo tempo (usa a mesma inserção genérica de `timeline_add_clip`). | 2 | testada |
| `timeline_trim_clip` | Ajusta as alças de entrada/saída de um clipe (não é um filtro de recorte de imagem). Como as posições do MLT são implícitas, encurtar/alongar desloca naturalmente o que vem depois na mesma trilha. | 2 | testada |
| `timeline_split_clip` | Corta um clipe em duas partes independentes, num quadro exato do produtor (não uma posição absoluta da timeline). | 2 | testada |
| `timeline_remove_clip` | Remove um clipe da timeline (o espaço vira vazio, sem deslocar o resto). | 2 | testada |
| `timeline_get_duration` | Duração total da timeline (maior fim de trilha). | 2 | testada |
| `timeline_get_position` | Posição atual do cursor de reprodução — conceito só de sessão/UI; não há player nesta fase (headless), então isso não afeta o render. | 2 | testada |
| `timeline_seek` | Move o cursor para uma posição (quadros) — mesmo conceito de sessão/UI. | 2 | testada |
| `timeline_get_zoom` | Nível de zoom atual da timeline — mesmo conceito de sessão/UI. | 2 | testada |
| `timeline_set_zoom` | Define o nível de zoom (0.1 a 10.0) — mesmo conceito de sessão/UI. | 2 | testada |

### Filtros

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `filter_add` | Aplica um efeito de vídeo/áudio a um clipe da timeline. O filtro fica anexado à entrada do clipe (`<entry>`), não ao produtor compartilhado — confirmado empiricamente que isso dá um efeito por corte, independente de outros cortes do mesmo produtor na timeline. | 2 | testada |
| `filter_remove` | Remove um efeito de um clipe. | 2 | testada |
| `filter_set_param` | Atualiza um único parâmetro de um filtro ativo. | 2 | testada |
| `filter_list` | Lista os filtros ativos de um clipe específico. | 2 | testada |
| `filter_list_available` | Lista todos os filtros que o runtime do libmlt expõe, chamando `melt -query filters` (um subprocesso comum — nunca uma chamada FFI direta deste servidor). | 2 | testada |

### Transições

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `transition_add` | Cria uma transição (ex. `luma`, para crossfade/wipe) entre duas trilhas, como `<transition>` filho do tractor principal, com `a_track`/`b_track` como índices numéricos das trilhas — sintaxe confirmada com `melt`. | 2 | testada |
| `transition_remove` | Remove uma transição pelo ID. | 2 | testada |
| `transition_set` | Altera um parâmetro de uma transição existente. | 2 | testada |
| `transition_list` | Lista todas as transições da timeline. | 2 | testada |

### Guias

Convenção própria deste projeto: guias/marcadores ficam como JSON numa propriedade do tractor principal. **Não confirmado** como igual ao que o Kdenlive real grava internamente — a pesquisa sobre o formato de arquivo não encontrou a representação XML interna de guias/marcadores (ver [riscos e bugs](09-riscos-e-bugs.md)).

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `guide_add` | Adiciona uma guia de navegação na timeline. | 2 | testada |
| `guide_remove` | Remove uma guia pelo ID. | 2 | testada |
| `guide_list` | Lista todas as guias da timeline. | 2 | testada |
| `guide_get` | Obtém uma guia específica pelo ID. | 2 | testada |
| `guide_update` | Atualiza posição e/ou comentário de uma guia. | 2 | testada |

### Marcadores

Mantidos como um conceito separado de guia, seguindo o catálogo original — ver "Problemas a corrigir" sobre a sobreposição conceitual entre os dois.

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `marker_add` | Adiciona um marcador de timeline (guia azul, categoria 2 do Kdenlive) ou de clipe (se `clip_ref` for dado: `"<trilha>:<índice>"` ou id de clipe da bin; vai em `kdenlive:markers` do clipe, `position` em quadros da mídia). | 2 | testada |
| `marker_list` | Lista todos os marcadores. | 2 | testada |
| `marker_remove` | Remove um marcador pelo ID. | 2 | testada |

### Exportação

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `export_xml` | Gera o XML do MLT/Kdenlive do projeto atual (em arquivo, ou como texto na própria resposta). | 1 | testada |
| `export_render` | Renderiza o projeto (o aberto na sessão, ou um caminho explícito) para um arquivo final via libmlt, em processo isolado, aplicando sempre codec/bitrate explícitos do preset (nunca o nome do preset — decisão D-005). É a renomeação de `render_project` da Fase 0/1 (ver [decisões](decisoes.md)). Suporta `wait=false` para rodar em segundo plano, acompanhado por `render_progress`/`render_cancel`. | 1 | testada Com `start_frame`/`end_frame` renderiza só esse trecho (só com `wait=true`, D-051). |
| `export_list_presets` | Lista os nomes dos presets de render disponíveis. | 1 | testada |

### Sessão

As duas ferramentas granulares do catálogo original (`session_undo_step`/`session_redo_step`) foram unificadas em `session_undo`/`session_redo` com um parâmetro `steps` (padrão 1) — exatamente a correção já prevista em "Problemas a corrigir". Elas não existem mais como ferramentas separadas.

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `session_undo` | Desfaz a(s) mudança(s) mais recente(s) (parâmetro `steps`, até 50 entradas de histórico por snapshot). | 2 | testada |
| `session_redo` | Refaz a(s) última(s) operação(ões) desfeita(s) (parâmetro `steps`). Uma edição nova depois de um undo limpa a pilha de redo. | 2 | testada |
| `session_status` | Estado da sessão: projeto carregado, flag de modificado, profundidade do histórico, passos disponíveis para refazer. | 2 | testada |
| `session_history` | Lista todas as entradas de histórico de undo, com descrições, da mais antiga para a mais recente. | 2 | testada |

### Clipe

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `clip_get_properties` | Propriedades do clipe na timeline: produtor, entrada/saída, velocidade, se está invertido, filtros anexados. | 2 | testada |
| `clip_set_speed` | Define a velocidade de reprodução de um clipe, envolvendo o produtor original num produtor `timewarp` do libmlt (confirmado em `melt -query producer`), sem alterar o produtor original nem outras instâncias dele. Verificado com render real: 100 quadros a 2x → 50 quadros, confirmado por `ffprobe`. Funciona com mídia real (produtores `avformat`); não garantido com produtores sintéticos `color`/`tone` (o `resource` do `timewarp` é resolvido como caminho de arquivo). | 2 | testada |
| `clip_reverse` | Inverte a direção de reprodução de um clipe (alterna o sinal da velocidade atual) — mesmo mecanismo de `clip_set_speed`. | 2 | testada |
| `clip_set_opacity` | Define a opacidade de um clipe, via o filtro `brightness`/`alpha` do libmlt, anexado à entrada do clipe. | 2 | testada |
| `clip_set_color` | Define a cor (tag `kdenlive:tags`, `#RRGGBB`) de um clipe **da bin** (não da timeline). Formato nativo do Kdenlive (D-041). | 2 | testada |
| `clip_add_note` | Define a descrição (`kdenlive:description`) de um clipe **da bin**. Formato nativo do Kdenlive (D-041). | 2 | testada |

### Trilha

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `track_get_info` | Informações detalhadas de uma trilha: tipo, nome, travada/oculta/muda, contagem e comprimento de clipes. | 2 | testada |
| `track_set_name` | Renomeia uma trilha. | 2 | testada |
| `track_set_locked` | Trava/destrava uma trilha (conceito só de edição; não afeta o render). | 2 | testada |
| `track_set_hidden` | Oculta/mostra o vídeo de uma trilha, independentemente do estado de mudo. | 2 | testada |
| `track_set_muted` | Muta/desmuta o áudio de uma trilha, independentemente do estado de visibilidade. | 2 | testada |
| `track_move_up` | Move a trilha uma posição acima (troca com a anterior). Erro se já for a primeira. | 2 | testada |
| `track_move_down` | Move a trilha uma posição abaixo (troca com a próxima). Erro se já for a última. | 2 | testada |

### Fila de render

| Nome | Descrição | Fase | Status |
|---|---|---|---|
| `render_queue_add` | Adiciona um trabalho à fila de render (não inicia por conta própria). | 2 | testada |
| `render_queue_list` | Lista os trabalhos da fila de render, com status e percentual. | 2 | testada |
| `render_queue_status` | Status geral da fila: processando ou não, contagem por status. | 2 | testada |
| `render_queue_start` | Inicia o processamento da fila em sequência, um render por vez (mesma garantia de `export_render`). | 2 | testada |
| `render_queue_stop` | Para a fila: encerra de verdade o processo filho em andamento (`SIGTERM`) e cancela os pendentes sem iniciá-los. Corrige o bug do original, em que `render_queue_stop` só marcava um estado interno sem parar nada (ver [riscos e bugs](09-riscos-e-bugs.md)). | 2 | testada |

## Ferramentas novas (propostas)

Vindas da seção 6.2 do briefing original. Nota de proveniência mantida do levantamento original: a página do Glama descreve, para a versão *atual* do repositório `Va1bhav512/kdenlive-mcp-server`, montagem em lote por intervalos e reenquadramento vertical — funções que não estavam na versão 0.1.3 analisada originalmente. **A verificar**: se essas duas funções já existem na versão atual do repositório de referência sob outro nome; não foi possível confirmar isso nesta execução.

| Nome | Categoria | Descrição | Fase | Status |
|---|---|---|---|---|
| `title_add` | Títulos | Cria um clipe de título (texto, fonte, cor, duração), via o produtor `pango` padrão do libmlt — **não** uma tentativa de reproduzir o formato XML interno `kdenlivetitle` do Kdenlive, cujo esquema exato (tags do `xmldata`) não foi confirmado numa pesquisa anterior. Posicionar o título na tela é feito depois, com `clip_set_transform`. | 2 | testada |
| `title_update` | Títulos | Altera texto e/ou estilo de um título já existente. | 2 | testada |
| `keyframe_set` | Keyframes | Define o valor de um parâmetro de filtro num quadro, usando a sintaxe de propriedade animada nativa do MLT (`"quadro=valor;quadro=valor"`), confirmada renderizando neste ambiente. | 2 | testada |
| `keyframe_list` | Keyframes | Lista os keyframes de um parâmetro, em ordem de quadro. | 2 | testada |
| `keyframe_remove` | Keyframes | Remove o keyframe de um quadro específico. | 2 | testada |
| `project_import_kdenlive` | Projeto | Importa um `.kdenlive` nativo para a sessão (hoje, um alias de `project_open`, com nome dedicado para quando a intenção é especificamente um arquivo Kdenlive). | 1 | testada |
| `subtitle_import_srt` | Legendas | Importa um `.srt` (formato público, não uma convenção do Kdenlive) e cria clipes de título sincronizados (via `title_add`) numa trilha, com o timing convertido pelo fps do projeto. | 2 | testada |
| `subtitle_export_srt` | Legendas | Exporta os clipes de título (texto) de uma trilha como um arquivo `.srt`. | 2 | testada |
| `audio_set_levels` | Áudio | Define o nível em dB de um clipe, via o filtro `volume`/`level` padrão do libmlt (confirmado via `melt -query`). | 2 | testada |
| `audio_normalize` | Áudio | Normaliza o loudness de um clipe para uma amplitude alvo, via `volume`/`normalize`. | 2 | testada |
| `audio_duck` | Áudio | Abaixa um clipe (ou todos os de uma trilha), em geral a música, enquanto outra trilha, em geral a voz, fala. Usa keyframes no `level` de um filtro `volume` próprio (sem plugins LADSPA; reaplicar troca só esse filtro). A fala é o que há de clipe na trilha da voz, ou, com `detect_speech`, só as partes não silenciosas dele (medidas com o `ffmpeg silencedetect`). Parâmetros: `duck_db`, `attack_frames`, `release_frames`. Verificada **medindo o áudio renderizado** (D-047). | 3 | testada |
| `clip_set_transform` | Transformação | Define posição, escala e rotação de um clipe, via o filtro `qtblend` padrão do libmlt (`rect`+`rotation`). Repassa os valores exatos do agente, sem calcular nenhuma geometria por conta própria. | 2 | testada |
| `clip_reframe_vertical` | Transformação | Reenquadra um clipe de vídeo/imagem para o quadro do projeto, que precisa ser vertical (`fill`: preenche e corta, sem faixas; `fit`: cabe inteiro). Mede o arquivo com o `ffprobe` (proporção de pixel e rotação incluídas), calcula o retângulo e o aplica pelo filtro `qtblend` (o "Transform" do Kdenlive), o mesmo de `clip_set_transform`. `focus_x`/`focus_y` (0 a 1) escolhem qual parte do vídeo fica visível no corte. Verificada **nos pixels do quadro renderizado** (D-045). | 2 | testada |
| `preview_frame` | Pré-visualização | Exporta um PNG de um quadro específico do projeto (da sessão ou de um caminho explícito), em processo isolado. | 1 | testada |
| `preview_frames` | Pré-visualização | Vários quadros de uma vez (lista, ou intervalo + passo; padrão ~12 espalhados pelo projeto), até 60: um PNG por quadro e uma folha de contato única em grade com o número e o tempo de cada quadro. Rápido: até 3 processos do motor em paralelo (D-051). | 2 | testada |
| `preview_gif` | Pré-visualização | GIF animado de um trecho (até 150 quadros; passo, fps e largura ajustáveis) para conferir movimento sem renderizar vídeo (D-051). | 2 | testada |
| `export_thumbnail` | Pré-visualização | Gera uma miniatura de um vídeo **já renderizado**, extraindo um quadro com `ffmpeg` direto (nunca o libmlt). | 2 | testada |
| `project_validate` | Validação | Confere mídia faltando na bin (arquivo que não existe mais) e trilhas vazias. | 1 | testada |
| `timeline_assemble_ranges` | Lote | Monta uma sequência a partir de vários intervalos do mesmo clipe, em sequência, numa única chamada. | 2 | testada |
| `bin_import_folder` | Lote | Importa todos os arquivos de mídia reconhecidos de uma pasta (não recursivo), sem falhar a chamada inteira por um arquivo com problema. | 2 | testada |
| `render_cancel` | Render | Cancela um render assíncrono em andamento (`export_render` com `wait=false`), encerrando de verdade o processo filho — diferente de `render_queue_stop` do original, ver acima. | 1 | testada |
| `render_progress` | Render | Consulta status e percentual de um render assíncrono em andamento. | 1 | testada |

## Nomes

Todas as ferramentas (herdadas e novas) usam `snake_case`, no formato `grupo_acao`.

## Problemas a corrigir antes de portar (do briefing original, §6.1) — status de cada um

- **`session_undo`/`session_undo_step`** faziam a mesma coisa com granularidade diferente: **corrigido** — unificados em `session_undo` com parâmetro `steps`. O mesmo vale para `session_redo`/`session_redo_step`.
- **`guide_*`/`marker_*`** têm conceitos sobrepostos: **resolvido** (D-041) — no Kdenlive tudo da timeline é guia; o marcador geral é uma guia da categoria 1 e o marcador de clipe vive no clipe da bin.
- **`project_get_render_profiles`** era alias de `project_list_profiles`, com nome enganoso: **corrigido** — removida do catálogo.
- **`timeline_list`, `timeline_get_info` e `track_get_info`** se sobrepunham: **parcialmente resolvido** — cada um manteve um propósito distinto (`timeline_list` é um resumo por trilha, `timeline_get_info` traz tudo incluindo os clipes de cada trilha, `track_get_info` é o detalhe de uma trilha só), mas não houve uma consolidação formal além disso.
- **`render_queue_stop`** só marcava o estado da fila como parado, sem encerrar o processo: **corrigido** — agora encerra de verdade (`SIGTERM`), e `render_cancel` faz o equivalente para jobs assíncronos avulsos.
- **`project_open`** aceitava só `.kdenlive-cli.json`: **corrigido** — abre o `.kdenlive`/MLT XML nativo diretamente.
- **`export_render`** no original não aplicava os presets de render (daí o bug de segmentation fault, ver [riscos e bugs](09-riscos-e-bugs.md)): **corrigido** — sempre aplica codec/bitrate explícitos (decisão D-005).
