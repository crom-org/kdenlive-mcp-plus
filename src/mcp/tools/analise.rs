//! Ferramentas de **análise** do que o agente montou, sem abrir o Kdenlive e
//! sem renderizar o vídeo inteiro: `preview_frames` (vários quadros de uma vez
//! e uma folha de contato com todos eles) e `preview_gif` (uma animação curta
//! de um trecho). Os quadros vêm do mesmo motor do `preview_frame`, em vários
//! processos filhos em paralelo; a montagem da folha e do GIF usa o `ffmpeg`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::engine::worker;
use crate::mcp::params::{PreviewFramesParams, PreviewGifParams};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document;

/// Quantos quadros no máximo por chamada de `preview_frames` / `preview_gif`.
const MAX_QUADROS_FOLHA: usize = 60;
const MAX_QUADROS_GIF: usize = 150;
/// Processos de render rodando ao mesmo tempo (cada um usa bastante memória).
const PARALELO: usize = 3;
/// Quantas miniaturas, mais ou menos, quando o agente não escolhe os quadros.
const MINIATURAS_PADRAO: usize = 12;

/// Escolhe os quadros a renderizar, a partir do que o agente pediu: uma lista
/// (`frames`) **ou** um intervalo (`start`/`end`/`step`); sem nada, espalha
/// cerca de [`MINIATURAS_PADRAO`] quadros pelo projeto todo. `duracao` é o
/// tamanho do projeto em quadros; `limite` o máximo de quadros aceito.
pub fn escolher_quadros(
    frames: Option<&[i64]>,
    start: Option<i64>,
    end: Option<i64>,
    step: Option<i64>,
    duracao: i64,
    limite: usize,
) -> Result<Vec<i64>, String> {
    if duracao <= 0 {
        return Err("o projeto está vazio (duração zero); não há quadros para analisar".to_string());
    }
    let ultimo = duracao - 1;
    let quadros: Vec<i64> = match frames {
        Some(lista) => {
            if start.is_some() || end.is_some() || step.is_some() {
                return Err("informe frames (uma lista) OU start/end/step (um intervalo), não os dois".to_string());
            }
            let mut vistos: Vec<i64> = Vec::new();
            for &q in lista {
                if q < 0 || q > ultimo {
                    return Err(format!("quadro {q} fora do projeto (de 0 a {ultimo})"));
                }
                if !vistos.contains(&q) {
                    vistos.push(q);
                }
            }
            vistos
        }
        None => {
            let inicio = start.unwrap_or(0);
            let fim = end.unwrap_or(ultimo);
            if inicio < 0 || fim < inicio || fim > ultimo {
                return Err(format!(
                    "intervalo inválido: de {inicio} a {fim} (o projeto vai de 0 a {ultimo})"
                ));
            }
            let total = fim - inicio + 1;
            let passo = match step {
                Some(p) if p >= 1 => p,
                Some(p) => return Err(format!("step precisa ser pelo menos 1, recebi {p}")),
                None => (total + MINIATURAS_PADRAO as i64 - 1) / MINIATURAS_PADRAO as i64,
            }
            .max(1);
            let mut lista: Vec<i64> = (inicio..=fim).step_by(passo as usize).collect();
            // o último quadro do intervalo sempre entra (é onde a animação termina)
            if lista.last() != Some(&fim) && lista.len() < limite {
                lista.push(fim);
            }
            lista
        }
    };
    if quadros.is_empty() {
        return Err("nenhum quadro escolhido".to_string());
    }
    if quadros.len() > limite {
        return Err(format!(
            "{} quadros são demais para uma chamada (máximo {limite}); aumente o step ou reduza o intervalo",
            quadros.len()
        ));
    }
    Ok(quadros)
}

/// Pasta padrão das imagens de análise (uma por processo do servidor).
fn pasta_padrao() -> PathBuf {
    std::env::temp_dir().join(format!("kdenlive-mcp-plus-preview-{}", std::process::id()))
}

/// Duração (quadros) e fps do projeto em `caminho`.
fn duracao_e_fps(caminho: &Path) -> Result<(i64, f64), String> {
    let texto = std::fs::read_to_string(caminho).map_err(|e| format!("não consegui ler {}: {e}", caminho.display()))?;
    let doc = document::parse(&texto).map_err(|e| e.to_string())?;
    Ok((document::project_duration(&doc), document::profile_fps(&doc)))
}

/// Linhas de uma grade de `n` miniaturas em `colunas` colunas.
fn linhas_da_grade(n: usize, colunas: usize) -> usize {
    n.div_ceil(colunas.max(1)).max(1)
}

/// Monta a folha de contato com o `ffmpeg`: cada quadro reduzido a `largura`
/// pixels, com o número do quadro e o tempo escritos nele (se o `ffmpeg` tiver
/// o `drawtext`; senão, sem rótulo), em grade de `colunas` colunas.
async fn montar_folha(
    quadros: &[(i64, PathBuf)],
    fps: f64,
    colunas: usize,
    largura: u32,
    saida: &Path,
) -> Result<(), String> {
    async fn rodar(quadros: &[(i64, PathBuf)], fps: f64, colunas: usize, largura: u32, saida: &Path, rotulos: bool) -> Result<(), String> {
        let mut comando = tokio::process::Command::new("ffmpeg");
        comando.args(["-y", "-v", "error"]);
        let mut filtro = String::new();
        for (i, (quadro, caminho)) in quadros.iter().enumerate() {
            comando.arg("-i").arg(caminho);
            let rotulo = if rotulos {
                format!(
                    ",drawtext=text='{quadro}  {:.2}s':x=10:y=8:fontsize=26:fontcolor=white:box=1:boxcolor=black@0.65:boxborderw=6",
                    *quadro as f64 / fps
                )
            } else {
                String::new()
            };
            filtro.push_str(&format!("[{i}:v]scale={largura}:-2{rotulo}[v{i}];"));
        }
        for i in 0..quadros.len() {
            filtro.push_str(&format!("[v{i}]"));
        }
        filtro.push_str(&format!(
            "concat=n={}:v=1:a=0,tile={colunas}x{}:padding=6:margin=6:color=0x1a1a1a[folha]",
            quadros.len(),
            linhas_da_grade(quadros.len(), colunas)
        ));
        let resultado = comando
            .arg("-filter_complex")
            .arg(filtro)
            .args(["-map", "[folha]", "-frames:v", "1"])
            .arg(saida)
            .output()
            .await
            .map_err(|e| format!("não consegui executar o ffmpeg ({e}); ele é necessário para a folha de contato"))?;
        if resultado.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&resultado.stderr).trim().to_string())
        }
    }
    match rodar(quadros, fps, colunas, largura, saida, true).await {
        Ok(()) => Ok(()),
        // sem fonte/`drawtext` no ffmpeg: a folha sai igual, só sem os números
        Err(_) => rodar(quadros, fps, colunas, largura, saida, false).await,
    }
}

/// Monta um GIF animado a partir de PNGs, `fps` quadros por segundo.
async fn montar_gif(quadros: &[PathBuf], fps: f64, largura: u32, saida: &Path) -> Result<(), String> {
    let lista = saida.with_extension("lista.txt");
    let duracao = 1.0 / fps;
    let mut texto = String::new();
    for caminho in quadros {
        texto.push_str(&format!("file '{}'\nduration {duracao:.5}\n", caminho.display()));
    }
    // o concat precisa do último arquivo repetido para respeitar a duração dele
    if let Some(ultimo) = quadros.last() {
        texto.push_str(&format!("file '{}'\n", ultimo.display()));
    }
    std::fs::write(&lista, texto).map_err(|e| format!("não consegui escrever {}: {e}", lista.display()))?;
    let resultado = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-f", "concat", "-safe", "0", "-i"])
        .arg(&lista)
        .arg("-vf")
        .arg(format!(
            "scale={largura}:-2:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4"
        ))
        .args(["-loop", "0"])
        .arg(saida)
        .output()
        .await
        .map_err(|e| format!("não consegui executar o ffmpeg ({e}); ele é necessário para o GIF"));
    let _ = std::fs::remove_file(&lista);
    let resultado = resultado?;
    if resultado.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&resultado.stderr).trim().to_string())
    }
}

impl KdenliveMcpServer {
    /// Renderiza `quadros` (já validados) como PNGs em `pasta`, até
    /// [`PARALELO`] processos ao mesmo tempo, e devolve `(quadro, caminho)` na
    /// ordem pedida. Segura o `render_lock`: não roda junto com um render de verdade.
    async fn renderizar_quadros(
        &self,
        projeto: &Path,
        pasta: &Path,
        quadros: &[i64],
    ) -> Result<Vec<(i64, PathBuf)>, String> {
        std::fs::create_dir_all(pasta).map_err(|e| format!("não consegui criar a pasta {}: {e}", pasta.display()))?;
        let _guard = self.render_lock.lock().await;
        let semaforo = Arc::new(Semaphore::new(PARALELO));
        let mut tarefas: JoinSet<Result<(i64, PathBuf), String>> = JoinSet::new();
        for &quadro in quadros {
            let projeto = projeto.to_path_buf();
            let alvo = pasta.join(format!("frame_{quadro:06}.png"));
            let semaforo = Arc::clone(&semaforo);
            tarefas.spawn(async move {
                let _vaga = semaforo.acquire_owned().await.map_err(|e| e.to_string())?;
                worker::render_frame_isolated(&projeto, &alvo, quadro, true)
                    .await
                    .map(|()| (quadro, alvo))
                    .map_err(|e| format!("quadro {quadro}: {e}"))
            });
        }
        let mut prontos: Vec<(i64, PathBuf)> = Vec::new();
        while let Some(resultado) = tarefas.join_next().await {
            prontos.push(resultado.map_err(|e| format!("tarefa de render interrompida: {e}"))??);
        }
        // a ordem de término varia; devolve na ordem em que o agente pediu
        prontos.sort_by_key(|(q, _)| quadros.iter().position(|x| x == q).unwrap_or(usize::MAX));
        Ok(prontos)
    }

    pub(crate) async fn preview_frames_impl(&self, params: PreviewFramesParams) -> Value {
        let projeto = match self.resolve_project_path_for_render(params.project_path.as_deref()).await {
            Ok(p) => p,
            Err(e) => return e,
        };
        if !projeto.exists() {
            return erro(format!("projeto não encontrado: {}", projeto.display()));
        }
        let (duracao, fps) = match duracao_e_fps(&projeto) {
            Ok(v) => v,
            Err(e) => return erro(e),
        };
        let quadros = match escolher_quadros(
            params.frames.as_deref(),
            params.start,
            params.end,
            params.step,
            duracao,
            MAX_QUADROS_FOLHA,
        ) {
            Ok(q) => q,
            Err(e) => return erro(e),
        };
        let pasta = params.output_dir.as_deref().map(PathBuf::from).unwrap_or_else(pasta_padrao);
        let colunas = params.columns.clamp(1, 8) as usize;
        let largura = params.thumb_width.clamp(120, 1920);

        let prontos = match self.renderizar_quadros(&projeto, &pasta, &quadros).await {
            Ok(p) => p,
            Err(e) => return erro(e),
        };

        let mut folha: Option<String> = None;
        let mut aviso: Option<String> = None;
        if params.contact_sheet {
            let alvo = pasta.join(format!(
                "folha_{}-{}.png",
                quadros.iter().min().copied().unwrap_or(0),
                quadros.iter().max().copied().unwrap_or(0)
            ));
            match montar_folha(&prontos, fps, colunas, largura, &alvo).await {
                Ok(()) => folha = Some(alvo.display().to_string()),
                Err(e) => aviso = Some(format!("os quadros foram renderizados, mas a folha de contato falhou: {e}")),
            }
        }
        json!({
            "ok": true,
            "fps": fps,
            "duracao_quadros": duracao,
            "contact_sheet": folha,
            "grade": json!({
                "colunas": colunas,
                "linhas": linhas_da_grade(prontos.len(), colunas),
                "ordem": "linha por linha, da esquerda para a direita, de cima para baixo",
            }),
            "frames": prontos
                .iter()
                .map(|(q, c)| json!({ "frame": q, "tempo_s": (*q as f64 / fps * 1000.0).round() / 1000.0, "path": c.display().to_string() }))
                .collect::<Vec<_>>(),
            "aviso": aviso,
        })
    }

    pub(crate) async fn preview_gif_impl(&self, params: PreviewGifParams) -> Value {
        let projeto = match self.resolve_project_path_for_render(params.project_path.as_deref()).await {
            Ok(p) => p,
            Err(e) => return e,
        };
        if !projeto.exists() {
            return erro(format!("projeto não encontrado: {}", projeto.display()));
        }
        let (duracao, fps_projeto) = match duracao_e_fps(&projeto) {
            Ok(v) => v,
            Err(e) => return erro(e),
        };
        // sem step: tantos quadros quanto couberem no limite, o mais fiel possível
        let intervalo = (params.end.unwrap_or(duracao - 1) - params.start.unwrap_or(0) + 1).max(1);
        let step = params
            .step
            .unwrap_or_else(|| (intervalo + MAX_QUADROS_GIF as i64 - 1) / MAX_QUADROS_GIF as i64)
            .max(1);
        let quadros = match escolher_quadros(None, params.start, params.end, Some(step), duracao, MAX_QUADROS_GIF) {
            Ok(q) => q,
            Err(e) => return erro(e),
        };
        // velocidade natural: um quadro do GIF a cada `step` quadros do projeto
        let fps_gif = params.fps.unwrap_or_else(|| (fps_projeto / step as f64).clamp(1.0, 25.0)).clamp(1.0, 50.0);
        let largura = params.width.clamp(120, 1920);
        let pasta = pasta_padrao();
        let saida = match params.output_path.as_deref() {
            Some(p) => PathBuf::from(p),
            None => pasta.join(format!("animacao_{}-{}.gif", quadros[0], quadros[quadros.len() - 1])),
        };
        if saida.extension().and_then(|e| e.to_str()).is_none_or(|e| !e.eq_ignore_ascii_case("gif")) {
            return erro("output_path precisa terminar em \".gif\"");
        }
        if saida.exists() && params.output_path.is_some() && !params.overwrite {
            return erro(format!("o arquivo de saída já existe: {} (chame novamente com overwrite=true)", saida.display()));
        }
        if let Some(pai) = saida.parent()
            && let Err(e) = std::fs::create_dir_all(pai)
        {
            return erro(format!("não consegui criar a pasta {}: {e}", pai.display()));
        }

        let prontos = match self.renderizar_quadros(&projeto, &pasta, &quadros).await {
            Ok(p) => p,
            Err(e) => return erro(e),
        };
        let caminhos: Vec<PathBuf> = prontos.iter().map(|(_, c)| c.clone()).collect();
        if let Err(e) = montar_gif(&caminhos, fps_gif, largura, &saida).await {
            return erro(format!("os quadros foram renderizados, mas o GIF falhou: {e}"));
        }
        let tamanho = std::fs::metadata(&saida).map(|m| m.len()).ok();
        json!({
            "ok": true,
            "path": saida.display().to_string(),
            "tamanho_bytes": tamanho,
            "quadros_usados": quadros,
            "fps_gif": fps_gif,
            "duracao_gif_s": (quadros.len() as f64 / fps_gif * 100.0).round() / 100.0,
            "fps_projeto": fps_projeto,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lista_explicita_mantem_a_ordem_tira_repetidos_e_confere_o_limite_do_projeto() {
        assert_eq!(escolher_quadros(Some(&[70, 0, 70, 30]), None, None, None, 150, 60), Ok(vec![70, 0, 30]));
        assert!(escolher_quadros(Some(&[150]), None, None, None, 150, 60).unwrap_err().contains("fora do projeto"));
        assert!(escolher_quadros(Some(&[-1]), None, None, None, 150, 60).is_err());
        assert!(escolher_quadros(Some(&[1]), Some(0), None, None, 150, 60).unwrap_err().contains("OU"));
    }

    #[test]
    fn intervalo_com_passo_inclui_o_ultimo_quadro() {
        assert_eq!(escolher_quadros(None, Some(0), Some(100), Some(25), 150, 60), Ok(vec![0, 25, 50, 75, 100]));
        // o passo não cai no fim: o último quadro entra mesmo assim
        assert_eq!(escolher_quadros(None, Some(0), Some(60), Some(25), 150, 60), Ok(vec![0, 25, 50, 60]));
    }

    #[test]
    fn sem_nada_espalha_cerca_de_doze_quadros_pelo_projeto_todo() {
        let q = escolher_quadros(None, None, None, None, 150, 60).unwrap();
        assert_eq!(q.first(), Some(&0));
        assert_eq!(q.last(), Some(&149));
        assert!((11..=14).contains(&q.len()), "{q:?}");
    }

    #[test]
    fn erros_de_intervalo_e_limite() {
        assert!(escolher_quadros(None, Some(10), Some(5), None, 150, 60).is_err());
        assert!(escolher_quadros(None, Some(0), Some(500), None, 150, 60).is_err());
        assert!(escolher_quadros(None, None, None, Some(0), 150, 60).is_err());
        assert!(escolher_quadros(None, None, None, Some(1), 150, 60).unwrap_err().contains("demais"));
        assert!(escolher_quadros(None, None, None, None, 0, 60).unwrap_err().contains("vazio"));
    }

    #[test]
    fn grade_calcula_as_linhas() {
        assert_eq!(linhas_da_grade(12, 3), 4);
        assert_eq!(linhas_da_grade(10, 3), 4);
        assert_eq!(linhas_da_grade(1, 3), 1);
    }
}
