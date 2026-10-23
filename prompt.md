# Como usar o kdenlive-mcp-plus (prompt para qualquer agente)

Cole este arquivo no início da conversa de um agente (Claude, Codex, etc.) ou aponte para ele. Ele ensina a usar o servidor MCP **kdenlive-mcp-plus** para editar vídeo, e o que conferir antes de começar.

## O que é

Servidor MCP em Rust (licença **MIT**) que edita vídeo de verdade, sem interface gráfica: projeto, biblioteca (bin), trilhas, clipes, filtros com keyframes, transições, títulos, legendas, áudio, guias, desfazer/refazer e render. Salva no formato nativo do Kdenlive (`.kdenlive`) e renderiza com o libmlt, o mesmo motor do Kdenlive. São 101 ferramentas. Código: `crom-org/kdenlive-mcp-plus` (branch `master`). Documentação completa em `docs/` (ferramentas em `docs/04-ferramentas.md`, bugs conhecidos em `docs/09-riscos-e-bugs.md`).

Regra de ouro: **tudo é feito pelas ferramentas do MCP**. Não edite o XML à mão, não use `melt`/`ffmpeg` para montar o vídeo. `ffmpeg`/`ffprobe` servem só para conferir o resultado.

## 1. Antes de começar (confira nesta ordem)

1. **O MCP está conectado?** Chame `server_info`. Deve devolver nome, versão, versão do libmlt e os presets. Se as ferramentas `mcp__kdenlive-mcp-plus__*` não existirem, pare e avise o usuário: o servidor não está configurado (veja "Configuração" no `README.md`).
2. **Há outro render ou outra sessão usando a máquina?** Rode:
   ```bash
   ps aux | grep -E "[k]denlive-mcp-plus .*(render|frame)|[m]elt|[f]fmpeg"
   ```
   Se houver um `kdenlive-mcp-plus render ...` de outra conversa, **não o mate** e não comece outro render pesado em paralelo: a máquina tem pouca memória livre (~12 de 14 GB em uso) e dois renders juntos podem derrubar o motor. Cada conversa tem o seu próprio processo `kdenlive-mcp-plus mcp`; isso é normal.
3. **O que já existe?** Liste `~/Vídeos/` (e a pasta do projeto) para não sobrescrever um `.kdenlive` ou `.mp4` de outra pessoa/sessão. Escolha um nome novo e caminho **absoluto** para o seu projeto.
4. **Ferramentas de apoio:** `ffmpeg` e `ffprobe` no `PATH` (a folha de contato, o GIF e a duração dependem deles). Em dúvida, `./scripts/check-env.sh`.
5. **Espaço em disco:** `df -h ~/Vídeos`. Um render Full HD longo ocupa centenas de MB.
6. **Leia o pedido inteiro** (título, duração, formato horizontal ou vertical, crédito final, onde publicar) e liste o que falta decidir. Só pergunte o que você realmente não consegue decidir sozinho.
7. **Veja o estado da sessão:** `session_status`. Se já houver projeto aberto que não é o seu, não o altere.

## 2. Fluxo recomendado

1. `project_list_profiles` e escolha o perfil: `atsc_1080p_30` (16:9), `vertical_hd_30` (1080x1920, TikTok/Reels/Shorts) etc. Use nomes reais da lista, nunca inventados.
2. `project_new` (já vem com uma trilha de vídeo e uma de áudio) e logo `project_save` num **caminho fixo**.
3. Se for usar material do usuário: `bin_import_clip` / `bin_import_folder`. O MCP **não gera áudio nem mídia**: o que ele cria sozinho são títulos, formas e animações. Som e vídeo vêm de arquivos importados. Se o vídeo ficar sem som, diga isso ao usuário.
4. Monte por cena: `timeline_add_track`, `timeline_add_clip`, `title_add`, `clip_set_transform`, `filter_add`, `keyframe_set`, `transition_add`. Prefira `timeline_assemble_ranges` a dezenas de chamadas soltas.
5. Salve depois de cada etapa (`project_save`). Com `KDENLIVE_MCP_AUTOSAVE=1` ou `KDENLIVE_MCP_AUTORELOAD=1` o servidor salva sozinho a cada edição.
6. **Confira antes de renderizar**: `preview_frames` (folha de contato com um quadro de cada cena), `preview_frame` e `preview_gif` para o movimento. Olhe a imagem de verdade e corrija o que estiver errado antes do render, que é a parte lenta.
7. `project_validate` (mídia sumida, trilhas vazias).
8. Renderize com `export_render` (preset `h264_hq`; `export_list_presets` mostra os outros). Para vídeo longo use `wait=false` e acompanhe com `render_progress`. Para testar só um trecho, use `start_frame`/`end_frame`.
9. Depois do render, confira o arquivo: `ffprobe` (duração, resolução, codec) e `export_thumbnail`.

Cada ferramenta devolve `{"ok": true, ...}` ou `{"ok": false, "erro": "<mensagem>"}`. Erro de negócio **não** é erro de protocolo: leia o `erro`, corrija e tente de novo. Errou uma edição? `session_undo` (até 50 passos); `session_history` mostra o que foi feito.

## 3. Armadilhas conhecidas (já custaram tempo)

- **Camadas:** cada elemento novo deve ficar numa trilha *acima* de tudo o que já está na tela no mesmo intervalo. Se não, o texto some atrás da faixa. Foi o erro do primeiro vídeo do Opus; o script `examples/demo_video/construir_demo_dinamico.py` mostra a solução.
- **`qtblend` mantém a proporção:** um retângulo que não seja da proporção do quadro vira um menor, com faixas. Para esticar de verdade ligue `distort=1` (`filter_set_param`); para girar em torno do centro, `rotate_center=1`.
- **Nunca** passe o nome de um preset (`h264_hq`) como valor de opção do avformat. Use sempre `export_render` com o nome do preset; ele resolve para codec/bitrate explícitos. Fazer diferente derrubava o projeto original com segmentation fault.
- **Projeto sem nenhuma trilha** fecha o Kdenlive 26. Não use `default_tracks=false` nem remova todas as trilhas.
- **Render longo:** o padrão é 30 min. Vídeo de 3 min em 1080p com muitas camadas leva ~35 min (≈0,3 s por quadro). Aumente `KDENLIVE_MCP_RENDER_TIMEOUT_SECS` ou prefira vídeos curtos. Dois renders ao mesmo tempo não são permitidos pelo servidor (um espera o outro).
- **`clip_set_speed`** só funciona com mídia real (arquivo), não com cores/tons sintéticos.
- **Ferramenta nova** devolve `Json<Resposta>` (struct), nunca `Json<serde_json::Value>`; senão o Claude Desktop recusa a lista inteira de ferramentas.
- **Stdout do modo `mcp`** é só protocolo. Nunca imprima nada ali; logs vão para o stderr.
- **Kdenlive aberto ao mesmo tempo:** com `KDENLIVE_MCP_AUTORELOAD=1` (ou chamando `kdenlive_live` com `enabled: true`) ele recarrega sozinho a cada edição; `kdenlive_reload` recarrega uma vez e `kdenlive_open` abre o Kdenlive com o projeto (prefere o AppImage de `~/Applications`). Não abra nem feche projetos nele enquanto você edita (travou nos testes) e salve as alterações manuais antes de continuar. Teste com o Kdenlive real só no harness próprio (Xvfb `:87` e D-Bus próprio), nunca na tela do usuário.
- **`pkill -f "texto"`** mata o próprio shell se o texto aparecer no comando; use `pkill -f "[t]exto"`. E nunca mate processos que você não iniciou.

## 4. Regras ao entregar

- Diga qual **modelo** fez o vídeo (nos créditos, no README e na descrição) e termine o vídeo com **"criado pela crom.run"**.
- Vídeo publicado como teste leva `(TESTE)` no título. Título sugerido: *(TESTE) Uma IA editou este vídeo sozinha no Kdenlive | MCP | \<modelo\>*.
- Publicar no YouTube e mandar mensagem em grupo é com o usuário: prepare o texto, não envie.
- **Não faça commit nem push** sem o usuário pedir. Quando pedir: rode `cargo test -- --test-threads=1` e `cargo clippy --all-targets -- -D warnings` antes, não apague nem renomeie arquivos versionados fora do pedido, e use a atribuição de commit que o ambiente indicar.
- Não invente versão, licença ou número. O que não foi verificado nesta execução, escreva "a verificar" e diga como verificar.
- A licença do projeto é **MIT** (`LICENSE`, `Cargo.toml` e D-009 já refletem isso). A compatibilidade com módulos GPL do libmlt ao distribuir binários está **a verificar**.

## 5. Relato final

Termine com: caminho do `.kdenlive` e do `.mp4`, duração e resolução medidas com `ffprobe`, número aproximado de chamadas ao MCP, o que ficou de fora ou não foi verificado, e o que depende do usuário (publicar, aprovar o título).
