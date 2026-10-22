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
