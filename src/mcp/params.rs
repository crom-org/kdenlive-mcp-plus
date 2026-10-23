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
    /// Renderiza só a partir deste quadro (inclusive, 0-based). Use junto com
    /// `end_frame`; só funciona com `wait=true`. Serve para conferir uma
    /// parte sem renderizar tudo.
    pub start_frame: Option<i64>,
    /// Renderiza só até este quadro (inclusive). Use junto com `start_frame`.
    pub end_frame: Option<i64>,
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
    /// Se `true` (padrão), o projeto já nasce com uma trilha de vídeo e uma
    /// de áudio, como o Kdenlive faz: o Kdenlive 26 fecha ao abrir um projeto
    /// sem nenhuma trilha. Use `false` só para um projeto que nunca será
    /// aberto no Kdenlive (ou que receberá trilhas antes de ser salvo).
    #[serde(default = "verdadeiro")]
    pub default_tracks: bool,
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
    /// Só para imagens paradas (png, jpg, webp, svg...): duração do clipe em
    /// quadros. Padrão: 5 segundos. A imagem pode ser esticada na timeline.
    pub duration_frames: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BinAddColorClipParams {
    /// Cor: `#RRGGBB`, `#RRGGBBAA` ou `0xRRGGBBAA` (ex. preto: `#000000`).
    pub color: String,
    /// Duração em quadros (pode ser esticado na timeline depois).
    pub duration_frames: i64,
    /// Nome na bin. Padrão: "Cor <cor>".
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
    /// Para um clipe de vídeo com som: trilha de ÁUDIO onde pôr o som dele,
    /// ligado ao vídeo (grupo vídeo+áudio do Kdenlive). A trilha de vídeo
    /// não toca o áudio do clipe; sem isto o clipe fica mudo.
    pub audio_track_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineAddClipsParams {
    /// Clipes a posicionar, na ordem (mesmos campos de timeline_add_clip).
    pub clips: Vec<TimelineAddClipParams>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineAddClipLoopParams {
    pub track_id: String,
    pub producer_id: String,
    /// Posição na trilha onde o loop começa.
    pub position: i64,
    /// Duração total a preencher, em quadros.
    pub duration_frames: i64,
    /// Trecho do produtor que se repete. Padrão: o clipe inteiro da bin.
    pub in_frame: Option<i64>,
    pub out_frame: Option<i64>,
    /// Onde, dentro do trecho, o loop começa (padrão 0). Útil para que dois
    /// trechos do mesmo loop não comecem iguais.
    pub start_offset: Option<i64>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreviewFramesParams {
    /// Caminho do projeto. Se omitido, usa o projeto aberto na sessão.
    pub project_path: Option<String>,
    /// Lista de quadros a renderizar (0-based). Informe isto OU start/end/step.
    pub frames: Option<Vec<i64>>,
    /// Início do intervalo (padrão: 0).
    pub start: Option<i64>,
    /// Fim do intervalo, inclusive (padrão: último quadro do projeto).
    pub end: Option<i64>,
    /// De quantos em quantos quadros (padrão: o suficiente para ~12 miniaturas).
    pub step: Option<i64>,
    /// Pasta onde gravar os PNGs. Padrão: uma pasta temporária do servidor.
    pub output_dir: Option<String>,
    /// Montar também uma única imagem com todos os quadros em grade. Padrão: true.
    #[serde(default = "verdadeiro")]
    pub contact_sheet: bool,
    /// Colunas da grade (1 a 8). Padrão: 3.
    #[serde(default = "tres")]
    pub columns: u32,
    /// Largura de cada miniatura na folha, em pixels. Padrão: 480.
    #[serde(default = "quatrocentos_e_oitenta")]
    pub thumb_width: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreviewGifParams {
    /// Caminho do projeto. Se omitido, usa o projeto aberto na sessão.
    pub project_path: Option<String>,
    /// Primeiro quadro (padrão: 0).
    pub start: Option<i64>,
    /// Último quadro, inclusive (padrão: o último do projeto).
    pub end: Option<i64>,
    /// De quantos em quantos quadros do projeto entra um quadro no GIF
    /// (padrão: 1, ou mais se o trecho passar de 150 quadros).
    pub step: Option<i64>,
    /// Quadros por segundo do GIF. Padrão: a velocidade natural do projeto.
    pub fps: Option<f64>,
    /// Largura do GIF em pixels. Padrão: 480.
    #[serde(default = "quatrocentos_e_oitenta")]
    pub width: u32,
    /// Caminho do `.gif` de saída. Padrão: uma pasta temporária do servidor.
    pub output_path: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
}

fn tres() -> u32 {
    3
}

fn quatrocentos_e_oitenta() -> u32 {
    480
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
    /// Cor em hexadecimal `#RRGGBB` (ex. "#ff0000").
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
    /// Trilha de baixo (a que sai, no dissolve).
    pub track_a: String,
    /// Trilha de cima (a que entra, no dissolve).
    pub track_b: String,
    /// Primeiro quadro da timeline em que a transição vale (inclusive).
    /// Informe junto com `out_frame`; sem os dois, vale na timeline inteira.
    pub in_frame: Option<i64>,
    /// Último quadro da timeline em que a transição vale (inclusive).
    pub out_frame: Option<i64>,
    /// Id da composição no Kdenlive (ex. "dissolve", "wipe"). Se omitido,
    /// `luma` vira "dissolve" (ou "wipe" com o parâmetro `resource`).
    pub kdenlive_id: Option<String>,
    /// Propriedades extras da transição (ex. {"softness": "0.2"},
    /// {"resource": "/caminho/wipe.pgm"}, {"reverse": "1"}).
    pub params: Option<std::collections::BTreeMap<String, String>>,
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
    /// Quadro do marcador: da timeline (marcador geral) ou da mídia do clipe
    /// (se `clip_ref` for dado; 0 = início do arquivo de mídia).
    pub position: i64,
    pub comment: String,
    /// Se for um marcador de um clipe específico, em vez de um marcador
    /// geral da timeline: `"<track_id>:<índice>"` (ou o id de um clipe da bin).
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineMoveClipParams {
    pub clip_ref: String,
    /// Nova posição, em quadros, na mesma trilha.
    pub new_position: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineTrimClipParams {
    pub clip_ref: String,
    pub in_frame: i64,
    pub out_frame: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineSplitClipParams {
    pub clip_ref: String,
    /// Quadro do **produtor** onde cortar (não uma posição absoluta da
    /// timeline): precisa estar estritamente depois do `in` e até o `out`
    /// do clipe.
    pub split_at: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineSeekParams {
    pub position: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineSetZoomParams {
    /// 0.1 a 10.0.
    pub zoom: f64,
}

// -- Fila de render -----------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderQueueAddParams {
    /// Caminho do projeto. Se omitido, usa o projeto aberto na sessão: o
    /// XML é copiado para um arquivo temporário já neste momento (adicionar
    /// à fila), não quando o job realmente começa a rodar — edições feitas
    /// na sessão depois de adicionar não afetam este job.
    pub project_path: Option<String>,
    pub output_path: String,
    #[serde(default = "preset_padrao")]
    pub preset: String,
    #[serde(default)]
    pub overwrite: bool,
}

// -- Títulos ------------------------------------------------------------------

fn familia_padrao() -> String {
    "Sans".to_string()
}

fn tamanho_fonte_padrao() -> i64 {
    48
}

fn cor_titulo_padrao() -> String {
    "0xffffffff".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TitleAddParams {
    pub text: String,
    /// Duração do clipe de título, em quadros.
    pub duration_frames: i64,
    #[serde(default = "familia_padrao")]
    pub font_family: String,
    #[serde(default = "tamanho_fonte_padrao")]
    pub font_size: i64,
    /// Cor no formato `0xRRGGBBAA` (ex. branco opaco: `0xffffffff`).
    #[serde(default = "cor_titulo_padrao")]
    pub color: String,
    /// Negrito.
    pub bold: Option<bool>,
    /// Alinhamento do texto na caixa: "left", "center" (padrão) ou "right".
    pub align: Option<String>,
    /// Borda esquerda da caixa de texto, em px (padrão 0).
    pub x: Option<i64>,
    /// Centro vertical do texto, em px (padrão: centro do quadro).
    pub y: Option<i64>,
    /// Largura da caixa de texto, em px (padrão: até a borda direita).
    pub box_width: Option<i64>,
    /// Espessura do contorno das letras, em px (padrão 0 = sem contorno).
    pub outline_width: Option<f64>,
    /// Cor do contorno (padrão preto).
    pub outline_color: Option<String>,
    /// Cor da caixa de fundo atrás do texto (ex. "#000000AA"). Sem ela, sem caixa.
    pub background_color: Option<String>,
    /// Folga da caixa de fundo em volta do texto, em px (padrão 24).
    pub padding: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TitleUpdateParams {
    /// Id do clipe de título (da bin).
    pub clip_id: String,
    pub text: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<i64>,
    pub color: Option<String>,
}

// -- Keyframes ------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeyframeSetParams {
    pub clip_ref: String,
    pub filter_id: String,
    pub param: String,
    pub frame: i64,
    pub value: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeyframeValue {
    pub frame: i64,
    /// Valor bruto que o filtro espera naquele quadro (ex. "-12", "0 0 1920 1080 1").
    pub value: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeyframeSetManyParams {
    pub clip_ref: String,
    pub filter_id: String,
    pub param: String,
    pub keyframes: Vec<KeyframeValue>,
    /// `true`: `frame` conta do início do clipe na timeline. `false` (padrão):
    /// quadro da mídia, como em keyframe_set.
    pub relative_to_clip: Option<bool>,
    /// `true`: apaga os keyframes anteriores do parâmetro. Padrão `false`.
    pub replace: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipFadeParams {
    pub clip_ref: String,
    /// "in" (entrada) ou "out" (saída).
    pub kind: String,
    /// Duração do fade, em quadros.
    pub duration_frames: i64,
    /// Só vídeo: "alpha" (padrão; transparência, mostra a trilha de baixo) ou
    /// "black" (do/para o preto). Ignorado em trilhas de áudio.
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TransformKeyframeParams {
    /// Quadro relativo ao início do clipe (0 = primeiro quadro do clipe).
    pub frame: i64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// 0 (transparente) a 1 (opaco). Padrão 1.
    pub opacity: Option<f64>,
    /// Rotação em graus, em torno do centro.
    pub rotation: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipAnimateTransformParams {
    pub clip_ref: String,
    pub keyframes: Vec<TransformKeyframeParams>,
    /// "linear" (padrão), "smooth" (suave) ou "hold" (degrau).
    pub interpolation: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KdenliveLiveParams {
    /// `true` liga: cada edição regrava o projeto e o Kdenlive aberto recarrega
    /// sozinho. `false` desliga.
    pub enabled: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KdenliveOpenParams {
    /// Projeto a abrir. Padrão: o projeto desta sessão (precisa ter sido salvo).
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeyframeListParams {
    pub clip_ref: String,
    pub filter_id: String,
    pub param: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KeyframeRemoveParams {
    pub clip_ref: String,
    pub filter_id: String,
    pub param: String,
    pub frame: i64,
}

// -- Legendas -------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubtitleImportSrtParams {
    pub path: String,
    /// Trilha onde colocar os clipes de título gerados. Se omitida, uma
    /// nova trilha de vídeo é criada.
    pub track_id: Option<String>,
    #[serde(default = "familia_padrao")]
    pub font_family: String,
    #[serde(default = "tamanho_fonte_padrao")]
    pub font_size: i64,
    #[serde(default = "cor_titulo_padrao")]
    pub color: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubtitleExportSrtParams {
    /// Trilha de onde ler os clipes de título (texto) a exportar.
    pub track_id: String,
    pub path: String,
}

// -- Áudio ----------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AudioSetLevelsParams {
    pub clip_ref: String,
    /// Nível em dB (ex. `-6.0`, `0.0`, `3.0`).
    pub db: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AudioNormalizeParams {
    pub clip_ref: String,
    /// Amplitude alvo em dB (padrão do próprio filtro MLT: -12.0).
    #[serde(default = "normalize_padrao")]
    pub target_db: f64,
}

fn normalize_padrao() -> f64 {
    -12.0
}

// -- Transformação de clipe ------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipSetTransformParams {
    pub clip_ref: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Graus de rotação (opcional).
    pub rotation: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipReframeVerticalParams {
    /// Referência do clipe na timeline, `"<track_id>:<índice>"`.
    pub clip_ref: String,
    /// `"fill"` (padrão): preenche o quadro e corta o que sobra, sem faixas
    /// pretas. `"fit"`: o vídeo inteiro aparece, com faixas nas sobras.
    #[serde(default = "modo_preencher")]
    pub mode: String,
    /// Em `fill`, qual parte do vídeo fica visível na horizontal: 0 = borda
    /// esquerda, 0.5 = centro (padrão), 1 = borda direita.
    #[serde(default = "meio")]
    pub focus_x: f64,
    /// O mesmo na vertical (importa quando o vídeo é mais alto que o quadro):
    /// 0 = topo, 0.5 = centro (padrão), 1 = base.
    #[serde(default = "meio")]
    pub focus_y: f64,
}

fn modo_preencher() -> String {
    "fill".to_string()
}

fn meio() -> f64 {
    0.5
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AudioDuckParams {
    /// Clipe a abaixar (a música), `"<track_id>:<índice>"`. Informe este **ou** `track_id`.
    pub clip_ref: Option<String>,
    /// Trilha a abaixar (todos os clipes dela). Informe este **ou** `clip_ref`.
    pub track_id: Option<String>,
    /// Trilha da voz: enquanto ela tem som, o alvo fica abaixado.
    pub voice_track_id: String,
    /// Quanto abaixar, em dB (positivo). Padrão: 12.
    #[serde(default = "duck_db_padrao")]
    pub duck_db: f64,
    /// Quadros da descida antes da fala começar. Padrão: 10. Mínimo: 1.
    #[serde(default = "attack_padrao")]
    pub attack_frames: i64,
    /// Quadros da subida depois da fala acabar. Padrão: 25. Mínimo: 1.
    #[serde(default = "release_padrao")]
    pub release_frames: i64,
    /// Se `true`, só conta como fala o que está acima do limiar dentro dos
    /// clipes da voz (pausas entre as frases soltam o volume); se `false`
    /// (padrão), qualquer parte com clipe na trilha da voz abaixa o alvo.
    #[serde(default)]
    pub detect_speech: bool,
    /// Com `detect_speech`: nível, em dB, abaixo do qual é silêncio. Padrão: -35.
    #[serde(default = "limiar_padrao")]
    pub silence_threshold_db: f64,
    /// Com `detect_speech`: duração mínima, em segundos, de uma pausa para
    /// ela soltar o volume. Padrão: 0.5.
    #[serde(default = "pausa_padrao")]
    pub min_silence_seconds: f64,
}

fn duck_db_padrao() -> f64 {
    12.0
}

fn attack_padrao() -> i64 {
    10
}

fn release_padrao() -> i64 {
    25
}

fn limiar_padrao() -> f64 {
    -35.0
}

fn pausa_padrao() -> f64 {
    0.5
}

// -- Miniatura --------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportThumbnailParams {
    /// Caminho de um vídeo já renderizado (não um projeto MLT).
    pub video_path: String,
    pub output_path: String,
    /// Instante, em segundos, de onde extrair a miniatura. Padrão: 1.0.
    #[serde(default = "um_segundo")]
    pub at_seconds: f64,
    #[serde(default)]
    pub overwrite: bool,
}

fn um_segundo() -> f64 {
    1.0
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
