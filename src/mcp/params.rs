//! Estruturas de parâmetros de entrada das ferramentas MCP. Saídas usam
//! `serde_json::Value` (montado com `serde_json::json!`) em vez de um
//! `struct` por ferramenta — simplifica a Fase 1, ao custo de um
//! `outputSchema` menos preciso (ver docs/decisoes.md).

use schemars::JsonSchema;
use serde::Deserialize;

use crate::engine::preset;
use crate::model::profile;

fn preset_padrao() -> String {
    preset::DEFAULT_PRESET.to_string()
}

fn profile_padrao() -> String {
    profile::DEFAULT_PROFILE.to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportRenderParams {
    /// Caminho do projeto a renderizar. Se omitido, usa o projeto aberto na
    /// sessão (ver project_new/project_open).
    pub project_path: Option<String>,
    /// Caminho de saída; a extensão precisa combinar com o preset escolhido.
    pub output_path: String,
    #[serde(default = "preset_padrao")]
    pub preset: String,
    #[serde(default)]
    pub overwrite: bool,
    /// Se `false`, devolve imediatamente um `job_id` em vez de esperar o
    /// render terminar; use render_progress/render_cancel para acompanhar.
    #[serde(default = "verdadeiro")]
    pub wait: bool,
}

fn verdadeiro() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectNewParams {
    /// Nome do perfil de vídeo (ex. "atsc_1080p_25"). Use
    /// project_list_profiles para ver os nomes válidos.
    #[serde(default = "profile_padrao")]
    pub profile: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectOpenParams {
    /// Caminho do projeto `.kdenlive` ou XML do MLT a abrir.
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectSaveParams {
    /// Caminho de saída. Se omitido, salva no caminho de onde o projeto foi
    /// aberto (erro se o projeto nunca teve um caminho).
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportXmlParams {
    /// Caminho de saída. Se omitido, devolve o XML como texto na resposta
    /// em vez de escrever um arquivo.
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinImportClipParams {
    /// Caminho do arquivo de mídia a importar.
    pub path: String,
    /// Nome de exibição do clipe na bin. Se omitido, usa o nome do arquivo.
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinClipIdParams {
    /// Id do clipe na bin (devolvido por bin_import_clip/bin_list_clips).
    pub clip_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineAddTrackParams {
    /// "video" ou "audio".
    pub kind: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineRemoveTrackParams {
    /// Id da trilha (devolvido por timeline_add_track/timeline_list).
    pub track_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineAddClipParams {
    /// Id da trilha onde posicionar o clipe.
    pub track_id: String,
    /// Id do produtor (da bin) a posicionar.
    pub producer_id: String,
    /// Primeiro quadro utilizado do produtor (0-based, inclusive).
    pub in_frame: i64,
    /// Último quadro utilizado do produtor (0-based, inclusive).
    pub out_frame: i64,
    /// Posição na trilha, em quadros, onde o clipe deve começar.
    pub position: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct JobIdParams {
    /// Id do job de render (devolvido por export_render com wait=false).
    pub job_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreviewFrameParams {
    /// Caminho do projeto. Se omitido, usa o projeto aberto na sessão.
    pub project_path: Option<String>,
    /// Número do quadro a exportar (0-based).
    pub frame: i64,
    /// Caminho do PNG de saída.
    pub output_path: String,
    #[serde(default)]
    pub overwrite: bool,
}

// -- Trilha -----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrackIdParams {
    pub track_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrackBoolParams {
    pub track_id: String,
    pub value: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrackSetNameParams {
    pub track_id: String,
    pub name: String,
}

// -- Clipe --------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipRefParams {
    /// Referência de clipe no formato `"<track_id>:<índice>"` (devolvida
    /// por timeline_add_clip/timeline_get_info).
    pub clip_ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipSetSpeedParams {
    pub clip_ref: String,
    /// 1.0 = normal, 2.0 = dobro, 0.5 = metade, negativo = reverso.
    pub speed: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipSetOpacityParams {
    pub clip_ref: String,
    /// 0.0 (transparente) a 1.0 (opaco).
    pub opacity: f64,
}

/// `clip_set_color`/`clip_add_note` (do Apêndice B original) atuam sobre o
/// clipe da **bin**, não sobre um clipe da timeline — por isso usam
/// `clip_id` (id do produtor), não `clip_ref`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinClipColorParams {
    pub clip_id: String,
    /// Cor em hexadecimal (ex. "#ff0000") ou nome.
    pub color: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinClipNoteParams {
    pub clip_id: String,
    pub note: String,
}

// -- Sessão (undo/redo) ------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionStepsParams {
    /// Quantos passos desfazer/refazer. Padrão 1.
    #[serde(default = "um")]
    pub steps: usize,
}

fn um() -> usize {
    1
}

// -- Filtros ------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FilterAddParams {
    pub clip_ref: String,
    /// Nome do serviço de filtro do libmlt (ex. "brightness", "qtblend").
    /// Use filter_list_available para a lista completa.
    pub service: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FilterRemoveParams {
    pub clip_ref: String,
    pub filter_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FilterSetParamParams {
    pub clip_ref: String,
    pub filter_id: String,
    pub param: String,
    pub value: String,
}

// -- Transições ---------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TransitionAddParams {
    /// Nome do serviço de transição do libmlt (ex. "luma" para dissolve/wipe).
    pub service: String,
    pub track_a: String,
    pub track_b: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TransitionIdParams {
    pub transition_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TransitionSetParamParams {
    pub transition_id: String,
    pub param: String,
    pub value: String,
}

// -- Guias e marcadores ---------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GuideAddParams {
    pub position: i64,
    pub comment: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GuideIdParams {
    pub guide_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GuideUpdateParams {
    pub guide_id: String,
    pub position: Option<i64>,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MarkerAddParams {
    pub position: i64,
    pub comment: String,
    /// Se for um marcador de um clipe específico, em vez de um marcador
    /// geral da timeline: `"<track_id>:<índice>"`.
    pub clip_ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MarkerIdParams {
    pub marker_id: String,
}

// -- Perfil do projeto --------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectSetProfileParams {
    /// Nome do perfil (ver project_list_profiles).
    pub profile: String,
}

// -- Bin: pastas, mover, renomear, importar pasta ----------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinCreateFolderParams {
    pub name: String,
    /// Id da pasta-pai, se esta for uma subpasta. Omitido = pasta na raiz.
    pub parent_folder_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinMoveClipParams {
    pub clip_id: String,
    /// Id da pasta de destino. Omitido move o clipe para a raiz da bin.
    pub folder_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinRenameParams {
    pub clip_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinImportFolderParams {
    /// Caminho da pasta cujos arquivos de mídia serão importados (não
    /// recursivo nesta fase).
    pub path: String,
}

// -- Montagem em lote ---------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineAssembleRangesParams {
    pub track_id: String,
    pub producer_id: String,
    /// Intervalos `[in, out]` do produtor, montados em sequência na
    /// trilha, um atrás do outro, a partir de `position`.
    pub ranges: Vec<[i64; 2]>,
    pub position: i64,
}
