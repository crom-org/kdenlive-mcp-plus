//! Recarregamento automático do Kdenlive aberto (decisão D-042).
//!
//! Com `KDENLIVE_MCP_AUTORELOAD=1`, depois de cada gravação do projeto o
//! servidor pede ao Kdenlive que está mostrando aquele arquivo que o
//! recarregue — o mesmo que **File > Revert**, disparado pelo D-Bus. Assim dá para ver o
//! vídeo sendo montado sem clicar em nada, e sem alterar o código do Kdenlive:
//! todo `QAction` do Kdenlive aparece no D-Bus da sessão do usuário em
//! `/kdenlive/MainWindow_1/actions/<nome>`.
//!
//! O D-Bus é falado pelo `busctl` (systemd), como subprocesso: sem
//! dependência nova no `Cargo.toml` e sem `unsafe`. Nada aqui é crítico —
//! se o `busctl` não existir, se não houver Kdenlive aberto ou se ele tiver
//! alterações não salvas, o servidor só registra o motivo no stderr; a
//! edição do agente nunca falha por causa disso.
//!
//! O Kdenlive só recarrega **sem confirmar** se o arquivo carrega como
//! "não modificado" — para isso o documento precisa ter `documentid`
//! (ver `model::document::new_document_id`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Objeto D-Bus da janela principal do Kdenlive.
const JANELA: &str = "/kdenlive/MainWindow_1";
/// Ação "File > Revert" do Kdenlive (`KStandardAction::revert`).
const ACAO_REVERT: &str = "/kdenlive/MainWindow_1/actions/file_revert";
/// Espera, depois da última gravação, antes de recarregar: várias edições
/// seguidas viram um recarregamento só.
const ESPERA_ANTES_DE_RECARREGAR: Duration = Duration::from_millis(400);
/// Limite de uma consulta ao `busctl`. Um Kdenlive que não responde nisso
/// está ocupado (carregando um projeto, por exemplo).
const LIMITE_CONSULTA: Duration = Duration::from_secs(4);
/// Limite do pedido de recarga: a chamada só é respondida quando o Kdenlive
/// termina de recarregar o projeto, o que pode levar vários segundos.
const LIMITE_RECARGA: Duration = Duration::from_secs(90);
/// Quantas vezes tentar de novo, e com que intervalo, se o Kdenlive estiver ocupado.
const TENTATIVAS_OCUPADO: u32 = 6;
const INTERVALO_OCUPADO: Duration = Duration::from_secs(2);

/// O que aconteceu ao tentar recarregar.
#[derive(Debug, PartialEq, Eq)]
pub enum Resultado {
    /// Pedido de recarga enviado à janela do Kdenlive (nome da conexão D-Bus).
    Recarregado(String),
    /// Não há Kdenlive com este projeto aberto no D-Bus da sessão.
    NaoEncontrado,
    /// O Kdenlive tem alterações não salvas neste projeto; recarregar
    /// as apagaria (e abriria um diálogo de confirmação), então não foi feito.
    ComAlteracoesNaoSalvas,
    /// Uma conexão do D-Bus não respondeu a tempo (provavelmente o Kdenlive
    /// carregando um projeto) e nenhuma outra era o projeto procurado. Vale
    /// tentar de novo daqui a pouco.
    Ocupado,
}

/// Programa usado para falar com o D-Bus. `KDENLIVE_MCP_BUSCTL` existe para
/// os testes (um `busctl` falso) e para quem tiver o `busctl` fora do `PATH`.
fn programa_busctl() -> String {
    std::env::var("KDENLIVE_MCP_BUSCTL").unwrap_or_else(|_| "busctl".to_string())
}

const MSG_DEMOROU: &str = "o busctl demorou demais para responder";

/// O Claude Desktop (e outros clientes MCP) inicia o servidor só com `HOME`,
/// `PATH`, `USER`, `LOGNAME` e `SHELL`: sem `DBUS_SESSION_BUS_ADDRESS` nem
/// `XDG_RUNTIME_DIR` o `busctl --user` não acha o barramento da sessão. Nesse
/// caso usamos o endereço padrão do systemd, `/run/user/<uid>/bus`, se o
/// socket existir. `None` = o ambiente já basta (ou não há socket).
fn endereco_padrao_do_barramento(
    dbus_definido: bool,
    runtime_dir_definido: bool,
    uid: u32,
    socket_existe: impl Fn(&str) -> bool,
) -> Option<String> {
    if dbus_definido || runtime_dir_definido {
        return None;
    }
    let socket = format!("/run/user/{uid}/bus");
    socket_existe(&socket).then(|| format!("unix:path={socket}"))
}

/// Dono do próprio processo (o uid do usuário), sem `unsafe`.
fn uid_do_processo() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

/// Roda `busctl --user --json=short <args>` e devolve o stdout.
async fn busctl(args: &[&str], limite: Duration) -> Result<String, String> {
    let mut comando = Command::new(programa_busctl());
    comando
        .arg("--user")
        .arg("--json=short")
        .arg(format!("--timeout={}", limite.as_secs()))
        .args(args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(uid) = uid_do_processo()
        && let Some(endereco) = endereco_padrao_do_barramento(
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
            std::env::var_os("XDG_RUNTIME_DIR").is_some(),
            uid,
            |socket| Path::new(socket).exists(),
        )
    {
        comando.env("DBUS_SESSION_BUS_ADDRESS", endereco);
    }
    let saida = tokio::time::timeout(limite, comando.output())
        .await
        .map_err(|_| MSG_DEMOROU.to_string())?
        .map_err(|e| format!("não consegui executar o busctl ({e}); o recarregamento automático precisa dele"))?;
    if saida.status.success() {
        Ok(String::from_utf8_lossy(&saida.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&saida.stderr).trim().to_string())
    }
}

/// O erro de uma chamada D-Bus é "não respondeu a tempo" (e não, por exemplo,
/// "o objeto não existe", que é a resposta de qualquer programa que não é o Kdenlive)?
fn e_falta_de_resposta(erro: &str) -> bool {
    erro == MSG_DEMOROU || erro.contains("timed out") || erro.contains("Timeout")
}

/// O processo é (provavelmente) um Kdenlive? O nome do processo no D-Bus é
/// `kdenlive` (pacote, flatpak, snap) ou `AppRun.wrapped` (AppImage).
/// Numa sessão de desktop há dezenas de conexões (cada uma que não responde
/// custa o tempo limite da consulta), então só estas são consultadas.
fn parece_kdenlive(processo: &str) -> bool {
    let p = processo.to_lowercase();
    p.contains("kdenlive") || p.starts_with("apprun")
}

/// Nomes de conexão (`:1.23`) de processos que parecem ser o Kdenlive, da
/// saída de `busctl list --json=short`. Os nomes bem conhecidos (`org.…`)
/// são ignorados: o Kdenlive só tem garantia da conexão única, e uma
/// conexão aparece em ambos.
pub fn parse_conexoes(json: &str) -> Vec<String> {
    let Ok(Value::Array(itens)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let mut nomes: Vec<String> = Vec::new();
    for item in itens {
        let Some(nome) = item.get("name").and_then(Value::as_str) else { continue };
        let processo = item.get("process").and_then(Value::as_str).unwrap_or("");
        if nome.starts_with(':') && parece_kdenlive(processo) && !nomes.iter().any(|n| n == nome) {
            nomes.push(nome.to_string());
        }
    }
    nomes
}

/// Valor de texto de `busctl get-property --json=short` (`{"type":"s","data":...}`).
pub fn parse_texto(json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    v.get("data")?.as_str().map(str::to_string)
}

/// Valor booleano de `busctl get-property --json=short` (`{"type":"b","data":...}`).
pub fn parse_booleano(json: &str) -> Option<bool> {
    let v: Value = serde_json::from_str(json).ok()?;
    v.get("data")?.as_bool()
}

/// O título da janela do Kdenlive é `<nome do arquivo sem a última extensão>
/// [*]/ <descrição do perfil>` (`KdenliveDoc::description`). Esta função diz
/// se esse título é o do projeto em `caminho`.
pub fn titulo_e_do_projeto(titulo: &str, caminho: &Path) -> bool {
    let Some(nome) = caminho.file_stem().map(|n| n.to_string_lossy().into_owned()) else {
        return false;
    };
    titulo.starts_with(&format!("{nome} [*]/"))
}

async fn propriedade_da_janela(conexao: &str, propriedade: &str) -> Result<String, String> {
    busctl(&["get-property", conexao, JANELA, "org.qtproject.Qt.QWidget", propriedade], LIMITE_CONSULTA).await
}

/// Procura o Kdenlive com `caminho` aberto e pede que ele recarregue o arquivo.
pub async fn recarregar(caminho: &Path) -> Result<Resultado, String> {
    let lista = busctl(&["list"], LIMITE_CONSULTA).await?;
    let mut ocupado = false;
    for conexao in parse_conexoes(&lista) {
        let titulo = match propriedade_da_janela(&conexao, "windowTitle").await {
            Ok(saida) => match parse_texto(&saida) {
                Some(t) => t,
                None => continue,
            },
            Err(e) => {
                // "o objeto não existe" = não é um Kdenlive; sem resposta = pode ser um ocupado
                ocupado |= e_falta_de_resposta(&e);
                continue;
            }
        };
        if !titulo_e_do_projeto(&titulo, caminho) {
            continue;
        }
        let modificado = match propriedade_da_janela(&conexao, "windowModified").await {
            Ok(saida) => parse_booleano(&saida).unwrap_or(false),
            Err(e) if e_falta_de_resposta(&e) => return Ok(Resultado::Ocupado),
            Err(_) => false,
        };
        if modificado {
            return Ok(Resultado::ComAlteracoesNaoSalvas);
        }
        // a chamada só volta quando o Kdenlive terminou de recarregar, então
        // o próximo pedido nunca encontra um recarregamento em andamento
        busctl(&["call", &conexao, ACAO_REVERT, "org.qtproject.Qt.QAction", "trigger"], LIMITE_RECARGA).await?;
        return Ok(Resultado::Recarregado(conexao));
    }
    Ok(if ocupado { Resultado::Ocupado } else { Resultado::NaoEncontrado })
}

/// Inicia a tarefa de fundo que recarrega o Kdenlive. Cada caminho enviado
/// pelo canal é um "o projeto foi gravado"; pedidos que chegam juntos viram
/// um recarregamento só (e uma gravação durante o recarregamento gera outro).
pub fn iniciar(runtime: &tokio::runtime::Handle) -> mpsc::UnboundedSender<PathBuf> {
    let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
    runtime.spawn(async move {
        let mut avisou_modificado = false;
        let mut avisou_busctl = false;
        while let Some(mut caminho) = rx.recv().await {
            tokio::time::sleep(ESPERA_ANTES_DE_RECARREGAR).await;
            while let Ok(mais_recente) = rx.try_recv() {
                caminho = mais_recente;
            }
            let mut resultado = recarregar(&caminho).await;
            // Kdenlive ocupado (carregando): espera e tenta de novo, sempre com
            // o caminho mais recente
            for _ in 1..TENTATIVAS_OCUPADO {
                if !matches!(resultado, Ok(Resultado::Ocupado)) {
                    break;
                }
                tokio::time::sleep(INTERVALO_OCUPADO).await;
                while let Ok(mais_recente) = rx.try_recv() {
                    caminho = mais_recente;
                }
                resultado = recarregar(&caminho).await;
            }
            match resultado {
                Ok(Resultado::Ocupado) => {
                    eprintln!(
                        "kdenlive-mcp-plus: o Kdenlive não respondeu a tempo; o projeto {} não foi recarregado \
                         (use File > Revert).",
                        caminho.display()
                    );
                }
                Ok(Resultado::Recarregado(conexao)) => {
                    avisou_modificado = false;
                    eprintln!("kdenlive-mcp-plus: Kdenlive ({conexao}) recarregado: {}", caminho.display());
                }
                Ok(Resultado::NaoEncontrado) => {}
                Ok(Resultado::ComAlteracoesNaoSalvas) => {
                    if !avisou_modificado {
                        avisou_modificado = true;
                        eprintln!(
                            "kdenlive-mcp-plus: o Kdenlive tem alterações não salvas em {}; não recarreguei \
                             para não apagá-las. Salve ou desfaça no Kdenlive e use File > Revert.",
                            caminho.display()
                        );
                    }
                }
                Err(e) => {
                    if !avisou_busctl {
                        avisou_busctl = true;
                        eprintln!("kdenlive-mcp-plus: recarregamento automático do Kdenlive falhou: {e}");
                    }
                }
            }
        }
    });
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conexoes_so_de_processos_que_parecem_kdenlive_sem_duplicatas_nem_nomes_conhecidos() {
        let json = r#"[
            {"name":":1.0","pid":10,"process":"AppRun.wrapped","user":"j"},
            {"name":":1.9","pid":10,"process":"AppRun.wrapped","user":"j"},
            {"name":":1.9","pid":10,"process":"AppRun.wrapped","user":"j"},
            {"name":":1.7","pid":11,"process":"kdenlive","user":"j"},
            {"name":":1.19","pid":99,"process":"busctl","user":"j"},
            {"name":":1.20","pid":98,"process":"claude-desktop","user":"j"},
            {"name":":1.21","pid":null,"process":null,"user":null},
            {"name":"org.freedesktop.DBus","pid":1,"process":"dbus-daemon","user":"j"},
            {"name":"org.kde.kdenlive-10","pid":null,"process":null,"user":null}
        ]"#;
        assert_eq!(parse_conexoes(json), vec![":1.0".to_string(), ":1.9".to_string(), ":1.7".to_string()]);
        assert!(parse_conexoes("isto não é json").is_empty());
    }

    #[test]
    fn barramento_padrao_so_entra_quando_o_ambiente_nao_diz_onde_fica() {
        let existe = |_: &str| true;
        // o ambiente já diz: não mexe
        assert_eq!(endereco_padrao_do_barramento(true, false, 1000, existe), None);
        assert_eq!(endereco_padrao_do_barramento(false, true, 1000, existe), None);
        // ambiente enxuto (Claude Desktop): usa o socket do systemd
        assert_eq!(
            endereco_padrao_do_barramento(false, false, 1000, existe),
            Some("unix:path=/run/user/1000/bus".to_string())
        );
        // sem o socket, não inventa endereço
        assert_eq!(endereco_padrao_do_barramento(false, false, 1000, |_| false), None);
    }

    #[test]
    fn propriedades_do_busctl_sao_lidas_como_texto_e_booleano() {
        assert_eq!(parse_texto(r#"{"type":"s","data":"a [*]/ HD"}"#), Some("a [*]/ HD".to_string()));
        assert_eq!(parse_booleano(r#"{"type":"b","data":true}"#), Some(true));
        assert_eq!(parse_booleano(r#"{"type":"b","data":false}"#), Some(false));
        assert_eq!(parse_texto("lixo"), None);
        assert_eq!(parse_booleano(r#"{"type":"s","data":"x"}"#), None);
    }

    #[test]
    fn titulo_do_kdenlive_e_comparado_com_o_nome_do_arquivo_sem_extensao() {
        let titulo = "meu video [*]/ HD 1080p 25 fps";
        assert!(titulo_e_do_projeto(titulo, Path::new("/home/j/Vídeos/meu video.kdenlive")));
        assert!(!titulo_e_do_projeto(titulo, Path::new("/home/j/Vídeos/outro.kdenlive")));
        // só a última extensão sai, como em QFileInfo::completeBaseName
        assert!(titulo_e_do_projeto("a.b [*]/ HD", Path::new("/x/a.b.kdenlive")));
        // um nome que apenas começa igual não conta
        assert!(!titulo_e_do_projeto("meu video 2 [*]/ HD", Path::new("/x/meu video.kdenlive")));
    }
}
