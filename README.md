# kdenlive-mcp-plus

Servidor MCP em Rust que edita vídeo de forma headless usando o libmlt — o mesmo motor de render do Kdenlive —, lendo e escrevendo o formato nativo `.kdenlive` (XML do MLT). O servidor expõe 89 ferramentas MCP — praticamente todo o catálogo de edição (projeto, bin, timeline, filtros, transições, títulos, legendas, áudio, fila de render e mais), descritas por completo em [docs/04-ferramentas.md](docs/04-ferramentas.md). Duas ferramentas do catálogo original (`clip_reframe_vertical`, `audio_duck`) ficaram fora desta execução, por motivos documentados ali.

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

Fala MCP via stdio (entrada/saída padrão) e expõe as 89 ferramentas descritas em [docs/04-ferramentas.md](docs/04-ferramentas.md) (projeto, bin, timeline, filtros, transições, títulos, legendas, áudio, fila de render, etc. — chame `server_info` primeiro para confirmar a versão do libmlt e a lista de presets de render).

## Configuração no Claude Desktop

Compile (`cargo build --release`) e adicione o servidor ao arquivo de configuração do Claude Desktop (em Linux, `~/.config/Claude/claude_desktop_config.json`; no menu: Settings ▸ Developer ▸ Edit Config), ajustando o caminho do binário. Reinicie o Claude Desktop depois:

```json
{
  "mcpServers": {
    "kdenlive-mcp-plus": {
      "command": "/caminho/absoluto/para/kdenlive-mcp-plus/target/release/kdenlive-mcp-plus",
      "args": ["mcp"],
      "env": { "KDENLIVE_MCP_AUTOSAVE": "1" }
    }
  }
}
```

## Acompanhando o vídeo sendo criado, com o Kdenlive aberto

O Kdenlive não tem API de controle remoto e não detecta sozinho que o arquivo do projeto mudou no disco, então o fluxo é este:

1. Peça ao Claude para criar o projeto e salvar num caminho fixo (ex. "crie um projeto novo e salve em `~/Vídeos/meu-video.kdenlive`" — a ferramenta `project_save`).
2. Abra esse mesmo arquivo no Kdenlive.
3. Com `KDENLIVE_MCP_AUTOSAVE=1` (como no JSON acima), **cada edição que o Claude faz regrava o arquivo sozinha** (de forma atômica, o Kdenlive nunca lê um arquivo pela metade). Sem essa variável, peça `project_save` depois de cada etapa.
4. No Kdenlive, use **File ▸ Revert** para recarregar a versão mais recente do disco e ver o que o Claude acabou de fazer. O Kdenlive marca o projeto como modificado ao abrir um arquivo gerado por fora, então o Revert pede confirmação ("This will delete all changes…"): clique em **Continue**. Se você mexeu no projeto dentro do Kdenlive, salve (Ctrl+S) antes — o Revert descarta alterações não salvas.

Para não editar o mesmo projeto nos dois lugares ao mesmo tempo, deixe o Claude montar e use o Kdenlive para conferir; edições feitas pela interface do Kdenlive só chegam ao Claude se ele reabrir o arquivo (`project_open`).

## Configuração no Claude Code

Confirmado com `claude mcp add --help` nesta execução — a forma de adicionar um servidor stdio local é:

```bash
claude mcp add kdenlive-mcp-plus -- /caminho/absoluto/para/kdenlive-mcp-plus/target/release/kdenlive-mcp-plus mcp
```

Compile em modo release primeiro (`cargo build --release`) e ajuste o caminho absoluto do binário. Rode `claude mcp add --help` para ver as demais opções (variáveis de ambiente com `-e`, escopo com `-s`, etc.).

## Documentação

Veja a documentação completa em [docs/](docs/README.md), a começar por [docs/01-visao.md](docs/01-visao.md).
