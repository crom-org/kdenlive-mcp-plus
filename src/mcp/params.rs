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
