//! Tabela de presets de render (Apêndice A do briefing do projeto) e
//! resolução de nome de preset para opções concretas do avformat.
//!
//! Nenhum nome de preset é passado ao avformat (ver decisão D-005): aqui ele
//! só serve para localizar a linha da tabela. O que chega ao consumidor são
//! sempre `vcodec`/`acodec`/`vb`/`ab` explícitos.

use super::EngineError;

/// Opções de render já resolvidas, prontas para configurar o consumidor
/// avformat. Nenhum destes valores é o nome de um preset.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub vcodec: Option<&'static str>,
    pub acodec: Option<&'static str>,
    pub vbitrate: Option<&'static str>,
    pub abitrate: Option<&'static str>,
    /// Extensão de arquivo esperada para a saída deste preset, sem o ponto.
    pub extension: &'static str,
    /// Desliga a trilha de áudio no consumidor avformat (propriedade `an=1`).
    pub disable_audio: bool,
    /// Desliga a trilha de vídeo no consumidor avformat (propriedade `vn=1`).
    pub disable_video: bool,
    pub overwrite: bool,
}

struct PresetDef {
    name: &'static str,
    vcodec: Option<&'static str>,
    acodec: Option<&'static str>,
    vbitrate: Option<&'static str>,
    abitrate: Option<&'static str>,
    extension: &'static str,
    disable_audio: bool,
    disable_video: bool,
}

/// Tabela do Apêndice A. Os codecs foram confirmados contra
/// `ffmpeg -encoders` e as propriedades `an`/`vn` contra
/// `melt -query "consumer=avformat"` neste ambiente (ver docs/07-ambiente.md).
const PRESETS: &[PresetDef] = &[
    PresetDef {
        name: "h264_hq",
        vcodec: Some("libx264"),
        acodec: Some("aac"),
        vbitrate: Some("8000k"),
        abitrate: Some("192k"),
        extension: "mp4",
        disable_audio: false,
        disable_video: false,
    },
    PresetDef {
        name: "h264_fast",
        vcodec: Some("libx264"),
        acodec: Some("aac"),
        vbitrate: Some("4000k"),
        abitrate: Some("128k"),
        extension: "mp4",
        disable_audio: false,
        disable_video: false,
    },
    PresetDef {
        name: "h265_hq",
        vcodec: Some("libx265"),
        acodec: Some("aac"),
        vbitrate: Some("6000k"),
        abitrate: Some("192k"),
        extension: "mp4",
        disable_audio: false,
        disable_video: false,
    },
    PresetDef {
        name: "webm_vp9",
        vcodec: Some("libvpx-vp9"),
        acodec: Some("libvorbis"),
        vbitrate: Some("5000k"),
        abitrate: Some("192k"),
        extension: "webm",
        disable_audio: false,
        disable_video: false,
    },
    PresetDef {
        name: "prores",
        vcodec: Some("prores_ks"),
        acodec: Some("pcm_s16le"),
        vbitrate: None,
        abitrate: None,
        extension: "mov",
        disable_audio: false,
        disable_video: false,
    },
    PresetDef {
        name: "lossless",
        vcodec: Some("ffv1"),
        acodec: Some("flac"),
        vbitrate: None,
        abitrate: None,
        extension: "mkv",
        disable_audio: false,
        disable_video: false,
    },
    PresetDef {
        name: "gif",
        vcodec: Some("gif"),
        acodec: None,
        vbitrate: None,
        abitrate: None,
        extension: "gif",
        disable_audio: true,
        disable_video: false,
    },
    PresetDef {
        name: "audio_only",
        vcodec: None,
        acodec: Some("pcm_s16le"),
        vbitrate: None,
        abitrate: None,
        extension: "wav",
        disable_audio: false,
        disable_video: true,
    },
];

/// Nome do preset padrão usado pela ferramenta MCP `render_project`.
pub const DEFAULT_PRESET: &str = "h264_hq";

/// Lista os nomes de todos os presets válidos, na ordem da tabela.
pub fn preset_names() -> Vec<&'static str> {
    PRESETS.iter().map(|p| p.name).collect()
}

/// Resolve um nome de preset para opções concretas de render.
///
/// Nunca retorna o nome do preset embutido em `RenderOptions`: a partir daqui
/// só existem codec, bitrate e extensão explícitos.
pub fn resolve_preset(name: &str, overwrite: bool) -> Result<RenderOptions, EngineError> {
    let def = PRESETS.iter().find(|p| p.name == name).ok_or_else(|| {
        EngineError::InvalidText(format!(
            "preset desconhecido: \"{name}\". Presets válidos: {}",
            preset_names().join(", ")
        ))
    })?;

    Ok(RenderOptions {
        vcodec: def.vcodec,
        acodec: def.acodec,
        vbitrate: def.vbitrate,
        abitrate: def.abitrate,
        extension: def.extension,
        disable_audio: def.disable_audio,
        disable_video: def.disable_video,
        overwrite,
    })
}
