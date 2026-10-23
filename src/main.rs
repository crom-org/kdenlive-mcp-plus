//! CLI: `mcp` inicia o servidor MCP via stdio; `render` renderiza direto,
//! sem MCP (é o mesmo subcomando que o servidor MCP invoca como processo
//! filho isolado, ver `engine::worker`); `--version` imprime a versão.
//!
//! Erros vão para stderr, com código de saída diferente de zero. No modo
//! `mcp`, o stdout carrega só mensagens do protocolo MCP.

use std::path::PathBuf;
use std::process::ExitCode;

const USO: &str = "uso: kdenlive-mcp-plus <mcp|render|frame|--version>\n\
     \n\
     \x20 mcp                                                    inicia o servidor MCP via stdio\n\
     \x20 render <projeto> <saida> [--preset NOME] [--overwrite]  renderiza direto, sem MCP\n\
     \x20 frame <projeto> <saida.png> <quadro> [--overwrite]      renderiza um quadro único (preview_frame)\n\
     \x20 --version                                               mostra nome e versão";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let codigo: u8 = match executar(&args) {
        Ok(()) => 0,
        Err(mensagem) => {
            eprintln!("erro: {mensagem}");
            1
        }
    };
    // `render` e `frame` são subprocessos de uma tarefa só: depois de gravar o
    // arquivo, saem sem a limpeza do `atexit` do libmlt, que derruba o
    // processo se uma thread de render ainda estiver no módulo Qt.
    if matches!(args.get(1).map(String::as_str), Some("render" | "frame")) {
        kdenlive_mcp_plus::engine::encerrar_sem_limpeza(i32::from(codigo));
    }
    ExitCode::from(codigo)
}

fn executar(args: &[String]) -> Result<(), String> {
    match args.get(1).map(String::as_str) {
        Some("mcp") => executar_mcp(),
        Some("render") => executar_render(&args[2..]),
        Some("frame") => executar_frame(&args[2..]),
        Some("--version") => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(outro) => Err(format!(
            "subcomando desconhecido: \"{outro}\".\n\n{USO}"
        )),
        None => Err(USO.to_string()),
    }
}

fn executar_mcp() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|erro| format!("não consegui iniciar o runtime assíncrono: {erro}"))?;
    runtime
        .block_on(kdenlive_mcp_plus::mcp::run())
        .map_err(|erro| format!("o servidor MCP terminou com erro: {erro}"))
}

fn executar_render(args: &[String]) -> Result<(), String> {
    let uso_render =
        "uso: kdenlive-mcp-plus render <projeto> <saida> [--preset NOME] [--overwrite]";

    let mut projeto: Option<PathBuf> = None;
    let mut saida: Option<PathBuf> = None;
    let mut preset = kdenlive_mcp_plus::engine::preset::DEFAULT_PRESET.to_string();
    let mut overwrite = false;
    let mut de: Option<i64> = None;
    let mut ate: Option<i64> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--from" | "--to" => {
                let nome = args[i].clone();
                i += 1;
                let valor = args
                    .get(i)
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or_else(|| format!("{nome} precisa de um número inteiro (quadro).\n\n{uso_render}"))?;
                if nome == "--from" { de = Some(valor) } else { ate = Some(valor) }
            }
            "--preset" => {
                i += 1;
                let valor = args
                    .get(i)
                    .ok_or_else(|| format!("--preset precisa de um valor.\n\n{uso_render}"))?;
                preset = valor.clone();
            }
            "--overwrite" => overwrite = true,
            outro if projeto.is_none() => projeto = Some(PathBuf::from(outro)),
            outro if saida.is_none() => saida = Some(PathBuf::from(outro)),
            outro => return Err(format!("argumento inesperado: \"{outro}\".\n\n{uso_render}")),
        }
        i += 1;
    }

    let projeto = projeto.ok_or_else(|| uso_render.to_string())?;
    let saida = saida.ok_or_else(|| uso_render.to_string())?;

    let opts = kdenlive_mcp_plus::engine::preset::resolve_preset(&preset, overwrite)
        .map_err(|erro| erro.to_string())?;

    let intervalo = match (de, ate) {
        (Some(a), Some(b)) => Some((a, b)),
        (None, None) => None,
        _ => return Err(format!("--from e --to andam juntos.\n\n{uso_render}")),
    };
    kdenlive_mcp_plus::engine::render::render_range(&projeto, &saida, &opts, intervalo)
        .map_err(|erro| erro.to_string())
}

fn executar_frame(args: &[String]) -> Result<(), String> {
    let uso_frame = "uso: kdenlive-mcp-plus frame <projeto> <saida.png> <quadro> [--overwrite]";

    let mut projeto: Option<PathBuf> = None;
    let mut saida: Option<PathBuf> = None;
    let mut quadro: Option<i64> = None;
    let mut overwrite = false;

    for arg in args {
        match arg.as_str() {
            "--overwrite" => overwrite = true,
            outro if projeto.is_none() => projeto = Some(PathBuf::from(outro)),
            outro if saida.is_none() => saida = Some(PathBuf::from(outro)),
            outro if quadro.is_none() => {
                quadro = Some(outro.parse::<i64>().map_err(|_| {
                    format!("quadro precisa ser um número inteiro, recebi \"{outro}\".\n\n{uso_frame}")
                })?);
            }
            outro => return Err(format!("argumento inesperado: \"{outro}\".\n\n{uso_frame}")),
        }
    }

    let projeto = projeto.ok_or_else(|| uso_frame.to_string())?;
    let saida = saida.ok_or_else(|| uso_frame.to_string())?;
    let quadro = quadro.ok_or_else(|| uso_frame.to_string())?;

    kdenlive_mcp_plus::engine::render::render_frame(&projeto, &saida, quadro, overwrite)
        .map_err(|erro| erro.to_string())
}
