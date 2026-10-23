# kdenlive-mcp-plus

Servidor **MCP** (Model Context Protocol) em Rust que deixa um agente de IA **editar vídeo de verdade**: projeto, biblioteca de mídia, trilhas, clipes, efeitos com keyframes, transições, títulos, legendas, áudio, guias, histórico de desfazer e render final. Tudo **sem interface gráfica**, salvando no **formato nativo do Kdenlive** (`.kdenlive`, XML do MLT) e renderizando com o **libmlt**, o mesmo motor do Kdenlive. Com o Kdenlive aberto, você acompanha o vídeo sendo montado: ele recarrega sozinho a cada edição.

93 ferramentas, 103 testes automatizados.

> **kdenlive-mcp-plus** é um nome provisório, e a licença do projeto ainda está pendente de decisão. Ver [docs/decisoes.md](docs/decisoes.md) (D-008 e D-009).

## Vídeos de demonstração

Os vídeos abaixo foram feitos por agentes de IA usando **apenas as ferramentas deste MCP**, sem editar nenhum arquivo à mão e sem material externo. Eles ficam no YouTube, não no repositório, para ele continuar leve.

### Versão dinâmica, feita pelo Claude Opus 5.5

[![Uma IA editou este vídeo sozinha no Kdenlive (Opus 5.5), no YouTube](https://i.ytimg.com/vi/-v83eXFuRwI/hqdefault.jpg)](https://youtu.be/-v83eXFuRwI)

[**▶ Assistir no YouTube**](https://youtu.be/-v83eXFuRwI) (1 min 38 s, sem som).

**Este vídeo foi produzido pelo Claude Opus 5.5**: cerca de 4.700 chamadas ao MCP, em 16 cenas mais curtas e rápidas. Tem palavras em sequência, entradas com "estalo" (o elemento passa do ponto e volta), cortinas coloridas entre as cenas, barras de áudio que pulsam e abaixam no ducking, uma barra de progresso do vídeo inteiro e, em cada cena, as ferramentas MCP usadas. Termina com **criado pela [crom.run](https://crom.run)**.

O script está em [examples/demo_video/construir_demo_dinamico.py](examples/demo_video/construir_demo_dinamico.py). Ele reaproveita o cliente MCP e a base do script do Sonnet e leva ~16 s para montar o projeto e ~8 min para renderizar. Antes do render, o Opus conferiu o vídeo com `preview_frames`, o que mostrou um erro de camadas: textos ficavam escondidos atrás das faixas. O script corrige isso pondo cada elemento novo numa trilha acima do que já está na tela.

### Versão completa, feita pelo Claude Sonnet 5.5

[![Uma IA editou este vídeo sozinha no Kdenlive (Sonnet 5.5), no YouTube](https://i.ytimg.com/vi/TK5J5h2BuKY/hqdefault.jpg)](https://www.youtube.com/watch?v=TK5J5h2BuKY)

[**▶ Assistir no YouTube**](https://www.youtube.com/watch?v=TK5J5h2BuKY) (3 min 37 s, sem som).

**Este vídeo foi produzido pelo Claude Sonnet 5.5 usando apenas as ferramentas deste sistema**: cerca de 2.000 chamadas ao MCP (títulos, trilhas, clipes, transformações com keyframes, guias e render), sem editar nenhum arquivo à mão e sem material externo. Os exemplos são desenhados com títulos e retângulos animados. Cada uma das 16 cenas ilustra um grupo de ferramentas: visão geral, perfis de vídeo, biblioteca e timeline, edição de clipes, efeitos e keyframes, transições, velocidade e reverso, reenquadramento vertical, títulos e legendas, áudio e ducking, guias e marcadores, desfazer e refazer, render e presets, análise do resultado, integração com o Kdenlive aberto e encerramento.

O script que montou o vídeo está em [examples/demo_video/](examples/demo_video/construir_demo.py) (um cliente MCP mínimo em Python mais as 16 cenas; leva ~15 s para montar o projeto e ~12 min para renderizar). A última cena do vídeo mostra 102 testes; hoje são 103. O vídeo foi conferido pelo próprio MCP antes do render, com `preview_frames` (folha de contato, uma imagem com um quadro de cada cena), e o projeto montado está em `.kdenlive`, aberto e editável no Kdenlive.

### Versão vertical para Shorts e TikTok, feita pelo Claude Haiku 5.5

[![Uma IA montou este vídeo vertical sozinha no Kdenlive (Haiku 5.5), no YouTube Shorts](https://i.ytimg.com/vi/3FXcWjukqZM/hqdefault.jpg)](https://youtube.com/shorts/3FXcWjukqZM)

[**▶ Assistir no YouTube Shorts**](https://youtube.com/shorts/3FXcWjukqZM) (53 s, formato 9:16).

**Este vídeo foi produzido pelo Claude Haiku 5.5** com o mesmo MCP: cerca de 760 chamadas, em 7 cenas. Mostra uma conversa em forma de chat entre o pedido e as respostas do agente, as ferramentas em fila, três números, uma timeline vertical, keyframes e um final com **criado pela [crom.run](https://crom.run)**.

O script está em [examples/demo_video/construir_tiktok.py](examples/demo_video/construir_tiktok.py). Ele monta o projeto em ~1 s, e o render leva poucos minutos.

## O que dá para fazer

- **Projetos**: criar com qualquer um dos 76 perfis de vídeo do MLT (SD, HD, Full HD, 4K, vertical 9:16, quadrado...), abrir, salvar, validar e importar `.kdenlive` nativos.
- **Biblioteca (bin)**: importar arquivos e pastas inteiras, organizar em pastas, renomear, cor e nota por clipe.
- **Timeline**: várias trilhas de vídeo e de áudio (nomear, mover, bloquear, ocultar, silenciar), colocar, mover, aparar, dividir e remover clipes, montar sequências a partir de intervalos de um clipe.
- **Clipes**: velocidade (câmera lenta, acelerado) e reverso, opacidade, posição, escala e rotação, reenquadramento de 16:9 para 9:16 com foco ajustável.
- **Efeitos e keyframes**: qualquer filtro do MLT com parâmetros animados no tempo.
- **Transições** entre trilhas (dissolver, cortinas e as demais do MLT).
- **Títulos nativos do Kdenlive** (editáveis no titulador dele) e **legendas SRT** (importar e exportar).
- **Áudio**: nível em dB, normalização de loudness e **ducking** (abaixar a música enquanto a voz fala, com detecção opcional de fala).
- **Guias e marcadores** que aparecem na régua do Kdenlive; cor e nota por clipe.
- **Sessão**: desfazer e refazer (50 níveis), histórico com a descrição de cada passo.
- **Render**: 8 presets, fila de render, progresso, cancelamento de verdade, render de um trecho, miniaturas, exportar o XML.
- **Análise sem renderizar o vídeo inteiro**: um quadro, vários quadros com folha de contato, GIF de um trecho.

## Ferramentas

Todas as 93 têm teste automatizado. A descrição completa de cada uma (parâmetros, retorno, origem, fase) está em [docs/04-ferramentas.md](docs/04-ferramentas.md); o agente também as recebe com descrição em inglês pelo protocolo.

### Servidor e projeto

| Ferramenta | O que faz |
|---|---|
| `server_info` | Nome e versão do servidor, versão do libmlt e lista de presets de render. Chame primeiro para conferir a conexão. |
| `project_new` | Cria um projeto (perfil à escolha; já com uma trilha de vídeo e uma de áudio, que o Kdenlive exige). |
| `project_open` | Abre um `.kdenlive` (ou XML do MLT) na sessão. |
| `project_import_kdenlive` | Importa um `.kdenlive` nativo para o modelo da sessão. |
| `project_save` | Grava o projeto (de forma atômica: o Kdenlive nunca lê um arquivo pela metade). |
| `project_get_info` | Resumo: caminho, perfil, trilhas, clipes na bin, duração. |
| `project_validate` | Confere mídia que sumiu, trilhas vazias, projeto sem trilhas etc. |
| `project_list_profiles` | Lista os 76 perfis de vídeo disponíveis. |
| `project_get_profile` | Perfil atual (largura, altura, fps, proporção). |
| `project_set_profile` | Troca o perfil do projeto. |

### Biblioteca (bin) e clipes de bin

| Ferramenta | O que faz |
|---|---|
| `bin_import_clip` | Importa um arquivo de mídia para a bin. |
| `bin_import_folder` | Importa uma pasta inteira. |
| `bin_list_clips` | Lista os itens da bin. |
| `bin_get_clip_details` | Detalhes de um item (arquivo, duração, nota, cor). |
| `bin_rename` | Renomeia um item. |
| `bin_create_folder` | Cria uma pasta na bin. |
| `bin_move_clip` | Move um item entre pastas. |
| `bin_remove_clip` | Remove um item. |
| `clip_set_color` | Cor de etiqueta de um item da bin. |
| `clip_add_note` | Nota de um item da bin. |

### Timeline e trilhas

| Ferramenta | O que faz |
|---|---|
| `timeline_add_track` | Adiciona uma trilha de vídeo ou de áudio. |
| `timeline_remove_track` | Remove uma trilha e seus clipes. |
| `timeline_add_clip` | Coloca um item da bin numa trilha, numa posição, com ponto de entrada e de saída. |
| `timeline_move_clip` | Muda a posição de um clipe. |
| `timeline_trim_clip` | Ajusta os pontos de entrada e saída. |
| `timeline_split_clip` | Divide um clipe em dois. |
| `timeline_remove_clip` | Remove um clipe da timeline. |
| `timeline_assemble_ranges` | Monta uma sequência a partir de intervalos de um clipe, numa chamada só. |
| `timeline_list` | Trilhas com seus clipes, bloqueio, silêncio. |
| `timeline_get_info` | Visão completa: trilhas, clipes, duração, posição, zoom, guias. |
| `timeline_get_duration` | Duração total. |
| `timeline_get_position` / `timeline_seek` | Posição do cursor (ler e mover). |
| `timeline_get_zoom` / `timeline_set_zoom` | Zoom da timeline (ler e ajustar). |
| `track_get_info` | Detalhes de uma trilha. |
| `track_set_name` | Renomeia uma trilha. |
| `track_set_locked` | Bloqueia ou desbloqueia. |
| `track_set_hidden` | Oculta ou mostra (vídeo). |
| `track_set_muted` | Silencia ou liga (áudio). |
| `track_move_up` / `track_move_down` | Reordena as trilhas. |

### Clipes da timeline

| Ferramenta | O que faz |
|---|---|
| `clip_get_properties` | Entrada, saída, velocidade, filtros do clipe. |
| `clip_set_speed` | Velocidade (0,5 = metade, 2 = dobro, negativo = reverso). |
| `clip_reverse` | Inverte o sentido da reprodução. |
| `clip_set_opacity` | Opacidade de 0 a 1. |
| `clip_set_transform` | Posição, escala e rotação (filtro `qtblend`, o "Transform" do Kdenlive). |
| `clip_reframe_vertical` | Reenquadra um clipe para o quadro vertical (9:16), `fill` ou `fit`, com foco ajustável. |

### Filtros, keyframes e transições

| Ferramenta | O que faz |
|---|---|
| `filter_list_available` | Filtros que o libmlt desta máquina oferece. |
| `filter_add` | Anexa um filtro a um clipe. |
| `filter_list` | Filtros de um clipe. |
| `filter_set_param` | Altera um parâmetro de um filtro. |
| `filter_remove` | Remove um filtro. |
| `keyframe_set` | Valor de um parâmetro num quadro (animado, com interpolação). |
| `keyframe_list` | Keyframes de um parâmetro. |
| `keyframe_remove` | Remove um keyframe. |
| `transition_add` | Transição entre duas trilhas (`luma`, `composite`...). |
| `transition_set` | Altera um parâmetro da transição. |
| `transition_list` | Lista as transições. |
| `transition_remove` | Remove uma transição. |

### Títulos e legendas

| Ferramenta | O que faz |
|---|---|
| `title_add` | Cria um título nativo do Kdenlive (texto, fonte, tamanho, cor, duração). |
| `title_update` | Altera texto ou estilo de um título. |
| `subtitle_import_srt` | Importa um `.srt` como títulos sincronizados. |
| `subtitle_export_srt` | Exporta as legendas da timeline em `.srt`. |

### Áudio

| Ferramenta | O que faz |
|---|---|
| `audio_set_levels` | Nível em dB de um clipe. |
| `audio_normalize` | Normaliza o loudness para uma amplitude alvo. |
| `audio_duck` | Ducking: abaixa um clipe (a música) enquanto outra trilha (a voz) fala; keyframes no `volume`, sem plugins; detecção de fala opcional. |

### Guias e marcadores

| Ferramenta | O que faz |
|---|---|
| `guide_add` / `guide_get` / `guide_update` / `guide_remove` / `guide_list` | Guias (capítulos) na régua da timeline, no formato do Kdenlive. |
| `marker_add` / `marker_list` / `marker_remove` | Marcadores de timeline ou de um clipe. |

### Sessão (desfazer e refazer)

| Ferramenta | O que faz |
|---|---|
| `session_undo` / `session_redo` | Desfaz e refaz (`steps` passos; até 50 níveis). |
| `session_history` | Histórico com a descrição de cada passo. |
| `session_status` | Projeto aberto, alterações pendentes, tamanho do histórico. |

### Render e exportação

| Ferramenta | O que faz |
|---|---|
| `export_list_presets` | Presets de render: `h264_hq`, `h264_fast`, `h265_hq`, `webm_vp9`, `prores`, `lossless`, `gif`, `audio_only`. |
| `export_render` | Renderiza o projeto (ou só um trecho, com `start_frame`/`end_frame`); assíncrono com `wait=false`. |
| `render_progress` / `render_cancel` | Acompanha e cancela de verdade (encerra o processo). |
| `render_queue_add` / `render_queue_list` / `render_queue_status` / `render_queue_start` / `render_queue_stop` | Fila de render, um por vez; parar encerra o render em andamento. |
| `export_xml` | O XML do MLT do projeto. |
| `export_thumbnail` | Miniatura de um vídeo já renderizado. |

### Análise (conferir o resultado)

| Ferramenta | O que faz |
|---|---|
| `preview_frame` | Um quadro do projeto como PNG. |
| `preview_frames` | Vários quadros de uma vez (lista, ou intervalo e passo) e uma **folha de contato** com todos, cada miniatura com o número do quadro e o tempo. |
| `preview_gif` | GIF animado de um trecho, para ver o movimento. |

As animações (keyframes) são avaliadas no quadro certo, e os previews funcionam mesmo quando o servidor é iniciado sem tela (como no Claude Desktop). A folha e o GIF precisam do `ffmpeg`.

## Como funciona

- **Servidor MCP** em Rust ([rmcp](https://github.com/modelcontextprotocol/rust-sdk), transporte stdio). Cada ferramenta devolve `{"ok": true, ...}` ou `{"ok": false, "erro": "<mensagem em português>"}`; erro de negócio nunca vira erro de protocolo.
- **O projeto é o XML do MLT** no formato que o Kdenlive grava (bin, sequência, trilhas como `tractor`, transições internas): o que o agente edita é exatamente o que o Kdenlive abre. O servidor edita esse XML em memória, com histórico de desfazer por snapshot.
- **Render em processo separado**: o próprio binário (`render`/`frame`) roda o libmlt por FFI (bindgen). Se o motor travar, o servidor continua respondendo e devolve um erro claro; cancelar encerra o processo de verdade.
- **Presets mapeados para codec, bitrate e extensão explícitos**; o nome de um preset nunca é passado ao avformat (era o bug que derrubava o projeto de referência).
- Detalhes em [docs/02-arquitetura.md](docs/02-arquitetura.md) e todas as decisões, com motivo e alternativas, em [docs/decisoes.md](docs/decisoes.md).

## Como compilar

Precisa do ambiente descrito em [docs/07-ambiente.md](docs/07-ambiente.md) (libmlt-dev, clang/libclang, ffmpeg). `ffmpeg`/`ffprobe` também são usados em tempo de execução (duração, folha de contato, GIF). Confira com:

```bash
./scripts/check-env.sh
cargo build --release
```

## Configuração no Claude Desktop

Adicione o servidor ao arquivo de configuração do Claude Desktop (em Linux, `~/.config/Claude/claude_desktop_config.json`; no menu: Settings ▸ Developer ▸ Edit Config), ajustando o caminho do binário. Reinicie o Claude Desktop depois:

```json
{
  "mcpServers": {
    "kdenlive-mcp-plus": {
      "command": "/caminho/absoluto/para/kdenlive-mcp-plus/target/release/kdenlive-mcp-plus",
      "args": ["mcp"],
      "env": { "KDENLIVE_MCP_AUTORELOAD": "1" }
    }
  }
}
```

O Claude Desktop inicia o servidor só com `HOME`, `PATH`, `USER`, `LOGNAME` e `SHELL`, sem `DISPLAY` nem `DBUS_SESSION_BUS_ADDRESS`; o servidor já se vira assim (usa o D-Bus padrão do usuário e um Qt sem tela para os previews e renders).

## Configuração no Claude Code

```bash
claude mcp add kdenlive-mcp-plus -- /caminho/absoluto/para/kdenlive-mcp-plus/target/release/kdenlive-mcp-plus mcp
```

Compile em modo release primeiro. Variáveis de ambiente com `-e` (veja `claude mcp add --help`).

## Acompanhando o vídeo sendo criado, com o Kdenlive aberto

Com `KDENLIVE_MCP_AUTORELOAD=1` o servidor grava o projeto a cada edição e **pede ao Kdenlive aberto que o recarregue sozinho** (a mesma ação do File ▸ Revert, pelo D-Bus: sem patch no Kdenlive e sem clicar em nada). O fluxo:

1. Peça ao Claude para criar o projeto e salvar num caminho fixo (ex. "crie um projeto novo e salve em `~/Vídeos/meu-video.kdenlive`"). O projeto novo já vem com uma trilha de vídeo e uma de áudio.
2. Abra esse mesmo arquivo no Kdenlive e espere ele terminar de carregar.
3. Peça as edições. A cada uma, o Kdenlive recarrega e mostra o resultado (a timeline volta ao início, porque é um Revert).

Cuidados:

- **Se você mexer no projeto dentro do Kdenlive e não salvar**, o servidor não recarrega (para não apagar o seu trabalho) e avisa no log. Salve (Ctrl+S) ou desfaça, e a próxima edição volta a recarregar.
- Não abra nem feche projetos no Kdenlive enquanto o Claude edita: recarregar durante o carregamento de outro projeto pode travar o Kdenlive.
- O recarregamento usa o `busctl` (systemd) e o D-Bus da sua sessão, e só enxerga um Kdenlive cujo processo se chama `kdenlive` ou `AppRun.wrapped` (AppImage). Sem isso, use **File ▸ Revert** à mão.
- Em projetos muito grandes, um preview em paralelo pode derrubar um processo do motor por falta de memória; o servidor refaz esse quadro sozinho, um por vez.

### Variáveis de ambiente

| Variável | Efeito |
|---|---|
| `KDENLIVE_MCP_AUTORELOAD=1` | Regrava o projeto a cada edição **e** recarrega o Kdenlive aberto. |
| `KDENLIVE_MCP_AUTOSAVE=1` | Só regrava o projeto a cada edição (o Kdenlive só atualiza com File ▸ Revert). |
| `KDENLIVE_MCP_RENDER_TIMEOUT_SECS` | Tempo limite de um render, em segundos (padrão 1800; aumente para vídeos longos). |
| `KDENLIVE_MCP_BUSCTL` | Caminho do `busctl`, se estiver fora do `PATH` (ou um programa de teste). |

Sem nenhuma das duas primeiras, peça `project_save` depois de cada etapa. Edições feitas pela interface do Kdenlive só chegam ao Claude se ele reabrir o arquivo (`project_open`).

## Conferindo o que foi montado (análise por quadros)

Sem abrir o Kdenlive nem renderizar o vídeo inteiro, o agente pode olhar o projeto: `preview_frame` (um quadro), `preview_frames` (vários de uma vez e a folha de contato), `preview_gif` (movimento) e `export_render` com `start_frame`/`end_frame` (só aquela parte, como vídeo). Foi assim que o vídeo de demonstração foi conferido, cena por cena, antes do render.

## Exemplos de pedidos

- "Crie um projeto horizontal para YouTube e salve em `~/Vídeos/canal.kdenlive`."
- "Importe os clipes da pasta `~/Vídeos/gravacoes`, coloque na timeline em sequência e crie uma guia em cada corte."
- "Reenquadre esse clipe para vertical, com o foco no lado direito."
- "Abaixe a música 15 dB enquanto a narração fala."
- "Mostre os quadros 0, 30, 60 e 90 do projeto" / "faça um GIF dos primeiros 3 segundos".
- "Renderize só do segundo 10 ao 20 em H.264 rápido."

## Render direto (sem MCP)

```bash
cargo run -- render <projeto.kdenlive> <saida.mp4> --preset h264_hq --overwrite
cargo run -- render <projeto.kdenlive> <trecho.mp4> --from 300 --to 600   # só os quadros 300 a 600
cargo run -- frame <projeto.kdenlive> <quadro.png> 120 --overwrite       # um quadro
cargo run -- mcp                                                          # o servidor MCP (stdio)
```

## Testes

```bash
cargo test -- --test-threads=1
cargo clippy --all-targets -- -D warnings
```

103 testes: unitários (modelo, formato, geometria do reenquadramento, aritmética do ducking), contrato MCP real via stdio, e verificações de resultado: o reenquadramento é conferido **nos pixels** do quadro renderizado, o ducking **medindo o áudio renderizado**, e o recarregamento do Kdenlive contra um `busctl` falso (e, manualmente, contra o Kdenlive 26.04.1 e o 23.08.5 reais). Os testes que usam `ffmpeg` são ignorados se ele faltar.

## Limitações conhecidas

- O nome e a licença do projeto estão pendentes ([docs/decisoes.md](docs/decisoes.md)).
- O MCP não gera áudio nem mídia: títulos e formas animadas sim; vídeo e som vêm de arquivos que você importa.
- O Kdenlive 26.04.1 fechou uma vez, sozinho, ao abrir o painel de efeitos de um clipe animado; a causa não foi isolada ([docs/09-riscos-e-bugs.md](docs/09-riscos-e-bugs.md)).
- O recarregamento automático depende de Linux com systemd/D-Bus; em outros sistemas use File ▸ Revert.
- Painéis próprios dentro do Kdenlive exigiriam um fork em C++/Qt e não foram feitos (D-006).

## Documentação

[docs/](docs/README.md): [visão](docs/01-visao.md), [arquitetura](docs/02-arquitetura.md), [concorrentes](docs/03-concorrentes.md), [ferramentas](docs/04-ferramentas.md), [roadmap](docs/05-roadmap.md), [testes](docs/06-testes.md), [ambiente](docs/07-ambiente.md), [licenças](docs/08-licencas.md), [riscos e bugs](docs/09-riscos-e-bugs.md), [glossário](docs/10-glossario.md) e [decisões](docs/decisoes.md).
