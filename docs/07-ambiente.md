# Ambiente

Ver também: [visão](01-visao.md) · [arquitetura](02-arquitetura.md) · [testes](06-testes.md) · [decisões](decisoes.md)

## Requisitos

- Ubuntu 24.04 x86_64 (única plataforma verificada nesta execução; ver "Outras plataformas" abaixo).
- Rust/cargo (via [rustup](https://rustup.rs)).
- `libmlt-dev` (fornece os cabeçalhos e o `.pc` do `pkg-config` para `mlt-framework-7`).
- `melt` (CLI do MLT, útil para depurar presets manualmente com `melt -query`).
- `clang`/`libclang-dev` (o `bindgen` precisa de `libclang.so` para gerar os bindings do libmlt).
- `ffmpeg`/`ffprobe` com as libs de desenvolvimento (`libavcodec-dev`, `libavformat-dev` etc.), usadas pelo módulo `avformat` do libmlt.

## Comandos de instalação (Ubuntu 24.04)

```bash
# Dependências do sistema. Isto puxa cerca de 82 pacotes por causa das
# dependências do próprio pacote libmlt7 do Ubuntu (GDAL, OpenCV, Qt5, LLVM) —
# tudo vindo do apt padrão, nada fora do repositório oficial.
sudo apt-get install --no-install-recommends libmlt-dev melt libclang-dev clang

# ffmpeg e suas libs de desenvolvimento. Num ambiente de desenvolvimento
# Ubuntu que já tem o ffmpeg instalado, isto normalmente já está presente;
# se não estiver:
sudo apt-get install --no-install-recommends ffmpeg
```

```bash
# Rust, via rustup, se ainda não tiver:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Importante:** antes dessas instalações, `libmlt-dev`, `melt`, `clang` e `libclang` **não estavam presentes** no ambiente desta execução. A instalação via `sudo apt-get` exige senha interativa, que o agente de IA que construiu este projeto não tinha. Por isso, nesta execução, o responsável pelo projeto rodou manualmente o comando `sudo apt-get install --no-install-recommends libmlt-dev melt libclang-dev clang`, a pedido do agente, antes de o trabalho continuar — ver decisão [D-012](decisoes.md#d-012) em [docs/decisoes.md](decisoes.md).

## Versões verificadas nesta execução

Ambiente: Ubuntu 24.04, x86_64, 22/10/2026.

| Ferramenta | Versão/local verificado |
|---|---|
| `cargo` / `rustc` | 1.96.0 |
| `pkg-config --modversion mlt-framework-7` | `7.22.0` (cabeçalhos em `/usr/include/mlt-7/framework/`) |
| `melt --version` | `7.22.0` |
| `clang --version` | Ubuntu clang 18.1.3 |
| `libclang.so` / `libclang.so.1` | `/usr/lib/llvm-18/lib/` |
| `ffmpeg` / `ffprobe` | 6.1.1-3ubuntu5 |

As libs de desenvolvimento do ffmpeg (`libavcodec-dev`, `libavformat-dev`, `libavutil-dev`, `libswscale-dev`, `libswresample-dev`, `libavfilter-dev`) já estavam instaladas neste ambiente antes da execução.

## Como rodar `scripts/check-env.sh`

O script [`scripts/check-env.sh`](../scripts/check-env.sh) verifica `cargo`, `rustc`, `pkg-config --modversion mlt-framework-7`, `ffmpeg`, `ffprobe`, `libclang.so` e (opcionalmente, sem falhar se faltar) `melt`. Ele **não instala nada** — só diagnostica, e imprime o comando de instalação sugerido quando algo falta.

```bash
./scripts/check-env.sh
```

Nesta execução, depois das instalações acima, ele reportou "Ambiente OK: cargo build / cargo test devem funcionar."

## O que fazer se algo faltar

1. Rode `./scripts/check-env.sh`.
2. Siga as sugestões de instalação que ele imprime para cada item que falhar.
3. Rode o script de novo até a saída final ser "Ambiente OK".

## Outras plataformas

**Pendente.** Este projeto só foi verificado em Ubuntu 24.04 x86_64 nesta execução. macOS e Windows — incluindo os nomes de pacote equivalentes (por exemplo, via Homebrew no macOS ou MSYS2 no Windows, para `libmlt`, `clang` etc.) — ainda não foram testados nem documentados.
