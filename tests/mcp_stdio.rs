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
    iniciar_cliente_com_env(&[]).await
}

async fn iniciar_cliente_com_env(env: &[(&str, &str)]) -> RunningService<rmcp::RoleClient, ()> {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_kdenlive-mcp-plus"));
    let mut comando = tokio::process::Command::new(exe);
    comando.arg("mcp");
    for (k, v) in env {
        comando.env(k, v);
    }
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
            "audio_duck",
            "audio_normalize",
            "audio_set_levels",
            "bin_create_folder",
            "bin_get_clip_details",
            "bin_import_clip",
            "bin_import_folder",
            "bin_list_clips",
            "bin_move_clip",
            "bin_remove_clip",
            "bin_rename",
            "clip_add_note",
            "clip_get_properties",
            "clip_reframe_vertical",
            "clip_reverse",
            "clip_set_color",
            "clip_set_opacity",
            "clip_set_speed",
            "clip_set_transform",
            "export_list_presets",
            "export_render",
            "export_thumbnail",
            "export_xml",
            "filter_add",
            "filter_list",
            "filter_list_available",
            "filter_remove",
            "filter_set_param",
            "guide_add",
            "guide_get",
            "guide_list",
            "guide_remove",
            "guide_update",
            "keyframe_list",
            "keyframe_remove",
            "keyframe_set",
            "marker_add",
            "marker_list",
            "marker_remove",
            "preview_frame",
            "preview_frames",
            "preview_gif",
            "project_get_info",
            "project_get_profile",
            "project_import_kdenlive",
            "project_list_profiles",
            "project_new",
            "project_open",
            "project_save",
            "project_set_profile",
            "project_validate",
            "render_cancel",
            "render_progress",
            "render_queue_add",
            "render_queue_list",
            "render_queue_start",
            "render_queue_status",
            "render_queue_stop",
            "server_info",
            "session_history",
            "session_redo",
            "session_status",
            "session_undo",
            "subtitle_export_srt",
            "subtitle_import_srt",
            "timeline_add_clip",
            "timeline_add_track",
            "timeline_assemble_ranges",
            "timeline_get_duration",
            "timeline_get_info",
            "timeline_get_position",
            "timeline_get_zoom",
            "timeline_list",
            "timeline_move_clip",
            "timeline_remove_clip",
            "timeline_remove_track",
            "timeline_seek",
            "timeline_set_zoom",
            "timeline_split_clip",
            "timeline_trim_clip",
            "title_add",
            "title_update",
            "track_get_info",
            "track_move_down",
            "track_move_up",
            "track_set_hidden",
            "track_set_locked",
            "track_set_muted",
            "track_set_name",
            "transition_add",
            "transition_list",
            "transition_remove",
            "transition_set",
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
    let resp = chamar(&client, "project_new", serde_json::json!({ "default_tracks": false })).await;
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
        matches!(
            progresso["status"].as_str(),
            Some("na fila") | Some("em andamento") | Some("concluído")
        ),
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

/// Exercita as ferramentas de trilha, clipe, sessão (undo/redo), filtros,
/// transições, guias e marcadores — tudo da Fase 2, grupos 1 a 3.
#[tokio::test]
async fn fase2_trilha_clipe_sessao_filtros_transicoes_guias_marcadores() {
    let client = iniciar_cliente().await;

    chamar(&client, "project_new", serde_json::json!({ "default_tracks": false })).await;

    // --- trilhas -------------------------------------------------------
    let t0 = chamar(&client, "timeline_add_track", serde_json::json!({ "kind": "video" })).await;
    let track_a = t0["track_id"].as_str().unwrap().to_string();
    let t1 = chamar(&client, "timeline_add_track", serde_json::json!({ "kind": "video" })).await;
    let track_b = t1["track_id"].as_str().unwrap().to_string();

    let renomeada = chamar(
        &client,
        "track_set_name",
        serde_json::json!({ "track_id": track_a, "name": "Principal" }),
    )
    .await;
    assert_ok(&renomeada, "track_set_name");

    chamar(&client, "track_set_locked", serde_json::json!({ "track_id": track_a, "value": true })).await;
    chamar(&client, "track_set_hidden", serde_json::json!({ "track_id": track_a, "value": true })).await;
    chamar(&client, "track_set_muted", serde_json::json!({ "track_id": track_a, "value": true })).await;

    let info_trilha = chamar(&client, "track_get_info", serde_json::json!({ "track_id": track_a })).await;
    assert_ok(&info_trilha, "track_get_info");
    assert_eq!(info_trilha["nome"], Value::from("Principal"));
    assert_eq!(info_trilha["travada"], Value::Bool(true));
    assert_eq!(info_trilha["oculta"], Value::Bool(true));
    assert_eq!(info_trilha["muda"], Value::Bool(true));

    let movida = chamar(&client, "track_move_up", serde_json::json!({ "track_id": track_b })).await;
    assert_ok(&movida, "track_move_up");
    let erro_borda = chamar(&client, "track_move_up", serde_json::json!({ "track_id": track_b })).await;
    assert_eq!(erro_borda["ok"], Value::Bool(false), "já deveria estar na primeira posição");
    chamar(&client, "track_move_down", serde_json::json!({ "track_id": track_b })).await;

    // --- clipe + velocidade/reversão/opacidade --------------------------
    let midia = caminho_temporario("fase2_midia.mp4");
    let _ = std::fs::remove_file(&midia);
    chamar(
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
    let importado = chamar(
        &client,
        "bin_import_clip",
        serde_json::json!({ "path": midia.to_string_lossy() }),
    )
    .await;
    let clip_id = importado["clip_id"].as_str().unwrap().to_string();

    let cor = chamar(
        &client,
        "clip_set_color",
        serde_json::json!({ "clip_id": clip_id, "color": "#ff0000" }),
    )
    .await;
    assert_ok(&cor, "clip_set_color");
    let nota = chamar(
        &client,
        "clip_add_note",
        serde_json::json!({ "clip_id": clip_id, "note": "clipe de teste da Fase 2" }),
    )
    .await;
    assert_ok(&nota, "clip_add_note");
    let detalhes = chamar(&client, "bin_get_clip_details", serde_json::json!({ "clip_id": clip_id })).await;
    assert_eq!(detalhes["cor"], Value::from("#ff0000"));
    assert_eq!(detalhes["nota"], Value::from("clipe de teste da Fase 2"));

    let posicionado = chamar(
        &client,
        "timeline_add_clip",
        serde_json::json!({ "track_id": track_a, "producer_id": clip_id, "in_frame": 0, "out_frame": 49, "position": 0 }),
    )
    .await;
    let clip_ref = posicionado["clip_ref"].as_str().unwrap().to_string();

    let propriedades = chamar(&client, "clip_get_properties", serde_json::json!({ "clip_ref": clip_ref })).await;
    assert_ok(&propriedades, "clip_get_properties");
    assert_eq!(propriedades["velocidade"], Value::from(1.0));

    let velocidade = chamar(
        &client,
        "clip_set_speed",
        serde_json::json!({ "clip_ref": clip_ref, "speed": 2.0 }),
    )
    .await;
    assert_ok(&velocidade, "clip_set_speed");

    let invertido = chamar(&client, "clip_reverse", serde_json::json!({ "clip_ref": clip_ref })).await;
    assert_ok(&invertido, "clip_reverse");
    assert_eq!(invertido["velocidade"], Value::from(-2.0), "deveria inverter a velocidade atual (2.0 -> -2.0)");

    let opacidade = chamar(
        &client,
        "clip_set_opacity",
        serde_json::json!({ "clip_ref": clip_ref, "opacity": 0.5 }),
    )
    .await;
    assert_ok(&opacidade, "clip_set_opacity");

    // --- sessão: undo/redo/status/history -------------------------------
    let status = chamar(&client, "session_status", serde_json::json!({})).await;
    assert_ok(&status, "session_status");
    assert_eq!(status["projeto_aberto"], Value::Bool(true));
    let profundidade_antes = status["profundidade_historico"].as_u64().unwrap();
    assert!(profundidade_antes > 0, "deveria ter histórico depois de tantas edições");

    let historico = chamar(&client, "session_history", serde_json::json!({})).await;
    assert_ok(&historico, "session_history");
    assert_eq!(historico["historico"].as_array().unwrap().len() as u64, profundidade_antes);

    let desfeito = chamar(&client, "session_undo", serde_json::json!({})).await;
    assert_ok(&desfeito, "session_undo");
    assert_eq!(desfeito["passos_desfeitos"], Value::from(1));

    let refeito = chamar(&client, "session_redo", serde_json::json!({})).await;
    assert_ok(&refeito, "session_redo");
    assert_eq!(refeito["passos_refeitos"], Value::from(1));

    // --- filtros ---------------------------------------------------------
    let disponiveis = chamar(&client, "filter_list_available", serde_json::json!({})).await;
    assert_ok(&disponiveis, "filter_list_available");
    assert!(
        disponiveis["filtros_disponiveis"]
            .as_array()
            .is_some_and(|f| f.iter().any(|s| s == "brightness")),
        "esperava \"brightness\" na lista de filtros disponíveis: {disponiveis}"
    );

    let filtro = chamar(
        &client,
        "filter_add",
        serde_json::json!({ "clip_ref": clip_ref, "service": "brightness" }),
    )
    .await;
    assert_ok(&filtro, "filter_add");
    // pode já ter um filtro "brightness" de clip_set_opacity; só confirmamos que agora há pelo menos 1
    let lista_filtros = chamar(&client, "filter_list", serde_json::json!({ "clip_ref": clip_ref })).await;
    assert_ok(&lista_filtros, "filter_list");
    assert!(!lista_filtros["filtros"].as_array().unwrap().is_empty());
    let filter_id = filtro["filter_id"].as_str().unwrap().to_string();

    let ajustado = chamar(
        &client,
        "filter_set_param",
        serde_json::json!({ "clip_ref": clip_ref, "filter_id": filter_id, "param": "level", "value": "2.0" }),
    )
    .await;
    assert_ok(&ajustado, "filter_set_param");

    let removido_filtro = chamar(
        &client,
        "filter_remove",
        serde_json::json!({ "clip_ref": clip_ref, "filter_id": filter_id }),
    )
    .await;
    assert_ok(&removido_filtro, "filter_remove");

    // --- transições -------------------------------------------------------
    let transicao = chamar(
        &client,
        "transition_add",
        serde_json::json!({ "service": "luma", "track_a": track_a, "track_b": track_b }),
    )
    .await;
    assert_ok(&transicao, "transition_add");
    let transition_id = transicao["transition_id"].as_str().unwrap().to_string();

    let lista_transicoes = chamar(&client, "transition_list", serde_json::json!({})).await;
    assert_ok(&lista_transicoes, "transition_list");
    assert_eq!(lista_transicoes["transicoes"].as_array().unwrap().len(), 1);

    let transicao_ajustada = chamar(
        &client,
        "transition_set",
        serde_json::json!({ "transition_id": transition_id, "param": "softness", "value": "0.3" }),
    )
    .await;
    assert_ok(&transicao_ajustada, "transition_set");

    let transicao_removida = chamar(
        &client,
        "transition_remove",
        serde_json::json!({ "transition_id": transition_id }),
    )
    .await;
    assert_ok(&transicao_removida, "transition_remove");

    // --- guias -----------------------------------------------------------
    let guia = chamar(&client, "guide_add", serde_json::json!({ "position": 100, "comment": "corte 1" })).await;
    assert_ok(&guia, "guide_add");
    let guide_id = guia["guide_id"].as_str().unwrap().to_string();

    let lista_guias = chamar(&client, "guide_list", serde_json::json!({})).await;
    assert_ok(&lista_guias, "guide_list");
    assert_eq!(lista_guias["guias"].as_array().unwrap().len(), 1);

    let guia_lida = chamar(&client, "guide_get", serde_json::json!({ "guide_id": guide_id })).await;
    assert_ok(&guia_lida, "guide_get");
    assert_eq!(guia_lida["position"], Value::from(100));

    chamar(
        &client,
        "guide_update",
        serde_json::json!({ "guide_id": guide_id, "position": 150 }),
    )
    .await;
    let guia_atualizada = chamar(&client, "guide_get", serde_json::json!({ "guide_id": guide_id })).await;
    assert_eq!(guia_atualizada["position"], Value::from(150));
    assert_eq!(guia_atualizada["comment"], Value::from("corte 1"), "não deveria ter mudado o comentário");

    let guia_removida = chamar(&client, "guide_remove", serde_json::json!({ "guide_id": guide_id })).await;
    assert_ok(&guia_removida, "guide_remove");
    assert!(chamar(&client, "guide_list", serde_json::json!({})).await["guias"].as_array().unwrap().is_empty());

    // --- marcadores --------------------------------------------------------
    let marcador_timeline = chamar(
        &client,
        "marker_add",
        serde_json::json!({ "position": 20, "comment": "marcador geral" }),
    )
    .await;
    assert_ok(&marcador_timeline, "marker_add (timeline)");
    let marcador_clipe = chamar(
        &client,
        "marker_add",
        serde_json::json!({ "position": 5, "comment": "marcador do clipe", "clip_ref": clip_ref }),
    )
    .await;
    assert_ok(&marcador_clipe, "marker_add (clipe)");

    let lista_marcadores = chamar(&client, "marker_list", serde_json::json!({})).await;
    assert_ok(&lista_marcadores, "marker_list");
    assert_eq!(lista_marcadores["marcadores"].as_array().unwrap().len(), 2);

    let id_marcador_timeline = marcador_timeline["marker_id"].as_str().unwrap().to_string();
    let marcador_removido = chamar(&client, "marker_remove", serde_json::json!({ "marker_id": id_marcador_timeline })).await;
    assert_ok(&marcador_removido, "marker_remove");
    assert_eq!(
        chamar(&client, "marker_list", serde_json::json!({})).await["marcadores"].as_array().unwrap().len(),
        1
    );

    let _ = std::fs::remove_file(&midia);
    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Exercita perfis de projeto, pastas/mover/renomear da bin, importar uma
/// pasta inteira e montagem em lote por intervalos — Fase 2, grupo 4.
#[tokio::test]
async fn fase2_perfis_pastas_da_bin_importar_pasta_e_montagem_em_lote() {
    let client = iniciar_cliente().await;

    let perfis = chamar(&client, "project_list_profiles", serde_json::json!({})).await;
    assert_ok(&perfis, "project_list_profiles");
    assert!(
        perfis["perfis"].as_array().is_some_and(|p| p.iter().any(|x| x["name"] == "atsc_720p_25")),
        "esperava \"atsc_720p_25\" na lista de perfis: {perfis}"
    );

    chamar(&client, "project_new", serde_json::json!({ "profile": "atsc_720p_25", "default_tracks": false })).await;
    let perfil_atual = chamar(&client, "project_get_profile", serde_json::json!({})).await;
    assert_ok(&perfil_atual, "project_get_profile");
    assert_eq!(perfil_atual["perfil"]["largura"], Value::from(1280));

    let trocado = chamar(
        &client,
        "project_set_profile",
        serde_json::json!({ "profile": "uhd_2160p_30" }),
    )
    .await;
    assert_ok(&trocado, "project_set_profile");
    let perfil_novo = chamar(&client, "project_get_profile", serde_json::json!({})).await;
    assert_eq!(perfil_novo["perfil"]["largura"], Value::from(3840));

    // --- pastas da bin, mover e renomear ---------------------------------
    let pasta_raiz = chamar(
        &client,
        "bin_create_folder",
        serde_json::json!({ "name": "Brutos" }),
    )
    .await;
    assert_ok(&pasta_raiz, "bin_create_folder");
    let folder_id = pasta_raiz["folder_id"].as_str().unwrap().to_string();

    let midia = caminho_temporario("fase2_grupo4_midia.mp4");
    let _ = std::fs::remove_file(&midia);
    chamar(
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
    let importado = chamar(&client, "bin_import_clip", serde_json::json!({ "path": midia.to_string_lossy() })).await;
    let clip_id = importado["clip_id"].as_str().unwrap().to_string();

    let movido = chamar(
        &client,
        "bin_move_clip",
        serde_json::json!({ "clip_id": clip_id, "folder_id": folder_id }),
    )
    .await;
    assert_ok(&movido, "bin_move_clip");

    let renomeado = chamar(
        &client,
        "bin_rename",
        serde_json::json!({ "clip_id": clip_id, "name": "clipe renomeado" }),
    )
    .await;
    assert_ok(&renomeado, "bin_rename");
    let detalhes = chamar(&client, "bin_get_clip_details", serde_json::json!({ "clip_id": clip_id })).await;
    assert_eq!(detalhes["nome"], Value::from("clipe renomeado"));

    // --- importar uma pasta inteira ---------------------------------------
    let pasta_midia = std::env::temp_dir().join(format!("kdenlive_mcp_plus_pasta_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta_midia);
    std::fs::create_dir_all(&pasta_midia).expect("cria pasta de mídia de teste");
    std::fs::copy(&midia, pasta_midia.join("copia1.mp4")).expect("copia mídia 1");
    std::fs::copy(&midia, pasta_midia.join("copia2.mp4")).expect("copia mídia 2");
    std::fs::write(pasta_midia.join("nao_e_midia.txt"), b"ignorar").expect("cria arquivo não-mídia");

    let pasta_importada = chamar(
        &client,
        "bin_import_folder",
        serde_json::json!({ "path": pasta_midia.to_string_lossy() }),
    )
    .await;
    assert_ok(&pasta_importada, "bin_import_folder");
    assert_eq!(
        pasta_importada["importados"].as_array().map(Vec::len),
        Some(2),
        "deveria ter importado só os 2 arquivos de mídia, ignorando o .txt: {pasta_importada}"
    );

    // --- montagem em lote por intervalos -----------------------------------
    let trilha = chamar(&client, "timeline_add_track", serde_json::json!({ "kind": "video" })).await;
    let track_id = trilha["track_id"].as_str().unwrap().to_string();

    let montagem = chamar(
        &client,
        "timeline_assemble_ranges",
        serde_json::json!({
            "track_id": track_id,
            "producer_id": clip_id,
            "ranges": [[0, 9], [20, 29], [40, 44]],
            "position": 0,
        }),
    )
    .await;
    assert_ok(&montagem, "timeline_assemble_ranges");
    let refs = montagem["clip_refs"].as_array().expect("deveria ter clip_refs");
    assert_eq!(refs.len(), 3);
    assert_eq!(refs[0], Value::from(format!("{track_id}:0")));
    assert_eq!(refs[1], Value::from(format!("{track_id}:1")));
    assert_eq!(refs[2], Value::from(format!("{track_id}:2")));

    let info = chamar(&client, "timeline_get_info", serde_json::json!({})).await;
    // 10 + 10 + 5 quadros = 25
    assert_eq!(info["duracao_quadros"], Value::from(25));

    let _ = std::fs::remove_file(&midia);
    let _ = std::fs::remove_dir_all(&pasta_midia);
    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Exercita mover/aparar/dividir/remover clipe e posição/zoom da timeline
/// — Fase 2, grupo 5.
#[tokio::test]
async fn fase2_mover_aparar_dividir_remover_clipe_e_posicao_zoom() {
    let client = iniciar_cliente().await;

    chamar(&client, "project_new", serde_json::json!({ "default_tracks": false })).await;
    let trilha = chamar(&client, "timeline_add_track", serde_json::json!({ "kind": "video" })).await;
    let track_id = trilha["track_id"].as_str().unwrap().to_string();

    let midia = caminho_temporario("fase2_grupo5_midia.mp4");
    let _ = std::fs::remove_file(&midia);
    chamar(
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
    let importado = chamar(&client, "bin_import_clip", serde_json::json!({ "path": midia.to_string_lossy() })).await;
    let clip_id = importado["clip_id"].as_str().unwrap().to_string();

    let c0 = chamar(
        &client,
        "timeline_add_clip",
        serde_json::json!({ "track_id": track_id, "producer_id": clip_id, "in_frame": 0, "out_frame": 99, "position": 0 }),
    )
    .await;
    let clip_ref_0 = c0["clip_ref"].as_str().unwrap().to_string();
    let c1 = chamar(
        &client,
        "timeline_add_clip",
        serde_json::json!({ "track_id": track_id, "producer_id": clip_id, "in_frame": 0, "out_frame": 9, "position": 100 }),
    )
    .await;
    let clip_ref_1 = c1["clip_ref"].as_str().unwrap().to_string();

    let duracao = chamar(&client, "timeline_get_duration", serde_json::json!({})).await;
    assert_ok(&duracao, "timeline_get_duration");
    assert_eq!(duracao["duracao_quadros"], Value::from(110));

    // --- divide o primeiro clipe em 2 ------------------------------------
    let dividido = chamar(
        &client,
        "timeline_split_clip",
        serde_json::json!({ "clip_ref": clip_ref_0, "split_at": 40 }),
    )
    .await;
    assert_ok(&dividido, "timeline_split_clip");
    let ref_primeiro = dividido["clip_ref_primeiro"].as_str().unwrap().to_string();
    let ref_segundo = dividido["clip_ref_segundo"].as_str().unwrap().to_string();

    let duracao_apos_split = chamar(&client, "timeline_get_duration", serde_json::json!({})).await;
    assert_eq!(
        duracao_apos_split["duracao_quadros"], Value::from(110),
        "dividir não deveria mudar a duração total"
    );

    // --- apara o primeiro pedaço (encurta) -------------------------------
    let aparado = chamar(
        &client,
        "timeline_trim_clip",
        serde_json::json!({ "clip_ref": ref_primeiro, "in_frame": 0, "out_frame": 19 }),
    )
    .await;
    assert_ok(&aparado, "timeline_trim_clip");
    let duracao_apos_trim = chamar(&client, "timeline_get_duration", serde_json::json!({})).await;
    assert_eq!(
        duracao_apos_trim["duracao_quadros"], Value::from(90),
        "encurtar o primeiro pedaço de 40 pra 20 quadros deveria encurtar a trilha em 20"
    );

    // --- move o segundo pedaço do split para o início --------------------
    let movido = chamar(
        &client,
        "timeline_move_clip",
        serde_json::json!({ "clip_ref": ref_segundo, "new_position": 0 }),
    )
    .await;
    assert_ok(&movido, "timeline_move_clip");

    // --- remove um clipe (clip_ref_1, o segundo clipe original) ----------
    let removido = chamar(&client, "timeline_remove_clip", serde_json::json!({ "clip_ref": clip_ref_1 })).await;
    assert_ok(&removido, "timeline_remove_clip");

    // --- posição (playhead) e zoom: conceitos só de sessão -----------------
    let posicao_inicial = chamar(&client, "timeline_get_position", serde_json::json!({})).await;
    assert_ok(&posicao_inicial, "timeline_get_position");
    assert_eq!(posicao_inicial["position"], Value::from(0));

    let buscado = chamar(&client, "timeline_seek", serde_json::json!({ "position": 42 })).await;
    assert_ok(&buscado, "timeline_seek");
    let posicao_depois = chamar(&client, "timeline_get_position", serde_json::json!({})).await;
    assert_eq!(posicao_depois["position"], Value::from(42));

    let zoom_invalido = chamar(&client, "timeline_set_zoom", serde_json::json!({ "zoom": 20.0 })).await;
    assert_eq!(zoom_invalido["ok"], Value::Bool(false), "zoom fora do intervalo deveria falhar");

    let zoom_ok = chamar(&client, "timeline_set_zoom", serde_json::json!({ "zoom": 2.5 })).await;
    assert_ok(&zoom_ok, "timeline_set_zoom");
    let zoom_lido = chamar(&client, "timeline_get_zoom", serde_json::json!({})).await;
    assert_eq!(zoom_lido["zoom"], Value::from(2.5));

    let _ = std::fs::remove_file(&midia);
    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Confirma que dois renders assíncronos (`export_render` com `wait=false`)
/// nunca rodam ao mesmo tempo: o segundo fica "na fila" enquanto o primeiro
/// está "em andamento" (um render de verdade por vez, decisão D-016,
/// agora valendo também para jobs assíncronos, não só para o caminho
/// síncrono).
#[tokio::test]
async fn jobs_assincronos_sao_serializados_pelo_mesmo_render_lock() {
    let client = iniciar_cliente().await;

    let saida1 = caminho_temporario("fila_job1.mp4");
    let saida2 = caminho_temporario("fila_job2.mp4");
    let _ = std::fs::remove_file(&saida1);
    let _ = std::fs::remove_file(&saida2);

    let job1 = chamar(
        &client,
        "export_render",
        serde_json::json!({
            "project_path": caminho_fixture_media().to_string_lossy(),
            "output_path": saida1.to_string_lossy(),
            "preset": "h264_hq",
            "overwrite": true,
            "wait": false,
        }),
    )
    .await;
    let job1_id = job1["job_id"].as_str().unwrap().to_string();

    let job2 = chamar(
        &client,
        "export_render",
        serde_json::json!({
            "project_path": caminho_fixture_media().to_string_lossy(),
            "output_path": saida2.to_string_lossy(),
            "preset": "h264_hq",
            "overwrite": true,
            "wait": false,
        }),
    )
    .await;
    let job2_id = job2["job_id"].as_str().unwrap().to_string();

    // dá um instante para o job1 realmente começar a rodar
    tokio::time::sleep(Duration::from_millis(200)).await;

    let status1 = chamar(&client, "render_progress", serde_json::json!({ "job_id": job1_id })).await;
    let status2 = chamar(&client, "render_progress", serde_json::json!({ "job_id": job2_id })).await;
    assert_eq!(
        status1["status"], Value::from("em andamento"),
        "job1 deveria já estar rodando: {status1}"
    );
    assert_eq!(
        status2["status"], Value::from("na fila"),
        "job2 deveria estar esperando o render_lock, não rodando ao mesmo tempo que o job1: {status2}"
    );

    // espera os dois terminarem (a fixture "média" dura ~2s cada)
    for _ in 0..50 {
        let s1 = chamar(&client, "render_progress", serde_json::json!({ "job_id": job1_id })).await;
        let s2 = chamar(&client, "render_progress", serde_json::json!({ "job_id": job2_id })).await;
        if s1["status"] == "concluído" && s2["status"] == "concluído" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    let final1 = chamar(&client, "render_progress", serde_json::json!({ "job_id": job1_id })).await;
    let final2 = chamar(&client, "render_progress", serde_json::json!({ "job_id": job2_id })).await;
    assert_eq!(final1["status"], Value::from("concluído"), "job1: {final1}");
    assert_eq!(final2["status"], Value::from("concluído"), "job2: {final2}");
    assert!(saida1.exists());
    assert!(saida2.exists());

    let _ = std::fs::remove_file(&saida1);
    let _ = std::fs::remove_file(&saida2);
    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Exercita a fila de render de ponta a ponta, incluindo a correção do bug
/// do projeto original: `render_queue_stop` aqui encerra de verdade o
/// render em andamento (processo filho morto) e cancela os pendentes, em
/// vez de só marcar um estado interno.
#[tokio::test]
async fn render_queue_processa_em_sequencia_e_stop_encerra_de_verdade() {
    let client = iniciar_cliente().await;

    let saida1 = caminho_temporario("fila_render_queue1.mp4");
    let saida2 = caminho_temporario("fila_render_queue2.mp4");
    let saida3 = caminho_temporario("fila_render_queue3.mp4");
    for s in [&saida1, &saida2, &saida3] {
        let _ = std::fs::remove_file(s);
    }

    let status_vazia = chamar(&client, "render_queue_status", serde_json::json!({})).await;
    assert_ok(&status_vazia, "render_queue_status (vazia)");
    assert_eq!(status_vazia["processando"], Value::Bool(false));

    for saida in [&saida1, &saida2, &saida3] {
        let adicionado = chamar(
            &client,
            "render_queue_add",
            serde_json::json!({
                "project_path": caminho_fixture_media().to_string_lossy(),
                "output_path": saida.to_string_lossy(),
                "preset": "h264_hq",
                "overwrite": true,
            }),
        )
        .await;
        assert_ok(&adicionado, "render_queue_add");
        assert_eq!(adicionado["item"]["status"], Value::from("pendente"));
    }

    let lista = chamar(&client, "render_queue_list", serde_json::json!({})).await;
    assert_ok(&lista, "render_queue_list");
    assert_eq!(lista["fila"].as_array().map(Vec::len), Some(3));

    // começar de novo antes de start deveria falhar (ainda não tem nada rodando pra parar)
    let parar_sem_rodar = chamar(&client, "render_queue_stop", serde_json::json!({})).await;
    assert_eq!(parar_sem_rodar["ok"], Value::Bool(false));

    let iniciado = chamar(&client, "render_queue_start", serde_json::json!({})).await;
    assert_ok(&iniciado, "render_queue_start");

    let iniciar_de_novo = chamar(&client, "render_queue_start", serde_json::json!({})).await;
    assert_eq!(iniciar_de_novo["ok"], Value::Bool(false), "não deveria processar a fila duas vezes ao mesmo tempo");

    // dá tempo do primeiro job realmente começar a rodar
    tokio::time::sleep(Duration::from_millis(300)).await;
    let status_rodando = chamar(&client, "render_queue_status", serde_json::json!({})).await;
    assert_eq!(status_rodando["processando"], Value::Bool(true));
    assert_eq!(status_rodando["em_andamento"], Value::from(1));

    // para a fila: deve matar o processo do job em andamento de verdade e
    // cancelar os pendentes, sem deixar nenhum arquivo do que foi
    // interrompido.
    let parado = chamar(&client, "render_queue_stop", serde_json::json!({})).await;
    assert_ok(&parado, "render_queue_stop");

    // espera a fila realmente terminar de processar (o job em andamento é
    // encerrado, e o loop de processamento sai)
    let mut processando_no_final = true;
    for _ in 0..30 {
        let s = chamar(&client, "render_queue_status", serde_json::json!({})).await;
        if s["processando"] == Value::Bool(false) {
            processando_no_final = false;
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(!processando_no_final, "a fila deveria ter parado de processar depois do stop");

    let lista_final = chamar(&client, "render_queue_list", serde_json::json!({})).await;
    let itens = lista_final["fila"].as_array().expect("deveria ter itens");
    let cancelados = itens.iter().filter(|i| i["status"] == "cancelado").count();
    assert!(cancelados >= 1, "pelo menos o job em andamento (ou os pendentes) deveriam ter sido cancelados: {lista_final}");

    for saida in [&saida1, &saida2, &saida3] {
        let _ = std::fs::remove_file(saida);
    }
    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Exercita títulos, keyframes, legendas (SRT), áudio, transformação de
/// clipe e miniatura — Fase 2, grupo 7.
#[tokio::test]
async fn fase2_titulos_keyframes_legendas_audio_transformacao_e_miniatura() {
    let client = iniciar_cliente().await;

    chamar(&client, "project_new", serde_json::json!({ "default_tracks": false })).await;

    // --- títulos -----------------------------------------------------------
    let titulo = chamar(
        &client,
        "title_add",
        serde_json::json!({ "text": "Olá, mundo!", "duration_frames": 50 }),
    )
    .await;
    assert_ok(&titulo, "title_add");
    let titulo_id = titulo["clip_id"].as_str().unwrap().to_string();

    let atualizado = chamar(
        &client,
        "title_update",
        serde_json::json!({ "clip_id": titulo_id, "text": "Texto novo", "font_size": 72 }),
    )
    .await;
    assert_ok(&atualizado, "title_update");
    let detalhes_titulo = chamar(&client, "bin_get_clip_details", serde_json::json!({ "clip_id": titulo_id })).await;
    assert_eq!(detalhes_titulo["nome"], Value::from("Texto novo"));

    // atualizar um clipe que não é título deve falhar
    let midia = caminho_temporario("fase2_grupo7_midia.mp4");
    let _ = std::fs::remove_file(&midia);
    chamar(
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
    let clipe_normal = chamar(&client, "bin_import_clip", serde_json::json!({ "path": midia.to_string_lossy() })).await;
    let clip_id_normal = clipe_normal["clip_id"].as_str().unwrap().to_string();
    let falha_update = chamar(
        &client,
        "title_update",
        serde_json::json!({ "clip_id": clip_id_normal, "text": "x" }),
    )
    .await;
    assert_eq!(falha_update["ok"], Value::Bool(false));

    // --- keyframes -----------------------------------------------------------
    let trilha = chamar(&client, "timeline_add_track", serde_json::json!({ "kind": "video" })).await;
    let track_id = trilha["track_id"].as_str().unwrap().to_string();
    let posicionado = chamar(
        &client,
        "timeline_add_clip",
        serde_json::json!({ "track_id": track_id, "producer_id": clip_id_normal, "in_frame": 0, "out_frame": 49, "position": 0 }),
    )
    .await;
    let clip_ref = posicionado["clip_ref"].as_str().unwrap().to_string();

    let filtro = chamar(
        &client,
        "filter_add",
        serde_json::json!({ "clip_ref": clip_ref, "service": "brightness" }),
    )
    .await;
    let filter_id = filtro["filter_id"].as_str().unwrap().to_string();

    chamar(
        &client,
        "keyframe_set",
        serde_json::json!({ "clip_ref": clip_ref, "filter_id": filter_id, "param": "level", "frame": 0, "value": "0.0" }),
    )
    .await;
    chamar(
        &client,
        "keyframe_set",
        serde_json::json!({ "clip_ref": clip_ref, "filter_id": filter_id, "param": "level", "frame": 25, "value": "1.0" }),
    )
    .await;
    let lista_kf = chamar(
        &client,
        "keyframe_list",
        serde_json::json!({ "clip_ref": clip_ref, "filter_id": filter_id, "param": "level" }),
    )
    .await;
    assert_ok(&lista_kf, "keyframe_list");
    assert_eq!(lista_kf["keyframes"].as_array().map(Vec::len), Some(2));

    let kf_removido = chamar(
        &client,
        "keyframe_remove",
        serde_json::json!({ "clip_ref": clip_ref, "filter_id": filter_id, "param": "level", "frame": 0 }),
    )
    .await;
    assert_ok(&kf_removido, "keyframe_remove");

    // --- áudio -----------------------------------------------------------
    let nivel = chamar(&client, "audio_set_levels", serde_json::json!({ "clip_ref": clip_ref, "db": -6.0 })).await;
    assert_ok(&nivel, "audio_set_levels");
    let normalizado = chamar(&client, "audio_normalize", serde_json::json!({ "clip_ref": clip_ref })).await;
    assert_ok(&normalizado, "audio_normalize");

    // --- transformação -----------------------------------------------------
    let transformado = chamar(
        &client,
        "clip_set_transform",
        serde_json::json!({ "clip_ref": clip_ref, "x": 0, "y": 0, "width": 640, "height": 360, "rotation": 15.0 }),
    )
    .await;
    assert_ok(&transformado, "clip_set_transform");

    // --- legendas (SRT) -----------------------------------------------------
    let srt_path = caminho_temporario("fase2_grupo7_legendas.srt");
    std::fs::write(
        &srt_path,
        "1\n00:00:00,000 --> 00:00:01,000\nPrimeira legenda\n\n2\n00:00:01,200 --> 00:00:02,000\nSegunda legenda\n",
    )
    .expect("escreve srt de teste");

    let importado_srt = chamar(
        &client,
        "subtitle_import_srt",
        serde_json::json!({ "path": srt_path.to_string_lossy() }),
    )
    .await;
    assert_ok(&importado_srt, "subtitle_import_srt");
    assert_eq!(importado_srt["quantidade_legendas"], Value::from(2));
    let trilha_legendas = importado_srt["track_id"].as_str().unwrap().to_string();

    let srt_exportado = caminho_temporario("fase2_grupo7_legendas_exportadas.srt");
    let _ = std::fs::remove_file(&srt_exportado);
    let exportado_srt = chamar(
        &client,
        "subtitle_export_srt",
        serde_json::json!({ "track_id": trilha_legendas, "path": srt_exportado.to_string_lossy() }),
    )
    .await;
    assert_ok(&exportado_srt, "subtitle_export_srt");
    assert_eq!(exportado_srt["quantidade_legendas"], Value::from(2));
    let conteudo_exportado = std::fs::read_to_string(&srt_exportado).expect("deveria ter escrito o srt");
    assert!(conteudo_exportado.contains("Primeira legenda"));
    assert!(conteudo_exportado.contains("Segunda legenda"));

    // --- miniatura -----------------------------------------------------------
    let thumb_path = caminho_temporario("fase2_grupo7_thumb.png");
    let _ = std::fs::remove_file(&thumb_path);
    let miniatura = chamar(
        &client,
        "export_thumbnail",
        serde_json::json!({ "video_path": midia.to_string_lossy(), "output_path": thumb_path.to_string_lossy(), "at_seconds": 0.2 }),
    )
    .await;
    assert_ok(&miniatura, "export_thumbnail");
    assert!(thumb_path.exists());

    for caminho in [&midia, &srt_path, &srt_exportado, &thumb_path] {
        let _ = std::fs::remove_file(caminho);
    }
    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Com `KDENLIVE_MCP_AUTOSAVE=1`, cada edição regrava o projeto no caminho
/// em que ele foi salvo pela primeira vez — sem `project_save` a cada passo.
#[tokio::test]
async fn autosave_regrava_o_arquivo_a_cada_edicao() {
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_autosave_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let alvo = dir.join("ao_vivo.kdenlive");

    let client = iniciar_cliente_com_env(&[("KDENLIVE_MCP_AUTOSAVE", "1")]).await;
    assert_ok(&chamar(&client, "project_new", serde_json::json!({})).await, "project_new");
    assert_ok(
        &chamar(&client, "project_save", serde_json::json!({ "path": alvo.to_string_lossy() })).await,
        "project_save",
    );
    let tamanho_inicial = std::fs::metadata(&alvo).expect("arquivo salvo").len();

    assert_ok(
        &chamar(&client, "guide_add", serde_json::json!({ "position": 10, "comment": "ao vivo" })).await,
        "guide_add",
    );
    let texto = std::fs::read_to_string(&alvo).expect("lê o arquivo");
    assert!(texto.contains("ao vivo"), "a guia deveria ter sido gravada sozinha:\n{texto}");
    assert!(std::fs::metadata(&alvo).unwrap().len() > tamanho_inicial);

    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Cria um `busctl` falso que registra cada chamada em `log` e responde
/// como se houvesse um Kdenlive (conexão `:1.5`) com o projeto `ao_vivo`
/// aberto — `modificado` vira o valor de `windowModified`.
/// `falhas_iniciais`: quantas consultas ao título da janela "demoram demais"
/// antes de ela responder (simula o Kdenlive ocupado carregando o projeto).
fn busctl_falso(dir: &std::path::Path, log: &std::path::Path, modificado: bool, falhas_iniciais: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("busctl_falso.sh");
    let corpo = format!(
        r#"#!/bin/bash
echo "${{@:4}}" >> "{log}"
case "$4" in
  list) echo '[{{"name":":1.5","pid":1,"process":"kdenlive","user":"j"}},{{"name":":1.6","pid":2,"process":"outro","user":"j"}}]' ;;
  get-property)
    if [ "$5" != ":1.5" ]; then echo "Object does not exist" >&2; exit 1; fi
    case "$8" in
      windowTitle)
        if [ "$(wc -l < "{contador}" 2>/dev/null || echo 0)" -lt {falhas} ]; then
          echo x >> "{contador}"; echo "Connection timed out" >&2; exit 1
        fi
        echo '{{"type":"s","data":"ao_vivo [*]/ HD 1080p 25 fps"}}' ;;
      windowModified) echo '{{"type":"b","data":{modificado}}}' ;;
    esac ;;
  call) echo "(chamado)" ;;
esac
"#,
        log = log.display(),
        modificado = modificado,
        contador = dir.join("falhas.cnt").display(),
        falhas = falhas_iniciais,
    );
    std::fs::write(&script, corpo).expect("grava o busctl falso");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("torna executável");
    script
}

/// Espera (até ~8 s) o log do `busctl` falso satisfazer `condicao`.
async fn esperar_log(log: &std::path::Path, condicao: impl Fn(&str) -> bool) -> String {
    for _ in 0..80 {
        let texto = std::fs::read_to_string(log).unwrap_or_default();
        if condicao(&texto) {
            return texto;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    std::fs::read_to_string(log).unwrap_or_default()
}

const CHAMADA_REVERT: &str = "call :1.5 /kdenlive/MainWindow_1/actions/file_revert org.qtproject.Qt.QAction trigger";

/// Com `KDENLIVE_MCP_AUTORELOAD=1`, depois de cada edição o servidor acha o
/// Kdenlive com o projeto aberto (pelo título da janela) e aciona o
/// File > Revert dele pelo D-Bus — aqui, contra um `busctl` falso.
#[tokio::test]
async fn autoreload_aciona_o_revert_do_kdenlive_que_mostra_o_projeto() {
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_autoreload_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let log = dir.join("busctl.log");
    let falso = busctl_falso(&dir, &log, false, 0);
    let alvo = dir.join("ao_vivo.kdenlive");

    let client = iniciar_cliente_com_env(&[
        ("KDENLIVE_MCP_AUTORELOAD", "1"),
        ("KDENLIVE_MCP_BUSCTL", &falso.to_string_lossy()),
    ])
    .await;
    assert_ok(&chamar(&client, "project_new", serde_json::json!({})).await, "project_new");
    assert_ok(
        &chamar(&client, "project_save", serde_json::json!({ "path": alvo.to_string_lossy() })).await,
        "project_save",
    );
    let texto = esperar_log(&log, |t| t.contains(CHAMADA_REVERT)).await;
    assert!(texto.contains(CHAMADA_REVERT), "project_save deveria ter pedido a recarga; log:\n{texto}");
    let chamadas_apos_save = texto.matches(CHAMADA_REVERT).count();

    // o autosave liga junto: uma edição regrava e recarrega de novo
    assert_ok(
        &chamar(&client, "guide_add", serde_json::json!({ "position": 10, "comment": "ao vivo" })).await,
        "guide_add",
    );
    let texto = esperar_log(&log, |t| t.matches(CHAMADA_REVERT).count() > chamadas_apos_save).await;
    assert!(
        texto.matches(CHAMADA_REVERT).count() > chamadas_apos_save,
        "a edição deveria ter pedido outra recarga; log:\n{texto}"
    );
    assert!(std::fs::read_to_string(&alvo).expect("lê o arquivo").contains("ao vivo"));

    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Se o Kdenlive tem alterações não salvas no projeto, o servidor não
/// recarrega (apagaria o trabalho do usuário e abriria um diálogo).
#[tokio::test]
async fn autoreload_nao_recarrega_se_o_kdenlive_tem_alteracoes_nao_salvas() {
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_autoreload_mod_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let log = dir.join("busctl.log");
    let falso = busctl_falso(&dir, &log, true, 0);
    let alvo = dir.join("ao_vivo.kdenlive");

    let client = iniciar_cliente_com_env(&[
        ("KDENLIVE_MCP_AUTORELOAD", "1"),
        ("KDENLIVE_MCP_BUSCTL", &falso.to_string_lossy()),
    ])
    .await;
    assert_ok(&chamar(&client, "project_new", serde_json::json!({})).await, "project_new");
    assert_ok(
        &chamar(&client, "project_save", serde_json::json!({ "path": alvo.to_string_lossy() })).await,
        "project_save",
    );
    // espera o servidor consultar o estado da janela; só então vale conferir
    let texto = esperar_log(&log, |t| t.contains("windowModified")).await;
    assert!(texto.contains("windowModified"), "o servidor deveria ter consultado a janela; log:\n{texto}");
    assert!(!texto.contains(CHAMADA_REVERT), "não deveria recarregar com alterações não salvas:\n{texto}");

    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Kdenlive ocupado (carregando): a consulta ao título "demora demais"; o
/// servidor espera e tenta de novo em vez de desistir — o projeto acaba
/// recarregado com o estado mais recente.
#[tokio::test]
async fn autoreload_tenta_de_novo_quando_o_kdenlive_esta_ocupado() {
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_autoreload_busy_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let log = dir.join("busctl.log");
    let falso = busctl_falso(&dir, &log, false, 2);
    let alvo = dir.join("ao_vivo.kdenlive");

    let client = iniciar_cliente_com_env(&[
        ("KDENLIVE_MCP_AUTORELOAD", "1"),
        ("KDENLIVE_MCP_BUSCTL", &falso.to_string_lossy()),
    ])
    .await;
    assert_ok(&chamar(&client, "project_new", serde_json::json!({})).await, "project_new");
    assert_ok(
        &chamar(&client, "project_save", serde_json::json!({ "path": alvo.to_string_lossy() })).await,
        "project_save",
    );
    let texto = esperar_log(&log, |t| t.contains(CHAMADA_REVERT)).await;
    assert!(texto.contains(CHAMADA_REVERT), "depois de ficar ocupado, deveria ter recarregado; log:\n{texto}");
    assert!(
        texto.matches("windowTitle").count() >= 3,
        "deveria ter consultado o título pelo menos 3 vezes (2 falhas + 1 sucesso); log:\n{texto}"
    );

    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// O Kdenlive 26 fecha ao abrir um projeto sem trilhas (verificado abrindo o
/// arquivo de verdade), então `project_new` já cria V1 e A1 — e
/// `project_validate` avisa quando um projeto fica sem trilha nenhuma.
#[tokio::test]
async fn project_new_cria_trilhas_padrao_e_validate_avisa_projeto_sem_trilhas() {
    let client = iniciar_cliente().await;

    let novo = chamar(&client, "project_new", serde_json::json!({})).await;
    assert_ok(&novo, "project_new");
    assert_eq!(novo["trilhas"], Value::from(2));
    let criadas = novo["new_tracks"].as_array().expect("new_tracks");
    assert_eq!(criadas.len(), 2);
    assert_eq!(criadas[0]["kind"], Value::from("video"));
    assert_eq!(criadas[1]["kind"], Value::from("audio"));
    let validacao = chamar(&client, "project_validate", serde_json::json!({})).await;
    assert!(
        !validacao["problemas"].to_string().contains("nenhuma trilha"),
        "com as trilhas padrão não deveria avisar: {validacao}"
    );

    let vazio = chamar(&client, "project_new", serde_json::json!({ "default_tracks": false })).await;
    assert_ok(&vazio, "project_new sem trilhas");
    assert_eq!(vazio["trilhas"], Value::from(0));
    assert_eq!(vazio["new_tracks"].as_array().map(Vec::len), Some(0));
    let validacao = chamar(&client, "project_validate", serde_json::json!({})).await;
    assert!(
        validacao["problemas"].to_string().contains("nenhuma trilha"),
        "projeto sem trilhas deveria gerar aviso: {validacao}"
    );

    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Cor (r, g, b) do pixel `(x, y)` de uma imagem, lida com o `ffmpeg`.
fn pixel(imagem: &std::path::Path, x: u32, y: u32) -> (u8, u8, u8) {
    let saida = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(imagem)
        .args(["-vf", &format!("crop=1:1:{x}:{y}"), "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .output()
        .expect("ffmpeg executa");
    assert!(saida.status.success(), "ffmpeg falhou: {}", String::from_utf8_lossy(&saida.stderr));
    assert_eq!(saida.stdout.len(), 3, "esperava 1 pixel RGB");
    (saida.stdout[0], saida.stdout[1], saida.stdout[2])
}

fn e_azul(c: (u8, u8, u8)) -> bool {
    c.2 > 150 && c.0 < 100 && c.1 < 100
}

fn e_vermelho(c: (u8, u8, u8)) -> bool {
    c.0 > 150 && c.1 < 100 && c.2 < 100
}

fn e_preto(c: (u8, u8, u8)) -> bool {
    c.0 < 40 && c.1 < 40 && c.2 < 40
}

/// `clip_reframe_vertical` de verdade: um vídeo 16:9 com a metade esquerda
/// azul e a direita vermelha vira um quadro 9:16, e o resultado é conferido
/// **no pixel** do quadro renderizado (centro, foco à esquerda/direita, fit).
#[tokio::test]
async fn clip_reframe_vertical_reenquadra_de_verdade_e_confere_os_pixels() {
    let ferramentas_ok = ["ffmpeg", "ffprobe"].iter().all(|p| {
        std::process::Command::new(p).arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
    });
    if !ferramentas_ok {
        eprintln!("ffmpeg/ffprobe não encontrados: teste ignorado");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_reframe_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let video = dir.join("azul_vermelho.mp4");
    let feito = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg("color=c=red:s=640x360:d=2:r=25,drawbox=x=0:y=0:w=320:h=360:color=blue:t=fill")
        .args(["-pix_fmt", "yuv420p"])
        .arg(&video)
        .status()
        .expect("ffmpeg executa");
    assert!(feito.success(), "não consegui gerar o vídeo de teste");

    let client = iniciar_cliente().await;
    assert_ok(
        &chamar(&client, "project_new", serde_json::json!({ "profile": "vertical_hd_30" })).await,
        "project_new vertical",
    );
    let importado = chamar(
        &client,
        "bin_import_clip",
        serde_json::json!({ "path": video.to_string_lossy(), "name": "azul e vermelho" }),
    )
    .await;
    assert_ok(&importado, "bin_import_clip");
    let clip_id = importado["clip_id"].as_str().expect("clip_id").to_string();
    let colocado = chamar(
        &client,
        "timeline_add_clip",
        serde_json::json!({ "track_id": "track0", "producer_id": clip_id, "in_frame": 0, "out_frame": 24, "position": 0 }),
    )
    .await;
    assert_ok(&colocado, "timeline_add_clip");

    let quadro = |nome: &str| dir.join(nome);
    let renderizar = |nome: &'static str| {
        let client = &client;
        let alvo = quadro(nome);
        async move {
            let _ = std::fs::remove_file(&alvo);
            let r = chamar(
                client,
                "preview_frame",
                serde_json::json!({ "frame": 5, "output_path": alvo.to_string_lossy(), "overwrite": true }),
            )
            .await;
            assert_ok(&r, "preview_frame");
            alvo
        }
    };

    // fill, centralizado: o corte passa bem no meio da emenda azul/vermelho
    let r = chamar(
        &client,
        "clip_reframe_vertical",
        serde_json::json!({ "clip_ref": "track0:0" }),
    )
    .await;
    assert_ok(&r, "clip_reframe_vertical fill");
    assert_eq!(r["source_size"]["width"], Value::from(640.0));
    assert_eq!(r["frame_size"]["height"], Value::from(1920.0));
    assert_eq!(r["rect"]["height"], Value::from(1920.0));
    assert_eq!(r["rect"]["x"], Value::from(-1166.67));
    let centro = renderizar("centro.png").await;
    assert!(e_azul(pixel(&centro, 100, 960)), "à esquerda do centro deveria ser azul: {:?}", pixel(&centro, 100, 960));
    assert!(e_vermelho(pixel(&centro, 980, 960)), "à direita do centro deveria ser vermelho: {:?}", pixel(&centro, 980, 960));
    // sem faixas pretas: nem em cima nem embaixo
    assert!(!e_preto(pixel(&centro, 100, 5)) && !e_preto(pixel(&centro, 980, 1915)));

    // fill com o foco na borda esquerda: só a parte azul fica visível
    assert_ok(
        &chamar(&client, "clip_reframe_vertical", serde_json::json!({ "clip_ref": "track0:0", "focus_x": 0.0 })).await,
        "clip_reframe_vertical foco 0",
    );
    let esquerda = renderizar("esquerda.png").await;
    assert!(e_azul(pixel(&esquerda, 100, 960)) && e_azul(pixel(&esquerda, 980, 960)));

    // foco na borda direita: só vermelho
    assert_ok(
        &chamar(&client, "clip_reframe_vertical", serde_json::json!({ "clip_ref": "track0:0", "focus_x": 1.0 })).await,
        "clip_reframe_vertical foco 1",
    );
    let direita = renderizar("direita.png").await;
    assert!(e_vermelho(pixel(&direita, 100, 960)) && e_vermelho(pixel(&direita, 980, 960)));

    // fit: o vídeo inteiro (azul | vermelho) no meio, faixas pretas em cima e embaixo
    let fit = chamar(
        &client,
        "clip_reframe_vertical",
        serde_json::json!({ "clip_ref": "track0:0", "mode": "fit" }),
    )
    .await;
    assert_ok(&fit, "clip_reframe_vertical fit");
    assert_eq!(fit["rect"]["width"], Value::from(1080.0));
    let caixa = renderizar("fit.png").await;
    assert!(e_preto(pixel(&caixa, 540, 100)) && e_preto(pixel(&caixa, 540, 1800)), "faixas pretas em cima e embaixo");
    assert!(e_azul(pixel(&caixa, 270, 960)) && e_vermelho(pixel(&caixa, 810, 960)));

    // reenquadrar de novo atualiza o mesmo filtro, não empilha outro
    let info = chamar(&client, "clip_get_properties", serde_json::json!({ "clip_ref": "track0:0" })).await;
    let qtblends = info["filtros"].as_array().expect("filtros").iter().filter(|f| f["service"] == "qtblend").count();
    assert_eq!(qtblends, 1, "deveria haver um único qtblend: {info}");

    // erros: modo inválido, clipe inexistente, projeto que não é vertical
    let ruim = chamar(
        &client,
        "clip_reframe_vertical",
        serde_json::json!({ "clip_ref": "track0:0", "mode": "esticar" }),
    )
    .await;
    assert_eq!(ruim["ok"], Value::Bool(false));
    let inexistente =
        chamar(&client, "clip_reframe_vertical", serde_json::json!({ "clip_ref": "track0:9" })).await;
    assert_eq!(inexistente["ok"], Value::Bool(false));
    assert_ok(
        &chamar(&client, "project_set_profile", serde_json::json!({ "profile": "atsc_1080p_25" })).await,
        "project_set_profile",
    );
    let horizontal =
        chamar(&client, "clip_reframe_vertical", serde_json::json!({ "clip_ref": "track0:0" })).await;
    assert_eq!(horizontal["ok"], Value::Bool(false));
    assert!(horizontal["erro"].as_str().unwrap_or("").contains("vertical"), "{horizontal}");

    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Nível médio, em dB, de um trecho de um áudio, medido com o `ffmpeg`
/// (`volumedetect`).
fn nivel_medio_db(audio: &std::path::Path, inicio_s: f64, duracao_s: f64) -> f64 {
    let saida = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-ss", &inicio_s.to_string(), "-t", &duracao_s.to_string(), "-i"])
        .arg(audio)
        .args(["-af", "volumedetect", "-f", "null", "-"])
        .output()
        .expect("ffmpeg executa");
    let texto = String::from_utf8_lossy(&saida.stderr).into_owned();
    texto
        .lines()
        .find_map(|l| l.split("mean_volume:").nth(1))
        .and_then(|r| r.trim().trim_end_matches("dB").trim().parse::<f64>().ok())
        .unwrap_or_else(|| panic!("não achei mean_volume em:\n{texto}"))
}

/// `audio_duck` de verdade: a música (seno de 440 Hz, 6 s) é abaixada 20 dB
/// só enquanto a voz (seno de 1000 Hz: 1 s de som, 1 s de pausa, 1 s de som,
/// a partir de 2 s) fala — e o resultado é conferido **medindo o áudio
/// renderizado** em janelas antes, durante e depois das falas.
#[tokio::test]
async fn audio_duck_abaixa_a_musica_so_enquanto_a_voz_fala_e_confere_o_audio() {
    let ferramentas_ok = ["ffmpeg", "ffprobe"].iter().all(|p| {
        std::process::Command::new(p).arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
    });
    if !ferramentas_ok {
        eprintln!("ffmpeg/ffprobe não encontrados: teste ignorado");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_duck_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let musica = dir.join("musica.wav");
    let voz = dir.join("voz.wav");
    let gerar = |args: &[&str], saida: &std::path::Path| {
        let ok = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y"])
            .args(args)
            .arg(saida)
            .status()
            .expect("ffmpeg executa")
            .success();
        assert!(ok, "não consegui gerar {}", saida.display());
    };
    gerar(&["-f", "lavfi", "-i", "sine=f=440:d=6:sample_rate=48000"], &musica);
    gerar(
        &["-f", "lavfi", "-i", "sine=f=1000:d=3:sample_rate=48000", "-af", "volume=enable='between(t,1,2)':volume=0"],
        &voz,
    );

    let client = iniciar_cliente().await;
    assert_ok(&chamar(&client, "project_new", serde_json::json!({})).await, "project_new");
    let trilha_voz = chamar(&client, "timeline_add_track", serde_json::json!({ "kind": "audio" })).await["track_id"]
        .as_str()
        .expect("track_id")
        .to_string();
    let importar = |nome: &'static str, arquivo: std::path::PathBuf| {
        let client = &client;
        async move {
            let r = chamar(
                client,
                "bin_import_clip",
                serde_json::json!({ "path": arquivo.to_string_lossy(), "name": nome }),
            )
            .await;
            assert_ok(&r, "bin_import_clip");
            r["clip_id"].as_str().expect("clip_id").to_string()
        }
    };
    let id_musica = importar("musica", musica.clone()).await;
    let id_voz = importar("voz", voz.clone()).await;
    // música na trilha de áudio padrão (track1), 0 a 6 s; voz na nova, de 2 s a 5 s
    assert_ok(
        &chamar(
            &client,
            "timeline_add_clip",
            serde_json::json!({ "track_id": "track1", "producer_id": id_musica, "in_frame": 0, "out_frame": 149, "position": 0 }),
        )
        .await,
        "timeline_add_clip música",
    );
    assert_ok(
        &chamar(
            &client,
            "timeline_add_clip",
            serde_json::json!({ "track_id": trilha_voz, "producer_id": id_voz, "in_frame": 0, "out_frame": 74, "position": 50 }),
        )
        .await,
        "timeline_add_clip voz",
    );
    // a voz fica muda no render: assim só a música é medida
    assert_ok(
        &chamar(&client, "track_set_muted", serde_json::json!({ "track_id": trilha_voz, "value": true })).await,
        "track_set_muted",
    );

    let renderizar = |nome: &'static str| {
        let client = &client;
        let alvo = dir.join(nome);
        async move {
            let _ = std::fs::remove_file(&alvo);
            let r = chamar(
                client,
                "export_render",
                serde_json::json!({ "output_path": alvo.to_string_lossy(), "preset": "audio_only", "overwrite": true }),
            )
            .await;
            assert_ok(&r, "export_render");
            alvo
        }
    };

    // referência: sem ducking
    let sem = renderizar("sem_ducking.wav").await;
    let base = nivel_medio_db(&sem, 0.2, 1.3);
    assert!(nivel_medio_db(&sem, 2.2, 0.6) > base - 3.0, "sem ducking o nível não deveria cair");

    // com detecção de fala: duas falas, de 2 a 3 s e de 4 a 5 s
    let duck = chamar(
        &client,
        "audio_duck",
        serde_json::json!({
            "clip_ref": "track1:0", "voice_track_id": trilha_voz,
            "duck_db": 20, "attack_frames": 5, "release_frames": 5, "detect_speech": true,
        }),
    )
    .await;
    assert_ok(&duck, "audio_duck detect_speech");
    let intervalos = duck["speech_intervals_frames"].as_array().expect("speech_intervals_frames");
    assert_eq!(intervalos.len(), 2, "duas falas esperadas: {duck}");
    let perto = |v: &Value, esperado: i64| (v.as_i64().unwrap() - esperado).abs() <= 2;
    assert!(perto(&intervalos[0][0], 50) && perto(&intervalos[0][1], 75), "{intervalos:?}");
    assert!(perto(&intervalos[1][0], 100) && perto(&intervalos[1][1], 125), "{intervalos:?}");
    assert_eq!(duck["targets"][0]["ducked"], Value::Bool(true));

    let com = renderizar("com_ducking.wav").await;
    let antes = nivel_medio_db(&com, 0.2, 1.3);
    let fala1 = nivel_medio_db(&com, 2.2, 0.6);
    let pausa = nivel_medio_db(&com, 3.4, 0.4);
    let fala2 = nivel_medio_db(&com, 4.2, 0.6);
    let depois = nivel_medio_db(&com, 5.3, 0.6);
    assert!((antes - base).abs() < 1.5, "antes da fala o nível não muda: {antes} vs {base}");
    assert!((base - fala1 - 20.0).abs() < 3.0, "durante a fala 1 deveria cair ~20 dB: base {base}, fala {fala1}");
    assert!((base - fala2 - 20.0).abs() < 3.0, "durante a fala 2 deveria cair ~20 dB: base {base}, fala {fala2}");
    assert!((base - pausa).abs() < 1.5, "na pausa entre as falas o volume volta: base {base}, pausa {pausa}");
    assert!((base - depois).abs() < 1.5, "depois das falas o volume volta: base {base}, depois {depois}");

    // sem detecção: o clipe da voz inteiro (2 a 5 s) conta, inclusive a pausa
    let inteiro = chamar(
        &client,
        "audio_duck",
        serde_json::json!({
            "clip_ref": "track1:0", "voice_track_id": trilha_voz,
            "duck_db": 20, "attack_frames": 5, "release_frames": 5,
        }),
    )
    .await;
    assert_ok(&inteiro, "audio_duck clipe inteiro");
    assert_eq!(inteiro["speech_intervals_frames"].as_array().map(Vec::len), Some(1));
    let inteiro_wav = renderizar("ducking_clipe_inteiro.wav").await;
    let pausa = nivel_medio_db(&inteiro_wav, 3.4, 0.4);
    assert!((base - pausa - 20.0).abs() < 3.0, "sem detect_speech a pausa também fica abaixada: base {base}, pausa {pausa}");

    // reaplicar troca o filtro de ducking em vez de empilhar
    let info = chamar(&client, "clip_get_properties", serde_json::json!({ "clip_ref": "track1:0" })).await;
    let volumes = info["filtros"].as_array().expect("filtros").iter().filter(|f| f["service"] == "volume").count();
    assert_eq!(volumes, 1, "deveria haver um único volume (o do ducking): {info}");

    // erros
    let sem_alvo = chamar(&client, "audio_duck", serde_json::json!({ "voice_track_id": trilha_voz })).await;
    assert_eq!(sem_alvo["ok"], Value::Bool(false));
    let mesma = chamar(
        &client,
        "audio_duck",
        serde_json::json!({ "clip_ref": format!("{trilha_voz}:0"), "voice_track_id": trilha_voz }),
    )
    .await;
    assert_eq!(mesma["ok"], Value::Bool(false));
    let sem_voz = chamar(&client, "audio_duck", serde_json::json!({ "clip_ref": "track1:0", "voice_track_id": "track1_nao_existe" })).await;
    assert_eq!(sem_voz["ok"], Value::Bool(false));
    let clipe_inexistente =
        chamar(&client, "audio_duck", serde_json::json!({ "clip_ref": "track1:9", "voice_track_id": trilha_voz })).await;
    assert_eq!(clipe_inexistente["ok"], Value::Bool(false));

    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Como o Claude Desktop inicia o servidor: sem `DISPLAY` nem `WAYLAND_DISPLAY`.
async fn iniciar_cliente_sem_tela() -> RunningService<rmcp::RoleClient, ()> {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_kdenlive-mcp-plus"));
    let mut comando = tokio::process::Command::new(exe);
    comando
        .arg("mcp")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("QT_QPA_PLATFORM");
    let transporte = TokioChildProcess::new(comando).expect("deveria conseguir iniciar o processo filho");
    ().serve(transporte).await.expect("o handshake initialize deveria funcionar")
}

/// Sem tela (como no Claude Desktop), o render de um título tem de sair com a
/// cor do título — e não o texto "INVALID" em branco sobre preto, que é o que o
/// módulo Qt do libmlt produz quando recusa o ambiente sem `DISPLAY`.
#[tokio::test]
async fn render_sem_display_desenha_o_titulo_e_nao_o_texto_invalid() {
    let ffmpeg_ok = std::process::Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false);
    if !ffmpeg_ok {
        eprintln!("ffmpeg não encontrado: teste ignorado");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_sem_tela_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let client = iniciar_cliente_sem_tela().await;
    assert_ok(&chamar(&client, "project_new", serde_json::json!({})).await, "project_new");
    // um painel azul-marinho que cobre o quadro inteiro
    let titulo = chamar(
        &client,
        "title_add",
        serde_json::json!({ "text": "████", "duration_frames": 30, "font_family": "DejaVu Sans Mono", "font_size": 1300, "color": "#0A1230" }),
    )
    .await;
    assert_ok(&titulo, "title_add");
    let clip_id = titulo["clip_id"].as_str().expect("clip_id").to_string();
    assert_ok(
        &chamar(
            &client,
            "timeline_add_clip",
            serde_json::json!({ "track_id": "track0", "producer_id": clip_id, "in_frame": 0, "out_frame": 29, "position": 0 }),
        )
        .await,
        "timeline_add_clip",
    );
    let png = dir.join("painel.png");
    assert_ok(
        &chamar(&client, "preview_frame", serde_json::json!({ "frame": 5, "output_path": png.to_string_lossy() })).await,
        "preview_frame sem DISPLAY",
    );
    let cor = pixel(&png, 960, 540);
    assert!(
        cor.2 > cor.0 && cor.2 > 20 && cor.0 < 40,
        "o canto deveria ser azul-marinho (#0A1230), e não preto/branco do INVALID: {cor:?}"
    );
    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Monta um projeto com um painel azul-marinho que **aparece** (opacidade 0 no
/// quadro 0, 1 a partir do quadro 20) e devolve o cliente já com ele aberto.
async fn cliente_com_painel_animado() -> RunningService<rmcp::RoleClient, ()> {
    let client = iniciar_cliente_sem_tela().await;
    assert_ok(&chamar(&client, "project_new", serde_json::json!({})).await, "project_new");
    let titulo = chamar(
        &client,
        "title_add",
        serde_json::json!({ "text": "████", "duration_frames": 60, "font_family": "DejaVu Sans Mono", "font_size": 1300, "color": "#0A1230" }),
    )
    .await;
    assert_ok(&titulo, "title_add");
    let clip_id = titulo["clip_id"].as_str().expect("clip_id").to_string();
    assert_ok(
        &chamar(
            &client,
            "timeline_add_clip",
            serde_json::json!({ "track_id": "track0", "producer_id": clip_id, "in_frame": 0, "out_frame": 59, "position": 0 }),
        )
        .await,
        "timeline_add_clip",
    );
    let t = chamar(
        &client,
        "clip_set_transform",
        serde_json::json!({ "clip_ref": "track0:0", "x": 0, "y": 0, "width": 1920, "height": 1080 }),
    )
    .await;
    assert_ok(&t, "clip_set_transform");
    let filtro = t["filter_id"].as_str().expect("filter_id").to_string();
    for (quadro, valor) in [(0, "0 0 1920 1080 0"), (20, "0 0 1920 1080 1")] {
        assert_ok(
            &chamar(
                &client,
                "keyframe_set",
                serde_json::json!({ "clip_ref": "track0:0", "filter_id": filtro, "param": "rect", "frame": quadro, "value": valor }),
            )
            .await,
            "keyframe_set",
        );
    }
    client
}

fn tem_ffmpeg() -> bool {
    std::process::Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// O `preview_frame` avalia a animação no quadro **pedido**, e não sempre no
/// quadro 0 (o que deixava um clipe que aparece com fade tudo preto), e
/// funciona sem `DISPLAY`, como no Claude Desktop.
#[tokio::test]
async fn preview_frame_avalia_a_animacao_no_quadro_pedido_e_sem_display() {
    if !tem_ffmpeg() {
        eprintln!("ffmpeg não encontrado: teste ignorado");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_anim_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let client = cliente_com_painel_animado().await;
    for (quadro, azul) in [(0, false), (40, true)] {
        let png = dir.join(format!("q{quadro}.png"));
        assert_ok(
            &chamar(
                &client,
                "preview_frame",
                serde_json::json!({ "frame": quadro, "output_path": png.to_string_lossy() }),
            )
            .await,
            "preview_frame",
        );
        let cor = pixel(&png, 960, 540);
        if azul {
            assert!(cor.2 > cor.0 && cor.2 > 20, "no quadro {quadro} o painel já apareceu (azul-marinho): {cor:?}");
        } else {
            assert!(e_preto(cor), "no quadro 0 a opacidade é 0 (preto): {cor:?}");
        }
    }
    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `preview_frames` renderiza vários quadros de uma vez, devolve um PNG por
/// quadro (na ordem pedida) e uma folha de contato; erros de quadro e de
/// combinação de parâmetros voltam como `ok: false`.
#[tokio::test]
async fn preview_frames_devolve_um_png_por_quadro_e_a_folha_de_contato() {
    if !tem_ffmpeg() {
        eprintln!("ffmpeg não encontrado: teste ignorado");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_frames_{}", std::process::id()));
    let client = cliente_com_painel_animado().await;

    let r = chamar(
        &client,
        "preview_frames",
        serde_json::json!({ "frames": [40, 0, 20], "output_dir": dir.to_string_lossy(), "columns": 2, "thumb_width": 240 }),
    )
    .await;
    assert_ok(&r, "preview_frames");
    let frames = r["frames"].as_array().expect("frames");
    let numeros: Vec<i64> = frames.iter().map(|f| f["frame"].as_i64().unwrap()).collect();
    assert_eq!(numeros, vec![40, 0, 20], "na ordem pedida");
    for f in frames {
        assert!(std::path::Path::new(f["path"].as_str().unwrap()).exists(), "{f}");
    }
    assert_eq!(r["grade"]["colunas"], Value::from(2));
    assert_eq!(r["grade"]["linhas"], Value::from(2));
    let folha = std::path::PathBuf::from(r["contact_sheet"].as_str().expect("contact_sheet"));
    assert!(folha.exists());
    // a folha tem 2 colunas de 240 px e 2 linhas: maior na largura que na altura
    let medidas = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0"])
        .arg(&folha)
        .output()
        .expect("ffprobe executa");
    let texto = String::from_utf8_lossy(&medidas.stdout).trim().to_string();
    let (w, h) = texto.split_once(',').expect("largura,altura");
    assert!(w.parse::<u32>().unwrap() > 480 && h.parse::<u32>().unwrap() > 270, "folha {texto}");

    // sem escolher nada: espalha quadros pelo projeto todo; sem folha se não quiser
    let auto = chamar(
        &client,
        "preview_frames",
        serde_json::json!({ "output_dir": dir.to_string_lossy(), "contact_sheet": false }),
    )
    .await;
    assert_ok(&auto, "preview_frames automático");
    assert!(auto["contact_sheet"].is_null());
    assert!(auto["frames"].as_array().unwrap().len() >= 6);

    // erros
    let fora = chamar(&client, "preview_frames", serde_json::json!({ "frames": [9999] })).await;
    assert_eq!(fora["ok"], Value::Bool(false));
    let mistura = chamar(&client, "preview_frames", serde_json::json!({ "frames": [1], "start": 0 })).await;
    assert_eq!(mistura["ok"], Value::Bool(false));

    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `preview_gif` gera um GIF do trecho, e `export_render` com
/// `start_frame`/`end_frame` renderiza só aquela parte (duração proporcional).
#[tokio::test]
async fn preview_gif_e_export_render_de_um_trecho() {
    if !tem_ffmpeg() {
        eprintln!("ffmpeg não encontrado: teste ignorado");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kdenlive_mcp_gif_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário");
    let client = cliente_com_painel_animado().await;

    let gif = dir.join("trecho.gif");
    let r = chamar(
        &client,
        "preview_gif",
        serde_json::json!({ "start": 0, "end": 59, "step": 5, "width": 240, "output_path": gif.to_string_lossy() }),
    )
    .await;
    assert_ok(&r, "preview_gif");
    // 0, 5, ..., 55 e mais o último quadro do intervalo (59)
    assert_eq!(r["quadros_usados"].as_array().map(Vec::len), Some(13));
    assert!(std::fs::metadata(&gif).expect("gif criado").len() > 500);
    let formato = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=codec_name,width", "-of", "csv=p=0"])
        .arg(&gif)
        .output()
        .expect("ffprobe executa");
    assert_eq!(String::from_utf8_lossy(&formato.stdout).trim(), "gif,240");
    // sobrescrever exige overwrite
    let de_novo = chamar(
        &client,
        "preview_gif",
        serde_json::json!({ "start": 0, "end": 10, "output_path": gif.to_string_lossy() }),
    )
    .await;
    assert_eq!(de_novo["ok"], Value::Bool(false));

    // só um trecho do render: 1 segundo (quadros 10 a 34 a 25 fps) de um projeto de 60 quadros
    let trecho = dir.join("trecho.mp4");
    let r = chamar(
        &client,
        "export_render",
        serde_json::json!({
            "output_path": trecho.to_string_lossy(), "preset": "h264_fast",
            "start_frame": 10, "end_frame": 34, "overwrite": true,
        }),
    )
    .await;
    assert_ok(&r, "export_render de um trecho");
    let duracao = r["duracao_segundos"].as_f64().expect("duração medida");
    assert!((duracao - 1.0).abs() < 0.2, "25 quadros a 25 fps = ~1 s, medi {duracao}");
    // intervalo pela metade, ou com wait=false: erro
    let metade = chamar(
        &client,
        "export_render",
        serde_json::json!({ "output_path": dir.join("x.mp4").to_string_lossy(), "start_frame": 3 }),
    )
    .await;
    assert_eq!(metade["ok"], Value::Bool(false));
    let sem_esperar = chamar(
        &client,
        "export_render",
        serde_json::json!({ "output_path": dir.join("y.mp4").to_string_lossy(), "start_frame": 0, "end_frame": 5, "wait": false }),
    )
    .await;
    assert_eq!(sem_esperar["ok"], Value::Bool(false));

    client.cancel().await.expect("fechamento limpo do cliente");
    let _ = std::fs::remove_dir_all(&dir);
}
