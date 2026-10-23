//! `transition_add`, `transition_remove`, `transition_set`,
//! `transition_list`.

use serde_json::{Value, json};

use crate::mcp::params::{TransitionAddParams, TransitionIdParams, TransitionSetParamParams};
use crate::mcp::server::{KdenliveMcpServer, erro};
use crate::model::document;

impl KdenliveMcpServer {
    pub(crate) async fn transition_add_impl(&self, params: TransitionAddParams) -> Value {
        let descricao = format!(
            "adicionar transição \"{}\" entre \"{}\" e \"{}\"",
            params.service, params.track_a, params.track_b
        );
        let range = match (params.in_frame, params.out_frame) {
            (Some(i), Some(o)) => Some((i, o)),
            (None, None) => None,
            _ => return erro("informe in_frame e out_frame juntos (ou nenhum dos dois, para a timeline inteira)"),
        };
        let extras: Vec<(String, String)> = params.params.clone().unwrap_or_default().into_iter().collect();
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let opcoes = document::TransitionOptions { range, kdenlive_id: params.kdenlive_id.as_deref(), params: &extras };
                let id = document::add_transition(&mut session.doc, &params.service, &params.track_a, &params.track_b, &opcoes)?;
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
                "in_frame": range.map(|r| r.0),
                "out_frame": range.map(|r| r.1),
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
                    .map(|(id, servico)| {
                        let intervalo = document::transition_range(&session.doc, &id);
                        json!({
                            "transition_id": id,
                            "service": servico,
                            "in_frame": intervalo.map(|r| r.0),
                            "out_frame": intervalo.map(|r| r.1),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .await;
        match resultado {
            Ok(transicoes) => json!({ "ok": true, "transicoes": transicoes }),
            Err(valor_erro) => valor_erro,
        }
    }
}
