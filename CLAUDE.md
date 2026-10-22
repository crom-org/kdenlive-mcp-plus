# CLAUDE.md

Instruções para quem (humano ou agente) continuar este projeto. Documentação completa em [docs/](docs/README.md); decisões já tomadas em [docs/decisoes.md](docs/decisoes.md) — não as reabra, registre uma decisão nova se precisar mudar algo.

## Comandos

```bash
cargo build                              # build
cargo test                               # roda toda a bateria (motor, presets, contrato MCP via stdio)
cargo clippy --all-targets -- -D warnings  # precisa passar sem avisos
./scripts/check-env.sh                   # confere libmlt-dev, melt, ffmpeg, libclang
cargo run -- render <projeto.xml> <saida.mp4> [--preset NOME] [--overwrite]
cargo run -- mcp                         # servidor MCP via stdio
cargo run -- --version
```

## Convenções

- **Idiomas**: documentação, comentários e mensagens de erro em português do Brasil. Identificadores de código em inglês. Descrições das ferramentas MCP (`#[tool(description = ...)]`) em inglês — o agente que as usa interpreta melhor esse idioma.
- **Erros**: tipados (`EngineError` em `src/engine/mod.rs`), nunca `unwrap()`/`expect()` em código de produção (`src/`). Toda mensagem diz o que aconteceu e, quando fizer sentido, o que fazer. Em testes (`tests/`), `unwrap()`/`expect()` são aceitáveis e idiomáticos.
- **`unsafe`**: só dentro de `src/engine/`. Cada bloco `unsafe` tem um comentário `// SAFETY:` explicando por que é seguro.
- **Stdout no modo `mcp`**: carrega só mensagens do protocolo MCP. Logs vão para stderr (hoje via `eprintln!`, ver decisão D-019 — se migrar para `tracing`, registre uma nova decisão).
- **Erros de negócio de `render_project`** (ex. preset inválido, projeto não encontrado, saída já existe) voltam dentro de `Json<RenderProjectOutput>` com `ok: false` e `erro: <mensagem>` — nunca como erro de protocolo MCP (decisão D-017). Mantenha essa convenção em ferramentas novas.
- **Presets de render**: nunca passe o nome de um preset (ex. `"h264_hq"`) como valor de uma opção do avformat (`vcodec`, `acodec`, etc.). É o bug que derrubava o projeto original com segmentation fault (ver [docs/09-riscos-e-bugs.md](docs/09-riscos-e-bugs.md)). Sempre resolva o preset em `src/engine/preset.rs` para valores explícitos antes de configurar o consumidor.
- **Propriedades do libmlt**: nunca assuma o nome de uma propriedade do avformat (ou de qualquer serviço do MLT); confirme com `melt -query "consumer=<id>"` (ou o equivalente para `producer=`/`filter=`/etc.) antes de usar.

## Antes de commitar

Não comite nem dê push sem o usuário pedir explicitamente — nenhum commit foi feito durante a construção desta base (Fase 0).

## O que não fazer

- Não exponha uma ferramenta nova no `tools/list` (ou seja, com `#[tool]`) sem um teste automatizado cobrindo pelo menos o caminho de sucesso e um caminho de erro.
- Não passe o nome de um preset como valor de uma opção do avformat (ver acima).
- Não escreva nada no stdout do processo quando ele estiver rodando em modo `mcp` — isso corrompe o transporte do protocolo. Logs vão para stderr.
- Não adicione `unsafe` fora de `src/engine/`.
- Não assuma que uma função FFI do libmlt teve sucesso sem checar o ponteiro/código de retorno contra nulo/erro.
- Não invente uma versão, uma licença ou um número que não foi verificado nesta execução — escreva "a verificar" e diga como verificar (ver as pendências em [docs/decisoes.md](docs/decisoes.md)).

## Onde estão as decisões

[docs/decisoes.md](docs/decisoes.md) — toda decisão de arquitetura/design relevante, com motivo e alternativas avaliadas, mais a lista de pendências para o responsável pelo projeto (nome final, licença, teste manual no Kdenlive, etc.).
