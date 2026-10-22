# Arquitetura

Ver também: [visão](01-visao.md) · [ferramentas](04-ferramentas.md) · [riscos e bugs](09-riscos-e-bugs.md) · [decisões](decisoes.md)

## Componentes

```
┌──────────────┐  tools/call (stdio, JSON-RPC)   ┌─────────────────────┐
│ Agente de IA │ ───────────────────────────────▶ │ Servidor MCP        │
│ (cliente MCP)│ ◀─────────────────────────────── │ (src/mcp)           │
└──────────────┘      resultado estruturado       └──────────┬──────────┘
                                                              │ spawn do próprio
                                                              │ binário, subcomando
                                                              │ `render` (D-003)
                                                              ▼
                                                   ┌─────────────────────┐
                                                   │ Processo filho       │
                                                   │ (render isolado)     │
                                                   │ src/engine::render   │
                                                   └──────────┬──────────┘
                                                              │ FFI (unsafe,
                                                              │ só em src/engine)
                                                              ▼
                                                   ┌─────────────────────┐
                                                   │ libmlt (C)           │
                                                   │ produtor "xml" ──▶   │
                                                   │ consumidor "avformat"│
                                                   └──────────┬──────────┘
                                                              │ lê           escreve
                                                              ▼              ▼
                                                   projeto .kdenlive   arquivo final
                                                   (XML do MLT)        (mp4/webm/gif/...)
```

- **Servidor MCP** (`src/mcp/`): fala o protocolo MCP via stdio (transporte `rmcp::transport::stdio`, SDK oficial `rmcp`). Expõe as ferramentas (nesta fase, `server_info` e `render_project`) e nunca chama o libmlt diretamente — isso é o ponto central do isolamento (D-003).
- **Processo de render** (subcomando `render` do próprio binário, `src/main.rs` + `src/engine/worker.rs`): é o processo filho que o servidor MCP invoca para isolar o motor libmlt. Um crash do motor C (ex. SIGSEGV por um projeto malformado) derruba só esse processo filho; o servidor MCP continua respondendo normalmente ao agente (verificado em teste, ver [06-testes.md](06-testes.md)).
- **Motor libmlt** (`src/engine/`): a camada que fala C via FFI. `ffi.rs` são os bindings gerados pelo `bindgen`; `render.rs` é a sequência de chamadas que renderiza; `preset.rs` resolve nomes de preset para opções concretas de codec/bitrate; `worker.rs` é quem spawna e supervisiona o processo filho (timeout, captura de stderr, interpretação de sinal).
- **Modelo do projeto**: não há um modelo de dados próprio nesta fase — o projeto É o XML do MLT (`.kdenlive`), lido e escrito diretamente pelo libmlt (D-004). As ferramentas de edição da Fase 1+ vão manipular esse XML (via o próprio libmlt ou diretamente), nunca um modelo intermediário convertido de/para JSON.
- **Persistência**: o arquivo `.kdenlive`/XML no disco é a única fonte de verdade. Não há banco de dados nem estado de sessão persistente nesta fase (ver "Undo/redo" abaixo sobre a Fase 1+).

## Fluxo de uma chamada de ponta a ponta (`render_project`)

1. O agente chama a ferramenta `render_project` via `tools/call`, com `project_path`, `output_path` e, opcionalmente, `preset` e `overwrite`.
2. O servidor MCP (`KdenliveMcpServer::render_project_impl`, em `src/mcp/server.rs`) resolve o preset (`engine::preset::resolve_preset`), valida que a extensão de `output_path` combina com o preset, confere que `project_path` existe e que `output_path` não existe (a menos que `overwrite=true`) — tudo isso **antes** de gastar um processo filho.
3. Um `tokio::sync::Mutex` garante que só um render acontece por vez neste servidor (D-016).
4. `engine::worker::render_isolated` spawna o próprio binário como processo filho: `<binário> render <projeto> <saida> --preset <nome> [--overwrite]`, com um timeout configurável (`KDENLIVE_MCP_RENDER_TIMEOUT_SECS`, padrão 1800s) e `kill_on_drop` para garantir que o processo seja morto se o timeout disparar.
5. Dentro do processo filho, `engine::render::render` roda a sequência verificada do libmlt (ver abaixo) e devolve sucesso ou um `EngineError` tipado, impresso em português no stderr e refletido no código de saída.
6. O servidor MCP interpreta o resultado do processo filho (código de saída, stderr, sinal) e monta a resposta — sempre com a mesma estrutura, sucesso ou erro (D-017): `{ok, caminho_saida, tamanho_bytes, preset_usado, duracao_segundos, notas, erro}`.
7. Em caso de sucesso, o servidor mede a duração do arquivo final com `ffprobe` (se disponível) e devolve o resultado estruturado ao agente.

## Formato `.kdenlive`

O projeto é o XML nativo do MLT/Kdenlive (`.kdenlive`, que é XML do MLT com a extensão renomeada). Esta fase não grava nenhum projeto — ela só lê um XML existente, via o serviço `xml` do libmlt (`mlt_factory_producer(profile, "xml", caminho)`), e renderiza. Escrever/editar o XML (criar projeto, adicionar trilhas e clipes etc.) é trabalho da Fase 1+ (ver [05-roadmap.md](05-roadmap.md) e [04-ferramentas.md](04-ferramentas.md)).

Estrutura MLT básica (confirmada rodando `melt` neste ambiente, ver `tests/fixtures/smoke.xml`): um `<profile>` (resolução, fps, aspecto); um ou mais `<producer>` (fonte de mídia ou geração sintética, como `color` ou `tone`); `<playlist>` (uma sequência de entradas de um produtor numa trilha); e um `<tractor>` com `<track>`s, cada uma apontando para uma playlist — é o que combina as trilhas de vídeo e áudio num resultado final. Ver [10-glossario.md](10-glossario.md) para definições de cada termo.

## Modelo de dados (conceitual, para a Fase 1+)

Ainda não implementado nesta fase, mas o vocabulário já é fixado pelo próprio formato MLT: **projeto** (o XML inteiro) → **perfil** (resolução/fps/aspecto) → **bin** (coleção de mídia importada, ainda não mapeada no MLT nesta fase — ver [10-glossario.md](10-glossario.md)) → **trilhas** (`track`, dentro do `tractor`) → **clipes** (entradas de uma `playlist`, com `in`/`out`) → **filtros** (efeitos anexados a um clipe ou trilha) → **transições** (entre duas camadas de trilha) → **guias/marcadores** (pontos no tempo, ver a nota de unificação em [04-ferramentas.md](04-ferramentas.md)).

## Camada de modelo (Fase 2)

A Fase 2 implementou praticamente todo o catálogo de edição por cima de uma camada de modelo própria, em `src/model/`:

- **`model::document`**: funções que leem/constroem/modificam um documento MLT XML em memória, representado como um DOM mutável (`xmltree::Element`, crate `xmltree`) — não um modelo tipado próprio nem objetos libmlt vivos via FFI (decisão D-022 em [decisoes.md](decisoes.md)). Essa escolha preserva o isolamento da decisão D-003: editar um projeto nunca chama o libmlt; só `render`/`preview_frame` (processos filho isolados) tocam o motor.
- **Convenções próprias deste módulo** (nenhuma é um conceito do MLT nem do Kdenlive, são só como este servidor organiza as coisas):
  - **Referência de clipe**: a string `"<track_id>:<índice>"` (decisão D-023) — localiza um clipe de volta depois de listado, já que o MLT não dá id estável a entradas de playlist.
  - Inserção de clipe na timeline (`timeline_add_clip`/`move_clip`/`split_clip`) aceita qualquer posição: no fim da trilha (cria um `<blank>` de preenchimento), exatamente no início de um clipe existente (empurra esse clipe e os seguintes para depois, usando a posição implícita do MLT) ou dentro de um `<blank>` (divide o blank em dois). Posição estritamente dentro de um `<entry>` já ocupado continua erro — sem "ripple" parcial.
- **`model::session::Session`**: o documento atual + histórico de undo/redo por snapshot (até 50 entradas) + contadores de id monotônicos (nunca reaproveitam um id removido na mesma sessão) + `playhead`/`zoom` (posição do cursor e nível de zoom da timeline — conceitos só de UI/sessão, sem efeito no render, já que não há player nesta fase headless).
- **`model::srt`**: leitura/escrita do formato público SRT (não uma convenção do Kdenlive), usado por `subtitle_import_srt`/`export_srt`.
- **Produtores e filtros do MLT usados pela edição** (todos confirmados via `melt -query` e renderização real, nunca inventados): produtor `timewarp` (velocidade/reversão de clipe, D-027), produtor `pango` (títulos, D-032, em vez do formato interno `kdenlivetitle` do Kdenlive, cujo esquema não foi confirmado), filtros `brightness` (opacidade), `qtblend` (posição/escala/rotação) e `volume` (nível/normalização de áudio) — todos anexados como `<filter>` filho do `<entry>` da timeline, não do `<producer>` compartilhado (confirmado que isso dá um efeito por corte, independente de outros usos do mesmo produtor — decisão D-028). Transições (`<transition>`) ficam no tractor principal, com índices numéricos de trilha.
- **Fila/jobs de render**: `engine::jobs::JobRegistry` (jobs assíncronos avulsos de `export_render(wait=false)`, com status `Queued`/`Running`/`Done`/`Failed`/`Cancelled`) e `engine::queue::RenderQueue` (construída por cima do mesmo registro e do mesmo `render_lock` compartilhado — não é um mecanismo paralelo). `render_cancel`/`render_queue_stop` encerram de verdade o processo filho em andamento (sinal real), diferente do bug equivalente do projeto original.

## Undo/redo por snapshot

Implementado na Fase 2 (ver `model::session::Session`, acima). Cada ferramenta de edição chama `snapshot_before` antes de mutar o documento; `session_undo`/`session_redo` (unificados com `session_undo_step`/`session_redo_step` do catálogo original, que não existem mais como ferramentas separadas — ver [04-ferramentas.md](04-ferramentas.md)) navegam essa pilha de snapshots, com um parâmetro `steps`.

## Erros

Todo erro do motor é um `EngineError` tipado (`src/engine/mod.rs`), com mensagem em português explicando o que aconteceu e, quando faz sentido, o que fazer: `ProjectNotFound`, `OutputExists`, `InvalidText` (nome de preset desconhecido, texto que não converte para C string etc.), `Create` (falha ao criar um objeto do libmlt), `Connect`, `Start`, `OutputMissing` (arquivo de saída ausente ou vazio depois do render), `Timeout` (processo filho excedeu o tempo configurado), `ChildFailed` (processo filho terminou com erro, com o código e o stderr capturados) e `Signal` (processo filho morto por um sinal, ex. SIGSEGV).

O servidor MCP nunca propaga esses erros como erro de protocolo MCP — eles sempre voltam dentro da mesma estrutura JSON da resposta de sucesso, com `ok: false` e `erro: <mensagem>` (D-017), para que o agente trate os dois casos do mesmo jeito.

## Logs

Logs deste processo (os poucos que existem nesta fase: erros do CLI) vão para stderr via `eprintln!` (D-019). O stdout, no modo `mcp`, carrega só mensagens do protocolo MCP — confirmado numa sessão manual (ver [06-testes.md](06-testes.md)). Os avisos que o próprio libmlt/ffmpeg imprimem no stderr durante o render (ex. "Timestamps are unset in a packet") também vão para lá, nunca para o stdout; ver [09-riscos-e-bugs.md](09-riscos-e-bugs.md).

## Decisões de design referenciadas aqui

D-001 (Rust), D-002 (libmlt via FFI), D-003 (render em processo separado), D-004 (MLT XML nativo), D-005 (presets explícitos), D-006 (sem fork do Kdenlive nesta fase), D-022 a D-036 (modelo em memória, convenções de referência de clipe/bin/guias/marcadores, produtores e filtros do MLT usados pela edição, fila de render) — ver [decisoes.md](decisoes.md) para o motivo e as alternativas avaliadas de cada uma.
