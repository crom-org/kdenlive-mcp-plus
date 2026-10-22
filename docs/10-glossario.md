# Glossário

[Índice](README.md) · [Arquitetura](02-arquitetura.md)

- **MLT**: o framework multimídia (*Media Lovin' Toolkit*) do qual o libmlt é a biblioteca; também o nome do formato XML que ele lê e escreve.
- **libmlt**: a biblioteca C que implementa o motor de edição e render do MLT. É o mesmo motor que o Kdenlive usa por baixo — este projeto chama a API C do libmlt diretamente, via FFI (ver [docs/02-arquitetura.md](02-arquitetura.md)).
- **.kdenlive**: a extensão de arquivo de projeto do Kdenlive. Por baixo, é XML do MLT, com algumas convenções e metadados extras específicos do Kdenlive (por exemplo, sobre a organização da bin).
- **producer (produtor)**: um serviço do MLT que fornece quadros de vídeo e/ou áudio a partir de uma fonte — um arquivo de mídia, uma cor sólida (`color`), um tom gerado (`tone`), ou até outro arquivo XML do MLT (`xml`).
- **consumer (consumidor)**: um serviço do MLT que recebe quadros de um produtor e faz algo com eles — grava num arquivo de saída (`avformat`), exibe na tela, etc. Este projeto usa o consumidor `avformat` para renderizar.
- **profile (perfil)**: a configuração básica de vídeo de um projeto MLT — resolução, taxa de quadros, proporção de tela, espaço de cor.
- **playlist**: uma sequência de produtores (clipes) um atrás do outro, formando uma trilha simples.
- **tractor**: o produtor "multitrilha" do MLT — combina várias trilhas (geralmente playlists) numa única saída, aplicando transições entre elas quando configurado. O fixture de teste deste projeto (`tests/fixtures/smoke.xml`) é um tractor com uma trilha de vídeo e uma de áudio.
- **track (trilha)**: uma das camadas de vídeo ou áudio dentro de um tractor.
- **FFI (Foreign Function Interface)**: o mecanismo que permite código Rust chamar funções escritas em C. Neste projeto, é como o Rust chama as funções do libmlt.
- **bindgen**: a ferramenta que lê cabeçalhos C (`.h`) e gera automaticamente declarações Rust equivalentes. É o que gera a camada de FFI deste projeto a partir dos cabeçalhos do libmlt (ver `build.rs` e `src/engine/ffi.rs`).
- **MCP (Model Context Protocol)**: o protocolo aberto que permite um agente de IA descobrir e chamar ferramentas expostas por um servidor — como as deste projeto.
- **stdio**: o transporte do MCP usado pelo servidor deste projeto — mensagens do protocolo trafegam pela entrada/saída padrão do processo, em vez de rede.
- **JSON-RPC**: o formato de mensagem (requisição/resposta em JSON) sobre o qual o MCP é construído.
- **Tier 1**: a categoria de SDKs oficialmente mantidos/endossados pela própria especificação do Model Context Protocol. O `rmcp`, usado neste projeto, é o SDK oficial em Rust, nessa categoria.
- **headless**: que funciona sem interface gráfica nem necessidade de uma tela — o princípio central deste projeto (ver [docs/01-visao.md](01-visao.md)).
- **keyframe**: um ponto no tempo onde se define o valor de um parâmetro (de um efeito, transição, etc.); entre dois keyframes, o valor é interpolado. Ainda não implementado neste projeto (ver `keyframe_set`/`keyframe_list`/`keyframe_remove` em [docs/04-ferramentas.md](04-ferramentas.md), planejados para a Fase 2).
- **bin**: o painel/coleção de mídia importada de um projeto do Kdenlive, antes de ela ser usada na timeline.

---
[Voltar ao índice](README.md)
