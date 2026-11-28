mod anticheat;
mod catalogo_lanzadores;
mod benchmarks;
mod covers;
mod cpu_rank;
mod diagnostico_procesos;
mod engine;
mod evaluacion;
mod evidencia_proceso;
#[cfg(test)]
mod fixtures_procesos;
mod gpu_rank;
mod hw;
mod launchers;
mod mangohud;
mod launch_command;
mod log;
mod log_watcher;
mod log_watcher_state;
mod net;
mod preflight;
mod proceso_linux;
mod profiles;
mod proton_pulse;
mod requirements;
mod rules;
mod sessions;
mod sessions_service;
mod shaders;
mod stats;
mod steam;
mod steam_launch;
#[cfg(test)]
mod test_entorno;

use serde::Serialize;
use tauri::async_runtime::spawn_blocking;

/// Carátula local (data URI base64) que el launcher guarda en disco, o null si
/// no tiene una. Va por comando (y no por URL de archivo) para no tener que
/// habilitar el protocolo `asset` ni tocar la CSP.
#[tauri::command]
async fn get_local_cover(
    launcher: String,
    appid: Option<u32>,
    key: Option<String>,
    runner: Option<String>,
) -> Option<String> {
    spawn_blocking(move || {
        covers::local_cover_with_runner(
            &launcher,
            appid,
            key.as_deref().unwrap_or(""),
            runner.as_deref(),
        )
    })
    .await
    .ok()
    .flatten()
}

/// Diagnóstico de las carátulas locales: qué carpetas existen y qué archivos
/// hay dentro. Se usa desde el botón de la UI para depurar sin adivinar.
#[tauri::command]
async fn diagnose_covers() -> String {
    spawn_blocking(covers::diagnose).await.unwrap_or_default()
}

/// Traza temporal de diagnóstico del frontend (C.3).
///
#[derive(Serialize)]
struct HwInfo {
    #[serde(flatten)]
    hw: hw::Hardware,
    tags: Vec<String>,
}

#[tauri::command]
async fn get_hardware() -> Result<HwInfo, String> {
    spawn_blocking(|| {
        let hw = hw::detect();
        let tags = hw.tags();
        HwInfo { hw, tags }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_rules(appid: u32) -> rules::Game {
    rules::for_appid(appid)
}

/// Requisitos del juego comparados con esta máquina.
///
/// `install_path` es opcional: si se conoce la carpeta del juego se comprueba el
/// espacio libre en esa partición (la que importa), no en `/`.
#[tauri::command]
async fn get_requirements(
    appid: u32,
    install_path: Option<String>,
) -> Result<requirements::RequirementsReport, String> {
    spawn_blocking(move || {
        let hw = hw::detect();
        let raw = steam::pc_requirements(appid);
        let machine = requirements::Machine {
            ram_gb: hw.ram_gb,
            vram_mb: hw.vram_mb,
            cpu_threads: hw.cpu_threads,
            cpu_cores: hw.cpu_cores,
            cpu_name: hw.cpu.clone(),
            gpu_name: hw.gpu.clone(),
            free_disk_gb: requirements::free_disk_gb(install_path.as_deref()),
        };
        // La API del motor decide el margen (VKD3D 20 %, DXVK 10 %, Vulkan 0).
        // Se detecta por firma de disco en la ruta real de instalacion; si no se
        // pasa ruta o no encaja ninguna firma, `api` queda `None` y
        // `proton_margin_percent` aplica el 10 % por defecto de forma explicita.
        // Ya no se consulta ninguna tabla `appid -> motor`: la que habia daba a
        // Elden Ring "Unreal Engine/dxvk" cuando es FromSoftware/VKD3D.
        let api = install_path
            .as_deref()
            .and_then(engine::detect_from_install_path)
            .map(|e| e.api);
        requirements::evaluate(&raw, &machine, api.as_deref())
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn scan_library() -> Result<Vec<steam::LibGame>, String> {
    spawn_blocking(steam::scan_library)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_store(term: String) -> Result<Vec<steam::LibGame>, String> {
    spawn_blocking(move || steam::search_store(&term))
        .await
        .map_err(|e| e.to_string())?
}

/// Opciones de lanzamiento que Steam tiene configuradas para un juego.
///
/// Solo lectura: no escribe nada en Steam (ver `steam_launch`). `None` = no se puede
/// saber, que es distinto de «Steam las tiene vacías» (`opciones: Some("")`).
#[tauri::command]
async fn steam_launch_options(appid: u32) -> Result<Option<steam_launch::OpcionesSteam>, String> {
    // Leer varios `localconfig.vdf` puede ser lento (son grandes): fuera del hilo de UI.
    tauri::async_runtime::spawn_blocking(move || Ok(steam_launch::leer_opciones(appid)))
        .await
        .map_err(|e| e.to_string())?
}

/// El catálogo del juego **más** las recomendaciones que el usuario puede activar.
///
/// ## Por qué existe
///
/// Las recomendaciones de motor y de hardware se ofrecen como botones que debían escribir
/// su argumento en el comando. Lo hacían por fuera del motor (concatenando en React), así
/// que el comando de arriba no las recogía: era el fallo de «selecciono una recomendación y
/// no pasa nada».
///
/// Ahora la recomendación es una definición más del catálogo, con índice propio a
/// continuación de `rules.params`. El motor la reconoce, la coloca en su capa y la
/// serializa igual que un ajuste nativo, sin que el frontend escriba ni un carácter.
///
/// `extras` viene del frontend en el orden en que las enseña. El índice es
/// `rules.params.len() + posición`, así que el frontend sabe a cuál se refiere sin que haya
/// dos listas diferentes.
pub fn catalogo_efectivo(appid: u32, extras: &[String]) -> Vec<launch_command::Definicion> {
    let mut catalogo: Vec<launch_command::Definicion> = rules::for_appid(appid)
        .params
        .iter()
        .enumerate()
        .map(|(indice, p)| launch_command::Definicion {
            indice,
            valor: p.value.clone(),
            kind: p.kind.clone(),
            proton_only: p.proton_only,
        })
        .collect();

    for valor in extras {
        let valor = valor.trim();
        // Una recomendación vacía o repetida apuntaría a dos definiciones para el mismo
        // texto: el motor la marcaría ambigua y activar una apagaría la otra.
        let es_valida = !valor.is_empty()
            && valor != "%command%"
            && !catalogo.iter().any(|d| d.valor == valor);
        if !es_valida {
            continue;
        }
        // El `kind` no es decorativo: `rol_de_definicion` solo reconoce `env` y `wrapper`, y
        // con otro valor el motor no ve la recomendación y no la añade al comando. Se deduce
        // del texto igual que hace el catálogo de `games.json`:
        //   - `FOO=bar` es una variable de entorno;
        //   - una sola palabra sin espacios es un envoltorio o un argumento suelto.
        // `FOO=bar` lo clasifica el lexer como `env` por su forma (`clave_env`), antes que
        // por el catálogo, así que el `kind` tiene que coincidir con esa clasificación o el
        // rol no cuadra y el motor no lo reconoce.
        let kind = if valor.contains('=') { "env" } else { "arg" };
        catalogo.push(launch_command::Definicion {
            indice: catalogo.len(),
            valor: valor.to_string(),
            kind: kind.to_string(),
            // La aplicabilidad sale del mismo predicado que usa el catálogo de reglas:
            // una recomendación `WINE_*` es tan solo-Proton como un `Param` con ese texto.
            proton_only: rules::is_proton_only(valor),
        });
    }

    catalogo
}

/// Catálogo efectivo con el índice real de cada valor, tal como lo ve el motor.
///
/// Existe porque calcular ese índice en la interfaz era imposible de hacer bien: el
/// catálogo salta las recomendaciones vacías, `%command%` y las que **ya están** en
/// `rules.params` (el perfil genérico trae `PROTON_LOCAL_SHADER_CACHE=1`, y `mangohud`
/// también suele estar). Un `params.length + indexOf` en React no ve esos saltos, así que a
/// partir de la primera recomendación descartada todos los índices se desplazaban y el
/// toggle escribía en otro ajuste: eso era el `WINE_FULLSCREEN_FSR=1` que no reaccionaba.
#[tauri::command]
fn catalogo_de_ajustes(appid: u32, extras: Option<Vec<String>>) -> Vec<IndiceAjuste> {
    catalogo_efectivo(appid, extras.as_deref().unwrap_or(&[]))
        .into_iter()
        .map(|d| IndiceAjuste {
            indice: d.indice,
            valor: d.valor,
        })
        .collect()
}

/// Nombre y posición de un ajuste dentro del catálogo efectivo.
#[derive(Clone, serde::Serialize)]
pub struct IndiceAjuste {
    /// Índice que espera el motor en `launch_command_changes`. Es el único válido.
    indice: usize,
    /// Texto del ajuste (`WINE_FULLSCREEN_FSR=1`, `mangohud`).
    valor: String,
}

/// Analiza un comando y devuelve su modelo estructurado.
///
/// Sin estado: el análisis que devuelve se reenvía tal cual a `serialize_launch_command`.
/// La app no guarda nada entre llamadas (ver la nota de diseño en `launch_command`).
#[tauri::command]
fn analyze_launch_command(
    command: String,
    appid: u32,
    // Textos de recomendación extra (motor y hardware), en el orden en que se enseñan.
    extras: Option<Vec<String>>,
) -> Result<launch_command::Analisis, String> {
    // Tope duro antes de tokenizar: el texto viene de Steam o de un perfil importado, pero
    // se comprueba igual (una línea de más no se puede colar por una ruta que no valide).
    if command.len() > 4 * launch_command::MAX_LONGITUD * 4 {
        return Err("El comando es demasiado largo para analizarlo.".to_string());
    }
    Ok(launch_command::analizar(
        &command,
        &catalogo_efectivo(appid, extras.as_deref().unwrap_or(&[])),
    ))
}

/// Aplica cambios de ajustes sobre un comando ya analizado.
///
/// `analisis` es lo que devolvió `analyze_launch_command`, sin modificar. Los cambios se
/// aplican sobre el texto original, no sobre una reconstrucción.
///
/// Devuelve el comando **y** si se pudo aplicar. No es un `Result` de Tauri a propósito:
/// un fallo del motor no es una excepción de la app, sino un estado que la UI tiene que
/// poder enseñar. Si los datos del IPC no cuadran, se devuelve el original intacto con
/// `aplicado: false` y el motivo; así la UI puede volver a analizar en vez de quedarse con
/// un estado que no corresponde a ningún comando.
#[derive(Serialize)]
struct ResultadoSerializacion {
    /// El comando resultante. Es el original si `aplicado` es `false`.
    comando: String,
    /// `true` si los cambios se aplicaron de verdad.
    aplicado: bool,
    /// Motivo del fallo, cuando `aplicado` es `false`.
    error: Option<launch_command::ErrorAnalisis>,
}

#[tauri::command]
fn serialize_launch_command(
    analisis: launch_command::Analisis,
    appid: u32,
    cambios: Vec<launch_command::Cambio>,
    // Las mismas recomendaciones que se pasaron al analizar. Sin ellas el catálogo no
    // tendría la misma longitud y los índices del análisis apuntarían a otro sitio.
    extras: Option<Vec<String>>,
) -> ResultadoSerializacion {
    let catalogo = catalogo_efectivo(appid, extras.as_deref().unwrap_or(&[]));
    match launch_command::intentar_serializar(&analisis, &catalogo, &cambios) {
        Ok(comando) => ResultadoSerializacion {
            comando,
            aplicado: true,
            error: None,
        },
        Err(error) => ResultadoSerializacion {
            comando: analisis.original.clone(),
            aplicado: false,
            error: Some(error),
        },
    }
}

/// ¿Se puede usar este comando tal cual? Devuelve el motivo si no.
///
/// Es la única validación de comandos del proyecto. `profiles.rs` llamará a la misma
/// función, para que un perfil importado y el editor no digan cosas distintas.
#[tauri::command]
fn validate_launch_command(command: String) -> Result<(), launch_command::MotivoRechazo> {
    launch_command::validar(&command)
}

/// Estado de cada ajuste del catálogo respecto a un comando ya analizado.
///
/// Es lo que la UI usará para pintar los toggles sin deducir nada: activo, inactivo,
/// parcial (queda una aparición que Thoriapp no controla), ambiguo o en conflicto.
#[tauri::command]
fn launch_command_states(
    analisis: launch_command::Analisis,
    appid: u32,
    extras: Option<Vec<String>>,
) -> Vec<launch_command::EstadoAjuste> {
    launch_command::estado_de_ajustes(
        &analisis,
        &catalogo_efectivo(appid, extras.as_deref().unwrap_or(&[])),
    )
}

/// Traduce «quiero que estos ajustes estén puestos» a cambios concretos.
///
/// La UI no decide qué hay que quitar ni qué añadir: compara el conjunto pedido con lo que
/// el análisis ya reconoce y devuelve solo las diferencias. Vive en Rust porque los
/// ajustes ignorables (ambiguos, duplicados, argumentos del juego) los conoce el motor.
///
/// Recibe `appid` y `extras` para construir **el mismo catálogo efectivo** con el que se
/// analizó el comando: sin eso, un índice de recomendación apuntaba a otro ajuste y el
/// cambio se traducía al valor equivocado.
#[tauri::command]
fn launch_command_changes(
    analisis: launch_command::Analisis,
    appid: u32,
    activos: Vec<usize>,
    extras: Option<Vec<String>>,
) -> Result<Vec<launch_command::Cambio>, launch_command::ErrorAnalisis> {
    analisis.validar_coherencia()?;
    let catalogo = catalogo_efectivo(appid, extras.as_deref().unwrap_or(&[]));
    Ok(launch_command::cambios_para(&analisis, &catalogo, &activos))
}

/// Quita del comando un segmento conservado, si el motor considera seguro hacerlo.
///
/// El frontend **no** decide qué se puede borrar: manda el índice del segmento y aquí se
/// comprueba que siga siendo un argumento suelto sin gestión propia (un envoltorio, un
/// argumento del juego o un segmento ambiguo darían un comando distinto del que el usuario
/// cree). Devuelve el comando nuevo, o un error explicando por qué no se toca.
#[tauri::command]
fn launch_command_remove_segment(
    analisis: launch_command::Analisis,
    indice: usize,
) -> Result<ResultadoSerializacion, launch_command::ErrorAnalisis> {
    analisis.validar_coherencia()?;

    let segmento = analisis
        .segmentos
        .get(indice)
        .ok_or(launch_command::ErrorAnalisis::IndiceDesconocido { indice })?;

    // Solo lo que el motor clasificó como conservado-por-desconocido. Borrar un envoltorio
    // cambiaría el comportamiento del juego sin que se vea en pantalla, y borrar lo que
    // Thoriapp reconoce es trabajo de los ajustes, no de esta acción.
    let es_borrable = matches!(
        segmento.gestion,
        launch_command::Gestion::Conservado {
            razon: launch_command::Conservado::Desconocido
        }
    );
    if !es_borrable {
        return Ok(ResultadoSerializacion {
            comando: analisis.original.clone(),
            aplicado: false,
            error: None,
        });
    }

    let texto = launch_command::quitar_segmento(&analisis, indice)?;
    Ok(ResultadoSerializacion {
        comando: texto,
        aplicado: true,
        error: None,
    })
}

/// Cómo se escribiría un valor del catálogo dentro de un comando (con comillas si hace falta).
///
/// Devuelve error si el valor no se puede meter de forma segura; la UI debe enseñarlo en
/// vez de insertarlo a mano.
#[tauri::command]
fn escape_launch_value(valor: String) -> Result<String, String> {
    launch_command::escapar_valor(&valor).map_err(|e| e.to_string())
}

/// Envoltorios conocidos por el sistema, para que el frontend no mantenga su propia copia.
#[tauri::command]
fn known_wrappers() -> Vec<String> {
    preflight::KNOWN_WRAPPERS.iter().map(|w| w.to_string()).collect()
}

/// Opciones de lanzamiento de Steam para el juego de una grabación.
///
/// El AppID de un CSV de MangoHud llega a 0 (el fichero no lo trae), así que se resuelve
/// por nombre contra la biblioteca instalada. Sin esto, una prueba analizada sin la ficha
/// del juego abierta se quedaba sin saber con qué argumentos se había hecho.
#[tauri::command]
async fn steam_launch_options_para_juego(
    appid: Option<u32>,
    game_name: Option<String>,
) -> Result<Option<steam_launch::OpcionesSteam>, String> {
    // Recorre bibliotecas y `localconfig.vdf` (megabytes): fuera del hilo de UI.
    spawn_blocking(move || Ok(steam_launch::leer_opciones_juego(appid, game_name.as_deref())))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn game_info(appid: u32) -> Result<steam::GameInfo, String> {
    spawn_blocking(move || steam::game_info(appid))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn analyze_log(path: String) -> Result<log::Stats, String> {
    spawn_blocking(move || log::analyze(&path))
        .await
        .map_err(|e| e.to_string())?
}

/// Extrae el host de una URL https y comprueba que NO sea solo el prefijo.
///
/// Antes se usaba `starts_with("https://github.com/")`, que acepta
/// `https://github.com.evil.com/` porque el texto empieza igual. Aquí se separa
/// el host real (hasta el primer `/`, `?` o `#`) y se compara exacto.
fn host_matches(url: &str, allowed_host: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };

    // El host termina en el primer `/`, `?` o `#`. Sin ninguno, es el host entero.
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");

    // Se descartan credenciales (`user@host`) y puerto (`host:443`).
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);

    host.eq_ignore_ascii_case(allowed_host)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    const ALLOWED_HOSTS: [&str; 7] = [
        "www.protondb.com",
        "store.steampowered.com",
        "docs.kernel.org",
        "github.com",
        "gitlab.freedesktop.org",
        "wiki.archlinux.org",
        "steamspy.com",
    ];

    // Solo https, y solo hacia los dominios de la lista. Comprobar el host
    // completo evita que un dominio parecido se cuele por prefijo.
    if !ALLOWED_HOSTS.iter().any(|h| host_matches(&url, h)) {
        return Err(format!("URL no permitida: {url}"));
    }

    let mut child = std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("No pude abrir el navegador: {e}"))?;
    // xdg-open puede quedarse vivo mientras el navegador esté abierto: se espera
    // en un hilo aparte para no bloquear el hilo del comando.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[tauri::command]
fn detect_engine(appid: u32, game_name: String, install_path: Option<String>) -> Option<engine::GameEngine> {
    // Primero por firma de disco (fiable); si no hay ruta, se intenta con el
    // nombre, que solo acierta en casos muy obvios.
    install_path
        .as_deref()
        .and_then(engine::detect_from_install_path)
        .or_else(|| engine::detect_engine(appid, &game_name))
}

#[tauri::command]
fn get_engine_recommendations(engine: engine::GameEngine) -> Vec<String> {
    // Sin GPU el filtro de RT no puede decidir, así que `dxr` se queda fuera: es
    // una opción que puede costar FPS y sin dato no se sostiene.
    engine::get_engine_recommendations_for(&engine, None)
}

/// Igual que `get_engine_recommendations`, sabiendo qué GPU hay en el equipo.
///
/// Es la que usa la UI: el nombre se detecta una vez al arrancar (`get_hardware`) y
/// se pasa aquí para que decisiones que dependen del modelo —hoy, recomendar o no
/// `VKD3D_CONFIG=dxr`— se tomen con dato en vez de a ciegas.
#[tauri::command]
fn get_engine_recommendations_for_gpu(
    engine: engine::GameEngine,
    gpu_name: Option<String>,
) -> Vec<String> {
    engine::get_engine_recommendations_for(&engine, gpu_name.as_deref())
}

#[tauri::command]
fn get_community_recommendations(hw_tags: Vec<String>) -> Vec<engine::CommunityRecommendation> {
    let all_recommendations = engine::get_hardware_recommendations();
    // Único punto de salida hacia la interfaz: se marca aquí para que ninguna
    // recomendación llegue sin `proton_only`, venga del filtro que venga.
    engine::marcar_proton_only(engine::filter_recommendations_by_hardware(
        all_recommendations,
        &hw_tags,
    ))
}

#[tauri::command]
async fn scan_external_launchers() -> Result<Vec<launchers::ExternalGame>, String> {
    spawn_blocking(launchers::scan_all_launchers)
        .await
        .map_err(|e| e.to_string())
}

/// Actualiza la biblioteca **y** la caché que usa el observador.
///
/// La interfaz ya relee la lista por su cuenta; esto además invalida la caché del motor de
/// procesos, para que un juego recién instalado se reconozca sin esperar a la revalidación
/// periódica de cinco minutos.
#[tauri::command]
async fn refresh_library() -> Result<Vec<launchers::ExternalGame>, String> {
    let refrescado = spawn_blocking(|| {
        crate::catalogo_lanzadores::actualizar();
        launchers::scan_all_launchers()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(refrescado)
}

/// Juegos de Heronite descartados por estar desinstalados (`is_installed:
/// false`). La UI los menciona para que no parezca que faltan juegos.
#[tauri::command]
fn heronite_uninstalled_count() -> usize {
    launchers::heronite_desinstalados()
}

/// Compara dos grupos de grabaciones (la misma configuración repetida varias veces).
///
/// Una sola comparación entre dos grabaciones no distingue una mejora real de la
/// varianza del juego; con 3 o más por lado sí se puede dar un intervalo. Devuelve
/// una entrada por métrica para que la UI decida cuál enseñar.
#[tauri::command]
fn compare_benchmark_groups(
    antes_ids: Vec<String>,
    despues_ids: Vec<String>,
) -> Result<Vec<stats::GroupComparison>, String> {
    let data = benchmarks::load_benchmarks_data();
    let pick = |ids: &[String]| -> Result<Vec<benchmarks::Benchmark>, String> {
        ids.iter()
            .map(|id| {
                data.benchmarks
                    .iter()
                    .find(|b| &b.id == id)
                    .cloned()
                    .ok_or_else(|| format!("Grabación no encontrada: {id}"))
            })
            .collect()
    };

    let antes = pick(&antes_ids)?;
    let despues = pick(&despues_ids)?;
    if antes.is_empty() || despues.is_empty() {
        return Err("Elige al menos una grabación de cada lado.".to_string());
    }
    // Juegos distintos no se comparan: el número saldría, pero no significaría nada.
    if antes[0].game_name != despues[0].game_name {
        return Err(format!(
            "Son juegos distintos ({} y {}): no se pueden comparar.",
            antes[0].game_name, despues[0].game_name
        ));
    }

    let avg_a: Vec<f64> = antes.iter().map(|b| b.avg_fps).collect();
    let avg_d: Vec<f64> = despues.iter().map(|b| b.avg_fps).collect();
    let low1_a: Vec<f64> = antes.iter().map(|b| b.fps_1_percent_low).collect();
    let low1_d: Vec<f64> = despues.iter().map(|b| b.fps_1_percent_low).collect();
    let low01_a: Vec<f64> = antes.iter().map(|b| b.fps_0_1_percent_low).collect();
    let low01_d: Vec<f64> = despues.iter().map(|b| b.fps_0_1_percent_low).collect();

    Ok(vec![
        stats::compare_groups(&avg_a, &avg_d, stats::Metric::AvgFps),
        stats::compare_groups(&low1_a, &low1_d, stats::Metric::Fps1PercentLow),
        stats::compare_groups(&low01_a, &low01_d, stats::Metric::Fps01PercentLow),
    ])
}

/// ¿Se está ejecutando como root?
///
/// Thoriapp escribe en la configuración del usuario (`~/.config/MangoHud`,
/// `~/.config/thoriapp`). Lanzada con `sudo`, crearía archivos con dueño root que
/// luego el usuario no puede modificar, y rompería el escritorio del usuario. Se
/// lee `/proc/self/status` para no añadir la dependencia `libc` solo por esto.
/// Interpreta el campo `Uid:` de `/proc/self/status`. Separado de
/// [`running_as_root`] para poder probarlo sin depender de con qué usuario
/// corren los tests.
fn uid_is_root(status: &str) -> bool {
    status
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|uid| uid.parse::<u32>().ok())
        == Some(0)
}

fn running_as_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .map(|s| uid_is_root(&s))
        .unwrap_or(false)
}

/// Cerrojo de instancia única: `<config>/thoriapp/instancia.lock`.
///
/// El historial de pruebas y las comparaciones guardadas se protegen con un `Mutex` de
/// proceso, que solo sirve si hay **un** proceso. Dos ventanas de Thoriapp escribirían el
/// mismo `benchmarks.json` con cerrojos distintos y se pisarían igual que sin cerrojo, así
/// que esta protección no es un extra: es la condición que hace válido aquel diseño.
///
/// Se usa `flock` del sistema (una dependencia menos que `fs2`) y se mantiene el archivo
/// abierto durante toda la vida del proceso: el cerrojo se suelta solo al morir el proceso,
/// incluso si se cae, así que nunca queda un bloqueo fantasma.
struct CerrojoDeInstancia {
    /// Se guarda para que el descriptor siga abierto: cerrarlo soltaría el cerrojo.
    _archivo: std::fs::File,
}

fn tomar_cerrojo_de_instancia() -> Option<CerrojoDeInstancia> {
    let dir = dirs::config_dir()?.join("thoriapp");
    std::fs::create_dir_all(&dir).ok()?;
    let ruta = dir.join("instancia.lock");
    let archivo = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&ruta)
        .ok()?;

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        // LOCK_EX | LOCK_NB: si otra instancia lo tiene, falla en vez de esperar.
        let r = unsafe { libc_flock(archivo.as_raw_fd(), 2 | 4) };
        if r != 0 {
            return None;
        }
    }
    Some(CerrojoDeInstancia { _archivo: archivo })
}

// `flock(2)`. Se declara a mano para no añadir la caja `libc` por una sola llamada.
#[cfg(unix)]
extern "C" {
    #[link_name = "flock"]
    fn libc_flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int)
        -> std::os::raw::c_int;
}


// ==================== SESIONES DE EJECUCIÓN (subfase 5.3) ====================
//
// El observador de procesos corre en su propio hilo (ver `sessions_service`) y no depende de
// qué pantalla esté abierta ni de que la ventana esté visible. Estos comandos solo leen lo
// observado y permiten corregirlo a mano. Ninguno escribe en Steam, en el juego ni en nada
// del sistema: el observador es de solo lectura.

/// Qué está haciendo el observador, y si se puede afirmar algo sobre la ejecución.
#[derive(Serialize)]
struct EstadoObservador {
    activo: bool,
    estado: String,
    /// Juegos que el observador está viendo **ahora**, con su nombre comercial.
    ///
    /// Va en la respuesta y no se deduce en React: el backend es la única fuente de verdad
    /// de lo observado. Vacío en cualquier estado que no sea `observando`.
    juegos: Vec<sessions_service::JuegoObservado>,
    /// Solo `true` si hay evidencia real de que el juego no corre. En Docker es `false`.
    puede_afirmar_que_no_corre: bool,
    pasadas: u64,
    coste_ms: Option<f64>,
    error: Option<String>,
}

#[tauri::command]
async fn estado_observador() -> EstadoObservador {
    use sessions_service::{servicio, EstadoDelObservador};
    let s = servicio();
    let estado = s.estado_legible();
    let busca = match &estado {
        EstadoDelObservador::Apagado => "apagado",
        EstadoDelObservador::SinVisibilidad => "sin_visibilidad",
        EstadoDelObservador::Observando { .. } => "observando",
        EstadoDelObservador::Buscando => "buscando",
    };
    let juegos = match &estado {
        EstadoDelObservador::Observando { juegos } => juegos.clone(),
        _ => Vec::new(),
    };
    EstadoObservador {
        activo: s.esta_activo(),
        estado: busca.to_string(),
        juegos,
        // Solo la ausencia de proceso *observado* permite afirmar que no corre.
        puede_afirmar_que_no_corre: false,
        pasadas: s.estado().pasadas.load(std::sync::atomic::Ordering::Relaxed),
        coste_ms: s
            .estado()
            .ultima_medida
            .lock()
            .ok()
            .and_then(|m| m.as_ref().map(|m| m.coste_ms)),
        error: s.estado().error.lock().ok().and_then(|e| e.clone()),
    }
}

/// Activa o desactiva el observador. Desactivado no se hace ni una lectura de `/proc`.
#[tauri::command]
async fn configurar_observador(activo: bool) -> bool {
    sessions_service::servicio().fijar_activo(activo);
    activo
}

/// Lista de sesiones guardadas, de la más reciente a la más antigua.
#[tauri::command]
async fn listar_sesiones() -> Result<Vec<sessions::SesionDeEjecucion>, String> {
    sessions_service::servicio().cargar().map(|d| d.sesiones)
}

/// La sesión de un benchmark, si sigue existiendo. `null` es una respuesta válida: la
/// prueba no se atribuyó, o la sesión se borró. La instantánea del benchmark basta para
/// interpretarla en ambos casos.
#[tauri::command]
async fn sesion_de_benchmark(benchmark_id: String) -> Result<Option<sessions::SesionDeEjecucion>, String> {
    let s = sessions_service::servicio();
    let d = s.cargar()?;
    Ok(d.sesiones
        .into_iter()
        .find(|x| x.grabaciones.iter().any(|g| g == &benchmark_id)))
}

/// Asigna a mano el juego de una sesión que no se pudo identificar.
///
/// Es la salida honesta: se guarda como decisión del usuario (`ConfirmadoPorUsuario`), no
/// como observación. No toca la instantánea de las pruebas ya guardadas.
#[tauri::command]
async fn asignar_juego_a_sesion(
    sesion_id: String,
    appid: u32,
    nombre: String,
) -> Result<(), String> {
    sessions_service::servicio().asignar_juego(&sesion_id, appid, &nombre)
}

/// Fija qué juego se está mirando, para que el observador sepa qué buscar.
///
/// **No** se usa como evidencia de identidad: solo dice qué nombre mirar en `/proc`. El
/// AppID que se guarde en la sesión sale de lo observado, nunca de esto.
#[tauri::command]
async fn fijar_pista_de_juego(nombre: Option<String>) {
    sessions_service::servicio().fijar_pista(nombre);
}

/// Guarda la configuración vigente en Thoriapp para poder capturarla con su instante si se
/// detecta un juego corriendo. La llama el frontend cuando cambia el perfil o el comando.
#[tauri::command]
async fn fijar_configuracion_de_sesion(
    comando: Option<String>,
    procedencia: Option<String>,
    perfil: Option<String>,
    opciones_steam: Option<String>,
) -> Result<(), String> {
    use benchmarks::{ConProcedencia, Procedencia};
    let proc_de = |p: Option<String>| match p.as_deref() {
        Some("motor_rust") => Procedencia::MotorRust,
        Some("configurado_en_thoriapp") => Procedencia::ConfiguradoEnThoriapp,
        Some("leido_de_steam") => Procedencia::LeidoDeSteam,
        Some("declarado_por_usuario") => Procedencia::DeclaradoPorUsuario,
        _ => Procedencia::Desconocido,
    };
    let ahora = sessions_service::ahora_legible();
    let cfg = sessions::ConfiguracionCapturada {
        comando_confirmado: comando.map(|v| ConProcedencia {
            valor: v,
            procedencia: proc_de(procedencia),
            capturado_en: Some(ahora.clone()),
        }),
        perfil: perfil.map(|v| ConProcedencia {
            valor: v,
            procedencia: Procedencia::ConfiguradoEnThoriapp,
            capturado_en: Some(ahora.clone()),
        }),
        // El motor todavía no publica una revisión estable de comando: no se inventa.
        revision_motor: None,
        opciones_steam: opciones_steam.map(|v| ConProcedencia {
            valor: v,
            procedencia: Procedencia::LeidoDeSteam,
            capturado_en: Some(ahora.clone()),
        }),
        capturado_en: ahora,
        posterior_al_arranque: false,
    };
    sessions_service::servicio().fijar_configuracion(cfg);
    Ok(())
}

fn main() {
    // Antes que nada: dos instancias no pueden compartir los archivos de datos.
    let Some(_cerrojo) = tomar_cerrojo_de_instancia() else {
        eprintln!(
            "Thoriapp ya está abierto. Usa la ventana que ya tienes: dos ventanas a la vez \
             escribirían el mismo historial de pruebas."
        );
        std::process::exit(0);
    };

    if running_as_root() {
        eprintln!(
            "Thoriapp no debe ejecutarse como root: escribiría la configuración del \
             escritorio con dueño root. Ejecútala con tu usuario normal."
        );
        std::process::exit(1);
    }

    // El observador de sesiones arranca aquí y vive lo que viva la app: no depende de la
    // pantalla abierta, porque el juego se lanza desde Steam con Thoriapp minimizado.
    //
    // La preferencia se lee **antes**: el hilo se crea siempre, pero empieza dormido si el
    // usuario lo dejó apagado, y en una instalación nueva no hay preferencia (apagado).
    // As?? el estado inicial es el del archivo, no el que le apetezca al proceso.
    sessions_service::servicio().hidratar_preferencia();
    sessions_service::servicio().arrancar();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_hardware,
            get_rules,
            get_requirements,
            scan_library,
            search_store,
            game_info,
            steam_launch_options,
            steam_launch_options_para_juego,
            benchmarks::ultimo_comando_registrado,
            analyze_log,
            open_url,
            detect_engine,
            get_engine_recommendations,
            get_engine_recommendations_for_gpu,
            get_community_recommendations,
            profiles::save_profile,
            profiles::load_profiles,
            profiles::load_profiles_for_game,
            profiles::delete_profile,
            profiles::export_profile,
            profiles::export_all_profiles,
            profiles::import_profile_from_text,
            profiles::preview_import_profile,
            profiles::update_profile,
            profiles::reorder_profiles,
            benchmarks::save_benchmark,
            benchmarks::load_benchmarks,
            benchmarks::load_benchmarks_for_game,
            benchmarks::is_log_already_saved,
            benchmarks::delete_benchmark,
            benchmarks::delete_benchmarks_for_game,
            benchmarks::delete_all_benchmarks,
            benchmarks::compare_benchmarks_evaluada,
            benchmarks::analyze_mangohud_log_command,
            benchmarks::detect_mangohud_log_path,
            benchmarks::check_mangohud_installed,
            scan_external_launchers,
            refresh_library,
            heronite_uninstalled_count,
            benchmarks::get_mangohud_log_duration,
            benchmarks::set_mangohud_log_duration,
            benchmarks::restore_mangohud_conf,
            preflight::get_runtime_env_command,
            analyze_launch_command,
            serialize_launch_command,
            validate_launch_command,
            launch_command_states,
            catalogo_de_ajustes,
            escape_launch_value,
            known_wrappers,
            launch_command_changes,
            launch_command_remove_segment,
            anticheat::get_anticheat_info,
            proton_pulse::get_proton_pulse_reports,
            compare_benchmark_groups,
            log_watcher::scan_new_mangohud_logs,
            log_watcher::list_mangohud_logs,
            log_watcher::mark_logs_processed,
            log_watcher::processed_logs_count,
            log_watcher::forget_processed_logs,
            shaders::get_shader_cache,
            benchmarks::save_comparison_evaluada,
            benchmarks::load_comparisons,
            benchmarks::delete_comparison,
            benchmarks::delete_all_comparisons,
            get_local_cover,
            diagnose_covers,
            estado_observador,
            configurar_observador,
            listar_sesiones,
            sesion_de_benchmark,
            asignar_juego_a_sesion,
            fijar_pista_de_juego,
            fijar_configuracion_de_sesion,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la app");
}

// ==================== TESTS ====================
// Solo la validación de URLs: es lógica pura y es la puerta de seguridad.

#[cfg(test)]
mod tests {
    use super::{host_matches, uid_is_root};

    #[test]
    fn el_chequeo_de_root_entiende_el_formato_de_proc() {
        // Formato real: "Uid:\t<real>\t<efectivo>\t<salvado>\t<fs>". Se usa el
        // primero, que es el del usuario real del proceso.
        assert!(uid_is_root("Name:\tthoriapp\nUid:\t0\t0\t0\t0\nGid:\t0\t0\t0\t0"));
        assert!(!uid_is_root("Name:\tthoriapp\nUid:\t1000\t1000\t1000\t1000"));
    }

    #[test]
    fn el_chequeo_de_root_no_acepta_basura() {
        // Sin línea `Uid:` no se asume root (mejor abrir que bloquear por un
        // formato inesperado en otro sistema).
        assert!(!uid_is_root(""));
        assert!(!uid_is_root("Gid:\t0\t0\t0\t0"));
        assert!(!uid_is_root("Uid:\troot\t0\t0\t0"));
        assert!(!uid_is_root("Uid:"));
    }

    #[test]
    fn acepta_el_dominio_exacto() {
        assert!(host_matches("https://github.com/foo/bar", "github.com"));
        assert!(host_matches("https://github.com", "github.com"));
        assert!(host_matches("https://github.com/", "github.com"));
    }

    #[test]
    fn rechaza_un_dominio_que_solo_empieza_igual() {
        // Este era el agujero: starts_with("https://github.com/") lo aceptaba.
        assert!(!host_matches("https://github.com.evil.com/x", "github.com"));
        assert!(!host_matches("https://github.com.evil.com/", "github.com"));
        assert!(!host_matches("https://githubXcom/x", "github.com"));
    }

    #[test]
    fn rechaza_subdominios_no_listados() {
        assert!(!host_matches("https://evil.github.com/x", "github.com"));
    }

    #[test]
    fn rechaza_http_sin_tls() {
        assert!(!host_matches("http://github.com/x", "github.com"));
        assert!(!host_matches("file:///etc/passwd", "github.com"));
        assert!(!host_matches("javascript:alert(1)", "github.com"));
    }

    #[test]
    fn ignora_credenciales_y_puerto() {
        assert!(host_matches("https://user:pass@github.com/x", "github.com"));
        assert!(host_matches("https://github.com:443/x", "github.com"));
    }

    #[test]
    fn separa_el_host_de_query_y_fragmento() {
        assert!(host_matches("https://github.com?x=1", "github.com"));
        assert!(host_matches("https://github.com#top", "github.com"));
    }

    #[test]
    fn distingue_cada_dominio_de_la_lista() {
        assert!(host_matches("https://www.protondb.com/app/440", "www.protondb.com"));
        assert!(!host_matches("https://protondb.com/app/440", "www.protondb.com"));
        assert!(host_matches("https://store.steampowered.com/app/440", "store.steampowered.com"));
    }

    #[test]
    fn una_url_vacia_no_pasa() {
        assert!(!host_matches("", "github.com"));
        assert!(!host_matches("https://", "github.com"));
    }
}

#[cfg(test)]
mod catalogo_efectivo_tests {
    use super::*;

    /// Regresión: una recomendación que no está en `rules.params` debe poder activarse.
    ///
    /// Antes solo se marcaba en un `Set` de React y el comando no cambiaba nunca: era el
    /// fallo «selecciono una recomendación y no pasa nada».
    #[test]
    fn una_recomendacion_entra_en_el_catalogo_con_indice_estable() {
        let base = catalogo_efectivo(0, &[]);
        let con_extra = catalogo_efectivo(0, &["RADV_PERFTEST=sam".to_string()]);

        assert_eq!(con_extra.len(), base.len() + 1);
        let extra = con_extra.last().unwrap();
        // El índice es la posición en el array. Aquí coincide con `params.len()` porque
        // este extra no se deduplica, pero **no** se puede reconstruir así en general: el
        // catálogo salta los repetidos (ver `el_mapa_de_indices_coincide…`). El frontend
        // pide el mapa a `catalogo_de_ajustes`, no lo calcula.
        assert_eq!(extra.indice, base.len());
        assert_eq!(extra.valor, "RADV_PERFTEST=sam");
    }

    /// Reproducción completa del escenario que fallaba: 7 Days to Die.
    ///
    /// 7DTD (251570) no tiene ficha propia y cae en el perfil genérico (6 parámetros), que
    /// ya incluye `mangohud` y `PROTON_LOCAL_SHADER_CACHE=1`. La interfaz enseña además las
    /// **recomendaciones**, que repiten esos dos valores. Contando todas las filas visibles,
    /// el frontend calculaba índices 6 y 7 para dos recomendaciones; el catálogo efectivo
    /// solo llega al 5 porque deduplica, así que el motor respondía `IndiceDesconocido` y la
    /// interfaz traducía «Ese ajuste no pertenece a este juego».
    ///
    /// El test recorre la ruta real: catálogo efectivo → análisis → cambios → serialización.
    #[test]
    fn el_flujo_de_7dtd_acepta_las_recomendaciones_repetidas() {
        // Las recomendaciones que React pasa como `extras` (dos ya están en el perfil).
        let extras = vec![
            "mangohud".to_string(),
            "PROTON_LOCAL_SHADER_CACHE=1".to_string(),
            "RADV_PERFTEST=sam".to_string(),
        ];
        let catalogo = catalogo_efectivo(251570, &extras);
        let base = rules::for_appid(251570).params.len();
        assert_eq!(base, 6, "el perfil genérico debería tener 6 parámetros");
        // Solo entra la que no estaba: las otras dos se deduplican (bien).
        assert_eq!(catalogo.len(), base + 1, "solo debería añadirse `RADV_PERFTEST=sam`");

        let original = "PROTON_USE_NTSYNC=1 game-performance mangohud %command%";
        let analisis = launch_command::analizar(original, &catalogo);

        // Lo que hace el frontend: marcar como activo cada valor visible. Los índices que
        // calcula React (6 y 7, contando las recomendaciones) **no existen** en el catálogo;
        // antes eso bastaba para rechazar el cambio.
        let indice_inventado = base + 1;
        let cambios = vec![launch_command::Cambio {
            indice: indice_inventado,
            valor: "RADV_PERFTEST=sam".to_string(),
            activo: true,
        }];

        let comando = launch_command::intentar_serializar(&analisis, &catalogo, &cambios)
            .expect("activar una recomendación visible no puede fallar");

        assert!(
            comando.contains("RADV_PERFTEST=sam"),
            "la recomendación debería estar en el comando: {comando}"
        );
        assert!(comando.ends_with("%command%"), "nada detrás del marcador: {comando}");
        // El resto del comando importado se conserva intacto.
        assert!(comando.contains("PROTON_USE_NTSYNC=1"));
        assert!(comando.contains("game-performance"));
        assert!(comando.contains("mangohud"));
    }

    /// Regresión del P0 «Ese ajuste no pertenece a este juego».
    ///
    /// El frontend calculaba el índice de una recomendación como `params.len() + posición`
    /// contando **todas** las visibles, pero el catálogo efectivo **descarta** las que ya
    /// están en `rules.params` (aquí, `mangohud`). Ese índice no existía y el motor
    /// rechazaba el cambio con `IndiceDesconocido`, aunque el ajuste estuviera en pantalla.
    ///
    /// Ahora la identidad es el valor, así que el índice puede ser cualquiera: lo que
    /// importa es que el valor esté en el catálogo.
    #[test]
    fn un_indice_que_no_existe_no_rompe_el_cambio_si_el_valor_es_correcto() {
        let catalogo = catalogo_efectivo(0, &[]);
        // `mangohud` ya está en el perfil genérico, así que el catálogo lo tiene una sola
        // vez; el frontend lo habría contado dos (params + recomendación).
        let real = catalogo.iter().find(|d| d.valor == "mangohud").unwrap().indice;

        let analisis = launch_command::analizar("%command%", &catalogo);
        let cambios = vec![launch_command::Cambio {
            indice: catalogo.len() + 7, // índice inventado, como el que mandaba React
            valor: "mangohud".to_string(),
            activo: true,
        }];

        let comando = launch_command::intentar_serializar(&analisis, &catalogo, &cambios)
            .expect("el valor sí pertenece al juego: no debe rechazarse");
        assert_eq!(comando, "mangohud %command%");
        // Y el índice sobrante no se ha colado en ninguna decisión.
        assert!(real < catalogo.len());
    }

    /// Regresión del toggle que «no hace nada» (`WINE_FULLSCREEN_FSR=1`).
    ///
    /// El motor aceptaba el cambio porque identifica el ajuste por su **valor**, pero la
    /// interfaz leía el estado con un índice calculado a mano (`params.len() + indexOf`)
    /// que no contaba las recomendaciones descartadas. Con el índice desplazado, la lectura
    /// `e.indice === indice` caía en otro ajuste (o en ninguno) y siempre respondía «no
    /// está puesto»: el toggle se podía pulsar y nunca se pintaba como activo.
    ///
    /// `catalogo_de_ajustes` existe para que la interfaz no tenga que adivinar nada.
    #[test]
    fn el_mapa_de_indices_coincide_con_los_estados_que_devuelve_el_motor() {
        // Lo que manda React: el perfil genérico ya trae `mangohud` y
        // `PROTON_LOCAL_SHADER_CACHE=1`, así que esos dos extras se descartan.
        let extras = vec![
            "mangohud".to_string(),
            "PROTON_LOCAL_SHADER_CACHE=1".to_string(),
            "WINE_FULLSCREEN_FSR=1".to_string(),
        ];
        let catalogo = catalogo_efectivo(0, &extras);
        let mapa: Vec<IndiceAjuste> = catalogo_de_ajustes(0, Some(extras.clone()))
            .into_iter()
            .map(|d| IndiceAjuste {
                indice: d.indice,
                valor: d.valor,
            })
            .collect();
        assert_eq!(mapa.len(), catalogo.len());
        for (m, d) in mapa.iter().zip(catalogo.iter()) {
            assert_eq!(m.indice, d.indice);
            assert_eq!(m.valor, d.valor);
        }

        // El índice real de FSR **no** es `params.len() + 2`, que es lo que habría calculado
        // React contando los tres extras.
        let base = rules::for_appid(0).params.len();
        let real = mapa
            .iter()
            .find(|d| d.valor == "WINE_FULLSCREEN_FSR=1")
            .expect("la recomendación debe estar en el mapa")
            .indice;
        assert_ne!(real, base + 2, "el cálculo a mano volvería a desviarse");
        assert_eq!(real, base, "solo sobrevive un extra y va justo después del perfil");

        // Y con ese índice, el estado se lee y se escribe de verdad.
        let analisis = launch_command::analizar("%command%", &catalogo);
        let cambios = vec![launch_command::Cambio {
            indice: real,
            valor: "WINE_FULLSCREEN_FSR=1".to_string(),
            activo: true,
        }];
        let comando = launch_command::intentar_serializar(&analisis, &catalogo, &cambios)
            .expect("activar FSR debe funcionar");
        assert!(comando.contains("WINE_FULLSCREEN_FSR=1"), "{comando}");

        let reanalizado = launch_command::analizar(&comando, &catalogo);
        let estados = launch_command::estado_de_ajustes(&reanalizado, &catalogo);
        let estado = estados
            .iter()
            .find(|e| e.indice == real)
            .expect("el estado del índice real debe existir");
        assert_eq!(estado.valor, "WINE_FULLSCREEN_FSR=1");
        // Esto es lo que la interfaz leía mal: con el índice correcto sí está activo.
        assert!(
            matches!(estado.situacion, launch_command::Situacion::Activo { .. }),
            "con el índice del motor el toggle se pinta como activo, llegó {:?}",
            estado.situacion
        );
    }

    /// Y al revés: un valor que **no** está en el catálogo sí se rechaza, con su error.
    #[test]
    fn un_valor_ajeno_se_rechaza_aunque_el_indice_exista() {
        let catalogo = catalogo_efectivo(0, &[]);
        let analisis = launch_command::analizar("%command%", &catalogo);
        let cambios = vec![launch_command::Cambio {
            indice: 0,
            valor: "NO_ES_DE_ESTE_JUEGO=1".to_string(),
            activo: true,
        }];

        let r = launch_command::intentar_serializar(&analisis, &catalogo, &cambios);
        assert!(r.is_err(), "se esperaba error, llegó {r:?}");
    }

    /// El motor reconoce el argumento como un ajuste más y lo coloca donde toca.
    #[test]
    fn el_motor_reconoce_la_recomendacion_y_la_anade_al_comando() {
        // Un valor que **no** esté ya en el perfil genérico: si estuviera, el catálogo lo
        // deduplica (bien) y este test no probaría la inserción.
        let recomendacion = "RADV_PERFTEST=nogttspill";
        let catalogo = catalogo_efectivo(0, &[recomendacion.to_string()]);
        let indice = catalogo.len() - 1;
        assert_eq!(catalogo[indice].valor, recomendacion);

        let analisis = launch_command::analizar("mangohud %command%", &catalogo);
        let cambios = launch_command::cambios_para(&analisis, &catalogo, &[indice, 2]);
        let comando = launch_command::intentar_serializar(&analisis, &catalogo, &cambios)
            .expect("el motor debería poder añadir la recomendación");

        assert!(comando.contains(recomendacion), "comando: {comando}");
        assert!(comando.ends_with("%command%"), "comando: {comando}");
    }

    /// Desactivarla la quita del comando, igual que cualquier otro ajuste.
    #[test]
    fn la_recomendacion_se_puede_quitar_y_el_resto_queda_intacto() {
        let catalogo = catalogo_efectivo(0, &["RADV_PERFTEST=nogttspill".to_string()]);
        let original = "RADV_PERFTEST=nogttspill mangohud %command%";
        let analisis = launch_command::analizar(original, &catalogo);

        // `mangohud` sigue activo: solo se apaga la recomendación.
        let cambios = launch_command::cambios_para(&analisis, &catalogo, &[2]);
        let comando = launch_command::intentar_serializar(&analisis, &catalogo, &cambios)
            .expect("se debería poder quitar");
        assert_eq!(comando, "mangohud %command%");
    }

    /// Una recomendación que ya es un ajuste del catálogo no se duplica.
    ///
    /// Con dos definiciones del mismo texto el motor la marca ambigua y activar una apagaría
    /// la otra.
    #[test]
    fn una_recomendacion_que_ya_es_ajuste_no_se_duplica() {
        let base = catalogo_efectivo(0, &[]);
        let repetida = base[0].valor.clone();
        let con_extra = catalogo_efectivo(0, &[repetida]);

        assert_eq!(con_extra.len(), base.len());
    }

    /// El marcador no puede colarse como recomendación: es estructura, no un ajuste.
    #[test]
    fn el_marcador_nunca_es_una_recomendacion() {
        let base = catalogo_efectivo(0, &[]);
        let con_extra = catalogo_efectivo(0, &["%command%".to_string(), "  ".to_string()]);
        assert_eq!(con_extra.len(), base.len());
    }
}
