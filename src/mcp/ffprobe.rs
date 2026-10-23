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

/// Trechos com som (fala) de `duracao_s` segundos de um arquivo, a partir de
/// `inicio_s`, como `(início, fim)` em segundos **relativos ao início do
/// trecho** — o complemento dos silêncios que o filtro `silencedetect` do
/// `ffmpeg` encontra (abaixo de `limiar_db`, por pelo menos `silencio_min_s`).
pub async fn probe_speech_intervals(
    path: &Path,
    inicio_s: f64,
    duracao_s: f64,
    limiar_db: f64,
    silencio_min_s: f64,
) -> Result<Vec<(f64, f64)>, String> {
    let saida = tokio::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-v", "info", "-vn"])
        .arg("-ss")
        .arg(format!("{inicio_s:.3}"))
        .arg("-t")
        .arg(format!("{duracao_s:.3}"))
        .arg("-i")
        .arg(path)
        .arg("-af")
        .arg(format!("silencedetect=noise={limiar_db}dB:d={silencio_min_s}"))
        .args(["-f", "null", "-"])
        .output()
        .await
        .map_err(|e| format!("não consegui executar o ffmpeg ({e}); ele é necessário para detectar fala"))?;
    let texto = String::from_utf8_lossy(&saida.stderr);
    if !saida.status.success() {
        return Err(format!(
            "o ffmpeg não conseguiu analisar o áudio de {} (o arquivo tem áudio?): {}",
            path.display(),
            texto.lines().last().unwrap_or("").trim()
        ));
    }
    Ok(parse_silencedetect(&texto, duracao_s))
}

/// Converte a saída do `silencedetect` (`silence_start:`/`silence_end:` no
/// stderr do ffmpeg) nos trechos **com som** de `[0, duracao_s]`.
pub fn parse_silencedetect(stderr: &str, duracao_s: f64) -> Vec<(f64, f64)> {
    let numero_depois = |linha: &str, chave: &str| -> Option<f64> {
        let resto = linha.split(chave).nth(1)?;
        resto.split(|c: char| c.is_whitespace() || c == '|').find(|p| !p.is_empty())?.parse::<f64>().ok()
    };
    let mut silencios: Vec<(f64, f64)> = Vec::new();
    let mut aberto: Option<f64> = None;
    for linha in stderr.lines().filter(|l| l.contains("silencedetect")) {
        if let Some(inicio) = numero_depois(linha, "silence_start:") {
            aberto = Some(inicio.max(0.0));
        } else if let Some(fim) = numero_depois(linha, "silence_end:") {
            silencios.push((aberto.take().unwrap_or(0.0), fim.min(duracao_s)));
        }
    }
    if let Some(inicio) = aberto {
        silencios.push((inicio, duracao_s)); // silêncio até o fim do trecho
    }
    // complemento: o que não é silêncio
    let mut falas: Vec<(f64, f64)> = Vec::new();
    let mut cursor = 0.0;
    for (ini, fim) in silencios {
        if ini > cursor {
            falas.push((cursor, ini));
        }
        cursor = cursor.max(fim);
    }
    if cursor < duracao_s {
        falas.push((cursor, duracao_s));
    }
    falas
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
    fn silencedetect_vira_trechos_com_som() {
        let saida = "[silencedetect @ 0x1] silence_start: 1.00002\n\
                     [silencedetect @ 0x1] silence_end: 2.00002 | silence_duration: 1.00001\n\
                     [silencedetect @ 0x1] silence_start: 3.5\n";
        let falas = parse_silencedetect(saida, 4.0);
        assert_eq!(falas, vec![(0.0, 1.00002), (2.00002, 3.5)]);
    }

    #[test]
    fn silencedetect_sem_silencio_e_tudo_fala_e_so_silencio_e_nada() {
        assert_eq!(parse_silencedetect("sem nada de interesse", 3.0), vec![(0.0, 3.0)]);
        let tudo_silencio = "[silencedetect @ 0x1] silence_start: 0\n";
        assert!(parse_silencedetect(tudo_silencio, 3.0).is_empty());
    }

    #[test]
    fn sem_fluxo_de_video_e_none() {
        assert_eq!(parse_video_size(r#"{"streams":[]}"#), None);
        assert_eq!(parse_video_size("lixo"), None);
    }
}
