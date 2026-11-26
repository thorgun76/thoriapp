//! Lectura de la evidencia que trae un proceso, sin tocar el sistema.
//!
//! Todo lo de aquí son funciones puras sobre textos: argumentos de línea de comandos, rutas
//! de Windows, nombres de proceso de Proton. Se puede probar con datos reales copiados de
//! un equipo con Steam, sin Steam y sin `/proc`.
//!
//! # Por qué existe este módulo
//!
//! La primera versión de la subfase 5.3 buscaba «el proceso cuyo nombre sea el del juego».
//! Un juego nativo cumple eso. Un juego bajo Proton no: lo que hay en `/proc` es un
//! preloader de Wine con `comm = GameThread` y una ruta dentro de
//! `compatibilitytools.d`. El nombre del juego solo aparece en los argumentos. Aquel
//! criterio descartaba cualquier juego de Proton, que son casi todos.
//!
//! La detección ahora se construye reuniendo evidencias y puntuándolas; ninguna prueba
//! aislada basta.

use crate::sessions::RasgosDeProceso;

/// Nombres de proceso que son infraestructura de Wine/Proton, nunca el juego.
///
/// Si uno de estos abriera sesión, cada partida generaría varias sesiones fantasma. Se
/// comparan en minúsculas y por igualdad del nombre sin extensiones.
const AUXILIARES: &[&str] = &[
    "wineserver",
    "winedevice",
    "wineboot",
    "winemenubuilder",
    "winecfg",
    "plugplay",
    "svchost",
    "rpcss",
    "services",
    "explorer",
    "conhost",
    "dxvk-setup",
    "gameoverlayui",
    "steamwebhelper",
    "steam",
    "reaper",
    "python3",
    "python",
    "sh",
    "bash",
    "pressure-vessel",
    // Instaladores y configuradores: un `.exe` bajo Wine, pero no un juego.
    "setup",
    "install",
    "installer",
    "unins000",
    "uninstall",
    "uninst",
    "dxsetup",
    "vcredist_x86",
    "vcredist_x64",
    "ue4prereqsetup_x64",
    "crashreportclient",
    // Manejadores de fallos que los motores instalan **dentro** de la carpeta del juego. Sin
    // esta lista pasaban por juego en cuanto la carpeta dejó de ser requisito, y al colgar
    // del proceso real lo convertían en `Envoltorio`.
    "unitycrashhandler64",
    "unitycrashhandler32",
    "crashpad_handler",
    "fmodstudio",
    // Infraestructura del sistema Windows dentro del prefijo. `tabtip.exe` es el teclado
    // táctil: vive en `C:\windows\system32`, lleva la misma carpeta declarada y el mismo
    // `SteamAppId` que el juego porque Proton se los pone a todo el contenedor.
    //
    // Esta lista es **protección complementaria**, no el mecanismo. Lo que decide es la
    // regla estructural de `repartir_papeles`: un auxiliar nunca tiene hijos que ejecutan
    // el juego, y el juego real siempre gana la comparación por evidencia.
    "tabtip",
    "xalia",
    "mshta",
    "rundll32",
    "regsvr32",
];

/// Términos que delatan un lanzamiento mediante Proton/Wine en la ruta de Linux.
const MARCAS_PROTON: &[&str] = &[
    "proton",
    "wine",
    "compatibilitytools",
    "pressure-vessel",
    "steamlinuxruntime",
];

/// Un `.exe` de Windows en un argumento delata un juego que corre bajo Wine.
const EXTENSIONES_JUEGO: &[&str] = &[".exe", ".x86_64", ".x86", ".AppImage", ".sh", ".msi"];

/// Extensiones que en Windows son ejecutables del sistema o instaladores, no juegos.
const EXTENSIONES_NO_JUEGO: &[&str] = &[
    ".dll", ".so", ".log", ".txt", ".ini", ".cfg", ".json", ".xml", ".bat", ".cache",
];

// ==================== CMDLINE ====================

/// Argumentos de un `cmdline` de `/proc` (separados por NUL), ya en `String`.
///
/// Si un argumento no es UTF-8 válido se descarta **ese** argumento, no la lista: perder la
/// evidencia entera por un byte raro es peor que perder un argumento.
pub fn argumentos_de_cmdline(datos: &[u8]) -> Vec<String> {
    datos
        .split(|b| *b == 0)
        .filter(|t| !t.is_empty())
        .filter_map(|t| std::str::from_utf8(t).ok())
        .map(|t| t.to_string())
        .collect()
}

/// Forma de un argumento, para poder guardarlo o descartarlo sin ambigüedad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentoRelevante {
    /// Un `.exe` (u otro ejecutable) de Windows. Se guarda el nombre del fichero.
    EjecutableDeJuego(String),
    /// Un ejemplar de `AppId=NNNN`.
    AppId(u32),
    /// `SteamLaunch` / `waitforexitandrun` / `reaper`: contexto de lanzamiento.
    MarcaDeLanzamiento(String),
    /// `-dx12`, `-windowed` y demás opciones del juego: **no** se guardan.
    Opcion,
    /// Cualquier otra cosa.
    Otro,
}

/// Clasifica un argumento **sin conservar su texto** salvo cuando es un nombre de fichero.
///
/// Es el punto donde se cumple la política de privacidad: de la línea de comandos solo sale
/// el nombre del ejecutable del juego y los AppID. Un `cmdline` puede llevar rutas
/// personales, tokens de cuentas o contraseñas; nada de eso atraviesa esta función.
pub fn clasificar_argumento(arg: &str) -> ArgumentoRelevante {
    let a = arg.trim();
    if a.is_empty() {
        return ArgumentoRelevante::Otro;
    }

    // `AppId=1623730` — tal cual lo pone el reaper de Steam.
    if let Some(resto) = a
        .strip_prefix("AppId=")
        .or_else(|| a.strip_prefix("appid="))
    {
        if let Ok(n) = resto.trim().parse::<u32>() {
            return ArgumentoRelevante::AppId(n);
        }
    }

    let minus = a.to_ascii_lowercase();
    for marca in ["steamlaunch", "waitforexitandrun", "reaper"] {
        if minus.contains(marca) {
            return ArgumentoRelevante::MarcaDeLanzamiento(marca.to_string());
        }
    }

    if a.starts_with('-') {
        return ArgumentoRelevante::Opcion;
    }

    // Un ejecutable: se mira la extensión del **nombre del fichero**, no del argumento
    // entero, para no confundir una ruta con puntos en las carpetas.
    let nombre = nombre_de_fichero(a);
    let bajo = nombre.to_ascii_lowercase();
    if EXTENSIONES_NO_JUEGO.iter().any(|e| bajo.ends_with(e)) {
        return ArgumentoRelevante::Otro;
    }
    if EXTENSIONES_JUEGO
        .iter()
        .any(|e| bajo.ends_with(&e.to_ascii_lowercase()))
    {
        return ArgumentoRelevante::EjecutableDeJuego(nombre);
    }

    ArgumentoRelevante::Otro
}

/// Lo que la línea de comandos aporta a la identificación, en crudo y sin texto libre.
///
/// Se serializa porque es lo que acaba en las sesiones guardadas: si un campo nuevo se
/// colara con texto de la línea de comandos, la prueba de privacidad lo delataría al
/// serializar.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct EvidenciaDeArgumentos {
    /// Nombres de fichero de los ejecutables de juego vistos (`Palworld-Win64-Shipping.exe`).
    pub ejecutables: Vec<String>,
    /// AppID leídos de `AppId=`.
    pub appids: Vec<u32>,
    /// Marcas de lanzamiento vistas.
    pub marcas: Vec<String>,
    /// ¿La línea tiene forma de ejecución de Windows (ruta `X:\...` o `.exe`)?
    pub parece_de_windows: bool,
    /// Carpeta `steamapps/common/<dir>` de donde sale algún ejecutable, si alguna ruta lo
    /// declara.
    ///
    /// Se guarda **solo el nombre de la carpeta**, nunca la ruta: es lo que distingue el
    /// ejecutable del juego de un helper del sistema invitado
    /// (`C:\windows\system32\tabtip.exe`) sin conservar dónde vive el usuario.
    pub install_dir: Option<String>,
    /// Posición del **primer argumento** que es un programa y no una bandera ni un `AppId=`.
    ///
    /// Sirve para separar «ejecuta» de «menciona»: en `pv-adverb -- reaper SteamLaunch
    /// AppId=… -- …/Deskrawl.exe` el primer programa es `reaper`, así que el `.exe` del final
    /// es un argumento reenviado. En `…/Deskrawl.exe` el propio `.exe` es el primer programa.
    /// `None` si no hay ningún argumento con forma de programa.
    pub posicion_del_programa: Option<usize>,
    /// Posición del **primer ejecutable de juego** visto.
    pub posicion_del_ejecutable: Option<usize>,
}

impl EvidenciaDeArgumentos {
    /// Extrae la evidencia de una lista de argumentos.
    pub fn de_argumentos(args: &[String]) -> Self {
        let mut e = Self::default();
        for (i, arg) in args.iter().enumerate() {
            if es_ruta_de_windows(arg) {
                e.parece_de_windows = true;
            }
            match clasificar_argumento(arg) {
                ArgumentoRelevante::EjecutableDeJuego(n) => {
                    // Del argumento **original** se saca solo la carpeta del juego; el resto
                    // de la ruta se descarta aquí mismo y no se guarda en ninguna parte.
                    if e.install_dir.is_none() {
                        e.install_dir = install_dir_de_ruta(arg);
                    }
                    if e.posicion_del_ejecutable.is_none() {
                        e.posicion_del_ejecutable = Some(i);
                    }
                    if !e.ejecutables.iter().any(|x| x.eq_ignore_ascii_case(&n)) {
                        e.ejecutables.push(n);
                    }
                }
                ArgumentoRelevante::AppId(id) => {
                    if !e.appids.contains(&id) {
                        e.appids.push(id);
                    }
                }
                ArgumentoRelevante::MarcaDeLanzamiento(m) => {
                    if !e.marcas.contains(&m) {
                        e.marcas.push(m);
                    }
                }
                ArgumentoRelevante::Opcion | ArgumentoRelevante::Otro => {}
            }
        }
        // El primer argumento que es un programa: el que no empieza por `-`, no es un
        // `AppId=` y no es una marca de lanzamiento (`SteamLaunch`, `reaper`).
        e.posicion_del_programa = args.iter().position(|a| {
            let t = a.trim();
            !t.is_empty()
                && !t.starts_with('-')
                && !matches!(
                    clasificar_argumento(t),
                    ArgumentoRelevante::AppId(_) | ArgumentoRelevante::MarcaDeLanzamiento(_)
                )
        });
        e
    }

    /// ¿El ejecutable de juego es **el** programa de este proceso, o solo va de argumento?
    ///
    /// `None` cuando no hay ejecutable de juego. Mencionar un `.exe` como argumento reenviado
    /// no demuestra ejecutarlo: eso es lo que separa al juego del envoltorio de Proton que se
    /// lo pasa al runtime.
    pub fn lo_ejecuta_y_no_solo_lo_menciona(&self) -> Option<bool> {
        match (self.posicion_del_ejecutable, self.posicion_del_programa) {
            (None, _) => None,
            (Some(ejec), Some(prog)) => Some(ejec == prog),
            // Sin ningún programa identificable no se puede afirmar que lo ejecute.
            (Some(_), None) => Some(false),
        }
    }

    /// AppID si la línea de comandos da **uno solo**. Dos AppID distintos en el mismo
    /// proceso no se resuelven eligiendo: se deja sin afirmar.
    pub fn appid_unico(&self) -> Option<u32> {
        match self.appids.as_slice() {
            [unico] => Some(*unico),
            _ => None,
        }
    }
}

// ==================== RUTAS Y NOMBRES ====================

/// ¿La cadena tiene forma de ruta de Windows (`S:\steamapps\...`)?
///
/// Se exige letra de unidad, dos puntos y barra (de cualquiera de los dos tipos): un texto
/// con dos puntos sueltos no basta.
pub fn es_ruta_de_windows(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// Nombre del fichero de una ruta, con barras invertidas o normales.
///
/// Las rutas de Windows no se pueden pasar a `Path`: en Linux, `\` es un carácter normal y
/// `Path::new("S:\\a\\b.exe").file_name()` devuelve la ruta entera.
pub fn nombre_de_fichero(ruta: &str) -> String {
    ruta.rsplit(['/', '\\'])
        .find(|t| !t.is_empty())
        .unwrap_or(ruta)
        .to_string()
}

/// ¿La ruta nombra un programa ejecutable?
///
/// Se decide por la extensión, que es lo que distingue un fichero lanzable de una carpeta
/// cuando el lanzador guarda ambos en el mismo campo.
pub fn parece_ejecutable(ruta: &str) -> bool {
    let bajo = nombre_de_fichero(ruta).to_ascii_lowercase();
    [
        ".exe",
        ".sh",
        ".jar",
        ".x86_64",
        ".appimage",
        ".bat",
        ".cmd",
    ]
    .iter()
    .any(|e| bajo.ends_with(e))
}

/// Carpeta que contiene a una ruta, con cualquier separador.
///
/// `"/home/u/Games/X/X.sh"` → `"/home/u/Games/X"`. Se usa para reconocer la instalación de un
/// juego cuando el lanzador solo guarda el ejecutable: el proceso arranca con el `cwd` en esa
/// carpeta, no en la del script.
pub fn ruta_padre(ruta: &str) -> Option<String> {
    let limpia = ruta.trim_end_matches(['/', '\\']);
    let corte = limpia.rfind(['/', '\\'])?;
    if corte == 0 {
        return None;
    }
    let padre = &limpia[..corte];
    if padre.is_empty() {
        None
    } else {
        Some(padre.to_string())
    }
}

/// ¿Dos rutas de carpeta apuntan al mismo sitio?
///
/// Compara **componentes**, no texto: `/juegos/Uno` no es `/juegos/Uno2`. Tolera la barra
/// final y el prefijo `pfx` de un prefijo de Wine, que es la diferencia entre lo que guarda un
/// lanzador y lo que declara el proceso.
pub fn misma_carpeta(a: &str, b: &str) -> bool {
    let na = normalizar_prefijo(a);
    let nb = normalizar_prefijo(b);
    !na.is_empty() && na == nb
}

/// ¿Dos rutas nombran el mismo ejecutable?
///
/// Se comparan las rutas normalizadas; si no coinciden, el **nombre de fichero**, porque el
/// mismo `.exe` aparece como `S:\steamapps\common\X\X.exe` en la línea de comandos y como
/// `/home/u/Games/X/X.exe` en el catálogo del lanzador.
pub fn mismo_ejecutable(configurado: &str, observado: &str) -> bool {
    let a = configurado.trim_end_matches(['/', '\\']);
    let b = observado.trim_end_matches(['/', '\\']);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let na = nombre_de_fichero(a).to_ascii_lowercase();
    let nb = nombre_de_fichero(b).to_ascii_lowercase();
    // Un nombre vacío o genérico no identifica nada: `wine64-preloader` es de todos.
    na == nb && (na.ends_with(".exe") || na.ends_with(".sh") || na.ends_with(".jar"))
}

/// Trozos de una ruta, con cualquier separador.
fn trozos(ruta: &str) -> Vec<&str> {
    ruta.split(['/', '\\']).filter(|t| !t.is_empty()).collect()
}

// ==================== RUTAS E IDENTIDAD MULTI-LANZADOR ====================

/// Componentes normalizados de una ruta, para comparar por **trozos** y no por texto.
///
/// Acepta separadores de Linux y de Windows. No toca mayúsculas: en Linux `Eslabong` y
/// `eslabong` son carpetas distintas y confundirlas atribuiría un juego a otro.
pub fn componentes_de_ruta(ruta: &str) -> Vec<&str> {
    // La unidad de Windows (`X:`) **se conserva** como primer tramo: forma parte de la ruta
    // y dos unidades distintas pueden apuntar a sitios distintos. Descartarla haría que
    // `X:\Games\Game` y `Z:\Games\Game` parecieran el mismo archivo.
    //
    // Quien quiera comparar una ruta de Windows con una de Linux debe traducirla antes con
    // [`MapeoDeUnidades`], que ya quita la unidad al convertirla.
    trozos(ruta)
}

/// ¿El trozo es una unidad de Windows (`X:`)?
fn es_unidad_de_windows(trozo: &str) -> bool {
    let b = trozo.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// ¿`ruta` cae **dentro** de `carpeta` (o es la propia carpeta)?
///
/// Se compara componente a componente: `/home/x/Game` no contiene a `/home/x/Game2`. Las
/// mayúsculas se respetan salvo en los tramos que son de Windows (`X:\`), donde no
/// distinguen.
pub fn esta_dentro(carpeta: &str, ruta: &str) -> bool {
    let c = componentes_de_ruta(carpeta);
    let r = componentes_de_ruta(ruta);
    if c.is_empty() || r.len() < c.len() {
        return false;
    }
    let c_windows = c.first().map(|p| es_unidad_de_windows(p)).unwrap_or(false);
    let r_windows = r.first().map(|p| es_unidad_de_windows(p)).unwrap_or(false);
    // Una ruta de Windows y otra de Linux no son comparables sin mapeo verificado: se
    // traducen antes con `MapeoDeUnidades`. Decir que no es la respuesta honesta.
    if c_windows != r_windows {
        return false;
    }
    if c_windows {
        // Las unidades de Windows no distinguen mayúsculas; el resto del árbol sí se respeta.
        let (cu, ru) = (c[0], r[0]);
        if !cu.eq_ignore_ascii_case(ru) {
            return false;
        }
        return c[1..].iter().zip(r[1..].iter()).all(|(a, b)| a == b);
    }
    c.iter().zip(r.iter()).all(|(a, b)| a == b)
}

/// Correspondencia entre unidades de Windows (`X:`) y rutas de Linux.
///
/// No se adivina: solo existe lo que se haya podido verificar (p. ej. leyendo
/// `dosdevices` del prefijo). Sin mapeo, una ruta de Windows se trata como **evidencia
/// parcial** y no se usa para afirmar que un proceso está dentro de una carpeta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapeoDeUnidades {
    /// Pares `(unidad, ruta_linux)`; la unidad es una sola letra en mayúscula.
    pares: Vec<(char, String)>,
}

impl MapeoDeUnidades {
    pub fn nuevo(pares: Vec<(char, &str)>) -> Self {
        let mut p: Vec<(char, String)> = pares
            .into_iter()
            .filter(|(_, r)| !r.is_empty())
            .map(|(u, r)| (u.to_ascii_uppercase(), r.trim_end_matches('/').to_string()))
            .collect();
        p.sort_by_key(|(u, _)| *u);
        p.dedup_by_key(|(u, _)| *u);
        Self { pares: p }
    }

    /// Traduce una ruta `X:\...` a Linux, si la unidad está mapeada.
    ///
    /// Devuelve `None` cuando no se puede verificar: es preferible no correlacionar a
    /// correlacionar mal.
    pub fn traducir(&self, ruta: &str) -> Option<String> {
        let bytes = ruta.as_bytes();
        if bytes.len() < 2 || bytes[1] != b':' {
            return None;
        }
        let unidad = (bytes[0] as char).to_ascii_uppercase();
        let resto = &ruta[2..];
        let base = self
            .pares
            .iter()
            .find(|(u, _)| *u == unidad)
            .map(|(_, r)| r.clone())?;
        let limpio = resto.trim_start_matches(['\\', '/']).replace('\\', "/");
        if limpio.is_empty() {
            Some(base)
        } else if base == "/" {
            Some(format!("/{limpio}"))
        } else {
            Some(format!("{base}/{limpio}"))
        }
    }
}

/// Una entrada del catálogo unificado de lanzadores.
///
/// Se construye a partir de `launchers::ExternalGame` y del catálogo de Steam. No es una
/// abstracción nueva de descubrimiento: es la **vista común** de datos que ya existían, para
/// que el motor de procesos no dependa de un lanzador concreto.
#[derive(Debug, Clone, PartialEq)]
pub struct EntradaDeCatalogo {
    /// Identificador estable de la plataforma de origen: AppID de Steam, `app_name` de
    /// Heroic/Heronite, o el id de Lutris. `None` si no hay ninguno.
    pub id_plataforma: Option<String>,
    /// De dónde **es** el juego: `steam`, `gog`, `epic`, `sideload`… `None` si no consta.
    pub plataforma: Option<String>,
    /// Quién lo lanzó: `steam`, `lutris`, `heroic`, `heronite`.
    pub launcher: String,
    /// Cómo se ejecuta: `wine`, `linux`, `proton-cachyos`… No decide la plataforma.
    pub runner: Option<String>,
    /// Runtime con el que el lanzador **dice** que ejecutará el juego (`proton-cachyos-slr`,
    /// `wine`…), leído de su configuración.
    ///
    /// Es **información configurada**, no observada: que Heroic tenga configurado Proton no
    /// demuestra que el proceso esté corriendo con esa versión. El runtime real lo dice el
    /// proceso (su prefijo, su binario), y eso es otra cosa. Se guarda aparte del `runner`
    /// justamente para no presentar lo configurado como si fuera lo que está pasando.
    pub runtime_configurado: Option<String>,
    pub titulo: String,
    /// Carpeta de instalación, tal cual la guarda el lanzador (no se persiste).
    pub install_dir: Option<String>,
    /// Prefijo de Wine tal cual lo declara el lanzador, sin normalizar.
    ///
    /// Es **evidencia de correlación**, no un identificador: dos juegos distintos pueden
    /// compartir prefijo (Heronite los agrupa así), y dos ejecuciones del mismo juego pueden
    /// usar prefijos distintos. Nunca se fusionan sesiones por esto.
    pub wine_prefix: Option<String>,
    /// Ejecutable configurado por el lanzador, si lo declara.
    pub exec: Option<String>,
}

impl EntradaDeCatalogo {
    /// ¿La ruta cae dentro de la instalación de esta entrada?
    pub fn contiene(&self, ruta: &str) -> bool {
        self.install_dir
            .as_deref()
            .map(|d| esta_dentro(d, ruta))
            .unwrap_or(false)
    }

    /// ¿El proceso usa el mismo prefijo de Wine que esta entrada?
    ///
    /// Se comparan los **componentes de ruta**, no el texto: Heronite guarda
    /// `/…/Prefixes/eslabong` (directorio base) mientras el proceso declara
    /// `/…/Prefixes/eslabong/pfx` (el prefijo real). Son el mismo prefijo, y `pfx` es un
    /// subdirectorio suyo. Comparar cadenas los separaría.
    ///
    /// Devuelve `false` si falta el dato en cualquiera de los dos lados: la ausencia de
    /// evidencia no es evidencia de que **no** coincidan, por eso quien la use debe tratarla
    /// como «no concluyente» y no como descarte.
    pub fn comparte_prefijo(&self, prefix_del_proceso: &str) -> Option<bool> {
        let declarado = self.wine_prefix.as_deref()?;
        // Se compara contra el prefijo declarado y también contra su `pfx`, que es lo que
        // muchos lanzadores ponen en el entorno.
        let base = normalizar_prefijo(declarado);
        let proceso = normalizar_prefijo(prefix_del_proceso);
        Some(esta_dentro(&base, &proceso) || esta_dentro(&proceso, &base))
    }
}

/// Reduce un prefijo de Wine a su directorio base.
///
/// Heronite guarda el **directorio base** (`Prefixes/eslabong`) mientras el proceso declara
/// el prefijo real (`Prefixes/eslabong/pfx`). Se quita el `pfx` final para que ambos hablen
/// de lo mismo, sin tocar nada más de la ruta.
pub fn normalizar_prefijo(prefijo: &str) -> String {
    let limpia = prefijo.trim_end_matches('/');
    match limpia.strip_suffix("/pfx") {
        Some(base) if !base.is_empty() => base.to_string(),
        _ => limpia.to_string(),
    }
}

/// Reduce varias coincidencias a identidades **distintas**.
///
/// El mismo juego registrado en Steam y en Lutris aparece dos veces; no es ambiguo, es una
/// sola identidad. Se agrupa por identificador de plataforma y, si no lo hay, por título.
pub fn agrupar_por_identidad<'a>(
    coincidencias: &[&'a EntradaDeCatalogo],
) -> Vec<&'a EntradaDeCatalogo> {
    let mut unicas: Vec<&EntradaDeCatalogo> = Vec::new();
    for e in coincidencias {
        let ya = unicas.iter().any(|u| {
            match (&u.id_plataforma, &e.id_plataforma) {
                (Some(a), Some(b)) => a == b && u.plataforma == e.plataforma,
                (None, None) => u.titulo == e.titulo,
                // Uno con id y otro sin él: se comparan por carpeta para no separar de más.
                _ => u.install_dir == e.install_dir && u.titulo == e.titulo,
            }
        });
        if !ya {
            unicas.push(e);
        }
    }
    unicas
}

/// Carpeta `steamapps/common/<dir>` de una ruta, si la tiene.
///
/// Funciona igual con `.../steamapps/common/7 Days to Die/7DaysToDie.x86_64` (Linux) que con
/// `S:\steamapps\common\Palworld\Pal\Binaries\Win64\Juego.exe` (Windows).
pub fn install_dir_de_ruta(ruta: &str) -> Option<String> {
    let partes = trozos(ruta);
    let pos = partes
        .iter()
        .rposition(|p| p.eq_ignore_ascii_case("common"))?;
    if pos == 0 || !partes[pos - 1].eq_ignore_ascii_case("steamapps") {
        return None;
    }
    partes.get(pos + 1).map(|s| s.to_string())
}

/// ¿La ruta pasa por un lanzador de Proton/Wine, en cualquier punto?
pub fn ruta_de_proton(ruta: &str) -> bool {
    let minus = ruta.to_ascii_lowercase();
    MARCAS_PROTON.iter().any(|m| minus.contains(m))
}

/// ¿Este nombre de proceso es un auxiliar de Wine/Proton y no el juego?
pub fn es_auxiliar(nombre: &str) -> bool {
    let base = nombre_de_fichero(nombre);
    let sin_ext = base
        .rsplit_once('.')
        .map(|(a, _)| a)
        .unwrap_or(&base)
        .to_ascii_lowercase();
    AUXILIARES.contains(&sin_ext.as_str())
}

/// Nombre del juego a partir de un ejecutable, para cruzar con el CSV y la biblioteca.
///
/// `Palworld-Win64-Shipping.exe` → `Palworld-Win64-Shipping` (sin extensión). No se recorta
/// el sufijo técnico aquí: eso lo hace la normalización al comparar.
pub fn nombre_de_juego_de_ejecutable(ejecutable: &str) -> String {
    let base = nombre_de_fichero(ejecutable);
    base.rsplit_once('.')
        .filter(|(a, ext)| {
            !a.is_empty()
                && EXTENSIONES_JUEGO
                    .iter()
                    .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(ext))
        })
        .map(|(a, _)| a.to_string())
        .unwrap_or(base)
}

/// Primer `steamapps/common/<dir>` encontrado en una lista de rutas.
pub fn primer_install_dir<'a>(rutas: impl IntoIterator<Item = &'a str>) -> Option<String> {
    rutas.into_iter().find_map(install_dir_de_ruta)
}

// ==================== AGRUPACIÓN ====================

/// Une en un solo grupo los procesos que comparten padre.
///
/// En un lanzamiento Proton el reaper de Steam arranca un árbol entero: `python3` del
/// runtime, el preloader de Wine, el `.exe` del juego y sus hilos. Todos cuelgan del mismo
/// padre (`PPID` 19328 en el caso real). Se agrupan por ese padre **y** por AppID/entorno,
/// de modo que dos juegos distintos con el mismo padre imaginario no se mezclen.
///
/// Los procesos huérfanos (padre 1) se agrupan de uno en uno: no hay evidencia de que
/// compartan nada.
///
/// Es el caso en que lo observado y lo candidato coinciden, que es lo que ocurre cuando el
/// filtro no descartó ningún eslabón. El escaneo real llama a
/// [`agrupar_con_topologia`] con la topología completa, porque ahí sí hay descartados.
#[cfg(test)]
pub fn agrupar_por_padre(procesos: &[RasgosDeProceso]) -> Vec<Vec<usize>> {
    agrupar_con_topologia(procesos, procesos)
}

/// Agrupa los procesos **candidatos** subiendo por la topología **observada**.
///
/// Aquí está la corrección de la fragmentación del árbol. Antes se subía solo por los
/// procesos que habían pasado el prefiltro, así que un eslabón descartado (`python3`,
/// `reaper`, `srt-bwrap` sin evidencia) cortaba la cadena y su hijo se convertía en raíz:
///
/// ```text
/// 253993 systemd-inhibit          ← grupo 0 (raíz, sin padre conocido)
///  └ 254011 python3              ← descartado por el filtro
///     └ 254020 reaper            ← descartado por el filtro
///        └ 254027 srt-bwrap
///           └ 254095 pv-adverb
///              └ 254204 preloader ← grupo 1 (raíz, porque sus padres no estaban)
/// ```
///
/// Con la topología completa, 254204 sube por 254095 → 254027 → 254020 → 254011 hasta
/// 253993, y los dos acaban en el mismo grupo, que es lo que de verdad ocurrió.
///
/// El eslabón descartado **solo** sirve de puente: no entra en el grupo como miembro, no
/// puntúa y no puede ser representante. Se separa la topología de la elegibilidad.
///
/// Límites, a propósito:
///
/// - Solo se sube por procesos **observados** en esta pasada. No se inventa el eslabón que
///   falta: si el padre no se vio, la cadena se corta ahí.
/// - Un ancestro **genérico** (systemd, Steam, Heroic, el escritorio) no une nada: si se
///   subiera hasta él, dos juegos distintos lanzados desde el mismo escritorio acabarían en
///   el mismo grupo. Por eso el ascenso se detiene en la primera raíz de lanzamiento.
pub fn agrupar_con_topologia(
    procesos: &[RasgosDeProceso],
    topologia: &[RasgosDeProceso],
) -> Vec<Vec<usize>> {
    use std::collections::BTreeMap;
    let mut por_padre: BTreeMap<(Option<u32>, Option<u32>), Vec<usize>> = BTreeMap::new();

    // La raíz de cada proceso se calcula una vez: sube por los padres que están en la lista.
    //
    // Un lanzamiento real es una cadena: `reaper → srt-bwrap → pv-adverb → juego`.
    // Agrupando por padre inmediato, cada eslabón caía en un grupo distinto y el envoltorio
    // acababa abriendo su propia sesión, presentado como «el juego». Subiendo hasta el último
    // ancestro conocido que aún se está examinando, el juego y sus envoltorios quedan juntos.
    // La subida usa la **topología observada** (`topologia`), no solo los candidatos: los
    // eslabones descartados siguen sirviendo de puente. `por_pid` solo se usa para los
    // miembros, así que aquí hay un índice propio sobre la topología.
    let por_pid_topo: std::collections::BTreeMap<u32, usize> = topologia
        .iter()
        .enumerate()
        .map(|(i, p)| (p.pid, i))
        .collect();
    let raices_de: Vec<Option<u32>> = procesos
        .iter()
        .map(|p| Some(raiz_en_topologia(p.pid, topologia, &por_pid_topo)))
        .collect();

    // El AppID separa **juegos distintos que comparten un lanzador**, no eslabones de la
    // misma cadena de lanzamiento.
    //
    // Es una precaución de diseño, **no** el arreglo de un fallo observado: cuando se midió
    // con el árbol real de Deskrawl, `pv-adverb` sí llevaba `SteamAppId=4623570` (lo confirmó
    // el usuario en su equipo) y la cadena no se partía. La hipótesis contraria —que el
    // envoltorio no declarara AppID— se comprobó y era falsa.
    //
    // Aun así el criterio se mantiene porque el caso es posible: un envoltorio que no herede
    // `SteamAppId` mientras el juego sí lo declara caería en otro grupo y quedaría solo. Un
    // proceso sin AppID hereda el de su cadena; uno con AppID propio conserva el suyo, así
    // que dos juegos bajo el mismo padre siguen separados.
    let appid_de: Vec<Option<u32>> = procesos
        .iter()
        .map(|p| {
            p.appid_del_entorno
                .as_ref()
                .and_then(|e| e.appid_consensuado().ok().flatten())
        })
        .collect();

    // El AppID **no** se hereda de otros procesos de la misma raíz.
    //
    // Se probó a hacerlo y se revirtió: un proceso sin AppID propio no lo recibe de un
    // hermano. Heredar el entorno no demuestra qué es cada proceso, y el AppID de un proceso
    // cercano puede ser el de otra ejecución (dos juegos bajo el mismo lanzador, un
    // envoltorio que sobrevive a la partida). El AppID de un proceso se usa solo como
    // **indicio de correlación**, nunca atribuyéndolo a quien no lo declara.
    //
    // La consecuencia práctica es que un envoltorio sin AppID forma su propio grupo, y eso es
    // lo correcto: si no hay juego visible, no hay sesión confirmada.
    for (i, p) in procesos.iter().enumerate() {
        let _ = p;
        let appid = appid_de[i];
        por_padre.entry((raices_de[i], appid)).or_default().push(i);
    }
    por_padre.into_values().collect()
}

/// Raíz de lanzamiento de un proceso: el mismo nodo para toda la cadena que arrancó el juego.
///
/// Se sube por los padres **observados** hasta el primero que sea infraestructura
/// (`es_ancestro_generico`). La raíz es el nodo **por debajo** de ese, es decir el primer
/// eslabón propio del lanzamiento. Eso es lo que hace la raíz simétrica: da igual si se
/// pregunta por el juego o por un envoltorio intermedio, la respuesta es el mismo nodo.
///
/// Por qué importa. La versión anterior subía mientras el nodo *actual* no fuera genérico y
/// devolvía `actual`, así que dos procesos de **el mismo lanzamiento** obtenían raíces
/// distintas según dónde se parase cada uno. Medido en el equipo (registro 1219 de
/// `diag_ciclo_vida.json`): el `systemd-inhibit` 426577 daba raíz 426576 y el
/// `wine64-preloader` 426782, que desciende de él, daba raíz 426577. Con el AppID en la
/// clave salían dos grupos con el **mismo** AppID y dos sesiones para un solo juego.
///
/// Límites, a propósito:
///
/// - Si la cadena llega a `1` o a un padre no observado **antes** de encontrar
///   infraestructura, no hay raíz compartida fiable: el proceso es su propia raíz. Meter a
///   todos los que suben a `1` en un mismo grupo fusionaría juegos distintos.
/// - Un proceso que **es** infraestructura tampoco une: su grupo lo forman sus hijos.
/// - El tope de saltos evita colgarse si `/proc` trajera un ciclo.
fn raiz_en_topologia(
    pid: u32,
    topologia: &[RasgosDeProceso],
    por_pid: &std::collections::BTreeMap<u32, usize>,
) -> u32 {
    let Some(&idx) = por_pid.get(&pid) else {
        return pid;
    };
    // La raíz es el **eslabón propio más alto** de la cadena de lanzamiento: se sube mientras
    // el padre observado sea parte del lanzamiento y se para al llegar a infraestructura.
    //
    // La pregunta se resuelve siempre desde el mismo punto —la base de la cadena— para que sea
    // simétrica: así el juego y todos sus envoltorios obtienen el mismo nodo. Subir «hasta
    // donde se pueda» desde cada proceso daba raíces distintas al mismo lanzamiento: medido
    // en el equipo, `426577` y `426590` salían con raíces diferentes y partían el árbol de
    // Palworld en dos grupos con el **mismo** AppID.
    //
    // Qué es infraestructura se decide por estructura, no por nombre: un nodo genérico es
    // infraestructura cuando **encabeza a más de una** rama de lanzamiento, porque entonces
    // varias ejecuciones cuelgan de él y unirlas fusionaría juegos distintos. Un
    // `systemd-inhibit` con una sola rama debajo pertenece a esa ejecución y es su ancla.
    let mut base = idx;
    for _ in 0..64 {
        let Some(ppid) = topologia[base].ppid else {
            break;
        };
        if ppid == 0 || ppid == 1 || ppid == topologia[base].pid {
            break;
        }
        let Some(&idx_padre) = por_pid.get(&ppid) else {
            break;
        };
        if es_infraestructura(&topologia[idx_padre], topologia) {
            break;
        }
        base = idx_padre;
    }
    // Si el propio proceso es infraestructura, no delimita nada: su raíz es él mismo.
    if es_infraestructura(&topologia[idx], topologia) {
        return pid;
    }
    topologia[base].pid
}

/// ¿Este nodo es infraestructura que encabeza **varias** ejecuciones?
///
/// Se decide por estructura: un proceso genérico del sistema o del lanzador que tiene más de
/// una rama de lanzamiento debajo. Con una sola rama, es el ancla de esa ejecución concreta
/// (`systemd-inhibit` lanzado por Steam para un juego) y sí pertenece al lanzamiento.
fn es_infraestructura(p: &RasgosDeProceso, topologia: &[RasgosDeProceso]) -> bool {
    if !es_ancestro_generico(p) {
        return false;
    }
    let hijos: Vec<&RasgosDeProceso> = topologia
        .iter()
        .filter(|otro| otro.ppid == Some(p.pid) && otro.pid != p.pid)
        .collect();
    // Un nodo genérico con hijos **no genéricos** encabeza ejecuciones concretas: es su ancla
    // (`systemd-inhibit` lanzado por Steam para un juego). No es infraestructura aunque tenga
    // más de un hijo, porque serían ejecuciones distintas y cada una se queda con su ancla.
    if hijos.iter().any(|hijo| !es_ancestro_generico(hijo)) {
        return false;
    }
    // Sin ramas propias debajo (solo otros genéricos, o ninguna) es infraestructura
    // compartida: subir hasta él fusionaría juegos distintos.
    true
}

/// ¿Este proceso es infraestructura del sistema o del lanzador, no parte de un lanzamiento?
///
/// Sirve para **no fusionar** dos juegos distintos que comparten escritorio o lanzador. Es un
/// criterio estructural y deliberadamente corto: los procesos con un rol concreto y conocido
/// en el arranque del escritorio. No es una lista de juegos y no decide si algo es un juego;
/// solo dónde termina la cadena que se puede unir.
pub fn es_ancestro_generico(p: &RasgosDeProceso) -> bool {
    let nombre = match p.ejecutable.as_deref() {
        Some(n) => nombre_de_fichero(n).to_ascii_lowercase(),
        None => return false,
    };
    let sin_ext = nombre.split('.').next().unwrap_or(&nombre);
    matches!(
        sin_ext,
        "systemd"
            | "systemd-inhibit"
            | "systemd-logind"
            | "init"
            | "gnome-shell"
            | "kwin_x11"
            | "kwin_wayland"
            | "plasmashell"
            | "xfwm4"
            | "gnome-session-binary"
            | "dbus-daemon"
            | "steam"
            | "steamwebhelper"
            | "heroic"
            | "lutris"
            | "heronite"
            | "gamescope"
            | "bash"
            | "sh"
            | "zsh"
            | "fish"
    ) || nombre.starts_with("systemd")
}

/// Qué papel juega un proceso dentro del lanzamiento del juego.
///
/// Es la pieza que faltaba. Hasta ahora `elegir_principal` repartía puntos con números
/// sueltos y listas de nombres, así que cada envoltorio nuevo (`systemd-inhibit` ayer,
/// `srt-bwrap` hoy) exigía una excepción más. Eso no escala: el árbol de Steam cambia con
/// cada runtime y hay envoltorios que nunca veremos.
///
/// El papel se decide con lo que el proceso **es**, no con cómo se llama:
///
/// - `Ancla`: la raíz de la cadena de lanzamiento (su padre no está en el grupo). Es Steam
///   o su `reaper`: arrancó el juego, pero no lo es.
/// - `Envoltorio`: tiene hijos en el grupo y **no** ejecuta el juego. Un envoltorio existe
///   para arrancar otra cosa; si el juego colgara de él, sería el juego.
/// - `EjecutableDelJuego`: hay evidencia de que es **el juego**. Esto es lo único que puede
///   ser «el juego».
/// - `CandidatoSinConfirmar`: un ejecutable que podría ser un juego, pero sin evidencia
///   suficiente. Un `.exe` bajo Wine **no** basta: también hay aplicaciones de escritorio.
///   No abre sesión; se conserva como observación para no inventar ni descartar.
/// - `Auxiliar`: es otra cosa dentro del árbol (helpers de Wine, manejadores de cuelgues).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PapelEnElLanzamiento {
    Ancla,
    Envoltorio,
    EjecutableDelJuego,
    CandidatoSinConfirmar,
    Auxiliar,
}

/// Decide el papel de cada proceso del grupo.
///
/// `es_raiz[i]` dice si el padre de `procesos[i]` no está en el grupo.
/// Igual que [`repartir_papeles`], con el catálogo de lanzadores disponible.
///
/// El catálogo decide si un proceso que no es de Steam es el juego. Sin él, un juego externo
/// se quedaba como máximo en `CandidatoSinConfirmar` y no había forma de ascenderlo.
pub fn repartir_papeles_con_catalogo(
    procesos: &[RasgosDeProceso],
    es_raiz: &[bool],
    indice: Option<&crate::proceso_linux::IndiceDelCatalogo>,
) -> Vec<PapelEnElLanzamiento> {
    procesos
        .iter()
        .enumerate()
        .map(|(i, p)| {
            // 1. ¿Ejecuta el juego? Es la única prueba que puede hacer «el juego» a algo.
            //
            // Ojo con la trampa que hizo falta descubrir en la prueba real de Deskrawl: un
            // envoltorio **también** menciona el `.exe` en su línea de comandos, porque lo
            // pasa como argumento (`srt-bwrap … -- Deskrawl.exe`). Mirar solo eso lo
            // ascendía a juego.
            //
            // Por eso manda la estructura: si el proceso **tiene hijos que ejecutan el
            // juego**, él no es el juego, por mucho que lo nombre. Ejecutar significa no
            // tener a nadie debajo ejecutándolo. Es el criterio que distingue el envoltorio
            // del runtime sin saber cómo se llama ninguno de los dos.
            let ejecuta_de_verdad = ejecuta_el_juego(p, indice)
                && !tiene_hijos_que_ejecutan(i, procesos, es_raiz, indice);
            if ejecuta_de_verdad {
                return PapelEnElLanzamiento::EjecutableDelJuego;
            }
            // 1b. Ejecuta algo, pero sin evidencia suficiente de que sea un juego.
            //
            // Un `.exe` bajo Wine **no** demuestra que sea un videojuego: también hay
            // aplicaciones de escritorio. Se marca como candidato para no crear una sesión
            // falsa, pero tampoco se descarta: el catálogo del lanzador puede confirmarlo en
            // la misma pasada, y entonces pasa a `EjecutableDelJuego`.
            let podria_ser_juego = podria_ejecutar_un_juego(p)
                && !tiene_hijos_que_ejecutan(i, procesos, es_raiz, indice);
            if podria_ser_juego {
                return PapelEnElLanzamiento::CandidatoSinConfirmar;
            }
            // 2. Auxiliar conocido de Wine/Proton.
            if p.auxiliar {
                return PapelEnElLanzamiento::Auxiliar;
            }
            // 3. Raíz de la cadena.
            if es_raiz.get(i).copied().unwrap_or(false) {
                return PapelEnElLanzamiento::Ancla;
            }
            // 4. Tiene hijos en el grupo y no ejecuta el juego: envuelve a otro.
            let tiene_hijos = procesos
                .iter()
                .any(|otro| otro.ppid == Some(p.pid) && otro.pid != p.pid);
            if tiene_hijos {
                return PapelEnElLanzamiento::Envoltorio;
            }
            // 5. Ni ejecuta el juego, ni es raíz, ni envuelve a nadie.
            PapelEnElLanzamiento::Auxiliar
        })
        .collect()
}

/// ¿Algún descendiente de `i` ejecuta el juego?
///
/// Se mira hacia abajo en el árbol del grupo: si un hijo (o nieto) es el que ejecuta el
/// `.exe`, este proceso no lo ejecuta, lo lanza.
fn tiene_hijos_que_ejecutan(
    i: usize,
    procesos: &[RasgosDeProceso],
    es_raiz: &[bool],
    indice: Option<&crate::proceso_linux::IndiceDelCatalogo>,
) -> bool {
    let mio = procesos[i].pid;
    // Recorrido por anchura sobre los procesos del grupo, evitando ciclos con un tope.
    let mut pendientes = vec![mio];
    let mut vistos = 0usize;
    while let Some(padre) = pendientes.pop() {
        vistos += 1;
        if vistos > 512 {
            break;
        }
        for (j, otro) in procesos.iter().enumerate() {
            if otro.ppid != Some(padre) || otro.pid == padre {
                continue;
            }
            // Solo cuenta como «ejecuta el juego» si lo que ejecuta es el **juego**, no
            // un helper de Windows. `tabtip.exe` o `xalia.exe` viven en `C:\windows\…`
            // y son infraestructura de Wine: que cuelguen del preloader no convierte al
            // preloader en envoltorio.
            let ejecuta = ejecuta_el_juego(otro, indice);
            if ejecuta {
                return true;
            }
            // Si el hijo es la raíz de su propia cadena, ya no cuelga de este proceso.
            if !es_raiz.get(j).copied().unwrap_or(false) {
                pendientes.push(otro.pid);
            }
        }
    }
    false
}

/// ¿El ejecutable de este proceso es el binario del juego (nativo)?
///
/// Solo cuenta la ruta real del ejecutable dentro de `steamapps/common`. El `.exe` de
/// Windows en la línea de comandos lo resuelve `ejecutable_del_juego`, no esto.
/// ¿Este proceso ejecuta el ejecutable del juego?
///
/// No basta con que su línea de comandos nombre un `.exe`, ni con que el `.exe` viva en la
/// carpeta del juego: Unity instala `UnityCrashHandler64.exe` **dentro** de esa misma
/// carpeta, y `tabtip.exe`/`xalia.exe` son infraestructura de Wine. Ninguno es el juego.
///
/// Lo que sí distingue al juego, sin listas de nombres: es el proceso cuyo `.exe` **da nombre
/// a la carpeta de instalación** (`steamapps/common/Deskrawl/Deskrawl.exe`). Así se reconoce
/// cualquier juego, presente o futuro, y cualquier helper queda fuera por su propio nombre.
/// ¿Este proceso declara la carpeta del juego en **su propia** línea de comandos?
///
/// El campo `ruta_del_juego` apunta a la carpeta de donde salió un `.exe`, y un helper puede
/// heredarla del grupo. Comparar el nombre contra una carpeta ajena fue lo que hizo que
/// `tabtip.exe` pasara por el juego: su `.exe` no está en `steamapps/common`, pero el campo
/// traía la carpeta del juego que otro proceso había declarado.
/// ¿El nombre corresponde a un binario del sistema Windows (`C:\windows\…`)?
///
/// Se reconoce por la **ruta completa** cuando la hay, y por el nombre de directorio cuando
/// solo llega una ruta parcial. No se confunde un juego llamado `WindowsGame.exe` con un
/// binario del sistema: se busca el segmento `windows` como componente de ruta, no como
/// subcadena del nombre.
pub fn es_ruta_del_sistema_de_windows(nombre: &str) -> bool {
    let limpia = nombre.replace('/', "\\");
    let partes: Vec<&str> = limpia.split('\\').filter(|p| !p.is_empty()).collect();
    // `C:\windows\system32\tabtip.exe` → hay un componente `windows` antes del fichero.
    // Se exige que no sea el primero tras la unidad salvo que venga de una ruta absoluta con
    // unidad, para no marcar un directorio de trabajo llamado `windows`.
    partes
        .iter()
        .take(partes.len().saturating_sub(1))
        .any(|p| p.eq_ignore_ascii_case("windows"))
        && (limpia.contains(":\\") || limpia.to_ascii_lowercase().contains("windows\\"))
}

/// ¿Este proceso **declara** la carpeta del juego y lo ejecuta de verdad?
///
/// Devuelve la carpeta cuando la evidencia es suficiente, `None` cuando no lo es.
fn declara_la_carpeta_del_juego(p: &RasgosDeProceso) -> Option<String> {
    // Cuando el proceso declara su instalación (`STEAM_COMPAT_INSTALL_PATH`), esa es la
    // evidencia buena: la pone el propio lanzador sobre el proceso del juego.
    //
    // Es lo que distingue el juego del script `proton` (un `python3`): los dos nombran el
    // `.exe` en su línea de comandos, pero solo el juego declara la carpeta de instalación.
    // Sin esta vía, el script `proton` pasaba por `EjecutableDelJuego` y competía por
    // representar la sesión.
    if let Some(declarada) = p.carpeta_declarada.as_deref() {
        let nombre = p.ejecutable_del_juego.as_deref()?;
        // Mencionar un `.exe` como argumento **no** es ejecutarlo.
        //
        // Aquí estaba el defecto real de Deskrawl: el envoltorio `pv-adverb` lleva
        // `…/Deskrawl.exe` en su línea de comandos porque se lo reenvía al runtime, y con la
        // carpeta declarada eso bastaba para abrir una sesión confirmada con él dentro. El
        // juego es el proceso que **ejecuta** ese `.exe`, no el que lo nombra.
        //
        // Solo se descarta cuando el dato es afirmativo (`Some(false)`): si no se pudo
        // determinar, la ausencia de esta evidencia no invalida las demás.
        if p.ejecuta_de_verdad == Some(false) {
            return None;
        }
        // El campo guarda la carpeta ya normalizada, pero se acepta también la ruta completa
        // `steamapps/common/<carpeta>`: así el dato vale tanto si viene del entorno real como
        // si lo escribe una fixture o un test a mano.
        let carpeta = install_dir_de_ruta(declarada).unwrap_or_else(|| declarada.to_string());
        // La carpeta declarada es **evidencia del lanzador**, no una suposición: la pone el
        // propio lanzador sobre el proceso del juego. Por eso basta con que el proceso
        // ejecute un `.exe` que no sea auxiliar; no se exige que el nombre dé nombre a la
        // carpeta, cosa que dejaría fuera `Game.exe`, `launcher.exe` y los títulos cuyo
        // ejecutable no coincide con la carpeta (la mayoría fuera de Steam).
        //
        // Lo que sí se exige es que el `.exe` **esté dentro** de la carpeta declarada cuando
        // la ruta del ejecutable se conoce: si apunta a otro sitio, la declaración no es de
        // este proceso o no es de este juego.
        if es_auxiliar(nombre) {
            return None;
        }
        // Un binario **del sistema Windows** no es el juego, aunque el proceso declare la
        // carpeta de instalación.
        //
        // `STEAM_COMPAT_INSTALL_PATH` la pone Proton sobre **todos** los procesos del
        // contenedor, no solo sobre el juego: la comparten el envoltorio y los auxiliares
        // de Wine. Por sí sola no puede decidir la elegibilidad.
        //
        // El caso real medido: `tabtip.exe` (el teclado táctil de Windows) vive en
        // `C:\windows\system32`, declara la carpeta de Deskrawl y abría una segunda sesión
        // confirmada siendo él el representante, por delante de `Deskrawl.exe`.
        //
        // El criterio es estructural, no una lista de nombres que no terminaría nunca:
        // `C:\windows\…` es infraestructura del sistema invitado. Si un juego viviera ahí
        // —posible en teoría, raro en la práctica— se quedaría en `CandidatoSinConfirmar`,
        // que es el error barato: no abrir sesión antes que abrir una falsa.
        if es_ruta_del_sistema_de_windows(nombre) {
            return None;
        }
        if let Some(ruta) = p.ruta_del_juego.as_deref() {
            let absoluta = MapeoDeUnidades::nuevo(vec![('Z', "/"), ('C', "/")]).traducir(ruta);
            let dentro = absoluta
                .as_deref()
                .map(|r| esta_dentro(declarada, r))
                .unwrap_or(true); // ruta relativa: no se puede comprobar, no se descarta
            if !dentro {
                return None;
            }
        }
        return Some(carpeta.to_lowercase());
    }
    // Sin carpeta declarada, se usa la evidencia de ejecución del propio proceso.
    //
    // Antes se exigía que el `.exe` **diera nombre** a su carpeta (`Deskrawl.exe` en
    // `Deskrawl`). Eso dejaba fuera juegos legítimos cuyo ejecutable no se llama como la
    // carpeta (`Game.exe`, `launcher.exe`, nombres comerciales distintos), que son la
    // mayoría fuera de Steam. Lo que de verdad distingue un juego de un ayudante es la
    // **estructura** (ejecuta el árbol y nadie ejecuta debajo) y que no sea un instalador,
    // y eso ya se comprueba en `repartir_papeles` antes de llegar aquí.
    let nombre = p.ejecutable_del_juego.as_deref()?;
    if es_auxiliar(nombre) {
        return None;
    }
    // La carpeta sale de la ruta del `.exe` si la hay; de la línea de comandos solo se
    // conoce el nombre, que no basta para afirmar una carpeta.
    let dir = p.ruta_del_juego.as_deref()?;
    let carpeta = componentes_de_ruta(dir);
    // Se descarta el nombre del fichero: la carpeta es lo que queda por encima.
    if carpeta.len() < 2 {
        return None;
    }
    Some(carpeta[carpeta.len() - 2].to_string())
}

fn ejecuta_el_juego(
    p: &RasgosDeProceso,
    indice: Option<&crate::proceso_linux::IndiceDelCatalogo>,
) -> bool {
    // Camino corto: el ejecutable Linux vive en la carpeta del juego (juego nativo).
    if dentro_del_juego(p) {
        return true;
    }
    // Por la carpeta que el proceso declara (`STEAM_COMPAT_INSTALL_PATH` o línea de comandos).
    if declara_la_carpeta_del_juego(p).is_some() {
        return true;
    }
    // Evidencia del **catálogo de lanzadores**, cuando está disponible.
    //
    // Es la vía para los juegos que no son de Steam y que no llevan ninguna marca de Steam:
    // si el catálogo conoce este proceso por su ejecutable configurado, su carpeta de
    // instalación o su prefijo de Wine, entonces sí es el juego. Sin este índice,
    // `ejecuta_el_juego` no confirmaba nunca un juego externo.
    if let Some(indice) = indice {
        if indice.confirma(p) {
            return true;
        }
    }
    // Fuera de Steam **no se confirma**: ser un `.exe` bajo Wine no demuestra que sea un
    // videojuego (también hay aplicaciones de escritorio). Aquí solo se confirma con
    // evidencia que el proceso mismo aporta: la carpeta que declara, o que el ejecutable dé
    // nombre a su instalación. Lo que quede fuera de ambos casos se marca como
    // `CandidatoSinConfirmar` y lo confirma el catálogo del lanzador, si lo conoce.
    false
}

/// ¿Este proceso **podría** ser un juego, aunque no haya evidencia para confirmarlo?
///
/// Es el hermano débil de [`ejecuta_el_juego`]: mismo requisito de forma (un `.exe` bajo Wine
/// que no sea auxiliar), pero sin carpeta declarada ni correlación de catálogo. Sirve para
/// no perder el rastro de un juego desconocido **sin** presentarlo como confirmado: un
/// `.exe` bajo Wine puede ser perfectamente una aplicación de escritorio.
fn podria_ejecutar_un_juego(p: &RasgosDeProceso) -> bool {
    let nombre = match p.ejecutable_del_juego.as_deref() {
        Some(n) => n,
        None => return false,
    };
    if !nombre.to_ascii_lowercase().ends_with(".exe") || es_auxiliar(nombre) {
        return false;
    }
    p.bajo_proton.unwrap_or(false)
}

fn dentro_del_juego(p: &RasgosDeProceso) -> bool {
    // Solo la ruta del **ejecutable real** del proceso, nunca `ruta_del_juego`: ese campo
    // guarda la carpeta de la que salió cualquier `.exe` mencionado, así que un helper como
    // `UnityCrashHandler64.exe` (que Unity instala dentro de la carpeta del juego) pasaba por
    // juego. Tener el nombre del binario es la evidencia; estar en una carpeta no lo es.
    let ruta = match p.ruta_ejecutable.as_deref() {
        Some(r) => r,
        None => return false,
    };
    let nombre = match p.ejecutable.as_deref() {
        Some(n) => n,
        None => return false,
    };
    if es_auxiliar(nombre) {
        return false;
    }
    // Estar dentro de `steamapps/common` **no basta**: ahí también viven los runtimes de
    // Steam (`SteamLinuxRuntime_4`, `SteamLinuxRuntime_sniper`…). El proceso tiene que ser,
    // además, un ejecutable del propio juego:
    //
    //  - nativo: un binario dentro de la carpeta del juego, con nombre que no sea de runtime;
    //  - bajo Wine: el binario Linux es el preloader, que **no** está en la carpeta del
    //    juego, así que este camino no se usa y decide `declara_la_carpeta_del_juego`.
    if es_runtime_de_steam(ruta) {
        return false;
    }
    install_dir_de_ruta(ruta).is_some()
}

/// ¿La ruta es de un runtime de Steam y no de un juego?
///
/// `SteamLinuxRuntime_*` ocupa `steamapps/common/` igual que un juego, así que la carpeta no
/// lo distingue. Se reconoce por su propio nombre o por vivir en `pressure-vessel`.
fn es_runtime_de_steam(ruta: &str) -> bool {
    let minus = ruta.to_ascii_lowercase();
    if minus.contains("pressure-vessel") {
        return true;
    }
    componentes_de_ruta(ruta)
        .iter()
        .any(|t| t.to_ascii_lowercase().starts_with("steamlinuxruntime"))
}

/// Elige, dentro de un grupo, el proceso que representa al juego.
///
/// Solo un `EjecutableDelJuego` puede serlo. Si no hay ninguno, devuelve `None`: es preferible
/// decir «identidad desconocida» que presentar un envoltorio, un runtime o un helper como si
/// fuera el juego. Esa es la decisión central de esta función.
///
/// Antes se puntuaba con números (AppID +100, Proton +10, envoltorio −150…) y cada envoltorio
/// nuevo obligaba a añadir una excepción. Eso es lo que hizo que `srt-bwrap` se presentara
/// como el juego: llevaba `SteamAppId` heredado y no estaba en ninguna lista negra.
///
/// Entre varios `EjecutableDelJuego` gana el que tiene la evidencia más fuerte:
/// ejecutable resuelto del propio proceso > AppID > más antiguo.
/// Igual que [`elegir_principal`], con el catálogo de lanzadores disponible.
///
/// Sin el catálogo, un juego externo nunca llegaba a `EjecutableDelJuego` —que es la regla
/// dura para poder representar la sesión— y el grupo se descartaba entero.
pub fn elegir_principal_con_catalogo(
    procesos: &[RasgosDeProceso],
    indice: Option<&crate::proceso_linux::IndiceDelCatalogo>,
) -> Option<usize> {
    let es_raiz = raices(procesos);
    let papeles = repartir_papeles_con_catalogo(procesos, &es_raiz, indice);

    let mut mejor: Option<(usize, i32)> = None;
    for (i, p) in procesos.iter().enumerate() {
        // Regla dura: lo que no ejecuta el juego no puede representarlo.
        if papeles[i] != PapelEnElLanzamiento::EjecutableDelJuego {
            continue;
        }
        let mut puntos = 0;
        if p.ejecutable_del_juego.is_some() {
            puntos += 20;
        }
        // El binario nativo dentro de su carpeta es la evidencia más directa que existe.
        if p.ruta_ejecutable
            .as_deref()
            .and_then(install_dir_de_ruta)
            .is_some()
        {
            puntos += 30;
        }
        // Declarar la instalación (`STEAM_COMPAT_INSTALL_PATH`) vale más que deducirla de la
        // línea de comandos. Lo deduce igual cualquier envoltorio que nombre el `.exe`; solo
        // el proceso del juego lleva la variable puesta por el lanzador. Sin esta diferencia,
        // el script `proton` (un `python3` que menciona el `.exe`) empataba con el juego y el
        // desempate lo decidía el PID.
        if p.carpeta_declarada.is_some() {
            puntos += 40;
        }
        let tiene_appid = p
            .appid_del_entorno
            .as_ref()
            .map(|e| !e.vacia())
            .unwrap_or(false);
        if tiene_appid {
            puntos += 10;
        }
        if p.bajo_proton == Some(true) {
            puntos += 5;
        }
        // A igualdad de evidencia, el primero es el más antiguo del árbol.
        match mejor {
            Some((_, best)) if best >= puntos => {}
            _ => mejor = Some((i, puntos)),
        }
    }
    mejor.map(|(i, _)| i)
}

/// Marca qué procesos son la raíz de su cadena: su padre no está en el grupo.
#[cfg(test)]
pub fn raices_publico(procesos: &[RasgosDeProceso]) -> Vec<bool> {
    raices(procesos)
}

#[cfg(test)]
pub fn ejecuta_el_juego_publico(p: &RasgosDeProceso, indice: Option<&crate::proceso_linux::IndiceDelCatalogo>) -> bool {
    ejecuta_el_juego(p, indice)
}

#[cfg(test)]
pub fn dentro_del_juego_publico(p: &RasgosDeProceso) -> bool {
    dentro_del_juego(p)
}

fn raices(procesos: &[RasgosDeProceso]) -> Vec<bool> {
    procesos
        .iter()
        .map(|p| match p.ppid {
            None => true,
            Some(ppid) => ppid == p.pid || !procesos.iter().any(|o| o.pid == ppid),
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Línea de comandos real de Palworld bajo `proton-cachyos-slr`, copiada del equipo del
    /// usuario. Es la evidencia que la versión anterior descartaba.
    fn cmdline_palworld() -> Vec<String> {
        argumentos_de_cmdline(
            b"S:\\steamapps\\common\\Palworld\\Pal\\Binaries\\Win64\\Palworld-Win64-Shipping.exe\0Pal\0-dx12\0",
        )
    }

    #[test]
    fn reconoce_el_ejecutable_de_windows_y_descarta_las_opciones() {
        let e = EvidenciaDeArgumentos::de_argumentos(&cmdline_palworld());
        assert_eq!(
            e.ejecutables,
            vec!["Palworld-Win64-Shipping.exe".to_string()]
        );
        assert!(e.parece_de_windows);
        // `-dx12` es una opción del juego: no se guarda como evidencia de nada.
        assert!(!e.ejecutables.iter().any(|x| x == "-dx12"));
    }

    #[test]
    fn la_ruta_de_windows_se_reconoce_y_no_se_confunde_con_otros_dos_puntos() {
        assert!(es_ruta_de_windows("S:\\steamapps\\common\\Palworld"));
        assert!(es_ruta_de_windows("C:/juegos/x.exe"));
        assert!(!es_ruta_de_windows("12:30"));
        assert!(!es_ruta_de_windows("http://ejemplo"));
        assert!(!es_ruta_de_windows("a:b"));
    }

    /// Un envoltorio no es el juego aunque herede el entorno de Steam entero.
    ///
    /// Es la regresión de `srt-bwrap`: llevaba `SteamAppId`, era el padre de todo y acabó
    /// presentado como «el proceso del juego». No hace falta conocer su nombre: basta ver
    /// que **no ejecuta el juego** y que tiene hijos en el grupo.
    #[test]
    fn un_envoltorio_no_es_el_juego_por_mucho_que_herede_el_entorno() {
        let rasgos: Vec<RasgosDeProceso> = crate::fixtures_procesos::DESKRAWL_SRT
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let es_raiz = raices(&rasgos);
        let papeles = repartir_papeles_con_catalogo(&rasgos, &es_raiz, None);
        let buscar = |exe: &str| {
            rasgos
                .iter()
                .position(|r| r.ejecutable.as_deref() == Some(exe))
                .unwrap_or_else(|| panic!("falta {exe} en la fixture"))
        };
        // El envoltorio del runtime: no ejecuta el juego, aunque encabece la cadena.
        assert_ne!(
            papeles[buscar("srt-bwrap")],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "srt-bwrap envuelve el lanzamiento, no es el juego"
        );
        // El proceso que sí ejecuta el .exe del juego es el que manda.
        assert_eq!(
            papeles[buscar("wine64-preloader")],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "el preloader que ejecuta Deskrawl.exe sí es el juego"
        );
        // Y el elegido es ese, no el envoltorio.
        let idx = elegir_principal_con_catalogo(&rasgos, None).expect("hay un juego en el grupo");
        assert_eq!(
            rasgos[idx].pid, 40200,
            "el principal es el que ejecuta el juego"
        );
    }

    /// Si no hay ningún proceso que ejecute el juego, no se elige nada.
    ///
    /// Antes se presentaba el envoltorio o el helper con más puntos; ahora se declara
    /// desconocido, que es lo honesto.
    #[test]
    fn sin_ejecutable_del_juego_no_se_elige_principal() {
        let rasgos = vec![
            RasgosDeProceso {
                pid: 10,
                ppid: Some(1),
                ejecutable: Some("srt-bwrap".into()),
                appid_del_entorno: Some(crate::sessions::EvidenciaAppId {
                    steam_app_id: Some("4623570".into()),
                    steam_game_id: Some("4623570".into()),
                }),
                ..Default::default()
            },
            RasgosDeProceso {
                pid: 11,
                ppid: Some(10),
                ejecutable: Some("helper".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            elegir_principal_con_catalogo(&rasgos, None),
            None,
            "sin evidencia de que nadie ejecute el juego, se dice desconocido"
        );
    }

    #[test]
    fn el_nombre_del_fichero_sale_igual_con_barras_invertidas() {
        assert_eq!(
            nombre_de_fichero("S:\\steamapps\\common\\Palworld\\Palworld-Win64-Shipping.exe"),
            "Palworld-Win64-Shipping.exe"
        );
        assert_eq!(nombre_de_fichero("/usr/bin/firefox"), "firefox");
        assert_eq!(nombre_de_fichero("firefox"), "firefox");
    }

    #[test]
    fn la_carpeta_de_instalacion_se_encuentra_en_rutas_de_los_dos_sistemas() {
        assert_eq!(
            install_dir_de_ruta("S:\\steamapps\\common\\Palworld\\Pal\\Binaries\\Win64\\x.exe")
                .as_deref(),
            Some("Palworld")
        );
        assert_eq!(
            install_dir_de_ruta(
                "/run/media/x/Steam/steamapps/common/7 Days to Die/7DaysToDie.x86_64"
            )
            .as_deref(),
            Some("7 Days to Die")
        );
        // Una carpeta `common` sin `steamapps` delante no vale.
        assert_eq!(install_dir_de_ruta("/opt/common/juego/x"), None);
    }

    #[test]
    fn reconoce_los_lanzadores_de_proton_y_los_auxiliares() {
        assert!(ruta_de_proton(
            "/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/i386-unix/wine64-preloader"
        ));
        for aux in [
            "wineserver",
            "winedevice.exe",
            "steamwebhelper",
            "GameOverlayUI.exe",
            "reaper",
        ] {
            assert!(es_auxiliar(aux), "{:?} debería ser auxiliar", aux);
        }
        // El juego no es auxiliar aunque su comm sea raro.
        assert!(!es_auxiliar("GameThread"));
        assert!(!es_auxiliar("Palworld-Win64-Shipping.exe"));
    }

    #[test]
    fn el_appid_de_steamlaunch_se_lee_y_dos_distintos_no_se_resuelven() {
        let uno = EvidenciaDeArgumentos::de_argumentos(&[
            "reaper".into(),
            "SteamLaunch".into(),
            "AppId=1623730".into(),
        ]);
        assert_eq!(uno.appid_unico(), Some(1623730));
        assert!(uno.marcas.iter().any(|m| m == "reaper"));

        let dos =
            EvidenciaDeArgumentos::de_argumentos(&["AppId=1623730".into(), "AppId=251570".into()]);
        assert_eq!(dos.appid_unico(), None, "dos AppID no se resuelven solos");
    }

    #[test]
    fn no_se_guarda_ningun_argumento_que_no_sea_nombre_de_juego() {
        // Una línea de comandos con secretos: nada de eso puede salir en la evidencia.
        let args = argumentos_de_cmdline(
            b"wine64-preloader\0--password=hunter2\0--token=abc123\0/home/ana/privado.txt\0AppId=1623730\0",
        );
        let e = EvidenciaDeArgumentos::de_argumentos(&args);
        let json = serde_json::to_string(&e).unwrap();
        for prohibido in ["hunter2", "abc123", "/home/ana", "privado.txt"] {
            assert!(
                !json.contains(prohibido),
                "se coló {:?} en {:?}",
                prohibido,
                json
            );
        }
        assert!(e.appids.contains(&1623730));
    }

    #[test]
    fn el_nombre_del_juego_pierde_solo_la_extension_de_ejecutable() {
        assert_eq!(
            nombre_de_juego_de_ejecutable("Palworld-Win64-Shipping.exe"),
            "Palworld-Win64-Shipping"
        );
        assert_eq!(
            nombre_de_juego_de_ejecutable("7DaysToDie.x86_64"),
            "7DaysToDie"
        );
        // Un nombre con puntos que no es extensión se queda entero.
        assert_eq!(nombre_de_juego_de_ejecutable("Doom.v2.exe"), "Doom.v2");
    }

    /// `pv-adverb` tampoco es el juego, y no hace falta nombrarlo para saberlo.
    ///
    /// El runtime de Steam añade envoltorios nuevos con cada versión (`srt-bwrap`,
    /// `pv-adverb`, `pv-bwrap`…). Aquí **no** hay ninguna lista de nombres: se comprueba que
    /// la regla estructural —«si tiene hijos que ejecutan el juego, él no lo ejecuta»—
    /// resuelve también este caso, que nunca se ha visto en una prueba real.
    #[test]
    fn otro_envoltorio_del_runtime_tampoco_es_el_juego() {
        let base = |pid: u32, ppid: Option<u32>, exe: &str, ruta: &str| RasgosDeProceso {
            pid,
            ejecutable: Some(exe.to_string()),
            ruta_ejecutable: Some(ruta.to_string()),
            ppid,
            ..Default::default()
        };
        let mut juego = base(
            50300,
            Some(50200),
            "wine64-preloader",
            "/run/media/juegos/steamapps/common/Deskrawl/Deskrawl.exe",
        );
        // El preloader ejecuta el .exe del juego: nombre del .exe == carpeta del juego.
        juego.ejecutable_del_juego = Some("Deskrawl.exe".to_string());
        juego.ruta_del_juego = Some("steamapps/common/Deskrawl".to_string());
        juego.bajo_proton = Some(true);

        let mut adverb = base(
            50200,
            Some(50100),
            "pv-adverb",
            "/usr/lib/pressure-vessel/bin/pv-adverb",
        );
        // Hereda el entorno de Steam, como `srt-bwrap`: eso no lo convierte en el juego.
        adverb.appid_del_entorno = Some(crate::sessions::EvidenciaAppId {
            steam_app_id: Some("4623570".to_string()),
            steam_game_id: Some("4623570".to_string()),
        });
        adverb.ejecutable_del_juego = Some("Deskrawl.exe".to_string());

        let raiz = base(50100, None, "steam", "/usr/bin/steam");
        let procesos = vec![raiz, adverb, juego];
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);

        assert_eq!(
            papeles[1],
            PapelEnElLanzamiento::Envoltorio,
            "pv-adverb envuelve el lanzamiento aunque herede el entorno"
        );
        assert_eq!(papeles[2], PapelEnElLanzamiento::EjecutableDelJuego);
        assert_eq!(
            elegir_principal_con_catalogo(&procesos, None).map(|i| procesos[i].pid),
            Some(50300),
            "el representativo es el que ejecuta el juego"
        );
    }

    #[test]
    fn s53h_arbol_real_el_representante_es_el_juego_y_no_el_envoltorio() {
        let procesos = arbol_real_deskrawl();
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);

        let buscar = |exe: &str| {
            procesos
                .iter()
                .position(|r| r.ejecutable.as_deref() == Some(exe))
                .unwrap_or_else(|| panic!("falta {exe}"))
        };

        println!("papel pv-adverb = {:?}", papeles[buscar("pv-adverb")]);
        println!("papel srt-bwrap = {:?}", papeles[buscar("srt-bwrap")]);
        println!("papel Deskrawl.exe = {:?}", papeles[buscar("Deskrawl.exe")]);

        assert_eq!(
            papeles[buscar("Deskrawl.exe")],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "el .exe del juego ejecuta el juego"
        );
        assert_ne!(
            papeles[buscar("pv-adverb")],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "pv-adverb envuelve el lanzamiento: no es el juego"
        );
        assert_eq!(
            elegir_principal_con_catalogo(&procesos, None).map(|i| procesos[i].pid),
            Some(378741),
            "el representante tiene que ser Deskrawl.exe"
        );
    }

    /// Árbol **real** del equipo del usuario, capturado con Deskrawl corriendo bajo
    /// GE-Proton y el runtime de Steam (CachyOS, 2026-11-25).
    ///
    /// ```text
    /// reaper(378520, AppId=4623570)
    ///  └ srt-bwrap(378525)
    ///     └ pv-adverb(378594)      ← también con SteamAppId=4623570
    ///        ├ Deskrawl.exe(378741) ← con SteamAppId=4623570
    ///        ├ wineserver, winedevice, explorer, services, rpcss, plugplay, xalia…
    ///        └ python3(378627, script `proton`) → steam.exe(378633)
    /// ```
    ///
    /// No es una maqueta ideal: `wine64-preloader` es el ejecutable Linux real del proceso
    /// del juego, y el `.exe` solo aparece en su `comm` y en su `cmdline`. Es exactamente lo
    /// que devolvieron `/proc/<pid>/comm` y `/proc/<pid>/exe`.
    fn arbol_real_deskrawl() -> Vec<RasgosDeProceso> {
        let mut v = Vec::new();

        let mut reaper = RasgosDeProceso {
            pid: 378520,
            ejecutable: Some("reaper".into()),
            ruta_ejecutable: Some("/home/jugador/.local/share/Steam/steamrt64/reaper".into()),
            ppid: Some(378519),
            appid_de_argumentos: Some(4623570),
            bajo_proton: Some(false),
            auxiliar: true,
            ..Default::default()
        };
        reaper.appid_del_entorno = Some(crate::sessions::EvidenciaAppId {
            steam_app_id: Some("4623570".into()),
            steam_game_id: Some("4623570".into()),
        });
        v.push(reaper);

        let mut bwrap = RasgosDeProceso {
            pid: 378525,
            ejecutable: Some("srt-bwrap".into()),
            ruta_ejecutable: Some(
                "/home/jugador/.local/share/Steam/steamapps/common/SteamLinuxRuntime_4/pressure-vessel/libexec/steam-runtime-tools-0/srt-bwrap"
                    .into(),
            ),
            ppid: Some(378520),
            bajo_proton: Some(true),
            ..Default::default()
        };
        bwrap.ejecutable_del_juego = Some("Deskrawl.exe".into());
        bwrap.ruta_del_juego = Some("Deskrawl".into());
        v.push(bwrap);

        // El envoltorio. Se deja **sin** AppID a propósito: es el caso límite que la herencia
        // de `agrupar_por_padre` cubre. En el equipo del usuario `pv-adverb` sí declaraba
        // `SteamAppId=4623570`, y con AppID o sin él la cadena no se parte (comprobado).
        let mut adverb = RasgosDeProceso {
            pid: 378594,
            ejecutable: Some("pv-adverb".into()),
            ruta_ejecutable: Some(
                "/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb".into(),
            ),
            ppid: Some(378525),
            bajo_proton: Some(true),
            appid_del_entorno: None,
            ..Default::default()
        };
        adverb.ejecutable_del_juego = Some("Deskrawl.exe".into());
        adverb.ruta_del_juego = Some("Deskrawl".into());
        v.push(adverb);

        // El juego. `comm = Deskrawl.exe`, `exe = wine64-preloader`, `cmdline` con la ruta
        // Windows. Su AppID viene del entorno.
        let mut juego = RasgosDeProceso {
            pid: 378741,
            ejecutable: Some("Deskrawl.exe".into()),
            ruta_ejecutable: Some(
                "/home/jugador/.local/share/Steam/compatibilitytools.d/GE-Proton11-9-x86_64/files/lib/wine/i386-unix/wine64-preloader"
                    .into(),
            ),
            ppid: Some(378594),
            bajo_proton: Some(true),
            ..Default::default()
        };
        let ev = crate::evidencia_proceso::EvidenciaDeArgumentos::de_argumentos(&[
            "S:\\steamapps\\common\\Deskrawl\\Deskrawl.exe".to_string(),
        ]);
        juego.ejecutable_del_juego = ev.ejecutables.first().cloned();
        juego.ruta_del_juego = ev.install_dir.clone();
        juego.appid_del_entorno = Some(crate::sessions::EvidenciaAppId {
            steam_app_id: Some("4623570".into()),
            steam_game_id: Some("4623570".into()),
        });
        // `STEAM_COMPAT_INSTALL_PATH=/home/jugador/.local/share/Steam/steamapps/common/Deskrawl`,
        // leído del entorno real del proceso.
        juego.carpeta_declarada = Some("steamapps/common/Deskrawl".into());
        v.push(juego);

        // El script `proton` de GE-Proton: nombra el `.exe` en su línea de comandos y cuelga
        // del envoltorio. No ejecuta el juego.
        let mut proton = RasgosDeProceso {
            pid: 378627,
            ejecutable: Some("python3".into()),
            ruta_ejecutable: Some("/usr/bin/python3.13".into()),
            ppid: Some(378594),
            bajo_proton: Some(true),
            auxiliar: true,
            ..Default::default()
        };
        proton.ejecutable_del_juego = Some("Deskrawl.exe".into());
        proton.ruta_del_juego = Some("Deskrawl".into());
        proton.appid_del_entorno = Some(crate::sessions::EvidenciaAppId {
            steam_app_id: Some("4623570".into()),
            steam_game_id: Some("4623570".into()),
        });
        // `python3` **no** declara `STEAM_COMPAT_INSTALL_PATH`: no es el juego. Su carpeta
        // solo se deduce de la línea de comandos, y eso es lo que hay que distinguir.
        proton.carpeta_declarada = None;
        v.push(proton);

        // Infraestructura de Wine, toda colgando del envoltorio.
        for (pid, exe) in [
            (378635u32, "wineserver"),
            (378642, "winedevice.exe"),
            (378705, "explorer.exe"),
            (378723, "rpcss.exe"),
            (378639, "services.exe"),
            (378652, "plugplay.exe"),
            (378734, "xalia.exe"),
        ] {
            v.push(RasgosDeProceso {
                pid,
                ejecutable: Some(exe.to_string()),
                ppid: Some(378594),
                auxiliar: true,
                ..Default::default()
            });
        }

        // Se ordenan por PID, como llegan de `/proc` (el orden no es el del árbol).
        v.sort_by_key(|p| p.pid);
        v
    }

    fn pid_de(procesos: &[RasgosDeProceso], exe: &str) -> u32 {
        procesos
            .iter()
            .find(|p| p.ejecutable.as_deref() == Some(exe))
            .map(|p| p.pid)
            .unwrap_or_else(|| panic!("{exe} no está en el árbol"))
    }

    fn s53j_la_carpeta_declarada_manda_sobre_la_deducida() {
        // Mismo `.exe` y misma carpeta: solo cambia quién la declara.
        let con_carpeta = |pid: u32, declara: bool| {
            let mut p = RasgosDeProceso {
                pid,
                ejecutable: Some("Deskrawl.exe".into()),
                ruta_ejecutable: Some("/usr/bin/python3.13".into()),
                ppid: Some(1),
                bajo_proton: Some(true),
                bajo_compatibilidad: Some(true),                ..Default::default()
            };
            p.ejecutable_del_juego = Some("Deskrawl.exe".into());
            p.ruta_del_juego = Some("Deskrawl".into());
            if declara {
                p.carpeta_declarada = Some("Deskrawl".into());
            }
            p
        };

        // El que **deduce** la carpeta tiene el PID menor: si el desempate fuera por PID,
        // ganaría él.
        let deduce = con_carpeta(100, false);
        let declara = con_carpeta(200, true);
        let procesos = vec![deduce, declara];

        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert!(
            papeles
                .iter()
                .all(|p| *p == PapelEnElLanzamiento::EjecutableDelJuego),
            "los dos ejecutan el .exe: la diferencia está en la fuerza de la evidencia"
        );
        assert_eq!(
            elegir_principal_con_catalogo(&procesos, None),
            Some(1),
            "gana el que declara la carpeta, no el del PID menor"
        );
    }

    /// Un envoltorio que arrastra la carpeta de su ejecutable no adelanta al juego.
    ///
    /// `srt-bwrap` vive en `steamapps/common/SteamLinuxRuntime_4`, así que su propia ruta
    /// parece una instalación. Suma por esa vía, pero **no** es `EjecutableDelJuego`, y la
    /// regla dura lo deja fuera antes de puntuar.
    #[test]
    fn s53j_el_runtime_no_gana_por_vivir_en_steamapps_common() {
        let procesos = arbol_real_deskrawl();
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        let bwrap = procesos
            .iter()
            .position(|p| p.ejecutable.as_deref() == Some("srt-bwrap"))
            .expect("está en el árbol");
        assert_ne!(
            papeles[bwrap],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "srt-bwrap vive en steamapps/common/SteamLinuxRuntime_4 y aun así no es el juego"
        );
        assert_eq!(
            elegir_principal_con_catalogo(&procesos, None).map(|i| procesos[i].pid),
            Some(378741),
            "el representante es el proceso del juego"
        );
    }
    fn s53i_el_envoltorio_sin_appid_no_se_separa_del_juego() {
        let procesos = arbol_real_deskrawl();
        let grupos = agrupar_por_padre(&procesos);

        // El juego y el envoltorio tienen que estar en el **mismo** grupo.
        let juego = pid_de(&procesos, "Deskrawl.exe");
        let adverb = pid_de(&procesos, "pv-adverb");
        let grupo_comun = grupos
            .iter()
            .find(|g| g.iter().any(|&i| procesos[i].pid == juego))
            .expect("el juego está en algún grupo");
        assert!(
            grupo_comun.iter().any(|&i| procesos[i].pid == adverb),
            "pv-adverb y Deskrawl.exe comparten cadena de lanzamiento: no pueden separarse \
             por que uno declare AppID y el otro no"
        );

        // Y en ese grupo, el representante es el juego.
        let miembros: Vec<RasgosDeProceso> =
            grupo_comun.iter().map(|&i| procesos[i].clone()).collect();
        assert_eq!(
            elegir_principal_con_catalogo(&miembros, None).map(|k| miembros[k].pid),
            Some(juego),
            "el representante tiene que ser Deskrawl.exe, no un envoltorio del runtime"
        );
    }

    /// Dos juegos distintos bajo el mismo lanzador siguen separados: cada uno declara su
    /// propio AppID. La herencia del AppID dentro de la cadena no puede fusionarlos.
    #[test]
    fn s53i_dos_juegos_bajo_el_mismo_lanzador_no_se_mezclan() {
        let comun = |pid: u32, ppid: u32| RasgosDeProceso {
            pid,
            ejecutable: Some(format!("helper{pid}")),
            ppid: Some(ppid),
            auxiliar: true,
            ..Default::default()
        };
        let juego = |pid: u32, nombre: &str, appid: u32| {
            let mut p = RasgosDeProceso {
                pid,
                ejecutable: Some(nombre.to_string()),
                ppid: Some(999),
                bajo_proton: Some(true),
                bajo_compatibilidad: Some(true),                ..Default::default()
            };
            p.appid_del_entorno = Some(crate::sessions::EvidenciaAppId {
                steam_app_id: Some(appid.to_string()),
                steam_game_id: Some(appid.to_string()),
            });
            p
        };
        let procesos = vec![
            comun(999, 1),
            juego(1001, "Uno.exe", 111),
            juego(1002, "Dos.exe", 222),
        ];
        let grupos = agrupar_por_padre(&procesos);
        let con_juegos: Vec<&Vec<usize>> = grupos
            .iter()
            .filter(|g| {
                g.iter()
                    .any(|&i| procesos[i].ejecutable.as_deref() != Some("helper999"))
            })
            .collect();
        assert_eq!(con_juegos.len(), 2, "cada juego en su grupo, no fusionados");
    }

    /// Un lanzamiento normal, con todos los eslabones presentes, sigue eligiendo el juego.
    #[test]
    fn s53i_el_representante_es_el_juego_cuando_el_arbol_esta_completo() {
        let procesos = arbol_real_deskrawl();
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        let buscar = |exe: &str| {
            procesos
                .iter()
                .position(|r| r.ejecutable.as_deref() == Some(exe))
                .expect("está en el árbol")
        };

        assert_eq!(
            papeles[buscar("Deskrawl.exe")],
            PapelEnElLanzamiento::EjecutableDelJuego
        );
        assert_ne!(
            papeles[buscar("pv-adverb")],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "pv-adverb envuelve el lanzamiento"
        );
        assert_eq!(
            elegir_principal_con_catalogo(&procesos, None).map(|i| procesos[i].pid),
            Some(378741)
        );
    }
}

/// Pruebas de la identificación multi-lanzador: comparación de rutas y correlación con el
/// catálogo de los lanzadores. Se escriben **antes** de la implementación para que el fallo
/// quede fijado.
#[cfg(test)]
mod pruebas_multilanzador {
    use super::*;

    // ==================== COMPONENTES DE RUTA ====================

    /// Compara por **componentes**, no por texto: una instalación no debe absorber a otra
    /// cuyo nombre solo comparte prefijo.
    #[test]
    fn la_instalacion_no_absorbe_a_otra_con_el_mismo_prefijo() {
        assert!(esta_dentro(
            "/home/jugador/Games/Game",
            "/home/jugador/Games/Game/drive_c/Game.exe"
        ));
        assert!(esta_dentro(
            "/home/jugador/Games/Game",
            "/home/jugador/Games/Game"
        ));
        assert!(
            !esta_dentro("/home/jugador/Games/Game", "/home/jugador/Games/Game2/Game2.exe"),
            "Game2 no está dentro de Game"
        );
        assert!(!esta_dentro(
            "/home/jugador/Games/Game",
            "/home/jugador/Games/GameExtra/x.exe"
        ));
    }

    /// Linux distingue mayúsculas: no se pueden bajar todas las rutas a minúsculas.
    #[test]
    fn linux_distingue_mayusculas() {
        assert!(!esta_dentro(
            "/home/jugador/Games/eslabong",
            "/home/jugador/Games/Eslabong/x.exe"
        ));
    }

    /// Las rutas de Windows que llegan por `cmdline` no se pueden traducir a ciegas: sin
    /// mapeo verificado no hay correspondencia.
    #[test]
    fn una_ruta_de_windows_sin_mapeo_no_se_traduce() {
        // La unidad se conserva: forma parte de la ruta.
        assert_eq!(
            componentes_de_ruta("X:\\Games\\Eslabong\\eslabong.exe"),
            vec!["X:", "Games", "Eslabong", "eslabong.exe"]
        );
        assert!(
            !esta_dentro(
                "/home/jugador/Games/Eslabong",
                "X:\\Games\\Eslabong\\eslabong.exe"
            ),
            "sin mapeo verificado no se afirma que esté dentro"
        );
    }

    /// Con mapeo conocido sí se puede comparar.
    #[test]
    fn con_mapeo_la_ruta_de_windows_se_corresponde() {
        let mapeo = MapeoDeUnidades::nuevo(vec![('X', "/")]);
        assert_eq!(
            mapeo.traducir("X:\\Games\\Eslabong\\eslabong.exe"),
            Some("/Games/Eslabong/eslabong.exe".to_string())
        );
    }

    /// La unidad de Windows **no** se puede descartar antes de comparar: `X:` y `Z:` pueden
    /// apuntar a sitios distintos. Sin mapeo, dos rutas con unidades distintas no son la
    /// misma ruta.
    #[test]
    fn dos_unidades_distintas_no_son_la_misma_ruta() {
        assert!(
            !esta_dentro("X:\\Games\\Game", "Z:\\Games\\Game\\game.exe"),
            "la unidad forma parte de la ruta: no se puede descartar"
        );
        // La misma unidad sí coincide.
        assert!(esta_dentro("X:\\Games\\Game", "X:\\Games\\Game\\game.exe"));
        // Y la unidad no se puede confundir con un tramo de directorio de Linux.
        assert!(!esta_dentro("/Games/Game", "X:\\Games\\Game\\game.exe"));
    }

    /// Espacios y caracteres especiales no rompen la comparación.
    #[test]
    fn los_espacios_no_rompen_la_comparacion() {
        assert!(esta_dentro(
            "/home/jugador/.local/share/Steam/steamapps/common/7 Days to Die",
            "/home/jugador/.local/share/Steam/steamapps/common/7 Days to Die/7DaysToDie.x86_64"
        ));
    }

    // ==================== ELEGIBILIDAD (punto 3) ====================

    fn proceso(pid: u32, exe: &str, ruta: &str, ppid: u32) -> RasgosDeProceso {
        RasgosDeProceso {
            pid,
            ejecutable: Some(exe.to_string()),
            ruta_ejecutable: Some(ruta.to_string()),
            ppid: Some(ppid),
            auxiliar: es_auxiliar(exe),
            ..Default::default()
        }
    }

    /// Un runtime del sistema **no** es el juego aunque viva en `steamapps/common`: si no
    /// hay ningún ejecutable real debajo, no hay representante.
    #[test]
    fn un_runtime_solo_no_es_el_juego() {
        let procesos = vec![
            proceso(1, "steam", "/usr/bin/steam", 0),
            proceso(
                2,
                "srt-bwrap",
                "/home/jugador/.local/share/Steam/steamapps/common/SteamLinuxRuntime_4/srt-bwrap",
                1,
            ),
        ];
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert_ne!(
            papeles[1],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "el runtime no puede ser EjecutableDelJuego"
        );
        assert_eq!(
            elegir_principal_con_catalogo(&procesos, None),
            None,
            "sin ejecutable real no hay representante"
        );
    }

    /// Un auxiliar dentro de la carpeta del juego (crash handler, configurador) no es el
    /// juego.
    #[test]
    fn un_auxiliar_dentro_de_la_instalacion_no_es_el_juego() {
        let procesos = vec![proceso(
            1,
            "UnityCrashHandler64.exe",
            "/home/jugador/Games/Eslabong/UnityCrashHandler64.exe",
            0,
        )];
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert_ne!(papeles[0], PapelEnElLanzamiento::EjecutableDelJuego);
        assert_eq!(elegir_principal_con_catalogo(&procesos, None), None);
    }

    // ---- Los cuatro casos del dominio, cada uno con su contrato ----

    /// Caso 1: **juego conocido** con ejecutable válido fuera de Steam.
    ///
    /// La carpeta del proceso coincide con la instalación que declara el catálogo del
    /// lanzador. La identidad se resuelve por correlación, no por `steamapps/common`.
    #[test]
    fn caso_1_juego_conocido_fuera_de_steam() {
        let catalogo = vec![entrada("Eslabong", "heronite", "/home/jugador/Games/Eslabong")];
        let ruta = "/home/jugador/Games/Eslabong/drive_c/eslabong.exe";
        let coinciden: Vec<&EntradaDeCatalogo> =
            catalogo.iter().filter(|e| e.contiene(ruta)).collect();
        let unicas = agrupar_por_identidad(&coinciden);
        assert_eq!(unicas.len(), 1);
        assert_eq!(unicas[0].titulo, "Eslabong");
    }

    /// Caso 2: **juego desconocido** con evidencia propia suficiente.
    ///
    /// No está en ningún catálogo, pero el proceso **declara su carpeta de instalación**
    /// (`STEAM_COMPAT_INSTALL_PATH`, que Heronite y UMU ponen sobre el juego). Esa es la
    /// evidencia propia: no basta con ser un `.exe` bajo Wine, porque también hay
    /// aplicaciones de escritorio.
    #[test]
    fn caso_2_juego_desconocido_con_evidencia_suficiente() {
        let mut juego = proceso(
            2,
            "JuegoRaro.exe",
            "/home/jugador/Games/Loquesea/drive_c/JuegoRaro.exe",
            1,
        );
        juego.bajo_proton = Some(true);
        // Evidencia propia: la carpeta de instalación que declara el proceso.
        juego.carpeta_declarada = Some("/home/jugador/Games/Loquesea".into());
        juego.ejecutable_del_juego = Some("JuegoRaro.exe".into());
        juego.ruta_del_juego = Some("JuegoRaro.exe".into());
        let procesos = vec![proceso(1, "wine", "/usr/bin/wine", 0), juego];
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert_eq!(
            papeles[1],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "un .exe bajo Wine que ejecuta el árbol es candidato aunque no esté en el catálogo"
        );
        assert_eq!(elegir_principal_con_catalogo(&procesos, None), Some(1));
    }

    /// Caso 3: **naturaleza no confirmada**. Un `.exe` bajo Wine **sin** evidencia de que
    /// sea un juego (es un instalador) no se presenta como juego, ni se inventa identidad.
    #[test]
    fn caso_3_proceso_sin_naturaleza_confirmada() {
        let mut instalador = proceso(
            2,
            "setup.exe",
            "/home/jugador/Games/Loquesea/drive_c/setup.exe",
            1,
        );
        instalador.bajo_proton = Some(true);
        instalador.ejecutable_del_juego = Some("setup.exe".into());
        instalador.ruta_del_juego = Some("setup.exe".into());
        let procesos = vec![proceso(1, "wine", "/usr/bin/wine", 0), instalador];
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert_ne!(
            papeles[1],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "un instalador no es un videojuego"
        );
        assert_eq!(elegir_principal_con_catalogo(&procesos, None), None);
    }

    // ---- Reproducción de Deskrawl real (PID 557367 / 557505 / 557514) ----

    /// Tres procesos tal como los reportó el usuario, sin nombres inventados.
    ///
    /// `ruta_del_juego` **no** es la ruta Windows del `.exe`: el pipeline guarda ahí la
    /// carpeta `steamapps/common/<dir>` (ver `ObservadorDeProc::rasgos`). Un fixture con la
    /// ruta Windows sería cómodo y reproduciría un estado que el observador real nunca
    /// produce, así que enmascaraba el defecto.
    ///
    /// Es la ejecución real de Deskrawl desde Steam: un envoltorio (`pv-adverb`), un auxiliar
    /// de Windows (`tabtip.exe`) y el ejecutable del juego (`Deskrawl.exe`), los tres con
    /// `SteamAppId=4623570` y la misma carpeta declarada.
    fn deskrawl_real() -> Vec<RasgosDeProceso> {
        use crate::sessions::EvidenciaAppId;
        let appid = || {
            Some(EvidenciaAppId {
                steam_app_id: Some("4623570".into()),
                steam_game_id: Some("4623570".into()),
                ..Default::default()
            })
        };
        let instalacion = Some("steamapps/common/Deskrawl".to_string());
        let prefijo =
            Some("/home/jugador/.local/share/Steam/steamapps/compatdata/4623570".to_string());
        vec![
            // Proceso 1: el envoltorio del runtime.
            RasgosDeProceso {
                pid: 557367,
                ppid: Some(557298),
                ejecutable: Some("pv-adverb".into()),
                ruta_ejecutable: Some(
                    "/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb"
                        .into(),
                ),
                inicio_del_proceso: Some("2026-11-25 22:37:59".into()),
                appid_del_entorno: appid(),
                bajo_proton: Some(true),
                bajo_compatibilidad: Some(true),                ejecutable_del_juego: Some("Deskrawl.exe".into()),
                ruta_del_juego: instalacion.clone(),
                carpeta_declarada: instalacion.clone(),
                wine_prefix: prefijo.clone(),
                cwd: instalacion.clone(),
                auxiliar: false,
                appid_de_argumentos: None,
                ejecuta_de_verdad: Some(false),
            },
            // Proceso 2: auxiliar de Windows dentro del prefijo.
            RasgosDeProceso {
                pid: 557505,
                ppid: Some(557367),
                ejecutable: Some("tabtip.exe".into()),
                ruta_ejecutable: Some("wine64-preloader".into()),
                inicio_del_proceso: Some("2026-11-25 22:38:09".into()),
                appid_del_entorno: appid(),
                bajo_proton: Some(true),
                bajo_compatibilidad: Some(true),                ejecutable_del_juego: Some("tabtip.exe".into()),
                ruta_del_juego: instalacion.clone(),
                carpeta_declarada: instalacion.clone(),
                wine_prefix: prefijo.clone(),
                cwd: instalacion.clone(),
                auxiliar: false,
                appid_de_argumentos: None,
                ejecuta_de_verdad: Some(false),
            },
            // Proceso 3: el ejecutable real del juego.
            RasgosDeProceso {
                pid: 557514,
                ppid: Some(557367),
                ejecutable: Some("Deskrawl.exe".into()),
                ruta_ejecutable: Some("wine64-preloader".into()),
                inicio_del_proceso: Some("2026-11-25 22:38:09".into()),
                appid_del_entorno: appid(),
                bajo_proton: Some(true),
                bajo_compatibilidad: Some(true),                ejecutable_del_juego: Some("Deskrawl.exe".into()),
                ruta_del_juego: instalacion.clone(),
                carpeta_declarada: instalacion.clone(),
                wine_prefix: prefijo,
                cwd: instalacion,
                auxiliar: false,
                appid_de_argumentos: None,
                ejecuta_de_verdad: Some(true),
            },
        ]
    }

    /// **Causa raíz medida**: ¿qué se puede saber de `tabtip.exe` frente a `Deskrawl.exe`?
    ///
    /// Los dos llegan con la misma carpeta declarada y el mismo AppID. La única diferencia
    /// que el observador **conserva** es el nombre del ejecutable, porque la ruta Windows se
    /// descarta a propósito al leer la línea de comandos.
    #[test]
    fn la_unica_diferencia_conservada_entre_tabtip_y_deskrawl_es_el_nombre() {
        let procesos = deskrawl_real();
        let tabtip = &procesos[1];
        let deskrawl = &procesos[2];
        assert_eq!(tabtip.carpeta_declarada, deskrawl.carpeta_declarada);
        assert_eq!(tabtip.ruta_del_juego, deskrawl.ruta_del_juego);
        assert_eq!(tabtip.ejecutable_del_juego, Some("tabtip.exe".into()));
        assert_eq!(deskrawl.ejecutable_del_juego, Some("Deskrawl.exe".into()));
    }

    /// **Defecto real.** El ejecutable del juego tiene que ganar, y los otros dos no pueden
    /// ser representantes.
    #[test]
    fn en_la_ejecucion_real_de_deskrawl_gana_el_ejecutable_del_juego() {
        let procesos = deskrawl_real();
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert_eq!(
            papeles[2],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "Deskrawl.exe es el juego, papeles: {:?}",
            papeles
        );
        assert_ne!(
            papeles[0],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "pv-adverb envuelve: no puede ser el juego, papeles: {:?}",
            papeles
        );
        assert_ne!(
            papeles[1],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "tabtip.exe es un auxiliar de Windows, papeles: {:?}",
            papeles
        );
        let principal =
            elegir_principal_con_catalogo(&procesos, None).expect("debe haber representante");
        assert_eq!(
            procesos[principal].pid, 557514,
            "el representante es Deskrawl.exe"
        );
    }

    /// **Defecto real.** Un solo lanzamiento no puede ser dos grupos.
    #[test]
    fn la_ejecucion_real_de_deskrawl_es_un_solo_grupo() {
        let procesos = deskrawl_real();
        let grupos = agrupar_por_padre(&procesos);
        assert_eq!(
            grupos.len(),
            1,
            "una ejecución es un grupo, no {}: {:?}",
            grupos.len(),
            grupos
        );
    }

    // ---- Wine prefix: evidencia de correlación, nunca identidad ----

    fn entrada_con_prefijo(titulo: &str, prefijo: Option<&str>) -> EntradaDeCatalogo {
        EntradaDeCatalogo {
            id_plataforma: Some(titulo.to_lowercase()),
            plataforma: None,
            launcher: "heronite".into(),
            runner: Some("wine".into()),
            titulo: titulo.into(),
            install_dir: None,
            wine_prefix: prefijo.map(|p| p.to_string()),
            exec: None,
            runtime_configurado: None,
        }
    }

    /// El caso real de Heronite: el catálogo guarda el **directorio base**
    /// (`/…/Prefixes/eslabong`) y el proceso declara el prefijo real
    /// (`/…/Prefixes/eslabong/pfx`). Son el mismo prefijo.
    #[test]
    fn el_directorio_base_y_su_pfx_son_el_mismo_prefijo() {
        let e = entrada_con_prefijo(
            "eslabong",
            Some("/home/jugador/Games/Heronite/Prefixes/eslabong"),
        );
        assert_eq!(
            e.comparte_prefijo("/home/jugador/Games/Heronite/Prefixes/eslabong/pfx"),
            Some(true),
            "pfx es un subdirectorio del prefijo declarado"
        );
        assert_eq!(
            e.comparte_prefijo("/home/jugador/Games/Heronite/Prefixes/eslabong"),
            Some(true)
        );
    }

    /// Dos juegos que comparten prefijo no son el mismo juego.
    ///
    /// El prefijo se comparte a menudo (Heronite agrupa juegos bajo un prefijo común): si
    /// sirviera para fusionar, se perdería uno de los dos.
    #[test]
    fn dos_juegos_distintos_pueden_compartir_prefijo_sin_fusionarse() {
        let mismo = "/home/jugador/Games/Heronite/Prefixes/comun";
        let a = entrada_con_prefijo("Eslabong", Some(mismo));
        let b = entrada_con_prefijo("OtroJuego", Some(mismo));
        assert_eq!(
            a.comparte_prefijo("/home/jugador/Games/Heronite/Prefixes/comun/pfx"),
            Some(true)
        );
        assert_eq!(
            b.comparte_prefijo("/home/jugador/Games/Heronite/Prefixes/comun/pfx"),
            Some(true),
            "el segundo también coincide con su propio prefijo"
        );
        // Pero son identidades distintas: no se agrupan.
        let coincidencias = vec![&a, &b];
        assert_eq!(
            agrupar_por_identidad(&coincidencias).len(),
            2,
            "compartir prefijo no los convierte en el mismo juego"
        );
    }

    /// Un juego con prefijo distinto no coincide.
    #[test]
    fn un_prefijo_distinto_no_coincide() {
        let e = entrada_con_prefijo("Eslabong", Some("/prefixes/eslabong"));
        assert_eq!(
            e.comparte_prefijo("/prefixes/otro/pfx"),
            Some(false),
            "otro prefijo es otra instalación"
        );
    }

    /// Prefijo desconocido en cualquiera de los dos lados: no se concluye nada.
    #[test]
    fn un_prefijo_desconocido_no_concluye_nada() {
        let sin_prefijo = entrada_con_prefijo("Eslabong", None);
        assert_eq!(
            sin_prefijo.comparte_prefijo("/prefixes/eslabong/pfx"),
            None,
            "sin dato en el catálogo: no se puede afirmar ni negar"
        );
    }

    /// Una ruta de prefijo no se compara por texto: `eslabong` y `eslabong2` son distintos.
    #[test]
    fn prefijos_de_nombre_parecido_no_se_confunden() {
        let e = entrada_con_prefijo("Eslabong", Some("/prefixes/eslabong"));
        assert_eq!(
            e.comparte_prefijo("/prefixes/eslabong2/pfx"),
            Some(false),
            "la comparación es por componentes, no por texto"
        );
    }

    // ---- Punto 2: la estructura sola tampoco basta ----

    /// Un auxiliar de **nombre genérico** que no está en `AUXILIARES` también es hoja dentro
    /// de la instalación. «Ejecuta el árbol y nadie ejecuta debajo» no demuestra que sea un
    /// juego: un configurador, un actualizador o un manejador de cuelgues desconocido cumple
    /// lo mismo.
    ///
    /// Sin evidencia adicional debe quedar `CandidatoSinConfirmar`, nunca confirmado.
    #[test]
    fn un_auxiliar_generico_desconocido_no_se_confirma() {
        for nombre in [
            "ConfigTool.exe",
            "Updater.exe",
            "Helper.exe",
            "GameConfig.exe",
            "LauncherSetup.exe",
        ] {
            let mut aux = proceso(2, nombre, &format!("/home/jugador/Games/JuegoX/{nombre}"), 1);
            aux.bajo_proton = Some(true);
            aux.ejecutable_del_juego = Some(nombre.into());
            aux.ruta_del_juego = Some(nombre.into());
            let procesos = vec![proceso(1, "wine", "/usr/bin/wine", 0), aux];
            let es_raiz = raices(&procesos);
            let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
            assert_ne!(
                papeles[1],
                PapelEnElLanzamiento::EjecutableDelJuego,
                "{nombre} es una hoja del árbol, pero no hay evidencia de que sea el juego"
            );
            assert_eq!(
                elegir_principal_con_catalogo(&procesos, None),
                None,
                "{nombre} no abre sesión"
            );
        }
    }

    /// Si el lanzador **declara** cuál es su ejecutable, un auxiliar desconocido no le roba
    /// el papel: la evidencia del catálogo manda sobre la heurística.
    #[test]
    fn el_ejecutable_del_lanzador_prevalece_sobre_un_auxiliar() {
        let mut juego = proceso(2, "Game.exe", "/home/jugador/Games/JuegoX/Game.exe", 1);
        juego.bajo_proton = Some(true);
        juego.carpeta_declarada = Some("/home/jugador/Games/JuegoX".into());
        juego.ejecutable_del_juego = Some("Game.exe".into());
        juego.ruta_del_juego = Some("Game.exe".into());
        let mut aux = proceso(
            3,
            "ConfigTool.exe",
            "/home/jugador/Games/JuegoX/ConfigTool.exe",
            1,
        );
        aux.bajo_proton = Some(true);
        aux.ejecutable_del_juego = Some("ConfigTool.exe".into());
        aux.ruta_del_juego = Some("ConfigTool.exe".into());
        let procesos = vec![proceso(1, "wine", "/usr/bin/wine", 0), juego, aux];
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert_eq!(
            papeles[1],
            PapelEnElLanzamiento::EjecutableDelJuego,
            "el que declara carpeta de instalación es el juego"
        );
        assert_eq!(
            elegir_principal_con_catalogo(&procesos, None),
            Some(1),
            "el principal es el juego (posición 1), no el configurador (posición 2)"
        );
    }

    // ---- Punto 1: un .exe bajo Wine NO es prueba de juego por sí solo ----

    /// Aplicaciones de escritorio de Windows ejecutadas con Wine: no son videojuegos.
    ///
    /// Sin evidencia adicional, el proceso queda **no confirmado**: no se crea sesión. Es
    /// preferible identidad desconocida a un falso positivo en el historial del usuario.
    #[test]
    fn una_aplicacion_de_escritorio_bajo_wine_no_es_un_juego() {
        let casos: &[(&str, &str)] = &[
            (
                "notepad.exe",
                "/home/jugador/.wine/drive_c/windows/notepad.exe",
            ),
            ("winword.exe", "/home/jugador/.wine/drive_c/Office/winword.exe"),
            (
                "photoshop.exe",
                "/home/jugador/.wine/drive_c/Adobe/photoshop.exe",
            ),
            (
                "thunderbird.exe",
                "/home/jugador/.wine/drive_c/thunderbird/thunderbird.exe",
            ),
            ("vlc.exe", "/home/jugador/.wine/drive_c/VideoLAN/vlc.exe"),
        ];
        for (nombre, ruta) in casos {
            let mut app = proceso(2, nombre, ruta, 1);
            app.bajo_proton = Some(true);
            app.ejecutable_del_juego = Some((*nombre).into());
            app.ruta_del_juego = Some((*nombre).into());
            let procesos = vec![proceso(1, "wine", "/usr/bin/wine", 0), app];
            let es_raiz = raices(&procesos);
            let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
            assert_eq!(
                papeles[1],
                PapelEnElLanzamiento::CandidatoSinConfirmar,
                "{nombre} podría ser un juego, pero no hay evidencia: queda sin confirmar"
            );
            assert_ne!(
                papeles[1],
                PapelEnElLanzamiento::EjecutableDelJuego,
                "{nombre} es una aplicación de escritorio, no un juego"
            );
            assert_eq!(
                elegir_principal_con_catalogo(&procesos, None),
                None,
                "{nombre} no debe abrir una sesión de juego"
            );
        }
    }

    /// Un `.exe` bajo Wine **sin** evidencia de ser juego no se confirma, pero tampoco se
    /// inventa: se conserva su rastro como no confirmado.
    #[test]
    fn un_exe_sin_evidencia_queda_no_confirmado() {
        let mut dudoso = proceso(
            2,
            "Programa.exe",
            "/home/jugador/.wine/drive_c/Programa.exe",
            1,
        );
        dudoso.bajo_proton = Some(true);
        dudoso.ejecutable_del_juego = Some("Programa.exe".into());
        dudoso.ruta_del_juego = Some("Programa.exe".into());
        let procesos = vec![proceso(1, "wine", "/usr/bin/wine", 0), dudoso];
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert_eq!(
            papeles[1],
            PapelEnElLanzamiento::CandidatoSinConfirmar,
            "sin evidencia se conserva como candidato, no se confirma"
        );
        assert_eq!(
            elegir_principal_con_catalogo(&procesos, None),
            None,
            "un candidato no es representante: no abre sesión"
        );
    }

    /// Caso 4: **envoltorio o auxiliar sin ejecutable real**.
    #[test]
    fn caso_4_envoltorio_sin_ejecutable_real() {
        let procesos = vec![
            proceso(1, "heroic", "/usr/bin/heroic", 0),
            proceso(
                2,
                "wine",
                "/home/jugador/.local/share/heroic/tools/wine/bin/wine",
                1,
            ),
        ];
        let es_raiz = raices(&procesos);
        let papeles = repartir_papeles_con_catalogo(&procesos, &es_raiz, None);
        assert!(papeles
            .iter()
            .all(|p| *p != PapelEnElLanzamiento::EjecutableDelJuego));
        assert_eq!(elegir_principal_con_catalogo(&procesos, None), None);
    }

    // ==================== CATÁLOGO ====================

    fn entrada(nombre: &str, launcher: &str, install: &str) -> EntradaDeCatalogo {
        EntradaDeCatalogo {
            id_plataforma: None,
            plataforma: None,
            launcher: launcher.to_string(),
            runner: None,
            titulo: nombre.to_string(),
            install_dir: Some(install.to_string()),
            wine_prefix: None,
            exec: None,
            runtime_configurado: None,
        }
    }

    /// Un proceso dentro de una instalación conocida se **correlaciona** con ese juego, pero
    /// eso es identidad, no papel.
    #[test]
    fn la_carpeta_correlaciona_con_el_juego() {
        let catalogo = vec![entrada("Eslabong", "heronite", "/home/jugador/Games/Eslabong")];
        let ruta = "/home/jugador/Games/Eslabong/drive_c/Game.exe";
        let elegido = catalogo.iter().find(|e| e.contiene(ruta));
        assert_eq!(elegido.map(|e| e.titulo.as_str()), Some("Eslabong"));
    }

    /// Dos juegos en la **misma** carpeta no se resuelven: sería adivinar.
    #[test]
    fn dos_juegos_en_la_misma_carpeta_son_ambiguos() {
        let catalogo = vec![
            entrada("Uno", "lutris", "/home/jugador/Games/Compartida"),
            entrada("Dos", "lutris", "/home/jugador/Games/Compartida"),
        ];
        let ruta = "/home/jugador/Games/Compartida/game.exe";
        let coinciden: Vec<&EntradaDeCatalogo> =
            catalogo.iter().filter(|e| e.contiene(ruta)).collect();
        assert_eq!(coinciden.len(), 2, "las dos coinciden y no se elige");
    }

    /// El mismo juego en Steam y en Lutris **no** es ambiguo: es una sola identidad.
    #[test]
    fn el_mismo_juego_en_dos_lanzadores_no_es_ambiguo() {
        let catalogo = vec![
            EntradaDeCatalogo {
                id_plataforma: Some("4623570".into()),
                plataforma: Some("steam".into()),
                launcher: "steam".into(),
                runner: None,
                titulo: "Deskrawl".into(),
                install_dir: Some("/home/jugador/.local/share/Steam/steamapps/common/Deskrawl".into()),
                wine_prefix: None,
                exec: None,
                runtime_configurado: None,
            },
            EntradaDeCatalogo {
                id_plataforma: Some("4623570".into()),
                plataforma: Some("steam".into()),
                launcher: "lutris".into(),
                runner: Some("wine".into()),
                titulo: "Deskrawl".into(),
                install_dir: Some("/home/jugador/.local/share/Steam/steamapps/common/Deskrawl".into()),
                wine_prefix: None,
                exec: None,
                runtime_configurado: None,
            },
        ];
        let ruta = "/home/jugador/.local/share/Steam/steamapps/common/Deskrawl/Deskrawl.exe";
        let coinciden: Vec<&EntradaDeCatalogo> =
            catalogo.iter().filter(|e| e.contiene(ruta)).collect();
        assert_eq!(coinciden.len(), 2, "coinciden las dos...");
        let unicas = agrupar_por_identidad(&coinciden);
        assert_eq!(unicas.len(), 1, "...pero son la misma identidad");
        assert_eq!(unicas[0].plataforma.as_deref(), Some("steam"));
    }

    /// El runner no decide la plataforma: un juego de Steam con Wine sigue siendo de Steam.
    #[test]
    fn el_runner_no_decide_la_plataforma() {
        let catalogo = vec![EntradaDeCatalogo {
            id_plataforma: Some("4623570".into()),
            plataforma: Some("steam".into()),
            launcher: "lutris".into(),
            runner: Some("wine".into()),
            titulo: "Deskrawl".into(),
            install_dir: Some("/home/jugador/Games/Deskrawl".into()),
            wine_prefix: None,
            exec: None,
            runtime_configurado: None,
        }];
        let ruta = "/home/jugador/Games/Deskrawl/Deskrawl.exe";
        let e = catalogo.iter().find(|e| e.contiene(ruta)).unwrap();
        assert_eq!(e.plataforma.as_deref(), Some("steam"));
        assert_eq!(e.launcher, "lutris");
    }
}
