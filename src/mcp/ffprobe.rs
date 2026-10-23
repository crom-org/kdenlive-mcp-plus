//! Sondagem de duração de mídia via `ffprobe` — um subprocesso comum, sem
//! nenhuma ligação com o libmlt (não quebra o isolamento da decisão D-003).
//! Usado para relatar a duração de um render (Fase 0) e para estimar o
//! `out` de um clipe importado para a bin (Fase 1).

use std::path::Path;

/// Duração em segundos, via `ffprobe`. Se o `ffprobe` não existir no PATH,
/// ou falhar, não é tratado como erro fatal pelo chamador: devolve
/// `(None, Some(nota))` explicando o motivo.
pub async fn probe_duration_seconds(path: &Path) -> (Option<f64>, Option<String>) {
    let resultado = tokio::process::Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("csv=p=0")
        .arg(path)
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
                         veja o arquivo diretamente"
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

/// Duração estimada em quadros, na taxa `fps` dada. `None` se a sondagem
/// falhar por qualquer motivo (ver `probe_duration_seconds`).
pub async fn probe_duration_frames(path: &Path, fps: f64) -> Option<i64> {
    let (segundos, _nota) = probe_duration_seconds(path).await;
    segundos.map(|s| (s * fps).round() as i64)
}

/// Tamanho de exibição (largura, altura) do primeiro fluxo de vídeo de um
/// arquivo, em pixels: já com a proporção de pixel aplicada e a rotação
/// (celular em pé) considerada — o que o produtor `avformat` do MLT entrega.
pub async fn probe_video_size(path: &Path) -> Result<(f64, f64), String> {
    let saida = tokio::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries"])
        .arg("stream=width,height,sample_aspect_ratio:stream_side_data=rotation")
        .args(["-of", "json"])
        .arg(path)
        .output()
        .await
        .map_err(|e| format!("não consegui executar o ffprobe ({e}); ele é necessário para medir o vídeo"))?;
    if !saida.status.success() {
        return Err(format!(
            "o ffprobe não conseguiu ler {}: {}",
            path.display(),
            String::from_utf8_lossy(&saida.stderr).trim()
        ));
    }
    parse_video_size(&String::from_utf8_lossy(&saida.stdout))
        .ok_or_else(|| format!("{} não tem um fluxo de vídeo (ou imagem) para reenquadrar", path.display()))
}

/// Extrai o tamanho de exibição da saída JSON do `ffprobe` (ver
/// [`probe_video_size`]). `None` se não houver fluxo de vídeo com tamanho.
pub fn parse_video_size(json: &str) -> Option<(f64, f64)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let stream = v.get("streams")?.as_array()?.first()?;
    let mut largura = stream.get("width")?.as_f64()?;
    let mut altura = stream.get("height")?.as_f64()?;
    if largura <= 0.0 || altura <= 0.0 {
        return None;
    }
    // proporção de pixel ("N/A", "0:1" e "1:1" não mudam nada)
    if let Some(sar) = stream.get("sample_aspect_ratio").and_then(|s| s.as_str())
        && let Some((n, d)) = sar.split_once(':')
        && let (Ok(n), Ok(d)) = (n.parse::<f64>(), d.parse::<f64>())
        && n > 0.0
        && d > 0.0
    {
        largura *= n / d;
    }
    // vídeo gravado em pé: o ffmpeg guarda 90/270 graus de rotação
    let girado = stream
        .get("side_data_list")
        .and_then(|l| l.as_array())
        .into_iter()
        .flatten()
        .filter_map(|d| d.get("rotation").and_then(|r| r.as_f64()))
        .any(|graus| (graus.abs() % 180.0 - 90.0).abs() < 0.5);
    if girado {
        std::mem::swap(&mut largura, &mut altura);
    }
    Some((largura, altura))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamanho_simples() {
        let json = r#"{"streams":[{"width":640,"height":360,"sample_aspect_ratio":"1:1"}]}"#;
        assert_eq!(parse_video_size(json), Some((640.0, 360.0)));
        // sem proporção de pixel informada
        assert_eq!(parse_video_size(r#"{"streams":[{"width":640,"height":360}]}"#), Some((640.0, 360.0)));
        assert_eq!(parse_video_size(r#"{"streams":[{"width":640,"height":360,"sample_aspect_ratio":"0:1"}]}"#), Some((640.0, 360.0)));
    }

    #[test]
    fn pixel_nao_quadrado_alarga_a_largura() {
        // 720x576 com SAR 16:15 (DVD PAL 4:3): 720*16/15 = 768
        let json = r#"{"streams":[{"width":720,"height":576,"sample_aspect_ratio":"16:15"}]}"#;
        assert_eq!(parse_video_size(json), Some((768.0, 576.0)));
    }

    #[test]
    fn rotacao_de_90_graus_troca_largura_e_altura() {
        let json = r#"{"streams":[{"width":1920,"height":1080,"side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]}]}"#;
        assert_eq!(parse_video_size(json), Some((1080.0, 1920.0)));
        let sem_giro = r#"{"streams":[{"width":1920,"height":1080,"side_data_list":[{"rotation":180}]}]}"#;
        assert_eq!(parse_video_size(sem_giro), Some((1920.0, 1080.0)));
    }

    #[test]
    fn sem_fluxo_de_video_e_none() {
        assert_eq!(parse_video_size(r#"{"streams":[]}"#), None);
        assert_eq!(parse_video_size("lixo"), None);
    }
}
