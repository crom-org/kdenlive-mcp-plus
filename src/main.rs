//! CLI: `mcp` inicia o servidor MCP via stdio; `render` renderiza direto,
//! sem MCP (é o mesmo subcomando que o servidor MCP invoca como processo
//! filho isolado, ver `engine::worker`); `--version` imprime a versão.
//!
//! Erros vão para stderr, com código de saída diferente de zero. No modo
//! `mcp`, o stdout carrega só mensagens do protocolo MCP.

use std::path::PathBuf;
use std::process::ExitCode;

const USO: &str = "uso: kdenlive-mcp-plus <mcp|render|--version>\n\
     \n\
     \x20 mcp                                                    inicia o servidor MCP via stdio\n\
     \x20 render <projeto> <saida> [--preset NOME] [--overwrite]  renderiza direto, sem MCP\n\
     \x20 --version                                               mostra nome e versão";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match executar(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(mensagem) => {
            eprintln!("erro: {mensagem}");
            ExitCode::FAILURE
        }
    }
}

fn executar(args: &[String]) -> Result<(), String> {
    match args.get(1).map(String::as_str) {
        Some("mcp") => executar_mcp(),
        Some("render") => executar_render(&args[2..]),
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

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
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

    kdenlive_mcp_plus::engine::render::render(&projeto, &saida, &opts).map_err(|erro| erro.to_string())
}
