//! Teste de contrato do servidor MCP via stdio: sobe o binário compilado em
//! modo `mcp`, faz o handshake `initialize`, confere que `tools/list` traz
//! exatamente as ferramentas desta fase, e exercita o fluxo completo de
//! projeto → bin → timeline → export/render através do protocolo MCP de
//! verdade (não chamando as funções Rust diretamente).

use std::path::PathBuf;
use std::time::Duration;

use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use serde_json::Value;

fn caminho_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/smoke.xml")
}

/// Fixture um pouco mais longa (10s a 25fps), usada só no teste de
/// render_cancel/render_progress, para dar tempo real de observar o job
/// "em andamento" antes de ele terminar.
fn caminho_fixture_media() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/medio.xml")
}

fn caminho_temporario(nome: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "kdenlive_mcp_plus_mcp_{}_{nome}",
        std::process::id()
    ))
}

async fn iniciar_cliente() -> RunningService<rmcp::RoleClient, ()> {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_kdenlive-mcp-plus"));
    let mut comando = tokio::process::Command::new(exe);
    comando.arg("mcp");
    let transporte =
        TokioChildProcess::new(comando).expect("deveria conseguir iniciar o processo filho");
    ().serve(transporte)
        .await
        .expect("o handshake initialize deveria funcionar")
}

/// Chama uma ferramenta e devolve o `structured_content` como JSON.
async fn chamar(
    client: &RunningService<rmcp::RoleClient, ()>,
    nome: &str,
    argumentos: Value,
) -> Value {
    let argumentos = argumentos
        .as_object()
        .cloned()
        .expect("os argumentos devem ser um objeto JSON");
    let resultado = client
        .call_tool(CallToolRequestParams::new(nome.to_string()).with_arguments(argumentos))
        .await
        .unwrap_or_else(|e| panic!("a ferramenta \"{nome}\" deveria responder: {e}"));
    resultado
        .structured_content
        .unwrap_or_else(|| panic!("a ferramenta \"{nome}\" deveria trazer structured_content"))
}

fn assert_ok(resposta: &Value, contexto: &str) {
    assert_eq!(
        resposta["ok"],
        Value::Bool(true),
        "{contexto}: esperava ok:true, resposta: {resposta}"
    );
}

#[tokio::test]
async fn tools_list_traz_exatamente_as_ferramentas_desta_fase() {
    let client = iniciar_cliente().await;

    let ferramentas = client
        .list_all_tools()
        .await
        .expect("tools/list deveria funcionar");
    let mut nomes: Vec<&str> = ferramentas.iter().map(|t| t.name.as_ref()).collect();
    nomes.sort_unstable();

    assert_eq!(
        nomes,
        vec![
            "bin_get_clip_details",
            "bin_import_clip",
            "bin_list_clips",
            "bin_remove_clip",
            "export_list_presets",
            "export_render",
            "export_xml",
            "preview_frame",
            "project_get_info",
            "project_import_kdenlive",
            "project_new",
            "project_open",
            "project_save",
            "project_validate",
            "render_cancel",
            "render_progress",
            "server_info",
            "timeline_add_clip",
            "timeline_add_track",
            "timeline_get_info",
            "timeline_list",
            "timeline_remove_track",
        ]
    );

    client.cancel().await.expect("fechamento limpo do cliente");
}

#[tokio::test]
async fn server_info_responde_com_presets_e_versoes() {
    let client = iniciar_cliente().await;

    let resposta = chamar(&client, "server_info", serde_json::json!({})).await;
    assert_ok(&resposta, "server_info");
    assert!(
        resposta["presets_disponiveis"]
            .as_array()
            .is_some_and(|presets| !presets.is_empty()),
        "resposta sem presets: {resposta}"
    );
    assert!(!resposta["versao_libmlt"].as_str().unwrap_or_default().is_empty());

    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Exercita o fluxo completo de edição desta fase, através do protocolo MCP
/// de verdade: criar projeto → importar clipe → montar timeline → inspecionar
/// → exportar/renderizar → salvar → reabrir → validar → remover.
#[tokio::test]
async fn fluxo_completo_projeto_bin_timeline_export_e_render() {
    let client = iniciar_cliente().await;

    // 1. project_new
    let resp = chamar(&client, "project_new", serde_json::json!({})).await;
    assert_ok(&resp, "project_new");
    assert_eq!(resp["trilhas"], Value::from(0));
    assert_eq!(resp["clipes_na_bin"], Value::from(0));

    // 2. bin_import_clip: primeiro renderiza o fixture sintético para um mp4
    //    real, para ter um arquivo de mídia de verdade para importar (sem
    //    depender de nenhuma mídia externa — D-020).
    let midia = caminho_temporario("midia_para_bin.mp4");
    let _ = std::fs::remove_file(&midia);
    let render_fixture = chamar(
        &client,
        "export_render",
        serde_json::json!({
            "project_path": caminho_fixture().to_string_lossy(),
            "output_path": midia.to_string_lossy(),
            "preset": "h264_hq",
            "overwrite": true,
        }),
    )
    .await;
    assert_ok(&render_fixture, "export_render (preparo da mídia)");

    let resp = chamar(
        &client,
        "bin_import_clip",
        serde_json::json!({ "path": midia.to_string_lossy(), "name": "clipe de teste" }),
    )
    .await;
    assert_ok(&resp, "bin_import_clip");
    let clip_id = resp["clip_id"]
        .as_str()
        .expect("bin_import_clip deveria trazer clip_id")
        .to_string();

    // 3. bin_list_clips / bin_get_clip_details
    let lista = chamar(&client, "bin_list_clips", serde_json::json!({})).await;
    assert_ok(&lista, "bin_list_clips");
    assert_eq!(lista["clipes"].as_array().map(Vec::len), Some(1));

    let detalhes = chamar(
        &client,
        "bin_get_clip_details",
        serde_json::json!({ "clip_id": clip_id }),
    )
    .await;
    assert_ok(&detalhes, "bin_get_clip_details");
    assert_eq!(detalhes["nome"], Value::from("clipe de teste"));
    assert_eq!(detalhes["arquivo_existe"], Value::Bool(true));

    // 4. timeline_add_track (vídeo e áudio) + timeline_add_clip
    let trilha_video = chamar(
        &client,
        "timeline_add_track",
        serde_json::json!({ "kind": "video" }),
    )
    .await;
    assert_ok(&trilha_video, "timeline_add_track video");
    let track_id = trilha_video["track_id"].as_str().unwrap().to_string();

    let trilha_audio = chamar(
        &client,
        "timeline_add_track",
        serde_json::json!({ "kind": "audio" }),
    )
    .await;
    assert_ok(&trilha_audio, "timeline_add_track audio");

    let clipe_posicionado = chamar(
        &client,
        "timeline_add_clip",
        serde_json::json!({
            "track_id": track_id,
            "producer_id": clip_id,
            "in_frame": 0,
            "out_frame": 24,
            "position": 0,
        }),
    )
    .await;
    assert_ok(&clipe_posicionado, "timeline_add_clip");
    assert_eq!(clipe_posicionado["clip_ref"], Value::from(format!("{track_id}:0")));

    // 5. timeline_list / timeline_get_info / project_get_info
    let lista_trilhas = chamar(&client, "timeline_list", serde_json::json!({})).await;
    assert_ok(&lista_trilhas, "timeline_list");
    assert_eq!(lista_trilhas["trilhas"].as_array().map(Vec::len), Some(2));

    let info_timeline = chamar(&client, "timeline_get_info", serde_json::json!({})).await;
    assert_ok(&info_timeline, "timeline_get_info");
    assert_eq!(info_timeline["duracao_quadros"], Value::from(25));

    let info_projeto = chamar(&client, "project_get_info", serde_json::json!({})).await;
    assert_ok(&info_projeto, "project_get_info");
    assert_eq!(info_projeto["trilhas"], Value::from(2));
    assert_eq!(info_projeto["clipes_na_bin"], Value::from(1));

    // 6. export_xml inline (sem path) e para um arquivo
    let xml_inline = chamar(&client, "export_xml", serde_json::json!({})).await;
    assert_ok(&xml_inline, "export_xml inline");
    assert!(xml_inline["xml"].as_str().unwrap_or_default().contains("<mlt"));

    let caminho_xml = caminho_temporario("sessao_exportada.xml");
    let _ = std::fs::remove_file(&caminho_xml);
    let xml_arquivo = chamar(
        &client,
        "export_xml",
        serde_json::json!({ "path": caminho_xml.to_string_lossy() }),
    )
    .await;
    assert_ok(&xml_arquivo, "export_xml em arquivo");
    assert!(caminho_xml.exists());

    // 7. export_render sem project_path: renderiza a sessão aberta
    let saida_sessao = caminho_temporario("sessao_renderizada.mp4");
    let _ = std::fs::remove_file(&saida_sessao);
    let render_sessao = chamar(
        &client,
        "export_render",
        serde_json::json!({
            "output_path": saida_sessao.to_string_lossy(),
            "preset": "h264_hq",
            "overwrite": true,
        }),
    )
    .await;
    assert_ok(&render_sessao, "export_render da sessão");
    assert!(saida_sessao.exists());

    // 8. preview_frame da sessão aberta
    let frame_png = caminho_temporario("preview.png");
    let _ = std::fs::remove_file(&frame_png);
    let preview = chamar(
        &client,
        "preview_frame",
        serde_json::json!({ "frame": 2, "output_path": frame_png.to_string_lossy() }),
    )
    .await;
    assert_ok(&preview, "preview_frame");
    assert!(frame_png.exists());

    // 9. project_save, depois project_open para reabrir e confirmar o roundtrip
    let caminho_projeto = caminho_temporario("projeto_salvo.xml");
    let _ = std::fs::remove_file(&caminho_projeto);
    let salvo = chamar(
        &client,
        "project_save",
        serde_json::json!({ "path": caminho_projeto.to_string_lossy() }),
    )
    .await;
    assert_ok(&salvo, "project_save");
    assert!(caminho_projeto.exists());

    let reaberto = chamar(
        &client,
        "project_open",
        serde_json::json!({ "path": caminho_projeto.to_string_lossy() }),
    )
    .await;
    assert_ok(&reaberto, "project_open");
    assert_eq!(reaberto["trilhas"], Value::from(2));
    assert_eq!(reaberto["clipes_na_bin"], Value::from(1));

    // 10. project_validate: a mídia existe, mas a trilha de áudio ficou
    //     vazia de propósito neste teste (só posicionamos um clipe na
    //     trilha de vídeo) — isso deve aparecer como o único problema.
    let validacao = chamar(&client, "project_validate", serde_json::json!({})).await;
    assert_ok(&validacao, "project_validate");
    let problemas = validacao["problemas"].as_array().expect("problemas deveria ser uma lista");
    assert_eq!(problemas.len(), 1, "validação: {validacao}");
    assert!(problemas[0].as_str().unwrap_or_default().contains("vazia"));

    // 11. bin_remove_clip e timeline_remove_track
    let removido = chamar(
        &client,
        "bin_remove_clip",
        serde_json::json!({ "clip_id": clip_id }),
    )
    .await;
    assert_ok(&removido, "bin_remove_clip");

    let trilha_removida = chamar(
        &client,
        "timeline_remove_track",
        serde_json::json!({ "track_id": track_id }),
    )
    .await;
    assert_ok(&trilha_removida, "timeline_remove_track");

    let info_final = chamar(&client, "timeline_list", serde_json::json!({})).await;
    assert_ok(&info_final, "timeline_list final");
    assert_eq!(info_final["trilhas"].as_array().map(Vec::len), Some(1));

    for caminho in [&midia, &caminho_xml, &saida_sessao, &frame_png, &caminho_projeto] {
        let _ = std::fs::remove_file(caminho);
    }
    client.cancel().await.expect("fechamento limpo do cliente");
}

#[tokio::test]
async fn export_render_sem_overwrite_retorna_erro_na_mesma_estrutura() {
    let client = iniciar_cliente().await;
    let saida = caminho_temporario("render_ja_existe.mp4");
    std::fs::write(&saida, b"conteudo qualquer, so para existir")
        .expect("deveria conseguir criar o arquivo de saida fake");

    let resposta = chamar(
        &client,
        "export_render",
        serde_json::json!({
            "project_path": caminho_fixture().to_string_lossy(),
            "output_path": saida.to_string_lossy(),
            "preset": "h264_hq",
            "overwrite": false,
        }),
    )
    .await;

    assert_eq!(
        resposta["ok"],
        Value::Bool(false),
        "deveria ter recusado sobrescrever: {resposta}"
    );
    assert!(
        resposta["erro"].as_str().is_some_and(|e| !e.is_empty()),
        "deveria trazer uma mensagem de erro em português: {resposta}"
    );

    let _ = std::fs::remove_file(&saida);
    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Isolamento: um projeto XML inválido falha no processo filho, mas o
/// servidor MCP continua respondendo normalmente a uma chamada seguinte, na
/// mesma conexão.
#[tokio::test]
async fn export_render_com_projeto_invalido_falha_isolado_e_servidor_continua_respondendo() {
    let client = iniciar_cliente().await;

    let projeto_invalido = caminho_temporario("projeto_invalido.xml");
    std::fs::write(&projeto_invalido, b"isto nao e um xml do mlt")
        .expect("deveria conseguir criar o projeto inválido");
    let saida = caminho_temporario("nao_deveria_existir.mp4");
    let _ = std::fs::remove_file(&saida);

    let resposta = chamar(
        &client,
        "export_render",
        serde_json::json!({
            "project_path": projeto_invalido.to_string_lossy(),
            "output_path": saida.to_string_lossy(),
            "preset": "h264_hq",
            "overwrite": true,
        }),
    )
    .await;
    assert_eq!(
        resposta["ok"],
        Value::Bool(false),
        "um projeto XML inválido deveria falhar: {resposta}"
    );
    assert!(!saida.exists());

    let info = chamar(&client, "server_info", serde_json::json!({})).await;
    assert_ok(&info, "server_info depois do erro isolado");

    let _ = std::fs::remove_file(&projeto_invalido);
    client.cancel().await.expect("fechamento limpo do cliente");
}

#[tokio::test]
async fn render_progress_e_render_cancel_acompanham_um_job_assincrono() {
    let client = iniciar_cliente().await;
    let saida = caminho_temporario("render_assincrono.mp4");
    let _ = std::fs::remove_file(&saida);

    let iniciado = chamar(
        &client,
        "export_render",
        serde_json::json!({
            "project_path": caminho_fixture_media().to_string_lossy(),
            "output_path": saida.to_string_lossy(),
            "preset": "h264_hq",
            "overwrite": true,
            "wait": false,
        }),
    )
    .await;
    assert_ok(&iniciado, "export_render wait=false");
    let job_id = iniciado["job_id"]
        .as_str()
        .expect("deveria trazer job_id")
        .to_string();

    // Consulta o progresso logo no início: deve ser um job conhecido.
    let progresso = chamar(
        &client,
        "render_progress",
        serde_json::json!({ "job_id": job_id }),
    )
    .await;
    assert_ok(&progresso, "render_progress");
    assert!(
        matches!(progresso["status"].as_str(), Some("em andamento") | Some("concluído")),
        "status inesperado logo após iniciar: {progresso}"
    );

    // Tenta cancelar rapidamente; a fixture "média" (10s) deve dar tempo de
    // o job ainda estar rodando, mas aceitamos os dois resultados honestos:
    // "cancelado" (conseguiu) ou um erro dizendo que já tinha terminado.
    let cancelado = chamar(&client, "render_cancel", serde_json::json!({ "job_id": job_id })).await;
    let cancelou_de_verdade = cancelado["ok"] == Value::Bool(true);
    if !cancelou_de_verdade {
        assert!(
            cancelado["erro"].as_str().is_some_and(|e| !e.is_empty()),
            "se não cancelou, deveria explicar por quê: {cancelado}"
        );
    }

    tokio::time::sleep(Duration::from_millis(500)).await;
    let status_final = chamar(
        &client,
        "render_progress",
        serde_json::json!({ "job_id": job_id }),
    )
    .await;
    assert_ok(&status_final, "render_progress final");
    let status = status_final["status"].as_str().unwrap_or_default();
    if cancelou_de_verdade {
        assert_eq!(status, "cancelado", "resposta final: {status_final}");
    } else {
        assert_eq!(status, "concluído", "resposta final: {status_final}");
    }

    // Um job desconhecido é um erro claro, não um pânico.
    let desconhecido = chamar(
        &client,
        "render_progress",
        serde_json::json!({ "job_id": "job-que-nao-existe" }),
    )
    .await;
    assert_eq!(desconhecido["ok"], Value::Bool(false));

    let _ = std::fs::remove_file(&saida);
    client.cancel().await.expect("fechamento limpo do cliente");
}
