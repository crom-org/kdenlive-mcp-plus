//! Teste de contrato do servidor MCP via stdio: sobe o binário compilado em
//! modo `mcp`, faz o handshake `initialize`, confere que `tools/list` traz
//! exatamente `server_info` e `render_project`, chama as duas e confere a
//! estrutura das respostas (sucesso e erro usam o mesmo formato).

use std::path::PathBuf;

use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;

fn caminho_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/smoke.xml")
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

#[tokio::test]
async fn tools_list_traz_exatamente_as_duas_ferramentas() {
    let client = iniciar_cliente().await;

    let ferramentas = client
        .list_all_tools()
        .await
        .expect("tools/list deveria funcionar");
    let mut nomes: Vec<&str> = ferramentas.iter().map(|t| t.name.as_ref()).collect();
    nomes.sort_unstable();

    assert_eq!(nomes, vec!["render_project", "server_info"]);

    client.cancel().await.expect("fechamento limpo do cliente");
}

#[tokio::test]
async fn server_info_responde_com_presets_e_versoes() {
    let client = iniciar_cliente().await;

    let resultado = client
        .call_tool(CallToolRequestParams::new("server_info"))
        .await
        .expect("server_info deveria responder");
    let estruturado = resultado
        .structured_content
        .expect("server_info deveria trazer structured_content");

    assert_eq!(estruturado["ok"], serde_json::Value::Bool(true));
    assert!(
        estruturado["presets_disponiveis"]
            .as_array()
            .is_some_and(|presets| !presets.is_empty()),
        "resposta sem presets: {estruturado}"
    );
    assert!(
        !estruturado["versao_libmlt"]
            .as_str()
            .unwrap_or_default()
            .is_empty(),
        "resposta sem versão do libmlt: {estruturado}"
    );

    client.cancel().await.expect("fechamento limpo do cliente");
}

#[tokio::test]
async fn render_project_renderiza_o_fixture_e_mede_a_duracao() {
    let client = iniciar_cliente().await;
    let saida = caminho_temporario("render_ok.mp4");
    let _ = std::fs::remove_file(&saida);

    let argumentos = serde_json::json!({
        "project_path": caminho_fixture().to_string_lossy(),
        "output_path": saida.to_string_lossy(),
        "preset": "h264_hq",
        "overwrite": true,
    })
    .as_object()
    .cloned()
    .expect("deveria montar os argumentos da chamada");

    let resultado = client
        .call_tool(CallToolRequestParams::new("render_project").with_arguments(argumentos))
        .await
        .expect("render_project deveria responder");
    let estruturado = resultado
        .structured_content
        .expect("render_project deveria trazer structured_content");

    assert_eq!(
        estruturado["ok"],
        serde_json::Value::Bool(true),
        "resposta de erro inesperada: {estruturado}"
    );
    assert!(saida.exists(), "o arquivo de saída deveria existir");
    assert!(
        estruturado["tamanho_bytes"].as_u64().unwrap_or(0) > 1000,
        "arquivo de saída relatado como pequeno demais: {estruturado}"
    );

    let _ = std::fs::remove_file(&saida);
    client.cancel().await.expect("fechamento limpo do cliente");
}

#[tokio::test]
async fn render_project_sem_overwrite_retorna_erro_na_mesma_estrutura() {
    let client = iniciar_cliente().await;
    let saida = caminho_temporario("render_ja_existe.mp4");
    std::fs::write(&saida, b"conteudo qualquer, so para existir")
        .expect("deveria conseguir criar o arquivo de saida fake");

    let argumentos = serde_json::json!({
        "project_path": caminho_fixture().to_string_lossy(),
        "output_path": saida.to_string_lossy(),
        "preset": "h264_hq",
        "overwrite": false,
    })
    .as_object()
    .cloned()
    .expect("deveria montar os argumentos da chamada");

    let resultado = client
        .call_tool(CallToolRequestParams::new("render_project").with_arguments(argumentos))
        .await
        .expect("render_project deveria responder mesmo quando a saída já existe");
    let estruturado = resultado
        .structured_content
        .expect("render_project deveria trazer structured_content mesmo em erro");

    assert_eq!(
        estruturado["ok"],
        serde_json::Value::Bool(false),
        "deveria ter recusado sobrescrever: {estruturado}"
    );
    assert!(
        estruturado["erro"].as_str().is_some_and(|e| !e.is_empty()),
        "deveria trazer uma mensagem de erro em português: {estruturado}"
    );

    let _ = std::fs::remove_file(&saida);
    client.cancel().await.expect("fechamento limpo do cliente");
}

/// Isolamento (§11.4 do briefing): um projeto XML inválido falha no
/// processo filho (o libmlt não consegue criar o produtor "xml" a partir de
/// um arquivo que não é MLT), mas o servidor MCP continua respondendo
/// normalmente a uma chamada seguinte na mesma conexão.
#[tokio::test]
async fn projeto_xml_invalido_falha_isolado_e_servidor_continua_respondendo() {
    let client = iniciar_cliente().await;

    let projeto_invalido = caminho_temporario("projeto_invalido.xml");
    std::fs::write(&projeto_invalido, b"isto nao e um xml do mlt")
        .expect("deveria conseguir criar o projeto inválido");
    let saida = caminho_temporario("nao_deveria_existir.mp4");
    let _ = std::fs::remove_file(&saida);

    let argumentos = serde_json::json!({
        "project_path": projeto_invalido.to_string_lossy(),
        "output_path": saida.to_string_lossy(),
        "preset": "h264_hq",
        "overwrite": true,
    })
    .as_object()
    .cloned()
    .expect("deveria montar os argumentos da chamada");

    let resultado = client
        .call_tool(CallToolRequestParams::new("render_project").with_arguments(argumentos))
        .await
        .expect("render_project deveria responder mesmo com um projeto inválido");
    let estruturado = resultado
        .structured_content
        .expect("render_project deveria trazer structured_content mesmo em erro");
    assert_eq!(
        estruturado["ok"],
        serde_json::Value::Bool(false),
        "um projeto XML inválido deveria falhar: {estruturado}"
    );
    assert!(!saida.exists(), "não deveria ter gerado arquivo de saída");

    // O processo filho que falhou não derrubou o servidor: uma chamada
    // seguinte, na mesma conexão, ainda funciona.
    let info = client
        .call_tool(CallToolRequestParams::new("server_info"))
        .await
        .expect("server_info deveria continuar funcionando depois do erro isolado");
    let info_estruturado = info
        .structured_content
        .expect("server_info deveria trazer structured_content");
    assert_eq!(info_estruturado["ok"], serde_json::Value::Bool(true));

    let _ = std::fs::remove_file(&projeto_invalido);
    client.cancel().await.expect("fechamento limpo do cliente");
}
