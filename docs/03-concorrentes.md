# Concorrentes e referências

Levantamento de projetos que se propõem a editar vídeo via MCP (ou, no caso do Kdenlive/libmlt, que são a base técnica deste projeto). Serve de checklist de funcionalidades e de referência de design — não de base de código (ver [docs/decisoes.md](decisoes.md), decisão D-010).

Todas as datas de consulta abaixo são **22/10/2026**. Onde uma fonte não confirma algo, o campo diz "a verificar" ou "não localizado nas fontes consultadas" — nunca um número ou nome inventado.

## Tabela principal (levantamento original)

| Projeto | Fonte | Licença | Linguagem | O que faz | Motor | Ferramentas | Observações |
|---|---|---|---|---|---|---|---|
| kdenlive-mcp-server (Va1bhav512) | PyPI `kdenlive-mcp-server` 0.1.3 (03/09/2026). Repositório: github.com/Va1bhav512/kdenlive-mcp-server | MIT: LICENSE do pacote 0.1.3 traz "Copyright (c) 2026 Vaibhav". A página do GitHub não mostrou LICENSE na consulta original; ver atualização abaixo | Python ≥ 3.10, FastMCP | Edita projetos Kdenlive via harness `cli-anything-kdenlive`. Projeto em `.kdenlive-cli.json`, não no `.kdenlive` nativo | `melt` via CLI; gera MLT XML | 73 no código (versão 0.1.3); 36 no README (versão 0.1.3) | Referência principal de design para este projeto (não é base de código, ver [docs/decisoes.md](decisoes.md) D-010). Bugs no "Apêndice C" do briefing original deste projeto |
| OrcCut (musyta-labs; aparece como "mcpCut" no Glama) | github.com/musyta-labs/OrcCut | MIT (rótulo da página; texto do LICENSE a verificar) | a verificar | Editor de timeline multitrack para agentes via MCP, com editor no navegador para revisão | Conflito nas fontes: o Glama diz FFmpeg/MLT; a página do GitHub (na consulta original) não citava MLT | 44 (README, consulta original) | O mais próximo da proposta deste projeto — mas ver nota de atualização abaixo: uma segunda pesquisa não localizou o repositório |
| Kinocut (KyaniteLabs; antes "mcp-video") | github.com/KyaniteLabs/kinocut | Apache-2.0 | Python 3.11+ | Edição de arquivos (cortes, legendas, reformatação, checagens). Não é timeline multitrack | FFmpeg (+ Hyperframes e Revideo; Whisper opcional) | 203 MCP (declaradas pelo projeto) | 219 estrelas; release mais recente 1.16.2 (21/10/2026). MLT e Kdenlive não citados |
| Velorn (VelornLabs) | github.com/VelornLabs/velorn | GPL-3.0 | Electron + React | Editor desktop com timeline, texto, keyframes e exportação. MCP em loopback (`http://127.0.0.1:19790/mcp`, confirmado) | FFmpeg (a fonte cita apenas "codecs" e aceleração por hardware; confirmação textual exata de "FFmpeg" ainda a verificar) | a verificar | Depende do app aberto com projeto (não é headless) |
| mcp-kdenlive (D-Ogi) | github.com/D-Ogi/mcp-kdenlive | não informada (confirmado em segunda consulta: nenhum arquivo de licença visível no repositório) | Python 3.10+ | Controla o Kdenlive em execução via D-Bus (`kdenlive-api`) | Kdenlive, via fork próprio (D-Ogi/kdenlive) com patch, confirmado | 29 (contagem própria: 5 compostas + 24 atômicas, confirmado) | Exige build do Kdenlive com patch. Alternativa a estudar na Fase 3 (ver [docs/05-roadmap.md](05-roadmap.md)) |
| CLI-Anything (HKUDS) | github.com/HKUDS/CLI-Anything | **Apache-2.0, confirmado** (badge + arquivo LICENSE do repositório) | Python | Gera CLIs para agentes. Contém `kdenlive/agent-harness`, usado pelo kdenlive-mcp-server | — | — | **Conflito confirmado**: o LICENSE do kdenlive-mcp-server original alega que o vendor `cli_anything` vem deste repositório sob MIT, mas o repositório HKUDS/CLI-Anything é Apache-2.0. Divergência real entre o que o original declara e a licença da sua própria fonte (ver [docs/08-licencas.md](08-licencas.md)) |

## Atualização da pesquisa (segunda rodada, 22/10/2026)

- **Va1bhav512/kdenlive-mcp-server**: existe arquivo `LICENSE` na raiz do repositório, mas o tipo não ficou visível só pela listagem do GitHub (não dá para confirmar se é MIT a partir disso — a verificar abrindo o arquivo). **Divergência nova**: o README atual do repositório declara **75 ferramentas**, mas as próprias tabelas do README somam só **38** — uma terceira contagem, diferente tanto do "36" quanto do "73" da versão 0.1.3 analisada originalmente. Há uma pasta `vendor/`, mas o `cli-anything-kdenlive` aparece instalado via `uv.sources` apontando para o GitHub (HKUDS/CLI-Anything), não necessariamente vendorizado como código copiado.
- **musyta-labs/OrcCut**: uma segunda busca na web **não encontrou o repositório nem a organização "musyta-labs"**. Foram encontrados projetos adjacentes (ex. "CatCut", Apache-2.0, MCP + Kdenlive, alpha, só Windows; e um pacote "mlt-python"), mas nenhum confirma a existência do OrcCut. Trata-se de "não localizado nas fontes consultadas", não apenas "a verificar" — a conclusão de posicionamento, abaixo, considera essa ressalva.
- **KyaniteLabs/kinocut**: confirmado Apache-2.0, 219 estrelas, release mais recente 1.16.2 (21/10/2026), sem citar MLT nem Kdenlive em nenhum lugar.
- **VelornLabs/velorn**: confirmado GPL-3.0 (não "GPLv3" genérico; é especificamente GPL-3.0); Electron + React confirmados; MCP roda em loopback `http://127.0.0.1:19790/mcp`, confirmado.
- **D-Ogi/mcp-kdenlive**: confirmado que não há arquivo de licença visível; 29 ferramentas (5 compostas + 24 atômicas); depende de um fork próprio (D-Ogi/kdenlive) com patch, mais D-Bus.
- **HKUDS/CLI-Anything**: confirmado Apache-2.0 via badge e arquivo LICENSE — o conflito de licença com o vendor do kdenlive-mcp-server original é real, não suposição.

## Outros projetos analisados nesta segunda rodada

- **burningion/video-editing-mcp**: **não é sobre MLT nem Kdenlive**. Usa o serviço comercial "Video Jungle" mais ferramentas como `add-video`/`search-videos`/`generate-edit-from-videos`; a edição local gera um projeto OpenTimelineIO para o DaVinci Resolve Studio. Python. Licença não declarada na página do repositório. Tratado como "fora do escopo direto" (depende de um serviço externo proprietário), mas documentado aqui porque era um item pendente de análise no levantamento original.

## Ainda não analisados

- Entradas do Glama com os códigos `zkmy4z1u2d`, `j3szyylq6x`, `b7xn3zuvkl` e `x15v5qmjnq` — não analisadas nesta rodada.
- Não foi encontrado MCP próprio para **Shotcut**, **OpenShot** ou **Olive**.
- Há um pedido por MCP no fórum do Shotcut — "a verificar" (não confirmado se já existe implementação).

## Fora do escopo (editores proprietários, só para contexto)

- AdobePremiereProMCP
- MCP do DaVinci Resolve

Nenhum dos dois foi analisado em profundidade: são citados apenas como contexto de mercado, por serem editores comerciais fechados, fora do escopo de comparação com um motor aberto (libmlt) e formato aberto (`.kdenlive`/MLT XML).

## Conclusão (com cautela)

Com base apenas nas fontes consultadas nas duas rodadas de pesquisa (22/10/2026), **não foi confirmado** que algum dos projetos analisados entregue, ao mesmo tempo:

1. leitura e escrita do `.kdenlive` nativo (não um formato intermediário próprio);
2. render headless através do próprio libmlt (não apenas FFmpeg, nem dependente de uma GUI aberta); e
3. ferramentas com testes automatizados comprováveis (não apenas "básicos de sucesso", como o caso documentado do kdenlive-mcp-server original).

Essa é exatamente a combinação que o kdenlive-mcp-plus se propõe a entregar. A frase acima deve ser lida como "não confirmado nas fontes consultadas" — não como uma afirmação absoluta de que nenhum projeto no mundo faz isso; novas fontes ou versões mais recentes desses projetos podem mudar esse quadro, especialmente no caso do OrcCut, que não foi localizado para reconfirmação.

---

[Voltar para o índice da documentação](README.md)
