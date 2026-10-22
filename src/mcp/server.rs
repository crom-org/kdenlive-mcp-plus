//! O servidor MCP: estado (`KdenliveMcpServer`) e a lista de ferramentas
//! expostas (`#[tool_router]`, abaixo). A lógica de cada ferramenta mora
//! nos módulos `crate::mcp::tools::*`, como métodos de
//! `impl KdenliveMcpServer` — aqui ficam só a descrição (em inglês, ver
//! CLAUDE.md) e a assinatura.
//!
//! Toda ferramenta sempre devolve `Json<serde_json::Value>` com `ok: bool`
//! e, em caso de erro, `erro: <mensagem em português>` — nunca como erro de
//! protocolo MCP (decisão D-017), para o agente tratar sucesso e erro do
//! mesmo jeito.

use std::sync::Arc;

use rmcp::Json;
use rmcp::ServerHandler;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::engine::jobs::JobRegistry;
use crate::engine::preset;
use crate::model::Session;
use crate::mcp::params::*;

/// Estado do servidor MCP.
///
/// - `session`: o projeto atualmente aberto, se algum (ver `model::Session`).
/// - `render_lock`: garante um render síncrono por vez (decisão D-016).
/// - `jobs`: registro dos renders assíncronos em andamento (`export_render`
///   com `wait=false`), para `render_progress`/`render_cancel`.
#[derive(Clone)]
pub struct KdenliveMcpServer {
    pub(crate) session: Arc<Mutex<Option<Session>>>,
    pub(crate) render_lock: Arc<Mutex<()>>,
    pub(crate) jobs: JobRegistry,
}

impl Default for KdenliveMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl KdenliveMcpServer {
    pub fn new() -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
            render_lock: Arc::new(Mutex::new(())),
            jobs: JobRegistry::new(),
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

/// Monta uma resposta de erro no formato padrão (`ok: false, erro: ...`).
pub(crate) fn erro(mensagem: impl Into<String>) -> Value {
    json!({ "ok": false, "erro": mensagem.into() })
}

const SEM_PROJETO_ABERTO: &str = "nenhum projeto aberto; chame project_new ou project_open primeiro";

impl KdenliveMcpServer {
    /// Acesso de leitura ao projeto aberto. Devolve o erro padrão se
    /// nenhum projeto estiver aberto.
    pub(crate) async fn with_session<T>(&self, f: impl FnOnce(&Session) -> T) -> Result<T, Value> {
        let guard = self.session.lock().await;
        match guard.as_ref() {
            Some(session) => Ok(f(session)),
            None => Err(erro(SEM_PROJETO_ABERTO)),
        }
    }

    /// Acesso de escrita ao projeto aberto. `f` devolve um
    /// `Result<T, ModelError>`; qualquer `Err` é convertido para a
    /// estrutura de erro padrão. Devolve o erro padrão se nenhum projeto
    /// estiver aberto.
    pub(crate) async fn with_session_mut<T>(
        &self,
        f: impl FnOnce(&mut Session) -> Result<T, crate::model::ModelError>,
    ) -> Result<T, Value> {
        let mut guard = self.session.lock().await;
        match guard.as_mut() {
            Some(session) => f(session).map_err(|e| erro(e.to_string())),
            None => Err(erro(SEM_PROJETO_ABERTO)),
        }
    }
}

#[tool_router]
impl KdenliveMcpServer {
    #[tool(
        description = "Report the server name, server version, the linked libmlt version, and \
         the list of available render presets. Takes no parameters. Call this first to check \
         connectivity and to discover valid preset names before calling export_render."
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

    // -- Projeto --------------------------------------------------------

    #[tool(
        description = "Create a new, empty project in this server's session (replacing any \
         previously open project). Parameters: profile (optional, default \"atsc_1080p_25\"; a \
         real libmlt profile name — width/height/fps/aspect ratio). Returns ok:true with the \
         profile summary, or ok:false with an error listing valid profile names."
    )]
    pub async fn project_new(&self, Parameters(params): Parameters<ProjectNewParams>) -> Json<Value> {
        Json(self.project_new_impl(params).await)
    }

    #[tool(
        description = "Open an existing .kdenlive or plain MLT XML project file into this \
         server's session (replacing any previously open project). Parameter: path (required). \
         Returns ok:true with a summary of what was loaded (profile, track count, bin clip \
         count), or ok:false with a Portuguese error message."
    )]
    pub async fn project_open(&self, Parameters(params): Parameters<ProjectOpenParams>) -> Json<Value> {
        Json(self.project_open_impl(params).await)
    }

    #[tool(
        description = "Alias of project_open, for discoverability when the intent is \
         specifically to import a native Kdenlive project file. Parameter: path (required). \
         Same behavior and return shape as project_open."
    )]
    pub async fn project_import_kdenlive(
        &self,
        Parameters(params): Parameters<ProjectOpenParams>,
    ) -> Json<Value> {
        Json(self.project_open_impl(params).await)
    }

    #[tool(
        description = "Save the current session's project to disk as MLT XML / .kdenlive. \
         Parameter: path (optional; if omitted, saves to the path the project was opened from — \
         an error if the project was never saved before and no path is given). Returns ok:true \
         with the path written, or ok:false with an error (e.g. no project open)."
    )]
    pub async fn project_save(&self, Parameters(params): Parameters<ProjectSaveParams>) -> Json<Value> {
        Json(self.project_save_impl(params).await)
    }

    #[tool(
        description = "Report metadata about the currently open project: video profile summary, \
         track count and kind, bin clip count, and total timeline duration in frames. Takes no \
         parameters. Returns ok:false if no project is open."
    )]
    pub async fn project_get_info(&self) -> Json<Value> {
        Json(self.project_get_info_impl().await)
    }

    #[tool(
        description = "Check the currently open project for common problems: bin clips whose \
         media file no longer exists on disk, and tracks with zero clips. Takes no parameters. \
         Returns ok:true with a list of problems found (empty list means nothing was found), or \
         ok:false if no project is open. This performs file-existence checks only; it never \
         calls the libmlt engine."
    )]
    pub async fn project_validate(&self) -> Json<Value> {
        Json(self.project_validate_impl().await)
    }

    // -- Bin --------------------------------------------------------------

    #[tool(
        description = "Import a media file into the current project's bin. Requires an open \
         project (project_new/project_open). Parameters: path (required, path to the media \
         file), name (optional display name; defaults to the file name). Probes the file's \
         duration with ffprobe on a best-effort basis (never touches the libmlt engine \
         directly). Returns ok:true with the new clip's id, or ok:false with an error."
    )]
    pub async fn bin_import_clip(
        &self,
        Parameters(params): Parameters<BinImportClipParams>,
    ) -> Json<Value> {
        Json(self.bin_import_clip_impl(params).await)
    }

    #[tool(
        description = "List every clip in the current project's bin, with id, display name, \
         media path and frame range. Takes no parameters. Returns ok:false if no project is \
         open."
    )]
    pub async fn bin_list_clips(&self) -> Json<Value> {
        Json(self.bin_list_clips_impl().await)
    }

    #[tool(
        description = "Report detailed properties of one bin clip: display name, media path, \
         frame range, and whether the media file still exists on disk. Parameter: clip_id \
         (required). Returns ok:false if the clip id is not found."
    )]
    pub async fn bin_get_clip_details(
        &self,
        Parameters(params): Parameters<BinClipIdParams>,
    ) -> Json<Value> {
        Json(self.bin_get_clip_details_impl(params).await)
    }

    #[tool(
        description = "Remove a clip from the project bin by id. If the clip is still placed on \
         a timeline track, only the bin entry is removed (the underlying producer stays, since \
         the track still references it). Parameter: clip_id (required). Returns ok:false if the \
         clip id is not found in the bin."
    )]
    pub async fn bin_remove_clip(&self, Parameters(params): Parameters<BinClipIdParams>) -> Json<Value> {
        Json(self.bin_remove_clip_impl(params).await)
    }

    // -- Timeline -----------------------------------------------------------

    #[tool(
        description = "Add a new, empty track to the current project's timeline. Parameter: \
         kind (required, either \"video\" or \"audio\"). Returns ok:true with the new track's \
         id, or ok:false with an error."
    )]
    pub async fn timeline_add_track(
        &self,
        Parameters(params): Parameters<TimelineAddTrackParams>,
    ) -> Json<Value> {
        Json(self.timeline_add_track_impl(params).await)
    }

    #[tool(
        description = "Remove a track and all its clips from the timeline. The underlying bin \
         producers are not deleted. Parameter: track_id (required). Returns ok:false if the \
         track id is not found."
    )]
    pub async fn timeline_remove_track(
        &self,
        Parameters(params): Parameters<TimelineRemoveTrackParams>,
    ) -> Json<Value> {
        Json(self.timeline_remove_track_impl(params).await)
    }

    #[tool(
        description = "Place a bin clip onto a specific track at a given time position (in \
         frames). If position is past the current end of the track, a gap (blank) is created to \
         fill it; placing a clip where it would overlap existing content on that track is an \
         error in this phase (no ripple/shift insert yet). Parameters: track_id, producer_id \
         (the bin clip's id), in_frame, out_frame, position (all required). Returns ok:true with \
         a clip reference string (\"<track_id>:<index>\", this server's own convention, used by \
         other timeline tools), or ok:false with an error."
    )]
    pub async fn timeline_add_clip(
        &self,
        Parameters(params): Parameters<TimelineAddClipParams>,
    ) -> Json<Value> {
        Json(self.timeline_add_clip_impl(params).await)
    }

    #[tool(
        description = "List every track in the current project's timeline, in order, with kind \
         (video/audio), clip count and total length in frames. Takes no parameters. Returns \
         ok:false if no project is open."
    )]
    pub async fn timeline_list(&self) -> Json<Value> {
        Json(self.timeline_list_impl().await)
    }

    #[tool(
        description = "Report comprehensive timeline info: every track with its clips (producer \
         id, in/out, position), total project duration in frames, and the video profile summary. \
         Takes no parameters. Returns ok:false if no project is open."
    )]
    pub async fn timeline_get_info(&self) -> Json<Value> {
        Json(self.timeline_get_info_impl().await)
    }

    // -- Exportação e render --------------------------------------------------

    #[tool(
        description = "Generate the MLT/Kdenlive XML of the current session's project. \
         Parameters: path (optional; if given, writes the XML to that file; if omitted, returns \
         the XML text directly in the response instead). This does not change the session's \
         saved-to path (use project_save for that). Returns ok:false if no project is open."
    )]
    pub async fn export_xml(&self, Parameters(params): Parameters<ExportXmlParams>) -> Json<Value> {
        Json(self.export_xml_impl(params).await)
    }

    #[tool(
        description = "List the available render presets by name only (for the full \
         description of each, see server_info or the project documentation). Takes no \
         parameters."
    )]
    pub async fn export_list_presets(&self) -> Json<Value> {
        Json(json!({ "ok": true, "presets": preset::preset_names() }))
    }

    #[tool(
        description = "Render a project to a final video or audio file using the libmlt engine, \
         in an isolated child process (a crash of the render engine never takes down this MCP \
         server). Parameters: project_path (optional; if omitted, renders the project currently \
         open in this server's session — an error if none is open), output_path (required; its \
         file extension must match the chosen preset's extension), preset (optional, default \
         \"h264_hq\"; call server_info to list valid preset names), overwrite (optional, default \
         false), wait (optional, default true; set false to return immediately with a job_id and \
         track it with render_progress/render_cancel instead of blocking this call until done). \
         With wait=true, returns ok:true with output path, size in bytes, preset used and \
         measured duration on success. With wait=false, returns ok:true with a job_id \
         immediately. Either way, ok:false carries a Portuguese error message explaining what \
         happened and, when applicable, what to do about it. Never silently overwrites an \
         existing file."
    )]
    pub async fn export_render(
        &self,
        Parameters(params): Parameters<ExportRenderParams>,
    ) -> Json<Value> {
        Json(self.export_render_impl(params).await)
    }

    #[tool(
        description = "Query the status and progress percentage of a render job started with \
         export_render(wait=false). Parameter: job_id (required). Returns ok:true with status \
         (\"em andamento\"/\"concluído\"/\"falhou\"/\"cancelado\") and a percent (0-100, \
         best-effort, parsed from the render engine's own progress output), or ok:false if the \
         job id is not known to this server (e.g. after a restart — job tracking does not \
         persist)."
    )]
    pub async fn render_progress(&self, Parameters(params): Parameters<JobIdParams>) -> Json<Value> {
        Json(self.render_progress_impl(params).await)
    }

    #[tool(
        description = "Cancel a render job started with export_render(wait=false) by actually \
         terminating its child process (unlike the original reference project's \
         render_queue_stop, which only marked internal state without stopping anything — see \
         this project's own documentation). Parameter: job_id (required). Returns ok:false if \
         the job id is not found or is not currently running."
    )]
    pub async fn render_cancel(&self, Parameters(params): Parameters<JobIdParams>) -> Json<Value> {
        Json(self.render_cancel_impl(params).await)
    }

    #[tool(
        description = "Render a single frame of a project as a PNG image, in an isolated child \
         process. Parameters: project_path (optional; defaults to the project open in this \
         server's session), frame (required, 0-based frame number), output_path (required, path \
         to write the PNG to), overwrite (optional, default false). Returns ok:true with the \
         output path on success, or ok:false with an error."
    )]
    pub async fn preview_frame(
        &self,
        Parameters(params): Parameters<PreviewFrameParams>,
    ) -> Json<Value> {
        Json(self.preview_frame_impl(params).await)
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
                 Call server_info to discover render presets and versions. Call project_new or \
                 project_open before any bin_*/timeline_*/export_* tool — they act on the \
                 project currently open in this server's session. Call project_save (or \
                 export_xml) to persist your edits to disk."
                    .to_string(),
            )
    }
}
