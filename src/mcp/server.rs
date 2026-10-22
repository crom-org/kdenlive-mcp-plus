//! O servidor MCP em si: exatamente duas ferramentas nesta execução,
//! `server_info` e `render_project` (ver docs/04-ferramentas.md para o
//! status de todas as outras, ainda não implementadas).
//!
//! Sucesso e erro de `render_project` sempre voltam na mesma estrutura
//! (`RenderProjectOutput`, com `ok: bool` e `erro: Option<String>`), para que
//! o agente trate os dois casos do mesmo jeito, em vez de precisar
//! diferenciar um erro de protocolo MCP de um erro de negócio.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::Json;
use rmcp::ServerHandler;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::engine::{preset, worker};

/// Estado do servidor MCP. `render_lock` garante um render por vez neste
/// processo (fila simples, ver docs/02-arquitetura.md).
#[derive(Clone)]
pub struct KdenliveMcpServer {
    render_lock: Arc<Mutex<()>>,
}

impl Default for KdenliveMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl KdenliveMcpServer {
    pub fn new() -> Self {
        Self {
            render_lock: Arc::new(Mutex::new(())),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ServerInfoOutput {
    /// Sempre `true`: esta ferramenta não tem um modo de falha conhecido.
    pub ok: bool,
    pub nome: String,
    pub versao_servidor: String,
    pub versao_libmlt: String,
    pub presets_disponiveis: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderProjectParams {
    /// Caminho do projeto a renderizar (XML do MLT ou `.kdenlive`).
    pub project_path: String,
    /// Caminho de saída; a extensão precisa combinar com o preset escolhido.
    pub output_path: String,
    #[serde(default = "preset_padrao")]
    pub preset: String,
    #[serde(default)]
    pub overwrite: bool,
}

fn preset_padrao() -> String {
    preset::DEFAULT_PRESET.to_string()
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RenderProjectOutput {
    pub ok: bool,
    pub caminho_saida: Option<String>,
    pub tamanho_bytes: Option<u64>,
    pub preset_usado: Option<String>,
    pub duracao_segundos: Option<f64>,
    pub notas: Option<String>,
    pub erro: Option<String>,
}

fn saida_de_erro(mensagem: String) -> RenderProjectOutput {
    RenderProjectOutput {
        ok: false,
        caminho_saida: None,
        tamanho_bytes: None,
        preset_usado: None,
        duracao_segundos: None,
        notas: None,
        erro: Some(mensagem),
    }
}

#[tool_router]
impl KdenliveMcpServer {
    #[tool(
        description = "Report the server name, server version, the linked libmlt version, and \
         the list of available render presets. Takes no parameters. Call this first to check \
         connectivity and to discover valid preset names before calling render_project."
    )]
    pub async fn server_info(&self) -> Json<ServerInfoOutput> {
        Json(ServerInfoOutput {
            ok: true,
            nome: env!("CARGO_PKG_NAME").to_string(),
            versao_servidor: env!("CARGO_PKG_VERSION").to_string(),
            versao_libmlt: env!("MLT_VERSION").to_string(),
            presets_disponiveis: preset::preset_names()
                .into_iter()
                .map(str::to_string)
                .collect(),
        })
    }

    #[tool(
        description = "Render a Kdenlive/MLT XML project to a final video or audio file using \
         the libmlt engine, in an isolated child process (a crash of the render engine never \
         takes down this MCP server). Parameters: project_path (required, path to the .kdenlive \
         or plain MLT XML project file), output_path (required, path to write to; its file \
         extension must match the chosen preset's extension), preset (optional, default \
         \"h264_hq\"; call server_info to list the valid preset names), overwrite (optional, \
         default false; set true to replace an existing output_path, which is otherwise never \
         silently overwritten). Always returns ok:true with output path, size in bytes, preset \
         used and measured duration on success, or ok:false with a Portuguese error message \
         explaining what happened and, when applicable, what to do about it."
    )]
    pub async fn render_project(
        &self,
        Parameters(params): Parameters<RenderProjectParams>,
    ) -> Json<RenderProjectOutput> {
        Json(self.render_project_impl(params).await)
    }
}

impl KdenliveMcpServer {
    async fn render_project_impl(&self, params: RenderProjectParams) -> RenderProjectOutput {
        let project_path = PathBuf::from(&params.project_path);
        let output_path = PathBuf::from(&params.output_path);

        let opts = match preset::resolve_preset(&params.preset, params.overwrite) {
            Ok(opts) => opts,
            Err(erro) => return saida_de_erro(erro.to_string()),
        };

        match output_path.extension().and_then(|e| e.to_str()) {
            Some(extensao) if extensao.eq_ignore_ascii_case(opts.extension) => {}
            Some(extensao) => {
                return saida_de_erro(format!(
                    "a extensão de output_path (\".{extensao}\") não combina com o preset \
                     \"{}\", que espera \".{}\"",
                    params.preset, opts.extension
                ));
            }
            None => {
                return saida_de_erro(format!(
                    "output_path precisa terminar em \".{}\" para o preset \"{}\"",
                    opts.extension, params.preset
                ));
            }
        }

        if !project_path.exists() {
            return saida_de_erro(format!(
                "projeto não encontrado: {}",
                project_path.display()
            ));
        }
        if output_path.exists() && !params.overwrite {
            return saida_de_erro(format!(
                "o arquivo de saída já existe: {} (chame novamente com overwrite=true para \
                 sobrescrever)",
                output_path.display()
            ));
        }

        let _guard = self.render_lock.lock().await;

        if let Err(erro) = worker::render_isolated(
            &project_path,
            &output_path,
            &params.preset,
            params.overwrite,
        )
        .await
        {
            return saida_de_erro(erro.to_string());
        }

        let tamanho_bytes = std::fs::metadata(&output_path).ok().map(|m| m.len());
        let (duracao_segundos, notas) = medir_duracao(&output_path).await;

        RenderProjectOutput {
            ok: true,
            caminho_saida: Some(output_path.display().to_string()),
            tamanho_bytes,
            preset_usado: Some(params.preset),
            duracao_segundos,
            notas,
            erro: None,
        }
    }
}

/// Mede a duração do arquivo renderizado com `ffprobe`. Se o `ffprobe` não
/// existir no PATH, ou falhar, não é um erro de render: o arquivo já foi
/// escrito com sucesso. Retorna `(None, Some(nota))` nesse caso.
async fn medir_duracao(output_path: &Path) -> (Option<f64>, Option<String>) {
    let resultado = tokio::process::Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("csv=p=0")
        .arg(output_path)
        .output()
        .await;

    match resultado {
        Ok(saida) if saida.status.success() => {
            let texto = String::from_utf8_lossy(&saida.stdout);
            match texto.trim().parse::<f64>() {
                Ok(segundos) => (Some(segundos), None),
                Err(_) => (
                    None,
                    Some(
                        "ffprobe rodou, mas a duração retornada não pôde ser interpretada; \
                         veja o arquivo de saída diretamente"
                            .to_string(),
                    ),
                ),
            }
        }
        Ok(_) => (
            None,
            Some("ffprobe rodou mas retornou erro ao medir a duração do arquivo".to_string()),
        ),
        Err(_) => (
            None,
            Some("ffprobe não encontrado no PATH; duração não medida".to_string()),
        ),
    }
}

#[tool_handler]
impl ServerHandler for KdenliveMcpServer {
    // Escrito à mão (em vez de `#[tool_handler(name = ..., version = ...)]`)
    // porque, sem isso, o atalho `Implementation::from_build_env()` do rmcp
    // lê o `CARGO_PKG_NAME`/`CARGO_PKG_VERSION` do próprio crate `rmcp`
    // ("rmcp"/"3.6.0"), não deste servidor; chamando `env!` aqui dentro, ele
    // é expandido no nosso próprio crate e lê o nosso Cargo.toml.
    fn get_info(&self) -> rmcp::model::ServerConfig {
        rmcp::model::ServerConfig::new(rmcp::model::ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "This server edits video projects headlessly using the libmlt engine (the \
                 same engine Kdenlive uses) and saves/reads native Kdenlive/MLT XML projects. \
                 Only two tools are implemented so far: server_info (discover presets and \
                 versions) and render_project (render a project to a final file). Call \
                 server_info first if you need the list of valid preset names."
                    .to_string(),
            )
    }
}
