//! Implementação de cada ferramenta MCP, como métodos de
//! `impl KdenliveMcpServer` (declarados em `crate::mcp::server`). Cada
//! arquivo aqui cobre uma categoria; as assinaturas `#[tool]` ficam em
//! `server.rs`, que só delega para estes métodos.

mod analise;
mod bin;
mod clip;
mod export;
mod filter;
mod guide;
mod keyframe;
mod marker;
mod project;
mod render_queue;
mod session;
mod subtitle;
mod timeline;
mod title;
mod track;
mod transition;
