# Visão do projeto

[Índice](README.md) · [Arquitetura](02-arquitetura.md) · [Ferramentas](04-ferramentas.md) · [Roadmap](05-roadmap.md)

## Problema

Hoje não existe uma forma totalmente headless — isto é, sem interface gráfica — de um agente de IA editar vídeo de ponta a ponta (linha do tempo, trilhas, clipes, efeitos, transições, marcadores e renderização final) usando o próprio motor do Kdenlive (o libmlt) e salvando no formato nativo `.kdenlive` (XML do MLT). Sem isso, um agente que precise editar vídeo programaticamente ou depende de automatizar uma interface gráfica (frágil e lento) ou usa um motor diferente do Kdenlive, perdendo a compatibilidade de abrir o resultado no Kdenlive para revisão ou ajuste manual.

## Objetivo

Construir um servidor MCP (Model Context Protocol) em Rust — o `kdenlive-mcp-plus` — que exponha essa edição de vídeo a um agente de IA, chamando diretamente a API C do libmlt e lendo/escrevendo o formato `.kdenlive` nativo. O resultado de qualquer edição feita pelo agente deve sempre poder ser aberto no Kdenlive de verdade pelo responsável pelo projeto, sem conversão.

## Quem usa

- **Um agente de IA**, via protocolo MCP: cria, edita e renderiza projetos de vídeo sozinho, sem nunca precisar de uma tela.
- **O responsável pelo projeto** (humano): pode abrir o mesmo arquivo `.kdenlive` gerado pelo agente no Kdenlive, para revisar o resultado ou ajustar qualquer detalhe à mão.

## Princípios

Estas oito decisões já foram tomadas e não devem ser reabertas; cada uma delas molda como o projeto é construído:

1. **Headless primeiro.** Tudo funciona sem tela. Abrir o Kdenlive é opcional e serve só para revisão humana — nenhuma ferramenta do servidor MCP depende de uma interface gráfica estar presente.
2. **Compatibilidade pelo formato.** O projeto é MLT XML (`.kdenlive`), e o motor é o libmlt — o mesmo motor que o Kdenlive usa internamente. Isso garante que o que o agente produz é exatamente o que o Kdenlive entende, sem camada de tradução.
3. **Nada pela metade.** Uma ferramenta MCP só aparece no `tools/list` quando está implementada **e** testada. Ferramentas ainda não construídas ficam listadas apenas na documentação (ver [docs/04-ferramentas.md](04-ferramentas.md)), com status `planejada`, nunca expostas ao agente antes da hora.
4. **Erros claros.** Toda falha devolve uma mensagem em português dizendo o que aconteceu e, quando fizer sentido, o que fazer a respeito. Nenhuma falha é silenciosa.
5. **Isolamento.** O render roda em um processo separado do servidor MCP. Se o motor libmlt travar (por exemplo, com um sinal do sistema operacional por causa de um projeto malformado ou um codec indisponível), o servidor MCP continua respondendo normalmente ao agente.
6. **Sem sobrescrita silenciosa.** Um arquivo de saída já existente só é sobrescrito se o agente pedir isso explicitamente, com `overwrite=true`.
7. **Stdout é do protocolo.** No modo servidor MCP, a saída padrão (stdout) carrega exclusivamente mensagens do protocolo MCP. Qualquer log do próprio servidor vai para a saída de erro (stderr).
8. **Nada sem verificação.** Quando um fato não foi confirmado, a documentação escreve "a verificar" e diz como verificar, em vez de inventar números, versões ou datas.

## Escopo desta execução (Fase 0)

- Motor de render sobre o libmlt, via FFI direta (sem passar pela linha de comando do `melt`).
- Render headless funcionando de ponta a ponta, isolado em processo separado.
- Servidor MCP mínimo, com exatamente duas ferramentas: `server_info` e `render_project`.
- Testes automatizados: do motor de render, dos presets (incluindo um teste de regressão de um bug conhecido do projeto de referência) e do contrato do protocolo MCP via stdio.
- Toda a documentação do projeto (este conjunto de arquivos em `docs/`, mais o `README.md` e o `CLAUDE.md` na raiz).

Ver [docs/05-roadmap.md](05-roadmap.md) para as fases seguintes.

## Fora do escopo desta execução

- Implementar as demais ferramentas de edição de timeline (cerca de 70, entre herdadas de um projeto de referência e novas propostas) — elas ficam listadas como `planejada` em [docs/04-ferramentas.md](04-ferramentas.md), para as Fases 1 e 2.
- Fazer fork do código-fonte do Kdenlive.
- Criar qualquer interface gráfica para este projeto.

## Critérios de sucesso desta fase

Todos atingidos nesta execução:

- `tools/list` do servidor MCP retorna exatamente `server_info` e `render_project` — nenhuma ferramenta a mais, nenhuma a menos.
- `cargo build`, `cargo test` e `cargo clippy --all-targets -- -D warnings` passam sem avisos.
- Um render manual do projeto de teste (`tests/fixtures/smoke.xml`) gera um `.mp4` que o `ffprobe` confirma como vídeo H.264 em 1280×720 com áudio AAC.
- Nenhum argumento com o nome de um preset (ex. `"h264_hq"`) é passado como valor de uma opção ao avformat — a regressão exata do bug descrito em [docs/09-riscos-e-bugs.md](09-riscos-e-bugs.md) que derrubava o projeto de referência com segmentation fault.
- Toda ferramenta exposta pelo servidor MCP tem teste automatizado cobrindo sucesso e erro.

---
[Voltar ao índice](README.md)
