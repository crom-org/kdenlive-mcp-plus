//! `session_undo`, `session_redo`, `session_status`, `session_history`.
//!
//! Unifica `session_undo`/`session_undo_step` (e o par `redo`) do catálogo
//! original num único par de ferramentas com parâmetro `steps` — ver o
//! problema listado em docs/04-ferramentas.md.

use serde_json::{Value, json};

use crate::mcp::params::SessionStepsParams;
use crate::mcp::server::{KdenliveMcpServer, erro};

impl KdenliveMcpServer {
    pub(crate) async fn session_undo_impl(&self, params: SessionStepsParams) -> Value {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return erro("nenhum projeto aberto; chame project_new ou project_open primeiro");
        };
        match session.undo(params.steps) {
            Ok(desfeitos) => json!({ "ok": true, "passos_desfeitos": desfeitos }),
            Err(e) => erro(e.to_string()),
        }
    }

    pub(crate) async fn session_redo_impl(&self, params: SessionStepsParams) -> Value {
        let mut guard = self.session.lock().await;
        let Some(session) = guard.as_mut() else {
            return erro("nenhum projeto aberto; chame project_new ou project_open primeiro");
        };
        match session.redo(params.steps) {
            Ok(refeitos) => json!({ "ok": true, "passos_refeitos": refeitos }),
            Err(e) => erro(e.to_string()),
        }
    }

    pub(crate) async fn session_status_impl(&self) -> Value {
        let guard = self.session.lock().await;
        match guard.as_ref() {
            Some(session) => json!({
                "ok": true,
                "projeto_aberto": true,
                "caminho": session.path.as_ref().map(|p| p.display().to_string()),
                "modificado": session.modified,
                "profundidade_historico": session.history_len(),
                "passos_para_refazer": session.redo_len(),
            }),
            None => json!({ "ok": true, "projeto_aberto": false }),
        }
    }

    pub(crate) async fn session_history_impl(&self) -> Value {
        let guard = self.session.lock().await;
        match guard.as_ref() {
            Some(session) => json!({ "ok": true, "historico": session.history_descriptions() }),
            None => erro("nenhum projeto aberto; chame project_new ou project_open primeiro"),
        }
    }
}
