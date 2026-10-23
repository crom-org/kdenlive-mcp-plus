//! Execução do render via libmlt, seguindo a sequência verificada:
//! `mlt_factory_init` → `mlt_profile_init` → produtor `xml` → consumidor
//! `avformat` → conectar → iniciar → esperar → fechar (nessa ordem, em todos
//! os caminhos, sucesso ou erro).
//!
//! O libmlt tem estado global; `RENDER_LOCK` serializa chamadas a esta função
//! dentro do mesmo processo (importante nos testes, que podem chamar
//! `render` mais de uma vez no mesmo binário de teste).

use std::ffi::{CString, c_void};
use std::path::Path;
use std::sync::{Mutex, Once};

use super::EngineError;
use super::ffi;
use super::preset::RenderOptions;

static FACTORY_INIT: Once = Once::new();
static RENDER_LOCK: Mutex<()> = Mutex::new(());

fn ensure_factory_initialized() {
    FACTORY_INIT.call_once(|| {
        // SAFETY: mlt_factory_init só pode ser chamado uma vez por processo;
        // o `Once` garante isso mesmo com chamadas concorrentes a `render`.
        // NULL faz o libmlt usar o diretório padrão de módulos.
        unsafe {
            ffi::mlt_factory_init(std::ptr::null());
        }
    });
}

fn path_to_cstring(path: &Path, contexto: &str) -> Result<CString, EngineError> {
    let texto = path.to_str().ok_or_else(|| {
        EngineError::InvalidText(format!(
            "{contexto}: o caminho não é UTF-8 válido: {}",
            path.display()
        ))
    })?;
    CString::new(texto).map_err(|_| {
        EngineError::InvalidText(format!(
            "{contexto}: o caminho contém um byte nulo: {}",
            path.display()
        ))
    })
}

/// Renderiza `project` (XML do MLT/Kdenlive) para `output`, usando as opções
/// já resolvidas de um preset. Não sobrescreve `output` existente a menos que
/// `opts.overwrite` seja verdadeiro.
pub fn render(project: &Path, output: &Path, opts: &RenderOptions) -> Result<(), EngineError> {
    render_range(project, output, opts, None)
}

/// Como [`render`], mas só o trecho `intervalo` = `(primeiro, último)` quadros
/// (inclusive, 0-based), se dado. A posição dos quadros do trecho é a do
/// projeto inteiro (as animações continuam certas), como num corte de
/// `mlt_producer_cut`. Serve para conferir só uma parte sem renderizar tudo.
pub fn render_range(
    project: &Path,
    output: &Path,
    opts: &RenderOptions,
    intervalo: Option<(i64, i64)>,
) -> Result<(), EngineError> {
    if let Some((inicio, fim)) = intervalo
        && (inicio < 0 || fim < inicio)
    {
        return Err(EngineError::InvalidText(format!(
            "intervalo inválido: de {inicio} a {fim} (o início não pode ser negativo nem maior que o fim)"
        )));
    }
    if !project.exists() {
        return Err(EngineError::ProjectNotFound(project.to_path_buf()));
    }
    if output.exists() && !opts.overwrite {
        return Err(EngineError::OutputExists(output.to_path_buf()));
    }

    let project_c = path_to_cstring(project, "projeto")?;
    let output_c = path_to_cstring(output, "saída")?;

    let _guard = RENDER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    ensure_factory_initialized();

    // SAFETY: NULL seleciona o perfil de vídeo padrão do libmlt. O ponteiro
    // retornado é verificado contra NULL a seguir e fechado em todos os
    // caminhos (sucesso ou erro) antes de retornar desta função.
    let profile = unsafe { ffi::mlt_profile_init(std::ptr::null()) };
    if profile.is_null() {
        return Err(EngineError::Create("perfil de vídeo (profile)"));
    }

    let resultado = render_com_perfil(profile, &project_c, &output_c, opts, intervalo);

    // SAFETY: `profile` não é nulo (checado acima) e não é mais usado depois
    // deste ponto, em nenhum dos caminhos de `render_com_perfil`.
    unsafe {
        ffi::mlt_profile_close(profile);
    }

    resultado?;

    let metadata = std::fs::metadata(output).map_err(|_| EngineError::OutputMissing(output.to_path_buf()))?;
    if metadata.len() == 0 {
        return Err(EngineError::OutputMissing(output.to_path_buf()));
    }

    Ok(())
}

/// Renderiza um único quadro de `project` como um PNG em `output` (usada
/// pela ferramenta `preview_frame`). `frame` é 0-based, na taxa de quadros
/// do perfil do projeto.
pub fn render_frame(project: &Path, output: &Path, frame: i64, overwrite: bool) -> Result<(), EngineError> {
    if !project.exists() {
        return Err(EngineError::ProjectNotFound(project.to_path_buf()));
    }
    if output.exists() && !overwrite {
        return Err(EngineError::OutputExists(output.to_path_buf()));
    }
    if frame < 0 {
        return Err(EngineError::InvalidText(format!(
            "o número do quadro não pode ser negativo: {frame}"
        )));
    }

    let project_c = path_to_cstring(project, "projeto")?;
    let output_c = path_to_cstring(output, "saída")?;

    let opts = crate::engine::preset::RenderOptions {
        vcodec: Some("png"),
        acodec: None,
        vbitrate: None,
        abitrate: None,
        extension: "png",
        disable_audio: true,
        disable_video: false,
        overwrite,
    };

    let _guard = RENDER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    ensure_factory_initialized();

    // SAFETY: ver a função `render`, acima — mesma garantia.
    let profile = unsafe { ffi::mlt_profile_init(std::ptr::null()) };
    if profile.is_null() {
        return Err(EngineError::Create("perfil de vídeo (profile)"));
    }

    let resultado = render_com_perfil(profile, &project_c, &output_c, &opts, Some((frame, frame)));

    // SAFETY: ver a função `render`, acima — mesma garantia.
    unsafe {
        ffi::mlt_profile_close(profile);
    }

    resultado?;

    let metadata = std::fs::metadata(output).map_err(|_| EngineError::OutputMissing(output.to_path_buf()))?;
    if metadata.len() == 0 {
        return Err(EngineError::OutputMissing(output.to_path_buf()));
    }

    Ok(())
}

fn render_com_perfil(
    profile: ffi::mlt_profile,
    project_c: &CString,
    output_c: &CString,
    opts: &RenderOptions,
    intervalo: Option<(i64, i64)>,
) -> Result<(), EngineError> {
    // SAFETY: `profile` é válido (verificado pelo chamador). "xml" é o nome
    // do serviço de produtor registrado pelo libmlt para ler projetos MLT.
    // `project_c` vive durante toda a chamada.
    let producer = unsafe {
        ffi::mlt_factory_producer(profile, c"xml".as_ptr(), project_c.as_ptr() as *const c_void)
    };
    if producer.is_null() {
        return Err(EngineError::Create(
            "produtor (producer) a partir do XML do projeto",
        ));
    }

    // Um quadro só (ou um trecho), **sem** `mlt_producer_set_in_and_out`:
    // reduzir o intervalo do produtor faz o libmlt contar a posição a partir do
    // novo início, então um filtro animado (keyframes) era avaliado sempre no
    // quadro 0 — o `preview_frame` de um clipe com animação saía no estado do
    // primeiro keyframe (preto, se ele começasse em opacidade 0). Um *corte*
    // (`mlt_producer_cut`) mantém a posição absoluta do produtor original,
    // como as listas de reprodução fazem com os clipes.
    let mut corte: ffi::mlt_producer = std::ptr::null_mut();
    if let Some((inicio, fim)) = intervalo {
        // SAFETY: `producer` não é nulo (checado acima); `inicio` e `fim` já
        // foram validados (não negativos, `inicio <= fim`) pelos chamadores.
        // `mlt_position` é `int32_t` nesta build do libmlt (ver mlt_types.h);
        // o `as i32` é uma conversão estreita, não uma reinterpretação de bits.
        corte = unsafe { ffi::mlt_producer_cut(producer, inicio as i32, fim as i32) };
        if corte.is_null() {
            // SAFETY: `producer` não é nulo neste ponto.
            unsafe { ffi::mlt_producer_close(producer) };
            return Err(EngineError::InvalidText(format!(
                "não consegui selecionar os quadros {inicio} a {fim} (fora do intervalo do projeto?)"
            )));
        }
    }
    let fonte = if corte.is_null() { producer } else { corte };

    // SAFETY: `profile` é válido. "avformat" é o nome do serviço de
    // consumidor do libmlt que usa o FFmpeg para gravar o arquivo de saída.
    // `output_c` vive durante toda a chamada.
    let consumer = unsafe {
        ffi::mlt_factory_consumer(profile, c"avformat".as_ptr(), output_c.as_ptr() as *const c_void)
    };
    if consumer.is_null() {
        // SAFETY: `producer` não é nulo neste ponto.
        unsafe { ffi::mlt_producer_close(producer) };
        return Err(EngineError::Create("consumidor (consumer) avformat"));
    }

    if let Err(erro) = configurar_consumer(consumer, opts) {
        fechar_tudo(consumer, producer, corte);
        return Err(erro);
    }
    // SAFETY: `producer` e `consumer` são válidos (verificados acima).
    let service = unsafe { ffi::mlt_producer_service(fonte) };
    // SAFETY: `consumer` e `service` são válidos.
    let conectado = unsafe { ffi::mlt_consumer_connect(consumer, service) };
    if conectado != 0 {
        fechar_tudo(consumer, producer, corte);
        return Err(EngineError::Connect);
    }

    // SAFETY: `consumer` é válido.
    let iniciado = unsafe { ffi::mlt_consumer_start(consumer) };
    if iniciado != 0 {
        fechar_tudo(consumer, producer, corte);
        return Err(EngineError::Start);
    }

    loop {
        // SAFETY: `consumer` é válido durante todo o laço de espera.
        let parado = unsafe { ffi::mlt_consumer_is_stopped(consumer) };
        if parado != 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    fechar_tudo(consumer, producer, corte);
    Ok(())
}

fn fechar_tudo(consumer: ffi::mlt_consumer, producer: ffi::mlt_producer, corte: ffi::mlt_producer) {
    // SAFETY: ambos os ponteiros vêm de chamadas bem-sucedidas a
    // mlt_factory_consumer/mlt_factory_producer e não são usados depois
    // deste close, em nenhum caminho de chamada desta função.
    unsafe {
        ffi::mlt_consumer_close(consumer);
        if !corte.is_null() {
            ffi::mlt_producer_close(corte);
        }
        ffi::mlt_producer_close(producer);
    }
}

fn configurar_consumer(consumer: ffi::mlt_consumer, opts: &RenderOptions) -> Result<(), EngineError> {
    // SAFETY: `consumer` não é nulo (verificado pelo chamador). As
    // propriedades retornadas são de propriedade do consumer e só são
    // lidas/escritas enquanto ele está vivo.
    let properties = unsafe { ffi::mlt_consumer_properties(consumer) };
    if properties.is_null() {
        return Err(EngineError::Create("propriedades do consumidor"));
    }

    definir_propriedade(properties, "real_time", "-1")?;
    if let Some(vcodec) = opts.vcodec {
        definir_propriedade(properties, "vcodec", vcodec)?;
    }
    if let Some(acodec) = opts.acodec {
        definir_propriedade(properties, "acodec", acodec)?;
    }
    if let Some(vbitrate) = opts.vbitrate {
        definir_propriedade(properties, "vb", vbitrate)?;
    }
    if let Some(abitrate) = opts.abitrate {
        definir_propriedade(properties, "ab", abitrate)?;
    }
    if opts.disable_audio {
        definir_propriedade(properties, "an", "1")?;
    }
    if opts.disable_video {
        definir_propriedade(properties, "vn", "1")?;
    }

    Ok(())
}

fn definir_propriedade(
    properties: ffi::mlt_properties,
    nome: &str,
    valor: &str,
) -> Result<(), EngineError> {
    let nome_c = CString::new(nome)
        .map_err(|_| EngineError::InvalidText(format!("nome de propriedade inválido: {nome}")))?;
    let valor_c = CString::new(valor)
        .map_err(|_| EngineError::InvalidText(format!("valor de propriedade inválido: {valor}")))?;
    // SAFETY: `properties` não é nulo (verificado pelo chamador); `nome_c` e
    // `valor_c` vivem durante toda a chamada.
    let rc = unsafe { ffi::mlt_properties_set(properties, nome_c.as_ptr(), valor_c.as_ptr()) };
    if rc != 0 {
        return Err(EngineError::Create("propriedade do consumidor avformat"));
    }
    Ok(())
}
