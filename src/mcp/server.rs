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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

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
use crate::engine::queue::RenderQueue;
use crate::model::Session;
use crate::mcp::params::*;

/// Estado do servidor MCP.
///
/// - `session`: o projeto atualmente aberto, se algum (ver `model::Session`).
/// - `render_lock`: garante um render de verdade por vez (decisão D-016) —
///   compartilhado entre `export_render` síncrono, jobs assíncronos
///   (`JobRegistry`) e a fila (`RenderQueue`).
/// - `jobs`: registro dos renders assíncronos avulsos (`export_render` com
///   `wait=false`), para `render_progress`/`render_cancel`.
/// - `queue`: a fila de render (`render_queue_*`), construída por cima do
///   mesmo `JobRegistry`/`render_lock`.
#[derive(Clone)]
pub struct KdenliveMcpServer {
    pub(crate) session: Arc<Mutex<Option<Session>>>,
    pub(crate) render_lock: Arc<Mutex<()>>,
    pub(crate) jobs: JobRegistry,
    pub(crate) queue: RenderQueue,
    /// Se `true` (variável de ambiente `KDENLIVE_MCP_AUTOSAVE=1`, ou a
    /// ferramenta `kdenlive_live`), cada edição bem-sucedida regrava o
    /// projeto no seu caminho — para poder acompanhá-lo no Kdenlive sem
    /// pedir `project_save` a cada passo. Só vale depois de o projeto ter um
    /// caminho. Compartilhado entre os clones do servidor (D-060).
    pub(crate) autosave: Arc<AtomicBool>,
    /// Com `KDENLIVE_MCP_AUTORELOAD=1` (ou `kdenlive_live`), cada gravação do
    /// projeto pede ao Kdenlive aberto que o recarregue (ver `mcp::live`). O
    /// canal leva o caminho gravado à tarefa de fundo; `None` = desligado.
    pub(crate) recarga: Arc<StdMutex<Option<tokio::sync::mpsc::UnboundedSender<PathBuf>>>>,
}

impl Default for KdenliveMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl KdenliveMcpServer {
    pub fn new() -> Self {
        let ligada = |nome: &str| matches!(std::env::var(nome).as_deref(), Ok("1" | "true" | "TRUE"));
        // sem runtime (ex.: fora do tokio) não há onde rodar a tarefa de fundo
        let recarga = if ligada("KDENLIVE_MCP_AUTORELOAD") {
            tokio::runtime::Handle::try_current().ok().map(|h| super::live::iniciar(&h))
        } else {
            None
        };
        Self {
            session: Arc::new(Mutex::new(None)),
            render_lock: Arc::new(Mutex::new(())),
            jobs: JobRegistry::new(),
            queue: RenderQueue::new(),
            autosave: Arc::new(AtomicBool::new(ligada("KDENLIVE_MCP_AUTOSAVE") || recarga.is_some())),
            recarga: Arc::new(StdMutex::new(recarga)),
        }
    }

    /// Pede ao Kdenlive aberto que recarregue `caminho` (se a recarga
    /// automática estiver ligada). Não espera nem falha: é só um aviso à
    /// tarefa de fundo.
    pub(crate) fn pedir_recarga(&self, caminho: &Path) {
        if let Ok(guard) = self.recarga.lock()
            && let Some(tx) = guard.as_ref()
        {
            let _ = tx.send(caminho.to_path_buf());
        }
    }

    /// `true` se a recarga automática do Kdenlive está ligada.
    pub(crate) fn recarga_ligada(&self) -> bool {
        self.recarga.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Liga a recarga automática (inicia a tarefa de fundo, se ainda não houver).
    pub(crate) fn ligar_recarga(&self) {
        if let Ok(mut guard) = self.recarga.lock()
            && guard.is_none()
            && let Ok(h) = tokio::runtime::Handle::try_current()
        {
            *guard = Some(super::live::iniciar(&h));
        }
    }

    /// Desliga a recarga automática (soltar o canal encerra a tarefa de fundo).
    pub(crate) fn desligar_recarga(&self) {
        if let Ok(mut guard) = self.recarga.lock() {
            *guard = None;
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
    /// Cada edição regrava o projeto (KDENLIVE_MCP_AUTOSAVE ou kdenlive_live).
    pub autosave: bool,
    /// O Kdenlive aberto recarrega a cada gravação (KDENLIVE_MCP_AUTORELOAD ou kdenlive_live).
    pub autoreload: bool,
}

/// Resposta JSON de uma ferramenta: sempre um objeto (`{"ok": ...}`). Existe
/// só para o `outputSchema` anunciado no `tools/list` dizer `"type": "object"`,
/// como a especificação do MCP exige — com `serde_json::Value` o schema saía sem
/// `type`, e o Claude Desktop (sessões do Cowork e do Code) recusava a lista
/// inteira de ferramentas.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct Resposta(pub Value);

impl From<Value> for Resposta {
    fn from(valor: Value) -> Self {
        Self(valor)
    }
}

impl JsonSchema for Resposta {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Resposta".into()
    }

    fn json_schema(_gerador: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "object", "additionalProperties": true })
    }
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
            Some(session) => {
                let resultado = f(session).map_err(|e| erro(e.to_string()));
                if resultado.is_ok() && self.autosave.load(Ordering::SeqCst) {
                    match session.autosave() {
                        Ok(Some(gravado)) => self.pedir_recarga(&gravado),
                        Ok(None) => {}
                        // A edição já foi aplicada; falhar o autosave não deve
                        // virar erro da ferramenta (o agente pode chamar project_save).
                        Err(e) => eprintln!("kdenlive-mcp-plus: autosave falhou: {e}"),
                    }
                }
                resultado
            }
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
            autosave: self.autosave.load(Ordering::SeqCst),
            autoreload: self.recarga_ligada(),
        })
    }

    // -- Projeto --------------------------------------------------------

    #[tool(
        description = "Create a new project in this server's session (replacing any previously \
         open project). Parameters: profile (optional, default \"atsc_1080p_25\"; a real libmlt \
         profile name — width/height/fps/aspect ratio), default_tracks (optional, default true: \
         start with one video track and one audio track, returned in new_tracks, because Kdenlive \
         crashes when it opens a project that has no tracks at all). Returns ok:true with the \
         project summary, or ok:false with an error listing valid profile names."
    )]
    pub async fn project_new(&self, Parameters(params): Parameters<ProjectNewParams>) -> Json<Resposta> {
        Json(self.project_new_impl(params).await.into())
    }

    #[tool(
        description = "Open an existing .kdenlive or plain MLT XML project file into this \
         server's session (replacing any previously open project). Parameter: path (required). \
         Returns ok:true with a summary of what was loaded (profile, track count, bin clip \
         count), or ok:false with a Portuguese error message."
    )]
    pub async fn project_open(&self, Parameters(params): Parameters<ProjectOpenParams>) -> Json<Resposta> {
        Json(self.project_open_impl(params).await.into())
    }

    #[tool(
        description = "Alias of project_open, for discoverability when the intent is \
         specifically to import a native Kdenlive project file. Parameter: path (required). \
         Same behavior and return shape as project_open."
    )]
    pub async fn project_import_kdenlive(
        &self,
        Parameters(params): Parameters<ProjectOpenParams>,
    ) -> Json<Resposta> {
        Json(self.project_open_impl(params).await.into())
    }

    #[tool(
        description = "Save the current session's project to disk as MLT XML / .kdenlive. \
         Parameter: path (optional; if omitted, saves to the path the project was opened from — \
         an error if the project was never saved before and no path is given). Returns ok:true \
         with the path written, or ok:false with an error (e.g. no project open)."
    )]
    pub async fn project_save(&self, Parameters(params): Parameters<ProjectSaveParams>) -> Json<Resposta> {
        Json(self.project_save_impl(params).await.into())
    }

    #[tool(
        description = "Report metadata about the currently open project: video profile summary, \
         track count and kind, bin clip count, and total timeline duration in frames. Takes no \
         parameters. Returns ok:false if no project is open."
    )]
    pub async fn project_get_info(&self) -> Json<Resposta> {
        Json(self.project_get_info_impl().await.into())
    }

    #[tool(
        description = "Check the currently open project for common problems: bin clips whose \
         media file no longer exists on disk, and tracks with zero clips. Takes no parameters. \
         Returns ok:true with a list of problems found (empty list means nothing was found), or \
         ok:false if no project is open. This performs file-existence checks only; it never \
         calls the libmlt engine."
    )]
    pub async fn project_validate(&self) -> Json<Resposta> {
        Json(self.project_validate_impl().await.into())
    }

    // -- Bin --------------------------------------------------------------

    #[tool(
        description = "Import a media file into the current project's bin. Requires an open \
         project (project_new/project_open). Parameters: path (required, path to the media \
         file), name (optional display name; defaults to the file name), duration_frames \
         (optional, still images only: png/jpg/webp/svg/bmp/tif become a native Kdenlive image \
         clip of this length, default 5 seconds; an image can also be stretched on the timeline \
         past it). Probes video/audio duration with ffprobe on a best-effort basis (never \
         touches the libmlt engine directly). Returns ok:true with the new clip's id, or \
         ok:false with an error."
    )]
    pub async fn bin_import_clip(
        &self,
        Parameters(params): Parameters<BinImportClipParams>,
    ) -> Json<Resposta> {
        Json(self.bin_import_clip_impl(params).await.into())
    }

    #[tool(
        description = "List every clip in the current project's bin, with id, display name, \
         media path and frame range. Takes no parameters. Returns ok:false if no project is \
         open."
    )]
    pub async fn bin_list_clips(&self) -> Json<Resposta> {
        Json(self.bin_list_clips_impl().await.into())
    }

    #[tool(
        description = "Report detailed properties of one bin clip: display name, media path, \
         frame range, and whether the media file still exists on disk. Parameter: clip_id \
         (required). Returns ok:false if the clip id is not found."
    )]
    pub async fn bin_get_clip_details(
        &self,
        Parameters(params): Parameters<BinClipIdParams>,
    ) -> Json<Resposta> {
        Json(self.bin_get_clip_details_impl(params).await.into())
    }

    #[tool(
        description = "Remove a clip from the project bin by id. If the clip is still placed on \
         a timeline track, only the bin entry is removed (the underlying producer stays, since \
         the track still references it). Parameter: clip_id (required). Returns ok:false if the \
         clip id is not found in the bin."
    )]
    pub async fn bin_remove_clip(&self, Parameters(params): Parameters<BinClipIdParams>) -> Json<Resposta> {
        Json(self.bin_remove_clip_impl(params).await.into())
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
    ) -> Json<Resposta> {
        Json(self.timeline_add_track_impl(params).await.into())
    }

    #[tool(
        description = "Remove a track and all its clips from the timeline. The underlying bin \
         producers are not deleted. Parameter: track_id (required). Returns ok:false if the \
         track id is not found."
    )]
    pub async fn timeline_remove_track(
        &self,
        Parameters(params): Parameters<TimelineRemoveTrackParams>,
    ) -> Json<Resposta> {
        Json(self.timeline_remove_track_impl(params).await.into())
    }

    #[tool(
        description = "Place a bin clip onto a specific track at a given time position (in \
         frames). If position is past the current end of the track, a gap (blank) is created to \
         fill it; placing a clip where it would overlap existing content on that track is an \
         error in this phase (no ripple/shift insert yet). Parameters: track_id, producer_id \
         (the bin clip's id), in_frame, out_frame, position (all required), audio_track_id \
         (optional: for a video clip with sound, an AUDIO track where its sound goes, linked to \
         the video as a Kdenlive video+audio group — a video track never plays the clip's audio, \
         so without this the clip is silent). Returns ok:true with a clip reference string \
         (\"<track_id>:<index>\", this server's own convention, used by other timeline tools) and \
         audio_clip_ref when audio_track_id was given, or ok:false with an error."
    )]
    pub async fn timeline_add_clip(
        &self,
        Parameters(params): Parameters<TimelineAddClipParams>,
    ) -> Json<Resposta> {
        Json(self.timeline_add_clip_impl(params).await.into())
    }

    #[tool(
        description = "List every track in the current project's timeline, in order, with kind \
         (video/audio), clip count and total length in frames. Takes no parameters. Returns \
         ok:false if no project is open."
    )]
    pub async fn timeline_list(&self) -> Json<Resposta> {
        Json(self.timeline_list_impl().await.into())
    }

    #[tool(
        description = "Report comprehensive timeline info: every track with its clips (producer \
         id, in/out, position), total project duration in frames, and the video profile summary. \
         Takes no parameters. Returns ok:false if no project is open."
    )]
    pub async fn timeline_get_info(&self) -> Json<Resposta> {
        Json(self.timeline_get_info_impl().await.into())
    }

    // -- Exportação e render --------------------------------------------------

    #[tool(
        description = "Generate the MLT/Kdenlive XML of the current session's project. \
         Parameters: path (optional; if given, writes the XML to that file; if omitted, returns \
         the XML text directly in the response instead). This does not change the session's \
         saved-to path (use project_save for that). Returns ok:false if no project is open."
    )]
    pub async fn export_xml(&self, Parameters(params): Parameters<ExportXmlParams>) -> Json<Resposta> {
        Json(self.export_xml_impl(params).await.into())
    }

    #[tool(
        description = "List the available render presets by name only (for the full \
         description of each, see server_info or the project documentation). Takes no \
         parameters."
    )]
    pub async fn export_list_presets(&self) -> Json<Resposta> {
        Json(json!({ "ok": true, "presets": preset::preset_names() }).into())
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
    ) -> Json<Resposta> {
        Json(self.export_render_impl(params).await.into())
    }

    #[tool(
        description = "Query the status and progress percentage of a render job started with \
         export_render(wait=false). Parameter: job_id (required). Returns ok:true with status \
         (\"em andamento\"/\"concluído\"/\"falhou\"/\"cancelado\") and a percent (0-100, \
         best-effort, parsed from the render engine's own progress output), or ok:false if the \
         job id is not known to this server (e.g. after a restart — job tracking does not \
         persist)."
    )]
    pub async fn render_progress(&self, Parameters(params): Parameters<JobIdParams>) -> Json<Resposta> {
        Json(self.render_progress_impl(params).await.into())
    }

    #[tool(
        description = "Cancel a render job started with export_render(wait=false) by actually \
         terminating its child process (unlike the original reference project's \
         render_queue_stop, which only marked internal state without stopping anything — see \
         this project's own documentation). Parameter: job_id (required). Returns ok:false if \
         the job id is not found or is not currently running."
    )]
    pub async fn render_cancel(&self, Parameters(params): Parameters<JobIdParams>) -> Json<Resposta> {
        Json(self.render_cancel_impl(params).await.into())
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
    ) -> Json<Resposta> {
        Json(self.preview_frame_impl(params).await.into())
    }

    // -- Trilha (Fase 2) --------------------------------------------------

    #[tool(
        description = "Report detailed info for a single track: kind (video/audio), name, \
         locked/hidden/muted state, clip count and length in frames. Parameter: track_id \
         (required)."
    )]
    pub async fn track_get_info(&self, Parameters(params): Parameters<TrackIdParams>) -> Json<Resposta> {
        Json(self.track_get_info_impl(params).await.into())
    }

    #[tool(description = "Rename a track. Parameters: track_id, name (both required).")]
    pub async fn track_set_name(&self, Parameters(params): Parameters<TrackSetNameParams>) -> Json<Resposta> {
        Json(self.track_set_name_impl(params).await.into())
    }

    #[tool(
        description = "Lock/unlock a track (editing-only concept; does not affect rendering). \
         Parameters: track_id, value (both required)."
    )]
    pub async fn track_set_locked(&self, Parameters(params): Parameters<TrackBoolParams>) -> Json<Resposta> {
        Json(self.track_set_locked_impl(params).await.into())
    }

    #[tool(
        description = "Hide/show a track's video, independently of its mute state. Parameters: \
         track_id, value (both required)."
    )]
    pub async fn track_set_hidden(&self, Parameters(params): Parameters<TrackBoolParams>) -> Json<Resposta> {
        Json(self.track_set_hidden_impl(params).await.into())
    }

    #[tool(
        description = "Mute/unmute a track's audio, independently of its hidden state. \
         Parameters: track_id, value (both required)."
    )]
    pub async fn track_set_muted(&self, Parameters(params): Parameters<TrackBoolParams>) -> Json<Resposta> {
        Json(self.track_set_muted_impl(params).await.into())
    }

    #[tool(
        description = "Move a track up one position (swap with the previous one). Parameter: \
         track_id (required). Returns ok:false if already the first track."
    )]
    pub async fn track_move_up(&self, Parameters(params): Parameters<TrackIdParams>) -> Json<Resposta> {
        Json(self.track_move_up_impl(params).await.into())
    }

    #[tool(
        description = "Move a track down one position (swap with the next one). Parameter: \
         track_id (required). Returns ok:false if already the last track."
    )]
    pub async fn track_move_down(&self, Parameters(params): Parameters<TrackIdParams>) -> Json<Resposta> {
        Json(self.track_move_down_impl(params).await.into())
    }

    // -- Clipe da timeline (Fase 2) -----------------------------------------

    #[tool(
        description = "Report timeline clip properties: producer id, in/out, current speed and \
         whether it is reversed, and the list of filters attached to it. Parameter: clip_ref \
         (required, \"<track_id>:<index>\")."
    )]
    pub async fn clip_get_properties(&self, Parameters(params): Parameters<ClipRefParams>) -> Json<Resposta> {
        Json(self.clip_get_properties_impl(params).await.into())
    }

    #[tool(
        description = "Set playback speed of a timeline clip (1.0=normal, 2.0=2x, 0.5=half, \
         negative=reverse), by wrapping the clip's producer in a libmlt \"timewarp\" producer \
         (verified to work correctly with real media files in this project's own tests; \
         behavior with synthetic color/tone sources is not guaranteed). Parameters: clip_ref, \
         speed (both required). This only affects this specific clip instance, not the shared \
         bin producer or other placements of it."
    )]
    pub async fn clip_set_speed(&self, Parameters(params): Parameters<ClipSetSpeedParams>) -> Json<Resposta> {
        Json(self.clip_set_speed_impl(params).await.into())
    }

    #[tool(
        description = "Reverse playback direction of a timeline clip (toggles the sign of its \
         current speed; same underlying mechanism as clip_set_speed). Parameter: clip_ref \
         (required)."
    )]
    pub async fn clip_reverse(&self, Parameters(params): Parameters<ClipRefParams>) -> Json<Resposta> {
        Json(self.clip_reverse_impl(params).await.into())
    }

    #[tool(
        description = "Set opacity of a timeline clip (0.0 transparent .. 1.0 opaque), via the \
         libmlt \"brightness\" filter's alpha parameter, attached to this specific clip (not \
         the shared producer). Parameters: clip_ref, opacity (both required)."
    )]
    pub async fn clip_set_opacity(&self, Parameters(params): Parameters<ClipSetOpacityParams>) -> Json<Resposta> {
        Json(self.clip_set_opacity_impl(params).await.into())
    }

    #[tool(
        description = "Set the color tag of a bin clip, shown in Kdenlive's project bin (hex \
         #RRGGBB, e.g. \"#ff0000\"). Parameters: clip_id (the bin clip's id, required), color \
         (required)."
    )]
    pub async fn clip_set_color(&self, Parameters(params): Parameters<BinClipColorParams>) -> Json<Resposta> {
        Json(self.clip_set_color_impl(params).await.into())
    }

    #[tool(
        description = "Set the description (free-text note) of a bin clip, shown in Kdenlive's \
         clip properties. Parameters: clip_id (the bin clip's id, required), note (required)."
    )]
    pub async fn clip_add_note(&self, Parameters(params): Parameters<BinClipNoteParams>) -> Json<Resposta> {
        Json(self.clip_add_note_impl(params).await.into())
    }

    // -- Sessão: undo/redo (Fase 2) ------------------------------------------

    #[tool(
        description = "Undo the most recent change(s) to the open project (snapshot-based, up to \
         50 history entries). Parameter: steps (optional, default 1). Unifies the original \
         catalog's session_undo/session_undo_step into one tool."
    )]
    pub async fn session_undo(&self, Parameters(params): Parameters<SessionStepsParams>) -> Json<Resposta> {
        Json(self.session_undo_impl(params).await.into())
    }

    #[tool(
        description = "Redo the most recently undone change(s). Parameter: steps (optional, \
         default 1). Unifies the original catalog's session_redo/session_redo_step into one \
         tool. Any new edit after an undo clears the redo stack."
    )]
    pub async fn session_redo(&self, Parameters(params): Parameters<SessionStepsParams>) -> Json<Resposta> {
        Json(self.session_redo_impl(params).await.into())
    }

    #[tool(
        description = "Report session state: whether a project is open, its modified flag, undo \
         history depth and redo steps available. Takes no parameters."
    )]
    pub async fn session_status(&self) -> Json<Resposta> {
        Json(self.session_status_impl().await.into())
    }

    #[tool(
        description = "List all undo history entries, oldest first, with a short description of \
         each change. Takes no parameters. Returns ok:false if no project is open."
    )]
    pub async fn session_history(&self) -> Json<Resposta> {
        Json(self.session_history_impl().await.into())
    }

    // -- Filtros (Fase 2) -----------------------------------------------------

    #[tool(
        description = "Attach a video/audio effect (libmlt filter service, e.g. \"brightness\", \
         \"qtblend\") to a timeline clip. Call filter_list_available for the full list of \
         services this libmlt build exposes. Parameters: clip_ref, service (both required). \
         Returns ok:true with the new filter's id (unique within this clip, used by \
         filter_remove/filter_set_param)."
    )]
    pub async fn filter_add(&self, Parameters(params): Parameters<FilterAddParams>) -> Json<Resposta> {
        Json(self.filter_add_impl(params).await.into())
    }

    #[tool(description = "Detach a filter from a clip. Parameters: clip_ref, filter_id (both required).")]
    pub async fn filter_remove(&self, Parameters(params): Parameters<FilterRemoveParams>) -> Json<Resposta> {
        Json(self.filter_remove_impl(params).await.into())
    }

    #[tool(
        description = "Update a single parameter on an active filter. Parameters: clip_ref, \
         filter_id, param, value (all required, value as a string)."
    )]
    pub async fn filter_set_param(&self, Parameters(params): Parameters<FilterSetParamParams>) -> Json<Resposta> {
        Json(self.filter_set_param_impl(params).await.into())
    }

    #[tool(description = "List all active filters on a specific timeline clip. Parameter: clip_ref (required).")]
    pub async fn filter_list(&self, Parameters(params): Parameters<ClipRefParams>) -> Json<Resposta> {
        Json(self.filter_list_impl(params).await.into())
    }

    #[tool(
        description = "Discover all filter services the libmlt runtime exposes (by running \
         \"melt -query filters\", a plain subprocess call — never the libmlt engine directly \
         from this server). Takes no parameters."
    )]
    pub async fn filter_list_available(&self) -> Json<Resposta> {
        Json(self.filter_list_available_impl().await.into())
    }

    // -- Transições (Fase 2) ----------------------------------------------------

    #[tool(
        description = "Create a blend transition / Kdenlive composition (libmlt transition \
         service, e.g. \"luma\" for a dissolve or wipe) between two timeline tracks. Parameters: \
         service, track_a (the lower track, the one going out), track_b (the upper track, the \
         one coming in) — all required, track ids as returned by timeline_add_track/timeline_list; \
         in_frame and out_frame (optional, timeline frames, inclusive: the transition only \
         applies in that range, which is how to crossfade two overlapping clips; without them it \
         spans the whole timeline); kdenlive_id (optional; default \"dissolve\" for luma, or \
         \"wipe\" when params has resource); params (optional object of extra properties, e.g. \
         {\"softness\": \"0.2\", \"reverse\": \"1\", \"resource\": \"/path/to/wipe.pgm\"}). Returns \
         the transition_id."
    )]
    pub async fn transition_add(&self, Parameters(params): Parameters<TransitionAddParams>) -> Json<Resposta> {
        Json(self.transition_add_impl(params).await.into())
    }

    #[tool(description = "Delete a transition by its id. Parameter: transition_id (required).")]
    pub async fn transition_remove(&self, Parameters(params): Parameters<TransitionIdParams>) -> Json<Resposta> {
        Json(self.transition_remove_impl(params).await.into())
    }

    #[tool(
        description = "Modify a parameter on an existing transition. Parameters: transition_id, \
         param, value (all required, value as a string)."
    )]
    pub async fn transition_set(&self, Parameters(params): Parameters<TransitionSetParamParams>) -> Json<Resposta> {
        Json(self.transition_set_impl(params).await.into())
    }

    #[tool(description = "List all transitions on the timeline. Takes no parameters.")]
    pub async fn transition_list(&self) -> Json<Resposta> {
        Json(self.transition_list_impl().await.into())
    }

    // -- Guias e marcadores (Fase 2) ---------------------------------------------

    #[tool(
        description = "Place a guide on the project timeline (shown in Kdenlive's timeline ruler \
         and guides list). Parameters: position (frames, required), comment (required)."
    )]
    pub async fn guide_add(&self, Parameters(params): Parameters<GuideAddParams>) -> Json<Resposta> {
        Json(self.guide_add_impl(params).await.into())
    }

    #[tool(description = "Delete a guide by id. Parameter: guide_id (required).")]
    pub async fn guide_remove(&self, Parameters(params): Parameters<GuideIdParams>) -> Json<Resposta> {
        Json(self.guide_remove_impl(params).await.into())
    }

    #[tool(description = "List all guide markers on the timeline. Takes no parameters.")]
    pub async fn guide_list(&self) -> Json<Resposta> {
        Json(self.guide_list_impl().await.into())
    }

    #[tool(description = "Get a single guide by id. Parameter: guide_id (required).")]
    pub async fn guide_get(&self, Parameters(params): Parameters<GuideIdParams>) -> Json<Resposta> {
        Json(self.guide_get_impl(params).await.into())
    }

    #[tool(
        description = "Update a guide's position and/or comment. Parameters: guide_id \
         (required), position (optional), comment (optional; at least one of the two should be \
         given)."
    )]
    pub async fn guide_update(&self, Parameters(params): Parameters<GuideUpdateParams>) -> Json<Resposta> {
        Json(self.guide_update_impl(params).await.into())
    }

    #[tool(
        description = "Add a timeline marker, or a clip marker if clip_ref is given. A timeline \
         marker is a blue guide (category 2) in Kdenlive; a clip marker lives on the bin clip, \
         and its position is in frames of the clip's media (0 = start of the file). Parameters: \
         position (frames, required), comment (required), clip_ref (optional, \
         \"<track_id>:<index>\" or a bin clip id)."
    )]
    pub async fn marker_add(&self, Parameters(params): Parameters<MarkerAddParams>) -> Json<Resposta> {
        Json(self.marker_add_impl(params).await.into())
    }

    #[tool(description = "List all markers (timeline and clip). Takes no parameters.")]
    pub async fn marker_list(&self) -> Json<Resposta> {
        Json(self.marker_list_impl().await.into())
    }

    #[tool(description = "Remove a marker by id. Parameter: marker_id (required).")]
    pub async fn marker_remove(&self, Parameters(params): Parameters<MarkerIdParams>) -> Json<Resposta> {
        Json(self.marker_remove_impl(params).await.into())
    }

    // -- Perfil do projeto (Fase 2) -----------------------------------------

    #[tool(
        description = "Enumerate all available video output profiles (real libmlt profile \
         names — resolution, frame rate, aspect ratio — verified in this project's own \
         environment). Takes no parameters."
    )]
    pub async fn project_list_profiles(&self) -> Json<Resposta> {
        Json(self.project_list_profiles_impl().await.into())
    }

    #[tool(
        description = "Get the current project's video profile (resolution, FPS, aspect ratio, \
         progressive). Takes no parameters. Returns ok:false if no project is open."
    )]
    pub async fn project_get_profile(&self) -> Json<Resposta> {
        Json(self.project_get_profile_impl().await.into())
    }

    #[tool(
        description = "Set or update the project's video profile. This only changes the \
         <profile> metadata; it does not rescale or move any clip already on the timeline. \
         Parameter: profile (required; call project_list_profiles for valid names)."
    )]
    pub async fn project_set_profile(
        &self,
        Parameters(params): Parameters<ProjectSetProfileParams>,
    ) -> Json<Resposta> {
        Json(self.project_set_profile_impl(params).await.into())
    }

    // -- Bin: pastas, mover, renomear, importar pasta (Fase 2) ---------------

    #[tool(
        description = "Create a folder in the project bin (this project's own convention for \
         kdenlive:folder.<parent>.<id> properties, confirmed against the Kdenlive file format \
         docs). Parameters: name (required), parent_folder_id (optional, for a subfolder). \
         Returns ok:true with the new folder's id."
    )]
    pub async fn bin_create_folder(
        &self,
        Parameters(params): Parameters<BinCreateFolderParams>,
    ) -> Json<Resposta> {
        Json(self.bin_create_folder_impl(params).await.into())
    }

    #[tool(
        description = "Move a bin clip into a folder (or back to the root bin if folder_id is \
         omitted). Parameters: clip_id (required), folder_id (optional)."
    )]
    pub async fn bin_move_clip(&self, Parameters(params): Parameters<BinMoveClipParams>) -> Json<Resposta> {
        Json(self.bin_move_clip_impl(params).await.into())
    }

    #[tool(description = "Rename a bin clip. Parameters: clip_id, name (both required).")]
    pub async fn bin_rename(&self, Parameters(params): Parameters<BinRenameParams>) -> Json<Resposta> {
        Json(self.bin_rename_impl(params).await.into())
    }

    #[tool(
        description = "Import every recognized media file directly inside a folder into the \
         project bin (not recursive in this phase). Parameter: path (required, folder path). \
         Returns ok:true with the list of imported clips and a list of any files that failed, \
         each with its own error — never fails the whole call just because one file could not \
         be imported."
    )]
    pub async fn bin_import_folder(
        &self,
        Parameters(params): Parameters<BinImportFolderParams>,
    ) -> Json<Resposta> {
        Json(self.bin_import_folder_impl(params).await.into())
    }

    // -- Montagem em lote (Fase 2) -------------------------------------------

    #[tool(
        description = "Assemble a sequence on a track from several ranges of the same bin clip, \
         in one call, placed back to back starting at position. Parameters: track_id, \
         producer_id, ranges (required, a list of [in_frame, out_frame] pairs), position \
         (required, where the first range starts). Returns ok:true with the list of clip_ref \
         strings created, in order."
    )]
    pub async fn timeline_assemble_ranges(
        &self,
        Parameters(params): Parameters<TimelineAssembleRangesParams>,
    ) -> Json<Resposta> {
        Json(self.timeline_assemble_ranges_impl(params).await.into())
    }

    // -- Timeline: mover/aparar/dividir/remover clipe, posição/zoom (Fase 2) --

    #[tool(
        description = "Delete a clip from the timeline (the slot becomes a blank, keeping \
         everything else in place). Parameter: clip_ref (required)."
    )]
    pub async fn timeline_remove_clip(&self, Parameters(params): Parameters<ClipRefParams>) -> Json<Resposta> {
        Json(self.timeline_remove_clip_impl(params).await.into())
    }

    #[tool(
        description = "Reposition a clip on the same track to a new time position (in frames). \
         See timeline_add_clip for the exact placement semantics (gaps/boundaries). Parameters: \
         clip_ref, new_position (both required). Returns ok:true with the clip's new clip_ref \
         (its index may change)."
    )]
    pub async fn timeline_move_clip(
        &self,
        Parameters(params): Parameters<TimelineMoveClipParams>,
    ) -> Json<Resposta> {
        Json(self.timeline_move_clip_impl(params).await.into())
    }

    #[tool(
        description = "Adjust the in/out trim handles of a timeline clip (not an image crop \
         filter — it's the range used from the underlying producer). Because MLT positions are \
         implicit, shortening/lengthening the clip naturally shifts everything after it on the \
         same track. Parameters: clip_ref, in_frame, out_frame (all required)."
    )]
    pub async fn timeline_trim_clip(
        &self,
        Parameters(params): Parameters<TimelineTrimClipParams>,
    ) -> Json<Resposta> {
        Json(self.timeline_trim_clip_impl(params).await.into())
    }

    #[tool(
        description = "Cut a clip into two independent pieces at a precise producer frame \
         offset (split_at refers to the underlying producer's own in/out range, not an absolute \
         timeline position — it must be strictly after in_frame and at most out_frame). \
         Parameters: clip_ref, split_at (both required). Returns ok:true with both pieces' \
         clip_ref strings."
    )]
    pub async fn timeline_split_clip(
        &self,
        Parameters(params): Parameters<TimelineSplitClipParams>,
    ) -> Json<Resposta> {
        Json(self.timeline_split_clip_impl(params).await.into())
    }

    #[tool(
        description = "Get total timeline duration in frames (the furthest end of any track). \
         Takes no parameters."
    )]
    pub async fn timeline_get_duration(&self) -> Json<Resposta> {
        Json(self.timeline_get_duration_impl().await.into())
    }

    #[tool(
        description = "Get the current playhead position (frames). This is a session/UI concept \
         only — there is no player in this headless server, so it has no effect on rendering; \
         it is just bookkeeping for the agent. Takes no parameters."
    )]
    pub async fn timeline_get_position(&self) -> Json<Resposta> {
        Json(self.timeline_get_position_impl().await.into())
    }

    #[tool(
        description = "Set the playhead position (frames) — same session/UI-only bookkeeping as \
         timeline_get_position. Parameter: position (required, >= 0)."
    )]
    pub async fn timeline_seek(&self, Parameters(params): Parameters<TimelineSeekParams>) -> Json<Resposta> {
        Json(self.timeline_seek_impl(params).await.into())
    }

    #[tool(
        description = "Get the current timeline zoom level (0.1 .. 10.0). Session/UI-only \
         bookkeeping, no effect on rendering. Takes no parameters."
    )]
    pub async fn timeline_get_zoom(&self) -> Json<Resposta> {
        Json(self.timeline_get_zoom_impl().await.into())
    }

    #[tool(
        description = "Set the timeline zoom level. Session/UI-only bookkeeping, no effect on \
         rendering. Parameter: zoom (required, 0.1 .. 10.0)."
    )]
    pub async fn timeline_set_zoom(
        &self,
        Parameters(params): Parameters<TimelineSetZoomParams>,
    ) -> Json<Resposta> {
        Json(self.timeline_set_zoom_impl(params).await.into())
    }

    // -- Fila de render (Fase 2) ---------------------------------------------

    #[tool(
        description = "Add a render job to the render queue (does not start it by itself — call \
         render_queue_start to begin processing). Parameters: project_path (optional, defaults \
         to the session's open project — captured at add time, not when the job actually runs), \
         output_path (required), preset (optional, default \"h264_hq\"), overwrite (optional, \
         default false). Returns ok:true with the new queue_id."
    )]
    pub async fn render_queue_add(
        &self,
        Parameters(params): Parameters<RenderQueueAddParams>,
    ) -> Json<Resposta> {
        Json(self.render_queue_add_impl(params).await.into())
    }

    #[tool(description = "List all render queue jobs (pending, running, done, failed or cancelled), in order. Takes no parameters.")]
    pub async fn render_queue_list(&self) -> Json<Resposta> {
        Json(self.render_queue_list_impl().await.into())
    }

    #[tool(
        description = "Get render queue status: whether it is currently being processed, and a \
         count of jobs by status. Takes no parameters."
    )]
    pub async fn render_queue_status(&self) -> Json<Resposta> {
        Json(self.render_queue_status_impl().await.into())
    }

    #[tool(
        description = "Start processing the render queue sequentially, one job at a time (same \
         one-render-at-a-time guarantee as export_render). Takes no parameters. Returns ok:false \
         if the queue is already being processed."
    )]
    pub async fn render_queue_start(&self) -> Json<Resposta> {
        Json(self.render_queue_start_impl().await.into())
    }

    #[tool(
        description = "Stop the render queue: actually terminates the currently running render's \
         child process (unlike the original reference project's render_queue_stop, which only \
         marked internal state without stopping anything) and cancels the remaining pending \
         jobs without starting them. Takes no parameters. Returns ok:false if the queue is not \
         currently being processed."
    )]
    pub async fn render_queue_stop(&self) -> Json<Resposta> {
        Json(self.render_queue_stop_impl().await.into())
    }

    // -- Títulos (Fase 2) ----------------------------------------------------

    #[tool(
        description = "Create a title clip with text, font, color and duration, in the project \
         bin, as a native Kdenlive title (\"kdenlivetitle\" producer, editable in Kdenlive's \
         titler; the font name is resolved to an installed font family). Parameters: text, \
         duration_frames (both required), font_family (optional, default \"Sans\"), font_size \
         (optional, default 48), color (optional, default opaque white \"0xffffffff\", format \
         0xRRGGBBAA or #RRGGBB), and optional styling: bold, align (\"left\", \"center\" default, \
         \"right\"), x (left edge of the text box, px, default 0), y (vertical center of the \
         text, px, default frame center), box_width (px, default up to the right edge), \
         outline_width (px) and outline_color, background_color (a box behind the text, e.g. \
         \"#000000AA\") and padding (px around the text, default 24). Use \\n for several lines. \
         Place it on a track with timeline_add_clip like any other bin clip."
    )]
    pub async fn title_add(&self, Parameters(params): Parameters<TitleAddParams>) -> Json<Resposta> {
        Json(self.title_add_impl(params).await.into())
    }

    #[tool(
        description = "Update the text and/or style of an existing title clip. Parameter: \
         clip_id (required, the bin clip's id); text, font_family, font_size, color (all \
         optional — only given fields are changed). Returns ok:false if clip_id is not a title \
         clip."
    )]
    pub async fn title_update(&self, Parameters(params): Parameters<TitleUpdateParams>) -> Json<Resposta> {
        Json(self.title_update_impl(params).await.into())
    }

    // -- Keyframes (Fase 2) --------------------------------------------------

    #[tool(
        description = "Set the value of a filter parameter at a specific frame, with \
         interpolation (uses the libmlt animated-property syntax \"frame=value;frame=value...\", \
         verified by rendering in this project's own tests). Parameters: clip_ref, filter_id, \
         param, frame, value (all required; value as a string — the raw value the filter \
         expects at that keyframe). Replaces any existing keyframe at the same frame."
    )]
    pub async fn keyframe_set(&self, Parameters(params): Parameters<KeyframeSetParams>) -> Json<Resposta> {
        Json(self.keyframe_set_impl(params).await.into())
    }

    #[tool(
        description = "List the keyframes of a filter parameter, in frame order. Parameters: \
         clip_ref, filter_id, param (all required)."
    )]
    pub async fn keyframe_list(&self, Parameters(params): Parameters<KeyframeListParams>) -> Json<Resposta> {
        Json(self.keyframe_list_impl(params).await.into())
    }

    #[tool(
        description = "Remove the keyframe at a specific frame. Parameters: clip_ref, \
         filter_id, param, frame (all required). Returns ok:false if there is no keyframe at \
         that exact frame."
    )]
    pub async fn keyframe_remove(&self, Parameters(params): Parameters<KeyframeRemoveParams>) -> Json<Resposta> {
        Json(self.keyframe_remove_impl(params).await.into())
    }

    // -- Legendas (Fase 2) -----------------------------------------------------

    #[tool(
        description = "Import an .srt subtitle file and create synchronized title clips (see \
         title_add) on a track, one per subtitle entry, positioned and timed from the SRT's own \
         timestamps (converted using the project's frame rate). Parameters: path (required); \
         track_id (optional, a new video track is created if omitted), font_family, font_size, \
         color (all optional, same defaults and meaning as title_add). Returns ok:true with the \
         track_id used and the list of clip_refs created."
    )]
    pub async fn subtitle_import_srt(
        &self,
        Parameters(params): Parameters<SubtitleImportSrtParams>,
    ) -> Json<Resposta> {
        Json(self.subtitle_import_srt_impl(params).await.into())
    }

    #[tool(
        description = "Export the timeline's subtitles on a track as an .srt file: every title \
         clip (Kdenlive title with text) on that track becomes one SRT entry, timed from its \
         timeline position and the project's frame rate. Parameters: track_id, path (both \
         required). Returns ok:false if the track has no title clips."
    )]
    pub async fn subtitle_export_srt(
        &self,
        Parameters(params): Parameters<SubtitleExportSrtParams>,
    ) -> Json<Resposta> {
        Json(self.subtitle_export_srt_impl(params).await.into())
    }

    // -- Áudio (Fase 2) --------------------------------------------------------

    #[tool(
        description = "Set the level in dB of a timeline clip, via the standard libmlt \"volume\" \
         filter's level parameter, attached to this specific clip. Parameters: clip_ref, db \
         (both required)."
    )]
    pub async fn audio_set_levels(
        &self,
        Parameters(params): Parameters<AudioSetLevelsParams>,
    ) -> Json<Resposta> {
        Json(self.audio_set_levels_impl(params).await.into())
    }

    #[tool(
        description = "Normalize the loudness of a timeline clip to a target amplitude in dB, \
         via the standard libmlt \"volume\" filter's normalize parameter. Parameters: clip_ref \
         (required), target_db (optional, default -12.0, the filter's own default)."
    )]
    pub async fn audio_normalize(
        &self,
        Parameters(params): Parameters<AudioNormalizeParams>,
    ) -> Json<Resposta> {
        Json(self.audio_normalize_impl(params).await.into())
    }

    // -- Transformação (Fase 2) --------------------------------------------------

    #[tool(
        description = "Look at several frames of the project at once, to analyse what was built \
         without opening Kdenlive or rendering the whole video. Renders each chosen frame as a PNG \
         (3 engine processes in parallel) and, by default, also assembles ONE contact-sheet image \
         with all of them in a grid, each labelled with its frame number and time (needs ffmpeg). \
         Parameters: frames (list of frame numbers, 0-based) OR start/end/step (an interval; with \
         nothing given, about 12 frames spread over the whole project); project_path (optional, \
         default the open project); output_dir (optional; default a temporary folder); \
         contact_sheet (optional, default true); columns (optional, 1..8, default 3); thumb_width \
         (optional, default 480). At most 60 frames per call. Returns the contact-sheet path, the \
         grid description (row by row), and for each frame its number, time in seconds and PNG \
         path. Animated filters (keyframes) are evaluated at the real frame position. Use \
         preview_gif for motion and export_render with start_frame/end_frame to render only a \
         part as video."
    )]
    pub async fn preview_frames(&self, Parameters(params): Parameters<PreviewFramesParams>) -> Json<Resposta> {
        Json(self.preview_frames_impl(params).await.into())
    }

    #[tool(
        description = "Make an animated GIF of a part of the project, to check motion (fades, \
         slides, keyframed effects) without rendering a video. Renders the needed frames as PNGs \
         (3 engine processes in parallel) and assembles the GIF with ffmpeg. Parameters: \
         start/end (frame interval, inclusive; default the whole project); step (take one frame \
         every N project frames; default 1, or more when the interval has over 150 frames); fps \
         (optional; default the natural speed of the project); width (optional, default 480); \
         output_path (optional .gif; default a temporary folder); overwrite (optional); \
         project_path (optional, default the open project). At most 150 frames. Returns the GIF \
         path, size, the frames used and its duration."
    )]
    pub async fn preview_gif(&self, Parameters(params): Parameters<PreviewGifParams>) -> Json<Resposta> {
        Json(self.preview_gif_impl(params).await.into())
    }

    #[tool(
        description = "Audio ducking: lower a clip (or every clip of a track), typically background \
         music, while another track (typically the voice-over) is speaking. Works with keyframes on \
         the level of a dedicated libmlt \"volume\" filter (no audio plugins needed); calling it \
         again replaces its own filter and leaves other volume filters alone. Parameters: \
         clip_ref OR track_id (the target to lower, exactly one); voice_track_id (required: the \
         track whose clips define the speech); duck_db (optional, default 12: how many dB to \
         lower); attack_frames (optional, default 10: ramp down before the speech starts) and \
         release_frames (optional, default 25: ramp up after it ends), both at least 1; \
         detect_speech (optional, default false: when false every clip of the voice track counts \
         as speech; when true only the non-silent parts of those clips do, measured with ffmpeg), \
         silence_threshold_db (optional, default -35) and min_silence_seconds (optional, default \
         0.5) tune that detection. Returns the speech intervals (timeline frames) and, per target \
         clip, the filter id and the keyframes (clip-local frame, dB)."
    )]
    pub async fn audio_duck(&self, Parameters(params): Parameters<AudioDuckParams>) -> Json<Resposta> {
        Json(self.audio_duck_impl(params).await.into())
    }

    #[tool(
        description = "Reframe a timeline video or image clip to the project frame, which must be \
         vertical (e.g. 16:9 footage in a 9:16 project: set the profile first with \
         project_set_profile \"vertical_hd_30\"). The tool measures the source file with ffprobe, \
         computes the rectangle and applies it with the libmlt \"qtblend\" filter (the same one as \
         clip_set_transform). Parameters: clip_ref (required); mode (optional): \"fill\" (default, \
         fills the frame and crops the excess, no black bars) or \"fit\" (whole video visible, \
         bars at the sides/top); focus_x and focus_y (optional, 0..1, default 0.5 = centered): \
         which part of the video stays visible when cropping (0 = left/top edge, 1 = right/bottom \
         edge). Returns the source size, the frame size and the rectangle applied."
    )]
    pub async fn clip_reframe_vertical(
        &self,
        Parameters(params): Parameters<ClipReframeVerticalParams>,
    ) -> Json<Resposta> {
        Json(self.clip_reframe_vertical_impl(params).await.into())
    }

    #[tool(
        description = "Define position, scale and rotation of a timeline clip, via the standard \
         libmlt \"qtblend\" filter (rect + rotation parameters), attached to this specific clip. \
         This tool only passes your exact values through to qtblend; it does not compute any \
         geometry on your behalf (see clip_reframe_vertical to fit a clip to the frame). Parameters: clip_ref, x, y, width, height \
         (all required, in pixels — the destination rectangle qtblend draws the source into), \
         rotation (optional, degrees). Note: by default the filter keeps the source aspect ratio \
         inside the rectangle, so a rectangle that is not 16:9 is letterboxed; to stretch the \
         clip to exactly the rectangle (e.g. to draw bars or boxes from a solid title), call \
         filter_set_param on the returned filter_id with param \"distort\" and value \"1\", and \
         set \"rotate_center\" to \"1\" to rotate around the center instead of the corner."
    )]
    pub async fn clip_set_transform(
        &self,
        Parameters(params): Parameters<ClipSetTransformParams>,
    ) -> Json<Resposta> {
        Json(self.clip_set_transform_impl(params).await.into())
    }

    // -- Miniatura (Fase 2) -----------------------------------------------------

    #[tool(
        description = "Generate a thumbnail image from an already-rendered video file (not an \
         MLT project — use preview_frame for that) by extracting one frame with ffmpeg directly \
         (a plain subprocess call, never the libmlt engine). Parameters: video_path, output_path \
         (both required), at_seconds (optional, default 1.0), overwrite (optional, default \
         false)."
    )]
    pub async fn export_thumbnail(
        &self,
        Parameters(params): Parameters<ExportThumbnailParams>,
    ) -> Json<Resposta> {
        Json(self.export_thumbnail_impl(params).await.into())
    }

    // -- Recursos novos (D-053 a D-060) ------------------------------------------

    #[tool(
        description = "Fade a timeline clip in or out with Kdenlive's own fade effects \
         (editable in Kdenlive): video clips use \"fade_from_black\"/\"fade_to_black\" \
         (brightness), audio clips use \"fadein\"/\"fadeout\" (volume). Parameters: clip_ref, \
         kind (\"in\" or \"out\"), duration_frames (all required), mode (optional, video only: \
         \"alpha\" default — the clip becomes transparent and shows the track below; or \"black\" \
         — from/to black). Calling again replaces the previous fade of the same kind."
    )]
    pub async fn clip_fade(&self, Parameters(params): Parameters<ClipFadeParams>) -> Json<Resposta> {
        Json(self.clip_fade_impl(params).await.into())
    }

    #[tool(
        description = "Animate a timeline clip's position, size, opacity and rotation over time \
         (Kdenlive's Transform effect, libmlt \"qtblend\"), in one call. Parameters: clip_ref, \
         keyframes (required: list of {frame, x, y, width, height, opacity?, rotation?}; frame \
         counts from the START OF THE CLIP on the timeline, 0 = its first frame; x/y/width/height \
         are the destination rectangle in project pixels; opacity 0..1, default 1; rotation in \
         degrees around the center), interpolation (optional: \"linear\" default, \"smooth\" or \
         \"hold\"). Replaces the clip's previous transform animation. Typical uses: fade in by \
         opacity, pop/zoom in by scaling the rectangle, slide in from outside the frame."
    )]
    pub async fn clip_animate_transform(
        &self,
        Parameters(params): Parameters<ClipAnimateTransformParams>,
    ) -> Json<Resposta> {
        Json(self.clip_animate_transform_impl(params).await.into())
    }

    #[tool(
        description = "Set many keyframes of one filter parameter in a single call (instead of \
         one keyframe_set per keyframe). Parameters: clip_ref, filter_id, param, keyframes (list \
         of {frame, value}) — all required; relative_to_clip (optional, default false: frame is \
         the clip's MEDIA frame, like keyframe_set; true: frame counts from the clip's start on \
         the timeline); replace (optional, default false: true drops the parameter's previous \
         keyframes). Returns how many keyframes the parameter has."
    )]
    pub async fn keyframe_set_many(&self, Parameters(params): Parameters<KeyframeSetManyParams>) -> Json<Resposta> {
        Json(self.keyframe_set_many_impl(params).await.into())
    }

    #[tool(
        description = "Create a solid color clip in the bin (native Kdenlive color clip, libmlt \
         \"color\" producer), e.g. a black background. Parameters: color (#RRGGBB, #RRGGBBAA or \
         0xRRGGBBAA) and duration_frames (both required; it can be stretched on the timeline \
         later), name (optional). Returns the new clip's id."
    )]
    pub async fn bin_add_color_clip(&self, Parameters(params): Parameters<BinAddColorClipParams>) -> Json<Resposta> {
        Json(self.bin_add_color_clip_impl(params).await.into())
    }

    #[tool(
        description = "Place several clips in one call (same fields as timeline_add_clip for \
         each, including the optional audio_track_id). Parameter: clips (required list). All or \
         nothing: if one clip fails, none is placed and the error says which one. Returns the \
         clip_refs in order."
    )]
    pub async fn timeline_add_clips(&self, Parameters(params): Parameters<TimelineAddClipsParams>) -> Json<Resposta> {
        Json(self.timeline_add_clips_impl(params).await.into())
    }

    #[tool(
        description = "Fill a duration on a track by repeating (looping) a bin clip, e.g. a short \
         animated background or a seamless loop. Parameters: track_id, producer_id, position, \
         duration_frames (all required); in_frame/out_frame (optional, the looped part of the \
         clip, default the whole bin clip); start_offset (optional, where inside that part the \
         loop starts, default 0). Returns the clip_refs of the pieces placed back to back."
    )]
    pub async fn timeline_add_clip_loop(
        &self,
        Parameters(params): Parameters<TimelineAddClipLoopParams>,
    ) -> Json<Resposta> {
        Json(self.timeline_add_clip_loop_impl(params).await.into())
    }

    #[tool(
        description = "Turn live preview in Kdenlive on or off for this session: when on, every \
         successful edit re-saves the project and the Kdenlive window showing it reloads by \
         itself (File > Revert triggered over D-Bus, never when Kdenlive has unsaved changes). \
         The project must have been saved once (project_save with a path). Parameter: enabled \
         (required, boolean). Same as starting the server with KDENLIVE_MCP_AUTORELOAD=1."
    )]
    pub async fn kdenlive_live(&self, Parameters(params): Parameters<KdenliveLiveParams>) -> Json<Resposta> {
        Json(self.kdenlive_live_impl(params).await.into())
    }

    #[tool(
        description = "Save the project now and ask the Kdenlive window showing it to reload it \
         once (File > Revert over D-Bus). Takes no parameters. Returns resultado: \
         \"recarregado\", \"kdenlive_com_o_projeto_nao_encontrado\", \
         \"kdenlive_tem_alteracoes_nao_salvas\" (not reloaded, to keep the user's changes) or \
         \"kdenlive_ocupado\"."
    )]
    pub async fn kdenlive_reload(&self) -> Json<Resposta> {
        Json(self.kdenlive_reload_impl().await.into())
    }

    #[tool(
        description = "Open the Kdenlive application (a window on the user's screen) with a \
         project, unless a Kdenlive already shows it. Parameter: path (optional, default this \
         session's saved project). The program is KDENLIVE_MCP_KDENLIVE, else the newest \
         ~/Applications/kdenlive*.AppImage, else `kdenlive` on the PATH. Returns the process id (or \
         ja_aberto:true). Combine with kdenlive_live to watch the edits as they happen."
    )]
    pub async fn kdenlive_open(&self, Parameters(params): Parameters<KdenliveOpenParams>) -> Json<Resposta> {
        Json(self.kdenlive_open_impl(params).await.into())
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
