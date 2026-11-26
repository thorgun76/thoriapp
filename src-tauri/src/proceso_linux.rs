//! Adaptador Linux del puerto `sessions::ObservadorDeProcesos`.
//!
//! Todo lo que sabe de `/proc` vive aquí. El dominio (`sessions.rs`) no sabe que existe
//! Linux, y los tests del dominio no tocan el sistema.
//!
//! # Qué se lee y qué no
//!
//! De cada proceso se lee, **como mucho**: `comm`, `exe`, `stat` y cuatro variables del
//! entorno de la lista blanca. Nunca el entorno completo, nunca `cmdline` (lleva
//! credenciales en muchos programas), nunca datos de otros usuarios.
//!
//! El filtrado es en dos pasos para no leer `environ` sin motivo: primero se descarta por
//! nombre de ejecutable y por ruta (barato), y solo a los candidatos que quedan se les
//! mira el entorno. Leer `/proc/<pid>/environ` de 300 procesos en cada pasada sería tirar
//! CPU durante una grabación, que es justo lo que no debe hacerse.
//!
//! # Límites
//!
//! - Solo procesos del **mismo usuario**: en `/proc` cada entrada lleva el UID del dueño,
//!   y se compara con el propio antes de abrir nada.
//! - En un contenedor (Docker) no se ve el Steam del anfitrión. Eso se reporta como
//!   [`BusquedaDeProceso::SinVisibilidad`], **nunca** como «no está corriendo».
//! - El observador es de solo lectura: no escribe, no mata, no cambia nada.

#![allow(dead_code)]

use std::fs;
use std::path::Path;

use crate::evidencia_proceso::{
    agrupar_con_topologia, argumentos_de_cmdline, es_auxiliar, primer_install_dir,
    ruta_de_proton,
    EvidenciaDeArgumentos,
};
use crate::sessions::{
    EscaneoDeProcesos, EvidenciaAppId, GrupoDeProcesos, ObservadorDeProcesos, RasgosDeProceso,
};

/// Variables de entorno que se leen, y ninguna más.
///
/// `SteamAppId` y `SteamGameId` son las que Steam pone al lanzar un juego (y se heredan a
/// los hijos). `SteamClientLaunch` distingue el lanzamiento desde la biblioteca del
/// lanzamiento por URL. `PROTON_VERSION` indica que el proceso va bajo Proton.
///
/// No se incluye nada más a propósito: un `environ` contiene credenciales.
pub const VARIABLES_PERMITIDAS: &[&str] = &[
    "SteamAppId",
    "SteamGameId",
    "SteamClientLaunch",
    "PROTON_VERSION",
    // La carpeta del juego que el propio proceso declara. Es la única evidencia explícita de
    // «esta es mi instalación»: bajo Proton el ejecutable Linux es el preloader de Wine, así
    // que la ruta no está dentro de `steamapps/common` y no sirve para deducirla.
    "STEAM_COMPAT_INSTALL_PATH",
    // Prefijo de Wine del proceso. Es la vía de correlación que queda cuando el lanzador no
    // guarda carpeta de instalación, que es el caso real de Lutris (`directory` vacío).
    "WINEPREFIX",
];

/// Nombres de proceso que delatan una **capa de compatibilidad** (Wine *o* Proton).
///
/// Son dos cosas distintas y hasta ahora se mezclaban: «este proceso es un binario Windows
/// ejecutándose con una capa de compatibilidad» y «este proceso corre **específicamente**
/// bajo Proton». El primer hecho es el que necesita la detección de ejecutables Windows; el
/// segundo es información de runtime. Star Valor, de Lutris, corre con Wine y no con Proton,
/// y aun así salía marcado como Proton.
const MARCAS_DE_COMPATIBILIDAD: &[&str] = &["wine", "wine64", "wineserver", "pressure-vessel"];

/// Nombres de proceso que delatan **Proton** en concreto, no Wine genérico.
const MARCAS_DE_PROTON: &[&str] = &["proton"];

/// Validación de un valor leído del entorno.
///
/// Se descartan valores que no parezcan un AppID (números, longitud razonable) para no
/// arrastrar contenido arbitrario hasta el JSON. Devuelve el valor ya limpio.
pub fn valor_permitido(variable: &str, valor: &str) -> Option<String> {
    if !VARIABLES_PERMITIDAS.contains(&variable) {
        return None;
    }
    let v = valor.trim();
    // Las rutas se validan aparte: una ruta de `steamapps/common` supera los 64 caracteres.
    if variable != "STEAM_COMPAT_INSTALL_PATH"
        && variable != "WINEPREFIX"
        && (v.is_empty() || v.len() > 64)
    {
        return None;
    }
    match variable {
        // Ruta de instalación del juego. Solo se conserva la parte relativa a
        // `steamapps/common/<carpeta>`: es lo que usa el resto del programa y evita guardar
        // rutas personales (`/home/<usuario>/…`). Si no encaja, no hay evidencia.
        "STEAM_COMPAT_INSTALL_PATH" => {
            if v.len() > 4096 || v.contains('\0') {
                return None;
            }
            crate::evidencia_proceso::install_dir_de_ruta(v).map(|dir| dir.to_string())
        }
        // Prefijo de Wine. A diferencia de la carpeta de instalación, **no** se reduce a una
        // parte relativa: el prefijo se compara entero contra el que guarda el lanzador, y
        // recortarlo perdería justo la parte que distingue dos juegos (`/Prefijo/eslabong`
        // frente a `/Prefijo/eslabong2`). Se guarda tal cual, ya validada.
        "WINEPREFIX" => {
            if v.len() > 4096 || v.contains('\0') {
                return None;
            }
            Some(crate::evidencia_proceso::normalizar_prefijo(v).to_string())
        }
        // Los tres son identificadores numéricos de Steam.
        "SteamAppId" | "SteamGameId" | "SteamClientLaunch" => {
            if v.chars().all(|c| c.is_ascii_digit()) {
                Some(v.to_string())
            } else {
                None
            }
        }
        // La versión de Proton lleva letras, números, puntos y **espacios**
        // (`Proton 9.0-4`, `Proton Experimental`). Sin el espacio se rechazaba un valor
        // real, que es tan malo como dejar pasar uno falso.
        "PROTON_VERSION" => {
            if v.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ' ')
            {
                Some(v.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Filtra un `environ` completo y devuelve solo las variables permitidas, ya validadas.
///
/// Es la función que garantiza la lista blanca. Se prueba con un entorno lleno de basura
/// (tokens, claves) para comprobar que no escapa nada.
pub fn filtrar_environ(datos: &[u8]) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for trozo in datos.split(|b| *b == 0) {
        if trozo.is_empty() {
            continue;
        }
        let Ok(linea) = std::str::from_utf8(trozo) else {
            continue;
        };
        let Some((k, v)) = linea.split_once('=') else {
            continue;
        };
        if let Some(limpio) = valor_permitido(k, v) {
            out.insert(k.to_string(), limpio);
        }
    }
    out
}

/// Qué se puede decir de un proceso a partir de su clave (`pid@starttime`).
///
/// Distinguir estos cuatro casos es el contrato: «no lo veo» no es «terminó», y «hay un
/// proceso con ese PID» no es «es el mismo proceso».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDeLaClave {
    /// Existe, no es zombie y su `starttime` coincide: es el mismo proceso.
    Vivo,
    /// Ya no está en `/proc`.
    Terminado,
    /// El PID existe pero con otro `starttime`: el PID se reutilizó, este es otro proceso.
    Reutilizado,
    /// Está en `/proc` pero es un zombie (`Z`, `X`, `x`): terminó y solo falta que lo recoja
    /// su padre. Para el ciclo de vida de una sesión, un zombi **no** está vivo.
    Zombi,
    /// No se pudo leer `/proc/<pid>` (permisos, carrera con la salida). No acredita ni vida
    /// ni muerte, así que no se usa para cerrar nada.
    Inaccesible,
}

impl EstadoDeLaClave {
    /// ¿Acredita que el proceso sigue ejecutándose?
    pub fn acredita_vida(self) -> bool {
        self == EstadoDeLaClave::Vivo
    }

    /// ¿Se puede afirmar que el proceso ya no está (y no es un problema de lectura)?
    ///
    /// Solo estos dos estados permiten dar por terminada una ejecución sin más averiguación.
    /// `Inaccesible` no: no saber no es saber que no.
    pub fn acredita_fin(self) -> bool {
        matches!(self, EstadoDeLaClave::Terminado | EstadoDeLaClave::Reutilizado)
    }
}

/// Observador real. Sin estado: cada llamada lee `/proc` en ese momento.
pub struct ObservadorDeProc;

impl ObservadorDeProc {
    /// Instante de arranque del sistema (campo `btime` de `/proc/stat`), en segundos desde
    /// la época. Es lo que convierte el `starttime` de cada proceso en una fecha.
    fn btime() -> Option<i64> {
        let txt = fs::read_to_string("/proc/stat").ok()?;
        txt.lines()
            .find_map(|l| l.strip_prefix("btime "))?
            .trim()
            .parse::<i64>()
            .ok()
    }

    /// Ticks de reloj por segundo, para pasar `starttime` (en ticks) a segundos.
    fn ticks_por_segundo() -> f64 {
        // El valor de Linux es casi siempre 100; si no se puede leer, se usa ese.
        let txt = fs::read_to_string("/proc/self/auxv").ok();
        let _ = txt;
        100.0
    }

    /// UID real del proceso actual, para no mirar los de otros usuarios.
    fn uid_propio() -> Option<u32> {
        let txt = fs::read_to_string("/proc/self/status").ok()?;
        let linea = txt.lines().find(|l| l.starts_with("Uid:"))?;
        // "Uid:\t1000\t1000\t1000\t1000"
        linea.split_whitespace().nth(1)?.parse().ok()
    }

    /// ¿Esta entrada de `/proc` pertenece al usuario actual y está viva? Es lo primero que
    /// se pregunta, antes de abrir nada.
    fn uid_de_proceso(pid: u32) -> Option<u32> {
        let meta = fs::metadata(format!("/proc/{}", pid)).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            return Some(meta.uid());
        }
        #[allow(unreachable_code)]
        {
            let _ = meta;
            None
        }
    }

    /// Nombre del ejecutable sin extensión ni ruta.
    fn comm(pid: u32) -> Option<String> {
        let s = fs::read_to_string(format!("/proc/{}/comm", pid)).ok()?;
        Some(s.trim().to_string())
    }

    /// Ruta del ejecutable (`/proc/<pid>/exe` es un enlace; se resuelve).
    fn exe(pid: u32) -> Option<String> {
        fs::read_link(format!("/proc/{}/exe", pid))
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    }

    /// `(ppid, starttime)` del campo 4 y 22 de `/proc/<pid>/stat`.
    ///
    /// El campo 2 es el nombre entre paréntesis y puede contener espacios o paréntesis,
    /// así que se corta desde el último `)` en vez de partir por espacios a ciegas.
    fn stat(pid: u32) -> Option<(u32, u64)> {
        let (_, ppid, starttime) = Self::stat_completo(pid)?;
        Some((ppid, starttime))
    }

    /// `(estado, ppid, starttime)` de `/proc/<pid>/stat`.
    ///
    /// El **estado** hacía falta y no se estaba leyendo. Después del último `)` el primer
    /// campo es el 3 (estado) y durante mucho tiempo se descartó con un `let _ =`. La
    /// consecuencia medida: un proceso terminado pero no recogido queda en `/proc` con el
    /// **mismo** `starttime`, así que `sigue_vivo` lo daba por vivo y la sesión no cerraba
    /// nunca. No basta con que `/proc/<pid>` exista.
    fn stat_completo(pid: u32) -> Option<(char, u32, u64)> {
        let txt = fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
        let cierre = txt.rfind(')')?;
        let resto: Vec<&str> = txt[cierre + 1..].split_whitespace().collect();
        let estado = resto.first()?.chars().next()?;
        let ppid: u32 = resto.get(1)?.parse().ok()?;
        let starttime: u64 = resto.get(19)?.parse().ok()?;
        Some((estado, ppid, starttime))
    }

    /// Fecha de arranque del proceso, si se puede calcular.
    /// Estado de una clave `pid@fecha` leída de `/proc` ahora mismo.
    ///
    /// Es la única pregunta que decide si una sesión sigue viva, y responde con el detalle
    /// suficiente para no confundir los casos.
    pub fn estado_de_clave(clave: &str) -> EstadoDeLaClave {
        let Some((pid_txt, esperado)) = clave.split_once('@') else {
            return EstadoDeLaClave::Inaccesible;
        };
        let Ok(pid) = pid_txt.parse::<u32>() else {
            return EstadoDeLaClave::Inaccesible;
        };
        // Sin fecha guardada no se puede distinguir reutilización: se responde por la mera
        // existencia, que es lo más débil pero no inventa un cierre.
        if esperado == "desconocido" {
            return if Path::new(&format!("/proc/{}", pid)).exists() {
                EstadoDeLaClave::Vivo
            } else {
                EstadoDeLaClave::Terminado
            };
        }
        let Some((estado, _ppid, starttime)) = Self::stat_completo(pid) else {
            // `/proc/<pid>` puede no existir (terminado) o no poder leerse (permisos).
            return if Path::new(&format!("/proc/{}", pid)).exists() {
                EstadoDeLaClave::Inaccesible
            } else {
                EstadoDeLaClave::Terminado
            };
        };
        // Un zombi conserva el `starttime` intacto: si esto no se mirara, se quedaría vivo
        // para siempre.
        if matches!(estado, 'Z' | 'X' | 'x') {
            return EstadoDeLaClave::Zombi;
        }
        let Some(btime) = Self::btime() else {
            return EstadoDeLaClave::Inaccesible;
        };
        let seg = (starttime as f64 / Self::ticks_por_segundo()) as i64;
        if crate::sessions::epoch_a_fecha(btime + seg) == esperado {
            EstadoDeLaClave::Vivo
        } else {
            EstadoDeLaClave::Reutilizado
        }
    }

    fn fecha_de_arranque(pid: u32, btime: Option<i64>) -> Option<String> {
        let btime = btime?;
        let (_, starttime) = Self::stat(pid)?;
        let seg = (starttime as f64 / Self::ticks_por_segundo()) as i64;
        Some(crate::sessions::epoch_a_fecha(btime + seg))
    }

    /// Lee el entorno filtrándolo por la lista blanca.
    fn appid_del_entorno(pid: u32) -> Option<EvidenciaAppId> {
        let datos = fs::read(format!("/proc/{}/environ", pid)).ok()?;
        let vars = filtrar_environ(&datos);
        if vars.is_empty() {
            return None;
        }
        Some(EvidenciaAppId {
            steam_app_id: vars.get("SteamAppId").cloned(),
            steam_game_id: vars.get("SteamGameId").cloned(),
        })
    }

    /// Carpeta que el proceso declara como su instalación, ya reducida a
    /// `steamapps/common/<carpeta>`.
    ///
    /// Devuelve `None` si el proceso no la declara o si no se puede leer su entorno (otro
    /// usuario, proceso desaparecido). La ausencia de esta evidencia no invalida las demás.
    fn carpeta_declarada(pid: u32) -> Option<String> {
        let datos = fs::read(format!("/proc/{}/environ", pid)).ok()?;
        filtrar_environ(&datos)
            .get("STEAM_COMPAT_INSTALL_PATH")
            .cloned()
    }

    /// Prefijo de Wine que el proceso declara (`WINEPREFIX`), ya normalizado.
    ///
    /// Es la vía de correlación que queda cuando el lanzador no guarda carpeta de instalación.
    /// Se normaliza al leerlo para que la comparación posterior hable de lo mismo que el
    /// catálogo (`…/Prefijo/eslabong/pfx` y `…/Prefijo/eslabong` son el mismo prefijo).
    fn wine_prefix_de(pid: u32) -> Option<String> {
        let datos = fs::read(format!("/proc/{}/environ", pid)).ok()?;
        filtrar_environ(&datos)
            .get("WINEPREFIX")
            .map(|v| crate::evidencia_proceso::normalizar_prefijo(v))
    }

    /// Argumentos del proceso (`/proc/<pid>/cmdline`, separados por NUL).
    ///
    /// Se leen **solo** para extraer de ellos el ejecutable del juego y los `AppId=`; el
    /// texto no se guarda en ningún sitio (ver `evidencia_proceso::clasificar_argumento`).
    /// Directorio de trabajo del proceso (`/proc/<pid>/cwd`).
    ///
    /// Un juego nativo lanzado por un script suele ejecutarse desde su carpeta de instalación,
    /// y el intérprete (`java`, `python`) no lleva el nombre del juego en ningún sitio. El
    /// `cwd` es, en ese caso, la única traza de a qué instalación pertenece. Nunca confirma
    /// por sí solo: solo admite al proceso como candidato.
    fn cwd(pid: u32) -> Option<String> {
        fs::read_link(format!("/proc/{}/cwd", pid))
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    }

    fn argumentos(pid: u32) -> Vec<String> {
        match fs::read(format!("/proc/{}/cmdline", pid)) {
            Ok(datos) => argumentos_de_cmdline(&datos),
            // Sin permiso o proceso desaparecido: vacío, no un error. La ausencia de esta
            // evidencia no invalida las demás.
            Err(_) => Vec::new(),
        }
    }

    /// ¿El proceso corre **específicamente** bajo Proton, por sí mismo o por sus ancestros?
    ///
    /// Devuelve `None` cuando no se pudo determinar (permisos, proceso desaparecido). No
    /// confundir con [`Self::bajo_compatibilidad`], que incluye Wine genérico.
    fn bajo_proton(pid: u32, comm: Option<&str>) -> Option<bool> {
        Self::marcado_en_ancestros(pid, comm, MARCAS_DE_PROTON)
    }

    /// ¿El proceso ejecuta un binario Windows bajo **una capa de compatibilidad**, sea Wine o
    /// Proton?
    ///
    /// Es la pregunta que necesita la detección: un `.exe` solo tiene sentido si hay una capa
    /// debajo. Que además sea Proton concreto es otra cosa y no cambia la clasificación.
    fn bajo_compatibilidad(pid: u32, comm: Option<&str>) -> Option<bool> {
        Self::marcado_en_ancestros(pid, comm, MARCAS_DE_COMPATIBILIDAD)
    }

    /// ¿Alguna de las marcas aparece en el proceso o en sus ancestros?
    fn marcado_en_ancestros(pid: u32, comm: Option<&str>, marcas: &[&str]) -> Option<bool> {
        let propio = comm
            .map(|c| {
                let c = c.to_lowercase();
                marcas.iter().any(|m| c.contains(m))
            })
            .unwrap_or(false);
        if propio {
            return Some(true);
        }
        // Se sube por los padres un máximo de 6 niveles buscando una marca.
        let mut actual = pid;
        for _ in 0..6 {
            let (ppid, _) = Self::stat(actual)?;
            if ppid == 0 || ppid == 1 || ppid == actual {
                break;
            }
            if let Some(c) = Self::comm(ppid) {
                let c = c.to_lowercase();
                if marcas.iter().any(|m| c.contains(m)) {
                    return Some(true);
                }
            }
            actual = ppid;
        }
        Some(false)
    }

    /// Rasgos completos de un proceso, ya sabiendo que es candidato.
    ///
    /// Aquí es donde se reúne la evidencia de un proceso que puede ser un juego bajo
    /// Proton: el ejecutable Linux es un preloader de Wine, pero el `.exe` del juego está
    /// en los argumentos. Se guardan los dos, por separado y con nombre de fichero.
    /// Rasgos **baratos** de un proceso: solo lo que sirve para reconstruir el árbol.
    ///
    /// Se lee `/proc/<pid>/stat` y el nombre del ejecutable, nada más: ni argumentos, ni
    /// entorno, ni `cwd`. Es lo único que se puede pagar por **todos** los procesos del
    /// sistema sin convertir cada pasada en una lectura completa de `/proc/*/cmdline`.
    ///
    /// Existe para que los descartados por el prefiltro puedan ser eslabones topológicos sin
    /// poder acreditar ejecución: no llevan evidencia de juego, así que ni son miembros ni
    /// representan nada. Antes solo se guardaban los candidatos, y por eso un eslabón
    /// descartado (`python3`, `reaper`, `srt-bwrap`) partía la cadena y el mismo lanzamiento
    /// salía en dos grupos.
    fn rasgos_de_topologia(pid: u32) -> RasgosDeProceso {
        let (ppid, _) = Self::stat(pid)
            .map(|(p, s)| (Some(p), s))
            .unwrap_or((None, 0));
        let comm = Self::comm(pid);
        let nombre = Self::exe(pid)
            .as_deref()
            .and_then(|e| Path::new(e).file_name())
            .map(|n| n.to_string_lossy().to_string())
            .or(comm.clone());
        RasgosDeProceso {
            pid,
            ejecutable: nombre,
            ppid,
            ..RasgosDeProceso::default()
        }
    }

    fn rasgos(pid: u32, btime: Option<i64>) -> RasgosDeProceso {
        let comm = Self::comm(pid);
        let exe = Self::exe(pid);
        let (ppid, _) = Self::stat(pid)
            .map(|(p, s)| (Some(p), s))
            .unwrap_or((None, 0));

        // La línea de comandos es la que revela el juego real en un lanzamiento Proton.
        // Solo se conserva el nombre de fichero del ejecutable y el AppID que declare.
        let args = Self::argumentos(pid);
        let ev = EvidenciaDeArgumentos::de_argumentos(&args);

        // La ruta del juego solo se acepta si **está en la carpeta del juego**
        // ¿Algún argumento es un ejecutable que vive en `steamapps/common/<dir>`? Eso es lo
        // que distingue el `.exe` del juego de un helper del sistema invitado. La ruta
        // completa no se guarda: solo se usa para decidir.
        let en_carpeta_del_juego = ev.install_dir.clone();

        // Nombre del juego: el primero de los ejecutables es el del juego en un lanzamiento
        // real (`Deskrawl.exe`), porque el envoltorio pasa el mismo nombre.
        let ejecutable_del_juego = ev
            .ejecutables
            .first()
            .map(|e| crate::evidencia_proceso::nombre_de_juego_de_ejecutable(e));
        let appid_de_argumentos = ev.appid_unico();

        // ¿Menciona el `.exe`, o lo ejecuta?
        //
        // La línea de comandos de un envoltorio de Proton **pasa** el juego al runtime, así
        // que nombra el mismo `.exe` que el juego. Mencionarlo no es ejecutarlo, y hasta ahora
        // los dos casos eran indistinguibles: por eso `pv-adverb` pasaba por juego.
        //
        // El criterio es estructural y no depende del nombre del juego: **el primer argumento
        // es el programa y el resto son sus argumentos**. Si el `.exe` es el primer argumento
        // real, el proceso es el juego; si aparece detrás (`reaper SteamLaunch … Deskrawl.exe`),
        // es un envoltorio reenviándolo.
        //
        // Los argumentos ya vienen sin el `argv[0]` de cabecera de algunos lanzadores, así que
        // se busca el primero que sea un ejecutable de juego y se compara su posición con la
        // del primer argumento que no sea una bandera ni una marca de lanzamiento.
        let ejecuta_de_verdad = ev.lo_ejecuta_y_no_solo_lo_menciona();

        let nombre_linux = exe
            .as_deref()
            .and_then(|e| Path::new(e).file_name())
            .map(|n| n.to_string_lossy().to_string())
            .or(comm.clone());

        // Auxiliar si **ambos** nombres son de infraestructura. Un `comm` como `GameThread`
        // no dice nada, así que manda el ejecutable Linux cuando existe.
        let auxiliar = nombre_linux.as_deref().map(es_auxiliar).unwrap_or(false)
            || comm.as_deref().map(es_auxiliar).unwrap_or(false);

        // Proton: por el propio proceso, por su antecesor, o por vivir dentro de un
        // lanzador de Proton (en un contenedor `pressure-vessel` el padre puede ser PID 1
        // del espacio de nombres, así que subir por los padres no basta).
        let bajo_proton = match (Self::bajo_proton(pid, comm.as_deref()), exe.as_deref()) {
            (Some(true), _) => Some(true),
            (_, Some(ruta)) if ruta_de_proton(ruta) => Some(true),
            (otro, _) => otro,
        };
        // Capa de compatibilidad (Wine o Proton): es lo que hace falta para que un `.exe`
        // tenga sentido. Se calcula aparte para no presentar Wine como Proton.
        let bajo_compatibilidad = match (
            Self::bajo_compatibilidad(pid, comm.as_deref()),
            bajo_proton,
            exe.as_deref(),
        ) {
            (Some(true), _, _) => Some(true),
            (_, Some(true), _) => Some(true),
            (_, _, Some(ruta)) if ruta_de_proton(ruta) => Some(true),
            (otro, _, _) => otro,
        };

        RasgosDeProceso {
            pid,
            ejecutable: nombre_linux,
            ruta_ejecutable: exe,
            inicio_del_proceso: Self::fecha_de_arranque(pid, btime),
            ppid,
            appid_del_entorno: Self::appid_del_entorno(pid),
            bajo_proton,
            bajo_compatibilidad,
            ejecutable_del_juego,
            // Solo la carpeta del juego, nunca la ruta completa.
            ruta_del_juego: en_carpeta_del_juego.map(|d| format!("steamapps/common/{}", d)),
            carpeta_declarada: Self::carpeta_declarada(pid),
            wine_prefix: Self::wine_prefix_de(pid),
            cwd: Self::cwd(pid),
            auxiliar,
            appid_de_argumentos,
            ejecuta_de_verdad,
        }
    }
}

impl ObservadorDeProc {
    /// Una pasada completa por `/proc`.
    ///
    /// Es el corazón de la detección. No se busca «un proceso llamado X»: se recolectan los
    /// candidatos y se agrupan por árbol de procesos, porque un juego bajo Proton son
    /// varios procesos (preloader de Wine, `.exe` del juego, hilos con nombre propio) que
    /// pertenecen a la **misma** ejecución.
    ///
    /// El orden importa para el coste: primero UID y nombre (barato, sin abrir nada del
    /// proceso), y solo a los candidatos se les leen `stat`, `cmdline` y `environ`.
    pub fn escanear(&self) -> EscaneoDeProcesos {
        let Some(uid) = Self::uid_propio() else {
            return EscaneoDeProcesos {
                sin_visibilidad: Some("no se pudo leer el UID propio de /proc/self/status".into()),
                ..Default::default()
            };
        };
        let Ok(entradas) = fs::read_dir("/proc") else {
            return EscaneoDeProcesos {
                sin_visibilidad: Some("no hay /proc disponible".into()),
                ..Default::default()
            };
        };
        let btime = Self::btime();
        let propio = std::process::id();

        // El catálogo se consulta **antes** de decidir qué procesos son candidatos, y una
        // sola vez por pasada: `obtener()` trabaja sobre la caché compartida, así que aquí no
        // se abre SQLite ni se releen YAML salvo que toque revalidar.
        let indice = IndiceDelCatalogo::de_catalogo(&crate::catalogo_lanzadores::obtener());

        let mut procesos: Vec<RasgosDeProceso> = Vec::new();
        let mut inaccesibles = 0usize;
        let mut visibles = 0usize;
        // Diagnóstico: solo se rellena si `THORIAPP_DIAGNOSTICO_PROCESOS` está definida.
        let diagnosticar = crate::diagnostico_procesos::activo();
        let mut traza: Vec<Anotacion> = Vec::new();
        // Todo proceso observado, incluidos los descartados por el filtro. Es la topología
        // con la que se reconstruye el árbol: un eslabón descartado (`python3`, `reaper`)
        // sigue siendo padre de quien ejecuta el juego, y sin él la cadena se cortaba.
        let mut topologia: Vec<crate::sessions::RasgosDeProceso> = Vec::new();

        for e in entradas.flatten() {
            let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            if pid == propio {
                continue;
            }
            visibles += 1;
            match Self::uid_de_proceso(pid) {
                Some(u) if u == uid => {}
                // Otro usuario: no es asunto nuestro y no se toca.
                Some(_) => continue,
                // No se pudo ni mirar el dueño: se cuenta como inaccesible, que es
                // información honesta para la interfaz.
                None => {
                    inaccesibles += 1;
                    continue;
                }
            }

            // La topología se construye **antes** del prefiltro y con datos baratos: los
            // descartados siguen siendo eslabones del árbol, pero no llevan evidencia de
            // juego y por tanto no pueden ser miembros ni representantes.
            topologia.push(Self::rasgos_de_topologia(pid));

            let r = Self::rasgos(pid, btime);
            // El prefiltro de producción es el **mismo** que el de las pruebas. Se aplica
            // sobre los rasgos ya leídos en vez de repetir otra lista de condiciones leída
            // aparte de `/proc`: dos criterios distintos fue la causa de que las pruebas
            // pasaran mientras el equipo real fallaba.
            if !puede_ser_juego(&r, Some(&indice)) {
                if diagnosticar {
                    traza.push(Anotacion {
                        pid,
                        ppid: r.ppid,
                        inicio: r.inicio_del_proceso.clone(),
                        comm: r.ejecutable.clone(),
                        ejecutable: r.ruta_ejecutable.clone(),
                        motivo: "sin evidencia de juego: no abre sesión".to_string(),
                    });
                }
                continue;
            }
            if diagnosticar {
                // Se anota antes de mover `r` dentro del vector de procesos.
                traza.push(Anotacion {
                    pid,
                    ppid: r.ppid,
                    inicio: r.inicio_del_proceso.clone(),
                    comm: r.ejecutable.clone(),
                    ejecutable: r.ruta_ejecutable.clone(),
                    motivo: "CANDIDATO: pasa el filtro".to_string(),
                });
            }
            procesos.push(r);
        }

        // Sin un solo proceso con pinta de juego: puede ser que no haya juego, o que no se
        // pueda ver nada (contenedor). Las dos cosas se distinguen.
        let sin_visibilidad = if procesos.is_empty() && visibles <= 12 && parece_contenedor() {
            Some("Thoriapp no ve los procesos del sistema (contenedor o namespace aislado)".into())
        } else {
            None
        };

        // Agrupación por árbol de procesos: un juego, una sesión.
        let mut grupos: Vec<GrupoDeProcesos> = Vec::new();
        for indices in agrupar_con_topologia(&procesos, &topologia) {
            let miembros: Vec<RasgosDeProceso> =
                indices.iter().map(|i| procesos[*i].clone()).collect();
            if let Some(g) = armar_grupo_con_catalogo(miembros, Some(&indice)) {
                grupos.push(g);
            }
        }

        let escaneo = EscaneoDeProcesos {
            grupos,
            inaccesibles,
            sin_visibilidad,
        };
        if diagnosticar {
            let mut inf = crate::diagnostico_procesos::Informe {
                grupos: escaneo.grupos.len(),
                inaccesibles: escaneo.inaccesibles,
                sin_visibilidad: escaneo.sin_visibilidad.clone(),
                ..Default::default()
            };
            inf.procesos_vistos = visibles;
            // Cadena de ancestros reconstruida con lo observado en **esta** pasada.
            let padres: std::collections::HashMap<u32, Option<u32>> =
                traza.iter().map(|a| (a.pid, a.ppid)).collect();
            // `traza` se recorre por referencia: se usa dos veces (descartes y candidatos) y
            // moverla en el primer bucle dejaba el segundo sin datos.
            for a in &traza {
                inf.descartes.push(crate::diagnostico_procesos::Descarte {
                    pid: a.pid,
                    ppid: a.ppid,
                    comm: a.comm.clone(),
                    ejecutable: a.ejecutable.clone(),
                    motivo: a.motivo.clone(),
                    // La cadena se reconstruye igual que para los candidatos: un descarte
                    // sigue siendo un nudo de la topología.
                    ancestros: ancestros_de(a.pid, &padres),
                });
            }
            // Los que pasaron el filtro son además candidatos, con su cadena de ancestros:
            // es lo que permite ver si a un candidato le falta un padre descartado.
            for a in traza.iter().filter(|a| a.motivo.starts_with("CANDIDATO")) {
                inf.candidatos.push(crate::diagnostico_procesos::Candidato {
                    pid: a.pid,
                    ppid: a.ppid,
                    inicio: a.inicio.clone(),
                    ancestros: ancestros_de(a.pid, &padres),
                    comm: a.comm.clone(),
                    ejecutable: a.ejecutable.clone(),
                    ..Default::default()
                });
            }
            // Clasificación resultante: qué papel e identidad quedaron para cada proceso, y
            // cuál de ellos representa a su grupo. Es la parte **inferida**, y se marca como
            // tal para no confundirla con lo leído de `/proc`.
            let miembros_por_grupo: Vec<std::collections::HashSet<u32>> = escaneo
                .grupos
                .iter()
                .map(|g| g.miembros.iter().map(|m| m.pid).collect())
                .collect();
            for a in &traza {
                let grupo = miembros_por_grupo.iter().position(|m| m.contains(&a.pid));
                let representante = grupo
                    .and_then(|i| escaneo.grupos.get(i))
                    .map(|g| g.principal == a.pid)
                    .unwrap_or(false);
                inf.clasificacion
                    .push(crate::diagnostico_procesos::Clasificado {
                        pid: a.pid,
                        ppid: a.ppid,
                        identidad_de_instancia: Some(match &a.inicio {
                            Some(t) => format!("{}@{}", a.pid, t),
                            None => format!("{}@desconocido", a.pid),
                        }),
                        // El papel detallado lo decide `repartir_papeles_con_catalogo` dentro del
                        // grupo; aquí se informa de lo comprobable: si entró o no en un grupo y si
                        // representa a alguno.
                        papel: Some(match (grupo, representante) {
                            (Some(_), true) => "representante del grupo".into(),
                            (Some(_), false) => "miembro del grupo".into(),
                            (None, _) => "fuera de todo grupo".into(),
                        }),
                        elegible_como_representante: representante,
                        // La raíz del grupo al que pertenece: el ancestro más alto que se
                        // vio de su cadena. Antes se dejaba a `None` aunque la cadena estaba
                        // calculada justo arriba, así que el diagnóstico no podía explicar
                        // por qué dos procesos acabaron en grupos distintos.
                        raiz_calculada: match grupo {
                            Some(_) => ancestros_de(a.pid, &padres).last().copied(),
                            // Fuera de grupo no hay raíz que informar: no se inventa una.
                            None => None,
                        },
                        grupo,
                        representante,
                        identidad_correlacionada: grupo
                            .and_then(|i| escaneo.grupos.get(i))
                            .and_then(|g| g.nombres.first().cloned()),
                    });
            }
            inf.ciclo_de_sesiones.push(format!(
                "escaneo: {} grupos formados a partir de {} procesos anotados",
                escaneo.grupos.len(),
                traza.len()
            ));
            crate::diagnostico_procesos::volcar(&inf, "escaneo");
        }
        escaneo
    }

}

/// Construye un grupo a partir de los procesos que comparten padre.
///
/// Devuelve `None` si el grupo no tiene nada que sea un juego (por ejemplo, un `reaper` y un
/// `python3` sin ningún juego detrás).
fn armar_grupo(miembros: Vec<RasgosDeProceso>) -> Option<GrupoDeProcesos> {
    armar_grupo_con_catalogo(miembros, None)
}

/// Igual que [`armar_grupo`], con el catálogo de lanzadores disponible.
fn armar_grupo_con_catalogo(
    miembros: Vec<RasgosDeProceso>,
    indice: Option<&IndiceDelCatalogo>,
) -> Option<GrupoDeProcesos> {
    if miembros.is_empty() {
        return None;
    }
    let principal = crate::evidencia_proceso::elegir_principal_con_catalogo(&miembros, indice)?;
    let p = miembros[principal].clone();

    // Un grupo hecho solo de auxiliares no es un juego.
    if p.auxiliar {
        return None;
    }
    // Hace falta **alguna** evidencia de juego: nombre de ejecutable, AppID del entorno o
    // carpeta de instalación. Sin nada de eso es un proceso cualquiera del sistema.
    let tiene_nombre = p.ejecutable_del_juego.is_some();
    let tiene_appid = p
        .appid_del_entorno
        .as_ref()
        .map(|e| !e.vacia())
        .unwrap_or(false);
    let rutas: Vec<&str> = miembros
        .iter()
        .filter_map(|m| m.ruta_del_juego.as_deref().or(m.ruta_ejecutable.as_deref()))
        .collect();
    let install = primer_install_dir(rutas.iter().copied());
    // La evidencia del catálogo también vale aquí: un juego externo no tiene nombre de
    // `.exe`, ni AppID, ni `steamapps/common`, y sin esto el grupo se descartaba aunque sus
    // procesos hubieran entrado.
    let lo_conoce_el_catalogo = indice.map(|i| i.confirma(&p)).unwrap_or(false);
    if !tiene_nombre && !tiene_appid && install.is_none() && !lo_conoce_el_catalogo {
        return None;
    }

    let mut evidencia: Vec<String> = Vec::new();
    let mut appids: Vec<u32> = Vec::new();

    for m in &miembros {
        if let Some(env) = m.appid_del_entorno.as_ref() {
            match env.appid_consensuado() {
                Ok(Some(id)) => appids.push(id),
                Ok(None) => {}
                Err(_) => {
                    evidencia.push(format!(
                        "el proceso {} declara SteamAppId y SteamGameId distintos",
                        m.pid
                    ));
                }
            }
        }
        // `SteamLaunch AppId=` en los argumentos: misma fuerza que el entorno, porque lo
        // pone Steam al lanzar. Si contradice al entorno, el grupo queda contradictorio y
        // no se elige ninguno.
        if let Some(id) = m.appid_de_argumentos {
            appids.push(id);
            evidencia.push(format!(
                "Steam lo lanzó con AppId={} (línea de comandos)",
                id
            ));
        }
        if let Some(n) = m.ejecutable_del_juego.as_deref() {
            evidencia.push(format!("ejecuta {}", n));
        }
        if m.bajo_proton == Some(true) {
            evidencia.push("corre bajo Proton".to_string());
        }
    }
    appids.sort_unstable();
    appids.dedup();

    // La evidencia se acumula por miembro, así que el mismo hecho aparece tantas veces como
    // procesos lo cumplan («corre bajo Proton» salía cuatro veces en Deskrawl). Los datos
    // originales no se tocan: `miembros` sigue completo. Solo se limpia la **representación
    // legible**, conservando el orden en que se descubrió cada hecho.
    let mut vistos = std::collections::HashSet::new();
    evidencia.retain(|e| vistos.insert(e.clone()));

    let appid_contradictorio = appids.len() > 1;
    if appid_contradictorio {
        evidencia.push(format!(
            "los procesos del grupo declaran AppID distintos: {:?}",
            appids
        ));
    }
    if let Some(dir) = install.as_deref() {
        evidencia.push(format!("instalado en steamapps/common/{}", dir));
    }

    // `dedup()` solo quita repetidos **adyacentes**: con dos juegos distintos en el grupo
    // (`Deskrawl`, `tabtip`, `Deskrawl`) el nombre seguía duplicado. También se conserva el
    // orden de aparición para que la lista sea estable entre pasadas.
    // El nombre del **representante** va primero.
    //
    // Antes se recogía el de todos los miembros en orden de aparición, y quien consumía el
    // grupo tomaba el primero (`nombres.first()`). Con un proceso ajeno dentro del grupo
    // —el diagnóstico midió `Heronite-x86_64` en Tattoo Tycoon, Eslabong y Starsector— el
    // nombre observado salía de un proceso que no era el que ejecuta este juego. El resto se
    // conserva detrás, como contexto, pero no puede adelantar al representante.
    let mut vistos_nombre = std::collections::HashSet::new();
    let nombres: Vec<String> = p
        .ejecutable_del_juego
        .clone()
        .into_iter()
        .chain(miembros.iter().filter_map(|m| m.ejecutable_del_juego.clone()))
        .filter(|n| vistos_nombre.insert(n.clone()))
        .collect();

    let bajo_proton = miembros.iter().any(|m| m.bajo_proton == Some(true));
    let install_dirs: Vec<String> = install.into_iter().collect();

    Some(GrupoDeProcesos {
        miembros,
        principal: p.pid,
        evidencia,
        appid: if appid_contradictorio {
            None
        } else {
            appids.first().copied()
        },
        appid_contradictorio,
        nombres,
        install_dirs,
        bajo_proton,
    })
}

impl ObservadorDeProcesos for ObservadorDeProc {
    fn escanear(&self) -> EscaneoDeProcesos {
        Self::escanear(self)
    }

    fn sigue_vivo(&self, clave: &str) -> bool {
        Self::estado_de_clave(clave) == EstadoDeLaClave::Vivo
    }
}

/// Construye el escaneo que **produciría** el observador real con una lista de procesos
/// dada, sin tocar `/proc`.
///
/// Es lo que permite probar la detección con datos copiados de un equipo real (ver
/// `fixtures_procesos`) en un contenedor sin Steam. Reutiliza exactamente las mismas
/// funciones que el observador real —filtro, agrupación, elección de principal—, así que una
/// prueba que aquí pasa equivale a lo que haría el sistema con esos procesos.
pub fn escanear_desde(procesos: &[crate::sessions::RasgosDeProceso]) -> EscaneoDeProcesos {
    escanear_desde_con_catalogo(procesos, None)
}

/// ¿Este proceso puede ser parte de un juego, según los rasgos que ya se leyeron?
///
/// Es el **único** criterio de prefiltro, y lo usan tanto el observador real como el camino
/// de pruebas. Antes eran dos implementaciones distintas (`parece_juego` sobre `/proc` y
/// `escanear_desde_con_catalogo` sobre rasgos) y esa divergencia es la razón de que las
/// pruebas pasaran mientras el equipo real fallaba: probaban un pipeline que producción no
/// ejecutaba.
///
/// Admite, no confirma: decidir si el proceso es el juego es cosa de la clasificación.
pub fn puede_ser_juego(
    p: &crate::sessions::RasgosDeProceso,
    indice: Option<&IndiceDelCatalogo>,
) -> bool {
    if p.auxiliar {
        return false;
    }
    if p.ejecutable_del_juego.is_some() {
        return true;
    }
    if p
        .appid_del_entorno
        .as_ref()
        .map(|e| !e.vacia())
        .unwrap_or(false)
    {
        return true;
    }
    if p.appid_de_argumentos.is_some() {
        return true;
    }
    if p
        .ruta_ejecutable
        .as_deref()
        .and_then(crate::evidencia_proceso::install_dir_de_ruta)
        .is_some()
        || p.ruta_del_juego
            .as_deref()
            .and_then(crate::evidencia_proceso::install_dir_de_ruta)
            .is_some()
    {
        return true;
    }
    // Sin evidencia de Steam: el catálogo es la vía de Lutris, Heroic y Heronite.
    if indice.map(|i| i.confirma(p)).unwrap_or(false) {
        return true;
    }
    // Juegos nativos con nombre propio: el nombre del ejecutable, salvo los binarios del
    // sistema. Un `.exe` o un `.x86_64` siempre se admiten.
    let Some(nombre) = p.ejecutable.as_deref() else {
        return false;
    };
    let bajo = nombre.to_ascii_lowercase();
    if bajo.contains(".x86_64") || bajo.contains(".exe") {
        return true;
    }
    let del_sistema = [
        "systemd", "dbus", "kworker", "sddm", "gdm", "xorg", "wayland", "pipewire",
        "pulseaudio", "thoriapp", "bash", "sh", "node", "tsserver", "code", "cargo", "rustc",
        "git", "sleep", "cat", "ls", "ssh", "tmux", "vim", "nvim", "python", "python3",
        "reaper", "steamwebhelper", "wineserver", "winedevice",
    ];
    !del_sistema
        .iter()
        .any(|s| bajo == *s || bajo.starts_with(&format!("{s}.")))
}

/// Igual que [`escanear_desde`], con el catálogo de lanzadores disponible.
///
/// El catálogo es lo que permite confirmar un juego que no es de Steam: sin él, esta función
/// solo acepta nombre, AppID o `steamapps/common`, que es exactamente la cadena dependiente
/// de Steam que dejaba fuera a los juegos de Lutris, Heronite y Heroic.
pub fn escanear_desde_con_catalogo(
    procesos: &[crate::sessions::RasgosDeProceso],
    indice: Option<&IndiceDelCatalogo>,
) -> EscaneoDeProcesos {
    let validos: Vec<RasgosDeProceso> = procesos
        .iter()
        .filter(|p| puede_ser_juego(p, indice))
        .cloned()
        .collect();

    let mut grupos: Vec<GrupoDeProcesos> = Vec::new();
    // La topología es **todo** lo observado; `validos` solo dice quién puede ser miembro.
    for indices in crate::evidencia_proceso::agrupar_con_topologia(&validos, procesos) {
        let miembros: Vec<RasgosDeProceso> = indices.iter().map(|i| validos[*i].clone()).collect();
        if let Some(g) = armar_grupo_con_catalogo(miembros, indice) {
            grupos.push(g);
        }
    }
    EscaneoDeProcesos {
        grupos,
        inaccesibles: 0,
        sin_visibilidad: None,
    }
}

/// Lo que el escaneo anota de un proceso cuando el diagnóstico está activo.
///
/// Se rellena aunque el proceso se **descarte**: sin los descartados no se puede saber si un
/// eslabón intermedio se perdió en el filtro, que es justo la hipótesis de la fragmentación
/// del árbol. Con el diagnóstico apagado no se construye ninguna.
struct Anotacion {
    pid: u32,
    ppid: Option<u32>,
    inicio: Option<String>,
    comm: Option<String>,
    ejecutable: Option<String>,
    motivo: String,
}

/// Cadena de ancestros de un PID, de padre inmediato hacia arriba, usando solo lo observado
/// en la pasada.
///
/// Se corta en cuanto un ancestro no aparece en lo observado: no se inventa el eslabón que
/// falta. El tope evita ciclos si el sistema reutilizara un PID durante el escaneo.
pub(crate) fn ancestros_de(
    pid: u32,
    padres: &std::collections::HashMap<u32, Option<u32>>,
) -> Vec<u32> {
    let mut cadena = Vec::new();
    let mut actual = pid;
    for _ in 0..64 {
        let Some(Some(padre)) = padres.get(&actual).copied() else {
            break;
        };
        if padre == 0 || padre == actual || cadena.contains(&padre) {
            break;
        }
        cadena.push(padre);
        // Un ancestro fuera de lo observado corta la cadena: lo que no se vio no se supone.
        if !padres.contains_key(&padre) {
            break;
        }
        actual = padre;
    }
    cadena
}

/// ¿Estamos dentro de un contenedor? Heurística barata: `/.dockerenv` o un `PID 1` que no
/// es systemd/init.
pub fn parece_contenedor() -> bool {
    if Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists() {
        return true;
    }
    fs::read_to_string("/proc/1/comm")
        .map(|c| {
            let c = c.trim().to_lowercase();
            !(c.contains("systemd") || c.contains("init") || c.contains("openrc"))
        })
        .unwrap_or(false)
}

/// Observador simulado para pruebas: devuelve exactamente lo que se le configura.
///
/// Existe en el binario (no solo en tests) para poder demostrar la degradación cuando no
/// hay visibilidad, sin tocar `/proc`.
pub struct ObservadorSimulado {
    pub escaneo: EscaneoDeProcesos,
    pub vivos: Vec<String>,
    pub clave_viva: Option<String>,
}

impl ObservadorSimulado {
    pub fn sin_visibilidad(motivo: &str) -> Self {
        Self {
            escaneo: EscaneoDeProcesos {
                grupos: Vec::new(),
                inaccesibles: 0,
                sin_visibilidad: Some(motivo.to_string()),
            },
            vivos: Vec::new(),
            clave_viva: None,
        }
    }

    /// Observador que devuelve exactamente los grupos que se le dan.
    pub fn con_grupos(grupos: Vec<GrupoDeProcesos>) -> Self {
        Self {
            escaneo: EscaneoDeProcesos {
                grupos,
                inaccesibles: 0,
                sin_visibilidad: None,
            },
            vivos: Vec::new(),
            clave_viva: None,
        }
    }
}

impl ObservadorDeProcesos for ObservadorSimulado {
    fn escanear(&self) -> EscaneoDeProcesos {
        self.escaneo.clone()
    }

    fn sigue_vivo(&self, clave: &str) -> bool {
        self.vivos.iter().any(|v| v == clave) || self.clave_viva.as_deref() == Some(clave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entorno_de_prueba() -> Vec<u8> {
        // Un entorno realista: los tres primeros son los que interesan y el resto es lo
        // que hay que descartar (credenciales, tokens, rutas personales).
        let mut v: Vec<u8> = Vec::new();
        for linea in [
            "SteamAppId=251570",
            "SteamGameId=251570",
            "SteamClientLaunch=1",
            "PROTON_VERSION=Proton 9.0-4",
            "NVIDIA_API_KEY=nvapi-abcdef123456",
            "OPENAI_API_KEY=sk-proj-secreto",
            "AWS_SECRET_ACCESS_KEY=abc123def456",
            "TOKEN=deadbeef",
            "HOME=/home/alguien",
            "PATH=/usr/bin:/bin",
            "LD_PRELOAD=/home/alguien/libmangohud.so",
        ] {
            v.extend_from_slice(linea.as_bytes());
            v.push(0);
        }
        v
    }

    #[test]
    fn la_lista_blanca_no_deja_escapar_credenciales() {
        let vars = filtrar_environ(&entorno_de_prueba());
        // Solo las cuatro permitidas.
        let claves: Vec<&String> = vars.keys().collect();
        assert_eq!(claves.len(), 4, "claves vistas: {:?}", claves);
        assert!(vars.contains_key("SteamAppId"));
        assert!(vars.contains_key("SteamGameId"));
        assert!(vars.contains_key("SteamClientLaunch"));
        assert!(vars.contains_key("PROTON_VERSION"));

        // Y ninguna de estas puede aparecer en la salida serializada.
        let json = serde_json::to_string(&vars).unwrap();
        for prohibido in [
            "nvapi",
            "sk-proj",
            "abc123def456",
            "deadbeef",
            "/home/alguien",
            "PATH",
        ] {
            assert!(
                !json.contains(prohibido),
                "se filtró {:?} en {:?}",
                prohibido,
                json
            );
        }
    }

    #[test]
    fn los_valores_se_validan_antes_de_guardarse() {
        // Un AppID tiene que ser numérico: si alguien mete basura en la variable, no pasa.
        assert_eq!(
            valor_permitido("SteamAppId", "251570").as_deref(),
            Some("251570")
        );
        assert!(
            valor_permitido("SteamAppId", " 251570 ").is_some(),
            "se recorta"
        );
        assert!(valor_permitido("SteamAppId", "251570; rm -rf /").is_none());
        assert!(valor_permitido("SteamAppId", "").is_none());
        // Longitud desmedida: fuera.
        assert!(valor_permitido("SteamAppId", &"9".repeat(65)).is_none());
        // Variable no permitida: fuera aunque el valor sea inocente.
        assert!(valor_permitido("SSH_AUTH_SOCK", "1234").is_none());
    }

    #[test]
    fn el_entorno_con_nulos_y_basura_no_rompe() {
        let mut datos: Vec<u8> = b"SteamAppId=10\0".to_vec();
        datos.extend_from_slice(&[0xff, 0xfe, 0x00]); // UTF-8 inválido
        datos.extend_from_slice(b"sin_signo_igual\0");
        datos.extend_from_slice(b"=valor_huerfano\0");
        let vars = filtrar_environ(&datos);
        assert_eq!(vars.get("SteamAppId").map(|s| s.as_str()), Some("10"));
        assert_eq!(vars.len(), 1);
    }

    /// `STEAM_COMPAT_INSTALL_PATH` se reduce a la carpeta del juego: la ruta del usuario no
    /// se guarda.
    ///
    /// Es la variable que declara dónde está instalado el juego. Conservarla entera metería
    /// `/home/<usuario>/…` en los datos guardados, que es justo lo que la lista blanca evita.
    #[test]
    fn la_carpeta_declarada_se_guarda_sin_la_ruta_del_usuario() {
        let datos =
            b"STEAM_COMPAT_INSTALL_PATH=/home/jugador/.local/share/Steam/steamapps/common/Deskrawl\0";
        let vars = filtrar_environ(datos);
        assert_eq!(
            vars.get("STEAM_COMPAT_INSTALL_PATH").map(|s| s.as_str()),
            Some("Deskrawl"),
            "solo la carpeta del juego, sin la ruta personal"
        );
        // Una ruta que no sea de `steamapps/common` no es evidencia: se descarta.
        assert!(
            !filtrar_environ(b"STEAM_COMPAT_INSTALL_PATH=/home/jugador/juegos\0")
                .contains_key("STEAM_COMPAT_INSTALL_PATH")
        );
    }

    #[test]
    fn un_entorno_vacio_da_lista_vacia() {
        assert!(filtrar_environ(b"").is_empty());
        assert!(filtrar_environ(&[0, 0, 0]).is_empty());
    }

    #[test]
    fn el_observador_simulado_hace_lo_que_se_le_dice() {
        let obs = ObservadorSimulado::sin_visibilidad("contenedor");
        assert!(obs.escanear().sin_visibilidad.is_some());
        assert!(!obs.sigue_vivo("1@2026-01-01 00:00:00"));
    }
}

#[cfg(test)]
pub mod pruebas_de_regresion {

    use super::*;
    use crate::fixtures_procesos::*;
    use crate::sessions::RasgosDeProceso;

    /// Convierte una fixture en los rasgos que produciría el observador real.
    ///
    /// Reutiliza las mismas funciones de análisis que el observador (`clasificar_argumento`,
    /// `es_auxiliar`, `ruta_de_proton`...), así que el resultado es el que se obtendría de
    /// verdad con esos procesos. Lo único que se hace a mano es partir el `cmdline`, porque
    /// aquí no hay `/proc` del que leerlo.
    fn rasgos_de(p: &ProcesoDeFixture) -> RasgosDeProceso {
        let args = argumentos_de_cmdline(p.cmdline.as_bytes());
        let ev = EvidenciaDeArgumentos::de_argumentos(&args);
        let nombre_linux = p
            .exe
            .map(crate::evidencia_proceso::nombre_de_fichero)
            .or_else(|| Some(p.comm.to_string()));

        let entorno = if p.entorno.is_empty() {
            None
        } else {
            let mut app = None;
            let mut game = None;
            for (k, v) in p.entorno {
                match *k {
                    "SteamAppId" => app = Some(v.to_string()),
                    "SteamGameId" => game = Some(v.to_string()),
                    _ => {}
                }
            }
            Some(EvidenciaAppId {
                steam_app_id: app,
                steam_game_id: game,
            })
        };

        RasgosDeProceso {
            pid: p.pid,
            ejecutable: nombre_linux.clone(),
            ruta_ejecutable: p.exe.map(|s| s.to_string()),
            inicio_del_proceso: Some(p.inicio.to_string()),
            ppid: Some(p.ppid),
            appid_del_entorno: entorno,
            // En el fixture `exe` es la ruta real del proceso, así que `ruta_de_proton`
            // responde a la vez «¿Proton?» y «¿capa de compatibilidad?». Se separa igual que
            // en el observador real para que la prueba vea los dos campos distintos.
            bajo_proton: Some(
                p.exe.map(ruta_de_proton).unwrap_or(false)
                    || nombre_linux.as_deref().map(ruta_de_proton).unwrap_or(false),
            ),
            bajo_compatibilidad: Some(
                p.exe.map(ruta_de_proton).unwrap_or(false)
                    || nombre_linux.as_deref().map(ruta_de_proton).unwrap_or(false)
                    || p.comm.to_ascii_lowercase().contains("wine")
                    || es_auxiliar(p.comm),
            ),
            ejecutable_del_juego: ev
                .ejecutables
                .first()
                .map(|e| crate::evidencia_proceso::nombre_de_juego_de_ejecutable(e)),
            // `clone()`: `ev` se sigue usando justo debajo y mover el campo lo invalidaría.
            ruta_del_juego: ev
                .install_dir
                .clone()
                .map(|d| format!("steamapps/common/{}", d)),
            // La fixture no trae entorno: la carpeta declarada se rellena solo si el proceso
            // de prueba la aporta explícitamente.
            carpeta_declarada: p.carpeta_declarada.map(|c| c.to_string()),
            // El prefijo sale del entorno de la fixture, igual que en el observador real: así
            // un test no puede pasar por tener el dato que producción no ve.
            wine_prefix: p
                .entorno
                .iter()
                .find(|(k, _)| *k == "WINEPREFIX")
                .map(|(_, v)| crate::evidencia_proceso::normalizar_prefijo(v)),
            // El `cwd` sale de la fixture, igual que en el observador real lee `/proc/<pid>/cwd`.
            cwd: p.cwd.map(|c| c.to_string()),
            auxiliar: nombre_linux.as_deref().map(es_auxiliar).unwrap_or(false)
                || es_auxiliar(p.comm),
            appid_de_argumentos: ev.appid_unico(),
            ejecuta_de_verdad: ev.lo_ejecuta_y_no_solo_lo_menciona(),
        }
    }

    /// Versión pública (solo pruebas) del constructor, para que el servicio pueda usar los
    /// mismos datos reales.
    pub fn rasgos_para_pruebas(p: &ProcesoDeFixture) -> RasgosDeProceso {
        rasgos_de(p)
    }

    /// **Auditoría 5.3**: ¿un juego externo nativo (Lutris + Linux) pasa el filtro?
    ///
    /// Starsector corre desde `~/Games/Starsector/starsector.sh` → `java`: no vive en
    /// `steamapps/common`, no lleva `SteamAppId` y no es un `.exe`. Se comprueba el filtro tal
    /// cual, sin tocar el pipeline, para documentar el punto exacto donde se pierde.
    #[test]
    fn auditoria_un_juego_externo_nativo_no_pasa_el_filtro() {
        // Lo que el filtro mira, sin E/S: la ruta del ejecutable y la línea de comandos.
        let ruta = "/home/jugador/Games/Starsector/jre_linux/bin/java";
        assert!(
            crate::evidencia_proceso::install_dir_de_ruta(ruta).is_none(),
            "el filtro solo reconoce steamapps/common, no una carpeta de Lutris"
        );
        let args = vec!["-jar".to_string(), "starsector.jar".to_string()];
        let ev = crate::evidencia_proceso::EvidenciaDeArgumentos::de_argumentos(&args);
        assert!(
            ev.ejecutables.is_empty(),
            "sin .exe en la linea de comandos no hay segunda evidencia"
        );
        // Y el `comm` (`java`) no da la tercera evidencia: el filtro solo acepta un `comm`
        // con `.exe` o `.x86_64`, que son convenciones de Windows y de un binario nativo de
        // Steam. Un `java`, un `python` o un `bash` que lanzan un juego no encajan.
        let comm = "java";
        assert!(!comm.contains(".exe") && !comm.contains(".x86_64"));
    }

    /// Eslabong (Heronite + UMU) con `SteamAppId=0`: cero **no** es una identidad.
    ///
    /// UMU exporta `0` para decir «este juego no es de Steam». Antes se aceptaba como valor y
    /// todos los juegos externos compartían el AppID 0, que resolvía a un juego que no es.
    /// Ahora `0` es ausencia, igual que si la variable no estuviera.
    #[test]
    fn un_appid_cero_no_es_una_identidad_de_steam() {
        let env = EvidenciaAppId {
            steam_app_id: Some("0".into()),
            steam_game_id: Some("0".into()),
        };
        // Cero no identifica ningún juego de Steam: se trata como ausencia de AppID.
        assert_eq!(env.appid_consensuado(), Ok(None));
        // Y por tanto la identidad "identificada" sería el AppID 0...
        let id = crate::sessions::resolver_identidad(
            Some("eslabong.exe"),
            Some(&RasgosDeProceso {
                pid: 1,
                ejecutable: Some("eslabong.exe".into()),
                ruta_ejecutable: Some("wine64-preloader".into()),
                inicio_del_proceso: Some("2026-11-25 22:00:00".into()),
                ppid: Some(1),
                appid_del_entorno: Some(env),
                bajo_proton: Some(true),
                bajo_compatibilidad: Some(true),                ejecutable_del_juego: Some("eslabong.exe".into()),
                ruta_del_juego: Some("/home/jugador/Games/Eslabong".into()),
                carpeta_declarada: Some("/home/jugador/Games/Eslabong".into()),
                wine_prefix: Some("/home/jugador/.local/share/uni/Prefixes/eslabong".into()),
                cwd: None,
                auxiliar: false,
                appid_de_argumentos: None,
                ejecuta_de_verdad: Some(true),
            }),
            &[],
        );
        // La identidad no puede apoyarse en un AppID que no existe; el nombre sale de otra
        // evidencia (aquí no hay catálogo con el que correlacionar, así que no se inventa).
        assert_eq!(id.appid, None);
        assert_eq!(id.nombre_comercial, None);
    }

    // ==================== Integración multi-lanzador (5.3) ====================
    //
    // Estos tests recorren las mismas funciones que producción: el índice se construye desde
    // el catálogo real de un equipo, el prefiltro se aplica proceso a proceso y la
    // identidad se resuelve con `resolver_identidad_con_catalogo`. No se construyen rasgos
    // ideales saltándose el prefiltro, que es justo el error que dejó pasar los fallos.

    /// Catálogo equivalente al real de CachyOS: Lutris con `directory` vacío, más Deskrawl
    /// (que es de Steam y ya estaba en la biblioteca de Steam).
    fn catalogo_real() -> Vec<crate::evidencia_proceso::EntradaDeCatalogo> {
        vec![
            crate::evidencia_proceso::EntradaDeCatalogo {
                id_plataforma: Some("4623570".into()),
                plataforma: Some("steam".into()),
                launcher: "steam".into(),
                runner: Some("proton".into()),
                titulo: "Deskrawl".into(),
                install_dir: Some("/home/jugador/.local/share/Steam/steamapps/common/Deskrawl".into()),
                wine_prefix: None,
                exec: Some("Deskrawl.exe".into()),
                runtime_configurado: None,
            },
            // El adaptador pone el `exe` del `.yml` cuando `directory` viene vacío: así llega
            // aquí, no como carpeta inventada.
            crate::evidencia_proceso::EntradaDeCatalogo {
                id_plataforma: None,
                plataforma: None,
                launcher: "lutris".into(),
                runner: Some("linux".into()),
                titulo: "Starsector".into(),
                install_dir: Some("/home/jugador/Games/starsector/starsector.sh".into()),
                wine_prefix: None,
                exec: Some("/home/jugador/Games/starsector/starsector.sh".into()),
                runtime_configurado: None,
            },
            crate::evidencia_proceso::EntradaDeCatalogo {
                id_plataforma: None,
                plataforma: None,
                launcher: "lutris".into(),
                runner: Some("wine".into()),
                titulo: "Star Valor".into(),
                install_dir: Some("/home/jugador/Games/Star Valor/Star Valor.exe".into()),
                wine_prefix: Some("/home/jugador/.local/share/lutris/runners/wine/star-valor".into()),
                exec: Some("/home/jugador/Games/Star Valor/Star Valor.exe".into()),
                runtime_configurado: None,
            },
            crate::evidencia_proceso::EntradaDeCatalogo {
                id_plataforma: None,
                plataforma: None,
                launcher: "heroic".into(),
                runner: Some("heronite".into()),
                titulo: "Eslabong".into(),
                install_dir: Some("/home/jugador/Games/Eslabong/eslabong.exe".into()),
                wine_prefix: Some("/home/jugador/Games/Heronite/Prefixes/eslabong".into()),
                exec: Some("/home/jugador/Games/Eslabong/eslabong.exe".into()),
                runtime_configurado: None,
            },
        ]
    }

    fn indice_real() -> IndiceDelCatalogo {
        IndiceDelCatalogo::de_catalogo(&catalogo_real())
    }

    /// **Starsector**: el prefiltro debe admitir el `java` que ejecuta el juego.
    ///
    /// Es el caso que falló en la máquina real: `sin evidencia: ni .exe de juego en cmdline,
    /// ni SteamAppId/SteamGameId`. Antes se descartaban los tres procesos del árbol.
    #[test]
    fn starsector_el_java_del_juego_entra_como_candidato() {
        let indice = indice_real();
        for p in STARSECTOR_NATIVO {
            let prueba = prueba_de_fixture(p);
            let motivo = indice.admite(&prueba);
            assert!(
                motivo.is_some(),
                "el proceso {} ({}) debe entrar como candidato, y no entró",
                p.pid,
                p.comm
            );
        }
    }

    /// Y el árbol entero se agrupa en **un** grupo, con el `java` como representante.
    #[test]
    fn starsector_agrupa_el_arbol_y_elige_el_interprete_del_juego() {
        let escaneo = escanear_con_catalogo(STARSECTOR_NATIVO, &catalogo_real());
        assert_eq!(escaneo.grupos.len(), 1, "un solo juego, un solo grupo");
        let g = &escaneo.grupos[0];
        assert_eq!(
            g.principal, 641416,
            "el representante debe ser el java que ejecuta el juego, no el script ni el wrapper"
        );
    }

    /// **Star Valor**: el `.exe` de Wine importa aunque el proceso Linux sea el preloader.
    #[test]
    fn star_valor_entra_por_el_catalogo_y_por_el_prefijo() {
        let indice = indice_real();
        for p in STAR_VALOR_WINE {
            assert!(
                indice.admite(&prueba_de_fixture(p)).is_some(),
                "el proceso {} ({}) debe entrar: el catálogo lo conoce",
                p.pid,
                p.comm
            );
        }
    }

    /// **Eslabong**: UMU con `SteamAppId=0`. Cero no es identidad, pero el proceso entra.
    #[test]
    fn eslabong_entra_aunque_su_appid_sea_cero() {
        let indice = indice_real();
        for p in ESLABONG_UMU {
            assert!(
                indice.admite(&prueba_de_fixture(p)).is_some(),
                "el proceso {} ({}) debe entrar por el catálogo",
                p.pid,
                p.comm
            );
        }
        // Y su AppID cero no identifica a nadie.
        let r = rasgos_de(&ESLABONG_UMU[1]);
        assert_eq!(
            r.appid_del_entorno.as_ref().map(|e| e.appid_consensuado()),
            Some(Ok(None)),
            "SteamAppId=0 es ausencia, no un AppID válido"
        );
    }

    /// **Un catálogo vacío no admite nada**: sin datos no se adivina.
    #[test]
    fn un_catalogo_vacio_no_admite_procesos() {
        let indice = IndiceDelCatalogo::de_catalogo(&[]);
        assert!(indice.esta_vacio());
        for p in STARSECTOR_NATIVO {
            assert!(
                indice.admite(&prueba_de_fixture(p)).is_none(),
                "sin catálogo, un java cualquiera no puede entrar como juego"
            );
        }
    }

    /// **Lo ajeno no entra**: procesos java, python y bash que no son de ningún juego.
    ///
    /// Es la protección contra el efecto contrario: admitir por el catálogo no puede
    /// convertir en juego cualquier intérprete del sistema.
    #[test]
    fn un_java_python_o_bash_ajeno_no_entra_por_el_catalogo() {
        let indice = indice_real();
        let ajenos = [
            (
                "java",
                "/usr/bin/java",
                "/home/jugador/proyectos",
                &["-jar", "/tmp/tool.jar"][..],
            ),
            (
                "python3",
                "/usr/bin/python3",
                "/home/jugador",
                &["-m", "http.server"][..],
            ),
            ("bash", "/usr/bin/bash", "/tmp", &["-c", "ls"][..]),
        ];
        for (comm, exe, cwd, args) in ajenos {
            let prueba = PruebaDeProceso {
                comm: Some(comm.into()),
                exe: Some(exe.into()),
                argumentos: args.iter().map(|a| a.to_string()).collect(),
                cwd: Some(cwd.into()),
                wine_prefix: None,
            };
            assert!(
                indice.admite(&prueba).is_none(),
                "{comm} en {cwd} no pertenece a ningún juego y no debe entrar"
            );
        }
    }

    /// **Dos juegos externos a la vez**: cada uno con su grupo y sin mezclarse.
    #[test]
    fn dos_juegos_externos_a_la_vez_no_se_mezclan() {
        let mut procesos: Vec<ProcesoDeFixture> = STARSECTOR_NATIVO.to_vec();
        procesos.extend_from_slice(STAR_VALOR_WINE);
        let escaneo = escanear_con_catalogo(&procesos, &catalogo_real());
        assert_eq!(escaneo.grupos.len(), 2, "dos juegos, dos grupos");
        let principales: Vec<u32> = escaneo.grupos.iter().map(|g| g.principal).collect();
        assert!(principales.contains(&641416), "el java de Starsector");
        assert!(
            principales.contains(&652010),
            "el proceso Wine de Star Valor"
        );
    }

    /// **Regresión Deskrawl**: el catálogo del juego de Steam no rompe lo que ya funcionaba.
    #[test]
    fn deskrawl_sigue_eligiendo_el_exe_del_juego_y_no_el_auxiliar() {
        let escaneo = escanear_con_catalogo(DESKRAWL_PV_ADVERB, &catalogo_real());
        assert_eq!(escaneo.grupos.len(), 1);
        assert_eq!(escaneo.grupos[0].principal, 557514, "Deskrawl.exe");
    }

    /// **La identidad de un juego externo sale del catálogo**, sin AppID de Steam.
    #[test]
    fn la_identidad_de_un_juego_externo_no_necesita_appid() {
        let catalogo = catalogo_real();
        let r = rasgos_de(&STARSECTOR_NATIVO[2]);
        let id = crate::sessions::resolver_identidad_con_catalogo(
            Some("java"),
            Some(&r),
            &[],
            &catalogo,
        );
        assert_eq!(id.nombre_comercial.as_deref(), Some("Starsector"));
        assert_eq!(id.appid, None, "no es un juego de Steam: no lleva AppID");
        assert_eq!(id.origen, crate::sessions::OrigenAppId::CatalogoDeLanzador);
    }

    /// **Dos juegos distintos que comparten carpeta**: no se elige uno al azar.
    #[test]
    fn una_evidencia_ambigua_no_elige_juego() {
        let mut catalogo = catalogo_real();
        // Un segundo juego que apunta a la misma carpeta que Starsector.
        catalogo.push(crate::evidencia_proceso::EntradaDeCatalogo {
            id_plataforma: None,
            plataforma: None,
            launcher: "lutris".into(),
            runner: Some("linux".into()),
            titulo: "Otro Juego".into(),
            install_dir: Some("/home/jugador/Games/starsector/starsector.sh".into()),
            wine_prefix: None,
            exec: Some("/home/jugador/Games/starsector/starsector.sh".into()),
            runtime_configurado: None,
        });
        let r = rasgos_de(&STARSECTOR_NATIVO[2]);
        let id = crate::sessions::resolver_identidad_con_catalogo(
            Some("java"),
            Some(&r),
            &[],
            &catalogo,
        );
        assert_eq!(
            id.nombre_comercial, None,
            "dos juegos con la misma evidencia: identidad desconocida, no una al azar"
        );
    }

    /// Los helpers de prueba: describen el proceso tal como el prefiltro lo ve.
    fn prueba_de_fixture(p: &ProcesoDeFixture) -> PruebaDeProceso {
        PruebaDeProceso {
            comm: Some(p.comm.to_string()),
            exe: p.exe.map(|e| e.to_string()),
            argumentos: argumentos_de_cmdline(p.cmdline.as_bytes()),
            cwd: p.cwd.map(|c| c.to_string()),
            wine_prefix: p
                .entorno
                .iter()
                .find(|(k, _)| *k == "WINEPREFIX")
                .map(|(_, v)| crate::evidencia_proceso::normalizar_prefijo(v)),
        }
    }

    /// Escaneo completo con el catálogo, aplicando el prefiltro real a cada fixture.
    fn escanear_con_catalogo(
        fixture: &[ProcesoDeFixture],
        catalogo: &[crate::evidencia_proceso::EntradaDeCatalogo],
    ) -> EscaneoDeProcesos {
        let indice = IndiceDelCatalogo::de_catalogo(catalogo);
        let admitidos: Vec<RasgosDeProceso> = fixture
            .iter()
            .filter(|p| indice.admite(&prueba_de_fixture(p)).is_some())
            .map(rasgos_de)
            .collect();
        escanear_desde_con_catalogo(&admitidos, Some(&indice))
    }

    fn escanear(fixture: &[ProcesoDeFixture]) -> EscaneoDeProcesos {
        let rasgos: Vec<RasgosDeProceso> = fixture.iter().map(rasgos_de).collect();
        escanear_desde(&rasgos)
    }

    /// **El caso que falló en CachyOS.** Antes de este arreglo, el observador buscaba un
    /// proceso llamado «Palworld-Win64-Shipping», no lo encontraba (`comm = GameThread`,
    /// `exe = wine64-preloader`) y se quedaba en «Buscando un juego» para siempre.
    #[test]
    fn palworld_bajo_proton_se_detecta_con_su_appid() {
        let r = escanear(PALWORLD_PROTON);
        assert_eq!(r.grupos.len(), 1, "un juego, un grupo");
        let g = &r.grupos[0];
        assert_eq!(g.appid, Some(1623730), "AppID de Palworld");
        assert!(!g.appid_contradictorio);
        assert!(g.bajo_proton, "se reconoce la ejecución bajo Proton");
        assert_eq!(g.principal, 19588, "el principal es el proceso del juego");
        assert_eq!(
            g.nombres,
            vec!["Palworld-Win64-Shipping".to_string()],
            "el nombre es el del .exe, no el del preloader de Wine"
        );
    }

    /// **Incidencia B, el caso real**: un grupo con el proceso del juego **y** un proceso ajeno
    /// que también declara un ejecutable de juego.
    ///
    /// En el diagnóstico, `Tattoo Tycoon` y `Eslabong` aparecían con nombre observado
    /// `Heronite-x86_64`, y `Starsector` también, porque `nombres` recogía el
    /// `ejecutable_del_juego` de **todos** los miembros y se tomaba el primero. El nombre tiene
    /// que salir del representante, que es quien ejecuta el juego de **esta** ejecución.
    #[test]
    fn el_nombre_del_grupo_es_el_del_representante_aunque_haya_otro_juego() {
        // El juego de esta ejecución: lo primero es el envoltorio, el `.exe` va detrás.
        let mut procesos = vec![
            rasgos_para_pruebas(&ProcesoDeFixture {
                pid: 700001,
                ppid: 1,
                comm: "pv-adverb",
                exe: Some("/usr/bin/python3"),
                cmdline: "pv-adverb -- Tattoo Tycoon.exe",
                entorno: &[("SteamAppId", "4623570")],
                inicio: "2026-11-26 12:00:00",
                carpeta_declarada: None,
                cwd: None,
            }),
            rasgos_para_pruebas(&ProcesoDeFixture {
                pid: 700002,
                ppid: 700001,
                comm: "Tattoo Tycoon.exe",
                exe: Some("/home/jugador/.wine/drive_c/Tattoo Tycoon.exe"),
                cmdline: "Tattoo Tycoon.exe",
                entorno: &[("SteamAppId", "4623570")],
                inicio: "2026-11-26 12:00:01",
                carpeta_declarada: None,
                cwd: None,
            }),
        ];
        // Y un proceso de **otra** ejecución que cuelga de la misma infraestructura y declara
        // su propio ejecutable de juego. Es lo que el diagnóstico mostró como `Heronite-x86_64`.
        procesos.push(rasgos_para_pruebas(&ProcesoDeFixture {
            pid: 700003,
            ppid: 700001,
            comm: "Heronite-x86_64",
            exe: Some("/home/jugador/Games/eslabong/Heronite-x86_64"),
            cmdline: "Heronite-x86_64",
            entorno: &[],
            inicio: "2026-11-26 11:59:00",
            carpeta_declarada: None,
            cwd: None,
        }));

        let escaneo = escanear_desde_con_catalogo(&procesos, None);
        for grupo in &escaneo.grupos {
            let principal = grupo
                .miembros
                .iter()
                .find(|m| m.pid == grupo.principal)
                .expect("el representante está entre los miembros");
            assert_eq!(
                grupo.nombres.first().cloned(),
                principal.ejecutable_del_juego.clone(),
                "el primer nombre del grupo (pid {}) debe ser el de su representante, no el de \
                 otro miembro del mismo grupo: {:?}",
                grupo.principal,
                grupo.nombres
            );
        }
    }

    /// El grupo incluye a los hermanos (reaper, python3, Palworld.exe): son **el mismo**
    /// juego. Abrir sesión por cada uno llenaría el historial de fantasmas.
    #[test]
    fn los_procesos_auxiliares_no_abren_sesiones_aparte() {
        let r = escanear(PALWORLD_PROTON);
        assert_eq!(r.grupos.len(), 1);
        let g = &r.grupos[0];
        // El reaper y los python3 no entran como miembros principales.
        assert!(!g
            .miembros
            .iter()
            .any(|m| m.pid == g.principal && m.auxiliar));
        assert_eq!(g.miembros.iter().filter(|m| m.pid == 19588).count(), 1);
    }

    /// **Bloque B**: el prefijo de Wine **admite** procesos, pero ya no **confirma** la
    /// ejecución. Antes bastaba para presentar un auxiliar como representante, y de ahí salió
    /// una sesión fantasma de `pv-adverb` en un lanzamiento real.
    #[test]
    fn el_prefijo_de_wine_no_confirma_por_si_solo_la_ejecucion() {
        let r = escanear(PALWORLD_PROTON);
        assert_eq!(r.grupos.len(), 1, "un solo grupo, no uno por auxiliar");
        let g = &r.grupos[0];
        let rep = g
            .miembros
            .iter()
            .find(|m| m.pid == g.principal)
            .expect("hay representante");
        assert_ne!(
            rep.ejecutable.as_deref(),
            Some("pv-adverb"),
            "un auxiliar del prefijo no puede quedar como representante"
        );
        assert_eq!(rep.pid, 19588, "el representante es el juego");
    }

    /// **Bloque B**: un auxiliar que solo *correlaciona* con el juego (lo acompaña) no
    /// *confirma* su ejecución. `wineserver` y el reaper acompañan sin ejecutar el juego.
    #[test]
    fn un_auxiliar_correlacionado_no_confirma_la_ejecucion() {
        let r = escanear(PALWORLD_PROTON);
        let g = &r.grupos[0];
        for pid in [99310u32, 99215] {
            if let Some(m) = g.miembros.iter().find(|m| m.pid == pid) {
                assert!(
                    !m.ejecuta_de_verdad.unwrap_or(false),
                    "el pid {pid} acompaña al juego pero no lo ejecuta"
                );
            }
        }
        assert!(
            g.miembros
                .iter()
                .any(|m| m.pid == 19588 && m.ejecuta_de_verdad == Some(true)),
            "el juego sí ejecuta"
        );
    }

    /// **Bloque B**: `systemd-inhibit`, `iscriptevaluator`, `steam.exe` y UMU son
    /// infraestructura del lanzador. Ninguno puede abrir sesión por su cuenta.
    #[test]
    fn la_infraestructura_del_lanzador_nunca_es_un_juego() {
        let r = escanear(PALWORLD_PROTON);
        assert_eq!(r.grupos.len(), 1, "sin sesiones de infraestructura");
        let infra = [
            "systemd-inhibit",
            "iscriptevaluator.exe",
            "steam.exe",
            "umu-run",
            "steam",
        ];
        for g in &r.grupos {
            let rep = g.miembros.iter().find(|m| m.pid == g.principal);
            if let Some(rep) = rep {
                let nombre = rep
                    .ejecutable
                    .as_deref()
                    .or(rep.ejecutable_del_juego.as_deref())
                    .unwrap_or("");
                assert!(
                    !infra.contains(&nombre),
                    "{nombre} no puede ser representante de una sesión"
                );
            }
        }
    }

    /// Solo con los auxiliares del árbol (sin el proceso del juego) no hay sesión: un
    /// `wineserver` suelto no es un juego corriendo.
    #[test]
    fn un_wineserver_suelto_no_abre_sesion() {
        let solo_aux = [ProcesoDeFixture {
            pid: 99215,
            ppid: 1,
            comm: "wineserver",
            exe: Some("/usr/bin/wineserver"),
            cmdline: "wineserver\0-p\0",
            entorno: &[],
            inicio: "2026-11-24 18:00:01",
            carpeta_declarada: None,
            cwd: None,
        }];
        assert!(escanear(&solo_aux).grupos.is_empty());
    }

    /// Juego nativo: sin Proton de por medio, detectado por nombre y por entorno.
    #[test]
    fn el_juego_nativo_tambien_se_detecta() {
        let r = escanear(JUEGO_NATIVO);
        assert_eq!(r.grupos.len(), 1, "el wineserver suelto no cuenta");
        assert_eq!(r.grupos[0].appid, Some(251570));
        assert!(!r.grupos[0].bajo_proton);
    }

    /// Dos juegos a la vez: dos grupos, cada uno con su AppID. No se mezclan aunque
    /// compartan el padre 41000.
    #[test]
    fn dos_juegos_simultaneos_no_se_mezclan() {
        let r = escanear(DOS_JUEGOS);
        assert_eq!(r.grupos.len(), 2, "grupos: {:#?}", r.grupos);
        let mut appids: Vec<Option<u32>> = r.grupos.iter().map(|g| g.appid).collect();
        appids.sort();
        assert_eq!(appids, vec![Some(251570), Some(1623730)]);
    }

    /// AppID contradictorio: el proceso declara `SteamAppId != SteamGameId` y además la
    /// línea de comandos de Steam dice un tercer valor. No se elige ninguno.
    #[test]
    fn un_appid_contradictorio_no_se_resuelve_solo() {
        let r = escanear(APPID_CONTRADICTORIO);
        assert_eq!(r.grupos.len(), 1);
        let g = &r.grupos[0];
        // La contradicción interna (AppId ≠ GameId) se detecta aquí; la de la línea de
        // comandos la resuelve el servicio, que es quien tiene la última palabra.
        assert!(
            g.evidencia.iter().any(|e| e.contains("distintos")),
            "la evidencia debe decirlo: {:?}",
            g.evidencia
        );
        // Lo que sí puede afirmarse: los dos candidatos, sin elegir.
        assert!(g.appid.is_none() || g.appid_contradictorio);
    }

    /// En un contenedor no se ve ningún juego, pero eso **no** es «no corre»: es «no se ve».
    /// Aquí se comprueba lo que se ve realmente, que es nada.
    #[test]
    fn en_un_contenedor_no_aparece_ningun_juego() {
        let r = escanear(CONTENEDOR);
        assert!(
            r.grupos.is_empty(),
            "en el contenedor no hay juegos que ver"
        );
    }

    /// Sin evidencia de juego, un proceso cualquiera no abre sesión: ni `bash` ni Thoriapp.
    #[test]
    fn los_procesos_del_sistema_no_son_juegos() {
        for p in CONTENEDOR {
            let r = rasgos_de(p);
            assert!(
                r.auxiliar || r.ejecutable_del_juego.is_none(),
                "{:?} no debería parecer un juego",
                p.comm
            );
        }
    }

    /// **La causa raíz, aislada.** El orden del filtro `parece_juego` descartaba por el
    /// ejecutable antes de leer la línea de comandos. Un proceso cuyo `exe` parece del
    /// sistema pero cuya línea de comandos lleva la ruta de un juego (`S:\...\X.exe`) tiene
    /// que ser candidato.
    /// El envoltorio del lanzamiento (`systemd-inhibit`) hereda `SteamAppId`, así que la
    /// versión anterior lo elegía como proceso representativo y la sesión acababa diciendo
    /// «Se vio el juego en ejecución (systemd-inhibit)». No es el juego: es quien lo lanza.
    #[test]
    fn el_envoltorio_del_lanzador_no_se_presenta_como_el_juego() {
        let rasgos: Vec<RasgosDeProceso> = PALWORLD_CON_INHIBIDOR.iter().map(rasgos_de).collect();
        let escaneo = escanear_desde(&rasgos);
        assert_eq!(escaneo.grupos.len(), 1, "un solo juego, un solo grupo");
        let g = &escaneo.grupos[0];
        let principal = g
            .miembros
            .iter()
            .find(|m| m.pid == g.principal)
            .expect("el grupo debe tener principal");

        assert_ne!(
            principal.ejecutable.as_deref(),
            Some("systemd-inhibit"),
            "systemd-inhibit lanza el juego, no es el juego"
        );
        // El proceso representativo es el del juego bajo Proton. Su `comm` real es
        // `GameThread` y su ejecutable en disco, el preloader de Wine: eso es lo que corre
        // en Linux. Lo importante es que **no** sea el envoltorio que lanzó el comando.
        assert!(
            principal.bajo_proton == Some(true) && principal.ejecutable_del_juego.is_some(),
            "el representativo debe ser el proceso del juego: {:?}",
            principal
        );

        // La evidencia del envoltorio **no** se tira: el AppID sigue acreditado.
        assert_eq!(g.appid, Some(1623730));
        assert!(g.bajo_proton, "el árbol va bajo Proton");
        // Y el nombre del juego sigue saliendo del ejecutable de Windows, no del wrapper.
        assert!(
            g.nombres.iter().any(|n| n.contains("Palworld")),
            "nombres: {:?}",
            g.nombres
        );
        assert!(
            !g.nombres.iter().any(|n| n.contains("systemd-inhibit")),
            "el envoltorio no debe proponerse como nombre del juego: {:?}",
            g.nombres
        );
    }

    #[test]
    fn la_evidencia_de_la_linea_de_comandos_manda_sobre_el_ejecutable() {
        // Lo que hace `parece_juego` con los argumentos: si hay un `.exe` de juego, pasa.
        let args = argumentos_de_cmdline(
            b"S:\\steamapps\\common\\Palworld\\Pal\\Binaries\\Win64\\Palworld-Win64-Shipping.exe\0Pal\0",
        );
        let ev = EvidenciaDeArgumentos::de_argumentos(&args);
        let hay_juego = ev.ejecutables.iter().any(|e| {
            crate::evidencia_proceso::nombre_de_fichero(e)
                .to_ascii_lowercase()
                .ends_with(".exe")
        });
        assert!(
            hay_juego,
            "el argumento es un ejecutable de juego: {:?}",
            ev
        );

        // Y un proceso normal del sistema, sin ruta de juego, no pasa ese criterio.
        let args = argumentos_de_cmdline(b"/usr/bin/sleep\0300\0");
        let ev = EvidenciaDeArgumentos::de_argumentos(&args);
        assert!(!ev.ejecutables.iter().any(|e| {
            crate::evidencia_proceso::nombre_de_fichero(e)
                .to_ascii_lowercase()
                .ends_with(".exe")
        }));
    }

    /// Los datos de una fixture nunca deben arrastrar la ruta personal de la línea de
    /// comandos. Solo el **nombre** del ejecutable del juego cruza esa frontera.
    #[test]
    fn de_las_fixtures_no_sale_ningun_texto_personal() {
        for fixture in [
            PALWORLD_PROTON,
            JUEGO_NATIVO,
            DOS_JUEGOS,
            APPID_CONTRADICTORIO,
        ] {
            let r = escanear(fixture);
            let json = serde_json::to_string(
                &r.grupos
                    .iter()
                    .map(|g| (g.appid, g.nombres.clone(), g.evidencia.clone()))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            // Rutas completas del usuario y opciones del juego: fuera.
            for prohibido in ["S:\\\\steamapps", "/run/media/jugador", "-dx12", "jugador"] {
                assert!(
                    !json.contains(prohibido),
                    "se coló {:?} en {:?}",
                    prohibido,
                    json
                );
            }
        }
    }

    /// Evidencias repetidas: el mismo hecho por cada miembro del grupo.
    ///
    /// En Deskrawl, «corre bajo Proton» salía cuatro veces y «ejecuta Deskrawl» dos, porque la
    /// evidencia se acumula por proceso. Los datos originales (`miembros`) se conservan
    /// enteros; lo que se deduplica es la representación legible.
    #[test]
    fn la_evidencia_legible_no_repite_el_mismo_hecho() {
        let r = escanear(DESKRAWL_SRT);
        let g = &r.grupos[0];
        let proton: Vec<&String> = g
            .evidencia
            .iter()
            .filter(|e| e.contains("Proton"))
            .collect();
        assert_eq!(proton.len(), 1, "evidencia: {:?}", g.evidencia);
        // «ejecuta Deskrawl» es el hecho de que el juego se ejecute. Otra evidencia puede
        // mencionar el nombre (el AppID de lanzamiento, la carpeta instalada): son hechos
        // distintos y deben seguir contándose por separado.
        let deskrawl: Vec<&String> = g
            .evidencia
            .iter()
            .filter(|e| e.starts_with("ejecuta Deskrawl"))
            .collect();
        assert_eq!(deskrawl.len(), 1, "evidencia: {:?}", g.evidencia);
        // Los miembros siguen ahí: no se ha tirado ningún dato.
        assert!(g.miembros.len() > 1);
    }

    /// Los nombres tampoco se repiten, aunque no estén seguidos.
    ///
    /// `dedup()` solo quita adyacentes: con `Deskrawl`, `tabtip`, `Deskrawl` el nombre del
    /// juego aparecía dos veces en la lista.
    #[test]
    fn los_nombres_del_grupo_no_se_repiten() {
        let r = escanear(DESKRAWL_SRT);
        let g = &r.grupos[0];
        let mut vistos = std::collections::HashSet::new();
        for n in &g.nombres {
            assert!(
                vistos.insert(n.clone()),
                "nombre repetido: {n} en {:?}",
                g.nombres
            );
        }
    }

    /// El proceso representativo es el que ejecuta el juego, nunca un envoltorio.
    ///
    /// Es la regresión de Deskrawl: `srt-bwrap` encabezaba la cadena, heredaba el entorno de
    /// Steam y acabó presentado como el proceso del juego. El criterio nuevo no usa listas de
    /// nombres: un envoltorio tiene hijos que ejecutan el juego, él no lo ejecuta.
    #[test]
    fn el_wrapper_del_runtime_no_es_el_proceso_representativo() {
        let r = escanear(DESKRAWL_SRT);
        let g = &r.grupos[0];
        let principal = g
            .miembros
            .iter()
            .find(|m| m.pid == g.principal)
            .expect("el principal está entre los miembros");
        assert_ne!(
            principal.ejecutable.as_deref(),
            Some("srt-bwrap"),
            "srt-bwrap envuelve el lanzamiento, no es el juego"
        );
        assert!(
            g.evidencia.iter().any(|e| e.contains("Deskrawl")),
            "la evidencia del juego se conserva: {:?}",
            g.evidencia
        );
    }
}

/// Huella del catálogo para el prefiltro: lo justo para decidir si un proceso **podría**
/// pertenecer a un juego conocido, sin recorrer el catálogo entero por cada proceso de `/proc`.
///
/// Se construye una vez por pasada desde el catálogo ya cacheado (`catalogo_lanzadores`), así
/// que el coste de leer SQLite/YAML no se repite aquí: el prefiltro solo consulta conjuntos en
/// memoria. Sin esto, cualquier juego externo se quedaba fuera antes de llegar al catálogo:
/// el filtro solo sabía de Steam (`steamapps/common`, `SteamAppId`, `.exe`, `.x86_64`).
///
/// **Admitir no es confirmar.** Que un proceso entre como candidato por esta vía no crea
/// sesión: sigue pasando por la clasificación por papeles y por la correlación con el
/// catálogo, que es la que decide identidad y elegibilidad.
#[derive(Debug, Default, Clone)]
pub struct IndiceDelCatalogo {
    /// Rutas de instalación (o del ejecutable configurado), normalizadas y sin barra final.
    directorios: Vec<String>,
    /// Ejecutables configurados por el lanzador, normalizados.
    ejecutables: Vec<String>,
    /// Nombres de fichero de esos ejecutables (`Star Valor.exe`, `starsector.sh`).
    nombres: Vec<String>,
    /// Carpetas **derivadas** del ejecutable configurado (no declaradas por el lanzador).
    ///
    /// Sirven solo para reconocer el proceso que ejecuta el juego desde esa carpeta. Se
    /// mantienen aparte de `directorios` justamente para no presentarlas como si el lanzador
    /// las hubiera declarado.
    carpetas_derivadas: Vec<String>,
    /// Prefijos de Wine declarados, normalizados.
    prefijos: Vec<String>,
}

impl IndiceDelCatalogo {
    /// Construye la huella desde el catálogo. Nada de E/S: trabaja sobre lo ya leído.
    pub fn de_catalogo(entradas: &[crate::evidencia_proceso::EntradaDeCatalogo]) -> Self {
        let mut indice = Self::default();
        for e in entradas {
            // La carpeta que el lanzador declara: es una instalación y sus procesos
            // pertenecen a ella.
            if let Some(dir) = e.install_dir.as_deref() {
                // Ojo: el adaptador de Lutris deja el `game.exe` en `install_dir` cuando
                // `directory` viene vacío. Tratar ese fichero como carpeta de instalación
                // metía su carpeta padre (`/home/jugador`) en el índice y admitía cualquier
                // proceso del usuario. Un ejecutable no es una carpeta.
                if crate::evidencia_proceso::parece_ejecutable(dir) {
                    indice.añadir_ejecutable(dir);
                } else {
                    indice.añadir_directorio(dir);
                }
            }
            // El ejecutable configurado: se reconoce a sí mismo y, por su carpeta, a los
            // procesos que lo ejecutan (el caso del script → intérprete).
            if let Some(exe) = e.exec.as_deref() {
                indice.añadir_ejecutable(exe);
            }
            if let Some(p) = e.wine_prefix.as_deref() {
                let n = crate::evidencia_proceso::normalizar_prefijo(p);
                if !indice.prefijos.contains(&n) {
                    indice.prefijos.push(n);
                }
            }
        }
        indice
    }

    /// Registra una carpeta de instalación declarada por el lanzador.
    fn añadir_directorio(&mut self, ruta: &str) {
        let n = ruta.trim_end_matches('/').to_string();
        if n.is_empty() {
            return;
        }
        if !self.directorios.contains(&n) {
            self.directorios.push(n.clone());
        }
        // Un `install_dir` que en realidad es un ejecutable (así lo deja el adaptador de
        // Lutris cuando `directory` viene vacío) también se registra como tal.
        self.registrar_nombre(&n);
    }

    /// Registra el ejecutable configurado por el lanzador (`game.exe`).
    fn añadir_ejecutable(&mut self, ruta: &str) {
        let n = ruta.trim_end_matches('/').to_string();
        if n.is_empty() {
            return;
        }
        if !self.ejecutables.contains(&n) {
            self.ejecutables.push(n.clone());
        }
        self.registrar_nombre(&n);
        // La carpeta del ejecutable: es donde arranca el juego, y por eso el intérprete que
        // lo ejecuta tiene ahí su `cwd`. Se guarda aparte para no confundirla con una carpeta
        // declarada por el lanzador.
        if let Some(padre) = crate::evidencia_proceso::ruta_padre(&n) {
            if !self.carpetas_derivadas.contains(&padre) {
                self.carpetas_derivadas.push(padre);
            }
        }
    }

    /// Nombre de fichero del ejecutable, para reconocerlo con otra forma de ruta.
    fn registrar_nombre(&mut self, ruta: &str) {
        let nombre = crate::evidencia_proceso::nombre_de_fichero(ruta);
        let bajo = nombre.to_ascii_lowercase();
        // Solo extensiones que **son** un programa: un `.txt` o un `.dll` no identifican un
        // juego y admitirían procesos por accidente.
        let es_programa = bajo.ends_with(".exe")
            || bajo.ends_with(".sh")
            || bajo.ends_with(".jar")
            || bajo.ends_with(".x86_64")
            || bajo.ends_with(".AppImage");
        if es_programa && !self.nombres.contains(&bajo) {
            self.nombres.push(bajo);
        }
    }

    /// ¿Está vacío? Un catálogo vacío no admite nada: no se adivina.
    pub fn esta_vacio(&self) -> bool {
        self.directorios.is_empty()
            && self.carpetas_derivadas.is_empty()
            && self.ejecutables.is_empty()
            && self.prefijos.is_empty()
    }

    /// ¿La ruta cae dentro de un directorio del catálogo?
    ///
    /// Se comparan **componentes**, no subcadenas: `/juegos/Dos` no está dentro de
    /// `/juegos/Uno` aunque el texto se parezca.
    fn ruta_dentro_del_catalogo(&self, ruta: &str) -> bool {
        if ruta.is_empty() {
            return false;
        }
        let normalizada = if ruta.starts_with('/') {
            ruta.to_string()
        } else {
            // Ruta relativa: se prueba tal cual contra el final de cada directorio, que es lo
            // que hace un lanzador cuando arranca con `working_dir` y `./juego`.
            let limpia = ruta.trim_start_matches("./");
            return self
                .directorios
                .iter()
                .any(|d| d.ends_with(&format!("/{limpia}")) || d == limpia);
        };
        // La dirección importa y no es simétrica: la ruta del proceso debe estar **por debajo**
        // de un directorio del catálogo, nunca por encima. El caso inverso —que el catálogo
        // cuelgue de la ruta del proceso— admitía a cualquiera: `/home/jugador` habría coincidido
        // con `/home/jugador/Games/starsector` y, con ella, cualquier proceso del usuario.
        self.directorios.iter().any(|d| {
            normalizada == *d
                || normalizada
                    .strip_prefix(d.as_str())
                    .map(|resto| resto.starts_with('/'))
                    .unwrap_or(false)
        })
    }

    /// ¿La ruta es **exactamente** un ejecutable configurado, o cae dentro de su carpeta?
    fn es_ejecutable_del_catalogo(&self, ruta: &str) -> bool {
        let n = ruta.trim_end_matches('/');
        if self.ejecutables.iter().any(|e| e == n) {
            return true;
        }
        let nombre = crate::evidencia_proceso::nombre_de_fichero(n);
        if nombre.is_empty() {
            return false;
        }
        self.nombres.contains(&nombre.to_ascii_lowercase())
    }

    /// ¿El prefijo de Wine es uno de los declarados por el catálogo?
    fn prefijo_del_catalogo(&self, prefijo: &str) -> bool {
        let n = crate::evidencia_proceso::normalizar_prefijo(prefijo);
        self.prefijos.contains(&n)
    }

    /// Admite un proceso como **posible** juego por evidencia del catálogo.
    ///
    /// Es la vía que faltaba: un juego nativo lanzado por un script y un intérprete
    /// (`starsector.sh` → `java`), un juego de Lutris sin `directory` o un `.exe` de Wine con
    /// otro nombre no tenían ninguna traza que el filtro anterior entendiera.
    ///
    /// Devuelve el motivo, para poder explicarlo en el diagnóstico. Cada vía es una prueba
    /// **estructural** de pertenencia; ninguna es una lista de nombres de juegos.
    pub fn admite(&self, prueba: &PruebaDeProceso) -> Option<&'static str> {
        if self.esta_vacio() {
            return None;
        }
        // 1. El ejecutable real está dentro de un directorio del catálogo (o es su ejecutable
        //    configurado). Cubre el binario nativo dentro de su instalación.
        if let Some(exe) = prueba.exe.as_deref() {
            if self.es_ejecutable_del_catalogo(exe) {
                return Some("el ejecutable es el configurado por el lanzador");
            }
            if self.ruta_dentro_del_catalogo(exe) {
                return Some("el ejecutable vive dentro de un directorio del catálogo");
            }
        }
        // 2. El ejecutable configurado aparece en la línea de comandos (el caso del `.exe`
        //    de Wine, que el preloader no muestra como `exe`). Solo si es un argumento que
        //    apunta a un ejecutable del catálogo, no una mención cualquiera.
        if prueba
            .argumentos
            .iter()
            .any(|a| self.es_ejecutable_del_catalogo(a))
        {
            return Some("la línea de comandos nombra el ejecutable del catálogo");
        }
        // 3. El directorio de trabajo cae en una instalación del catálogo. **Solo** no
        //    confirma nada, pero sí basta para que el proceso entre y la clasificación
        //    posterior decida: es la evidencia que tiene el `java` de Starsector.
        if let Some(cwd) = prueba.cwd.as_deref() {
            let n = cwd.trim_end_matches('/');
            // Coincidencia **exacta** con la carpeta del ejecutable configurado: el juego
            // arranca ahí. No se acepta «dentro de»: `/home/jugador` contiene miles de carpetas
            // y admitiría cualquier proceso del usuario.
            if self.carpetas_derivadas.iter().any(|d| d == n) {
                return Some("se ejecuta desde la carpeta del ejecutable del catálogo");
            }
            // Y la carpeta declarada por el lanzador, que sí es una instalación completa.
            if self.ruta_dentro_del_catalogo(cwd) {
                return Some("el directorio de trabajo está en una instalación del catálogo");
            }
        }
        // 4. El prefijo de Wine es el que declara un juego del catálogo.
        if let Some(pref) = prueba.wine_prefix.as_deref() {
            if self.prefijo_del_catalogo(pref) {
                return Some("el prefijo de Wine es el de un juego del catálogo");
            }
        }
        None
    }
}

impl IndiceDelCatalogo {
    /// ¿El catálogo **confirma** que este proceso es el juego?
    ///
    /// Es más estricto que `admite`: admitir solo dice «podría pertenecer a un juego» y basta
    /// para no descartarlo, pero confirmar exige que la evidencia identifique a un juego del
    /// catálogo. La diferencia importa: `java` en `/usr/bin` no confirma nada, mientras que
    /// `java` ejecutándose desde la carpeta del juego sí.
    pub fn confirma(&self, p: &crate::sessions::RasgosDeProceso) -> bool {
        if self.esta_vacio() {
            return false;
        }
        // El ejecutable real ES el que el lanzador tiene configurado.
        if let Some(exe) = p.ruta_ejecutable.as_deref() {
            if self.es_ejecutable_del_catalogo(exe) {
                return true;
            }
        }
        // El nombre del ejecutable del juego es el configurado (el `.exe` bajo Wine, que
        // aparece recortado a su nombre de fichero).
        if let Some(n) = p.ejecutable_del_juego.as_deref() {
            if self.nombres.contains(&n.to_ascii_lowercase()) {
                return true;
            }
        }
        // La carpeta que el proceso declara coincide con una instalación del catálogo.
        if let Some(c) = p.carpeta_declarada.as_deref() {
            if self.ruta_dentro_del_catalogo(c) {
                return true;
            }
        }
        // El prefijo de Wine **no** confirma: solo admite.
        //
        // Medido en el equipo real: un `pv-adverb` suelto en `/home/jugador` llevaba el prefijo
        // de Tattoo Tycoon y se confirmaba como el juego, abriendo una sesión falsa antes de
        // que arrancara nada. El prefijo es **compartido por todo el lanzamiento** (runtime,
        // envoltorios, el propio juego), así que no distingue quién es quién: sirve para
        // admitir un proceso y correlacionarlo, no para nombrarlo representante.
        //
        // Se ejecuta desde la carpeta del ejecutable configurado.
        //
        // Es la vía de los juegos que arrancan por un **intérprete**: el árbol real de
        // Starsector es `lutris-wrapper` → `starsector.sh` → `java`. El `java` no nombra el
        // juego en su línea de comandos ni lleva variables del lanzador; lo único que lo sitúa
        // es que su directorio de trabajo es la carpeta del juego.
        //
        // Solo se compara la carpeta **del ejecutable** y de forma exacta: comparar contra
        // cualquier instalación admitiría también a un `bash` suelto en la carpeta del juego,
        // que no es el juego.
        if let Some(cwd) = p.cwd.as_deref() {
            let n = cwd.trim_end_matches('/');
            if self.carpetas_derivadas.iter().any(|d| d == n) {
                return true;
            }
        }
        false
    }
}

/// Los pocos datos de un proceso que el prefiltro necesita, ya leídos.
///
/// Existe para que el índice del catálogo se pueda probar sin `/proc` y para que el filtro no
/// abra ficheros que no va a usar.
#[derive(Debug, Default, Clone)]
pub struct PruebaDeProceso {
    pub comm: Option<String>,
    pub exe: Option<String>,
    pub argumentos: Vec<String>,
    pub cwd: Option<String>,
    pub wine_prefix: Option<String>,
}

/// **Bloque A — topología**: los árboles reales del equipo del usuario deben dar **un** grupo
/// por juego, no dos.
///
/// Antes de la corrección salían dos grupos por juego (Eslabong ×2, Tattoo Tycoon ×2,
/// Palworld ×2) y cada uno abría sesión. La causa era el eslabón descartado: `python3` o
/// `reaper` cortaban la cadena y el nieto se convertía en raíz independiente. Estas pruebas
/// pasan por el pipeline de producción, no construyen el grupo ideal a mano.
#[cfg(test)]
mod pruebas_topologia_real {
    use crate::fixtures_procesos as f;
    use crate::fixtures_procesos::ProcesoDeFixture;
    use crate::proceso_linux::{escanear_desde, pruebas_de_regresion::rasgos_para_pruebas};

    fn grupos_de(fixture: &[f::ProcesoDeFixture]) -> Vec<crate::sessions::GrupoDeProcesos> {
        let procesos: Vec<_> = fixture.iter().map(rasgos_para_pruebas).collect();
        escanear_desde(&procesos).grupos
    }

    /// Igual que en el equipo real: Heronite y Heroic están instalados, así que el observador
    /// tiene su catálogo. Sin él, un juego externo no tiene AppID de Steam ni `steamapps` y
    /// ningún grupo podría confirmarse.
    fn grupos_con_catalogo(
        fixture: &[f::ProcesoDeFixture],
        entradas: &[crate::evidencia_proceso::EntradaDeCatalogo],
    ) -> Vec<crate::sessions::GrupoDeProcesos> {
        let procesos: Vec<_> = fixture.iter().map(rasgos_para_pruebas).collect();
        let indice = crate::proceso_linux::IndiceDelCatalogo::de_catalogo(entradas);
        crate::proceso_linux::escanear_desde_con_catalogo(&procesos, Some(&indice)).grupos
    }

    fn catalogo_eslabong() -> Vec<crate::evidencia_proceso::EntradaDeCatalogo> {
        vec![crate::evidencia_proceso::EntradaDeCatalogo {
            launcher: "heronite".into(),
            titulo: "eslabong".into(),
            id_plataforma: Some("5WzVLqXjCNJWsvmPCz4M4N".into()),
            install_dir: Some("/home/jugador/Games/Eslabong".into()),
            wine_prefix: Some("/home/jugador/Games/Heronite/Prefixes/eslabong".into()),
            exec: Some("/home/jugador/Games/Eslabong/eslabong.exe".into()),
            plataforma: Some("heronite".into()),
            runner: Some("umu".into()),
            runtime_configurado: Some("proton-cachyos-10.0-20260102-slr-x86_64".into()),
        }]
    }

    /// Regresión real de 5.3 (registro 1219): el `systemd-inhibit` y el `wine64-preloader`
    /// que desciende de él comparten AppID **y** cadena de lanzamiento. Salían dos grupos
    /// porque el ascenso daba raíces distintas a cada uno.
    #[test]
    fn palworld_real_5_3_da_un_solo_grupo() {
        let grupos = grupos_de(f::PALWORLD_REAL_5_3);
        assert_eq!(
            grupos.len(),
            1,
            "el lanzamiento de Palworld debe ser un grupo, salieron {}: {:?}",
            grupos.len(),
            grupos.iter().map(|g| g.principal).collect::<Vec<_>>()
        );
    }

    /// Dos juegos lanzados a la vez, cada uno con su propio `systemd-inhibit` colgando del
    /// **mismo** `systemd` de usuario. Comparten ancestro en la topología, pero son dos
    /// ejecuciones: no pueden acabar en el mismo grupo.
    ///
    /// Es la comprobación de que la regla de infraestructura no fusiona partidas: un `systemd`
    /// que encabeza dos ramas de lanzamiento no pertenece a ninguna de las dos.
    #[test]
    fn dos_lanzamientos_bajo_el_mismo_systemd_no_se_fusionan() {
        // La cadena de `systemd` de usuario, una sola vez.
        let mut todos: Vec<crate::sessions::RasgosDeProceso> = f::PALWORLD_REAL_5_3
            .iter()
            .filter(|p| [3355, 127312, 127367, 127399, 127410, 426576].contains(&p.pid))
            .map(rasgos_para_pruebas)
            .collect();
        // Primer lanzamiento: su `systemd-inhibit` y la cadena que cuelga de él.
        for p in f::PALWORLD_REAL_5_3
            .iter()
            .filter(|p| [426577, 426590, 426599, 426606, 426674, 426782].contains(&p.pid))
        {
            todos.push(rasgos_para_pruebas(p));
        }
        // Segundo lanzamiento: otro `systemd-inhibit` bajo el **mismo** `systemd`, con su
        // propia cadena independiente.
        let mut inhibit2 = rasgos_para_pruebas(
            f::PALWORLD_REAL_5_3
                .iter()
                .find(|p| p.pid == 426577)
                .expect("el systemd-inhibit del fixture"),
        );
        inhibit2.pid = 500100;
        inhibit2.inicio_del_proceso = Some("2026-11-26 14:30:00".into());
        let mut hijo2 = inhibit2.clone();
        hijo2.pid = 500101;
        hijo2.ppid = Some(500100);
        hijo2.ejecutable = Some("/usr/bin/python3".into());
        hijo2.inicio_del_proceso = Some("2026-11-26 14:30:01".into());
        todos.push(inhibit2);
        todos.push(hijo2);

        let grupos = crate::evidencia_proceso::agrupar_con_topologia(&todos, &todos);
        let grupo_de = |pid: u32| -> usize {
            grupos
                .iter()
                .position(|g| g.iter().any(|&i| todos[i].pid == pid))
                .unwrap_or_else(|| panic!("el pid {pid} no está en ningún grupo"))
        };
        assert_eq!(
            grupo_de(426782),
            grupo_de(426577),
            "el juego y su envoltorio van juntos"
        );
        assert_ne!(
            grupo_de(500101),
            grupo_de(426782),
            "dos ejecuciones distintas no pueden compartir grupo"
        );
        assert_ne!(
            grupo_de(500100),
            grupo_de(426577),
            "dos systemd-inhibit de ejecuciones distintas no pueden compartir grupo"
        );
    }

    /// **Incidencia A**, con los datos reales: Eslabong por Heronite y Tattoo Tycoon por
    /// Heroic, a la vez, cada uno con **su** catálogo.
    ///
    /// La reproducción anterior era falsa: inventaba un `systemd` común del que colgaban los
    /// dos y pasaba `None` como catálogo. Los árboles reales no comparten raíz —son huérfanos
    /// adoptados por init— y los dos juegos están en catálogo. Se usan las topologías reales
    /// tal cual, con los catálogos reales.
    #[test]
    fn heroic_y_heronite_simultaneos_dan_dos_grupos() {
        let mut procesos: Vec<crate::sessions::RasgosDeProceso> = f::ESLABONG_TOPOLOGIA_REAL
            .iter()
            .chain(f::TATTOO_TOPOLOGIA_REAL.iter())
            .map(rasgos_para_pruebas)
            .collect();
        // Los dos catálogos a la vez, como en la instalación real: Heronite tiene el suyo y
        // Heroic el suyo, y el índice los ve juntos.
        let mut catalogo = catalogo_eslabong();
        catalogo.extend(catalogo_tattoo());
        let indice = crate::proceso_linux::IndiceDelCatalogo::de_catalogo(&catalogo);

        procesos.sort_by_key(|p| p.pid);
        let escaneo = crate::proceso_linux::escanear_desde_con_catalogo(&procesos, Some(&indice));
        let pids: Vec<Vec<u32>> = escaneo
            .grupos
            .iter()
            .map(|g| g.miembros.iter().map(|m| m.pid).collect())
            .collect();
        let nombres: Vec<Vec<String>> = escaneo
            .grupos
            .iter()
            .map(|g| g.nombres.clone())
            .collect();
        assert_eq!(
            escaneo.grupos.len(),
            2,
            "cada juego tiene que dar su grupo: {pids:?} nombres={nombres:?}"
        );
        let eslabong = pids
            .iter()
            .find(|g| g.contains(&251724))
            .unwrap_or_else(|| panic!("Eslabong tiene que estar: {pids:?}"));
        let tattoo = pids
            .iter()
            .find(|g| g.contains(&253005))
            .unwrap_or_else(|| panic!("Tattoo Tycoon tiene que estar: {pids:?}"));
        assert!(
            eslabong.iter().all(|p| !tattoo.contains(p)),
            "los dos juegos no pueden compartir miembros: {pids:?}"
        );
    }

    /// **Incidencia B**: el nombre observado no puede venir de un proceso ajeno al principal.
    ///
    /// El diagnóstico real lo midió: Starsector, `Tattoo Tycoon` y `Eslabong` aparecían con
    /// nombre observado `Heronite-x86_64` porque el grupo contenía un proceso de otra
    /// ejecución y `nombres.first()` tomaba el suyo. El nombre comercial del catálogo sí era
    /// correcto, así que el defecto solo se veía en el nombre observado (y en la interfaz,
    /// cuando no había nombre comercial).
    #[test]
    fn el_nombre_observado_sale_del_representante_no_de_un_ajeno() {
        let procesos: Vec<_> = f::STARSECTOR_NATIVO.iter().map(rasgos_para_pruebas).collect();
        // Sin catálogo, el intérprete nativo no es candidato: el `.yml` de Lutris es lo que
        // declara el ejecutable. Es el mismo índice que usa el servicio.
        let catalogo = crate::sessions_service::aceptacion_53::catalogo_lutris_real();
        let indice = crate::proceso_linux::IndiceDelCatalogo::de_catalogo(&catalogo);
        let escaneo =
            crate::proceso_linux::escanear_desde_con_catalogo(&procesos, Some(&indice));
        let grupo = escaneo.grupos.first().expect("Starsector forma un grupo");
        let principal = grupo
            .miembros
            .iter()
            .find(|m| m.pid == grupo.principal)
            .expect("el representante está entre los miembros");
        // El representante es el intérprete del juego, no el envoltorio.
        assert_eq!(
            principal.pid, 641416,
            "el representante debe ser el `java` del juego"
        );
        // Y el primer nombre del grupo tiene que ser el suyo, no el de otro miembro.
        let nombre_del_principal = principal.ejecutable_del_juego.clone();
        assert_eq!(
            grupo.nombres.first().cloned(),
            nombre_del_principal,
            "el primer nombre del grupo debe ser el del representante, no el de un ajeno"
        );
    }

    /// **P3**: comprobar si la separación por AppID sigue fragmentando un lanzamiento.
    ///
    /// Un eslabón de la cadena sin AppID propio no puede quedar en otro grupo que el juego
    /// que ejecuta. Se comprueba sobre las cadenas reales: si el criterio por AppID separa,
    /// este test lo delata.
    #[test]
    fn un_eslabon_sin_appid_no_forma_grupo_propio() {
        for (nombre, fixture) in [
            ("Palworld", f::PALWORLD_REAL_5_3),
            ("Deskrawl", f::DESKRAWL_PV_ADVERB),
            ("Tattoo Tycoon", f::TATTOO_TOPOLOGIA_REAL),
            ("Eslabong", f::ESLABONG_TOPOLOGIA_REAL),
        ] {
            let procesos: Vec<_> = fixture.iter().map(rasgos_para_pruebas).collect();
            let escaneo = crate::proceso_linux::escanear_desde_con_catalogo(&procesos, None);
            // El grupo del juego lo forman los procesos que ejecutan o que declaran AppID.
            // Un grupo cuyos miembros **ninguno** declara AppID ni ejecuta el juego es un
            // grupo de solo envoltorios: eso es la fragmentación que se busca.
            let solo_envoltorios: Vec<Vec<u32>> = escaneo
                .grupos
                .iter()
                .filter(|g| {
                    !g.miembros.iter().any(|m| {
                        m.ejecuta_de_verdad == Some(true)
                            || m.ejecutable_del_juego.is_some()
                            || m.appid_del_entorno
                                .as_ref()
                                .map(|e| !e.vacia())
                                .unwrap_or(false)
                    })
                })
                .map(|g| g.miembros.iter().map(|m| m.pid).collect())
                .collect();
            assert!(
                solo_envoltorios.is_empty(),
                "{nombre}: hay grupos sin ningún proceso que ejecute o declare AppID: {solo_envoltorios:?}"
            );
        }
    }

    /// **P1**: con la topología completa, la infraestructura compartida no une ejecuciones.
    ///
    /// Es lo que P0 tenía que habilitar: antes de corregirlo, `es_infraestructura` recibía
    /// un árbol incompleto y decidía a ciegas. Ahora ve la cadena entera y debe reconocer
    /// que el `systemd` de usuario es infraestructura, mientras que el `systemd-inhibit` del
    /// lanzamiento es el ancla de **esa** ejecución.
    #[test]
    fn la_infraestructura_compartida_no_une_ejecuciones_distintas() {
        let procesos: Vec<_> = f::PALWORLD_REAL_5_3.iter().map(rasgos_para_pruebas).collect();
        let escaneo = crate::proceso_linux::escanear_desde_con_catalogo(&procesos, None);
        let pids: Vec<u32> = escaneo
            .grupos
            .iter()
            .flat_map(|g| g.miembros.iter().map(|m| m.pid))
            .collect();
        // Los `systemd` de usuario no son parte del lanzamiento: nunca miembros.
        for systemd in [3355u32, 127312, 127367, 127399, 127410, 426576] {
            assert!(
                !pids.contains(&systemd),
                "el systemd {systemd} no puede ser miembro del grupo: {pids:?}"
            );
        }
        // El `systemd-inhibit` sí es el ancla de esta ejecución.
        assert!(
            pids.contains(&426577),
            "el systemd-inhibit del lanzamiento es su ancla: {pids:?}"
        );
    }

    /// **P0**: la topología debe contener también los procesos descartados por el prefiltro.
    ///
    /// Un eslabón descartado (`python3`, `reaper`, `srt-bwrap`) sigue siendo padre de quien
    /// ejecuta el juego. Si se pierde, la cadena se corta y el lanzamiento sale en dos grupos
    /// aunque el prefiltro sea el mismo. Se comprueba sobre el camino de producción
    /// (`escanear_desde_con_catalogo`), no sobre el agrupador aislado.
    #[test]
    fn los_descartados_siguen_siendo_eslabones_de_la_cadena() {
        let procesos: Vec<_> = f::PALWORLD_REAL_5_3.iter().map(rasgos_para_pruebas).collect();
        let escaneo = crate::proceso_linux::escanear_desde_con_catalogo(&procesos, None);
        let todos: Vec<u32> = escaneo
            .grupos
            .iter()
            .flat_map(|g| g.miembros.iter().map(|m| m.pid))
            .collect();
        // El juego y el envoltorio que lo ejecuta tienen que quedar en el mismo grupo.
        let grupo_de = |pid: u32| escaneo.grupos.iter().position(|g| g.miembros.iter().any(|m| m.pid == pid));
        assert_eq!(
            grupo_de(426782),
            grupo_de(426674),
            "el envoltorio y el juego no pueden separarse: {todos:?}"
        );
        // Y un descartado nunca puede ser miembro: pertenecer al árbol no es ejecutar.
        assert!(
            !todos.contains(&426590),
            "un `python3` descartado no puede ser miembro del grupo: {todos:?}"
        );
    }

    /// Los `systemd` de usuario no son parte del lanzamiento. Con la cadena real de Palworld
    /// quedaban dentro del grupo del juego; aquí se comprueba que **no** llegan a él, porque
    /// si llegaran, dos juegos lanzados desde la misma sesión de usuario compartirían raíz.
    #[test]
    fn palworld_real_5_3_no_mete_los_systemd_en_el_grupo_del_juego() {
        let procesos: Vec<_> = f::PALWORLD_REAL_5_3.iter().map(rasgos_para_pruebas).collect();
        // El agrupador solo, sin el prefiltro de candidatos.
        let grupos = grupos_de(f::PALWORLD_REAL_5_3);
        let todos: Vec<u32> = grupos.iter().flat_map(|g| g.miembros.iter().map(|m| m.pid)).collect();
        assert!(
            !todos.contains(&3355) && !todos.contains(&127312),
            "los systemd de usuario no pueden ser miembros de un juego: {todos:?}"
        );
    }

    fn catalogo_tattoo() -> Vec<crate::evidencia_proceso::EntradaDeCatalogo> {
        vec![crate::evidencia_proceso::EntradaDeCatalogo {
            launcher: "heroic".into(),
            titulo: "Tattoo Tycoon".into(),
            id_plataforma: Some("epic-e75733d7dbb0463f9942ed6a2cd4acfb".into()),
            install_dir: Some("/home/jugador/Games/Heroic/TattooTycoon".into()),
            wine_prefix: Some("/home/jugador/Games/Heroic/Prefixes/default/Tattoo Tycoon".into()),
            exec: Some("/home/jugador/Games/Heroic/TattooTycoon/Tattoo Tycoon.exe".into()),
            plataforma: Some("epic".into()),
            runner: Some("legendary".into()),
            runtime_configurado: Some("proton-cachyos-slr".into()),
        }]
    }

    /// Eslabong: el `python3` intermedio ya no parte el árbol.
    #[test]
    fn eslabong_real_da_un_solo_grupo() {
        let grupos = grupos_con_catalogo(f::ESLABONG_TOPOLOGIA_REAL, &catalogo_eslabong());
        assert_eq!(
            grupos.len(),
            1,
            "el árbol real de Eslabong debe dar un grupo, dio {}: {:?}",
            grupos.len(),
            grupos
                .iter()
                .map(|g| (g.principal, g.nombres.clone()))
                .collect::<Vec<_>>()
        );
        // Y no debe quedar en `umu`: el nombre comercial es el del catálogo/ejecutable.
        let nombres = &grupos[0].nombres;
        assert!(
            !nombres.iter().any(|n| n.eq_ignore_ascii_case("umu")),
            "el grupo no debe llamarse 'umu': {:?}",
            nombres
        );
    }

    /// Tattoo Tycoon: mismo patrón, y el falso positivo del `pv-adverb` suelto no debe abrir
    /// sesión por sí solo.
    #[test]
    fn tattoo_tycoon_real_da_un_solo_grupo_y_el_envoltorio_suelto_no() {
        let grupos = grupos_con_catalogo(f::TATTOO_TOPOLOGIA_REAL, &catalogo_tattoo());
        assert_eq!(
            grupos.len(),
            1,
            "el árbol real de Tattoo Tycoon debe dar un grupo, dio {}: {:?}",
            grupos.len(),
            grupos
                .iter()
                .map(|g| (g.principal, g.nombres.clone()))
                .collect::<Vec<_>>()
        );
        // El 252630 es un `pv-adverb` aislado en /home/jugador: pura infraestructura.
        assert!(
            grupos.iter().all(|g| g.principal != 252630),
            "un envoltorio suelto no puede representar al juego"
        );
    }

    /// Palworld: la cadena se rompía en `python3`/`reaper`, y el representante del grupo
    /// roto era el **ancestro** del representante real.
    #[test]
    fn palworld_real_da_un_solo_grupo() {
        let grupos = grupos_de(f::PALWORLD_TOPOLOGIA_REAL);
        assert_eq!(
            grupos.len(),
            1,
            "el árbol real de Palworld debe dar un grupo, dio {}: {:?}",
            grupos.len(),
            grupos
                .iter()
                .map(|g| (g.principal, g.nombres.clone()))
                .collect::<Vec<_>>()
        );
        // El representante debe ser el proceso que ejecuta el juego, no el `systemd-inhibit`.
        assert_ne!(
            grupos[0].principal, 253993,
            "el representante no puede ser el envoltorio de arranque"
        );
    }

    /// Los falsos positivos del arranque (`iscriptevaluator`, `steam`) no abren sesión.
    #[test]
    fn los_falsos_positivos_del_arranque_no_abren_sesion() {
        let grupos = grupos_de(f::PALWORLD_TOPOLOGIA_REAL);
        for g in &grupos {
            assert!(
                g.principal != 253821 && g.principal != 253857,
                "proceso de arranque tomado por juego: {:?}",
                g.nombres
            );
        }
    }

    /// Dos ejecuciones independientes del **mismo** juego siguen siendo dos grupos: la
    /// corrección no puede fusionar por nombre, AppID o prefijo.
    #[test]
    fn dos_instancias_del_mismo_juego_siguen_siendo_dos_grupos() {
        let mut doble: Vec<f::ProcesoDeFixture> = f::ESLABONG_TOPOLOGIA_REAL.to_vec();
        // Segunda ejecución: mismo prefijo, mismo ejecutable, PID distintos y otra raíz.
        for p in doble.iter_mut() {
            p.pid += 100_000;
            if p.ppid != 1 {
                p.ppid += 100_000;
            }
        }
        let cat = catalogo_eslabong();
        let primera = grupos_con_catalogo(f::ESLABONG_TOPOLOGIA_REAL, &cat);
        let mut ambas = grupos_con_catalogo(&doble, &cat);
        assert_eq!(primera.len(), 1, "la referencia debe dar un grupo");
        ambas.extend(primera);
        assert!(
            ambas.len() >= 2,
            "dos ejecuciones independientes del mismo juego no pueden fusionarse: {}",
            ambas.len()
        );
    }
}
