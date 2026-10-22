//! Tabela de perfis de vídeo (resolução, taxa de quadros, proporção), cada
//! linha verificada rodando `melt -query "profile=<nome>"` neste ambiente —
//! são os mesmos perfis que o libmlt (e portanto o Kdenlive) reconhece
//! nativamente. Nenhum perfil "amigável" inventado: o `name` daqui é
//! exatamente o identificador real do libmlt.

use std::fmt::Write as _;

/// Um perfil de vídeo com todos os campos que o elemento `<profile>` do MLT
/// XML usa.
#[derive(Debug, Clone, Copy)]
pub struct ProfileDef {
    pub name: &'static str,
    pub description: &'static str,
    pub width: u32,
    pub height: u32,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    pub progressive: u8,
    pub sample_aspect_num: u32,
    pub sample_aspect_den: u32,
    pub display_aspect_num: u32,
    pub display_aspect_den: u32,
    pub colorspace: u32,
}

/// Perfil padrão de um projeto novo, quando nenhum é especificado.
pub const DEFAULT_PROFILE: &str = "atsc_1080p_25";

/// Tabela completa, extraída de `melt -query profiles` + `melt -query
/// "profile=<nome>"` para cada um, em 22/10/2026 (ver docs/decisoes.md).
pub const PROFILES: &[ProfileDef] = &[
    ProfileDef { name: "atsc_1080i_50", description: "HD 1080i 25 fps", width: 1920, height: 1080, frame_rate_num: 25, frame_rate_den: 1, progressive: 0, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080i_5994", description: "HD 1080i 29.97 fps", width: 1920, height: 1080, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 0, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080i_60", description: "HD 1080i 30 fps", width: 1920, height: 1080, frame_rate_num: 30, frame_rate_den: 1, progressive: 0, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080p_2398", description: "HD 1080p 23.98 fps", width: 1920, height: 1080, frame_rate_num: 24000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080p_24", description: "HD 1080p 24 fps", width: 1920, height: 1080, frame_rate_num: 24, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080p_25", description: "HD 1080p 25 fps", width: 1920, height: 1080, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080p_2997", description: "HD 1080p 29.97 fps", width: 1920, height: 1080, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080p_30", description: "HD 1080p 30 fps", width: 1920, height: 1080, frame_rate_num: 30, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080p_50", description: "HD 1080p 50 fps", width: 1920, height: 1080, frame_rate_num: 50, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080p_5994", description: "HD 1080p 59.94 fps", width: 1920, height: 1080, frame_rate_num: 60000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_1080p_60", description: "HD 1080p 60 fps", width: 1920, height: 1080, frame_rate_num: 60, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_720p_2398", description: "HD 720p 23.98 fps", width: 1280, height: 720, frame_rate_num: 24000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_720p_24", description: "HD 720p 24 fps", width: 1280, height: 720, frame_rate_num: 24, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_720p_25", description: "HD 720p 25 fps", width: 1280, height: 720, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_720p_2997", description: "HD 720p 29.97 fps", width: 1280, height: 720, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_720p_30", description: "HD 720p 30 fps", width: 1280, height: 720, frame_rate_num: 30, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_720p_50", description: "HD 720p 50 fps", width: 1280, height: 720, frame_rate_num: 50, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_720p_5994", description: "HD 720p 59.94 fps", width: 1280, height: 720, frame_rate_num: 60000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "atsc_720p_60", description: "HD 720p 60 fps", width: 1280, height: 720, frame_rate_num: 60, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "cif_15", description: "CIF 15 fps", width: 352, height: 288, frame_rate_num: 15, frame_rate_den: 1, progressive: 1, sample_aspect_num: 59, sample_aspect_den: 54, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "cif_ntsc", description: "CIF NTSC", width: 352, height: 288, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 10, sample_aspect_den: 11, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "cif_pal", description: "CIF PAL", width: 352, height: 288, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 59, sample_aspect_den: 54, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "cvd_ntsc", description: "CVD NTSC", width: 352, height: 480, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 0, sample_aspect_num: 20, sample_aspect_den: 11, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "cvd_pal", description: "CVD PAL", width: 352, height: 576, frame_rate_num: 25, frame_rate_den: 1, progressive: 0, sample_aspect_num: 59, sample_aspect_den: 27, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "dv_ntsc", description: "DV/DVD NTSC", width: 720, height: 480, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 0, sample_aspect_num: 8, sample_aspect_den: 9, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "dv_ntsc_wide", description: "DV/DVD Widescreen NTSC", width: 720, height: 480, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 0, sample_aspect_num: 32, sample_aspect_den: 27, display_aspect_num: 16, display_aspect_den: 9, colorspace: 601 },
    ProfileDef { name: "dv_pal", description: "DV/DVD PAL", width: 720, height: 576, frame_rate_num: 25, frame_rate_den: 1, progressive: 0, sample_aspect_num: 16, sample_aspect_den: 15, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "dv_pal_wide", description: "DV/DVD Widescreen PAL", width: 720, height: 576, frame_rate_num: 25, frame_rate_den: 1, progressive: 0, sample_aspect_num: 64, sample_aspect_den: 45, display_aspect_num: 16, display_aspect_den: 9, colorspace: 601 },
    ProfileDef { name: "hdv_1080_25p", description: "HDV 1440x1080p 25 fps", width: 1440, height: 1080, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 4, sample_aspect_den: 3, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "hdv_1080_30p", description: "HDV 1440x1080p 29.97 fps", width: 1440, height: 1080, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 4, sample_aspect_den: 3, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "hdv_1080_50i", description: "HDV 1440x1080i 25 fps", width: 1440, height: 1080, frame_rate_num: 25, frame_rate_den: 1, progressive: 0, sample_aspect_num: 4, sample_aspect_den: 3, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "hdv_1080_60i", description: "HDV 1440x1080i 29.97 fps", width: 1440, height: 1080, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 0, sample_aspect_num: 4, sample_aspect_den: 3, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "hdv_720_25p", description: "HD 720p 25 fps", width: 1280, height: 720, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "hdv_720_30p", description: "HD 720p 29.97 fps", width: 1280, height: 720, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "hdv_720_50p", description: "HD 720p 50 fps", width: 1280, height: 720, frame_rate_num: 50, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "hdv_720_60p", description: "HD 720p 59.94 fps", width: 1280, height: 720, frame_rate_num: 60000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "qcif_15", description: "QCIF 15 fps", width: 176, height: 144, frame_rate_num: 15, frame_rate_den: 1, progressive: 1, sample_aspect_num: 59, sample_aspect_den: 54, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "qcif_ntsc", description: "QCIF NTSC", width: 176, height: 144, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 10, sample_aspect_den: 11, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "qcif_pal", description: "QCIF PAL", width: 176, height: 144, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 59, sample_aspect_den: 54, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "qhd_1440p_2398", description: "2.5K QHD 1440p 23.98 fps", width: 2560, height: 1440, frame_rate_num: 24000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "qhd_1440p_24", description: "2.5K QHD 1440p 24 fps", width: 2560, height: 1440, frame_rate_num: 24, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "qhd_1440p_25", description: "2.5K QHD 1440p 25 fps", width: 2560, height: 1440, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "qhd_1440p_2997", description: "2.5K QHD 1440p 29.97 fps", width: 2560, height: 1440, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "qhd_1440p_30", description: "2.5K QHD 1440p 30 fps", width: 2560, height: 1440, frame_rate_num: 30, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "qhd_1440p_50", description: "2.5K QHD 1440p 50 fps", width: 2560, height: 1440, frame_rate_num: 50, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "qhd_1440p_5994", description: "2.5K QHD 1440p 59.94 fps", width: 2560, height: 1440, frame_rate_num: 60000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "qhd_1440p_60", description: "2.5K QHD 1440p 60 fps", width: 2560, height: 1440, frame_rate_num: 60, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "quarter_15", description: "QVGA 15 fps", width: 320, height: 240, frame_rate_num: 15, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "quarter_ntsc", description: "QVGA 29.97 fps", width: 320, height: 240, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "quarter_ntsc_wide", description: "QVGA Widescreen 29.97 fps", width: 426, height: 240, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 601 },
    ProfileDef { name: "quarter_pal", description: "384x288 4:3 PAL", width: 384, height: 288, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "quarter_pal_wide", description: "512x288 16:9 PAL", width: 512, height: 288, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 601 },
    ProfileDef { name: "sdi_486i_5994", description: "NTSC 29.97 fps", width: 720, height: 486, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 0, sample_aspect_num: 8, sample_aspect_den: 9, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "sdi_486p_2398", description: "NTSC 23.98 fps", width: 720, height: 486, frame_rate_num: 24000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 8, sample_aspect_den: 9, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "square_1080p_30", description: "Square 1080p 30 fps", width: 1080, height: 1080, frame_rate_num: 30, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 1, display_aspect_den: 1, colorspace: 709 },
    ProfileDef { name: "square_1080p_60", description: "Square 1080p 60 fps", width: 1080, height: 1080, frame_rate_num: 60, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 1, display_aspect_den: 1, colorspace: 709 },
    ProfileDef { name: "square_ntsc", description: "VGA NTSC", width: 640, height: 480, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "square_ntsc_wide", description: "VGA Widescreen NTSC", width: 854, height: 480, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 601 },
    ProfileDef { name: "square_pal", description: "768x576 4:3 PAL", width: 768, height: 576, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "square_pal_wide", description: "1024x576 16:9 PAL", width: 1024, height: 576, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 601 },
    ProfileDef { name: "svcd_ntsc", description: "SVCD NTSC", width: 480, height: 480, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 0, sample_aspect_num: 15, sample_aspect_den: 11, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "svcd_ntsc_wide", description: "SVCD Widescreen NTSC", width: 480, height: 480, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 0, sample_aspect_num: 20, sample_aspect_den: 11, display_aspect_num: 16, display_aspect_den: 9, colorspace: 601 },
    ProfileDef { name: "svcd_pal", description: "SVCD PAL", width: 480, height: 576, frame_rate_num: 25, frame_rate_den: 1, progressive: 0, sample_aspect_num: 59, sample_aspect_den: 36, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "svcd_pal_wide", description: "SVCD Widescreen PAL", width: 480, height: 576, frame_rate_num: 25, frame_rate_den: 1, progressive: 0, sample_aspect_num: 59, sample_aspect_den: 27, display_aspect_num: 16, display_aspect_den: 9, colorspace: 601 },
    ProfileDef { name: "uhd_2160p_2398", description: "4K UHD 2160p 23.98 fps", width: 3840, height: 2160, frame_rate_num: 24000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "uhd_2160p_24", description: "4K UHD 2160p 24 fps", width: 3840, height: 2160, frame_rate_num: 24, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "uhd_2160p_25", description: "4K UHD 2160p 25 fps", width: 3840, height: 2160, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "uhd_2160p_2997", description: "4K UHD 2160p 29.97 fps", width: 3840, height: 2160, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "uhd_2160p_30", description: "4K UHD 2160p 30 fps", width: 3840, height: 2160, frame_rate_num: 30, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "uhd_2160p_50", description: "4K UHD 2160p 50 fps", width: 3840, height: 2160, frame_rate_num: 50, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "uhd_2160p_5994", description: "4K UHD 2160p 59.94 fps", width: 3840, height: 2160, frame_rate_num: 60000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "uhd_2160p_60", description: "4K UHD 2160p 60 fps", width: 3840, height: 2160, frame_rate_num: 60, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 16, display_aspect_den: 9, colorspace: 709 },
    ProfileDef { name: "vcd_ntsc", description: "VCD NTSC", width: 352, height: 240, frame_rate_num: 30000, frame_rate_den: 1001, progressive: 1, sample_aspect_num: 10, sample_aspect_den: 11, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "vcd_pal", description: "VCD PAL", width: 352, height: 288, frame_rate_num: 25, frame_rate_den: 1, progressive: 1, sample_aspect_num: 59, sample_aspect_den: 54, display_aspect_num: 4, display_aspect_den: 3, colorspace: 601 },
    ProfileDef { name: "vertical_hd_30", description: "Vertical HD 30 fps", width: 1080, height: 1920, frame_rate_num: 30, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 9, display_aspect_den: 16, colorspace: 709 },
    ProfileDef { name: "vertical_hd_60", description: "Vertical HD 60 fps", width: 1080, height: 1920, frame_rate_num: 60, frame_rate_den: 1, progressive: 1, sample_aspect_num: 1, sample_aspect_den: 1, display_aspect_num: 9, display_aspect_den: 16, colorspace: 709 },
];

/// Busca um perfil pelo nome exato (identificador real do libmlt).
pub fn find(name: &str) -> Option<&'static ProfileDef> {
    PROFILES.iter().find(|p| p.name == name)
}

/// Lista todos os nomes de perfil válidos, na ordem da tabela.
pub fn profile_names() -> Vec<&'static str> {
    PROFILES.iter().map(|p| p.name).collect()
}

impl ProfileDef {
    /// Serializa este perfil como o elemento `<profile .../>` do MLT XML.
    pub fn to_xml_attrs(&self) -> Vec<(String, String)> {
        vec![
            ("description".to_string(), self.description.to_string()),
            ("width".to_string(), self.width.to_string()),
            ("height".to_string(), self.height.to_string()),
            ("progressive".to_string(), self.progressive.to_string()),
            ("sample_aspect_num".to_string(), self.sample_aspect_num.to_string()),
            ("sample_aspect_den".to_string(), self.sample_aspect_den.to_string()),
            ("display_aspect_num".to_string(), self.display_aspect_num.to_string()),
            ("display_aspect_den".to_string(), self.display_aspect_den.to_string()),
            ("frame_rate_num".to_string(), self.frame_rate_num.to_string()),
            ("frame_rate_den".to_string(), self.frame_rate_den.to_string()),
            ("colorspace".to_string(), self.colorspace.to_string()),
        ]
    }

    /// Um resumo de uma linha, por exemplo `"1920x1080, 25 fps, 16:9"`.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        let fps = self.frame_rate_num as f64 / self.frame_rate_den as f64;
        let _ = write!(
            s,
            "{}x{}, {:.2} fps, {}:{}",
            self.width, self.height, fps, self.display_aspect_num, self.display_aspect_den
        );
        s
    }
}
