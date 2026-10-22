//! Implementação de cada ferramenta MCP, como métodos de
//! `impl KdenliveMcpServer` (declarados em `crate::mcp::server`). Cada
//! arquivo aqui cobre uma categoria; as assinaturas `#[tool]` ficam em
//! `server.rs`, que só delega para estes métodos.

mod bin;
mod export;
mod project;
mod timeline;
