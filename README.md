# kdenlive-mcp-plus

Servidor MCP em Rust que edita vídeo de forma headless usando o libmlt — o mesmo motor de render do Kdenlive —, lendo e escrevendo o formato nativo `.kdenlive` (XML do MLT). O servidor expõe 93 ferramentas MCP — todo o catálogo de edição (projeto, bin, timeline, filtros, transições, títulos, legendas, áudio com ducking, reenquadramento vertical, análise por quadros/folha de contato/GIF, fila de render e mais), descritas por completo em [docs/04-ferramentas.md](docs/04-ferramentas.md) e todas testadas.

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

Fala MCP via stdio (entrada/saída padrão) e expõe as 93 ferramentas descritas em [docs/04-ferramentas.md](docs/04-ferramentas.md) (projeto, bin, timeline, filtros, transições, títulos, legendas, áudio, fila de render, etc. — chame `server_info` primeiro para confirmar a versão do libmlt e a lista de presets de render).

## Configuração no Claude Desktop

Compile (`cargo build --release`) e adicione o servidor ao arquivo de configuração do Claude Desktop (em Linux, `~/.config/Claude/claude_desktop_config.json`; no menu: Settings ▸ Developer ▸ Edit Config), ajustando o caminho do binário. Reinicie o Claude Desktop depois:

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

## Acompanhando o vídeo sendo criado, com o Kdenlive aberto

Com `KDENLIVE_MCP_AUTORELOAD=1` (como no JSON acima) o servidor grava o projeto a cada edição e **pede ao Kdenlive aberto que o recarregue sozinho** (a mesma ação do File ▸ Revert, pelo D-Bus — sem patch no Kdenlive e sem clicar em nada). O fluxo:

1. Peça ao Claude para criar o projeto e salvar num caminho fixo (ex. "crie um projeto novo e salve em `~/Vídeos/meu-video.kdenlive`" — a ferramenta `project_save`). O projeto novo já vem com uma trilha de vídeo e uma de áudio.
2. Abra esse mesmo arquivo no Kdenlive e espere ele terminar de carregar.
3. Peça as edições. A cada uma, o Kdenlive recarrega e mostra o resultado (a timeline volta ao início, porque é um Revert).

Cuidados:

- **Se você mexer no projeto dentro do Kdenlive e não salvar**, o servidor não recarrega (para não apagar o seu trabalho) e avisa no log do servidor. Salve (Ctrl+S) ou desfaça, e o próximo passo do Claude volta a recarregar.
- Não abra nem feche projetos no Kdenlive enquanto o Claude está editando: recarregar durante o carregamento de outro projeto pode travar o Kdenlive.
- O recarregamento usa o `busctl` (parte do systemd) e o D-Bus da sua sessão. Sem eles, ou sem o Kdenlive aberto, o servidor segue editando normalmente e você usa **File ▸ Revert** à mão.
- Só com `KDENLIVE_MCP_AUTOSAVE=1` (sem `AUTORELOAD`) o arquivo é regravado a cada edição, mas o Kdenlive só atualiza quando você usa File ▸ Revert. Sem nenhuma das duas, peça `project_save` depois de cada etapa.

Edições feitas pela interface do Kdenlive só chegam ao Claude se ele reabrir o arquivo (`project_open`); deixe o Claude montar e use o Kdenlive para conferir.

## Conferindo o que foi montado (análise por quadros)

Sem abrir o Kdenlive nem renderizar o vídeo inteiro, o agente pode olhar o projeto:

- `preview_frame` — um quadro; `preview_frames` — vários de uma vez (por lista ou por intervalo e passo) e **uma folha de contato** com todos, cada miniatura com o número do quadro e o tempo;
- `preview_gif` — uma animação do trecho para ver o movimento;
- `export_render` com `start_frame`/`end_frame` — renderiza só aquela parte como vídeo (mais lento, mas é o resultado final de verdade).

As animações (keyframes) são avaliadas no quadro certo, e os previews funcionam mesmo quando o servidor é iniciado sem tela (como no Claude Desktop). A folha e o GIF precisam do `ffmpeg`.

## Configuração no Claude Code

Confirmado com `claude mcp add --help` nesta execução — a forma de adicionar um servidor stdio local é:

```bash
claude mcp add kdenlive-mcp-plus -- /caminho/absoluto/para/kdenlive-mcp-plus/target/release/kdenlive-mcp-plus mcp
```

Compile em modo release primeiro (`cargo build --release`) e ajuste o caminho absoluto do binário. Rode `claude mcp add --help` para ver as demais opções (variáveis de ambiente com `-e`, escopo com `-s`, etc.).

## Documentação

Veja a documentação completa em [docs/](docs/README.md), a começar por [docs/01-visao.md](docs/01-visao.md).
