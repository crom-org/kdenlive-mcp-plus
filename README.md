# kdenlive-mcp-plus

Servidor MCP em Rust que edita vídeo de forma headless usando o libmlt — o mesmo motor de render do Kdenlive —, lendo e escrevendo o formato nativo `.kdenlive` (XML do MLT). Nesta fase (Fase 0), o servidor expõe duas ferramentas MCP: `server_info` e `render_project`. As demais, herdadas e novas, estão planejadas e documentadas em [docs/04-ferramentas.md](docs/04-ferramentas.md), mas ainda não implementadas.

> **kdenlive-mcp-plus** é um nome provisório, e a licença do projeto ainda está pendente de decisão. Ver [docs/decisoes.md](docs/decisoes.md) (D-008 e D-009).

## Como compilar

Precisa do ambiente descrito em [docs/07-ambiente.md](docs/07-ambiente.md) (libmlt-dev, clang/libclang, ffmpeg com libs de desenvolvimento). Confira com:

```bash
./scripts/check-env.sh
```

Depois:

```bash
cargo build --release
```

## Como usar — render direto (sem MCP)

```bash
cargo run -- render <projeto.xml> <saida.mp4> --preset h264_hq --overwrite
```

- `--preset` (opcional, padrão `h264_hq`): nome do preset de render. Veja a lista completa chamando a ferramenta `server_info`, ou em [docs/04-ferramentas.md](docs/04-ferramentas.md): `h264_hq`, `h264_fast`, `h265_hq`, `webm_vp9`, `prores`, `lossless`, `gif`, `audio_only`.
- `--overwrite` (opcional, padrão: não sobrescreve): só com essa flag um `<saida.mp4>` já existente é substituído.

## Como usar — servidor MCP

```bash
cargo run -- mcp
```

Fala MCP via stdio (entrada/saída padrão) e expõe as ferramentas `server_info` e `render_project`.

## Configuração no Claude Code

Confirmado com `claude mcp add --help` nesta execução — a forma de adicionar um servidor stdio local é:

```bash
claude mcp add kdenlive-mcp-plus -- /caminho/absoluto/para/kdenlive-mcp-plus/target/release/kdenlive-mcp-plus mcp
```

Compile em modo release primeiro (`cargo build --release`) e ajuste o caminho absoluto do binário. Rode `claude mcp add --help` para ver as demais opções (variáveis de ambiente com `-e`, escopo com `-s`, etc.).

## Documentação

Veja a documentação completa em [docs/](docs/README.md), a começar por [docs/01-visao.md](docs/01-visao.md).
