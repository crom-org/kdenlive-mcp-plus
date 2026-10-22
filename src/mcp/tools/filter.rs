//! `filter_add`, `filter_remove`, `filter_set_param`, `filter_list`,
//! `filter_list_available`.

use serde_json::{Value, json};

use crate::mcp::params::{ClipRefParams, FilterAddParams, FilterRemoveParams, FilterSetParamParams};
use crate::mcp::server::{KdenliveMcpServer, erro, parse_clip_ref};
use crate::model::document;

impl KdenliveMcpServer {
    pub(crate) async fn filter_add_impl(&self, params: FilterAddParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("adicionar filtro \"{}\" ao clipe {}", params.service, params.clip_ref);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                let id = document::add_filter(&mut session.doc, &track_id, indice, &params.service)?;
                session.mark_modified();
                Ok(id)
            })
            .await;
        match resultado {
            Ok(filter_id) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "filter_id": filter_id,
                "service": params.service,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn filter_remove_impl(&self, params: FilterRemoveParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!("remover filtro \"{}\" do clipe {}", params.filter_id, params.clip_ref);
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::remove_filter(&mut session.doc, &track_id, indice, &params.filter_id)?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({ "ok": true, "clip_ref": params.clip_ref, "filter_id": params.filter_id }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn filter_set_param_impl(&self, params: FilterSetParamParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let descricao = format!(
            "definir {}={} no filtro \"{}\" do clipe {}",
            params.param, params.value, params.filter_id, params.clip_ref
        );
        let resultado = self
            .with_session_mut(|session| {
                session.snapshot_before(descricao);
                document::set_filter_param(
                    &mut session.doc,
                    &track_id,
                    indice,
                    &params.filter_id,
                    &params.param,
                    &params.value,
                )?;
                session.mark_modified();
                Ok(())
            })
            .await;
        match resultado {
            Ok(()) => json!({
                "ok": true,
                "clip_ref": params.clip_ref,
                "filter_id": params.filter_id,
                "param": params.param,
                "value": params.value,
            }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn filter_list_impl(&self, params: ClipRefParams) -> Value {
        let (track_id, indice) = match parse_clip_ref(&params.clip_ref) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let resultado = self
            .with_session(|session| {
                document::list_filters(&session.doc, &track_id, indice)
                    .into_iter()
                    .map(|(id, servico)| json!({ "filter_id": id, "service": servico }))
                    .collect::<Vec<_>>()
            })
            .await;
        match resultado {
            Ok(filtros) => json!({ "ok": true, "clip_ref": params.clip_ref, "filtros": filtros }),
            Err(valor_erro) => valor_erro,
        }
    }

    pub(crate) async fn filter_list_available_impl(&self) -> Value {
        match listar_servicos("filters").await {
            Ok(servicos) => json!({ "ok": true, "filtros_disponiveis": servicos }),
            Err(mensagem) => erro(mensagem),
        }
    }
}

/// Lista os serviços de um tipo (`"filters"`/`"transitions"`) que o
/// runtime do libmlt expõe, chamando `melt -query <tipo>` — um subprocesso
/// comum (como o `ffprobe`), não uma chamada FFI deste servidor: mantém o
/// isolamento da decisão D-003.
pub(crate) async fn listar_servicos(tipo: &str) -> Result<Vec<String>, String> {
    let saida = tokio::process::Command::new("melt")
        .arg("-query")
        .arg(tipo)
        .output()
        .await
        .map_err(|e| format!("não consegui rodar \"melt -query {tipo}\": {e}"))?;

    let texto = String::from_utf8_lossy(&saida.stdout);
    let servicos: Vec<String> = texto
        .lines()
        .filter_map(|linha| linha.trim().strip_prefix("- "))
        .map(str::to_string)
        .collect();

    if servicos.is_empty() {
        return Err(format!(
            "\"melt -query {tipo}\" não devolveu nenhum serviço (melt está instalado e no PATH?)"
        ));
    }
    Ok(servicos)
}
