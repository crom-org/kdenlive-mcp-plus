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

/// Separa uma referência de clipe `"<track_id>:<índice>"` (convenção deste
/// servidor, ver `model::document`) em suas duas partes.
pub(crate) fn parse_clip_ref(clip_ref: &str) -> Result<(String, usize), Value> {
    let (track_id, indice) = clip_ref.rsplit_once(':').ok_or_else(|| {
        erro(format!(
            "clip_ref inválido: \"{clip_ref}\". Esperava o formato \"<track_id>:<índice>\""
        ))
    })?;
    let indice = indice.parse::<usize>().map_err(|_| {
        erro(format!(
            "clip_ref inválido: \"{clip_ref}\". O índice (\"{indice}\") precisa ser um número"
        ))
    })?;
    Ok((track_id.to_string(), indice))
}

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

    // -- Trilha (Fase 2) --------------------------------------------------

    #[tool(
        description = "Report detailed info for a single track: kind (video/audio), name, \
         locked/hidden/muted state, clip count and length in frames. Parameter: track_id \
         (required)."
    )]
    pub async fn track_get_info(&self, Parameters(params): Parameters<TrackIdParams>) -> Json<Value> {
        Json(self.track_get_info_impl(params).await)
    }

    #[tool(description = "Rename a track. Parameters: track_id, name (both required).")]
    pub async fn track_set_name(&self, Parameters(params): Parameters<TrackSetNameParams>) -> Json<Value> {
        Json(self.track_set_name_impl(params).await)
    }

    #[tool(
        description = "Lock/unlock a track (editing-only concept; does not affect rendering). \
         Parameters: track_id, value (both required)."
    )]
    pub async fn track_set_locked(&self, Parameters(params): Parameters<TrackBoolParams>) -> Json<Value> {
        Json(self.track_set_locked_impl(params).await)
    }

    #[tool(
        description = "Hide/show a track's video, independently of its mute state. Parameters: \
         track_id, value (both required)."
    )]
    pub async fn track_set_hidden(&self, Parameters(params): Parameters<TrackBoolParams>) -> Json<Value> {
        Json(self.track_set_hidden_impl(params).await)
    }

    #[tool(
        description = "Mute/unmute a track's audio, independently of its hidden state. \
         Parameters: track_id, value (both required)."
    )]
    pub async fn track_set_muted(&self, Parameters(params): Parameters<TrackBoolParams>) -> Json<Value> {
        Json(self.track_set_muted_impl(params).await)
    }

    #[tool(
        description = "Move a track up one position (swap with the previous one). Parameter: \
         track_id (required). Returns ok:false if already the first track."
    )]
    pub async fn track_move_up(&self, Parameters(params): Parameters<TrackIdParams>) -> Json<Value> {
        Json(self.track_move_up_impl(params).await)
    }

    #[tool(
        description = "Move a track down one position (swap with the next one). Parameter: \
         track_id (required). Returns ok:false if already the last track."
    )]
    pub async fn track_move_down(&self, Parameters(params): Parameters<TrackIdParams>) -> Json<Value> {
        Json(self.track_move_down_impl(params).await)
    }

    // -- Clipe da timeline (Fase 2) -----------------------------------------

    #[tool(
        description = "Report timeline clip properties: producer id, in/out, current speed and \
         whether it is reversed, and the list of filters attached to it. Parameter: clip_ref \
         (required, \"<track_id>:<index>\")."
    )]
    pub async fn clip_get_properties(&self, Parameters(params): Parameters<ClipRefParams>) -> Json<Value> {
        Json(self.clip_get_properties_impl(params).await)
    }

    #[tool(
        description = "Set playback speed of a timeline clip (1.0=normal, 2.0=2x, 0.5=half, \
         negative=reverse), by wrapping the clip's producer in a libmlt \"timewarp\" producer \
         (verified to work correctly with real media files in this project's own tests; \
         behavior with synthetic color/tone sources is not guaranteed). Parameters: clip_ref, \
         speed (both required). This only affects this specific clip instance, not the shared \
         bin producer or other placements of it."
    )]
    pub async fn clip_set_speed(&self, Parameters(params): Parameters<ClipSetSpeedParams>) -> Json<Value> {
        Json(self.clip_set_speed_impl(params).await)
    }

    #[tool(
        description = "Reverse playback direction of a timeline clip (toggles the sign of its \
         current speed; same underlying mechanism as clip_set_speed). Parameter: clip_ref \
         (required)."
    )]
    pub async fn clip_reverse(&self, Parameters(params): Parameters<ClipRefParams>) -> Json<Value> {
        Json(self.clip_reverse_impl(params).await)
    }

    #[tool(
        description = "Set opacity of a timeline clip (0.0 transparent .. 1.0 opaque), via the \
         libmlt \"brightness\" filter's alpha parameter, attached to this specific clip (not \
         the shared producer). Parameters: clip_ref, opacity (both required)."
    )]
    pub async fn clip_set_opacity(&self, Parameters(params): Parameters<ClipSetOpacityParams>) -> Json<Value> {
        Json(self.clip_set_opacity_impl(params).await)
    }

    #[tool(
        description = "Set a color label on a bin clip (hex like \"#ff0000\" or a name). This \
         project's own convention, not confirmed to match what real Kdenlive stores. Parameters: \
         clip_id (the bin clip's id, required), color (required)."
    )]
    pub async fn clip_set_color(&self, Parameters(params): Parameters<BinClipColorParams>) -> Json<Value> {
        Json(self.clip_set_color_impl(params).await)
    }

    #[tool(
        description = "Add or update a free-text note on a bin clip. This project's own \
         convention, not confirmed to match what real Kdenlive stores. Parameters: clip_id (the \
         bin clip's id, required), note (required)."
    )]
    pub async fn clip_add_note(&self, Parameters(params): Parameters<BinClipNoteParams>) -> Json<Value> {
        Json(self.clip_add_note_impl(params).await)
    }

    // -- Sessão: undo/redo (Fase 2) ------------------------------------------

    #[tool(
        description = "Undo the most recent change(s) to the open project (snapshot-based, up to \
         50 history entries). Parameter: steps (optional, default 1). Unifies the original \
         catalog's session_undo/session_undo_step into one tool."
    )]
    pub async fn session_undo(&self, Parameters(params): Parameters<SessionStepsParams>) -> Json<Value> {
        Json(self.session_undo_impl(params).await)
    }

    #[tool(
        description = "Redo the most recently undone change(s). Parameter: steps (optional, \
         default 1). Unifies the original catalog's session_redo/session_redo_step into one \
         tool. Any new edit after an undo clears the redo stack."
    )]
    pub async fn session_redo(&self, Parameters(params): Parameters<SessionStepsParams>) -> Json<Value> {
        Json(self.session_redo_impl(params).await)
    }

    #[tool(
        description = "Report session state: whether a project is open, its modified flag, undo \
         history depth and redo steps available. Takes no parameters."
    )]
    pub async fn session_status(&self) -> Json<Value> {
        Json(self.session_status_impl().await)
    }

    #[tool(
        description = "List all undo history entries, oldest first, with a short description of \
         each change. Takes no parameters. Returns ok:false if no project is open."
    )]
    pub async fn session_history(&self) -> Json<Value> {
        Json(self.session_history_impl().await)
    }

    // -- Filtros (Fase 2) -----------------------------------------------------

    #[tool(
        description = "Attach a video/audio effect (libmlt filter service, e.g. \"brightness\", \
         \"qtblend\") to a timeline clip. Call filter_list_available for the full list of \
         services this libmlt build exposes. Parameters: clip_ref, service (both required). \
         Returns ok:true with the new filter's id (unique within this clip, used by \
         filter_remove/filter_set_param)."
    )]
    pub async fn filter_add(&self, Parameters(params): Parameters<FilterAddParams>) -> Json<Value> {
        Json(self.filter_add_impl(params).await)
    }

    #[tool(description = "Detach a filter from a clip. Parameters: clip_ref, filter_id (both required).")]
    pub async fn filter_remove(&self, Parameters(params): Parameters<FilterRemoveParams>) -> Json<Value> {
        Json(self.filter_remove_impl(params).await)
    }

    #[tool(
        description = "Update a single parameter on an active filter. Parameters: clip_ref, \
         filter_id, param, value (all required, value as a string)."
    )]
    pub async fn filter_set_param(&self, Parameters(params): Parameters<FilterSetParamParams>) -> Json<Value> {
        Json(self.filter_set_param_impl(params).await)
    }

    #[tool(description = "List all active filters on a specific timeline clip. Parameter: clip_ref (required).")]
    pub async fn filter_list(&self, Parameters(params): Parameters<ClipRefParams>) -> Json<Value> {
        Json(self.filter_list_impl(params).await)
    }

    #[tool(
        description = "Discover all filter services the libmlt runtime exposes (by running \
         \"melt -query filters\", a plain subprocess call — never the libmlt engine directly \
         from this server). Takes no parameters."
    )]
    pub async fn filter_list_available(&self) -> Json<Value> {
        Json(self.filter_list_available_impl().await)
    }

    // -- Transições (Fase 2) ----------------------------------------------------

    #[tool(
        description = "Create a blend transition (libmlt transition service, e.g. \"luma\" for \
         crossfade/wipe) between two timeline tracks. Parameters: service, track_a, track_b (all \
         required, track ids as returned by timeline_add_track/timeline_list)."
    )]
    pub async fn transition_add(&self, Parameters(params): Parameters<TransitionAddParams>) -> Json<Value> {
        Json(self.transition_add_impl(params).await)
    }

    #[tool(description = "Delete a transition by its id. Parameter: transition_id (required).")]
    pub async fn transition_remove(&self, Parameters(params): Parameters<TransitionIdParams>) -> Json<Value> {
        Json(self.transition_remove_impl(params).await)
    }

    #[tool(
        description = "Modify a parameter on an existing transition. Parameters: transition_id, \
         param, value (all required, value as a string)."
    )]
    pub async fn transition_set(&self, Parameters(params): Parameters<TransitionSetParamParams>) -> Json<Value> {
        Json(self.transition_set_impl(params).await)
    }

    #[tool(description = "List all transitions on the timeline. Takes no parameters.")]
    pub async fn transition_list(&self) -> Json<Value> {
        Json(self.transition_list_impl().await)
    }

    // -- Guias e marcadores (Fase 2) ---------------------------------------------

    #[tool(
        description = "Place a navigation guide on the project timeline. This project's own \
         convention (not confirmed to match real Kdenlive's internal guide format — see this \
         project's documentation). Parameters: position (frames, required), comment (required)."
    )]
    pub async fn guide_add(&self, Parameters(params): Parameters<GuideAddParams>) -> Json<Value> {
        Json(self.guide_add_impl(params).await)
    }

    #[tool(description = "Delete a guide by id. Parameter: guide_id (required).")]
    pub async fn guide_remove(&self, Parameters(params): Parameters<GuideIdParams>) -> Json<Value> {
        Json(self.guide_remove_impl(params).await)
    }

    #[tool(description = "List all guide markers on the timeline. Takes no parameters.")]
    pub async fn guide_list(&self) -> Json<Value> {
        Json(self.guide_list_impl().await)
    }

    #[tool(description = "Get a single guide by id. Parameter: guide_id (required).")]
    pub async fn guide_get(&self, Parameters(params): Parameters<GuideIdParams>) -> Json<Value> {
        Json(self.guide_get_impl(params).await)
    }

    #[tool(
        description = "Update a guide's position and/or comment. Parameters: guide_id \
         (required), position (optional), comment (optional; at least one of the two should be \
         given)."
    )]
    pub async fn guide_update(&self, Parameters(params): Parameters<GuideUpdateParams>) -> Json<Value> {
        Json(self.guide_update_impl(params).await)
    }

    #[tool(
        description = "Add a timeline marker, or a clip marker if clip_ref is given (markers are \
         a separate concept from guides in this project's model, following the original \
         catalog). Parameters: position (frames, required), comment (required), clip_ref \
         (optional, \"<track_id>:<index>\")."
    )]
    pub async fn marker_add(&self, Parameters(params): Parameters<MarkerAddParams>) -> Json<Value> {
        Json(self.marker_add_impl(params).await)
    }

    #[tool(description = "List all markers (timeline and clip). Takes no parameters.")]
    pub async fn marker_list(&self) -> Json<Value> {
        Json(self.marker_list_impl().await)
    }

    #[tool(description = "Remove a marker by id. Parameter: marker_id (required).")]
    pub async fn marker_remove(&self, Parameters(params): Parameters<MarkerIdParams>) -> Json<Value> {
        Json(self.marker_remove_impl(params).await)
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
