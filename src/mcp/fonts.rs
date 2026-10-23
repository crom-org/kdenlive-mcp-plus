//! Resolução de nome de fonte via fontconfig (`fc-match`), um subprocesso
//! comum — sem nenhuma chamada ao libmlt.
//!
//! O Kdenlive só aceita sem aviso um título cuja fonte seja o nome exato de
//! uma família instalada (`QFontInfo(QFont(nome)).family() == nome`, em
//! `DocumentChecker`); apelidos genéricos como "Sans" fazem ele abrir o
//! projeto com o diálogo "Clip Problems → Title Font". Por isso o nome pedido
//! pelo agente é resolvido para a família real antes de ser gravado.

/// Família instalada que o fontconfig usa para `nome` (ex. "Sans" → "Noto
/// Sans"). Se o `fc-match` não existir ou falhar, devolve `nome` como veio.
pub async fn resolve_font_family(nome: &str) -> String {
    let saida = tokio::process::Command::new("fc-match")
        .arg("-f")
        .arg("%{family[0]}")
        .arg(nome)
        .output()
        .await;
    match saida {
        Ok(s) if s.status.success() => {
            let familia = String::from_utf8_lossy(&s.stdout).trim().to_string();
            if familia.is_empty() { nome.to_string() } else { familia }
        }
        _ => nome.to_string(),
    }
}
