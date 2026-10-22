//! `project_new`, `project_open`/`project_import_kdenlive`, `project_save`,
//! `project_get_info`, `project_validate`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::mcp::params::{ProjectNewParams, ProjectOpenParams, ProjectSaveParams};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document;
use crate::model::Session;

/// Resumo do perfil de vídeo de um documento, como JSON.
fn resumo_perfil(doc: &xmltree::Element) -> Value {
    let Some(perfil) = document::profile_element(doc) else {
        return json!(null);
    };
    let ler = |nome: &str| -> i64 { perfil.attributes.get(nome).and_then(|v| v.parse().ok()).unwrap_or(0) };
    let fn_ = ler("frame_rate_num").max(1);
    let fd = ler("frame_rate_den").max(1);
    json!({
        "largura": ler("width"),
        "altura": ler("height"),
        "fps": fn_ as f64 / fd as f64,
        "proporcao": format!("{}:{}", ler("display_aspect_num"), ler("display_aspect_den")),
        "progressivo": ler("progressive") != 0,
    })
}

fn resumo_sessao(session: &Session) -> Value {
    json!({
        "ok": true,
        "caminho": session.path.as_ref().map(|p| p.display().to_string()),
        "modificado": session.modified,
        "perfil": resumo_perfil(&session.doc),
        "trilhas": document::track_ids(&session.doc).len(),
        "clipes_na_bin": document::bin_clip_ids(&session.doc).len(),
        "duracao_quadros": document::project_duration(&session.doc),
    })
}

impl KdenliveMcpServer {
    pub(crate) async fn project_new_impl(&self, params: ProjectNewParams) -> Value {
        let nova = match Session::new(&params.profile) {
            Ok(s) => s,
            Err(e) => return erro(e.to_string()),
        };
        let resumo = resumo_sessao(&nova);
        *self.session.lock().await = Some(nova);
        resumo
    }

    pub(crate) async fn project_open_impl(&self, params: ProjectOpenParams) -> Value {
        let caminho = PathBuf::from(&params.path);
        if !caminho.exists() {
            return erro(format!("projeto não encontrado: {}", caminho.display()));
        }
        let aberta = match Session::open(&caminho) {
            Ok(s) => s,
            Err(e) => return erro(e.to_string()),
        };
        let resumo = resumo_sessao(&aberta);
        *self.session.lock().await = Some(aberta);
        resumo
    }

    pub(crate) async fn project_save_impl(&self, params: ProjectSaveParams) -> Value {
        let caminho: Option<PathBuf> = params.path.map(PathBuf::from);
        let resultado = self
            .with_session_mut(|session| session.save(caminho.as_deref()))
            .await;
        match resultado {
            Ok(caminho_salvo) => json!({ "ok": true, "caminho": caminho_salvo.display().to_string() }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn project_get_info_impl(&self) -> Value {
        match self.with_session(resumo_sessao).await {
            Ok(resumo) => resumo,
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn project_validate_impl(&self) -> Value {
        let problemas = self
            .with_session(|session| validar_documento(&session.doc))
            .await;
        match problemas {
            Ok(lista) => json!({ "ok": true, "problemas": lista, "sem_problemas": lista_is_empty(&lista) }),
            Err(valor_erro) => valor_erro,
        }
    }
}

fn lista_is_empty(valor: &Value) -> bool {
    valor.as_array().is_some_and(|v| v.is_empty())
}

fn validar_documento(doc: &xmltree::Element) -> Value {
    let mut problemas: Vec<String> = Vec::new();

    for id in document::bin_clip_ids(doc) {
        if let Some(producer) = document::find_producer(doc, &id)
            && let Some(resource) = document::get_property(producer, "resource")
            && !Path::new(&resource).exists()
        {
            problemas.push(format!(
                "clipe \"{id}\" na bin aponta para um arquivo que não existe: {resource}"
            ));
        }
    }

    for track_id in document::track_ids(doc) {
        let vazio = document::find_playlist(doc, &track_id)
            .map(|p| document::list_clips(doc, &track_id).is_empty() && document::track_length(p) == 0)
            .unwrap_or(true);
        if vazio {
            problemas.push(format!("trilha \"{track_id}\" está vazia"));
        }
    }

    json!(problemas)
}
