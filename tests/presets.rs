//! Teste de regressão do bug do projeto original (ver docs/09-riscos-e-bugs.md,
//! Apêndice C): o nome do preset (ex. "h264_hq") nunca deve ser passado como
//! valor de uma opção do avformat — isso derrubava o `melt` com segmentation
//! fault porque o libx264 rejeita "h264_hq" como nome de codec.

use kdenlive_mcp_plus::engine::preset;

#[test]
fn todo_preset_resolve_para_codec_bitrate_e_extensao_explicitos() {
    for nome in preset::preset_names() {
        let opts = preset::resolve_preset(nome, false)
            .unwrap_or_else(|erro| panic!("preset \"{nome}\" deveria resolver, mas falhou: {erro}"));

        assert!(!opts.extension.is_empty(), "preset \"{nome}\" sem extensão");
        assert!(
            opts.vcodec.is_some() || opts.disable_video,
            "preset \"{nome}\" não tem vcodec nem desliga o vídeo"
        );
        assert!(
            opts.acodec.is_some() || opts.disable_audio,
            "preset \"{nome}\" não tem acodec nem desliga o áudio"
        );
    }
}

#[test]
fn nenhum_argumento_enviado_ao_consumidor_contem_o_nome_do_preset() {
    for nome in preset::preset_names() {
        let opts = preset::resolve_preset(nome, false).expect("preset válido");
        for (campo, valor) in [
            ("vcodec", opts.vcodec),
            ("acodec", opts.acodec),
            ("vbitrate", opts.vbitrate),
            ("abitrate", opts.abitrate),
        ] {
            let Some(valor) = valor else { continue };
            if nome == "gif" && campo == "vcodec" && valor == "gif" {
                // Coincidência legítima do Apêndice A: "gif" é ao mesmo tempo
                // o nome do preset e o nome real do codec do ffmpeg
                // (confirmado com `ffmpeg -encoders | grep gif`). Não é o bug
                // do original, que passava um identificador de preset
                // composto (ex. "h264_hq") como se fosse nome de codec.
                continue;
            }
            assert_ne!(
                valor, nome,
                "preset \"{nome}\" vazou o próprio nome para a opção \"{campo}\" do avformat"
            );
        }
    }
}

#[test]
fn preset_desconhecido_gera_erro_listando_os_validos() {
    let erro = preset::resolve_preset("preset_que_nao_existe", false).expect_err("deveria falhar");
    let mensagem = erro.to_string();

    for nome in preset::preset_names() {
        assert!(
            mensagem.contains(nome),
            "mensagem de erro deveria listar \"{nome}\": {mensagem}"
        );
    }
}
