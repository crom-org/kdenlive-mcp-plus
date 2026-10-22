#!/usr/bin/env bash
# Verifica se o ambiente tem o que o kdenlive-mcp-plus precisa para compilar e
# testar: cargo, rustc, pkg-config (com mlt-framework-7), ffmpeg, ffprobe e
# libclang (necessário para o bindgen). Não instala nada; só diagnostica.
#
# Uso: scripts/check-env.sh

set -u
falhou=0

verificar_comando() {
    local nome="$1"
    if command -v "$nome" >/dev/null 2>&1; then
        echo "ok: $nome encontrado em $(command -v "$nome")"
    else
        echo "faltando: $nome não encontrado no PATH"
        falhou=1
    fi
}

echo "=== cargo e rustc ==="
verificar_comando cargo
verificar_comando rustc
if command -v cargo >/dev/null 2>&1; then
    cargo --version
    rustc --version
fi

echo
echo "=== pkg-config e mlt-framework-7 (libmlt-dev) ==="
verificar_comando pkg-config
if command -v pkg-config >/dev/null 2>&1; then
    if pkg-config --modversion mlt-framework-7 >/dev/null 2>&1; then
        echo "ok: mlt-framework-7 versão $(pkg-config --modversion mlt-framework-7)"
    else
        echo "faltando: pkg-config não encontrou mlt-framework-7."
        echo "  instale com: sudo apt-get install --no-install-recommends libmlt-dev"
        falhou=1
    fi
fi

echo
echo "=== ffmpeg e ffprobe ==="
verificar_comando ffmpeg
verificar_comando ffprobe

echo
echo "=== libclang (necessário para o bindgen) ==="
if find /usr/lib /usr/lib64 /usr/local/lib -iname "libclang.so*" 2>/dev/null | grep -q .; then
    echo "ok: libclang encontrada"
    find /usr/lib /usr/lib64 /usr/local/lib -iname "libclang.so*" 2>/dev/null
else
    echo "faltando: libclang.so não encontrada."
    echo "  instale com: sudo apt-get install --no-install-recommends libclang-dev clang"
    falhou=1
fi

echo
echo "=== melt (opcional, útil para depurar presets manualmente) ==="
verificar_comando melt

echo
if [ "$falhou" -eq 0 ]; then
    echo "Ambiente OK: cargo build / cargo test devem funcionar."
    exit 0
else
    echo "Faltam dependências (ver mensagens acima). Instale-as e rode este script de novo."
    exit 1
fi
