//! `guide_add`, `guide_remove`, `guide_list`, `guide_get`, `guide_update`.
//!
//! Convenção própria deste projeto (não confirmada como igual à do
//! Kdenlive real — ver docs/09-riscos-e-bugs.md e `model::document`).

use serde_json::{Value, json};

use crate::mcp::params::{GuideAddParams, GuideIdParams, GuideUpdateParams};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document::{self, Guide};

fn guide_json(g: &Guide) -> Value {
    json!({ "guide_id": g.id, "position": g.position, "comment": g.comment })
}

impl KdenliveMcpServer {
    pub(crate) async fn guide_add_impl(&self, params: GuideAddParams) -> Value {
        let descricao = format!("adicionar guia em {}", params.position);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = document::add_guide(&mut session.doc, params.position, &params.comment)?;
                session.mark_modified();
                Ok(id)
            })
            .await;
        match resultado {
            Ok(id) => json!({ "ok": true, "guide_id": id, "position": params.position, "comment": params.comment }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn guide_remove_impl(&self, params: GuideIdParams) -> Value {
        let descricao = format!("remover guia \"{}\"", params.guide_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::remove_guide(&mut session.doc, &params.guide_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "guide_id": params.guide_id }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn guide_list_impl(&self) -> Value {
        let resultado = self
            .with_session(|session| document::list_guides(&session.doc).iter().map(guide_json).collect::<Vec<_>>())
            .await;
        match resultado {
            Ok(guias) => json!({ "ok": true, "guias": guias }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn guide_get_impl(&self, params: GuideIdParams) -> Value {
        let resultado = self
            .with_session(|session| document::get_guide(&session.doc, &params.guide_id))
            .await;
        match resultado {
            Ok(Some(guia)) => {
                let mut v = guide_json(&guia);
                v["ok"] = json!(true);
                v
            }
            Ok(None) => erro(format!("guia não encontrada: {}", params.guide_id)),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn guide_update_impl(&self, params: GuideUpdateParams) -> Value {
        let descricao = format!("atualizar guia \"{}\"", params.guide_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::update_guide(&mut session.doc, &params.guide_id, params.position, params.comment.as_deref())?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "guide_id": params.guide_id }),
            Err(valor_erro) => valor_erro,
        }
    }
}
