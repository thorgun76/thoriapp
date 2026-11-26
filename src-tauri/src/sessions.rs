//! Sesiones de ejecución: qué se pudo observar de un juego mientras corría.
//!
//! # Qué es y qué no es
//!
//! Una **sesión** agrupa los datos de una ejecución del juego: qué proceso se vio, qué
//! configuración tenía Thoriapp en ese momento, qué opciones decía Steam, y qué
//! grabaciones se le atribuyen con evidencia suficiente.
//!
//! No es «lo que pasó». Es **lo que se pudo observar**. Cada dato lleva su procedencia y
//! su instante, y lo que no se observó se queda en `None` en vez de rellenarse con el
//! estado actual. La regla del proyecto vale también aquí:
//!
//! > La ausencia de evidencia nunca se convierte en evidencia de ausencia.
//!
//! Por eso `EvidenciaProceso::SinVisibilidad` (Docker, otro usuario, kernel con `hidepid`)
//! **no** significa «el juego no estaba corriendo»: significa «no se pudo mirar». La
//! interfaz distingue las dos cosas.
//!
//! # Por qué un módulo aparte
//!
//! `benchmarks.rs` almacena y calcula; esto **observa** y decide a qué sesión pertenece
//! una grabación. Son responsabilidades distintas y con reglas de evidencia propias. Aquí
//! vive el dominio (tipos y reglas, sin E/S); la lectura real de `/proc` está en
//! `proceso_linux` detrás del puerto [`ObservadorDeProcesos`], para poder probar todo con
//! un observador simulado.

// El dominio se escribe entero antes de que la interfaz lo consuma: los tipos y las reglas
// de asociación están completos y probados, y su cableado a React llega en esta misma
// subfase. Sin esto, `cargo build` avisaría de cada variante aún no construida y taparía
// los avisos que importan.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::benchmarks::ConProcedencia;

/// Versión del esquema de sesiones guardadas.
///
/// Se sube solo si cambia la forma de lo persistido. Los ficheros sin `version` se tratan
/// como la primera.
pub const SESSIONS_SCHEMA_VERSION: u32 = 1;

// ==================== EVIDENCIA DE PROCESO ====================

/// ¿Se pudo ver el proceso del juego?
///
/// La distinción es el corazón de la subfase: un proceso que no se ve no es un proceso que
/// no exista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenciaProceso {
    /// Se vio un proceso del juego y se le siguió.
    Observada,
    /// No hay visibilidad (contenedor, otro usuario, `hidepid`). **No** es «no corre».
    SinVisibilidad,
    /// Se listaron procesos pero no se pudo leer lo necesario de los candidatos.
    SinPermisos,
    /// Se observó antes, pero en esta pasada no se le vio.
    ///
    /// **No** es lo mismo que `Terminada`: `/proc` puede fallar, el proceso puede estar
    /// bloqueado o el escaneo puede haber llegado antes de que se montara el árbol. Se
    /// mantiene la sesión abierta y se reintenta; solo tras varias pasadas seguidas sin
    /// verlo se pasa a `Terminada`.
    VisibilidadPerdida,
    /// Se observó antes, pero el proceso ya no está: la sesión terminó.
    Terminada,
    /// No se buscó (observador desactivado por el usuario).
    #[default]
    NoObservada,
}

impl EvidenciaProceso {
    /// ¿Esta situación autoriza a decir «el juego no está corriendo»?
    ///
    /// Solo `Terminada`: sabemos que existía y que se fue. Con `SinVisibilidad` la
    /// respuesta honesta es «no se puede saber».
    pub fn permite_afirmar_que_no_corre(&self) -> bool {
        matches!(self, EvidenciaProceso::Terminada)
    }

    /// ¿La sesión sigue considerándose en curso?
    ///
    /// `VisibilidadPerdida` cuenta: el juego puede seguir ahí y solo haberse perdido de
    /// vista. Cerrar por una pasada sin verlo inventaría finalizaciones.
    pub fn sigue_en_curso(&self) -> bool {
        matches!(
            self,
            EvidenciaProceso::Observada | EvidenciaProceso::VisibilidadPerdida
        )
    }
}

// ==================== IDENTIDAD DEL JUEGO ====================

/// De dónde salió el AppID de una sesión.
///
/// Una coincidencia por nombre **no** es lo mismo que el AppID que el juego puso en su
/// entorno, y ambas cosas son distintas de que el usuario lo confirme a mano.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrigenAppId {
    /// `SteamAppId`/`SteamGameId` leídos del entorno del proceso observado.
    EntornoDelProceso,
    /// Del ejecutable observado (coincide con un juego de la biblioteca).
    Ejecutable,
    /// De la carpeta `steamapps/common/<installdir>` donde vive el ejecutable.
    DirectorioDeInstalacion,
    /// Sin observación directa: el nombre del ejecutable o del CSV coincide con **un
    /// único** juego de la biblioteca. Es una inferencia, no una observación.
    InferidoPorNombre,
    /// Del catálogo multi-lanzador (Lutris, Heroic, Heronite o Steam), cruzando la carpeta
    /// declarada, el prefijo de Wine o el ejecutable configurado.
    ///
    /// No lleva `appid` de Steam porque el juego puede no ser de Steam: la identidad es el
    /// título del catálogo, no un AppID. Es una observación, no una inferencia por nombre.
    CatalogoDeLanzador,
    /// Lo fijó el usuario a mano.
    ConfimadoPorUsuario,
    /// No se pudo determinar.
    #[default]
    Desconocido,
}

impl OrigenAppId {
    /// ¿Es una observación directa (el AppID se leyó tal cual del proceso)?
    pub fn es_observacion_directa(&self) -> bool {
        matches!(
            self,
            OrigenAppId::EntornoDelProceso
                | OrigenAppId::Ejecutable
                | OrigenAppId::DirectorioDeInstalacion
                | OrigenAppId::CatalogoDeLanzador
        )
    }
}

/// Identidad del juego de una sesión, con la incertidumbre a la vista.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct IdentidadDeSesion {
    /// AppID si se conoce. Un candidato no confirmado no se escribe aquí.
    pub appid: Option<u32>,
    /// Cómo se obtuvo `appid`.
    pub origen: OrigenAppId,
    /// Candidatos que quedaron sin resolver. Si hay más de uno, `appid` es `None`.
    #[serde(default)]
    pub candidatos: Vec<u32>,
    /// Nombre del juego tal como aparecía en la evidencia (ejecutable o CSV).
    ///
    /// Es el nombre **observado**, no el comercial: puede ser `Palworld-Win64-Shipping.exe`.
    /// No sirve para enseñarlo al usuario, solo para comparar y auditar.
    pub nombre_observado: Option<String>,
    /// Nombre **comercial** del juego, resuelto por AppID contra la biblioteca de Steam.
    ///
    /// Es el único nombre que la interfaz puede enseñar como «el juego». Si el AppID no
    /// consta en la biblioteca, queda `None`: se dice «identidad desconocida» en vez de
    /// caer al nombre del ejecutable o del envoltorio.
    #[serde(default)]
    pub nombre_comercial: Option<String>,
    /// Qué evidencia se reunió, en palabras, para poder auditar la decisión.
    #[serde(default)]
    pub evidencia: Vec<String>,
}

impl IdentidadDeSesion {
    /// Identidad desconocida: no se sabe de qué juego era.
    ///
    /// Se usa cuando no hay prueba suficiente, **aunque** la interfaz tenga un juego
    /// abierto: eso no es evidencia.
    pub fn desconocida() -> Self {
        Self::default()
    }

    pub fn con_appid(appid: u32, origen: OrigenAppId) -> Self {
        Self {
            appid: Some(appid),
            origen,
            ..Self::default()
        }
    }

    /// ¿La identidad es una observación directa, o solo una inferencia?
    pub fn es_observada(&self) -> bool {
        self.appid.is_some() && self.origen.es_observacion_directa()
    }

    /// ¿Quedó ambigua (varios candidatos y ninguno confirmado)?
    pub fn es_ambigua(&self) -> bool {
        self.appid.is_none() && self.candidatos.len() > 1
    }
}

// ==================== PROCESO OBSERVADO ====================

/// Lo que se pudo leer de un proceso del juego.
///
/// Solo nombres y datos mínimos: nunca el entorno completo, ni credenciales, ni rutas
/// personales. Ver la lista blanca en [`crate::proceso_linux`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProcesoObservado {
    pub pid: u32,
    /// Instante de arranque según el sistema (campo 22 de `/proc/<pid>/stat` convertido a
    /// fecha). Es lo que hace al proceso identificable aunque el PID se reutilice.
    pub inicio_del_proceso: Option<String>,
    /// Momento en que Thoriapp lo vio por primera vez. Puede ser mucho después de que
    /// arrancara (si Thoriapp se abrió después).
    pub primera_observacion: Option<String>,
    /// Nombre del ejecutable (basename), no la ruta completa.
    pub ejecutable: Option<String>,
    /// AppID que el propio proceso lleva en su entorno, si lo lleva.
    pub appid_del_entorno: Option<EvidenciaAppId>,
    /// ¿El proceso parece correr bajo Proton/Wine?
    pub bajo_proton: Option<bool>,
    /// ¿Es un proceso hijo de Steam?
    pub hijo_de_steam: Option<bool>,
}

impl ProcesoObservado {
    /// Clave que identifica la ejecución aunque el PID se reutilice o el sistema se
    /// reinicie: `pid` + instante de arranque.
    pub fn clave(&self) -> String {
        match &self.inicio_del_proceso {
            Some(t) => format!("{}@{}", self.pid, t),
            // Sin instante de arranque no hay forma de distinguir dos usos del mismo PID:
            // se dice explícitamente que falta en vez de inventar uno.
            None => format!("{}@desconocido", self.pid),
        }
    }
}

/// `SteamAppId`/`SteamGameId` tal como aparecían en el entorno del proceso.
///
/// Se guardan los dos por separado porque **pueden discrepar**: `SteamAppId` a veces
/// identifica al cliente de Steam y no al juego. Si discrepan y no hay otra evidencia, se
/// conserva la contradicción en vez de elegir uno.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EvidenciaAppId {
    pub steam_app_id: Option<String>,
    pub steam_game_id: Option<String>,
}

impl EvidenciaAppId {
    pub fn vacia(&self) -> bool {
        self.steam_app_id.is_none() && self.steam_game_id.is_none()
    }

    /// AppID consensuado por las dos variables.
    ///
    /// `Ok(Some(id))` si coinciden, `Ok(None)` si no hay ninguna, `Err(ids)` si discrepan.
    /// El error no es un fallo: es el resultado honesto cuando la evidencia se contradice.
    ///
    /// **`0` es ausencia, no identidad.** UMU y algunos lanzadores exportan
    /// `SteamAppId=0`/`SteamGameId=0` para decir «este juego no es de Steam». Aceptarlo como
    /// identificador hacía que todos los juegos externos compartieran el AppID 0 y que la
    /// identidad resolviera a un juego que no es. Un `0` no identifica ningún juego de Steam,
    /// así que se devuelve `None`, igual que si la variable no estuviera.
    pub fn appid_consensuado(&self) -> Result<Option<u32>, (Option<String>, Option<String>)> {
        let valido = |v: &str| match v.trim().parse::<u32>() {
            Ok(0) => None,
            otro => otro.ok(),
        };
        match (&self.steam_app_id, &self.steam_game_id) {
            (None, None) => Ok(None),
            (Some(a), None) => Ok(valido(a)),
            (None, Some(g)) => Ok(valido(g)),
            // Dos ceros coinciden, pero siguen sin ser una identidad.
            (Some(a), Some(g)) if a.trim() == g.trim() => Ok(valido(a)),
            (Some(a), Some(g)) => Err((Some(a.clone()), Some(g.clone()))),
        }
    }
}

// ==================== CONFIGURACIÓN CAPTURADA ====================

/// Configuración de Thoriapp tal como estaba en un instante concreto.
///
/// Se guarda con su instante para no hacerla pasar por «lo que estaba activo durante el
/// juego». Capturarla después de que el juego arrancara no prueba nada sobre el arranque.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ConfiguracionCapturada {
    /// Comando confirmado por el motor de Rust.
    pub comando_confirmado: Option<ConProcedencia<String>>,
    /// Perfil cargado en Thoriapp.
    pub perfil: Option<ConProcedencia<String>>,
    /// Revisión del motor. `None` mientras el motor no publique una revisión estable:
    /// no se inventa un número.
    pub revision_motor: Option<u32>,
    /// Texto de `LaunchOptions` que Steam tenía en ese momento.
    pub opciones_steam: Option<ConProcedencia<String>>,
    /// Cuándo se tomó esta foto (`YYYY-MM-DD HH:MM:SS`).
    pub capturado_en: String,
    /// ¿La foto se tomó **después** de que arrancara el proceso observado?
    ///
    /// Si es `true`, la configuración describe lo que había puesto más tarde, no
    /// necesariamente lo que se usó al arrancar. La interfaz no debe presentarla como
    /// «comando de arranque».
    pub posterior_al_arranque: bool,
}

// ==================== RASGOS DE PROCESO (entrada del puerto) ====================

/// Datos mínimos de un proceso, tal como los entrega el puerto.
///
/// Es deliberadamente pequeño: el adaptador real filtra en `/proc` antes de construir
/// esto, y solo lee `environ` cuando ya hay un candidato claro.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RasgosDeProceso {
    pub pid: u32,
    /// Nombre del ejecutable (basename).
    pub ejecutable: Option<String>,
    /// Ruta completa del ejecutable, solo para reconocer `steamapps/common`.
    pub ruta_ejecutable: Option<String>,
    /// Instante de arranque ya formateado.
    pub inicio_del_proceso: Option<String>,
    pub ppid: Option<u32>,
    /// AppID del entorno. `None` si no se consultó (para no leer `environ` sin motivo).
    pub appid_del_entorno: Option<EvidenciaAppId>,
    /// ¿Corre **específicamente** bajo Proton? `None` si no se pudo determinar.
    ///
    /// Es información de **runtime**, no de clasificación: Wine genérico no es Proton.
    pub bajo_proton: Option<bool>,
    /// ¿Ejecuta un binario Windows bajo **una capa de compatibilidad** (Wine o Proton)?
    ///
    /// Es la pregunta que necesita la detección, y es distinta de `bajo_proton`: Star Valor
    /// (Lutris + Wine) tiene `bajo_compatibilidad = true` y `bajo_proton = false`.
    pub bajo_compatibilidad: Option<bool>,
    /// Nombre del ejecutable **del juego**, que en Proton no es el ejecutable Linux.
    ///
    /// En un lanzamiento Proton el proceso visible desde `/proc` corre el preloader de
    /// Wine; el juego de verdad (`.exe` de Windows) aparece en los argumentos. Guardar los
    /// dos por separado es lo que evita decir «el juego es wine64-preloader».
    pub ejecutable_del_juego: Option<String>,
    /// Ruta del ejecutable del juego tal como se leyó (puede ser de Windows).
    ///
    /// De una ruta Windows solo se conserva el **nombre del fichero**: el resto es la ruta
    /// del usuario y no hace falta para nada.
    pub ruta_del_juego: Option<String>,
    /// La carpeta que el proceso **declara** como su instalación
    /// (`STEAM_COMPAT_INSTALL_PATH`, ya reducida a `steamapps/common/<carpeta>`).
    ///
    /// Es la única evidencia explícita de «esta es la instalación de la que salgo». Bajo
    /// Proton el ejecutable Linux es el preloader de Wine, cuya ruta está en
    /// `compatibilitytools.d/`, no en `steamapps/common`; sin esta variable la carpeta solo
    /// podía deducirse de la línea de comandos, que cualquier envoltorio también menciona.
    pub carpeta_declarada: Option<String>,
    /// Prefijo de Wine que el proceso declara en su entorno (`WINEPREFIX`), ya normalizado.
    ///
    /// Es la segunda vía de correlación con el catálogo, y la que queda cuando el lanzador no
    /// guarda carpeta de instalación: en Lutris `directory` viene vacío en casi todos los
    /// juegos de Wine, así que sin el prefijo no habría nada que comparar.
    ///
    /// `None` cuando el proceso no lo lleva o no se pudo leer: la ausencia no es un descarte.
    pub wine_prefix: Option<String>,
    /// Directorio de trabajo del proceso (`/proc/<pid>/cwd`), sin normalizar.
    ///
    /// Es la única evidencia que tienen los juegos que arrancan por un **intérprete**: el
    /// script de Lutris lanza `bash starsector.sh`, y ese a su vez `java …`. Ni el entorno ni
    /// la línea de comandos de esos procesos dicen qué juego son; su directorio de trabajo sí,
    /// porque arrancan desde la carpeta del juego. Por sí solo no confirma nada: un `bash` en
    /// `/` también tiene un `cwd`.
    pub cwd: Option<String>,
    /// El proceso es un auxiliar del juego (wineserver, winedevice, y similares), no el
    /// juego. Nunca abre sesión por sí mismo.
    pub auxiliar: bool,
    /// AppID leído de la línea de comandos (`AppId=1623730` del `reaper` de Steam).
    ///
    /// Es evidencia del lanzamiento, no del entorno: sirve cuando el proceso del juego no
    /// heredó `SteamAppId`, que es lo habitual en un árbol de Proton.
    pub appid_de_argumentos: Option<u32>,
    /// ¿El `.exe` de `ejecutable_del_juego` lo **ejecuta** este proceso, o solo aparece
    /// mencionado en su línea de comandos?
    ///
    /// Es la diferencia entre «este proceso es el juego» y «este proceso pasa el juego a
    /// otro». El envoltorio de Proton (`pv-adverb`, `srt-bwrap`, el script `proton`) lleva
    /// `…/Deskrawl.exe` como argumento porque se lo reenvía al runtime; el proceso del juego
    /// es el que recibe ese `.exe` ya resuelto.
    ///
    /// Mencionar un `.exe` **no** demuestra ejecutarlo, así que esta distinción decide quién
    /// puede ser representante. `None` cuando no se puede afirmar (el dato no se leyó).
    pub ejecuta_de_verdad: Option<bool>,
}

/// Catálogo de juegos conocidos, mínimo, para cruzar evidencia.
///
/// Se construye una vez por pasada desde `steam::scan_library`; aquí es un dato plano para
/// que el dominio no dependa de la E/S de Steam.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JuegoDeBiblioteca {
    pub appid: u32,
    pub nombre: String,
    /// Carpeta dentro de `steamapps/common`, si se conoce.
    pub installdir: Option<String>,
}

/// Resultado de buscar el proceso del juego.
#[derive(Debug, Clone, PartialEq)]
pub enum BusquedaDeProceso {
    /// Un proceso claro.
    Encontrado(Box<RasgosDeProceso>),
    /// Ningún candidato.
    Ninguno,
    /// Varios candidatos igual de plausibles: no se elige.
    Ambiguo(Vec<u32>),
    /// No se pudo mirar (contenedor, permisos).
    SinVisibilidad(String),
}

/// Puerto de observación de procesos.
///
/// La implementación real lee `/proc`; la de pruebas devuelve lo que se le diga. Nada más
/// del módulo sabe de Linux.
pub trait ObservadorDeProcesos: Send + Sync {
    /// Mira `/proc` y devuelve los procesos que podrían ser un juego, ya agrupados.
    ///
    /// **No recibe un nombre**: cuando la 5.3 pedía «busca el proceso llamado X», solo
    /// encontraba juegos nativos que se llamaran X desde Linux. Un juego bajo Proton corre
    /// un preloader de Wine cuyo `comm` es `GameThread` y cuya ruta está dentro de
    /// `compatibilitytools.d`; el nombre X solo aparece en los argumentos. La detección
    /// tiene que mirar la evidencia, no esperar que le digan qué buscar.
    fn escanear(&self) -> EscaneoDeProcesos;

    /// ¿El proceso de `clave` sigue vivo? Se usa para cerrar sesiones.
    fn sigue_vivo(&self, clave: &str) -> bool;
}

/// Lo que una pasada de `/proc` ha permitido ver.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EscaneoDeProcesos {
    /// Grupos de procesos que podrían ser un juego (uno por juego, no uno por proceso).
    pub grupos: Vec<GrupoDeProcesos>,
    /// Procesos del mismo usuario que no se pudieron leer (permisos). No es un fallo: es
    /// el motivo por el que puede faltar evidencia.
    pub inaccesibles: usize,
    /// No se pudo mirar nada (sin `/proc`, contenedor aislado, sin UID propio).
    pub sin_visibilidad: Option<String>,
}

/// Un juego y los procesos que lo componen.
///
/// Un juego bajo Proton son varios procesos: el preloader que se ve desde Linux, el
/// `.exe` del juego y sus hilos con nombre propio. Son la **misma** sesión; si cada uno
/// abriera la suya, la lista de pruebas se llenaría de fantasmas.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GrupoDeProcesos {
    /// Procesos del grupo, ya recolectados. Uno de ellos es el principal.
    pub miembros: Vec<RasgosDeProceso>,
    /// PID del proceso que representa al juego.
    pub principal: u32,
    /// Evidencia reunida, en texto legible, para poder explicar la decisión.
    pub evidencia: Vec<String>,
    /// AppID acordado por la evidencia del grupo, si lo hay.
    pub appid: Option<u32>,
    /// El AppID se contradice entre miembros.
    pub appid_contradictorio: bool,
    /// Nombres de juego candidatos (del ejecutable Windows o del nativo), únicos.
    pub nombres: Vec<String>,
    /// Rutas `steamapps/common/<dir>` vistas en el grupo.
    pub install_dirs: Vec<String>,
    /// ¿Algún miembro va bajo Proton?
    pub bajo_proton: bool,
}

// ==================== SESIÓN ====================

/// Qué tipo de sesión es. Distinguir estas dos cosas evita presentar una agrupación por
/// nombre como si fuera una ejecución observada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoDeSesion {
    /// Se observó un proceso y se le siguió.
    Observada,
    /// Grabaciones del mismo juego agrupadas sin proceso identificado.
    #[default]
    AgrupadaSinProceso,
}

/// Una ejecución de un juego, con lo que se pudo observar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SesionDeEjecucion {
    pub id: String,
    pub tipo: TipoDeSesion,
    pub identidad: IdentidadDeSesion,
    pub proceso: Option<ProcesoObservado>,
    pub evidencia_proceso: EvidenciaProceso,
    /// Configuración de Thoriapp capturada **al abrir la sesión** (cuando se vio el
    /// proceso por primera vez). No se reescribe después.
    pub configuracion_inicial: Option<ConfiguracionCapturada>,
    /// Grabaciones atribuidas (ids de benchmark).
    #[serde(default)]
    pub grabaciones: Vec<String>,
    /// Primera vez que se vio.
    pub abierta_en: String,
    /// Última vez que se vio el proceso vivo. `None` mientras siga abierta.
    pub ultima_observacion: Option<String>,
    /// Claves (`pid@starttime`) de **todos** los procesos vistos en la ejecución, no solo del
    /// representante.
    ///
    /// Es lo que da continuidad a la sesión: una ejecución sigue siendo la misma mientras
    /// viva cualquiera de sus procesos. El representante puede cambiar entre pasadas sin que
    /// cambie la ejecución, y confundir ambos fue el fallo real de Deskrawl (210465 → 210612,
    /// una sola partida, dos sesiones).
    ///
    /// Vacío en sesiones guardadas antes de existir este campo: para ellas se sigue usando la
    /// clave del representante, sin reescribir nada histórico.
    #[serde(default)]
    pub vivos_de_la_ejecucion: Vec<String>,
    /// Claves (`pid@starttime`) del **árbol de lanzamiento** visto al abrir la sesión.
    ///
    /// Son los envoltorios y auxiliares que rodeaban al juego (`wine64-preloader`, `pv-adverb`,
    /// el propio `dolphin` si el juego se lanzó desde él). **No acreditan ejecución**: un
    /// envoltorio sobrevive a la partida y por eso este campo nunca decide si la sesión sigue
    /// viva.
    ///
    /// Sirve para una sola cosa: reconocer que dos pasadas hablan de la **misma** partida
    /// cuando el representante cambia (`210465` → `210612`, misma ejecución de Deskrawl). Es
    /// la continuidad de instancia, que es distinta de la evidencia de ejecución y no puede
    /// confundirse con ella. Separar ambas fue lo que faltaba: cuando la lista de ejecución
    /// guardaba todo el árbol, la continuidad funcionaba pero también mantenía viva la sesión
    /// para siempre; al filtrarla, la sesión dejaba de mantenerse por los auxiliares, pero se
    /// perdía la señal de continuidad.
    #[serde(default)]
    pub arbol_del_lanzamiento: Vec<String>,
    pub cerrada_en: Option<String>,
    /// Pasadas seguidas en las que no se le ha visto.
    ///
    /// Una sola pasada sin verlo no cierra la sesión: `/proc` puede fallar o el escaneo
    /// llegar antes de que el árbol esté montado. Se cierra tras varias seguidas, y el
    /// contador vuelve a cero en cuanto se le ve otra vez. `serde(default)` para que las
    /// sesiones ya guardadas se lean sin migración.
    #[serde(default)]
    pub pasadas_sin_verlo: u32,
}

impl SesionDeEjecucion {
    /// Sesión observada a partir de un proceso.
    pub fn observada(
        id: String,
        rasgos: RasgosDeProceso,
        identidad: IdentidadDeSesion,
        ahora: &str,
    ) -> Self {
        let proceso = ProcesoObservado {
            pid: rasgos.pid,
            inicio_del_proceso: rasgos.inicio_del_proceso.clone(),
            primera_observacion: Some(ahora.to_string()),
            ejecutable: rasgos.ejecutable.clone(),
            appid_del_entorno: rasgos.appid_del_entorno.clone(),
            bajo_proton: rasgos.bajo_proton,
            // Tener padre no demuestra ser hijo de Steam: eso necesita el árbol de
            // procesos, que aquí no se tiene. Se deja sin afirmar.
            hijo_de_steam: None,
        };
        Self {
            id,
            tipo: TipoDeSesion::Observada,
            identidad,
            proceso: Some(proceso),
            evidencia_proceso: EvidenciaProceso::Observada,
            configuracion_inicial: None,
            grabaciones: Vec::new(),
            abierta_en: ahora.to_string(),
            ultima_observacion: Some(ahora.to_string()),
            cerrada_en: None,
            // Recién vista: ninguna pasada sin verla.
            pasadas_sin_verlo: 0,
            // Solo se conoce al representante; el grupo lo añade el servicio.
            vivos_de_la_ejecucion: Vec::new(),
            // Sin grupo no hay árbol de lanzamiento que recordar.
            arbol_del_lanzamiento: Vec::new(),
        }
    }

    /// Agrupación de grabaciones sin proceso conocido.
    ///
    /// Se marca como tal para que la interfaz no la presente como una ejecución
    /// observada. Es lo que queda cuando el observador no ve nada (Docker, juego cerrado,
    /// grabación importada de otro equipo).
    pub fn agrupada(id: String, identidad: IdentidadDeSesion, ahora: &str) -> Self {
        Self {
            id,
            tipo: TipoDeSesion::AgrupadaSinProceso,
            identidad,
            proceso: None,
            evidencia_proceso: EvidenciaProceso::SinVisibilidad,
            configuracion_inicial: None,
            grabaciones: Vec::new(),
            abierta_en: ahora.to_string(),
            ultima_observacion: None,
            cerrada_en: None,
            pasadas_sin_verlo: 0,
            // Sin proceso no hay ejecución que seguir.
            vivos_de_la_ejecucion: Vec::new(),
            arbol_del_lanzamiento: Vec::new(),
        }
    }

    pub fn esta_abierta(&self) -> bool {
        self.cerrada_en.is_none() && self.evidencia_proceso == EvidenciaProceso::Observada
    }

    /// Clave de identidad del proceso, si lo hay.
    pub fn clave_proceso(&self) -> Option<String> {
        self.proceso.as_ref().map(|p| p.clave())
    }
}

// ==================== ASOCIACIÓN DE GRABACIONES ====================

/// Qué se sabe de la relación entre una grabación y una sesión.
///
/// No es un booleano a propósito: «no se pudo comprobar» y «no coincide» son cosas
/// distintas, y confundirlas es justo lo que produce atribuciones falsas.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asociacion {
    /// La grabación empezó dentro de la vida del proceso. Es lo más fuerte que puede
    /// lograrse sin instrumentalizar el juego.
    Verificada,
    /// Encaja razonablemente, pero con margen de error (por ejemplo, el proceso es
    /// anterior a Thoriapp y no se sabe cuándo empezó la grabación con precisión).
    Probable,
    /// Una sola sesión posible para ese juego, sin datos del proceso.
    UnicaSesionDelJuego,
    /// Varias sesiones plausibles: no se elige.
    Ambigua,
    /// Hay proceso, pero la grabación es muy anterior o posterior: no se atribuye.
    NoCoincide,
    /// No hay forma de comprobarlo (sin proceso observado, grabación importada).
    #[default]
    NoComprobable,
}

impl Asociacion {
    /// ¿Se puede guardar la grabación con esta sesión sin mentir?
    ///
    /// `Ambigua` y `NoCoincide` no se atribuyen automáticamente. `NoComprobable` sí
    /// permite guardar, pero como agrupación sin proceso.
    pub fn permite_atribuir(&self) -> bool {
        matches!(
            self,
            Asociacion::Verificada | Asociacion::Probable | Asociacion::UnicaSesionDelJuego
        )
    }
}

/// Margen tolerado entre el nombre del CSV y la ventana del proceso.
///
/// El nombre del fichero lleva la hora de **inicio** de la grabación con resolución de
/// segundos, y `elapsed` del CSV dice cuánto duró. Con eso se reconstruye la ventana. Los
/// 60 s de holgura absorben cierres lentos, diferencias de reloj y el propio sondeo.
pub const MARGEN_ASOCIACION_SEGUNDOS: i64 = 60;

/// Ventana temporal de una grabación, reconstruida con lo que se sabe.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VentanaDeGrabacion {
    /// Inicio según el nombre del fichero (`YYYY-MM-DD HH:MM:SS`).
    pub inicio: Option<String>,
    /// Duración grabada, según `elapsed` del CSV.
    pub duracion_segundos: Option<f64>,
}

impl VentanaDeGrabacion {
    /// Fin estimado. `None` si falta algún dato: no se supone una duración.
    pub fn fin_estimado(&self) -> Option<String> {
        let inicio = self.inicio.as_ref()?;
        let dur = self.duracion_segundos?;
        let t = parse_fecha(inicio)?;
        Some(epoch_a_fecha(t + dur as i64))
    }
}

/// `YYYY-MM-DD HH:MM:SS` → segundos desde la época (UTC, tratando la fecha como UTC para
/// que la resta sea consistente; ambas fechas salen de la misma fuente).
pub fn parse_fecha(s: &str) -> Option<i64> {
    let s = s.trim();
    // Solo se cambia el separador entre la fecha y la hora: los guiones de la fecha y los
    // dos puntos de la hora se quedan. El nombre del CSV usa `_` entre ambas
    // (`2026-11-24_18-19-57`), y ese `_` es el único que se convierte en espacio.
    let normalizada = match s.split_once('_') {
        Some((fecha, resto)) => format!("{} {}", fecha, resto.replace('_', ":")),
        None => s.to_string(),
    };
    let s = normalizada;
    // La hora del nombre del CSV viene con guiones (`18-19-57`); la del resto de la app, con
    // dos puntos. Se aceptan las dos formas en el segundo campo.
    let (fecha, hora) = s.split_once(' ')?;
    let hora = if hora.contains('-') {
        hora.replace('-', ":")
    } else {
        hora.to_string()
    };
    let hora = hora.as_str();
    let mut fd = fecha.split('-');
    let (y, m, d) = (
        fd.next()?.parse::<i32>().ok()?,
        fd.next()?.parse::<u32>().ok()?,
        fd.next()?.parse::<u32>().ok()?,
    );
    let mut fh = hora.split(':');
    let (hh, mm) = (
        fh.next()?.parse::<u32>().ok()?,
        fh.next()?.parse::<u32>().ok()?,
    );
    // Los segundos pueden venir con fracción (`HH:MM:SS.mmm`).
    let ss = fh.next().unwrap_or("0");
    let ss = ss.split('.').next()?.parse::<u32>().ok()?;
    if !(1..=12).contains(&m) || d == 0 || d > 31 || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    let dias = dias_desde_epoca(y, m, d);
    Some(dias * 86_400 + hh as i64 * 3_600 + mm as i64 * 60 + ss as i64)
}

/// Días desde 1970-01-01 para una fecha civil. Algoritmo de Howard Hinnant, sin tablas.
fn dias_desde_epoca(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as i64;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146_097 + doe - 719_468
}

/// Segundos desde la época → `YYYY-MM-DD HH:MM:SS`.
pub fn epoch_a_fecha(t: i64) -> String {
    let dias = t.div_euclid(86_400);
    let resto = t.rem_euclid(86_400);
    let (y, m, d) = fecha_desde_dias(dias);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m,
        d,
        resto / 3_600,
        (resto % 3_600) / 60,
        resto % 60
    )
}

fn fecha_desde_dias(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// Decide si una grabación puede atribuirse a una sesión.
///
/// Se apoya en dos cosas, ninguna infalible por separado:
///
/// 1. **Temporal**: la ventana de la grabación cae dentro de la vida del proceso (con
///    holgura). Una grabación de hace un mes no encaja en un proceso de hace una hora,
///    aunque el juego sea el mismo.
/// 2. **Identidad**: el juego de la grabación y el de la sesión tienen que poder ser el
///    mismo. Si no se sabe ninguno de los dos, no hay atribución automática.
pub fn decidir_asociacion(
    sesion: &SesionDeEjecucion,
    appid_grabacion: Option<u32>,
    nombre_grabacion: &str,
    ventana: &VentanaDeGrabacion,
) -> Asociacion {
    // Una grabación de la que no se sabe nada no se puede comprobar.
    let Some(inicio) = ventana.inicio.as_deref().and_then(parse_fecha) else {
        return Asociacion::NoComprobable;
    };

    // Identidad: si ambos lados dicen un AppID distinto, no es la misma ejecución.
    if let (Some(a), Some(b)) = (appid_grabacion.filter(|a| *a != 0), sesion.identidad.appid) {
        if a != b {
            return Asociacion::NoCoincide;
        }
    }

    match &sesion.proceso {
        Some(proc) => {
            // Sin instante de arranque no hay ventana que comparar: como mucho, probable.
            let Some(arranque) = proc.inicio_del_proceso.as_deref().and_then(parse_fecha) else {
                return Asociacion::Probable;
            };
            let fin_grabacion = inicio + ventana.duracion_segundos.unwrap_or(0.0) as i64;
            // El final de la sesión, si se conoce, también cuenta como límite.
            let fin_sesion = sesion
                .cerrada_en
                .as_deref()
                .and_then(parse_fecha)
                .or_else(|| sesion.ultima_observacion.as_deref().and_then(parse_fecha));

            // La grabación tiene que empezar después del arranque (con holgura por los
            // desfases de reloj) y no después de que la sesión terminara.
            let empieza_despues_del_arranque = inicio + MARGEN_ASOCIACION_SEGUNDOS >= arranque;
            let empieza_antes_del_fin = match fin_sesion {
                Some(fin) => inicio <= fin + MARGEN_ASOCIACION_SEGUNDOS,
                None => true,
            };
            if empieza_despues_del_arranque && empieza_antes_del_fin {
                // Si además se sabe que la grabación cabía entera dentro del proceso, es
                // lo más firme que se puede decir.
                if fin_sesion.map(|f| fin_grabacion <= f + MARGEN_ASOCIACION_SEGUNDOS) == Some(true)
                {
                    Asociacion::Verificada
                } else {
                    Asociacion::Probable
                }
            } else {
                Asociacion::NoCoincide
            }
        }
        None => {
            // Sin proceso: solo se justifica agrupar si hay identidad y el nombre encaja.
            let nombre_encaja = sesion
                .identidad
                .nombre_observado
                .as_deref()
                .map(|n| nombres_compatibles(n, nombre_grabacion))
                .unwrap_or(false);
            if sesion.identidad.appid.is_some() && nombre_encaja {
                Asociacion::UnicaSesionDelJuego
            } else {
                Asociacion::NoComprobable
            }
        }
    }
}

// ==================== NORMALIZACIÓN DE NOMBRES ====================

/// Sufijos técnicos que MangoHud y los ejecutables añaden al nombre del juego. Es la misma
/// lista que usa `benchmarks`, aplicada aquí para poder comparar sin acoplar módulos.
const SUFIJOS_TECNICOS: &[&str] = &[
    "-win64-shipping",
    "-win32-shipping",
    "-shipping",
    "-win64",
    "-win32",
    "-x64",
    "-linux64",
    "-linux",
    "-macos",
    "-mac",
    "-client",
    "-server",
    ".x86_64",
    ".x86",
    ".exe",
    ".bin",
    ".sh",
];

/// Normaliza un nombre de juego para comparar.
///
/// Convierte a minúsculas y **elimina** espacios, guiones y guiones bajos, porque unos
/// sitios escriben `7 Days to Die`, otros `7DaysToDie` y otros `7-days-to-die`, y los tres
/// son el mismo juego. También quita el sufijo técnico (`-Win64-Shipping`, `.x86_64`).
///
/// Lo que **no** hace: decidir a qué juego pertenece el resultado. Eso es cosa de
/// [`resolver_identidad`], que exige que la coincidencia sea única.
pub fn normalizar_nombre(nombre: &str) -> String {
    let mut n = nombre.trim().to_lowercase();
    // Sufijo primero, sobre el nombre con separadores: `Palworld-Win64-Shipping`.
    for suf in SUFIJOS_TECNICOS {
        if n.ends_with(suf) {
            n.truncate(n.len() - suf.len());
            break;
        }
    }
    n.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
}

/// ¿Dos nombres pueden referirse al mismo juego?
///
/// Se compara la forma normalizada y se admite que uno empiece por el otro, pero **solo si
/// el trozo añadido no lo convierte en otro juego distinto**: se exige que el más corto
/// tenga al menos 4 caracteres y que el más largo no aporte más de 12 caracteres de cola.
/// Con eso `palworld` casa con `palworldwin64shipping` pero `doom` no casa con `doometernal`.
pub fn nombres_compatibles(a: &str, b: &str) -> bool {
    let na = normalizar_nombre(a);
    let nb = normalizar_nombre(b);
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    if na == nb {
        return true;
    }
    let (corto, largo) = if na.len() <= nb.len() {
        (&na, &nb)
    } else {
        (&nb, &na)
    };
    if corto.len() < 4 || !largo.starts_with(corto.as_str()) {
        return false;
    }
    // Lo que sobra solo puede ser un sufijo de forma de ejecutable, no otro título:
    // `PalworldWin64Shipping` sí, `DoomEternal` no. Se exige que el resto empiece por una
    // palabra técnica conocida; si no, serían dos juegos distintos que comparten prefijo.
    let resto = &largo[corto.len()..];
    const CONTINUACIONES: &[&str] = &[
        "win64",
        "win32",
        "shipping",
        "x64",
        "x86",
        "linux",
        "macos",
        "mac",
        "client",
        "server",
        "shippingexe",
        "bg",
        "game",
    ];
    CONTINUACIONES.iter().any(|c| resto.starts_with(c))
}

/// Resuelve la identidad del juego cruzando las evidencias disponibles.
///
/// Reglas, en orden de fuerza:
///
/// 1. **Entorno del proceso**: si `SteamAppId` y `SteamGameId` coinciden, manda ese AppID.
///    Si discrepan, no se elige: queda ambiguo con los dos candidatos.
/// 2. **Ejecutable**: si la ruta está dentro de `steamapps/common/<installdir>` y ese
///    installdir es **de un solo** juego instalado, se toma su AppID.
/// 3. **Nombre**: si el nombre normalizado casa con **un único** juego de la biblioteca,
///    es `InferidoPorNombre` (una inferencia, no una observación).
/// 4. Varias coincidencias → `Ambiguo`. Ninguna → `Desconocido`.
///
/// El juego abierto en la interfaz **no** entra aquí: no es evidencia.
/// Resuelve la identidad de un juego a partir de la evidencia observada.
///
/// `catalogo` es el catálogo multi-lanzador (Steam + Lutris + Heroic + Heronite), ya
/// cacheado. Es el parámetro que faltaba: sin él, esta función **solo** entendía Steam, y un
/// juego de Lutris o Heronite quedaba sin identificar por mucho que el catálogo lo conociera.
/// Se pasa como `&[EntradaDeCatalogo]` y no se lee de la caché aquí dentro, para que la
/// función siga siendo pura y probable con datos de un equipo real.
pub fn resolver_identidad(
    nombre_observado: Option<&str>,
    rasgos: Option<&RasgosDeProceso>,
    biblioteca: &[JuegoDeBiblioteca],
) -> IdentidadDeSesion {
    resolver_identidad_con_catalogo(nombre_observado, rasgos, biblioteca, &[])
}

/// Igual que `resolver_identidad`, con el catálogo multi-lanzador disponible.
///
/// El orden de las vías no es casual: primero lo que el proceso **declara** (AppID y carpeta
/// de Steam), y solo después el catálogo. Cuando la evidencia directa habla, manda.
pub fn resolver_identidad_con_catalogo(
    nombre_observado: Option<&str>,
    rasgos: Option<&RasgosDeProceso>,
    biblioteca: &[JuegoDeBiblioteca],
    catalogo: &[crate::evidencia_proceso::EntradaDeCatalogo],
) -> IdentidadDeSesion {
    let mut evidencia: Vec<String> = Vec::new();
    let nombre = nombre_observado.map(|n| n.to_string());

    // 1. Entorno del proceso.
    if let Some(env) = rasgos.and_then(|r| r.appid_del_entorno.as_ref()) {
        if !env.vacia() {
            match env.appid_consensuado() {
                Ok(Some(id)) => {
                    evidencia.push(format!("SteamAppId y SteamGameId coinciden en {}", id));
                    // El AppID dice **qué** juego es; el título lo pone el catálogo. Antes se
                    // devolvía sin nombre y el nombre se rellenaba después en el servicio.
                    // Al dejar de pisarlo allí, un juego de Steam se quedaba sin título y la
                    // interfaz mostraba «juego sin identificar»: esa fue la regresión.
                    //
                    // Se resuelve aquí, con el **mismo** catálogo y el mismo criterio con el
                    // que se resuelven los juegos externos: no hay un segundo motor.
                    let nombre_comercial = catalogo
                        .iter()
                        .find(|e| e.id_plataforma.as_deref() == Some(id.to_string().as_str()))
                        .map(|e| e.titulo.clone());
                    if let Some(t) = &nombre_comercial {
                        evidencia.push(format!("el catálogo llama a ese AppID «{}»", t));
                    }
                    return IdentidadDeSesion {
                        appid: Some(id),
                        origen: OrigenAppId::EntornoDelProceso,
                        candidatos: vec![id],
                        nombre_observado: nombre,
                        nombre_comercial,
                        evidencia,
                    };
                }
                Ok(None) => {}
                Err((a, g)) => {
                    // Contradicción: se conservan los dos, no se elige.
                    evidencia.push(format!(
                        "SteamAppId={:?} y SteamGameId={:?} no coinciden",
                        a, g
                    ));
                    let mut cands: Vec<u32> = [a, g]
                        .iter()
                        .flatten()
                        .filter_map(|v| v.trim().parse::<u32>().ok())
                        .collect();
                    cands.sort_unstable();
                    cands.dedup();
                    return IdentidadDeSesion {
                        appid: None,
                        origen: OrigenAppId::Desconocido,
                        candidatos: cands,
                        nombre_observado: nombre,
                        nombre_comercial: None,
                        evidencia,
                    };
                }
            }
        }
    }

    // 2. Directorio de instalación del ejecutable.
    if let Some(ruta) = rasgos.and_then(|r| r.ruta_ejecutable.as_deref()) {
        if let Some(installdir) = installdir_de_ruta(ruta) {
            let coincidencias: Vec<&JuegoDeBiblioteca> = biblioteca
                .iter()
                .filter(|j| {
                    j.installdir
                        .as_deref()
                        .map(|d| normalizar_nombre(d) == normalizar_nombre(&installdir))
                        .unwrap_or(false)
                })
                .collect();
            match coincidencias.as_slice() {
                [unico] => {
                    evidencia.push(format!(
                        "el ejecutable está en steamapps/common/{}",
                        installdir
                    ));
                    return IdentidadDeSesion {
                        appid: Some(unico.appid),
                        origen: OrigenAppId::DirectorioDeInstalacion,
                        candidatos: vec![unico.appid],
                        nombre_observado: nombre,
                        nombre_comercial: None,
                        evidencia,
                    };
                }
                [] => {}
                varios => {
                    let mut cands: Vec<u32> = varios.iter().map(|j| j.appid).collect();
                    cands.sort_unstable();
                    evidencia.push(format!("varios juegos comparten la carpeta {}", installdir));
                    return IdentidadDeSesion {
                        appid: None,
                        origen: OrigenAppId::Desconocido,
                        candidatos: cands,
                        nombre_observado: nombre,
                        nombre_comercial: None,
                        evidencia,
                    };
                }
            }
        }
    }

    // 3 y 4. Por nombre, exigiendo coincidencia única.
    let candidato_nombre = nombre
        .as_deref()
        .or_else(|| rasgos.and_then(|r| r.ejecutable.as_deref()));

    if let Some(n) = candidato_nombre {
        let mut cands: Vec<u32> = biblioteca
            .iter()
            .filter(|j| nombres_compatibles(&j.nombre, n))
            .map(|j| j.appid)
            .collect();
        cands.sort_unstable();
        cands.dedup();
        match cands.as_slice() {
            [unico] => {
                evidencia.push(format!(
                    "el nombre {:?} coincide con un único juego de la biblioteca",
                    n
                ));
                return IdentidadDeSesion {
                    appid: Some(*unico),
                    origen: OrigenAppId::InferidoPorNombre,
                    candidatos: vec![*unico],
                    nombre_observado: nombre,
                    nombre_comercial: None,
                    evidencia,
                };
            }
            [] => {
                evidencia.push(format!(
                    "ningún juego de la biblioteca coincide con {:?}",
                    n
                ));
            }
            varios => {
                evidencia.push(format!(
                    "{:?} coincide con {} juegos: no se elige",
                    n,
                    varios.len()
                ));
                return IdentidadDeSesion {
                    appid: None,
                    origen: OrigenAppId::Desconocido,
                    candidatos: cands,
                    nombre_observado: nombre,
                    nombre_comercial: None,
                    evidencia,
                };
            }
        }
    }

    // 5. Catálogo multi-lanzador: carpeta declarada, prefijo de Wine y ejecutable.
    //
    // Es la única vía que puede identificar un juego que no es de Steam. Devuelve identidad
    // real (no solo el nombre comercial) usando el identificador estable del lanzador.
    if !catalogo.is_empty() {
        let mut coincidencias: Vec<&crate::evidencia_proceso::EntradaDeCatalogo> = Vec::new();

        // 5a. Por la carpeta que el propio proceso declara.
        if let Some(carpeta) = rasgos.and_then(|r| r.carpeta_declarada.as_deref()) {
            coincidencias = catalogo
                .iter()
                .filter(|e| {
                    e.install_dir
                        .as_deref()
                        .map(|d| crate::evidencia_proceso::misma_carpeta(d, carpeta))
                        .unwrap_or(false)
                })
                .collect();
            if !coincidencias.is_empty() {
                evidencia.push(format!(
                    "la carpeta declarada {:?} coincide con el catálogo",
                    carpeta
                ));
            }
        }

        // 5b. Por el prefijo de Wine, solo si la carpeta no decidió.
        if coincidencias.is_empty() {
            if let Some(prefijo) = rasgos.and_then(|r| r.wine_prefix.as_deref()) {
                coincidencias = catalogo
                    .iter()
                    .filter(|e| e.comparte_prefijo(prefijo) == Some(true))
                    .collect();
                if !coincidencias.is_empty() {
                    evidencia.push(format!(
                        "el prefijo de Wine {:?} coincide con el catálogo",
                        prefijo
                    ));
                }
            }
        }

        // 5b bis. Por el directorio de trabajo del proceso.
        //
        // Es la vía de los juegos que arrancan por un intérprete. En el árbol real de
        // Starsector (`lutris-wrapper` → `starsector.sh` → `java`) ninguno de los tres
        // procesos nombra el juego ni lleva variables del lanzador; lo único que los sitúa es
        // que trabajan en la carpeta del juego. Sin esta vía no hay con qué identificarlos.
        if coincidencias.is_empty() {
            if let Some(cwd) = rasgos.and_then(|r| r.cwd.as_deref()) {
                let c = cwd.trim_end_matches('/');
                coincidencias = catalogo
                    .iter()
                    .filter(|e| {
                        [e.install_dir.as_deref(), e.exec.as_deref()]
                            .into_iter()
                            .flatten()
                            .filter_map(crate::evidencia_proceso::ruta_padre)
                            .any(|padre| padre.trim_end_matches('/') == c)
                    })
                    .collect();
                if !coincidencias.is_empty() {
                    evidencia.push(format!(
                        "el directorio de trabajo {:?} es la carpeta del juego en el catálogo",
                        cwd
                    ));
                }
            }
        }

        // 5c. Por el ejecutable configurado del lanzador.
        if coincidencias.is_empty() {
            if let Some(exe) = rasgos
                .and_then(|r| r.ejecutable_del_juego.as_deref())
                .or(nombre.as_deref())
            {
                coincidencias = catalogo
                    .iter()
                    .filter(|e| {
                        e.exec
                            .as_deref()
                            .map(|c| crate::evidencia_proceso::mismo_ejecutable(c, exe))
                            .unwrap_or(false)
                    })
                    .collect();
                if !coincidencias.is_empty() {
                    evidencia.push(format!(
                        "el ejecutable {:?} es el configurado en el catálogo",
                        exe
                    ));
                }
            }
        }

        match coincidencias.as_slice() {
            [] => {}
            [unica] => {
                // Puede ser el mismo juego en dos lanzadores: se agrupa por título, que es
                // la identidad compartida, en vez de por identificador de plataforma.
                return IdentidadDeSesion {
                    appid: None,
                    origen: OrigenAppId::CatalogoDeLanzador,
                    candidatos: Vec::new(),
                    nombre_observado: nombre,
                    nombre_comercial: Some(unica.titulo.clone()),
                    evidencia,
                };
            }
            varias => {
                // Si todas son el mismo juego (Steam + Lutris), no hay ambigüedad.
                let mismo = varias
                    .iter()
                    .all(|e| normalizar_nombre(&e.titulo) == normalizar_nombre(&varias[0].titulo));
                if mismo {
                    return IdentidadDeSesion {
                        appid: None,
                        origen: OrigenAppId::CatalogoDeLanzador,
                        candidatos: Vec::new(),
                        nombre_observado: nombre,
                        nombre_comercial: Some(varias[0].titulo.clone()),
                        evidencia,
                    };
                }
                evidencia.push(format!(
                    "{} juegos del catálogo coinciden: no se elige",
                    varias.len()
                ));
            }
        }
    }

    IdentidadDeSesion {
        appid: None,
        origen: OrigenAppId::Desconocido,
        candidatos: Vec::new(),
        nombre_observado: nombre,
        nombre_comercial: None,
        evidencia,
    }
}

/// Extrae `installdir` de una ruta tipo `.../steamapps/common/<dir>/.../juego.exe`.
fn installdir_de_ruta(ruta: &str) -> Option<String> {
    let partes: Vec<&str> = ruta.split('/').collect();
    let pos = partes.iter().position(|p| *p == "common")?;
    // Se exige que justo antes esté `steamapps`, para no confundirlo con otra `common`.
    if pos == 0 || partes[pos - 1] != "steamapps" {
        return None;
    }
    partes.get(pos + 1).map(|s| s.to_string())
}

// ==================== PERSISTENCIA ====================

/// Lo que se guarda en disco de las sesiones.
///
/// Se limita en tamaño: las sesiones no crecen sin fin. Se conservan las más recientes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SesionesData {
    /// Versión del esquema. Ausente en los ficheros de la primera versión.
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub sesiones: Vec<SesionDeEjecucion>,
    /// Preferencia del usuario sobre el observador de procesos.
    ///
    /// `None` = nunca lo ha decidido (instalación nueva), y entonces manda el valor por
    /// defecto, que es **desactivado**. Se guarda la ausencia en vez de un `false` para poder
    /// distinguir «nunca eligió» de «eligió apagarlo»: es justo lo que no se podía saber
    /// cuando el observador se activaba solo al arrancar.
    #[serde(default)]
    pub observador_activo: Option<bool>,
}

impl SesionesData {
    /// ¿Debe arrancar activo el observador?
    ///
    /// Solo si el usuario lo dejó explícitamente activado. En una instalación nueva no hay
    /// preferencia guardada, y mirar `/proc` sin que nadie lo haya pedido no es aceptable.
    pub fn observador_deberia_estar_activo(&self) -> bool {
        self.observador_activo.unwrap_or(false)
    }
}

/// Máximo de sesiones guardadas. Más allá de esto se descartan las cerradas más antiguas.
pub const MAX_SESIONES: usize = 100;

/// Recorta la lista conservando las más recientes y, a igualdad, las abiertas.
pub fn recortar_sesiones(sesiones: &mut Vec<SesionDeEjecucion>) {
    if sesiones.len() <= MAX_SESIONES {
        return;
    }
    // Abiertas primero (nunca se tiran), luego por fecha de apertura descendente.
    sesiones.sort_by(|a, b| {
        b.esta_abierta()
            .cmp(&a.esta_abierta())
            .then_with(|| b.abierta_en.cmp(&a.abierta_en))
    });
    sesiones.truncate(MAX_SESIONES);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ahora() -> &'static str {
        "2026-11-24 19:00:00"
    }

    // ---------- fechas ----------

    #[test]
    fn las_fechas_van_y_vuelven() {
        let casos = [
            "1970-01-01 00:00:00",
            "2026-11-24 18:19:57",
            "2000-02-29 12:00:00",
            "2026-01-01 00:00:00",
        ];
        for c in casos {
            let t = parse_fecha(c).unwrap();
            assert_eq!(epoch_a_fecha(t), c, "ida y vuelta de {:?}", c);
        }
    }

    #[test]
    fn la_fecha_del_csv_se_entiende_con_guion_bajo() {
        let a = parse_fecha("2026-11-24_18-19-57").unwrap();
        let b = parse_fecha("2026-11-24 18:19:57").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn una_fecha_imposible_no_se_acepta() {
        assert!(parse_fecha("2026-13-01 00:00:00").is_none());
        assert!(parse_fecha("2026-01-01 25:00:00").is_none());
        assert!(parse_fecha("basura").is_none());
        assert!(parse_fecha("").is_none());
    }

    #[test]
    fn el_fin_estimado_necesita_los_dos_datos() {
        let mut v = VentanaDeGrabacion {
            inicio: Some("2026-11-24 18:19:57".into()),
            duracion_segundos: Some(60.0),
        };
        assert_eq!(v.fin_estimado().as_deref(), Some("2026-11-24 18:20:57"));
        // Sin duración no se inventa el final.
        v.duracion_segundos = None;
        assert!(v.fin_estimado().is_none());
    }

    // ---------- normalización ----------

    #[test]
    fn los_tres_formatos_de_7_days_to_die_coinciden() {
        // El defecto que motivó esta función: la biblioteca dice "7 Days to Die" y el CSV
        // dice "7DaysToDie"; sin normalizar los separadores no casaban.
        let a = normalizar_nombre("7DaysToDie");
        assert_eq!(a, "7daystodie");
        assert_eq!(normalizar_nombre("7 Days to Die"), a);
        assert_eq!(normalizar_nombre("7-days-to-die"), a);
        assert!(nombres_compatibles("7DaysToDie", "7 Days to Die"));
        assert!(nombres_compatibles("7DaysToDie.x86_64", "7 Days to Die"));
    }

    #[test]
    fn palworld_casa_aunque_lleve_sufijo_de_ejecutable() {
        assert!(nombres_compatibles("Palworld-Win64-Shipping", "Palworld"));
        assert!(nombres_compatibles("Palworld.exe", "Palworld"));
        assert_eq!(normalizar_nombre("Palworld-Win64-Shipping"), "palworld");
    }

    #[test]
    fn nombres_cortos_no_casan_por_prefijo() {
        // "doom" y "doometernal" son juegos distintos: el prefijo solo vale si el trozo
        // que sobra parece un sufijo técnico, no otro título.
        assert!(!nombres_compatibles("doom", "doometernal"));
        assert!(!nombres_compatibles("d", "doom"));
        assert!(!nombres_compatibles("", ""));
    }

    // ---------- entorno ----------

    #[test]
    fn steam_app_id_y_game_id_que_coinciden_dan_un_appid() {
        let e = EvidenciaAppId {
            steam_app_id: Some("251570".into()),
            steam_game_id: Some("251570".into()),
        };
        assert_eq!(e.appid_consensuado(), Ok(Some(251570)));
    }

    #[test]
    fn si_las_dos_variables_discrepan_no_se_elige() {
        let e = EvidenciaAppId {
            steam_app_id: Some("753".into()),     // el cliente
            steam_game_id: Some("251570".into()), // el juego
        };
        assert!(e.appid_consensuado().is_err());
    }

    #[test]
    fn con_una_sola_variable_se_usa_esa() {
        let e = EvidenciaAppId {
            steam_app_id: None,
            steam_game_id: Some("251570".into()),
        };
        assert_eq!(e.appid_consensuado(), Ok(Some(251570)));
    }

    // ---------- resolución de identidad ----------

    fn biblioteca() -> Vec<JuegoDeBiblioteca> {
        vec![
            JuegoDeBiblioteca {
                appid: 251570,
                nombre: "7 Days to Die".into(),
                installdir: Some("7 Days To Die".into()),
            },
            JuegoDeBiblioteca {
                appid: 1623730,
                nombre: "Palworld".into(),
                installdir: Some("Palworld".into()),
            },
            JuegoDeBiblioteca {
                appid: 1091500,
                nombre: "Cyberpunk 2077".into(),
                installdir: Some("Cyberpunk 2077".into()),
            },
        ]
    }

    #[test]
    fn el_entorno_manda_sobre_el_nombre() {
        let rasgos = RasgosDeProceso {
            pid: 10,
            ejecutable: Some("Palworld-Win64-Shipping.exe".into()),
            appid_del_entorno: Some(EvidenciaAppId {
                steam_app_id: Some("1623730".into()),
                steam_game_id: Some("1623730".into()),
            }),
            ..Default::default()
        };
        let id = resolver_identidad(
            Some("Palworld-Win64-Shipping"),
            Some(&rasgos),
            &biblioteca(),
        );
        assert_eq!(id.appid, Some(1623730));
        assert_eq!(id.origen, OrigenAppId::EntornoDelProceso);
        assert!(
            id.es_observada(),
            "leerlo del proceso sí es observación directa"
        );
    }

    #[test]
    fn un_entorno_contradictorio_deja_la_identidad_ambigua() {
        let rasgos = RasgosDeProceso {
            pid: 11,
            appid_del_entorno: Some(EvidenciaAppId {
                steam_app_id: Some("753".into()),
                steam_game_id: Some("251570".into()),
            }),
            ..Default::default()
        };
        let id = resolver_identidad(Some("7DaysToDie"), Some(&rasgos), &biblioteca());
        assert_eq!(id.appid, None, "no se elige uno de los dos");
        assert!(id.es_ambigua());
        assert_eq!(id.candidatos, vec![753, 251570]);
    }

    #[test]
    fn la_carpeta_de_instalacion_resuelve_sin_entorno() {
        let rasgos = RasgosDeProceso {
            pid: 12,
            ruta_ejecutable: Some(
                "/home/x/.local/share/Steam/steamapps/common/Palworld/Palworld.exe".into(),
            ),
            ..Default::default()
        };
        let id = resolver_identidad(Some("Palworld"), Some(&rasgos), &biblioteca());
        assert_eq!(id.appid, Some(1623730));
        assert_eq!(id.origen, OrigenAppId::DirectorioDeInstalacion);
    }

    #[test]
    fn una_coincidencia_unica_por_nombre_es_solo_inferida() {
        let id = resolver_identidad(Some("7DaysToDie"), None, &biblioteca());
        assert_eq!(id.appid, Some(251570));
        assert_eq!(id.origen, OrigenAppId::InferidoPorNombre);
        assert!(!id.es_observada(), "inferir no es observar");
    }

    #[test]
    fn varios_juegos_parecidos_dejan_la_identidad_ambigua() {
        let biblio = vec![
            JuegoDeBiblioteca {
                appid: 1,
                nombre: "Doom".into(),
                installdir: None,
            },
            JuegoDeBiblioteca {
                appid: 2,
                nombre: "Doom Eternal".into(),
                installdir: None,
            },
        ];
        // Exactamente "Doom" casa con uno: se admite, pero como inferencia.
        let id = resolver_identidad(Some("Doom"), None, &biblio);
        assert_eq!(id.appid, Some(1));
        assert_eq!(id.origen, OrigenAppId::InferidoPorNombre);

        // Y "DoomEternal" es el otro: el prefijo compartido no confunde.
        let id2 = resolver_identidad(Some("DoomEternal"), None, &biblio);
        assert_eq!(id2.appid, Some(2));

        // Un nombre que no es ninguno no resuelve, y «no encontrarlo» no es ambigüedad.
        let id3 = resolver_identidad(Some("JuegoRaro"), None, &biblio);
        assert_eq!(id3.appid, None);
        assert!(!id3.es_ambigua(), "no encontrarlo no es ambigüedad");
    }

    #[test]
    fn sin_evidencia_no_se_inventa_el_juego_abierto() {
        // El caso prohibido: hay un juego seleccionado en la interfaz, pero nada que
        // demuestre que la grabación sea suya.
        let id = resolver_identidad(Some("JuegoDesconocido"), None, &biblioteca());
        assert_eq!(id.appid, None);
        assert_eq!(id.origen, OrigenAppId::Desconocido);
    }

    // ---------- asociación ----------

    fn sesion_con_proceso(arranque: &str, fin: Option<&str>) -> SesionDeEjecucion {
        let mut s = SesionDeEjecucion::observada(
            "s1".into(),
            RasgosDeProceso {
                pid: 100,
                inicio_del_proceso: Some(arranque.into()),
                ..Default::default()
            },
            IdentidadDeSesion::con_appid(251570, OrigenAppId::EntornoDelProceso),
            arranque,
        );
        s.cerrada_en = fin.map(|f| f.to_string());
        if let Some(f) = fin {
            s.ultima_observacion = Some(f.to_string());
        }
        s
    }

    fn ventana(inicio: &str, dur: f64) -> VentanaDeGrabacion {
        VentanaDeGrabacion {
            inicio: Some(inicio.into()),
            duracion_segundos: Some(dur),
        }
    }

    #[test]
    fn una_grabacion_dentro_del_proceso_se_verifica() {
        let s = sesion_con_proceso("2026-11-24 18:00:00", Some("2026-11-24 18:30:00"));
        let a = decidir_asociacion(
            &s,
            Some(251570),
            "7DaysToDie",
            &ventana("2026-11-24 18:10:00", 60.0),
        );
        assert_eq!(a, Asociacion::Verificada);
    }

    #[test]
    fn una_grabacion_muy_posterior_no_se_atribuye() {
        // El juego estuvo abierto a las 18:00 y la grabación es de dos horas después.
        let s = sesion_con_proceso("2026-11-24 18:00:00", Some("2026-11-24 18:30:00"));
        let a = decidir_asociacion(
            &s,
            Some(251570),
            "7DaysToDie",
            &ventana("2026-11-24 20:00:00", 60.0),
        );
        assert_eq!(a, Asociacion::NoCoincide);
    }

    #[test]
    fn una_grabacion_anterior_al_arranque_no_se_atribuye() {
        let s = sesion_con_proceso("2026-11-24 18:00:00", None);
        let a = decidir_asociacion(
            &s,
            Some(251570),
            "7DaysToDie",
            &ventana("2026-11-24 17:00:00", 60.0),
        );
        assert_eq!(a, Asociacion::NoCoincide);
    }

    #[test]
    fn un_juego_distinto_no_se_atribuye_aunque_coincida_la_hora() {
        let s = sesion_con_proceso("2026-11-24 18:00:00", Some("2026-11-24 18:30:00"));
        let a = decidir_asociacion(
            &s,
            Some(1623730),
            "Palworld",
            &ventana("2026-11-24 18:10:00", 60.0),
        );
        assert_eq!(a, Asociacion::NoCoincide);
    }

    #[test]
    fn sin_proceso_solo_se_agrupa_si_el_nombre_encaja() {
        let s = SesionDeEjecucion::agrupada(
            "g1".into(),
            IdentidadDeSesion {
                appid: Some(251570),
                origen: OrigenAppId::InferidoPorNombre,
                candidatos: vec![251570],
                nombre_observado: Some("7DaysToDie".into()),
                nombre_comercial: None,
                evidencia: vec![],
            },
            ahora(),
        );
        assert_eq!(
            decidir_asociacion(
                &s,
                None,
                "7DaysToDie",
                &ventana("2026-11-24 18:10:00", 60.0)
            ),
            Asociacion::UnicaSesionDelJuego
        );
        assert_eq!(
            decidir_asociacion(&s, None, "Palworld", &ventana("2026-11-24 18:10:00", 60.0)),
            Asociacion::NoComprobable
        );
    }

    #[test]
    fn sin_ventana_no_hay_atribucion_automatica() {
        let s = sesion_con_proceso("2026-11-24 18:00:00", None);
        let v = VentanaDeGrabacion::default();
        assert_eq!(
            decidir_asociacion(&s, Some(251570), "7DaysToDie", &v),
            Asociacion::NoComprobable
        );
    }

    // ---------- evidencia de proceso ----------

    #[test]
    fn la_invisibilidad_no_permite_decir_que_no_corre() {
        // El caso Docker: no se ve el proceso del anfitrión. Eso NO es «no está corriendo».
        assert!(!EvidenciaProceso::SinVisibilidad.permite_afirmar_que_no_corre());
        assert!(!EvidenciaProceso::SinPermisos.permite_afirmar_que_no_corre());
        assert!(!EvidenciaProceso::NoObservada.permite_afirmar_que_no_corre());
        assert!(EvidenciaProceso::Terminada.permite_afirmar_que_no_corre());
    }

    #[test]
    fn la_clave_del_proceso_usa_pid_y_arranque() {
        let p = ProcesoObservado {
            pid: 42,
            inicio_del_proceso: Some("2026-11-24 18:00:00".into()),
            ..Default::default()
        };
        assert_eq!(p.clave(), "42@2026-11-24 18:00:00");

        // Sin instante de arranque se dice que falta, no se inventa: el PID solo no
        // distingue dos ejecuciones distintas.
        let sin = ProcesoObservado {
            pid: 42,
            ..Default::default()
        };
        assert_eq!(sin.clave(), "42@desconocido");
    }

    // ---------- sesión ----------

    #[test]
    fn la_configuracion_posterior_al_arranque_se_marca() {
        let cfg = ConfiguracionCapturada {
            comando_confirmado: None,
            perfil: None,
            revision_motor: None,
            opciones_steam: None,
            capturado_en: "2026-11-24 18:05:00".into(),
            posterior_al_arranque: true,
        };
        // La marca existe para que la interfaz no lo llame "comando de arranque".
        assert!(cfg.posterior_al_arranque);
        assert!(cfg.revision_motor.is_none(), "no se inventa revisión");
    }

    #[test]
    fn una_sesion_agrupada_no_se_presenta_como_observada() {
        let s = SesionDeEjecucion::agrupada("g".into(), IdentidadDeSesion::desconocida(), ahora());
        assert_eq!(s.tipo, TipoDeSesion::AgrupadaSinProceso);
        assert!(!s.esta_abierta());
        assert!(s.proceso.is_none());
    }

    #[test]
    fn las_sesiones_se_recortan_conservando_las_abiertas() {
        let mut lista: Vec<SesionDeEjecucion> = (0..MAX_SESIONES + 20)
            .map(|i| {
                let mut s = SesionDeEjecucion::agrupada(
                    format!("s{}", i),
                    IdentidadDeSesion::desconocida(),
                    &format!("2026-01-01 00:{:02}:00", i % 60),
                );
                s.evidencia_proceso = EvidenciaProceso::Terminada;
                s.cerrada_en = Some("2026-01-02 00:00:00".into());
                s
            })
            .collect();
        // Una abierta al final, que no debe perderse.
        let mut abierta = SesionDeEjecucion::observada(
            "viva".into(),
            RasgosDeProceso::default(),
            IdentidadDeSesion::desconocida(),
            "2020-01-01 00:00:00",
        );
        abierta.evidencia_proceso = EvidenciaProceso::Observada;
        lista.push(abierta);

        recortar_sesiones(&mut lista);
        assert_eq!(lista.len(), MAX_SESIONES);
        assert!(
            lista.iter().any(|s| s.id == "viva"),
            "las abiertas se conservan"
        );
    }
}
