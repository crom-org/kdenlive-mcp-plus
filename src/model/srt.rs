//! Leitura e escrita do formato SRT (legendas) — um formato de texto
//! público e bem documentado, não uma convenção interna do Kdenlive. Usado
//! por `subtitle_import_srt`/`subtitle_export_srt`, que criam/leem clipes
//! de título (`document::add_title_clip`) numa trilha.

#[derive(Debug, Clone, PartialEq)]
pub struct SrtEntry {
    pub start_frame: i64,
    pub end_frame: i64,
    pub text: String,
}

/// Lê um arquivo `.srt`. Tolerante a `\r\n` e a linhas de índice ausentes
/// ou fora de ordem (ignora o número do índice; usa só o timing e o
/// texto).
pub fn parse(texto: &str, fps: f64) -> Result<Vec<SrtEntry>, String> {
    let normalizado = texto.replace("\r\n", "\n");
    let mut entradas = Vec::new();

    for bloco in normalizado.split("\n\n") {
        let bloco = bloco.trim();
        if bloco.is_empty() {
            continue;
        }
        let mut linhas = bloco.lines();

        let primeira = linhas.next().ok_or("bloco SRT vazio")?;
        // Se a primeira linha for só um número (índice), pula para a próxima
        // (o timing); senão, assume que a própria primeira linha já é o timing.
        let linha_timing = if primeira.trim().chars().all(|c| c.is_ascii_digit()) {
            linhas.next().ok_or_else(|| format!("bloco sem linha de timing depois do índice \"{primeira}\""))?
        } else {
            primeira
        };

        let (inicio_texto, fim_texto) = linha_timing
            .split_once("-->")
            .ok_or_else(|| format!("linha de timing inválida (esperava \"-->\"): \"{linha_timing}\""))?;
        let start_frame = timestamp_to_frames(inicio_texto.trim(), fps)
            .ok_or_else(|| format!("timestamp de início inválido: \"{inicio_texto}\""))?;
        let end_frame = timestamp_to_frames(fim_texto.trim(), fps)
            .ok_or_else(|| format!("timestamp de fim inválido: \"{fim_texto}\""))?;

        let texto_legenda: String = linhas.collect::<Vec<_>>().join("\n");
        if texto_legenda.trim().is_empty() {
            return Err(format!("bloco SRT sem texto de legenda (timing: \"{linha_timing}\")"));
        }

        entradas.push(SrtEntry { start_frame, end_frame, text: texto_legenda });
    }

    if entradas.is_empty() {
        return Err("nenhuma entrada de legenda encontrada no arquivo".to_string());
    }
    Ok(entradas)
}

/// Escreve uma lista de entradas como texto `.srt`.
pub fn write(entradas: &[SrtEntry], fps: f64) -> String {
    let mut saida = String::new();
    for (i, entrada) in entradas.iter().enumerate() {
        saida.push_str(&(i + 1).to_string());
        saida.push('\n');
        saida.push_str(&frames_to_timestamp(entrada.start_frame, fps));
        saida.push_str(" --> ");
        saida.push_str(&frames_to_timestamp(entrada.end_frame, fps));
        saida.push('\n');
        saida.push_str(&entrada.text);
        saida.push_str("\n\n");
    }
    saida
}

fn frames_to_timestamp(frame: i64, fps: f64) -> String {
    let total_ms = ((frame.max(0) as f64) / fps * 1000.0).round() as i64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

fn timestamp_to_frames(ts: &str, fps: f64) -> Option<i64> {
    let (hms, ms) = ts.split_once(',').or_else(|| ts.split_once('.'))?;
    let mut partes = hms.split(':');
    let h: i64 = partes.next()?.trim().parse().ok()?;
    let m: i64 = partes.next()?.trim().parse().ok()?;
    let s: i64 = partes.next()?.trim().parse().ok()?;
    let ms: i64 = ms.trim().parse().ok()?;
    let total_ms = (h * 3600 + m * 60 + s) * 1000 + ms;
    Some((total_ms as f64 / 1000.0 * fps).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXEMPLO: &str = "1\n00:00:01,000 --> 00:00:04,000\nOlá, mundo!\n\n2\n00:00:05,500 --> 00:00:07,250\nSegunda linha\ncom duas linhas\n";

    #[test]
    fn le_um_srt_com_duas_entradas() {
        let entradas = parse(EXEMPLO, 25.0).expect("deveria conseguir ler");
        assert_eq!(entradas.len(), 2);
        assert_eq!(entradas[0].start_frame, 25);
        assert_eq!(entradas[0].end_frame, 100);
        assert_eq!(entradas[0].text, "Olá, mundo!");
        assert_eq!(entradas[1].text, "Segunda linha\ncom duas linhas");
    }

    #[test]
    fn escrever_e_ler_de_volta_preserva_os_dados() {
        let original = vec![
            SrtEntry { start_frame: 0, end_frame: 24, text: "primeira".to_string() },
            SrtEntry { start_frame: 50, end_frame: 99, text: "segunda\ncom quebra".to_string() },
        ];
        let texto = write(&original, 25.0);
        let relido = parse(&texto, 25.0).expect("deveria reler o que foi escrito");
        assert_eq!(relido, original);
    }

    #[test]
    fn rejeita_um_srt_vazio() {
        assert!(parse("", 25.0).is_err());
        assert!(parse("   \n\n  ", 25.0).is_err());
    }

    #[test]
    fn rejeita_timing_invalido() {
        let erro = parse("1\nisto nao e um timing\ntexto\n", 25.0).unwrap_err();
        assert!(erro.contains("timing"));
    }
}
