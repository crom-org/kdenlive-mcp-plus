//! Testes do motor de render contra o fixture sintético
//! `tests/fixtures/smoke.xml` (um `tractor` com uma trilha de vídeo `color`
//! e uma trilha de áudio `tone`, sem depender de nenhuma mídia externa).

use std::path::PathBuf;

use kdenlive_mcp_plus::engine::{EngineError, preset, render};

fn caminho_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/smoke.xml")
}

fn caminho_temporario(nome: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "kdenlive_mcp_plus_render_{}_{nome}",
        std::process::id()
    ))
}

fn ffprobe_disponivel() -> bool {
    std::process::Command::new("ffprobe")
        .arg("-version")
        .output()
        .is_ok()
}

#[test]
fn renderiza_o_fixture_para_mp4_com_tamanho_acima_de_mil_bytes() {
    let saida = caminho_temporario("smoke_tamanho.mp4");
    let _ = std::fs::remove_file(&saida);

    let opts = preset::resolve_preset("h264_hq", true).expect("preset h264_hq deveria existir");
    render::render(&caminho_fixture(), &saida, &opts).expect("render do fixture deveria funcionar");

    let tamanho = std::fs::metadata(&saida)
        .expect("arquivo de saída deveria existir")
        .len();
    assert!(tamanho > 1000, "esperava mais de 1000 bytes, veio {tamanho}");

    let _ = std::fs::remove_file(&saida);
}

#[test]
fn renderiza_h264_720p_com_audio_aac_quando_ffprobe_existe() {
    if !ffprobe_disponivel() {
        eprintln!("ffprobe não encontrado no PATH; pulando a verificação de codecs/resolução");
        return;
    }

    let saida = caminho_temporario("smoke_codecs.mp4");
    let _ = std::fs::remove_file(&saida);

    let opts = preset::resolve_preset("h264_hq", true).expect("preset h264_hq deveria existir");
    render::render(&caminho_fixture(), &saida, &opts).expect("render do fixture deveria funcionar");

    let saida_ffprobe = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name,width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(&saida)
        .output()
        .expect("ffprobe deveria executar");
    let texto = String::from_utf8_lossy(&saida_ffprobe.stdout);

    assert!(texto.contains("h264"), "esperava codec h264: {texto}");
    assert!(texto.contains("1280"), "esperava largura 1280: {texto}");
    assert!(texto.contains("720"), "esperava altura 720: {texto}");
    assert!(texto.contains("aac"), "esperava codec de áudio aac: {texto}");

    let _ = std::fs::remove_file(&saida);
}

#[test]
fn projeto_inexistente_gera_erro_especifico() {
    let projeto = PathBuf::from("/caminho/que/certamente/nao/existe/projeto.xml");
    let saida = caminho_temporario("nao_deveria_existir.mp4");

    let opts = preset::resolve_preset("h264_hq", true).expect("preset h264_hq deveria existir");
    let erro = render::render(&projeto, &saida, &opts).expect_err("deveria falhar");

    assert!(
        matches!(erro, EngineError::ProjectNotFound(_)),
        "erro inesperado: {erro:?}"
    );
}

#[test]
fn saida_existente_sem_overwrite_gera_erro_especifico() {
    let saida = caminho_temporario("ja_existe.mp4");
    std::fs::write(&saida, b"conteudo qualquer, so para existir")
        .expect("deveria conseguir criar o arquivo fake");

    let opts = preset::resolve_preset("h264_hq", false).expect("preset h264_hq deveria existir");
    let erro = render::render(&caminho_fixture(), &saida, &opts)
        .expect_err("deveria falhar por a saída já existir");

    assert!(
        matches!(erro, EngineError::OutputExists(_)),
        "erro inesperado: {erro:?}"
    );

    let _ = std::fs::remove_file(&saida);
}
