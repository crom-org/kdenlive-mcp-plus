# Testes

Ver também: [visão](01-visao.md) · [arquitetura](02-arquitetura.md) · [ferramentas](04-ferramentas.md) · [ambiente](07-ambiente.md) · [riscos e bugs](09-riscos-e-bugs.md)

## Estratégia

A regra do projeto é simples: **toda ferramenta exposta no `tools/list` tem teste automatizado.** É exatamente por isso que, nesta fase (Fase 0), só `server_info` e `render_project` aparecem lá — são as únicas implementadas e testadas. Ver [docs/04-ferramentas.md](04-ferramentas.md) para o status de todas as outras.

A estratégia tem cinco camadas:

### 1. Unitário/regressão de presets (`tests/presets.rs`, 3 testes)

Confere que todo preset da tabela do Apêndice A resolve para codec, bitrate e extensão explícitos. Confere, acima de tudo, que **nenhum valor enviado ao avformat é literalmente o nome do próprio preset** — essa é a regressão do bug do projeto original (`kdenlive-mcp-server`), que passava `preset=h264_hq` direto ao avformat; o libx264 rejeitava "h264_hq" como nome de preset de codificação e o `melt` terminava com segmentation fault (ver [docs/09-riscos-e-bugs.md](09-riscos-e-bugs.md)).

Há uma exceção documentada e intencional nesse teste: o preset `gif` tem `vcodec=gif`, porque "gif" é ao mesmo tempo o nome do preset e o nome real do codec do ffmpeg (confirmado com `ffmpeg -encoders | grep gif`). Esse caso específico é excluído do teste, com um comentário no código explicando por quê — não é o bug do original, é uma coincidência legítima.

O terceiro teste confere que um preset desconhecido gera um erro que lista os nomes válidos.

### 2. Render real contra um fixture sintético (`tests/render.rs`, 4 testes)

Renderiza `tests/fixtures/smoke.xml` — um projeto MLT XML sintético, com uma trilha de vídeo gerada pelo produtor `color` e uma trilha de áudio gerada pelo produtor `tone`, sem nenhuma mídia externa, 100% reproduzível — para MP4, e confere que o arquivo de saída tem mais de 1000 bytes.

Se o `ffprobe` existir no PATH, confere também que o vídeo gerado é H.264 1280×720 com áudio AAC. Os outros dois testes confirmam que um projeto inexistente gera o erro específico `ProjectNotFound`, e que uma saída já existente sem `overwrite=true` gera o erro específico `OutputExists`.

### 3. Contrato do protocolo MCP via stdio (`tests/mcp_stdio.rs`, 5 testes)

Sobe o binário compilado em modo `mcp` como processo filho real, usando o cliente do próprio `rmcp` (feature `transport-child-process`), com o caminho do binário vindo de `env!("CARGO_BIN_EXE_kdenlive-mcp-plus")`. Faz o handshake `initialize`, confere que `tools/list` retorna exatamente `render_project` e `server_info` — nem mais, nem menos —, chama `server_info` e confere a estrutura da resposta, chama `render_project` com o fixture e confere que o arquivo foi gerado, e chama `render_project` de novo contra uma saída já existente sem `overwrite` para confirmar que o erro vem **na mesma estrutura JSON** da resposta de sucesso (campo `ok: false` e `erro: <mensagem>`, nunca um erro de protocolo MCP).

Esse arquivo também testa isolamento: um projeto XML inválido falha dentro do processo filho isolado, mas o servidor continua respondendo normalmente a uma chamada seguinte, na mesma conexão.

### 4. Verificação manual via sessão MCP crua

Além dos testes automatizados, uma sessão manual foi feita nesta execução enviando mensagens JSON-RPC cruas (`initialize`, `notifications/initialized`, `tools/list`, `tools/call`) diretamente pela entrada padrão do binário em modo `mcp`. Essa sessão confirmou que a saída padrão carrega só JSON do protocolo (nenhum log misturado) e que os nomes de ferramenta, versões e schemas aparecem corretamente. Isso foi feito e confirmado nesta execução.

### 5. Verificação manual de abertura no Kdenlive

**Pendente, testar no seu computador** — este ambiente não tem tela, então esta etapa não pôde ser feita aqui. Checklist para o responsável pelo projeto:

1. Instalar o Kdenlive estável atual ([kdenlive.org](https://kdenlive.org)) — a versão do apt do Ubuntu 24.04 (23.08.5) é antiga demais para validar com confiança.
2. Gerar uma saída com `cargo run -- render tests/fixtures/smoke.xml saida.mp4` (ou qualquer projeto XML exportado pelo servidor).
3. Abrir o Kdenlive e tentar importar/abrir o XML original usado no render (não o `.mp4` resultante — o que importa é se o Kdenlive aceita o XML como projeto).
4. Registrar o resultado aqui nesta seção, substituindo esta linha.

**Resultado:** pendente, testar no seu computador.

## Comandos para rodar a bateria completa

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
./scripts/check-env.sh
cargo run -- render tests/fixtures/smoke.xml /tmp/saida_manual.mp4
cargo run -- --version
```

Nesta execução, todos esses comandos foram rodados e passaram: build sem warnings; 12 testes de integração passando (5 em `mcp_stdio.rs`, 3 em `presets.rs`, 4 em `render.rs`); clippy limpo; `check-env.sh` reportando "Ambiente OK"; e o render manual gerando um MP4 H.264 1280×720 com áudio AAC, confirmado por `ffprobe`.
