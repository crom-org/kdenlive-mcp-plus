//! Servidor MCP em transporte stdio.

pub mod server;

pub use server::KdenliveMcpServer;

/// Serve o servidor MCP por stdio até o cliente desconectar.
///
/// O stdout fica reservado para mensagens do protocolo MCP (é o transporte);
/// qualquer log deste processo deve ir para stderr (ver `src/main.rs`).
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    use rmcp::ServiceExt;

    let server = KdenliveMcpServer::new();
    let running = server.serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
