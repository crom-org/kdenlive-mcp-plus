# Licenças

## 1. Licença deste projeto

**Pendente.** O campo `license` não foi preenchido no `Cargo.toml` — é uma decisão do responsável pelo projeto, registrada como D-009 em [docs/decisoes.md](decisoes.md). Nenhum outro documento deste repositório deve assumir uma licença específica para o kdenlive-mcp-plus até essa decisão ser tomada.

## 2. Dependências Rust diretas

Licenças confirmadas rodando `cargo info <crate>` neste ambiente, em 22/10/2026:

| Crate | Versão usada | Licença |
|---|---|---|
| rmcp | 3.6.0 | Apache-2.0 |
| bindgen (build-dependency) | 0.73.2 | BSD-3-Clause |
| pkg-config (build-dependency) | 0.3.34 | MIT OR Apache-2.0 |
| tokio | 1.54.1 | MIT |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| schemars | 1.2.3 | MIT |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |

Esta tabela cobre **só as dependências diretas** — as listadas em `[dependencies]`/`[build-dependencies]` do `Cargo.toml`. As dependências transitivas (dezenas de outras crates, trazidas por rmcp, tokio etc.) ainda não têm um levantamento de licença completo.

**Pendência:** rodar uma auditoria completa de licenças transitivas antes de qualquer distribuição binária, por exemplo com `cargo install cargo-license && cargo license`, ou com `cargo-deny`. Nenhuma licença transitiva deve ser assumida como compatível sem essa auditoria.

## 3. libmlt

Núcleo sob **LGPL-2.1**, confirmado no arquivo `COPYING` e no badge do repositório github.com/mltframework/mlt (consulta de 22/10/2026). O mesmo repositório também contém, na raiz, os textos completos da GPLv2 e da GPLv3, mas não há confirmação textual de quais módulos/plugins específicos (ex. módulos de efeito individuais) usam GPL em vez de LGPL.

**A verificar módulo por módulo** antes de redistribuir qualquer binário vinculado estaticamente a módulos específicos do libmlt — este projeto, por ora, só usa o framework central via FFI (`mlt_factory_*`, `mlt_profile_*`, `mlt_producer_*`, `mlt_consumer_*`) e os serviços registrados em tempo de execução (`xml`, `avformat`), carregados dinamicamente pelo próprio libmlt, o que reduz o risco imediato mas não substitui essa auditoria.

## 4. Kdenlive

Divergência real entre fontes, confirmada em 22/10/2026:

- O arquivo `COPYING` em `invent.kde.org/multimedia/kdenlive` diz **GPL-3.0-only**.
- A Wikipedia diz **GPL-3.0-or-later**.
- Uma issue do próprio GitLab do Kdenlive (#1167) diz que a licença real é `LicenseRef-KDE-Accepted-GPL`, sob a convenção [REUSE](https://reuse.software/) — ou seja, misturada por arquivo, não uma licença única e uniforme para o projeto inteiro.

Esta é uma divergência real a ser resolvida (lendo os cabeçalhos SPDX dos arquivos relevantes do Kdenlive) antes de qualquer decisão que dependa da licença exata do projeto Kdenlive — não é um erro desta pesquisa, é o estado real das fontes.

Este projeto (kdenlive-mcp-plus) não embute nem redistribui código do Kdenlive; apenas lê/escreve o formato de arquivo `.kdenlive` (XML do MLT) e usa o mesmo motor de render (libmlt) que o Kdenlive usa. A licença do Kdenlive em si só passaria a importar diretamente se uma fase futura (Fase 3, ver [docs/05-roadmap.md](05-roadmap.md)) viesse a embutir ou derivar código do Kdenlive.

## 5. Aviso de marca

"Kdenlive" é marca registrada da KDE e.V., conforme aviso no `LICENSE` do projeto de referência `kdenlive-mcp-server` (ver [docs/03-concorrentes.md](03-concorrentes.md)). Este projeto (kdenlive-mcp-plus) **não é afiliado à KDE e.V. nem ao projeto Kdenlive**; o nome é usado apenas de forma descritiva, para indicar compatibilidade de formato de arquivo e uso do mesmo motor de render. O nome do projeto é provisório — ver decisão D-008 em [docs/decisoes.md](decisoes.md).

## 6. Projetos de referência (registro, sem análise jurídica)

Para contexto, as licenças dos projetos citados em [docs/03-concorrentes.md](03-concorrentes.md), sem que isso constitua uma análise jurídica de compatibilidade:

- **kdenlive-mcp-server**: MIT (com a ressalva sobre a divergência entre o LICENSE do pacote PyPI e o que é visível no repositório GitHub, ver docs/03-concorrentes.md).
- **HKUDS/CLI-Anything**: Apache-2.0, confirmado — e isso conflita com a alegação de MIT feita pelo vendor do kdenlive-mcp-server original para o mesmo código (mesma ressalva).
- **Kinocut**: Apache-2.0.
- **Velorn**: GPL-3.0.
- **mcp-kdenlive (D-Ogi)**: sem licença declarada.
- **OrcCut**: não localizado para confirmação (ver docs/03-concorrentes.md).

## 7. Regras de atribuição

Este projeto não copia código de nenhum dos projetos de referência listados acima — decisão D-010 em [docs/decisoes.md](decisoes.md). Por isso, não há obrigação de atribuição de código-fonte de terceiros. Ainda assim, o kdenlive-mcp-server original é citado como referência de design onde relevante, incluindo em [docs/09-riscos-e-bugs.md](09-riscos-e-bugs.md) (bugs e particularidades que motivaram decisões de arquitetura deste projeto).

---

[Voltar para o índice da documentação](README.md)
