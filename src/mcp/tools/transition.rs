//! `transition_add`, `transition_remove`, `transition_set`,
//! `transition_list`.

use serde_json::{Value, json};

use crate::mcp::params::{TransitionAddParams, TransitionIdParams, TransitionSetParamParams};
use crate::mcp::server::KdenliveMcpServer;
use crate::model::document;

impl KdenliveMcpServer {
    pub(crate) async fn transition_add_impl(&self, params: TransitionAddParams) -> Value {
        let descricao = format!(
            "adicionar transição \"{}\" entre \"{}\" e \"{}\"",
            params.service, params.track_a, params.track_b
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = document::add_transition(&mut session.doc, &params.service, &params.track_a, &params.track_b)?;
                session.mark_modified();
                Ok(id)
            })
            .await;
        match resultado {
            Ok(transition_id) => json!({
                "ok": true,
                "transition_id": transition_id,
                "service": params.service,
                "track_a": params.track_a,
                "track_b": params.track_b,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn transition_remove_impl(&self, params: TransitionIdParams) -> Value {
        let descricao = format!("remover transição \"{}\"", params.transition_id);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::remove_transition(&mut session.doc, &params.transition_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "transition_id": params.transition_id }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn transition_set_impl(&self, params: TransitionSetParamParams) -> Value {
        let descricao = format!(
            "definir {}={} na transição \"{}\"",
            params.param, params.value, params.transition_id
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::set_transition_param(&mut session.doc, &params.transition_id, &params.param, &params.value)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({
                "ok": true,
                "transition_id": params.transition_id,
                "param": params.param,
                "value": params.value,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn transition_list_impl(&self) -> Value {
        let resultado = self
            .with_session(|session| {
                document::list_transitions(&session.doc)
                    .into_iter()
                    .map(|(id, servico)| json!({ "transition_id": id, "service": servico }))
                    .collect::<Vec<_>>()
            })
            .await;
        match resultado {
            Ok(transicoes) => json!({ "ok": true, "transicoes": transicoes }),
            Err(valor_erro) => valor_erro,
        }
    }
}
