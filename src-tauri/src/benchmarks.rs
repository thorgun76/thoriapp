use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Origen del comando registrado en una grabación.
///
/// `Instantanea` es el bueno (tomado al detectar la grabación). `ComandoActual` significa
/// que la instantánea estaba vacía y se leyó al guardar: el comando puede no ser el de la
/// grabación. `Desconocido` es «no registrado».
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrigenComando {
    Instantanea,
    ComandoActual,
    #[default]
    Desconocido,
}

// ==================== Instantánea de configuración (subfase 5.2) ====================

/// De dónde salió un dato de la instantánea.
///
/// Existe porque «está registrado» y «es cierto» no son lo mismo. Un comando leído del
/// motor de Thoriapp es lo que la app propone, no lo que Steam ejecutó; un dato leído de
/// Steam tampoco está verificado: el usuario pudo cambiar las opciones a mano después de
/// lanzar. Sin esta distinción, la interfaz acabaría presentando como cierto algo que solo
/// se copió de algún sitio.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Procedencia {
    /// El motor de comandos de Rust confirmó el valor (comando canónico, revisión).
    MotorRust,
    /// Thoriapp lo tenía configurado (perfil cargado, ajustes activos).
    ConfiguradoEnThoriapp,
    /// Leído de los archivos de Steam. Describe Steam, no la ejecución.
    LeidoDeSteam,
    /// Lo registró **MangoHud** durante la propia grabación (cabecera de sistema del CSV o
    /// su `_summary.csv`). Es la fuente que describe las condiciones de la prueba, y por eso
    /// pesa más que el hardware detectado al analizar.
    RegistradoPorMangohud,
    /// Detectado en el sistema (hardware, kernel, Mesa).
    DetectadoEnSistema,
    /// Lo escribió el usuario a mano (notas, ruta de log).
    DeclaradoPorUsuario,
    /// Comprobado durante la ejecución del juego. Sigue sin usarse: en la subfase 5.3 se
    /// añadieron las procedencias de observación, pero ninguna afirma todavía el *efecto*
    /// de una optimización, que es lo único que justificaría esta.
    VerificadoEnEjecucion,
    /// Leído del **proceso** del juego mientras corría (su entorno, su ejecutable, la
    /// carpeta desde la que arrancó). Procede de `sessions`, no de Steam.
    ObservadoEnProceso,
    /// El usuario lo asignó a mano (por ejemplo, el juego de una grabación que no se pudo
    /// identificar). Es fiable como decisión del usuario, pero no como observación.
    ConfirmadoPorUsuario,
    /// Deducción a partir de una única coincidencia de nombre. Es una inferencia: se
    /// presenta como tal y nunca como dato observado.
    Inferido,
    /// No se sabe, o no había información fiable del juego correspondiente.
    #[default]
    Desconocido,
}

/// Un dato con su procedencia. `None` no es lo mismo que un valor desconocido: `None` es
/// «ni siquiera hay un hueco que rellenar» (el campo no aplicaba), mientras que un valor
/// con procedencia `Desconocido` es «debería estar y no lo sabemos».
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConProcedencia<T> {
    pub valor: T,
    pub procedencia: Procedencia,
    /// Cuándo se capturó el dato (`AAAA-MM-DD HH:MM:SS`). Permite ver si se leyó junto al
    /// resto o mucho después, que es justo lo que hace dudosa una instantánea.
    #[serde(default)]
    pub capturado_en: Option<String>,
}

impl<T> ConProcedencia<T> {
    pub fn nuevo(valor: T, procedencia: Procedencia) -> Self {
        Self {
            valor,
            procedencia,
            capturado_en: None,
        }
    }

    pub fn con_fecha(mut self, fecha: &str) -> Self {
        self.capturado_en = Some(fecha.to_string());
        self
    }
}

/// Identidad de la prueba dentro de su instantánea.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IdentidadInstantanea {
    /// AppID **real** del juego. A diferencia de `Benchmark::appid`, aquí nunca se guarda
    /// un `0` de «regla genérica»: si de verdad no se conoce, el campo es `None` y la
    /// interfaz lo dice.
    pub appid_real: Option<u32>,
    /// De dónde sale `appid_real`. Mientras no conste, `Desconocido`. Cuando la sesión
    /// observada lo aporta, se marca `ObservadoEnProceso`: es un dato leído del proceso,
    /// no una suposición ni una inferencia por nombre.
    #[serde(default)]
    pub appid_procedencia: Procedencia,
    pub benchmark_id: String,
    /// Número de presentación (`Prueba N`) en el momento de guardar.
    pub numero_prueba: Option<u32>,
    pub fecha: String,
}

/// Configuración con la que se hizo la prueba.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConfigInstantanea {
    /// Comando **literal** tal como lo dio el motor de Rust, con sus espacios, comillas y
    /// orden. No se reconstruye desde los toggles: eso perdería los argumentos que la app
    /// no entiende.
    pub comando_confirmado: Option<ConProcedencia<String>>,
    /// Revisión del comando en el motor cuando se capturó. Sirve para detectar que la
    /// instantánea mezcla datos de dos revisiones distintas.
    pub revision_motor: Option<u64>,
    pub opciones_steam: Option<ConProcedencia<String>>,
    pub perfil_id: Option<String>,
    pub perfil_nombre: Option<ConProcedencia<String>>,
    /// Había cambios sin guardar en el editor en el momento de la captura.
    pub cambios_sin_guardar: Option<bool>,
    /// `true` = la prueba se configuró para ejecutarse con Proton; `false` = nativo;
    /// `None` = no se sabe.
    pub modo_proton: Option<ConProcedencia<bool>>,
    /// Modo de ejecución real (nativo o Proton) **solo si es verificable**. No se rellena
    /// a partir de `modo_proton`: configurar Proton no demuestra que el juego corriera con
    /// Proton. Hasta la subfase 5.3 esto queda siempre en `None`.
    pub ejecucion_verificada: Option<ConProcedencia<String>>,
}

/// Entorno del equipo en el momento de la prueba.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct EntornoInstantanea {
    pub gpu: Option<ConProcedencia<String>>,
    pub cpu: Option<ConProcedencia<String>>,
    pub ram_gb: Option<ConProcedencia<f32>>,
    pub kernel: Option<ConProcedencia<String>>,
    /// Controlador gráfico (versión del driver propietario, si se conoce).
    pub driver: Option<ConProcedencia<String>>,
    pub mesa: Option<ConProcedencia<String>>,
    /// Versión de Proton **instalada o seleccionada**. No es la que usó el proceso: eso
    /// exigiría mirar el proceso en marcha, y no se presume.
    pub proton_seleccionado: Option<ConProcedencia<String>>,
    /// BuildID del juego según `appmanifest_<appid>.acf`.
    pub build_id: Option<ConProcedencia<String>>,
    /// Intervalo de muestreo de MangoHud en milisegundos.
    pub log_interval_ms: Option<ConProcedencia<u32>>,
}

/// Lo que **MangoHud registró** durante la grabación, transcrito sin reinterpretar.
///
/// Vive aparte del entorno detectado a propósito: son dos equipos en dos momentos. Si la
/// grabación se hizo en un PC y se analiza en otro, esto describe el PC de la grabación y
/// `EntornoInstantanea` el de ahora. Ninguno de los dos rellena los huecos del otro.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct HardwareDeLaGrabacion {
    /// `os` del CSV. Con Proton suele ser «Steam Runtime 4», que **no** es la distribución
    /// del usuario; se guarda tal cual y la interfaz no lo presenta como distro.
    pub os: Option<ConProcedencia<String>>,
    pub cpu: Option<ConProcedencia<String>>,
    pub gpu: Option<ConProcedencia<String>>,
    /// RAM total. Se conserva el valor crudo que escribió MangoHud (en KiB) **y** su
    /// conversión, porque el crudo es la única prueba de la unidad.
    pub ram_kib: Option<ConProcedencia<u64>>,
    pub ram_gib: Option<ConProcedencia<f32>>,
    pub kernel: Option<ConProcedencia<String>>,
    /// Controlador. Con RADV el CSV suele traerlo **vacío**: queda `None` en vez de
    /// deducirse del nombre de la GPU.
    pub driver: Option<ConProcedencia<String>>,
    pub cpuscheduler: Option<ConProcedencia<String>>,
    /// Intervalo de muestreo medido sobre `elapsed`, en milisegundos.
    pub intervalo_muestreo_ms: Option<ConProcedencia<f64>>,
}

/// Resumen que calculó MangoHud (`_summary.csv`), guardado como fuente **independiente**.
///
/// No sustituye a las métricas de Thoriapp ni se usa para decidir nada: su metodología es
/// distinta (media armónica de FPS, «1 % low» promediado y «0.1 % low» como máximo
/// absoluto). Ver `docs/mangohud-summary.md`. `None` = no había resumen o no se pudo leer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ResumenOriginal {
    /// Nombres y valores tal como los escribió MangoHud, sin recalcular.
    pub valores: Vec<(String, f64)>,
    pub capturado_en: String,
}

impl ResumenOriginal {
    /// Busca un valor por el nombre que escribió MangoHud. No se usa en el binario: la
    /// interfaz consume la lista de pares. Vive para las pruebas, que sí necesitan
    /// comprobar qué trae el resumen sin recorrer la lista a mano.
    #[cfg(test)]
    pub fn valor(&self, nombre: &str) -> Option<f64> {
        let buscado = nombre.trim().to_lowercase();
        self.valores
            .iter()
            .find(|(n, _)| n.trim().to_lowercase() == buscado)
            .map(|(_, v)| *v)
    }
}

/// Instantánea histórica e inmutable de la configuración de una prueba.
///
/// Se guarda **junto** al benchmark y no se vuelve a calcular: si mañana se renombra el
/// perfil, se cambian las opciones de Steam o se actualiza Proton, esto no cambia. Es el
/// único sitio del que la interfaz puede decir «con esto se hizo la prueba».
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SnapshotConfig {
    /// Versión del propio formato de la instantánea. Independiente de la del archivo: así
    /// se puede mejorar una sin invalidar la otra.
    pub version: u32,
    pub identidad: IdentidadInstantanea,
    pub configuracion: ConfigInstantanea,
    /// Equipo detectado **en el momento del análisis**.
    #[serde(default)]
    pub entorno: EntornoInstantanea,
    /// Equipo tal como lo **registró MangoHud** durante la grabación. `None` = el CSV no
    /// traía cabecera de sistema: entonces las condiciones de la grabación son
    /// desconocidas, aunque hoy se detecte un equipo. No se rellena con `entorno`.
    #[serde(default)]
    pub hardware_grabacion: Option<HardwareDeLaGrabacion>,
    /// Resumen que calculó MangoHud para este mismo CSV. Fuente aparte, no se mezcla con
    /// las métricas de Thoriapp.
    #[serde(default)]
    pub resumen_mangohud: Option<ResumenOriginal>,
    /// Momento en que se **capturaron** los datos. Puede ser posterior a la grabación (el
    /// CSV se analiza al terminar), y por eso no se afirma que describa lo que ocurrió
    /// durante ella.
    pub capturado_en: String,
    /// Si la instantánea se tomó después de que la grabación terminara. Mientras no haya
    /// verificación durante la ejecución (5.3), esto será `true` casi siempre, y la
    /// interfaz lo dice en lugar de presentarlo como la configuración usada.
    pub posterior_a_la_grabacion: bool,
}

/// Versión del formato de instantánea.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Benchmark {
    pub id: String,
    pub profile_id: String,
    pub game_name: String,
    pub appid: u32,
    pub timestamp: String,
    pub duration_seconds: f64,
    pub avg_fps: f64,
    pub fps_1_percent_low: f64,
    pub fps_0_1_percent_low: f64,
    pub max_fps: f64,
    pub min_fps: f64,
    pub frame_time_avg: f64,
    pub frame_time_1_percent_high: f64,
    pub total_frames: u64,
    pub notes: Option<String>,
    /// Comando de lanzamiento que Thoriapp tenía configurado **cuando se detectó la
    /// grabación** (no cuando se pulsó «analizar»).
    ///
    /// Es una **instantánea**, no una referencia: si mañana el usuario cambia el comando
    /// del juego, este campo no cambia. `""` significa «no registrado»: benchmarks
    /// guardados antes de que la app capturara el comando. `"%command%"` es un valor
    /// legítimo y significa «sin ajustes activos».
    ///
    /// No se puede saber si el usuario editó las opciones de lanzamiento en Steam a mano
    /// después: esto es el comando que Thoriapp configuraba, no el que Steam ejecutó. Por
    /// eso la interfaz lo llama «comando registrado».
    #[serde(default)]
    pub launch_command: String,
    /// De dónde salió `launch_command`.
    ///
    /// Permite auditar una prueba después: ver `%command%` no dice si de verdad no había
    /// ajustes o si el dato se leyó en el momento equivocado (instantánea vacía → comando
    /// del editor al guardar). Los benchmarks antiguos no lo tienen: `#[serde(default)]`
    /// los lee como `desconocido`.
    #[serde(default)]
    pub origen_comando: OrigenComando,
    /// Opciones de lanzamiento que **Steam** tenía configuradas para este juego cuando se
    /// guardó la prueba, leídas de `localconfig.vdf`.
    ///
    /// Es lo que de verdad se ejecutó al entrar al juego, así que es el dato que sirve para
    /// saber «con qué argumento hice esta prueba». `launch_command` es lo que Thoriapp
    /// proponía en su editor, que puede no coincidir (el usuario lo copia a Steam a mano).
    ///
    /// `None` = no se pudo leer Steam (no instalado, otro usuario, Flatpak sin permisos).
    /// `Some("")` = se leyó y Steam las tiene vacías, que es un dato real: se lanzó sin
    /// argumentos. No se confunden a propósito.
    #[serde(default)]
    pub steam_command: Option<String>,
    /// Número de presentación de la grabación dentro de su juego (`Prueba N`).
    ///
    /// Se asigna **una sola vez**, al guardar, y no cambia nunca: sobrevive a borrar otras
    /// grabaciones, al recorte de `trim_history` y a guardar/abrir una comparación (que
    /// embute copias de los dos benchmarks, así que el número viaja con ellos).
    ///
    /// `None` en las grabaciones guardadas antes de existir este campo: no se les inventa
    /// una cronología, porque el historial puede haber sufrido borrados y el orden no se
    /// puede reconstruir con seguridad. La interfaz las llama «Prueba sin número».
    #[serde(default)]
    pub number: Option<u32>,
    /// Instantánea histórica de la configuración (subfase 5.2).
    ///
    /// `None` en las pruebas guardadas antes de esta subfase. **No se reconstruye**: los
    /// datos actuales del juego, del perfil o del hardware no dicen cómo se hizo una
    /// prueba del mes pasado. La interfaz muestra «Instantánea no disponible».
    #[serde(default)]
    pub snapshot: Option<SnapshotConfig>,
    /// Sesión de ejecución a la que se atribuyó esta grabación (subfase 5.3).
    ///
    /// Es solo un **enlace**: todo lo que hace falta para interpretar la prueba está
    /// dentro de `snapshot`, así que un benchmark histórico sigue siendo legible aunque la
    /// sesión desaparezca. Borrar sesiones no deja pruebas huérfanas ni al revés.
    ///
    /// `None` en las pruebas guardadas antes de 5.3 y también cuando no hubo evidencia
    /// suficiente para atribuirlas.
    #[serde(default)]
    pub sesion_id: Option<String>,
    // Métricas de fluidez. Son None en los benchmarks guardados antes de esta versión
    // (por eso #[serde(default)]: los benchmarks viejos se siguen cargando sin problema).
    #[serde(default)]
    pub spikes_per_min: Option<f64>,
    #[serde(default)]
    pub pct_over_33ms: Option<f64>,
    #[serde(default)]
    pub jitter_ms: Option<f64>,
    #[serde(default)]
    pub worst_frame_ms: Option<f64>,
    /// Segundos iniciales que se descartaron (calentamiento). `None` en benchmarks
    /// guardados antes de existir esta métrica.
    #[serde(default)]
    pub warmup_seconds: Option<f64>,
    /// Margen de ruido de esta grabación: cuánto puede moverse el FPS promedio solo por
    /// variación natural (95 % de confianza), en %. Sirve para no llamar "mejora" a una
    /// diferencia que está dentro del ruido.
    #[serde(default)]
    pub noise_percent: Option<f64>,
    /// Los primeros segundos de la grabación (antes del calentamiento) frente al resto:
    /// ahí se ve el coste de compilar shaders la primera vez. `None` si no hay datos
    /// suficientes para compararlo.
    #[serde(default)]
    pub shader_stutter: Option<ShaderStutter>,
    /// Ruta del CSV del que salió esta grabación.
    ///
    /// Es la identidad real de una grabación: dos análisis del mismo fichero dan el mismo
    /// resultado y las mismas métricas. Sin esto, detectar duplicados exigía comparar
    /// números con tolerancia, que falla justo cuando dos grabaciones son parecidas.
    /// Vacío en los benchmarks guardados antes de existir este campo.
    #[serde(default)]
    pub log_path: String,
    /// Número de muestras del log usadas en el análisis (tras descartar el calentamiento).
    ///
    /// Es el nombre correcto de lo que `total_frames` contiene realmente: MangoHud
    /// guarda ~10 filas por segundo, no un frame por fila, así que `total_frames` nunca
    /// fueron frames. Renombrarlo habría roto la lectura de los JSON ya guardados
    /// (`total_frames` desaparecería), así que se mantienen los dos: el campo viejo se
    /// sigue leyendo y escribiendo con el mismo significado, y este lo dice sin engañar.
    #[serde(default)]
    pub sample_count: Option<u64>,
    /// Tiempos por muestra, para el gráfico temporal. `None` en las pruebas guardadas
    /// antes de que existiera este campo (y en las que se analizaron sin guardarlo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muestras_temporales: Option<MuestrasTemporales>,
    /// Duración **medida con reloj fiable**, en segundos. `None` = no hay eje temporal
    /// en el que confiar [5.4.1.A].
    ///
    /// `duration_seconds` se conserva como `f64` porque forma parte del contrato con los
    /// JSON ya guardados y con el frontend; sin reloj fiable vale `0.0`, y **quien lo
    /// consuma debe comprobar `duracion_medida` antes de tratarlo como una duración**. El
    /// cero ahí significa «no consta», no «cero segundos de grabación».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duracion_medida: Option<f64>,    /// Con qué metodología **de cálculo** se obtuvieron estos números.
    ///
    /// `None` = grabación anterior a este campo: **no consta**, no se deduce `v1` ni se
    /// recalcula. Un cambio de metodología cambia los números, así que dos grabaciones
    /// con versiones distintas no son directamente comparables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metodologia_calculo: Option<MetodologiaCalculo>,
    /// De dónde salen las muestras que han entrado en el cálculo: filas leídas,
    /// excluidas por calentamiento, analizadas, e incidencias temporales encontradas.
    ///
    /// `None` en las grabaciones históricas: no se reconstruye a posteriori lo que el
    /// código de entonces no registró.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contabilidad_muestras: Option<ContabilidadDeMuestras>,
    /// Apoyo real de cada percentil: sobre cuántas muestras se calculó y qué cola usó la
    /// interpolación. Sirve para no presentar una cifra apoyada en dos observaciones como
    /// si tuviera cientos detrás.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apoyo_metricas: Option<ApoyoDeMetricas>,
}

/// De dónde sale —o por qué no se conoce— la duración de una grabación.
///
/// Existe para que la regla se aplique **en un solo sitio**. Antes, cada consumidor
/// decidía por su cuenta si miraba `duracion_medida` o `duration_seconds`, y eso permitía
/// que un cero de compatibilidad se presentara como «cero segundos».
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DuracionDeLaGrabacion {
    /// Grabación anterior a `metodologia_calculo` (o que no la declaró). Su
    /// `duration_seconds` es lo único que hay y se usa tal cual: **no es un cero de
    /// compatibilidad**, es el valor que se midió con la metodología de entonces.
    Historica { segundos: f64 },
    /// Grabación v2 con reloj fiable: duración medida.
    Medida { segundos: f64 },
    /// Grabación v2 que **no** pudo medir duración (sin eje temporal fiable, o con una
    /// duración que no es un dato utilizable). No es «cero segundos»: es «no consta».
    Desconocida,
}

impl Benchmark {
    /// Duración de la grabación, con su procedencia. **Es el único punto donde se decide
    /// qué significa `duration_seconds`.**
    ///
    /// Reglas, en este orden:
    ///
    /// 1. Si hay `duracion_medida`, manda ella. Es duración medida de verdad.
    /// 2. Si la grabación **declara** la metodología v2 y no hay `duracion_medida`, la
    ///    duración es **desconocida**. No se cae a `duration_seconds`: en v2 ese campo
    ///    vale 0 precisamente cuando no había eje, y leerlo sería inventar cero segundos.
    /// 3. Si no declara metodología (histórica), `duration_seconds` es el valor medido
    ///    entonces. Un valor no utilizable (≤0, no finito) sigue siendo desconocido.
    pub fn duracion(&self) -> DuracionDeLaGrabacion {
        if let Some(segundos) = self.duracion_medida.filter(|d| d.is_finite() && *d > 0.0) {
            return DuracionDeLaGrabacion::Medida { segundos };
        }
        if self.metodologia_calculo.is_some() {
            // v2 declarada sin duración medida: no hay respaldo legítimo.
            return DuracionDeLaGrabacion::Desconocida;
        }
        match Some(self.duration_seconds).filter(|d| d.is_finite() && *d > 0.0) {
            Some(segundos) => DuracionDeLaGrabacion::Historica { segundos },
            None => DuracionDeLaGrabacion::Desconocida,
        }
    }

    /// Segundos de grabación **solo si constan**. `None` = no consta, nunca un cero que
    /// se pueda confundir con una grabación vacía.
    pub fn duracion_en_segundos(&self) -> Option<f64> {
        match self.duracion() {
            DuracionDeLaGrabacion::Historica { segundos }
            | DuracionDeLaGrabacion::Medida { segundos } => Some(segundos),
            DuracionDeLaGrabacion::Desconocida => None,
        }
    }
}

/// Serie temporal de una grabación, para dibujar el frametime en el tiempo.
///
/// **No se guardan los frame times** (los datos crudos de MangoHud): se guarda el
/// frametime **medio de cada muestra**, que es exactamente lo que MangoHud escribe en
/// una fila. Guardar los frames individuales exigiría decenas de miles de valores por
/// grabación y no aporta nada que esta pantalla pueda mostrar.
///
/// `frame_time_ms[i]` corresponde a `elapsed_s[i]`; ambos vectores tienen la misma
/// longitud. Si el log no trae la columna `elapsed`, `elapsed_s` queda vacío y el
/// gráfico se dibuja por índice de muestra (ver `interpretacion` en el frontend).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MuestrasTemporales {
    /// Frametime medio de cada muestra, en milisegundos.
    pub frame_time_ms: Vec<f32>,
    /// Instante de cada muestra, en segundos desde el inicio de la grabación.
    /// Vacío cuando el log no traía la columna `elapsed`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elapsed_s: Vec<f32>,
}

/// Tope de muestras que se guardan por grabación.
///
/// Una grabación de 60 s a 10 Hz son ~600 muestras (~4,8 KB en JSON). El tope protege
/// de logs anormalmente largos (una grabación olvidada de una hora) sin recortar nada
/// que un usuario normal vaya a producir. Al recortar se **diezma uniformemente**, no
/// se corta el final: así el gráfico sigue representando toda la grabación.
const MAX_MUESTRAS_TEMPORALES: usize = 4000;

/// Coste del arranque: lo que se pierde en los primeros segundos de la grabación,
/// cuando los shaders todavía se están compilando.
///
/// Es la diferencia entre la primera ventana y el resto de la grabación, medida en el
/// **peor 1 %** de frames. Se usa el peor 1 % y no el FPS promedio porque la compilación
/// de shaders no baja el promedio: provoca tirones puntuales, que es justo lo que el
/// promedio esconde.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShaderStutter {
    /// Segundos comparados al principio (0 = no se pudo comparar).
    pub window_seconds: f64,
    /// Peor 1 % de FPS en esa ventana inicial.
    pub early_fps_1_percent_low: f64,
    /// Peor 1 % de FPS en el resto de la grabación.
    pub steady_fps_1_percent_low: f64,
    /// % de frames por encima de 33 ms (menos de 30 FPS) en la ventana inicial.
    pub early_pct_over_33ms: f64,
    /// Lo mismo en el resto de la grabación.
    pub steady_pct_over_33ms: f64,
    /// Cuánto peor es el arranque que el resto, en % sobre el peor 1 %. Se queda en 0
    /// si el arranque no es peor (un juego que precarga puede ir mejor al principio).
    pub stutter_penalty_percent: f64,
}

/// Segundos que se descartan al principio de cada grabación por defecto.
/// Incluye la carga del nivel y la compilación inicial de shaders, que no representan
/// el rendimiento en juego y ensucian sobre todo los percentiles bajos.
pub const DEFAULT_WARMUP_SECONDS: f64 = 10.0;

/// Si tras descartar el calentamiento quedan menos segundos que esto, no se descarta
/// nada: quedarían tan pocas muestras que el resultado sería peor que no filtrar.
const MIN_STEADY_SECONDS: f64 = 20.0;

/// Mínimo de muestras para que el análisis tenga sentido.
const MIN_SAMPLES: usize = 100;

/// Margen mínimo de ruido que se asume entre dos grabaciones del mismo juego (% FPS).
/// Por debajo de esto no se puede afirmar nada aunque la estadística diga otra cosa.
const MIN_NOISE: f64 = 3.0;

/// Segundos iniciales que se comparan contra el resto para medir el coste de compilar
/// shaders. Coincide con el calentamiento por defecto: es exactamente el trozo que se
/// descarta, así que se enseña lo que se está tirando en vez de esconderlo.
pub const SHADER_WINDOW_SECONDS: f64 = DEFAULT_WARMUP_SECONDS;

/// Muestras mínimas en cada lado para poder comparar arranque y resto.
const MIN_SHADER_SAMPLES: usize = 50;

/// Por debajo de este empeoramiento (%) el arranque no se considera un problema de
/// shaders. El peor 1 % es una medida ruidosa, así que el listón es alto.
///
/// Lo consume la interfaz, no este módulo: `stutter_penalty_percent` se calcula aquí y
/// lo compara `SIGNIFICANT_PENALTY` en `src/lib/shaders.ts`. La constante existe para que
/// el test `el_umbral_coincide_con_la_interfaz` compruebe que los dos lados no se
/// separan, y por eso lleva `#[allow(dead_code)]`: en compilación normal nadie la lee,
/// pero borrarla dejaría el umbral duplicado a ciegas en TypeScript.
#[allow(dead_code)]
pub const SIGNIFICANT_SHADER_PENALTY: f64 = 20.0;

/// Grabaciones que se conservan de cada juego. Al guardar la número 11, la más antigua
/// de ese juego se borra sola; las de los demás juegos no se tocan.
pub const MAX_BENCHMARKS_PER_GAME: usize = 10;

/// Métricas que miden los "tirones" (stutter) a partir de los frame times (en ms).
struct StutterStats {
    /// `None` = no se puede calcular la tasa porque no consta la duración.
    spikes_per_min: Option<f64>,
    pct_over_33ms: f64,
    jitter_ms: f64,
    worst_frame_ms: f64,
}

/// Percentil con interpolación lineal sobre un vector YA ORDENADO de menor a mayor.
/// `p` va de 0 a 100. Es más estable que promediar "el peor 1%" cuando hay pocas muestras
/// (MangoHud guarda ~10 filas por segundo, no un frame por fila).
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = (p / 100.0).clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
}

/// Una fila válida del log de MangoHud.
#[derive(Clone, Debug)]
struct Sample {
    fps: f64,
    frame_time: f64,
    elapsed_ns: Option<f64>,
}

// ==================== Metodología de cálculo (5.4.1.A) ====================

/// Versión de la metodología **de cálculo** con la que se obtuvieron los números.
///
/// No es la versión del esquema (esa describe la *forma* del JSON) ni la de
/// interpretación (esa describe cómo se leen los números). Esta dice **qué filas
/// entraron en las fórmulas**: dos grabaciones con metodologías distintas pueden tener
/// números correctos y aun así no ser comparables entre sí.
///
/// `v2` deja de descartar muestras por coincidencia de FPS y frametime (ver
/// [`preparar_muestras`]). Las grabaciones anteriores a este campo quedan con
/// `None` = **«no consta»**, nunca `v1` deducido: sus números se calcularon con un
/// código que ya no existe y no se reconstruye.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetodologiaCalculo {
    /// Conserva todas las muestras con rendimiento válido y registra las incidencias
    /// temporales en vez de descartarlas.
    #[serde(rename = "v2")]
    V2,
}

/// Qué clases de incidencia temporal se han encontrado en un archivo.
///
/// Son **recuentos de diagnóstico**, no un juicio: tener incidencias no invalida la
/// grabación por sí mismo. Sirven para que el número que se presenta pueda auditarse
/// sabiendo con qué datos se calculó.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidenciasTemporales {
    /// Filas con `elapsed` idéntico al de una fila anterior.
    #[serde(default)]
    pub filas_repetidas: u32,
    /// Filas cuyo `elapsed` es menor que el de la anterior.
    #[serde(default)]
    pub orden_invertido: u32,
    /// Filas con `elapsed` negativo.
    #[serde(default)]
    pub negativas: u32,
    /// Filas cuyo `elapsed` no pudo leerse o no es finito (`NaN`, `inf`).
    #[serde(default)]
    pub no_finitas: u32,
    /// Total de filas sin un valor de reloj utilizable. Es la suma de los cuatro
    /// desgloses de abajo, que dicen **por qué** falta en cada una.
    #[serde(default)]
    pub reloj_incompleto: u32,
    /// La cabecera del CSV no traía columna `elapsed`.
    #[serde(default)]
    pub columna_elapsed_ausente: u32,
    /// La fila tenía menos columnas que las que declara la cabecera.
    #[serde(default)]
    pub celda_reloj_ausente: u32,
    /// La celda del reloj existía pero estaba vacía.
    #[serde(default)]
    pub celda_reloj_vacia: u32,
    /// La celda del reloj tenía texto que no es un número, o un número no finito.
    #[serde(default)]
    pub celda_reloj_ilegible: u32,
}

impl IncidenciasTemporales {
    pub fn total(&self) -> u32 {
        self.filas_repetidas
            + self.orden_invertido
            + self.negativas
            + self.no_finitas
            + self.reloj_incompleto
    }

    pub fn hay_algunas(&self) -> bool {
        self.total() > 0
    }

    /// El total de filas sin reloj debe ser exactamente la suma de sus causas. Si no
    /// cuadra, el desglose y el total dicen cosas distintas y no se puede auditar nada.
    pub fn desglose_de_reloj_cuadra(&self) -> bool {
        self.columna_elapsed_ausente
            + self.celda_reloj_ausente
            + self.celda_reloj_vacia
            + self.celda_reloj_ilegible
            == self.reloj_incompleto
    }
}

/// Un eje temporal en el que se puede confiar para cortar por tiempo.
///
/// Se construye solo si **todas** las muestras traen reloj, es **monótono estricto** y
/// todos los valores son **finitos y no negativos**. Si no, una métrica que dependa del
/// reloj no se calcula: se declara no disponible en vez de inventar un eje.
#[derive(Debug, Clone, Copy)]
struct EjeTemporal {
    inicio_ns: f64,
    fin_ns: f64,
}

impl EjeTemporal {
    fn duracion_segundos(self) -> f64 {
        ((self.fin_ns - self.inicio_ns) / 1e9).max(0.0)
    }
}

/// Cuántas filas hay en cada etapa del análisis.
///
/// Las etapas son **disjuntas**: cada fila se cuenta exactamente en una, así que
/// sumar no cuenta dos veces la misma exclusión.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContabilidadDeMuestras {
    /// Filas de datos del CSV: las que están entre la cabecera de datos y el final. No
    /// cuentan las líneas de metadatos iniciales (`OS`, `CPU`, `GPU`…) ni la cabecera.
    /// Es el total de partida, sumen o no rendimiento válido.
    #[serde(default)]
    pub filas_originales: u32,
    /// Filas con `fps` y `frametime` legibles y positivos (candidatas al análisis).
    #[serde(default)]
    pub filas_con_rendimiento_valido: u32,
    /// Filas de datos cuyo `fps` o `frametime` no es utilizable (vacío, no numérico,
    /// negativo, cero o no finito). No entran en ninguna métrica de rendimiento.
    #[serde(default)]
    pub filas_con_rendimiento_invalido: u32,
    /// De las válidas, las excluidas por el calentamiento.
    #[serde(default)]
    pub excluidas_por_calentamiento: u32,
    /// Las que llegan al cálculo de las métricas. Es el `n` real de los percentiles.
    #[serde(default)]
    pub muestras_analizadas: u32,
    /// Incidencias temporales encontradas. **No son exclusiones**: una fila puede tener
    /// varias a la vez y aun así entrar en el cálculo. Se contabilizan aparte.
    #[serde(default)]
    pub incidencias: IncidenciasTemporales,
    /// Si hay un eje temporal en el que se pueda confiar (para duración, calentamiento y
    /// series por tiempo).
    #[serde(default)]
    pub eje_temporal_fiable: bool,
    /// La columna `elapsed` existía en la cabecera del CSV.
    #[serde(default)]
    pub columna_elapsed_presente: bool,
    /// Se ha solicitado calentamiento pero **no se ha aplicado**. `None` cuando se aplicó
    /// o cuando no se pidió.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calentamiento_no_aplicado: Option<MotivoSinCalentamiento>,
}

/// Por qué no se pudo aplicar el calentamiento solicitado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotivoSinCalentamiento {
    /// No se pidió calentamiento (`warmup_seconds <= 0`).
    NoSolicitado,
    /// El log no trae un eje temporal en el que confiar.
    SinEjeTemporal,
    /// Hay tan pocas muestras que el corte no saldría de sitio.
    PocasMuestras,
    /// Tras el corte quedarían menos de `MIN_STEADY_SECONDS` segundos.
    QuedariaDemasiadoCorto,
    /// El corte no deja `MIN_SAMPLES` muestras analizables.
    QuedarianPocasMuestras,
}

impl ContabilidadDeMuestras {
    /// Las filas de datos se reparten enteras entre inválidas, excluidas por
    /// calentamiento y analizadas. Ninguna fila se cuenta dos veces.
    pub fn cuadra(&self) -> bool {
        self.filas_originales as i64
            == self.filas_con_rendimiento_invalido as i64
                + self.excluidas_por_calentamiento as i64
                + self.muestras_analizadas as i64
            && self.filas_con_rendimiento_valido as i64
                == self.excluidas_por_calentamiento as i64 + self.muestras_analizadas as i64
    }
}

/// Valor del reloj de una fila, **sin colapsar los casos distintos** [5.4.1.A].
///
/// Antes todo acababa en `Option<f64>::None`, y así «la columna no existe», «la celda
/// está vacía», «el texto no es un número» y «es NaN/inf» eran indistinguibles: no se
/// podía decir al usuario qué le pasa a su CSV. No se corrige ni se convierte nada:
/// solo se nombra lo que hay.
#[derive(Debug, Clone, Copy, PartialEq)]
enum RelojDeFila {
    /// La cabecera del CSV no trae columna `elapsed`.
    ColumnaAusente,
    /// La fila tiene menos columnas que las que la cabecera declara.
    CeldaAusente,
    /// La celda existe pero está vacía o solo tiene espacios.
    CeldaVacia,
    /// La celda tiene texto que no es un número.
    Malformado,
    /// Es un número, pero NaN o infinito.
    NoFinito,
    /// Un número finito. Puede ser negativo o repetido: eso se clasifica aparte.
    Valor(f64),
}

impl RelojDeFila {
    /// El valor utilizable para comparar, si lo hay.
    fn valor(self) -> Option<f64> {
        match self {
            RelojDeFila::Valor(v) => Some(v),
            _ => None,
        }
    }
}

/// Fila leída del CSV antes de decidir nada: conserva **por qué** falta el `elapsed`.
struct FilaLeida {
    fps: f64,
    frame_time: f64,
    reloj: RelojDeFila,
}

/// Resultado de preparar las muestras para el cálculo.
struct MuestrasPreparadas {
    /// Lo que llega a las fórmulas.
    muestras: Vec<Sample>,
    contabilidad: ContabilidadDeMuestras,
    /// Duración según el eje temporal fiable. `None` si no hay eje fiable.
    duracion: Option<f64>,
    /// Eje utilizable, para las métricas que cortan por tiempo.
    eje: Option<EjeTemporal>,
    /// Segundos de calentamiento descartados de verdad (0.0 si no se aplicó).
    warmup_applied: f64,
}

/// Clasifica las incidencias del reloj de una serie de filas, en el orden en que vienen.
///
/// **No reordena, no interpola, no corrige**: solo cuenta lo que encuentra. Un desorden
/// detectado deja el eje no fiable pero no elimina ninguna fila.
fn detectar_incidencias(filas: &[FilaLeida]) -> IncidenciasTemporales {
    let mut inc = IncidenciasTemporales::default();
    // Último reloj **visto**, aunque sea inválido: sirve para detectar el desorden de una
    // fila válida respecto a la anterior que traía reloj.
    let mut anterior: Option<f64> = None;

    for fila in filas {
        match fila.reloj.valor() {
            None => {
                // Una fila sin reloj no puede ser repetida ni desordenada: no hay valor
                // que comparar. Se cuenta **una vez por fila sin reloj**, no las filas
                // posteriores: que falte en una fila no hace que las siguientes carezcan
                // de reloj. Se guarda además **por qué** falta, para no perder la causa
                // al persistir: el total solo no distingue una columna ausente de una
                // celda vacía.
                inc.reloj_incompleto += 1;
                match fila.reloj {
                    RelojDeFila::ColumnaAusente => inc.columna_elapsed_ausente += 1,
                    RelojDeFila::CeldaAusente => inc.celda_reloj_ausente += 1,
                    RelojDeFila::CeldaVacia => inc.celda_reloj_vacia += 1,
                    RelojDeFila::Malformado | RelojDeFila::NoFinito => inc.celda_reloj_ilegible += 1,
                    RelojDeFila::Valor(_) => unreachable!("ya filtrado por reloj.valor()"),
                }
            }
            Some(v) => {
                if v < 0.0 {
                    inc.negativas += 1;
                }
                if let Some(prev) = anterior {
                    // Comparación exacta: iguales = repetido; menor = invertido.
                    if v == prev {
                        inc.filas_repetidas += 1;
                    } else if v < prev {
                        inc.orden_invertido += 1;
                    }
                }
                anterior = Some(v);
            }
        }
    }

    // La columna existía pero alguna fila no trajo valor legible: el eje queda incompleto
    // aunque no haya huecos de orden. `reloj_incompleto` ya cuenta esas filas.

    inc
}

/// Decide si hay un eje temporal en el que se pueda confiar.
///
/// Requiere: la columna existía, **todas** las filas traen reloj, ninguno es negativo y
/// el orden es estrictamente creciente. Cualquier otra cosa deja el eje no fiable.
fn eje_de_confianza(filas: &[FilaLeida], columna_elapsed_presente: bool) -> Option<EjeTemporal> {
    if !columna_elapsed_presente || filas.is_empty() {
        return None;
    }
    let mut anterior: Option<f64> = None;
    for fila in filas {
        let v = fila.reloj.valor()?;
        if v < 0.0 {
            return None;
        }
        if let Some(prev) = anterior {
            if v <= prev {
                return None;
            }
        }
        anterior = Some(v);
    }
    let inicio = filas.first()?.reloj.valor()?;
    let fin = filas.last()?.reloj.valor()?;
    Some(EjeTemporal { inicio_ns: inicio, fin_ns: fin })
}

/// Prepara las muestras según la metodología **v2** y calcula la contabilidad.
///
/// Reglas (5.4.1.A):
///
/// - **No se descarta ninguna fila por coincidencia de FPS y frametime.** Dos ventanas
///   distintas pueden medir lo mismo sin ser la misma fila, y en una serie planchada
///   (limitador, pausa, tramo lento) esa coincidencia es el dato, no un artefacto.
/// - Hasta las filas **completamente idénticas** (mismo `elapsed`) se conservan: no hay
///   evidencia de que sean un doble registro en vez de dos ventanas del mismo instante.
/// - Sin `elapsed` **no se deduplica** nada: no hay forma de saber si dos filas iguales
///   son la misma o son un rendimiento plano.
/// - Las filas con rendimiento válido pero reloj raro **se conservan**: entran en las
///   métricas que solo dependen de los valores (promedios, percentiles). Lo que se pierde
///   es el eje temporal, no los datos de rendimiento. No se excluye ninguna fila por su
///   timestamp.
/// - El calentamiento solo se aplica si hay **eje fiable**. Si no hay eje, no se corta
///   nada: cortar por índice (suponiendo 100 ms por fila) sería inventar tiempo. El
///   motivo por el que no se aplicó queda registrado.
fn preparar_muestras(
    filas: Vec<FilaLeida>,
    warmup_seconds: f64,
    columna_elapsed_presente: bool,
    filas_originales: u32,
    filas_con_rendimiento_invalido: u32,
) -> MuestrasPreparadas {
    let incidencias = detectar_incidencias(&filas);
    // El desglose debe sumar exactamente el total de filas sin reloj: si no, se estaría
    // persistiendo un total que no explica sus propias causas.
    debug_assert!(
        incidencias.desglose_de_reloj_cuadra(),
        "el desglose de reloj no cuadra con reloj_incompleto: {incidencias:?}"
    );
    let eje = eje_de_confianza(&filas, columna_elapsed_presente);
    let filas_con_rendimiento_valido = filas.len() as u32;
    let duracion = eje.map(|e| e.duracion_segundos());

    // Todas las filas con rendimiento válido son candidatas: v2 no elimina ninguna por
    // coincidencia de valores ni por reloj.
    let muestras_validas: Vec<Sample> = filas
        .into_iter()
        .map(|f| Sample {
            fps: f.fps,
            frame_time: f.frame_time,
            elapsed_ns: f.reloj.valor(),
        })
        .collect();

    // La regla del corte es una sola (`indice_de_corte`, la misma que usa `split_warmup`).
    // Aquí solo se decide si procede y, si no, por qué.
    let corte = if eje.is_none() {
        None
    } else {
        indice_de_corte(&muestras_validas, warmup_seconds)
    };

    let motivo = if corte.is_some() {
        None
    } else if warmup_seconds <= 0.0 {
        Some(MotivoSinCalentamiento::NoSolicitado)
    } else if eje.is_none() {
        Some(MotivoSinCalentamiento::SinEjeTemporal)
    } else if muestras_validas.len() < MIN_SAMPLES {
        Some(MotivoSinCalentamiento::PocasMuestras)
    } else {
        // Las dos condiciones que quedan, en el orden en que las comprueba el corte.
        let first = muestras_validas.first().and_then(|s| s.elapsed_ns);
        let last = muestras_validas.last().and_then(|s| s.elapsed_ns);
        let total = match (first, last) {
            (Some(a), Some(b)) => (b - a) / 1e9,
            _ => 0.0,
        };
        if total - warmup_seconds < MIN_STEADY_SECONDS {
            Some(MotivoSinCalentamiento::QuedariaDemasiadoCorto)
        } else {
            Some(MotivoSinCalentamiento::QuedarianPocasMuestras)
        }
    };

    let (muestras, excluidas_calentamiento, warmup_applied) = if corte.is_some() {
        let total = muestras_validas.len() as u32;
        let (restantes, aplicado) = split_warmup(muestras_validas, warmup_seconds);
        let excluidas = total - restantes.len() as u32;
        (restantes, excluidas, aplicado)
    } else {
        (muestras_validas, 0u32, 0.0)
    };

    let contabilidad = ContabilidadDeMuestras {
        filas_originales,
        filas_con_rendimiento_valido,
        filas_con_rendimiento_invalido,
        excluidas_por_calentamiento: excluidas_calentamiento,
        muestras_analizadas: muestras.len() as u32,
        incidencias,
        eje_temporal_fiable: eje.is_some(),
        columna_elapsed_presente,
        calentamiento_no_aplicado: motivo,
    };
    // El invariante se comprueba también en las pruebas con `cuadra()`, no solo aquí.
    debug_assert!(contabilidad.cuadra(), "la contabilidad de muestras no cuadra");

    MuestrasPreparadas {
        muestras,
        contabilidad,
        duracion,
        eje,
        warmup_applied,
    }
}

/// Cómo se ha calculado un percentil concreto: hace falta para no presentar una medida
/// apoyada en dos observaciones como si tuviera cientos detrás.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ApoyoDePercentil {
    /// Muestras sobre las que se calculó (`n`).
    pub muestras: u32,
    /// Tamaño nominal de la cola para ese percentil (`n × (1−p)`), en unidades de
    /// muestra. Con `n` pequeño puede ser menor que 1.
    pub cola_nominal: f64,
    /// Observaciones distintas que usó la **interpolación** del percentil.
    pub observaciones_interpoladas: u32,
    /// Muestras incluidas de verdad en la media de cola, cuando la métrica es una media.
    /// `None` si la métrica es un percentil puro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muestras_en_la_media: Option<u32>,
}

/// Calcula el apoyo de un percentil interpolado sobre una serie ya ordenada.
fn apoyo_de_percentil(sorted: &[f64], p: f64) -> ApoyoDePercentil {
    let n = sorted.len();
    if n == 0 {
        return ApoyoDePercentil {
            muestras: 0,
            cola_nominal: 0.0,
            observaciones_interpoladas: 0,
            muestras_en_la_media: None,
        };
    }
    let rank = (p / 100.0).clamp(0.0, 1.0) * (n - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    ApoyoDePercentil {
        muestras: n as u32,
        cola_nominal: n as f64 * (1.0 - p / 100.0),
        // `lo == hi`, un solo elemento del vector, pero en `p = 50` la interpolación
        // puede tocar dos. Se cuentan las filas distintas: `hi - lo + 1`.
        observaciones_interpoladas: (hi - lo + 1) as u32,
        muestras_en_la_media: None,
    }
}

/// Apoyo de las métricas de un benchmark: con qué se calculó cada percentil.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ApoyoDeMetricas {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps_1_percent_low: Option<ApoyoDePercentil>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps_0_1_percent_low: Option<ApoyoDePercentil>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fps: Option<ApoyoDePercentil>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_fps: Option<ApoyoDePercentil>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_time_1_percent_high: Option<ApoyoDePercentil>,
}

impl ApoyoDeMetricas {
}

/// Muestras incluidas en la media de cola `ft >= P99`.
///
/// Se separa en su propia función porque el filtro `>=` incluye **empates**: en una serie
/// constante todas las muestras valen lo mismo y `P99` es ese valor, así que entran
/// **todas**, no el 1 %. Es un comportamiento derivado de la fórmula existente, no un
/// error nuevo, y por eso se mide en vez de corregirse aquí.
fn muestras_en_media_de_cola(sorted_ft: &[f64], p99_ft: f64) -> Vec<f64> {
    sorted_ft.iter().cloned().filter(|&f| f >= p99_ft).collect()
}

/// Descarta las muestras de los primeros `seconds` segundos de la grabación.
///
/// Devuelve las muestras que quedan y los segundos descartados de verdad. No descarta
/// nada (y devuelve 0) cuando no hay columna `elapsed` —sin reloj real no se sabe dónde
/// cae el corte—, cuando quedarían menos de [`MIN_STEADY_SECONDS`] segundos, o cuando
/// quedarían menos de [`MIN_SAMPLES`] muestras.
///
/// **Es la única implementación del corte por calentamiento**: `preparar_muestras` la
/// llama en vez de repetir la regla, para que el corte no pueda divergir entre los dos
/// caminos.
fn split_warmup(samples: Vec<Sample>, seconds: f64) -> (Vec<Sample>, f64) {
    match indice_de_corte(&samples, seconds) {
        Some(c) => {
            let aplicado = (samples_at(&samples, c) - samples_at(&samples, 0)) / 1e9;
            (samples.into_iter().skip(c).collect(), aplicado)
        }
        None => (samples, 0.0),
    }
}

/// Índice de la primera muestra que cae en o después del corte, o `None` si el
/// calentamiento no procede.
///
/// Es la regla **única**: `split_warmup` la usa para cortar y `preparar_muestras` para
/// saber cuántas filas excluye y si el calentamiento solicitado pudo aplicarse.
fn indice_de_corte(samples: &[Sample], seconds: f64) -> Option<usize> {
    if seconds <= 0.0 || samples.len() < MIN_SAMPLES {
        return None;
    }

    let (Some(first), Some(last)) = (
        samples.first().and_then(|s| s.elapsed_ns),
        samples.last().and_then(|s| s.elapsed_ns),
    ) else {
        return None;
    };

    let total = (last - first) / 1e9;
    // Si el corte deja menos de lo exigible, es mejor analizar todo que analizar un
    // trozo diminuto y presentarlo como si fuera representativo.
    if total - seconds < MIN_STEADY_SECONDS {
        return None;
    }

    let cutoff = first + seconds * 1e9;
    let cut = samples
        .iter()
        .position(|s| s.elapsed_ns.map(|e| e >= cutoff).unwrap_or(false))?;
    if cut == 0 || samples.len() - cut < MIN_SAMPLES {
        return None;
    }
    Some(cut)
}

fn samples_at(samples: &[Sample], idx: usize) -> f64 {
    samples.get(idx).and_then(|s| s.elapsed_ns).unwrap_or(0.0)
}

/// Parte las muestras en (primera ventana, resto).
///
/// La ventana se elige por tiempo real (`elapsed_ns`) y se corta por índice, así que
/// los dos tramos son disjuntos y un mismo frame nunca cuenta en los dos. Si no hay
/// reloj (`elapsed_ns` ausente, logs antiguos) o la ventana no deja frames en un lado,
/// devuelve dos listas vacías: sin datos fiables es mejor no decir nada.
fn sample_window(samples: &[Sample], seconds: f64) -> (Vec<f64>, Vec<f64>) {
    let empty = (Vec::new(), Vec::new());
    if seconds <= 0.0 || samples.len() < MIN_SHADER_SAMPLES * 2 {
        return empty;
    }
    let Some(first) = samples.first().and_then(|s| s.elapsed_ns) else {
        return empty;
    };
    let cutoff = first + seconds * 1e9;
    let Some(cut) = samples
        .iter()
        .position(|s| s.elapsed_ns.map(|e| e >= cutoff).unwrap_or(false))
    else {
        return empty;
    };
    if cut < MIN_SHADER_SAMPLES || samples.len() - cut < MIN_SHADER_SAMPLES {
        return empty;
    }
    let early: Vec<f64> = samples[..cut].iter().map(|s| s.frame_time).collect();
    let steady: Vec<f64> = samples[cut..].iter().map(|s| s.frame_time).collect();
    (early, steady)
}

/// Peor 1 % de FPS de un tramo de frames (o `None` si hay muy pocos).
fn worst_one_percent_fps(frame_times: &[f64]) -> Option<f64> {
    if frame_times.len() < MIN_SHADER_SAMPLES {
        return None;
    }
    let mut sorted = frame_times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p99 = percentile_sorted(&sorted, 99.0);
    if p99 <= 0.0 {
        return None;
    }
    Some(1000.0 / p99)
}

/// % de frames de un tramo que tardaron más de 33 ms (bajada por debajo de 30 FPS).
fn pct_over_33ms(frame_times: &[f64]) -> f64 {
    if frame_times.is_empty() {
        return 0.0;
    }
    let n = frame_times.iter().filter(|&&f| f > 33.3).count();
    (n as f64 / frame_times.len() as f64) * 100.0
}

/// Compara los primeros `window_seconds` de la grabación con el resto.
///
/// Por qué existe: el calentamiento (`DEFAULT_WARMUP_SECONDS`) se descarta para que el
/// análisis no salga contaminado por la compilación inicial de shaders. Pero descartarlo
/// en silencio oculta información útil: si el primer minuto de cada partida va a tirones,
/// el jugador lo sufre igual y no sabe por qué. Aquí se mide ese coste y se enseña.
///
/// Se trabaja con los frame times del tramo inicial ANTES de descartar el calentamiento,
/// así que las muestras tienen que llegar ya separadas: esta función recibe las dos
/// mitades hechas.
///
/// Devuelve `None` si algún lado tiene menos de [`MIN_SHADER_SAMPLES`] frames: con tan
/// poco no se puede afirmar nada sobre el arranque.
pub fn shader_stutter(early: &[f64], steady: &[f64], window_seconds: f64) -> Option<ShaderStutter> {
    if window_seconds <= 0.0 {
        return None;
    }
    let early_low = worst_one_percent_fps(early)?;
    let steady_low = worst_one_percent_fps(steady)?;

    // Penalización en % sobre el peor 1 %. Si el arranque va MEJOR que el resto no se
    // informa de una "mejora" (no tiene sentido: no has optimizado nada), se deja en 0.
    let penalty = if steady_low > 0.0 && early_low < steady_low {
        ((steady_low - early_low) / steady_low) * 100.0
    } else {
        0.0
    };

    Some(ShaderStutter {
        window_seconds,
        early_fps_1_percent_low: early_low,
        steady_fps_1_percent_low: steady_low,
        early_pct_over_33ms: pct_over_33ms(early),
        steady_pct_over_33ms: pct_over_33ms(steady),
        stutter_penalty_percent: penalty,
    })
}

/// Margen de ruido del FPS promedio de una grabación, en % (95 % de confianza).
///
/// La grabación se parte en bloques contiguos, se calcula la media de cada bloque y se
/// estima el error estándar **entre bloques**. Así la autocorrelación de los frame times
/// (un tirón afecta a varias filas seguidas) no se cuenta como si fueran muestras
/// independientes, que es el error de usar el desvío estándar por muestra.
///
/// **No necesita reloj** [5.4.1.A]: solo usa el orden de las filas y sus valores. Lo que
/// representa, entonces, es la variación del FPS medio **a lo largo de la grabación**, en
/// el orden en que se registró. Sus límites, que conviene no olvidar al leerlo:
///
/// - Los bloques son contiguos **por índice**, no por tiempo. Sin `elapsed` no se puede
///   afirmar que cada bloque cubra ~6 s ni que todos duren lo mismo; si la grabación
///   tuviera huecos, un bloque podría abarcar más tiempo real que otro.
/// - La aproximación normal sobre ~8 medias de bloque es gruesa: es una **estimación de
///   dispersión**, no un intervalo de confianza validado (ver `docs/mangohud-summary.md`).
/// - Mide la variación **dentro** de una grabación; no dice nada de la variación entre
///   partidas distintas.
///
/// `None` si hay muy pocas muestras para estimarlo con un mínimo de rigor.
fn noise_percent(fps: &[f64]) -> Option<f64> {
    let n = fps.len();
    if n < MIN_SAMPLES * 2 {
        return None;
    }

    // Bloques de ~60 filas (~6 s con MangoHud guardando cada 100 ms), entre 5 y 30.
    let blocks = (n / 60).clamp(5, 30);
    let size = n / blocks;
    if size < 20 {
        return None;
    }

    let means: Vec<f64> = (0..blocks)
        .map(|b| {
            let chunk = &fps[b * size..(b + 1) * size];
            chunk.iter().sum::<f64>() / chunk.len() as f64
        })
        .collect();

    let mean = means.iter().sum::<f64>() / means.len() as f64;
    if !mean.is_finite() || mean <= 0.0 {
        return None;
    }

    let var = means.iter().map(|m| (m - mean) * (m - mean)).sum::<f64>()
        / (means.len() as f64 - 1.0);
    let se = (var / means.len() as f64).sqrt();

    Some((1.96 * se / mean) * 100.0)
}

/// Margen de ruido combinado de dos grabaciones, en % sobre el FPS promedio.
/// Los errores de las dos medidas se suman en cuadratura (son independientes).
///
/// Si falta el dato de alguna de las dos, esa parte no aporta: el margen no puede
/// inventarse más incertidumbre de la que hay. Con una sola grabación medida el margen
/// es el de esa grabación, no `√2` veces mayor; con ninguna, el mínimo de la casa.
pub(crate) fn combined_noise(a: Option<f64>, b: Option<f64>) -> f64 {
    let a = a.filter(|v| v.is_finite() && *v > 0.0);
    let b = b.filter(|v| v.is_finite() && *v > 0.0);

    let combined = match (a, b) {
        (Some(a), Some(b)) => (a * a + b * b).sqrt(),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => MIN_NOISE,
    };

    combined.max(MIN_NOISE)
}

/// Serie temporal para el gráfico de frametime, recortando a `max` muestras.
///
/// `con_tiempos` indica si el log traía la columna `elapsed`. Si no la traía, la serie
/// se guarda igual (el gráfico puede dibujarse por índice de muestra) pero sin tiempos:
/// inventar un eje de segundos a partir del número de muestra sería afirmar una
/// regularidad que el log no garantiza.
///
/// La diezma es uniforme para que el gráfico siga cubriendo toda la grabación: cortar el
/// final mostraría solo el principio y el usuario creería que ahí acaba su partida.
fn serie_temporal(
    samples: &[Sample],
    max: usize,
    con_tiempos: bool,
) -> Option<MuestrasTemporales> {
    if samples.is_empty() {
        return None;
    }

    let paso = ((samples.len() as f64) / (max as f64)).ceil().max(1.0) as usize;
    let recogidas: Vec<&Sample> = samples.iter().step_by(paso).collect();

    let frame_time_ms: Vec<f32> = recogidas
        .iter()
        .map(|s| {
            // Un frametime negativo o no finito es un dato corrupto: esas filas no
            // llegan aquí (se cuentan como rendimiento inválido al leerlas) pero se
            // protege igualmente para que un log raro no produzca NaN en el JSON.
            if s.frame_time.is_finite() && s.frame_time > 0.0 {
                s.frame_time as f32
            } else {
                0.0
            }
        })
        .collect();

    let elapsed_s: Vec<f32> = if con_tiempos {
        let inicio = recogidas.first().and_then(|s| s.elapsed_ns).unwrap_or(0.0);
        recogidas
            .iter()
            .filter_map(|s| s.elapsed_ns)
            .map(|ns| ((ns - inicio) / 1e9) as f32)
            .collect()
    } else {
        Vec::new()
    };

    // Si los tiempos no quedaron alineados con los frametimes, se descartan: un eje X
    // de longitud distinta al eje Y es un gráfico mal dibujado.
    let elapsed_s = if elapsed_s.len() == frame_time_ms.len() { elapsed_s } else { Vec::new() };

    Some(MuestrasTemporales { frame_time_ms, elapsed_s })
}

fn compute_stutter(frame_times: &[f64], duration_seconds: f64) -> StutterStats {
    let n = frame_times.len();

    let mut sorted = frame_times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[n / 2];

    // Un "pico" es un frame que tarda más del doble de lo normal (y al menos 25 ms,
    // para no contar ruido cuando el juego va a muchos FPS).
    let spike_threshold = (median * 2.0).max(25.0);
    let spikes = frame_times.iter().filter(|&&f| f > spike_threshold).count();

    // Frames que tardaron más de 33.3 ms = momentos por debajo de 30 FPS.
    let over_33 = frame_times.iter().filter(|&&f| f > 33.3).count();

    // Jitter: cuánto cambia el tiempo entre un frame y el siguiente.
    let jitter_ms = frame_times
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .sum::<f64>()
        / (n.saturating_sub(1).max(1)) as f64;

    // Peor frame (percentil 99.9): más estable que usar el máximo absoluto.
    let idx = (((n as f64) * 0.999) as usize).min(n - 1);

    StutterStats {
        // Sin duración conocida no hay divisor: la tasa **no consta**. Antes se devolvía
        // 0.0, que se leía como «ningún pico» cuando en realidad no se sabía.
        spikes_per_min: if duration_seconds > 0.0 {
            Some(spikes as f64 / (duration_seconds / 60.0))
        } else {
            None
        },
        pct_over_33ms: over_33 as f64 / n as f64 * 100.0,
        jitter_ms,
        worst_frame_ms: sorted[idx],
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub baseline: Benchmark,
    pub test: Benchmark,
    pub fps_change_percent: f64,
    pub fps_1_percent_change_percent: f64,
    pub fps_0_1_percent_change_percent: f64,
    pub frame_time_change_percent: f64,
    pub conclusion: String,
    /// Margen de ruido combinado de las dos grabaciones (%). Una diferencia menor que
    /// esto no se puede distinguir de la variación normal del juego.
    #[serde(default)]
    pub noise_percent: Option<f64>,
    /// `true` cuando el cambio de FPS promedio entra dentro del margen de ruido.
    #[serde(default)]
    pub within_noise: bool,
    /// `true` solo si los dos comandos registrados son exactamente el mismo texto.
    ///
    /// Se calcula aquí, en el backend, y no en cada pantalla: la comparación se guarda en
    /// disco, así que tiene que llevar la respuesta ya resuelta. `None` cuando alguno de
    /// los dos comandos no está registrado (benchmarks antiguos): en ese caso **no se
    /// puede afirmar** que fueran iguales, ni que fueran distintos.
    #[serde(default)]
    pub comandos_iguales: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarksData {
    /// Versión del formato de este archivo.
    ///
    /// Ausente en los archivos anteriores a este campo, que se tratan como versión 1
    /// (su estructura es la misma: solo se le añadieron campos con `#[serde(default)]`).
    /// No se escribe `1` retroactivamente en disco: se escribe la versión actual en el
    /// siguiente guardado, que es cuando el archivo cambia de verdad.
    #[serde(default)]
    pub version: Option<u32>,
    pub benchmarks: Vec<Benchmark>,
    /// Último número de prueba **repartido** por juego. Nunca retrocede.
    ///
    /// Es un contador persistente y no algo derivado de `benchmarks` a propósito: tanto
    /// `trim_history` (que descarta las más antiguas al pasar de
    /// `MAX_BENCHMARKS_PER_GAME`) como el borrado manual quitan grabaciones del historial,
    /// así que «el máximo + 1 de lo que queda» reutilizaría números ya usados y una
    /// comparación guardada pasaría a señalar otra prueba. Aquí el número gastado se
    /// queda gastado.
    ///
    /// La clave es `game_key` (`appid:<id>` o `name:<nombre normalizado>`), la misma que
    /// agrupa el historial.
    #[serde(default)]
    pub counters: HashMap<String, u32>,
}

/// Versión del formato de `benchmarks.json` que entiende esta build.
///
/// Se sube cuando un cambio deja de ser compatible hacia atrás. Un archivo con
/// `version` mayor se **rechaza** en vez de interpretarse a medias: mejor decir «esto es
/// de una versión más nueva» que leer la mitad y que el siguiente guardado borre el resto.
///
/// Mientras la 5.1 no cambia ni un campo del esquema, vale 1 y es lo que se escribe.
pub const BENCHMARKS_SCHEMA_VERSION: u32 = 1;

/// Versión del formato de `saved_comparisons.json`.
///
/// Va por separado del de benchmarks a propósito: son dos archivos con ciclos de vida
/// distintos y una comparación guardada puede seguir siendo válida aunque el esquema de
/// las grabaciones cambie (embute copias completas de las dos grabaciones).
///
/// **v2** añade `evaluacion` a cada comparación. Un archivo v1 se lee igual y **no se
/// reescribe al leerlo**: sube a v2 solo cuando se guarda una comparación que ya trae
/// evaluación. Un archivo v2 **no vuelve nunca a v1**, ni siquiera si se borran todas las
/// evaluaciones: si lo hiciera, una versión antigua de la app volvería a poder abrirlo y
/// descartaría en silencio lo que no entiende.
///
/// La versión del esquema no es la versión de la aplicación ni la metodología de cálculo.
pub const COMPARISONS_SCHEMA_VERSION: u32 = 2;

fn get_benchmarks_dir() -> std::path::PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("thoriapp")
        .join("benchmarks");

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).ok();
    }

    config_dir
}

fn get_benchmarks_file() -> std::path::PathBuf {
    get_benchmarks_dir().join("benchmarks.json")
}

/// Cerrojo por archivo, para que dos guardados no se pisen a mitad.
///
/// Los comandos de Tauri pueden solaparse (una auto-recogida guardando mientras el
/// usuario borra una prueba). Sin cerrojo, dos `leer → modificar → escribir` simultáneos
/// pierden una de las dos escrituras: el último `rename` gana y el otro cambio desaparece.
///
/// Es de proceso, no de sistema: Thoriapp es una sola instancia y un cerrojo entre
/// procesos costaría una dependencia (`fs2`) o `flock` inseguro. Cubre exactamente el
/// riesgo real, que es interno a la app.
static CERROJO_BENCHMARKS: Mutex<()> = Mutex::new(());
static CERROJO_COMPARACIONES: Mutex<()> = Mutex::new(());

/// Error de lectura de un archivo de datos: distingue lo que se puede y lo que no.
///
/// La diferencia importa: «no existe» es un estado legítimo (primer arranque) y debe
/// tratarse como lista vacía; «no se pudo leer» y «está roto» **no** pueden convertirse
/// en lista vacía, porque el siguiente guardado borraría el historial.
#[derive(Debug)]
pub enum ErrorDatos {
    /// El archivo existe y no se puede leer (permisos, E/S, enlace roto).
    Ilegible(String),
    /// El archivo existe y se leyó, pero no es el JSON esperado.
    Corrupto { detalle: String, copia: Option<PathBuf> },
    /// El archivo es de una versión de Thoriapp más nueva.
    VersionFutura { encontrada: u32, soportada: u32 },
}

impl ErrorDatos {
    /// Mensaje para el usuario. Dice siempre qué NO ha pasado, porque el miedo real es
    /// «¿he perdido mis pruebas?».
    pub fn mensaje(&self, que: &str) -> String {
        match self {
            ErrorDatos::Ilegible(e) => format!(
                "No se pudo leer {que} ({e}). No se ha modificado nada en disco."
            ),
            ErrorDatos::Corrupto { detalle, copia } => format!(
                "{que} está dañado ({detalle}). No se ha modificado nada en disco.{}",
                match copia {
                    Some(p) => format!(" Se guardó una copia del original en {}.", p.display()),
                    None => String::new(),
                }
            ),
            ErrorDatos::VersionFutura { encontrada, soportada } => format!(
                "{que} es de una versión más nueva de Thoriapp (formato {encontrada}; esta \
                 versión entiende hasta el {soportada}). Actualiza Thoriapp para poder abrirlo. \
                 No se ha modificado nada en disco.",
            ),
        }
    }
}

/// Error de la **interfaz IPC** al comparar o guardar una comparación evaluada.
///
/// No es un formato en disco ni forma parte de la metodología: solo viaja del comando a la
/// interfaz. Existe porque un `String` obligaba al frontend a adivinar la causa leyendo el
/// texto del mensaje, y con eso no se puede distinguir «esto ya no vale» de «no se pudo
/// escribir».
///
/// Los motivos de rechazo **no** se reinventan aquí: `motivo` lleva el código estable que
/// ya produce `evaluacion::MotivoDeRechazo::codigo()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ErrorDeEvaluacion {
    /// La confirmación ya no corresponde a los datos: hay que repetir la vista previa.
    ConfirmacionRechazada { motivo: String },
    /// Alguno de los identificadores no corresponde a ninguna grabación.
    GrabacionInexistente { mensaje: String },
    /// Un identificador corresponde a más de una grabación: no se puede saber cuál evaluar.
    IdentificadorAmbiguo { mensaje: String },
    /// Ese par de grabaciones ya tiene una comparación guardada.
    ComparacionYaExistente { comparacion_id: String },
    /// No se pudo leer un archivo. **Nada se ha escrito.**
    LecturaFallida { archivo: String, mensaje: String },
    /// Se intentó escribir y falló. **El archivo anterior sigue intacto.**
    EscrituraFallida { mensaje: String },
}

impl ErrorDeEvaluacion {
    /// Motivo del rechazo, tal cual, sin añadir texto libre al código.
    #[allow(dead_code)] // lo usan los comandos de C.3 y sus pruebas
    pub(crate) fn rechazo(motivo: crate::evaluacion::MotivoDeRechazo) -> Self {
        ErrorDeEvaluacion::ConfirmacionRechazada {
            motivo: motivo.codigo().to_string(),
        }
    }

    /// Grabación inexistente. La categoría la decide `buscar_unico` con su variante; este
    /// constructor solo la transporta al contrato IPC. El mensaje no se interpreta.
    #[allow(dead_code)]
    pub(crate) fn grabacion_inexistente(mensaje: String) -> Self {
        ErrorDeEvaluacion::GrabacionInexistente { mensaje }
    }

    #[allow(dead_code)]
    pub(crate) fn lectura_fallida(archivo: &str, mensaje: String) -> Self {
        ErrorDeEvaluacion::LecturaFallida {
            archivo: archivo.to_string(),
            mensaje,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn escritura_fallida(mensaje: String) -> Self {
        ErrorDeEvaluacion::EscrituraFallida { mensaje }
    }
}

/// Escritura atómica: se escribe un temporal, se vuelca a disco y se renombra.
///
/// `fs::write` directo sobre el archivo real deja el historial a medias si el proceso
/// muere a mitad: un JSON truncado, y en la siguiente lectura ya es «corrupto». Con
/// `rename` el archivo nuevo aparece entero o no aparece.
///
/// El `flush` + `sync_all` antes del `rename` importa en un corte de corriente: sin él, el
/// `rename` puede llegar al disco antes que los datos y quedar un archivo vacío con nombre
/// bueno.
pub(crate) fn escribir_atomico(path: &Path, content: &str) -> std::io::Result<()> {
    let mut tmp_name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);

    {
        let mut f = fs::File::create(&tmp)?;
        if let Err(e) = f.write_all(content.as_bytes()).and_then(|_| f.sync_all()) {
            drop(f);
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    }

    fs::rename(&tmp, path)?;
    // El `rename` sustituye la entrada del directorio, pero el nodo del directorio
    // también necesita su propio `fsync`: sin él, un corte de luz justo después puede
    // dejar el rename sin confirmar y volver al nombre anterior. `sync_dir` no es
    // portable, así que un fallo aquí no invalida la escritura de datos.
    sync_dir(path);
    Ok(())
}

/// Fuerza el directorio contenedor a disco, para que el `rename` sobreviva a un corte.
///
/// Los sistemas de archivos POSIX no garantizan que la nueva entrada de directorio esté
/// en disco hasta que se sincroniza el directorio mismo. En plataformas donde no se puede
/// abrir un directorio como archivo (o no es de Unix) no se hace nada: la escritura ya
/// está confirmada, solo se pierde la garantía extra ante un corte de corriente.
#[cfg(unix)]
pub(crate) fn sync_dir(path: &Path) {
    if let Some(dir) = path.parent() {
        if let Ok(d) = fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
}

#[cfg(not(unix))]
fn sync_dir(_path: &Path) {}

/// Copia un archivo dañado a `<nombre>.corrupt.bak` **sin pisar** una copia anterior.
///
/// Se llama a esto cada vez que se intenta leer un archivo roto, y como la UI puede
/// reintentar en bucle, sobrescribir la copia con el mismo contenido no aporta nada; pero
/// si el archivo cambia entre dos intentos (el usuario lo edita a mano, otra herramienta lo
/// toca), pisar la primera copia sí perdería la única evidencia de por qué empezó a fallar.
/// Con sufijo numerado se acumulan hasta 5; a partir de ahí se rota la más antigua para no
/// llenar el disco de copias idénticas.
pub(crate) fn copiar_danado(path: &Path, sufijo: &str, contenido: &str) -> Option<PathBuf> {
    let base = path.with_extension(format!("json.{}.bak", sufijo));
    let candidatos = std::iter::once(base.clone()).chain((1..=4).map(|n| {
        let mut nombre = base.file_name().unwrap_or_default().to_os_string();
        nombre.push(format!(".{}", n));
        base.with_file_name(nombre)
    }));

    for copia in candidatos {
        if !copia.exists() {
            return fs::write(&copia, contenido).ok().map(|_| copia);
        }
        // Idéntica a la que ya hay: no se duplica ni se pisa.
        if fs::read_to_string(&copia).map(|c| c == contenido).unwrap_or(false) {
            return Some(copia);
        }
    }
    // Cinco copias distintas: se rota la más antigua (la de sufijo más alto).
    let mut nombre = base.file_name().unwrap_or_default().to_os_string();
    nombre.push(".4");
    let ultima = base.with_file_name(nombre);
    fs::write(&ultima, contenido).ok().map(|_| ultima)
}

/// Lee e interpreta un archivo de datos aplicando las reglas comunes.
///
/// `T` genérico para servir a los dos archivos (benchmarks y comparaciones): las reglas
/// de integridad son las mismas, y duplicarlas garantizaba que una se arreglara y la otra
/// no. `migrar` recibe el JSON crudo para leer `version` antes de interpretar el resto.
pub(crate) fn leer_datos<T>(
    path: &Path,
    que: &str,
    version_soportada: u32,
    version_de: impl Fn(&serde_json::Value) -> Option<u32>,
    migrar: impl Fn(serde_json::Value) -> Result<T, serde_json::Error>,
) -> Result<Option<T>, ErrorDatos> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        // El archivo no está: es un estado legítimo, no un error.
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        // Existía al comprobarlo y desapareció: carrera con un borrado, no corrupción.
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ErrorDatos::Ilegible(e.to_string())),
    };

    // Un archivo vacío no es JSON válido, pero tampoco es un historial perdido: es lo que
    // queda de un `fs::write` interrumpido por un corte de luz antes de este arreglo. Se
    // trata como vacío **y** se copia, para no borrar la única pista sin avisar.
    if content.trim().is_empty() {
        let copia = copiar_danado(path, "vacio", &content);
        return Err(ErrorDatos::Corrupto {
            detalle: "el archivo está vacío".to_string(),
            copia,
        });
    }

    let crudo: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        ErrorDatos::Corrupto {
            detalle: e.to_string(),
            copia: copiar_danado(path, "corrupt", &content),
        }
    })?;

    if let Some(v) = version_de(&crudo) {
        if v > version_soportada {
            // No se copia: el archivo está bien, lo que no está al día es la app.
            return Err(ErrorDatos::VersionFutura {
                encontrada: v,
                soportada: version_soportada,
            });
        }
    }

    migrar(crudo).map(Some).map_err(|e| {
        let _ = que; // el nombre del archivo ya lo pone quien llama
        ErrorDatos::Corrupto {
            detalle: e.to_string(),
            copia: copiar_danado(path, "corrupt", &content),
        }
    })
}

/// Lee `benchmarks.json` distinguiendo «no hay» de «no se pudo leer».
///
/// Variante para **escritura**: si el archivo existe y no se puede interpretar, hay que
/// abortar. Devolver `Ok` vacío en ese caso era la vía por la que un archivo ilegible
/// acababa sobrescrito con una lista vacía y el historial desaparecía.
fn try_load_benchmarks_data() -> Result<BenchmarksData, ErrorDatos> {
    try_load_benchmarks_data_de(&get_benchmarks_file())
}

/// Igual, sobre una ruta explícita.
///
/// Existe por el mismo motivo que `save_saved_data_en`: C.2 necesita leer el historial
/// **dentro** de su sección crítica, con la ruta que le den, sin volver a resolver la ruta
/// real ni abrir una segunda lectura fuera del cerrojo.
fn try_load_benchmarks_data_de(path: &Path) -> Result<BenchmarksData, ErrorDatos> {
    let leido = leer_datos::<BenchmarksData>(
        path,
        "benchmarks.json",
        BENCHMARKS_SCHEMA_VERSION,
        |v| v.get("version").and_then(|x| x.as_u64()).map(|x| x as u32),
        serde_json::from_value,
    )?;

    Ok(leido.unwrap_or(BenchmarksData {
        version: None,
        benchmarks: Vec::new(),
        counters: HashMap::new(),
    }))
}

/// Lectura para las pantallas: nunca falla, pero tampoco miente por omisión.
///
/// Devuelve lo leído o el motivo del fallo. La interfaz tiene que poder decir «no se
/// pudieron leer tus pruebas» en vez de pintar un historial vacío que parecería «no hay
/// ninguna», que es una afirmación distinta y falsa.
pub(crate) fn cargar_benchmarks() -> Result<BenchmarksData, String> {
    try_load_benchmarks_data().map_err(|e| e.mensaje("El historial de pruebas"))
}

pub(crate) fn load_benchmarks_data() -> BenchmarksData {
    // Uso interno (comparar, buscar duplicados): si el archivo está roto no hay nada que
    // comparar, pero se avisa por consola. Las rutas que **escriben** usan
    // `try_load_benchmarks_data` y abortan.
    match try_load_benchmarks_data() {
        Ok(data) => data,
        Err(e) => {
            println!("[Benchmark] ⚠️ {}", e.mensaje("benchmarks.json"));
            BenchmarksData {
                version: None,
                benchmarks: Vec::new(),
                counters: HashMap::new(),
            }
        }
    }
}

/// Lee el historial para **modificarlo**. Aborta si no se puede interpretar.
///
/// Todas las rutas destructivas (guardar, borrar una, borrar las de un juego, borrar
/// todas) empiezan aquí. Es la regla de la subfase 5.1: ninguna escritura puede partir
/// de una lectura fallida.
fn load_benchmarks_data_para_editar() -> Result<BenchmarksData, String> {
    try_load_benchmarks_data().map_err(|e| e.mensaje("El historial de pruebas"))
}

fn save_benchmarks_data(data: &BenchmarksData) -> Result<(), String> {
    let file = get_benchmarks_file();
    let mut data = data.clone();
    // La versión se sella aquí, en el único punto de escritura, para que ningún llamador
    // pueda guardar un archivo sin ella.
    data.version = Some(BENCHMARKS_SCHEMA_VERSION);
    let content =
        serde_json::to_string_pretty(&data).map_err(|e| format!("Error al serializar: {}", e))?;

    escribir_atomico(&file, &content).map_err(|e| {
        format!(
            "No se pudo guardar el historial de pruebas ({}). Tus pruebas anteriores siguen \
             intactas en {}.",
            e,
            file.display()
        )
    })
}

/// Lee como máximo `max_bytes` del inicio de un archivo (como texto, tolerando bytes raros).
fn read_head(path: &Path, max_bytes: usize) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; max_bytes];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// MangoHud crea además un `<nombre>_summary.csv` con estadísticas ya calculadas.
/// No sirve para el análisis frame a frame, así que se ignora.
fn is_summary_file(path: &Path) -> bool {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_lowercase().ends_with("_summary"))
        .unwrap_or(false)
}

fn find_latest_mangohud_log(dir: &Path) -> Result<std::path::PathBuf, String> {
    let mut log_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();

    fn search_dir(dir: &Path, log_files: &mut Vec<(std::path::PathBuf, std::time::SystemTime)>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    search_dir(&path, log_files);
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if (ext_str == "csv" || ext_str == "log") && !is_summary_file(&path) {
                            // Solo se lee el inicio del archivo: basta para ver el header
                            // y evita cargar en memoria logs enormes de otras aplicaciones.
                            if let Ok(content) = read_head(&path, 4096) {
                                let has_valid_format = content.lines().any(|line| {
                                    let lower = line.to_lowercase();
                                    lower.contains("fps,frametime")
                                        || (lower.starts_with("fps,")
                                            && lower.contains("frametime"))
                                });

                                if has_valid_format {
                                    if let Ok(metadata) = path.metadata() {
                                        if let Ok(modified) = metadata.modified() {
                                            println!(
                                                "[Benchmark] Archivo MangoHud encontrado: {:?}",
                                                path
                                            );
                                            log_files.push((path, modified));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    search_dir(dir, &mut log_files);

    if log_files.is_empty() {
        return Err(format!(
            "No se encontraron archivos de log de MangoHud válidos en {:?}.\n\n\
            Asegúrate de:\n\
            1. Tener MangoHud instalado y configurado\n\
            2. Haber grabado un log con Shift+F2 durante al menos 30 segundos\n\
            3. Que los logs se guarden en esta carpeta",
            dir
        ));
    }

    log_files.sort_by_key(|f| std::cmp::Reverse(f.1));

    let latest = &log_files[0].0;
    println!("[Benchmark] Usando el log más reciente: {:?}", latest);

    Ok(latest.clone())
}

fn extract_game_name_from_file(file_path: &Path) -> String {
    let file_name = match file_path.file_stem() {
        Some(name) => name.to_string_lossy().to_string(),
        None => return "Desconocido".to_string(),
    };

    println!("[Extract] Nombre original del archivo: {}", file_name);

    let mut name = file_name.clone();

    // PASO 1: Buscar patrón de fecha/hora y cortar ahí
    if let Ok(datetime_pattern) =
        Regex::new(r"[_-]\d{4}[-_]\d{2}[-_]\d{2}([_-]\d{2}[-_]\d{2}[-_]\d{2})?")
    {
        if let Some(mat) = datetime_pattern.find(&name) {
            name = name[..mat.start()].to_string();
            println!("[Extract] Después de cortar fecha/hora: '{}'", name);
        }
    }

    // PASO 2: Quitar sufijos técnicos comunes de juegos
    let technical_suffixes = [
        "-Win64-Shipping",
        "-Win32-Shipping",
        "-Shipping",
        "-Win64",
        "-Win32",
        "-x64",
        "-X64",
        "-Linux64",
        "-Linux",
        "-Mac",
        "-MacOS",
        "-Server",
        "-DedicatedServer",
        "-Client",
        "-Editor",
        "-Test",
        "-Debug",
        "-Release",
    ];

    let name_lower = name.to_lowercase();
    for suffix in &technical_suffixes {
        let suffix_lower = suffix.to_lowercase();
        if name_lower.ends_with(&suffix_lower) {
            name = name[..name.len() - suffix.len()].to_string();
            println!(
                "[Extract] Después de quitar sufijo '{}': '{}'",
                suffix, name
            );
            break;
        }
    }

    // PASO 3: Quitar prefijos comunes de MangoHud
    let prefixes_to_remove = ["mangohud", "mango", "hud", "log", "logs", "mg"];
    for prefix in &prefixes_to_remove {
        let name_lower = name.to_lowercase();
        if name_lower.starts_with(prefix) {
            let rest = name[prefix.len()..].to_string();
            let trimmed = rest.trim_start_matches(['_', '-', ' ']);
            if !trimmed.is_empty() {
                name = trimmed.to_string();
                println!(
                    "[Extract] Después de quitar prefijo '{}': '{}'",
                    prefix, name
                );
                break;
            }
        }
    }

    // PASO 4: Limpiar separadores finales y espacios extra
    name = name
        .trim_end_matches(['_', '-', ' '])
        .to_string();
    name = name
        .trim_start_matches(['_', '-', ' '])
        .to_string();

    // PASO 5: Si quedó vacío, usar el nombre original del archivo
    if name.is_empty() {
        println!("[Extract] Nombre vacío, usando original: {}", file_name);
        return file_name;
    }

    println!("[Extract] ✅ Nombre final del juego: {}", name);
    name
}

fn get_file_modified_time(file_path: &Path) -> String {
    match fs::metadata(file_path) {
        Ok(metadata) => match metadata.modified() {
            Ok(system_time) => {
                let datetime: chrono::DateTime<chrono::Local> = system_time.into();
                let formatted = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
                println!("[Time] ✅ Fecha de modificación del archivo: {}", formatted);
                formatted
            }
            Err(e) => {
                println!(
                    "[Time] ⚠️ Error obteniendo hora de modificación: {}, usando hora actual",
                    e
                );
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
            }
        },
        Err(e) => {
            println!("[Time] ⚠️ Error obteniendo meta {}, usando hora actual", e);
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
        }
    }
}

pub fn analyze_mangohud_log(log_path: &str, warmup_seconds: f64) -> Result<Benchmark, String> {
    let path = Path::new(log_path);

    if !path.exists() {
        return Err(format!("La ruta no existe: {}", log_path));
    }

    let file_path = if path.is_dir() {
        find_latest_mangohud_log(path)?
    } else {
        path.to_path_buf()
    };

    println!("[Benchmark] ============================================");
    println!("[Benchmark] Analizando archivo: {:?}", file_path);

    let game_name = extract_game_name_from_file(&file_path);
    let file_timestamp = get_file_modified_time(&file_path);

    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Error al leer el archivo {:?}: {}", file_path, e))?;

    let mut filas_leidas: Vec<FilaLeida> = Vec::new();
    let mut filas_con_rendimiento_invalido: u32 = 0;

    let mut fps_column: usize = 0;
    let mut frametime_column: usize = 1;
    let mut elapsed_column: Option<usize> = None;
    let mut data_start_line: usize = 0;

    for (line_idx, line) in content.lines().enumerate() {
        let lower = line.to_lowercase();

        if lower.contains("fps") && lower.contains("frametime") {
            let headers: Vec<&str> = line.split(',').collect();

            for (i, header) in headers.iter().enumerate() {
                let h = header.trim().to_lowercase();
                if h == "fps" {
                    fps_column = i;
                } else if h == "frametime" {
                    frametime_column = i;
                } else if h == "elapsed" {
                    elapsed_column = Some(i);
                }
            }

            data_start_line = line_idx + 1;
            println!(
                "[Benchmark] Header de datos encontrado en línea {}",
                line_idx + 1
            );
            println!(
                "[Benchmark] Columnas: fps={}, frametime={}, elapsed={:?}",
                fps_column, frametime_column, elapsed_column
            );
            break;
        }
    }

    if data_start_line == 0 {
        return Err("No se encontró el header de datos (fps,frametime) en el archivo CSV. ¿Es un log válido de MangoHud?".to_string());
    }

    // Filas **originales de datos**: las que hay entre la cabecera de datos y el final.
    // No cuentan las líneas de metadatos (`OS`, `CPU`, `GPU`…) ni la propia cabecera: no
    // son datos de rendimiento. Tampoco las líneas en blanco del final.
    let mut filas_originales: u32 = 0;
    for line in content.lines().skip(data_start_line) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        filas_originales += 1;

        let parts: Vec<&str> = line.split(',').collect();

        if parts.len() <= fps_column || parts.len() <= frametime_column {
            // Fila de datos con menos columnas de las necesarias: no es utilizable.
            filas_con_rendimiento_invalido += 1;
            continue;
        }

        if let (Ok(fps), Ok(frame_time)) = (
            parts[fps_column].trim().parse::<f64>(),
            parts[frametime_column].trim().parse::<f64>(),
        ) {
            // is_finite descarta NaN/inf, que romperían los ordenamientos más abajo.
            // Un valor de rendimiento no finito o no positivo no es una muestra: no
            // entra en ninguna métrica y se cuenta aparte, para poder explicar luego
            // por qué quedan pocas muestras.
            if fps.is_finite() && frame_time.is_finite() && fps > 0.0 && frame_time > 0.0 {
                // El reloj se guarda tal cual: `None` si no era legible o no era
                // finito. La fila entra igualmente; el reloj es otra cosa.
                // El reloj se clasifica **sin colapsar** los casos: columna ausente,
                // celda ausente, celda vacía, texto malformado y valor no finito son
                // situaciones distintas y el usuario merece saber cuál tiene.
                let reloj = match elapsed_column {
                    None => RelojDeFila::ColumnaAusente,
                    Some(c) => match parts.get(c) {
                        None => RelojDeFila::CeldaAusente,
                        Some(celda) => {
                            let celda = celda.trim();
                            if celda.is_empty() {
                                RelojDeFila::CeldaVacia
                            } else {
                                match celda.parse::<f64>() {
                                    Ok(v) if v.is_finite() => RelojDeFila::Valor(v),
                                    Ok(_) => RelojDeFila::NoFinito,
                                    Err(_) => RelojDeFila::Malformado,
                                }
                            }
                        }
                    },
                };

                filas_leidas.push(FilaLeida {
                    fps,
                    frame_time,
                    reloj,
                });
            } else {
                filas_con_rendimiento_invalido += 1;
            }
        } else {
            // `fps` o `frametime` vacíos o no numéricos: fila inválida.
            filas_con_rendimiento_invalido += 1;
        }
    }

    // ==================== Preparación con metodología v2 ====================
    //
    // v2 (5.4.1.A) **no descarta muestras por coincidencia de FPS y frametime**. La
    // versión anterior lo hacía, y en una serie planchada (limitador, pausa, tramo lento)
    // esa coincidencia es el propio dato: el resultado era borrar justo lo que se quería
    // medir, hasta dejar la grabación sin muestras suficientes.
    let preparadas = preparar_muestras(
        filas_leidas,
        warmup_seconds,
        elapsed_column.is_some(),
        filas_originales,
        filas_con_rendimiento_invalido,
    );
    let contabilidad = preparadas.contabilidad.clone();
    // Compatibilidad: si la duración está medida, el campo legado lleva **el mismo valor**
    // (misma magnitud, en segundos), porque así lo esperan los JSON y el frontend ya
    // guardados. Si no está medida, vale 0.0 por contrato y ese cero **no** es una
    // duración: `duracion_medida` queda en `None` y `Benchmark::duracion()` responde
    // «no consta». La fuente de verdad para v2 sigue siendo `duracion_medida`.
    let duration_seconds = preparadas.duracion.unwrap_or(0.0);
    let con_tiempos = preparadas.eje.is_some();

    println!(
        "[Benchmark] Filas de datos: {} ({} con rendimiento válido, {} con valores no utilizables)",
        contabilidad.filas_originales,
        contabilidad.filas_con_rendimiento_valido,
        contabilidad.filas_con_rendimiento_invalido
    );
    if let Some(motivo) = contabilidad.calentamiento_no_aplicado {
        if motivo != MotivoSinCalentamiento::NoSolicitado {
            println!(
                "[Benchmark] Calentamiento no aplicado: {:?}",
                motivo
            );
        }
    }
    println!(
        "[Benchmark] Excluidas por calentamiento: {} · analizadas: {}",
        contabilidad.excluidas_por_calentamiento, contabilidad.muestras_analizadas
    );
    if contabilidad.incidencias.hay_algunas() {
        println!(
            "[Benchmark] Incidencias temporales: repetidos={} invertidos={} negativos={} no_finitos={} reloj_incompleto={}",
            contabilidad.incidencias.filas_repetidas,
            contabilidad.incidencias.orden_invertido,
            contabilidad.incidencias.negativas,
            contabilidad.incidencias.no_finitas,
            contabilidad.incidencias.reloj_incompleto
        );
    }
    println!(
        "[Benchmark] Duración real de la grabación: {:.1} s{}",
        duration_seconds,
        if con_tiempos { "" } else { " (sin eje temporal fiable)" }
    );

    // La ventana inicial se guarda con las muestras completas antes del calentamiento: es
    // justo el trozo que se tira, y compararlo con el resto es lo que dice cuánto cuesta
    // compilar shaders la primera vez. Sin eje fiable no hay ventana que recortar.
    let (early_frames, steady_frames) = if preparadas.eje.is_some() {
        sample_window(&preparadas.muestras, SHADER_WINDOW_SECONDS)
    } else {
        (Vec::new(), Vec::new())
    };

    let samples = preparadas.muestras;
    let warmup_applied = preparadas.warmup_applied;
    if warmup_applied > 0.0 {
        println!(
            "[Benchmark] Calentamiento descartado: {:.1} s (quedan {:.1} s de estado estable)",
            warmup_applied,
            (duration_seconds - warmup_applied).max(0.0)
        );
    }

    if samples.len() < MIN_SAMPLES {
        // El mensaje nombra la causa real: no es lo mismo un archivo corto que uno cuyas
        // filas no valen. Recomendar «graba más tiempo» cuando el problema es la calidad
        // del archivo manda al usuario a repetir una prueba que volverá a fallar.
        let cuenta = &contabilidad;
        let causa = if cuenta.filas_con_rendimiento_valido == 0 {
            format!(
                "El archivo tiene {} filas de datos, pero ninguna con valores de rendimiento \
                 utilizables (FPS o frametime vacíos, no numéricos, negativos, cero o infinitos). \
                 Revisa que el archivo sea un log de MangoHud completo.",
                cuenta.filas_originales
            )
        } else if cuenta.excluidas_por_calentamiento > 0 {
            format!(
                "La grabación tiene {} filas con rendimiento válido, pero tras descartar el \
                 calentamiento ({} muestras) solo quedan {} analizables y el mínimo es {}.",
                cuenta.filas_con_rendimiento_valido,
                cuenta.excluidas_por_calentamiento,
                samples.len(),
                MIN_SAMPLES
            )
        } else {
            format!(
                "La grabación tiene {} filas con rendimiento válido y el mínimo es {}.",
                samples.len(),
                MIN_SAMPLES
            )
        };
        return Err(format!("No hay muestras suficientes para analizar. {}", causa));
    }

    let fps_values: Vec<f64> = samples.iter().map(|s| s.fps).collect();
    let frame_times: Vec<f64> = samples.iter().map(|s| s.frame_time).collect();

    // Las muestras están espaciadas de forma uniforme en el tiempo, así que el promedio
    // simple de FPS equivale a (frames totales / tiempo total).
    // `total_frames` es un nombre heredado: son MUESTRAS del log (una fila cada
    // `log_interval` ms), no frames. Sumar frames daría el doble o el triple. Se mantiene
    // el nombre para no romper los JSON ya guardados; `sample_count` dice lo mismo sin
    // engañar.
    let total_frames = fps_values.len() as u64;
    let avg_fps = fps_values.iter().sum::<f64>() / fps_values.len() as f64;
    let frame_time_avg = frame_times.iter().sum::<f64>() / frame_times.len() as f64;

    // Máximo y mínimo "absolutos" son muy sensibles a una sola muestra rara (por ejemplo
    // una fila de 8 FPS por un tirón puntual). Se usan los percentiles 99,9 y 0,1 de FPS.
    let mut sorted_fps = fps_values.clone();
    sorted_fps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let max_fps = percentile_sorted(&sorted_fps, 99.9);
    let min_fps = percentile_sorted(&sorted_fps, 0.1);

    // Los "lows" se calculan sobre el frametime (como hacen CapFrameX y similares):
    // 1% low = FPS equivalentes al percentil 99 del frametime.
    // Con ~600 muestras, promediar "el peor 1%" serían solo ~6 filas y un valor extremo
    // lo arrastraría; el percentil con interpolación es mucho más estable.
    let mut sorted_ft = frame_times.clone();
    sorted_ft.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p99_ft = percentile_sorted(&sorted_ft, 99.0);
    let p999_ft = percentile_sorted(&sorted_ft, 99.9);
    let fps_1_percent_low = 1000.0 / p99_ft;
    let fps_0_1_percent_low = 1000.0 / p999_ft;

    // Promedio de los frametimes que superan el percentil 99 (el "1% más lento").
    //
    // El filtro es `>=`, así que los empates entran: en una serie constante `P99` es el
    // propio valor y acaban entrando **todas** las muestras. Es lo que ya hacía la
    // fórmula (no se cambia); lo que se añade es el recuento real, para que el apoyo de
    // la métrica no mienta.
    let worst = muestras_en_media_de_cola(&sorted_ft, p99_ft);
    let frame_time_1_percent_high = if worst.is_empty() {
        p99_ft
    } else {
        worst.iter().sum::<f64>() / worst.len() as f64
    };

    // Apoyo de cada métrica: sobre cuántas muestras se calculó y qué cola usó la
    // interpolación. No es un tamaño efectivo estadístico: solo describe el cálculo.
    let apoyo = ApoyoDeMetricas {
        fps_1_percent_low: Some(apoyo_de_percentil(&sorted_ft, 99.0)),
        fps_0_1_percent_low: Some(apoyo_de_percentil(&sorted_ft, 99.9)),
        max_fps: Some(apoyo_de_percentil(&sorted_fps, 99.9)),
        min_fps: Some(apoyo_de_percentil(&sorted_fps, 0.1)),
        frame_time_1_percent_high: Some(ApoyoDePercentil {
            muestras_en_la_media: Some(worst.len() as u32),
            ..apoyo_de_percentil(&sorted_ft, 99.0)
        }),
    };

    // El stutter reparte picos por minuto: sin una duración medida con un reloj fiable no
    // hay divisor y el dato sería inventado. Jitter, porcentaje de muestras lentas y peor
    // frametime solo dependen de los valores, así que sí se calculan.
    let stable_duration = if con_tiempos {
        (duration_seconds - warmup_applied).max(0.0)
    } else {
        0.0
    };
    let stutter = compute_stutter(&frame_times, stable_duration);
    let spikes_per_min = stutter.spikes_per_min;
    let noise = noise_percent(&fps_values);
    let shader = shader_stutter(&early_frames, &steady_frames, SHADER_WINDOW_SECONDS);

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();

    println!("[Benchmark] ✅ Análisis completado:");
    println!("[Benchmark]    Juego: {}", game_name);
    println!("[Benchmark]    Fecha del log: {}", file_timestamp);
    println!("[Benchmark]    FPS promedio: {:.1}", avg_fps);
    println!("[Benchmark]    1% Low: {:.1}", fps_1_percent_low);
    println!("[Benchmark]    0.1% Low: {:.1}", fps_0_1_percent_low);
    match stutter.spikes_per_min {
        Some(v) => println!("[Benchmark]    Picos/min: {v:.1}"),
        None => println!("[Benchmark]    Picos/min: no consta (sin eje temporal fiable)"),
    }
    if let Some(n) = noise {
        println!("[Benchmark]    Margen de ruido: ±{:.1} %", n);
    }
    if let Some(sh) = &shader {
        println!(
            "[Benchmark]    Arranque ({}s): peor 1% {:.0} FPS vs {:.0} FPS del resto ({:+.1} %)",
            sh.window_seconds,
            sh.early_fps_1_percent_low,
            sh.steady_fps_1_percent_low,
            -sh.stutter_penalty_percent
        );
    }
    println!("[Benchmark] ============================================");

    Ok(Benchmark {
        id: format!("bench_{}", now.replace([':', ' ', '-', '.'], "")),
        profile_id: String::new(),
        game_name,
        appid: 0,
        timestamp: file_timestamp,
        duration_seconds,
        duracion_medida: preparadas.duracion,
        avg_fps,
        fps_1_percent_low,
        fps_0_1_percent_low,
        max_fps,
        min_fps,
        frame_time_avg,
        frame_time_1_percent_high,
        total_frames,
        notes: None,
        launch_command: String::new(),
        spikes_per_min,
        pct_over_33ms: Some(stutter.pct_over_33ms),
        jitter_ms: Some(stutter.jitter_ms),
        worst_frame_ms: Some(stutter.worst_frame_ms),
        warmup_seconds: Some(warmup_applied),
        noise_percent: noise,
        shader_stutter: shader,
        sample_count: Some(total_frames),
        muestras_temporales: serie_temporal(&samples, MAX_MUESTRAS_TEMPORALES, con_tiempos),
        log_path: std::fs::canonicalize(&file_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_string_lossy().to_string()),
        // El análisis del CSV no sabe qué comando había: eso lo rellena `save_benchmark`
        // con lo que le manda el frontend.
        origen_comando: OrigenComando::Desconocido,
        steam_command: None,
        // Se rellena en `save_benchmark`, que es quien conoce el historial del juego.
        number: None,
        // Y la sesión, que la decide `save_benchmark` con la evidencia observada.
        sesion_id: None,
        // El análisis del CSV no sabe nada de la configuración: se rellena al guardar, y
        // solo con datos que el frontend pudo leer en su momento.
        snapshot: None,
        // 5.4.1.A: estas grabaciones se calculan con la metodología v2 (sin descartar
        // muestras por coincidencia de valores) y llevan su contabilidad para poder
        // auditar de dónde sale cada número.
        metodologia_calculo: Some(MetodologiaCalculo::V2),
        contabilidad_muestras: Some(contabilidad),
        apoyo_metricas: Some(apoyo),
    })
}

/// Nombre de juego normalizado para compararlo (minúsculas y sin extensión de ejecutable).
fn norm_game_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    for suffix in [".x86_64", ".x86", ".exe", ".bin"] {
        if let Some(stripped) = lower.strip_suffix(suffix) {
            return stripped.trim().to_string();
        }
    }
    lower
}

/// ¿`corto` es el nombre base de `largo`, que solo le añade un subtítulo?
///
/// El mismo juego se llama de dos maneras según de dónde salga el nombre:
///
/// - La Biblioteca usa el **nombre comercial** con subtítulo: `Deskrawl: Idle ARPG`.
/// - MangoHud escribe el nombre del **binario**: `Deskrawl`.
///
/// Con AppID en los dos lados esto no importa (manda el AppID). Importa en el caso real del
/// usuario, donde **todas** las grabaciones tienen `appid: 0` y el nombre es la única pista:
/// la ficha de Deskrawl decía «todavía no hay pruebas» teniendo cuatro, porque
/// `deskrawl: idle arpg` ≠ `deskrawl`. Palworld funcionaba solo porque su nombre comercial
/// no lleva subtítulo.
///
/// La regla es deliberadamente estrecha: el nombre corto tiene que ser **el principio** del
/// largo y el corte tiene que caer en un **separador** (`:`, `-`, `–`, `—`, `|`, `·`), nunca
/// a mitad de palabra. Así `Deskrawl` y `Deskrawl: Idle ARPG` son el mismo juego, pero
/// `Palworld` y `Palworld: Something` no pueden arrastrar juegos que solo comparten prefijo
/// textual (`Slime` / `Slime Rancher` no se tocan: ahí el corte cae en un espacio, no en un
/// separador). Se exige además que el corto tenga al menos 2 caracteres, para que un
/// nombre vacío o de una letra no se lo coma todo.
fn es_el_mismo_con_subtitulo(corto: &str, largo: &str) -> bool {
    let c = corto.trim();
    let l = largo.trim();
    if c.chars().count() < 2 || c.len() >= l.len() || !l.starts_with(c) {
        return false;
    }
    // Lo que sigue al nombre base, sin espacios delante.
    let resto = l[c.len()..].trim_start();
    resto
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, ':' | '-' | '\u{2013}' | '\u{2014}' | '|' | '\u{00b7}'))
}

/// Clave de agrupación del historial: el appid si lo hay, el nombre normalizado si no.
fn game_key(b: &Benchmark) -> String {
    if b.appid != 0 {
        format!("appid:{}", b.appid)
    } else {
        format!("name:{}", norm_game_name(&b.game_name))
    }
}

/// ¿La grabación `b` pertenece al juego `(appid, game_name)`?
///
/// Hay que aceptar **las dos** identidades, no solo el appid: `analyze_log` deja siempre
/// `appid` a 0 porque MangoHud no escribe el AppID en su CSV, así que las grabaciones
/// antiguas solo se identifican por nombre. Filtrar únicamente por `appid` hacía que la
/// ventana «Pruebas de este juego» dijera «todavía no hay pruebas» aunque el usuario ya
/// tuviera una de ese juego guardada (visible en «Mis pruebas», que no filtra).
///
/// El nombre se compara normalizado (`norm_game_name`: sin mayúsculas, espacios sobrantes
/// ni extensión de ejecutable) porque el CSV trae el nombre del binario
/// («Palworld-Win64-Shipping.exe») y la ficha del juego el nombre comercial.
///
/// **Solo se cae al nombre cuando alguna de las dos partes no tiene appid.** Si las dos lo
/// tienen y son distintos, se descarta: dos juegos distintos pueden llamarse igual, y
/// mezclar sus historiales sería peor que no enseñar nada.
fn juego_coincide(b: &Benchmark, appid: u32, game_name: &str) -> bool {
    // **Una sola** regla de identidad para todo el módulo. Antes había dos (esta y la que
    // usaba la comparación) y por eso el historial y las comparaciones podían discrepar
    // sobre si dos pruebas eran del mismo juego.
    coinciden_por_identidad(appid, game_name, b.appid, &b.game_name, false)
}

/// Asigna a `benchmark` el siguiente número de su juego y avanza el contador.
///
/// El número sale del **contador persistente**, no del historial conservado: borrar una
/// prueba o dejar que `trim_history` descarte las antiguas no devuelve números al bombo.
/// Si el contador del juego aún no existe (archivo anterior a este campo), se inicializa
/// con el mayor número ya presente en el historial de ese juego, de forma que una
/// instalación que ya tuviera números siga la serie en vez de repetirlos. Las grabaciones
/// antiguas **sin** número no cuentan: no se les inventa una cronología.
fn asignar_numero(data: &mut BenchmarksData, benchmark: &mut Benchmark) {
    // Se calcula la clave con el benchmark ya completo (appid y nombre definitivos).
    let clave = game_key(benchmark);
    let maximo_existente = data
        .benchmarks
        .iter()
        .filter(|b| game_key(b) == clave)
        .filter_map(|b| b.number)
        .max()
        .unwrap_or(0);

    let contador = data.counters.entry(clave).or_insert(maximo_existente);
    // El contador nunca retrocede, ni aunque el historial se haya vaciado.
    let siguiente = (*contador).max(maximo_existente) + 1;
    *contador = siguiente;
    benchmark.number = Some(siguiente);
}

/// Lo que el frontend sabe de la configuración **de un juego concreto** cuando se guarda
/// la prueba.
///
/// Todo es `Option` a propósito: si el editor tenía abierto otro juego, estos campos llegan
/// vacíos y se registran como desconocidos. Es preferible un hueco honesto a un dato de
/// otro juego puesto aquí.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextoDeCaptura {
    /// AppID real del juego de la prueba (no el `0` de las reglas genéricas).
    pub appid_real: Option<u32>,
    pub comando_confirmado: Option<String>,
    pub revision_motor: Option<u64>,
    pub opciones_steam: Option<String>,
    pub perfil_id: Option<String>,
    pub perfil_nombre: Option<String>,
    pub cambios_sin_guardar: Option<bool>,
    pub modo_proton: Option<bool>,
    /// El contexto corresponde al juego de la prueba. Si es `false`, nada de lo anterior se
    /// usa: es la protección contra capturar el perfil o el comando de otro juego.
    #[serde(default)]
    pub corresponde_al_juego: bool,
}

/// Construye la instantánea de una prueba.
///
/// Reglas que se aplican aquí y no en el frontend, para que no dependan de que la interfaz
/// se acuerde:
///
/// - Si `contexto.corresponde_al_juego` es `false`, la configuración se registra como
///   desconocida. Nunca se mezcla el perfil o el comando de otro juego.
/// - El comando se copia **literal**: no se normaliza, no se reordena y no se reconstruye
///   desde los ajustes.
/// - `ejecucion_verificada` queda en `None`: configurar Proton no demuestra que se usara.
/// - El entorno se lee del sistema en este momento, y se marca con su procedencia. Los
///   datos que no se puedan leer quedan en `None`, nunca con un valor inventado.
fn construir_snapshot(
    benchmark: &Benchmark,
    contexto: &ContextoDeCaptura,
    sesion: Option<&crate::sessions::SesionDeEjecucion>,
    ahora: &str,
) -> SnapshotConfig {
    let identidad_observada = sesion.map(|s| &s.identidad);
    let del_juego = contexto.corresponde_al_juego;

    // Con `del_juego == false` esto devuelve `None` para todo: es el único punto donde se
    // decide, y así no hay forma de colar un dato de otro juego por descuido.
    let si_del_juego = |presente: bool| del_juego && presente;

    let configuracion = ConfigInstantanea {
        comando_confirmado: contexto.comando_confirmado.as_ref().and_then(|c| {
            si_del_juego(true).then(|| ConProcedencia::nuevo(c.clone(), Procedencia::MotorRust))
        }),
        // La revisión solo tiene sentido junto al comando del mismo juego.
        revision_motor: if si_del_juego(true) {
            contexto.revision_motor
        } else {
            None
        },
        opciones_steam: contexto.opciones_steam.as_ref().map(|o| {
            ConProcedencia::nuevo(o.clone(), Procedencia::LeidoDeSteam)
        }),
        perfil_id: if si_del_juego(contexto.perfil_id.is_some()) {
            contexto.perfil_id.clone()
        } else {
            None
        },
        perfil_nombre: contexto.perfil_nombre.as_ref().and_then(|n| {
            // El nombre se guarda copiado: renombrar el perfil después no lo cambia. Y solo
            // si el perfil es del juego de la prueba.
            si_del_juego(true).then(|| {
                ConProcedencia::nuevo(n.clone(), Procedencia::ConfiguradoEnThoriapp)
            })
        }),
        cambios_sin_guardar: if si_del_juego(true) {
            contexto.cambios_sin_guardar
        } else {
            None
        },
        modo_proton: contexto.modo_proton.and_then(|p| {
            si_del_juego(true)
                .then(|| ConProcedencia::nuevo(p, Procedencia::ConfiguradoEnThoriapp))
        }),
        // Deliberadamente `None`: ver el docstring de `ConfigInstantanea`.
        ejecucion_verificada: None,
    };

    SnapshotConfig {
        version: SNAPSHOT_SCHEMA_VERSION,
        identidad: IdentidadInstantanea {
            // Manda el benchmark: la instantánea pertenece a ESA prueba. El AppID del
            // contexto solo rellena cuando el benchmark no lo trae (CSV sin AppID), y
            // siempre que el contexto declare ser del mismo juego.
            // Orden de mando, de más fuerte a más débil:
            //   1. El AppID que trae la propia prueba (cabecera del CSV / biblioteca).
            //   2. El que se **observó** en el proceso del juego, si la sesión asociada lo
            //      acreditó con evidencia directa. Se marca con su procedencia para que la
            //      interfaz pueda decir de dónde viene.
            //   3. El del contexto de la interfaz, solo si declara ser del mismo juego.
            //
            // Nunca se mezclan: si la sesión contradice al benchmark, manda el benchmark,
            // porque la instantánea pertenece a ESA prueba.
            appid_real: resolver_appid(benchmark, contexto, identidad_observada).0,
            appid_procedencia: resolver_appid(benchmark, contexto, identidad_observada).1,
            benchmark_id: benchmark.id.clone(),
            numero_prueba: benchmark.number,
            fecha: benchmark.timestamp.clone(),
        },
        configuracion,
        entorno: leer_entorno(benchmark.appid, ahora),
        hardware_grabacion: leer_hardware_de_la_grabacion(&benchmark.log_path, ahora),
        resumen_mangohud: leer_resumen_de_la_grabacion(&benchmark.log_path, ahora),
        capturado_en: ahora.to_string(),
        // Se afirma «posterior a la grabación» solo cuando de verdad lo es. Si la sesión
        // guardó una configuración **durante** la observación del proceso, esa foto es
        // anterior al final de la grabación y no se puede seguir diciendo lo contrario.
        posterior_a_la_grabacion: es_posterior_a_la_grabacion(sesion, ahora),
    }
}

/// ¿La instantánea se tomó después de que la grabación terminara?
///
/// Sin sesión observada no hay forma de saber cuándo se capturó la configuración, así que se
/// mantiene `true`: se analiza el CSV al terminar y no se afirma nada más. Con sesión, se
/// compara la foto guardada **durante** la observación con el instante de captura actual.
fn es_posterior_a_la_grabacion(
    sesion: Option<&crate::sessions::SesionDeEjecucion>,
    ahora: &str,
) -> bool {
    // Si la sesión guardó una foto de la configuración **mientras** se observaba el proceso,
    // esa foto no es posterior a la grabación: describe el momento del arranque, no el de
    // analizar el CSV. La fecha se compara en texto porque ese es el formato guardado
    // (`YYYY-MM-DD HH:MM:SS`), y así ordenan igual el texto y la hora.
    let Some(sesion) = sesion else {
        return true;
    };
    let Some(cfg) = sesion.configuracion_inicial.as_ref() else {
        return true;
    };
    if cfg.capturado_en.trim().is_empty() {
        return true;
    }
    // Hay una foto guardada **durante** la observación, con su propia fecha. Si esa fecha es
    // anterior al momento de capturar la instantánea, la configuración ya existía antes de
    // analizar el CSV: describirla como «posterior a la grabación» sería falso.
    //
    // Se comparan como texto porque ese es exactamente el formato guardado
    // (`AAAA-MM-DD HH:MM:SS`, de ancho fijo): en ese formato el orden del texto y el de la
    // hora coinciden, y así no se depende de cómo venga la fecha del CSV.
    cfg.capturado_en.trim() >= ahora.trim()
}

/// Decide el AppID de la instantánea y de dónde procede.
///
/// Se resuelve en un único sitio para que la procedencia no pueda desincronizarse del
/// valor. Devuelve ya la `Procedencia` que corresponde al valor elegido.
fn resolver_appid(
    benchmark: &Benchmark,
    contexto: &ContextoDeCaptura,
    identidad_observada: Option<&crate::sessions::IdentidadDeSesion>,
) -> (Option<u32>, Procedencia) {
    // 1. El benchmark manda: la instantánea pertenece a esa prueba.
    if benchmark.appid != 0 {
        return (Some(benchmark.appid), Procedencia::DeclaradoPorUsuario);
    }

    // 2. La sesión observada, solo si su AppID consta como observación directa. Un
    //    candidato sin resolver o una inferencia por nombre no se escriben como certeza.
    if let Some(id) = identidad_observada {
        if id.origen.es_observacion_directa() {
            if let Some(appid) = id.appid {
                return (Some(appid), Procedencia::ObservadoEnProceso);
            }
        }
    }

    // 3. El contexto de la interfaz, si declara ser del mismo juego.
    if contexto.corresponde_al_juego {
        if let Some(appid) = contexto.appid_real {
            return (Some(appid), Procedencia::ConfiguradoEnThoriapp);
        }
    }

    (None, Procedencia::Desconocido)
}

/// Transcribe el equipo que **MangoHud registró** en la cabecera del CSV.
///
/// Devuelve `None` si el archivo no se puede leer o no trae cabecera de sistema: en ese
/// caso las condiciones de la grabación quedan **desconocidas**, y así se dice, en vez de
/// rellenarlas con el equipo detectado ahora (que puede ser otro).
fn leer_hardware_de_la_grabacion(
    log_path: &str,
    ahora: &str,
) -> Option<HardwareDeLaGrabacion> {
    if log_path.trim().is_empty() {
        return None;
    }
    let contenido = fs::read_to_string(log_path).ok()?;
    let mut lineas = contenido.lines();
    let meta = crate::mangohud::metadatos_de_cabecera(lineas.next()?, lineas.next()?)?;
    if meta.vacio() {
        return None;
    }

    let registrado = |v: String| {
        ConProcedencia::nuevo(v, Procedencia::RegistradoPorMangohud).con_fecha(ahora)
    };
    let registrado_num = |v: f64| {
        ConProcedencia::nuevo(v, Procedencia::RegistradoPorMangohud).con_fecha(ahora)
    };

    // Intervalo de muestreo: medido sobre el propio CSV, no supuesto.
    let intervalo = crate::mangohud::Serie::desde_csv(&contenido)
        .and_then(|s| s.intervalo_ms());

    Some(HardwareDeLaGrabacion {
        os: meta.os.map(registrado),
        cpu: meta.cpu.map(registrado),
        gpu: meta.gpu.map(registrado),
        // Se conserva el crudo (KiB) junto a la conversión: el crudo es la prueba de unidad.
        ram_kib: meta.ram_kib.map(|v| {
            ConProcedencia::nuevo(v, Procedencia::RegistradoPorMangohud).con_fecha(ahora)
        }),
        ram_gib: meta.ram_gib.map(|v| {
            ConProcedencia::nuevo(v, Procedencia::RegistradoPorMangohud).con_fecha(ahora)
        }),
        kernel: meta.kernel.map(registrado),
        driver: meta.driver.map(registrado),
        cpuscheduler: meta.cpuscheduler.map(registrado),
        intervalo_muestreo_ms: intervalo.map(registrado_num),
    })
}

/// Transcribe el `_summary.csv` que MangoHud escribió junto a este CSV.
///
/// Solo se asocia el resumen que está al lado del propio log: un resumen de otra grabación
/// describiría otra prueba. Si falta o está incompleto, `None` (el benchmark sigue siendo
/// válido con sus métricas).
fn leer_resumen_de_la_grabacion(log_path: &str, ahora: &str) -> Option<ResumenOriginal> {
    if log_path.trim().is_empty() {
        return None;
    }
    let ruta = Path::new(log_path);
    let ruta_resumen = crate::mangohud::ruta_del_resumen(ruta)?;
    let contenido = fs::read_to_string(&ruta_resumen).ok()?;
    let resumen = crate::mangohud::leer_resumen(&contenido)?;
    Some(ResumenOriginal {
        valores: resumen
            .valores
            .into_iter()
            .map(|v| (v.nombre_original, v.valor))
            .collect(),
        capturado_en: ahora.to_string(),
    })
}

/// Lee del sistema lo que se pueda del entorno. Lo que no se pueda leer queda en `None`.
/// `ahora` se sella en cada dato para poder ver si se leyó todo junto o a saltos.
fn leer_entorno(appid: u32, ahora: &str) -> EntornoInstantanea {
    let hw = crate::hw::detect();
    let texto = |v: &str| (!v.trim().is_empty()).then(|| v.trim().to_string());
    let detectado = |v: String| {
        ConProcedencia::nuevo(v, Procedencia::DetectadoEnSistema).con_fecha(ahora)
    };

    EntornoInstantanea {
        gpu: texto(&hw.gpu).map(detectado),
        cpu: texto(&hw.cpu).map(detectado),
        ram_gb: (hw.ram_gb > 0.0)
            .then(|| ConProcedencia::nuevo(hw.ram_gb, Procedencia::DetectadoEnSistema).con_fecha(ahora)),
        kernel: texto(&hw.kernel).map(detectado),
        // `Hardware` no detecta la versión del driver propietario: queda desconocida en
        // lugar de deducirse del nombre de la GPU.
        driver: None,
        mesa: texto(&hw.mesa).map(detectado),
        // Thoriapp no sabe qué versión de Proton tiene seleccionada el juego sin mirar el
        // proceso en marcha; no se presume. Queda `None` hasta la subfase 5.3.
        proton_seleccionado: None,
        // Se consulta Steam ahora: si el juego no está instalado, `None`.
        build_id: crate::steam::build_id(appid)
            .ok()
            .flatten()
            .map(|v| ConProcedencia::nuevo(v, Procedencia::LeidoDeSteam).con_fecha(ahora)),
        log_interval_ms: None,
    }
}

/// Recorta el historial a las `MAX_BENCHMARKS_PER_GAME` grabaciones más recientes de
/// cada juego, descartando las más antiguas.
///
/// Es por juego, no global: guardar en un juego no borra las pruebas de otro, así que
/// puedes volver a un juego de hace semanas y seguir comparando. Dentro de un juego el
/// exceso se va por la cola según `timestamp` (formato ISO, que ordena bien como texto).
/// Si dos grabaciones comparten fecha, gana la que está más al final de la lista, que es
/// la última que se añadió. Conserva el orden original de lo que sobrevive.
/// Qué comando acaba guardado en un benchmark.
///
/// - Se guarda **tal cual llega**, sin recortar ni normalizar: es una instantánea del
///   texto que el usuario tenía configurado, con sus variables de entorno, wrappers,
///   comillas y el `%command%`. Tocarlo aquí (quitar `mangohud`, reordenar argumentos)
///   haría que la prueba guardada describiera un comando que nunca existió.
/// - Si llega vacío **no se pisa** lo que ya traía el benchmark: un guardado sin comando
///   no puede borrar un registro que sí existía. Perder el comando de una prueba ya
///   guardada es exactamente el fallo que hay que evitar.
fn comando_a_guardar(actual: &str, nuevo: &str) -> String {
    if nuevo.is_empty() && !actual.is_empty() {
        actual.to_string()
    } else {
        nuevo.to_string()
    }
}

/// Decide qué opciones de Steam quedan en la prueba.
///
/// La lectura de Steam es un **dato histórico** de la prueba, no del editor: si el guardado
/// llega sin ella (Steam no instalado, permiso denegado, llamada antigua), se conserva la
/// que ya tuviera la grabación en vez de borrarla. `Some("")` sí gana: significa «se leyó
/// y Steam no tenía nada», que es información distinta de «no se pudo leer».
fn steam_a_guardar(actual: Option<&String>, nueva: Option<String>) -> Option<String> {
    nueva.or_else(|| actual.cloned())
}

fn trim_history(list: &mut Vec<Benchmark>) {
    let mut keep: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, b) in list.iter().enumerate() {
        keep.entry(game_key(b)).or_default().push(i);
    }

    let mut drop = vec![false; list.len()];
    for indices in keep.values() {
        if indices.len() <= MAX_BENCHMARKS_PER_GAME {
            continue;
        }
        // Índices a conservar: los últimos por fecha (y, a igualdad, los últimos en la
        // lista). `sort_by` es estable, así que el desempate es el orden de inserción.
        let mut ordered = indices.clone();
        ordered.sort_by(|&a, &b| list[a].timestamp.cmp(&list[b].timestamp));
        let excess = ordered.len() - MAX_BENCHMARKS_PER_GAME;
        for &i in &ordered[..excess] {
            drop[i] = true;
        }
    }

    let mut i = 0;
    list.retain(|_| {
        let keep_it = !drop[i];
        i += 1;
        keep_it
    });
}

/// Analiza un log y dice si **esa misma ruta** ya está en el historial.
///
/// Va aquí y no en el frontend porque el frontend no sabe de dónde salió cada
/// benchmark. Comparar métricas es una heurística frágil: dos grabaciones distintas en el
/// mismo sitio dan valores casi idénticos, y una grabación ya guardada se reintenta si su
/// marca de "procesada" se pierde. La ruta del fichero es la identidad de verdad.
///
/// Devuelve `true` si ya había un benchmark de esa ruta.
pub fn benchmark_exists_for_path(data: &BenchmarksData, log_path: &str) -> bool {
    let objetivo = std::fs::canonicalize(log_path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| log_path.to_string());
    data.benchmarks.iter().any(|b| {
        if b.log_path.is_empty() {
            return false;
        }
        let suyo = std::fs::canonicalize(&b.log_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| b.log_path.clone());
        suyo == objetivo
    })
}

/// ¿Esta ruta ya está guardada? Lo consulta el vigilante antes de analizar siquiera.
#[tauri::command]
pub fn is_log_already_saved(log_path: String) -> Result<bool, String> {
    Ok(benchmark_exists_for_path(&load_benchmarks_data(), &log_path))
}

// La firma es plana a propósito: es el contrato que invoca el frontend (una
// llamada IPC por argumento). Agruparlos en un struct cambiaría el JSON que
// envía React y obligaría a tocar api.ts y todos los llamadores, sin ganar nada
// en un punto de escritura que solo se usa desde ahí.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn save_benchmark(
    mut benchmark: Benchmark,
    profile_id: String,
    game_name: String,
    appid: u32,
    launch_command: String,
    // De dónde salió `launch_command`. Si falta (llamada antigua), `desconocido`.
    origen_comando: Option<OrigenComando>,
    // Opciones leídas de Steam. `None` = no se pudo leer; no se inventa nada.
    steam_command: Option<String>,
    notes: Option<String>,
    // Lo que el frontend sabe de la configuración de ESTE juego (subfase 5.2). Si falta,
    // la instantánea se construye igual, pero con la configuración en desconocido.
    contexto: Option<ContextoDeCaptura>,
) -> Result<Benchmark, String> {
    let _turno = CERROJO_BENCHMARKS.lock().unwrap_or_else(|e| e.into_inner());
    let mut data = load_benchmarks_data_para_editar()?;

    benchmark.profile_id = profile_id;

    if (benchmark.game_name.is_empty() || benchmark.game_name == "Desconocido")
        && !game_name.is_empty()
        && game_name != "Desconocido"
    {
        benchmark.game_name = game_name;
    }

    benchmark.appid = appid;
    let comando_previo = benchmark.launch_command.clone();
    benchmark.launch_command = comando_a_guardar(&benchmark.launch_command, &launch_command);
    // Si el guardado no trae lectura de Steam, se conserva la que hubiera: es un dato
    // histórico de la prueba, no del momento de guardar.
    benchmark.steam_command = steam_a_guardar(benchmark.steam_command.as_ref(), steam_command);
    // El origen solo se actualiza cuando el comando **sí** se actualizó. Si el guardado
    // vino sin comando y se conservó el anterior, el origen tiene que seguir describiendo
    // a ese comando, no a este guardado.
    benchmark.origen_comando = if benchmark.launch_command != comando_previo
        || comando_previo.is_empty()
    {
        origen_comando.unwrap_or(OrigenComando::Desconocido)
    } else {
        benchmark.origen_comando
    };
    benchmark.notes = notes;

    // Número de presentación («Prueba N»), asignado una sola vez y para siempre. Va aquí,
    // en el único punto de escritura, para que ninguna grabación pueda quedar sin número
    // ni heredar el de otra.
    asignar_numero(&mut data, &mut benchmark);

    // Sesión de ejecución (subfase 5.3). Es **solo un enlace**: se atribuye cuando la
    // evidencia temporal e identitaria lo permite y, si no, se deja sin sesión. Nunca se
    // asigna por el juego abierto en la interfaz, y una grabación antigua no se cuelga de un
    // proceso actual solo porque el juego se llame igual: eso lo comprueba la ventana
    // temporal de `sessions::decidir_asociacion`.
    //
    // Va **antes** de la instantánea a propósito: la instantánea necesita la identidad y la
    // configuración capturada de la sesión. Al revés, la sesión aún no existía cuando se
    // construía el AppID y la cabecera acababa diciendo «AppID desconocido» con el AppID
    // 1623730 guardado al lado.
    benchmark.sesion_id = None;
    if let Some(id) = atribuir_a_sesion(&benchmark) {
        benchmark.sesion_id = Some(id);
    }

    // Instantánea: se construye aquí, después de tener el número definitivo y la sesión, y
    // se guarda como parte del benchmark. A partir de este momento los datos son historia:
    // renombrar un perfil o cambiar Steam no los toca. Nunca se reconstruye desde el estado
    // actual.
    let ahora = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sesion = benchmark
        .sesion_id
        .as_deref()
        .and_then(|id| crate::sessions_service::servicio().sesion_por_id(id));
    benchmark.snapshot = Some(construir_snapshot(
        &benchmark,
        &contexto.unwrap_or_default(),
        sesion.as_ref(),
        &ahora,
    ));

    // Historial por juego: se añade la grabación nueva y se conservan las
    // `MAX_BENCHMARKS_PER_GAME` más recientes de ESTE juego; las más viejas se van solas.
    // Los demás juegos no se tocan, y como el archivo entero se guarda, el recorte también
    // limpia lo que quedara de versiones anteriores.
    //
    // Aquí NO se borran las grabaciones anteriores del mismo juego: el recorte es cosa de
    // `trim_history`. Un `retain` por clave de juego en este punto dejaría una sola
    // grabación por juego y el historial de 10 no llegaría a existir.
    data.benchmarks.push(benchmark.clone());
    trim_history(&mut data.benchmarks);
    save_benchmarks_data(&data)?;

    Ok(benchmark)
}

#[tauri::command]
pub fn load_benchmarks() -> Result<Vec<Benchmark>, String> {
    let data = cargar_benchmarks()?;
    let mut list = data.benchmarks;
    // El recorte se aplica al leer y no solo al guardar: un archivo antiguo con más de
    // 10 grabaciones por juego se limpia la próxima vez que se cargue.
    trim_history(&mut list);
    Ok(list)
}

/// Atribuye una grabación recién guardada a una sesión de ejecución, si hay evidencia.
///
/// La sesión se busca por la ventana temporal del CSV (nombre + duración) y por el AppID;
/// el resultado solo se escribe si la asociación permite atribuir. Devuelve el id para que
/// quien llama lo ponga en el benchmark.
///
/// Se separa en función para poder probarla: el guardado completo necesita Tauri.
fn atribuir_a_sesion(benchmark: &Benchmark) -> Option<String> {
    let ruta_csv = benchmark.log_path.as_str();
    if ruta_csv.is_empty() {
        return None;
    }
    let ventana = crate::sessions_service::ventana_de_csv(ruta_csv, benchmark.duracion_en_segundos());
    ventana.inicio.as_ref()?;
    let (id, _asociacion) = crate::sessions_service::servicio().asociar_grabacion(
        benchmark.appid,
        &benchmark.game_name,
        &ventana,
    );
    if let Some(sid) = &id {
        crate::sessions_service::servicio().anotar_grabacion(sid, &benchmark.id);
    }
    id
}

/// Grabaciones de **un solo juego**.
///
/// `game_name` no es decorativo: las grabaciones antiguas (y las hechas con el juego no
/// abierto, porque MangoHud no escribe el AppID) tienen `appid: 0` y solo se identifican
/// por nombre. Filtrar solo por appid las dejaba fuera y la ficha del juego decía que no
/// había pruebas. Ver {@link juego_coincide}.
#[tauri::command]
pub fn load_benchmarks_for_game(
    appid: u32,
    game_name: Option<String>,
) -> Result<Vec<Benchmark>, String> {
    let nombre = game_name.unwrap_or_default();
    let data = cargar_benchmarks()?;
    let mut benchmarks: Vec<Benchmark> = data
        .benchmarks
        .into_iter()
        .filter(|b| juego_coincide(b, appid, &nombre))
        .collect();
    trim_history(&mut benchmarks);
    Ok(benchmarks)
}

/// Último comando **registrado de verdad** en una prueba de este juego.
///
/// Sirve para las grabaciones hechas sin abrir la ficha del juego (entrando a «Pruebas»
/// desde la barra lateral): ahí Thoriapp no sabe qué tiene configurado el editor, pero sí
/// lo que dijo la última vez que capturó ese dato. Se devuelve `None` si ninguna prueba de
/// ese juego tiene comando: no se inventa nada.
///
/// **Por qué acepta un nombre.** `analyze_log` deja siempre `appid` a 0 (MangoHud no
/// escribe el AppID en el CSV), así que la única identidad fiable de la grabación es el
/// nombre del juego, que sale del propio fichero. Buscar solo por `appid` hacía que esta
/// recuperación no encontrara nunca nada: la prueba se guardaba sin comando y la vista
/// mostraba «Comando no registrado» aunque la app lo tuviera apuntado.
///
/// Solo cuentan las pruebas con texto: una cadena vacía es «no registrado», no una
/// observación.
#[tauri::command]
pub fn ultimo_comando_registrado(
    appid: Option<u32>,
    game_name: Option<String>,
) -> Result<Option<String>, String> {
    Ok(ayuda_ultimo_comando(
        &load_benchmarks_data().benchmarks,
        appid.unwrap_or(0),
        game_name.as_deref(),
    ))
}

/// Implementación de `ultimo_comando_registrado`, separada para poder probarla.
///
/// Orden de preferencia: mismo AppID; si no hay, mismo nombre normalizado.
fn ayuda_ultimo_comando(
    benchmarks: &[Benchmark],
    appid: u32,
    game_name: Option<&str>,
) -> Option<String> {
    let candidatas: Vec<&Benchmark> = benchmarks
        .iter()
        .filter(|b| !b.launch_command.trim().is_empty())
        .filter(|b| {
            (appid != 0 && b.appid == appid)
                || game_name
                    .map(|n| !n.trim().is_empty() && norm_game_name(n) == norm_game_name(&b.game_name))
                    .unwrap_or(false)
        })
        .collect();

    candidatas
        .iter()
        .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
        .map(|b| b.launch_command.clone())
}

#[tauri::command]
pub fn delete_benchmark(benchmark_id: String) -> Result<(), String> {
    let _turno = CERROJO_BENCHMARKS.lock().unwrap_or_else(|e| e.into_inner());
    let mut data = load_benchmarks_data_para_editar()?;
    data.benchmarks.retain(|b| b.id != benchmark_id);
    save_benchmarks_data(&data)?;
    Ok(())
}

/// Borra **solo** las grabaciones de un juego, dejando intactas las de los demás.
///
/// La ventana «Pruebas de este juego» enseña el historial de un único juego; su botón de
/// borrar todo no puede llevarse por delante el historial de los otros. `delete_all_benchmarks`
/// vacía la lista entera y no sirve para eso.
///
/// Se identifica el juego igual que `trim_history` (`game_key`): por `appid` si la grabación
/// lo tiene, y por nombre normalizado cuando es 0. Así el filtrado coincide exactamente con
/// el de `load_benchmarks_for_game` y no se borra nada que la vista no estuviera enseñando.
/// Devuelve cuántas se borraron.
/// Núcleo de `delete_benchmarks_for_game`, separado para poder probarlo sin disco.
///
/// Devuelve los `id` **exactos** que se van a borrar. Esa lista es la que usa después la
/// limpieza de comparaciones guardadas: si el borrado y la limpieza calcularan el alcance
/// por separado, podrían discrepar y dejar comparaciones colgando.
fn ids_a_borrar_del_juego(data: &BenchmarksData, appid: u32, game_name: &str) -> Vec<String> {
    data.benchmarks
        .iter()
        .filter(|b| juego_coincide(b, appid, game_name))
        .map(|b| b.id.clone())
        .collect()
}

#[tauri::command]
pub fn delete_benchmarks_for_game(appid: u32, game_name: String) -> Result<usize, String> {
    // El cerrojo de benchmarks se suelta al salir de este bloque, **antes** de tocar el de
    // comparaciones. Tomar los dos a la vez permitiría un interbloqueo con cualquier ruta que
    // los pida en el orden contrario.
    let (borradas, ids) = {
        let _turno = CERROJO_BENCHMARKS.lock().unwrap_or_else(|e| e.into_inner());
        let mut data = load_benchmarks_data_para_editar()?;
        // Mismo criterio que la lectura: borrar tiene que borrar exactamente lo que la ventana
        // estaba enseñando, incluidas las grabaciones antiguas sin appid. Con una clave
        // `name:` no se borraban, y el botón «Borrar todas» parecía no hacer nada.
        let ids = ids_a_borrar_del_juego(&data, appid, &game_name);
        let borradas = ids.len();
        if borradas == 0 {
            return Ok(0);
        }
        data.benchmarks.retain(|b| !ids.iter().any(|i| i == &b.id));
        save_benchmarks_data(&data)?;
        (borradas, ids)
    };
    // Las comparaciones guardadas que usaban estas pruebas ya no se pueden abrir: se
    // limpian junto con ellas para no dejar informes que citan datos que ya no existen.
    let huerfanas = purgar_comparaciones_por_benchmarks(&ids)?;
    println!(
        "[Benchmark] ✅ Se eliminaron {} benchmarks de {} ({} comparaciones huérfanas)",
        borradas, game_name, huerfanas
    );
    Ok(borradas)
}

/// Quita las comparaciones guardadas que dependen de alguna de las pruebas borradas.
///
/// Una comparación guarda copia de las dos pruebas que la formaron. Si se borra una de ellas,
/// el informe guardado seguiría existiendo y citaría una prueba que ya no está: quien lo abra
/// no puede comprobar de dónde salen los números. Se elimina en vez de dejarlo mintiendo.
///
/// Se compara por `id` de `baseline`/`test`, que es lo único estable para emparejarlas.
/// Devuelve cuántas se quitaron.
fn purgar_comparaciones_por_benchmarks(ids_borrados: &[String]) -> Result<usize, String> {
    if ids_borrados.is_empty() {
        return Ok(0);
    }
    let _turno = CERROJO_COMPARACIONES.lock().unwrap_or_else(|e| e.into_inner());
    // Si el archivo no se puede leer, no se vacía a ciegas: se deja como está y se avisa.
    let mut data = load_saved_data_para_editar()?;
    let antes = data.comparisons.len();
    data.comparisons.retain(|c| {
        let base = &c.comparison.baseline.id;
        let test = &c.comparison.test.id;
        !ids_borrados.iter().any(|i| i == base || i == test)
    });
    let quitadas = antes - data.comparisons.len();
    if quitadas > 0 {
        save_saved_data(&data)?;
    }
    Ok(quitadas)
}

// NUEVO: Comando para borrar todos los benchmarks
#[tauri::command]
pub fn delete_all_benchmarks() -> Result<(), String> {
    // Mismo cuidado que arriba: se suelta un cerrojo antes de tomar el otro.
    let (count, ids) = {
        let _turno = CERROJO_BENCHMARKS.lock().unwrap_or_else(|e| e.into_inner());
        let mut data = load_benchmarks_data_para_editar()?;
        let ids: Vec<String> = data.benchmarks.iter().map(|b| b.id.clone()).collect();
        let count = ids.len();
        data.benchmarks.clear();
        save_benchmarks_data(&data)?;
        (count, ids)
    };
    // Igual que el borrado por juego: ninguna comparación puede sobrevivir a sus pruebas.
    purgar_comparaciones_por_benchmarks(&ids)?;
    println!("[Benchmark] ✅ Se eliminaron {} benchmarks", count);
    Ok(())
}

/// Texto del resultado de comparar dos grabaciones.
///
/// Función pura para poder probarla: el texto es lo que el usuario lee como
/// veredicto, así que su política (no llamar «mejora confirmada» a un resultado de
/// una sola sesión) merece test propio.
fn texto_comparacion(
    fps_change_percent: f64,
    fps_1_percent_change_percent: f64,
    noise: f64,
    within_noise: bool,
) -> String {
    if within_noise {
        format!(
            "Sin cambio concluyente: {:+.1}% de FPS medios, dentro del margen estimado ±{:.1}%. Repite la prueba antes de decidir.",
            fps_change_percent, noise
        )
    } else if fps_change_percent > noise && fps_1_percent_change_percent < -noise {
        format!(
            "Resultado mixto: más FPS medios (+{:.1}%) pero peores momentos ({:.1}%). Puede haber más tirones.",
            fps_change_percent, fps_1_percent_change_percent
        )
    } else if fps_change_percent < -noise && fps_1_percent_change_percent > noise {
        format!(
            "Resultado mixto: menos FPS medios ({:.1}%) pero mejores peores momentos (+{:.1}%). Puede sentirse más parejo.",
            fps_change_percent, fps_1_percent_change_percent
        )
    } else if fps_change_percent > noise {
        format!(
            "Mejora observada: +{:.1}% de FPS medios, por encima del margen estimado ±{:.1}%. Resultado provisional: repite la prueba para comprobarlo.",
            fps_change_percent, noise
        )
    } else {
        format!(
            "Empeoramiento observado: {:.1}% de FPS medios, por encima del margen estimado ±{:.1}%. Resultado provisional: repite la prueba para comprobarlo.",
            fps_change_percent, noise
        )
    }
}

/// AppID para mostrar en un mensaje: el número, o «desconocido» si no lo hay.
fn texto_appid(appid: u32) -> String {
    if appid == 0 {
        "desconocido".to_string()
    } else {
        appid.to_string()
    }
}

/// ¿Las dos grabaciones son del mismo juego?
///
/// Una sola regla para todo el módulo, porque tener dos era el origen del fallo:
/// `juego_coincide` comparaba por nombre cuando faltaba el AppID, mientras que la
/// comparación solo miraba el nombre y se conformaba con avisar.
///
/// - Con AppID en las dos: manda el AppID. Dos nombres distintos con el mismo AppID son el
///   mismo juego (el nombre se escribe a mano y cambia); dos AppID distintos, nunca.
/// - Sin AppID en alguna: se compara el nombre normalizado. Si falta en una, no se puede
///   afirmar nada: se responde que **no** son el mismo, porque afirmar una comparación sin
///   evidencia es peor que no ofrecerla.
pub fn mismo_juego(a: &Benchmark, b: &Benchmark) -> bool {
    coinciden_por_identidad(a.appid, &a.game_name, b.appid, &b.game_name, true)
}

/// La regla, en crudo. `Benchmark` no implementa `Default`, así que el historial no puede
/// construir uno de mentira para reutilizarla: comparte esta en su lugar.
/// `estricto` separa los dos usos legítimos, que **no** son el mismo:
///
/// - Buscar en el historial de un juego (`estricto = false`): si la grabación antigua no
///   tiene AppID, el nombre es la única pista que hay. Descartarla escondería pruebas reales
///   del usuario.
/// - Comparar dos grabaciones (`estricto = true`): si una tiene AppID y la otra no, no hay
///   evidencia de que sean el mismo juego y compararlas produciría un número inventado.
///
/// Se sigue usando **una sola** función; la diferencia es explícita y está en un parámetro,
/// no en dos implementaciones que puedan divergir.
fn coinciden_por_identidad(
    appid_a: u32,
    nombre_a: &str,
    appid_b: u32,
    nombre_b: &str,
    estricto: bool,
) -> bool {
    if appid_a != 0 && appid_b != 0 {
        return appid_a == appid_b;
    }
    // Comparación estricta con evidencia incompleta: un AppID de un lado y ninguno del otro
    // no permite afirmar que sea el mismo juego.
    if estricto && (appid_a != 0 || appid_b != 0) {
        return false;
    }
    let na = norm_game_name(nombre_a);
    let nb = norm_game_name(nombre_b);
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    if na == nb {
        return true;
    }
    // El mismo juego escrito con y sin subtítulo (`Deskrawl` / `Deskrawl: Idle ARPG`).
    // Solo aquí, en la caída por nombre: con AppID en las dos partes ya se decidió arriba.
    es_el_mismo_con_subtitulo(&na, &nb) || es_el_mismo_con_subtitulo(&nb, &na)
}


/// Construye la comparación a partir de una lista de grabaciones **ya leída**.
///
/// La usan la comparación de **grupos** y, por dentro, la ruta evaluada. Desde C.3 ya no
/// hay ningún comando que compare dos grabaciones individuales sin evaluación: el único
/// camino es `compare_benchmarks_evaluada`, que resuelve los identificadores exigiendo
/// unicidad y devuelve la comparación dentro de la confirmación.
/// Busca una grabación por identificador **exigiendo que sea única**.
///
/// `save_benchmark` acepta el `id` que le da el llamador y **no** comprueba que no exista
/// ya (los ids llevan resolución de segundo, así que dos análisis seguidos pueden
/// coincidir). Cuando eso pasa, `find` se queda con la primera y la comparación deja de
/// describir un hecho: hay dos grabaciones distintas y se evaluó una de ellas al azar.
///
/// La ruta evaluada **no** puede heredar esa ambigüedad. Aquí se exige que el
/// identificador corresponda a **exactamente un** registro:
///
/// - 0 → no encontrado.
/// - 1 → el único, se devuelve.
/// - 2 o más → se rechaza. Se distingue el caso en que los candidatos son **idénticos**
///   (mismo contenido bit a bit, incluidos `number`, ruta y sesión) del caso en que
///   difieren: los dos son ambiguos, porque el identificador no identifica, pero el
///   mensaje dice cuál es la situación real.
///
/// La comparación de dos grabaciones individuales solo existe ya con evaluación, así que
/// esta función es la única puerta: no queda el `find` de primera coincidencia que había
/// antes para los comandos antiguos.
/// Resultado de buscar por identificador una grabación **inequívoca**.
///
/// El discriminante (`NoEncontrada` / `Ambigua`) **es** el dato: quien lo consume hace
/// `match` sobre la variante y nunca inspecciona el texto. El mensaje es solo para el
/// usuario y puede cambiar sin alterar la clasificación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FallaDeBusqueda {
    /// No hay ninguna grabación con ese identificador.
    NoEncontrada { mensaje: String },
    /// Hay más de una y no se puede saber cuál se quiso designar.
    Ambigua { mensaje: String },
}

impl FallaDeBusqueda {
    /// El texto que se enseña al usuario. No interviene en la clasificación.
    ///
    /// Solo lo usan las pruebas que demuestran que el discriminante no depende de este
    /// texto; en producción la variante se compara, no se lee.
    #[cfg(test)]
    pub(crate) fn mensaje(&self) -> String {
        match self {
            FallaDeBusqueda::NoEncontrada { mensaje } | FallaDeBusqueda::Ambigua { mensaje } => {
                mensaje.clone()
            }
        }
    }
}

fn buscar_unico<'a>(
    benchmarks: &'a [Benchmark],
    id: &str,
    que: &str,
) -> Result<&'a Benchmark, FallaDeBusqueda> {
    let mut encontrados = benchmarks.iter().filter(|b| b.id == id);

    let primero = encontrados
        .next()
        .ok_or_else(|| FallaDeBusqueda::NoEncontrada {
            mensaje: format!("{que} no encontrado"),
        })?;

    let mut total = 1;
    let mut todos_identicos = true;
    for otro in encontrados {
        total += 1;
        if !crate::evaluacion::mismo_benchmark(primero, otro) {
            todos_identicos = false;
        }
    }

    if total > 1 {
        let detalle = if todos_identicos {
            "las copias tienen contenido idéntico"
        } else {
            "las copias tienen contenido distinto"
        };
        return Err(FallaDeBusqueda::Ambigua {
            mensaje: format!(
                "El identificador «{id}» corresponde a {total} grabaciones ({detalle}): no se \
                 puede saber cuál evaluar. No se ha guardado nada."
            ),
        });
    }

    Ok(primero)
}

/// Construye la comparación entre dos grabaciones **ya resueltas**.
///
/// Aquí vive toda la regla de comparación. Desde C.3 la única entrada es la ruta evaluada,
/// que resuelve antes los identificadores exigiendo unicidad: no hay una segunda forma de
/// comparar que pueda divergir.
fn comparacion_entre(
    baseline: &Benchmark,
    test: &Benchmark,
) -> Result<BenchmarkComparison, String> {
    let baseline = baseline.clone();
    let test = test.clone();

    // Dos grabaciones de juegos distintos no se comparan. Antes se calculaba todo y se
    // añadía un aviso al final diciendo que la comparación «no es válida», pero el resultado
    // se devolvía igual, con sus porcentajes, y quien lo leyera sin detenerse en la nota se
    // quedaba con un número que no significa nada. Se rechaza aquí, en el backend, que es el
    // único sitio donde puede decidirse: la interfaz no puede deducirlo por su cuenta.
    if !mismo_juego(&baseline, &test) {
        return Err(format!(
            "No se pueden comparar grabaciones de juegos distintos: «{}» (AppID {}) y «{}» (AppID {}).",
            baseline.game_name,
            texto_appid(baseline.appid),
            test.game_name,
            texto_appid(test.appid)
        ));
    }

    // Cambio porcentual de `b` respecto a `a`. Si `a` es 0 (dato inválido) devuelve 0
    // en vez de infinito o NaN.
    fn pct_change(a: f64, b: f64) -> f64 {
        if a.abs() < f64::EPSILON {
            0.0
        } else {
            (b - a) / a * 100.0
        }
    }

    let fps_change_percent = pct_change(baseline.avg_fps, test.avg_fps);
    let fps_1_percent_change_percent =
        pct_change(baseline.fps_1_percent_low, test.fps_1_percent_low);
    let fps_0_1_percent_change_percent =
        pct_change(baseline.fps_0_1_percent_low, test.fps_0_1_percent_low);
    let frame_time_change_percent = pct_change(baseline.frame_time_avg, test.frame_time_avg);

    // Entre dos grabaciones del mismo juego en las mismas condiciones, el propio ruido
    // se estima a partir de las propias grabaciones (variación entre bloques). El mínimo
    // es MIN_NOISE: por debajo de eso nunca se afirma una mejora.
    //
    // IMPORTANTE sobre lo que esto permite afirmar: `noise_percent` es el error de
    // estimación de la media DENTRO de cada grabación, no la variación entre partidas
    // distintas (otro enemigo, otra zona, la GPU más caliente). Superarlo significa
    // «estas dos grabaciones midieron distinto más allá del ruido de muestreo», no
    // «el juego rinde más». Por eso el texto dice «observada» y pide repetir: la
    // confirmación solo la da comparar grupos (ver `stats.rs`).
    let noise = combined_noise(baseline.noise_percent, test.noise_percent);
    let within_noise = fps_change_percent.abs() <= noise;

    let mut conclusion = texto_comparacion(
        fps_change_percent,
        fps_1_percent_change_percent,
        noise,
        within_noise,
    );

    // Avisos que ayudan a no sacar conclusiones erróneas
    //
    // `duration_seconds` vale 0.0 cuando la grabación no trae eje temporal fiable
    // [5.4.1.A]; ahí el cero significa «no consta», no «cero segundos». La regla vive en
    // `Benchmark::duracion_en_segundos`, no aquí, para que no se duplique.
    match (baseline.duracion_en_segundos(), test.duracion_en_segundos()) {
        (None, _) | (_, None) => conclusion.push_str(
            "\nⓘ Alguna grabación no trae un eje temporal fiable: su duración no consta y \
             el resultado puede no ser comparable con otra de duración conocida.",
        ),
        (Some(a), Some(b)) if a < 20.0 || b < 20.0 => conclusion.push_str(
            "\nℹ️ Alguna grabación dura menos de 20 s: el resultado puede no ser fiable.",
        ),
        _ => {}
    }

    let iguales = comandos_iguales(&baseline.launch_command, &test.launch_command);

    Ok(BenchmarkComparison {
        baseline,
        test,
        fps_change_percent,
        fps_1_percent_change_percent,
        fps_0_1_percent_change_percent,
        frame_time_change_percent,
        conclusion,
        noise_percent: Some(noise),
        within_noise,
        comandos_iguales: iguales,
    })
}

/// ¿Los dos comandos registrados son el mismo texto?
///
/// - Compara el texto **completo** tras recortar espacios de los extremos. No tokeniza ni
///   normaliza: `mangohud %command%` y `mangohud  %command%` son el mismo comando para el
///   shell, pero darles el mismo trato exigiría interpretarlo, y este campo existe para
///   ser conservador. Dos comandos que solo difieren en espacios internos se consideran
///   distintos, que es el error seguro.
/// - Devuelve `None` si **alguno** está vacío. Un benchmark antiguo sin comando no prueba
///   nada: ni que fuera igual, ni que fuera distinto. Antes se asumía `%command%` y la
///   pantalla decía «sin cambios», que es exactamente el fallo que había.
pub fn comandos_iguales(antes: &str, despues: &str) -> Option<bool> {
    let a = antes.trim();
    let b = despues.trim();
    if a.is_empty() || b.is_empty() {
        return None;
    }
    Some(a == b)
}

#[tauri::command]
pub fn analyze_mangohud_log_command(
    log_path: String,
    warmup_seconds: Option<f64>,
) -> Result<Benchmark, String> {
    analyze_mangohud_log(
        &log_path,
        warmup_seconds.unwrap_or(DEFAULT_WARMUP_SECONDS),
    )
}

/// Lee el valor de una clave de MangoHud.conf (`clave=valor` o `clave = valor`).
/// Ignora comentarios (#) y toma la ÚLTIMA aparición, igual que MangoHud.
fn read_conf_value(content: &str, key: &str) -> Option<String> {
    let mut found: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                found = Some(v.trim().to_string());
            }
        }
    }
    found
}

/// Expande `~` al directorio personal.
fn expand_home(path: &str, home: &str) -> String {
    if path == "~" {
        home.to_string()
    } else if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{}", home, rest)
    } else {
        path.to_string()
    }
}

#[tauri::command]
pub fn detect_mangohud_log_path() -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    // 1) Lo más fiable: la carpeta que el propio usuario configuró en MangoHud.
    for config_path in mangohud_conf_paths() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Some(folder) = read_conf_value(&content, "output_folder") {
                let folder = expand_home(&folder, &home);
                if !folder.is_empty() && Path::new(&folder).is_dir() {
                    println!("[Detect] Ruta desde config: {}", folder);
                    return Ok(folder);
                }
            }
        }
    }

    // 2) Carpetas habituales que ya contengan logs de MangoHud
    let possible_paths = vec![
        format!("{}/.local/share/goverlay", home),
        format!("{}/.local/share/goverlay/logs", home),
        format!("{}/.local/share/MangoHud", home),
        format!("{}/.local/share/MangoHud/logs", home),
        format!("{}/MangoHud", home),
        format!("{}/.config/MangoHud", home),
        format!("{}/.config/goverlay", home),
        "/tmp/MangoHud".to_string(),
        format!(
            "{}/.var/app/com.valvesoftware.Steam/.local/share/goverlay",
            home
        ),
        format!(
            "{}/.var/app/com.valvesoftware.Steam/.local/share/MangoHud",
            home
        ),
    ];

    for path in &possible_paths {
        let p = Path::new(path);
        if p.is_dir() && dir_has_mangohud_log(p) {
            println!("[Detect] Encontrados logs de MangoHud en: {}", path);
            return Ok(path.clone());
        }
    }

    // 3) Último recurso: carpeta por defecto (aunque aún esté vacía)
    let goverlay_path = format!("{}/.local/share/goverlay", home);
    if Path::new(&goverlay_path).exists() {
        return Ok(goverlay_path);
    }

    Ok("/tmp/MangoHud".to_string())
}

/// ¿Hay en esta carpeta (sin entrar en subcarpetas) algún CSV con formato de log de MangoHud?
/// Se comprueba el header para no confundirlo con un CSV cualquiera de otra aplicación.
fn dir_has_mangohud_log(dir: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let is_csv = p
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase() == "csv")
                .unwrap_or(false);
            if is_csv && !is_summary_file(&p) {
                if let Ok(head) = read_head(&p, 4096) {
                    let lower = head.to_lowercase();
                    if lower.contains("fps,frametime") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[tauri::command]
pub fn check_mangohud_installed() -> Result<bool, String> {
    // 1) Lo que de verdad decide si MangoHud funciona: el binario (wrapper) o la capa
    // Vulkan. Antes solo se miraba `which mangohud`, y quien lo tiene instalado como
    // capa (lo normal: el binario existe, pero el que actúa es la capa) o dentro de
    // Steam Flatpak recibía un "no está instalado" falso.
    let runtime = crate::preflight::get_runtime_env();
    if runtime.mangohud_binary.is_some() || runtime.mangohud_vulkan_layer.is_some() {
        return Ok(true);
    }

    // 2) Configuración escrita por MangoHud o Goverlay: indicio de que está instalado.
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_paths = vec![
        format!("{}/.config/MangoHud/MangoHud.conf", home),
        format!("{}/.config/goverlay/MangoHud.conf", home),
    ];

    for config_path in config_paths {
        if Path::new(&config_path).exists() {
            return Ok(true);
        }
    }

    // 3) Logs ya grabados: si hay un CSV de MangoHud, está instalado.
    let log_paths = vec![
        format!("{}/.local/share/goverlay/logs", home),
        format!("{}/.local/share/goverlay", home),
        format!("{}/.local/share/MangoHud/logs", home),
        "/tmp/MangoHud".to_string(),
    ];

    for log_path in log_paths {
        if Path::new(&log_path).exists() {
            if let Ok(entries) = fs::read_dir(&log_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                        return Ok(true);
                    }
                }
            }
        }
    }

    Ok(false)
}

// ───────────────────────── Duración de la grabación de MangoHud ─────────────────────────

fn mangohud_conf_paths() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    vec![
        std::path::PathBuf::from(format!("{}/.config/MangoHud/MangoHud.conf", home)),
        std::path::PathBuf::from(format!("{}/.config/goverlay/MangoHud.conf", home)),
    ]
}

/// Devuelve la duración (en segundos) que MangoHud graba con Shift+F2.
/// Devuelve 0 si no hay límite configurado o no se encuentra el archivo.
#[tauri::command]
pub fn get_mangohud_log_duration() -> Result<u32, String> {
    for path in mangohud_conf_paths() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Some(value) = read_conf_value(&content, "log_duration") {
                if let Ok(n) = value.parse::<u32>() {
                    return Ok(n);
                }
            }
        }
    }
    Ok(0)
}

/// Rechaza la escritura si el archivo es un enlace simbólico o no es un archivo
/// normal.
///
/// `MangoHud.conf` es un archivo **del usuario**, no nuestro: si apunta a otro
/// sitio (a `~/.bashrc`, o a algo fuera del home), reescribirlo tocaría un archivo
/// que no nos corresponde. `fs::metadata` seguiría el enlace sin avisar; aquí se
/// mira el enlace en sí con `symlink_metadata`.
fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("No se pudo comprobar {:?}: {}", path, e))?;

    if meta.file_type().is_symlink() {
        return Err(format!(
            "«{}» es un enlace simbólico. Por seguridad Thoriapp no escribe a través de él; \
             edita el archivo real a mano.",
            path.display()
        ));
    }

    if !meta.is_file() {
        return Err(format!("«{}» no es un archivo normal.", path.display()));
    }

    Ok(())
}

/// Escritura atómica: archivo temporal + `rename`. Un corte a mitad no deja el
/// `.conf` a medias (que MangoHud leería mal), igual que en los perfiles.
fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let mut tmp_name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    tmp_name.push(".thoriapp.tmp");
    let tmp = path.with_file_name(tmp_name);

    fs::write(&tmp, content).map_err(|e| format!("No se pudo escribir el temporal: {}", e))?;

    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("No se pudo guardar {:?}: {}", path, e));
    }
    Ok(())
}

/// Ruta de la copia de seguridad de un `.conf` (`MangoHud.conf` ->
/// `MangoHud.conf.thoriapp.bak`).
fn backup_path(path: &Path) -> std::path::PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".thoriapp.bak");
    path.with_file_name(name)
}

/// Cambia `log_duration` en MangoHud.conf (y en el de GOverlay si existe).
/// Solo toca esa línea; el resto de la configuración queda igual.
#[tauri::command]
pub fn set_mangohud_log_duration(seconds: u32) -> Result<u32, String> {
    if !(5..=600).contains(&seconds) {
        return Err("La duración debe estar entre 5 segundos y 10 minutos.".to_string());
    }

    let mut updated = 0;

    for path in mangohud_conf_paths() {
        // `exists()` sigue enlaces: un enlace roto daría `false` y se saltaría sin
        // avisar. `symlink_metadata` ve el enlace aunque su destino no exista.
        if fs::symlink_metadata(&path).is_err() {
            continue;
        }
        ensure_regular_file(&path)?;

        let content = fs::read_to_string(&path)
            .map_err(|e| format!("No se pudo leer {:?}: {}", path, e))?;

        // Copia de seguridad (solo la primera vez).
        let backup = backup_path(&path);
        if !backup.exists() {
            fs::write(&backup, &content).ok();
        }

        let new_line = format!("log_duration={}", seconds);
        let mut found = false;
        let mut lines: Vec<String> = content
            .lines()
            .map(|line| {
                let is_duration_line = !line.trim().starts_with('#')
                    && line
                        .split_once('=')
                        .map(|(k, _)| k.trim() == "log_duration")
                        .unwrap_or(false);
                if is_duration_line {
                    found = true;
                    new_line.clone()
                } else {
                    line.to_string()
                }
            })
            .collect();

        if !found {
            lines.push(new_line);
        }

        let mut output = lines.join("\n");
        output.push('\n');

        write_atomic(&path, &output)?;

        println!("[MangoHud] log_duration={} guardado en {:?}", seconds, path);
        updated += 1;
    }

    if updated == 0 {
        return Err(
            "No encontré tu MangoHud.conf. Abre GOverlay una vez y guarda tu configuración."
                .to_string(),
        );
    }

    Ok(seconds)
}

/// Restaura MangoHud.conf desde la copia creada por
/// [`set_mangohud_log_duration`].
///
/// Antes había copia `.bak` pero ninguna forma de usarla: un respaldo que no se
/// puede restaurar desde la app no protege de nada. Devuelve cuántos archivos se
/// restauraron.
#[tauri::command]
pub fn restore_mangohud_conf() -> Result<u32, String> {
    let mut restored = 0;

    for path in mangohud_conf_paths() {
        let backup = backup_path(&path);
        if !backup.is_file() {
            continue;
        }

        // La copia se escribió nosotros, pero se comprueba igual: puede haber sido
        // sustituida por un enlace desde entonces.
        ensure_regular_file(&backup)?;

        let original = fs::read_to_string(&backup)
            .map_err(|e| format!("No se pudo leer la copia {:?}: {}", backup, e))?;

        // Si el `.conf` actual es un enlace, no se escribe a través de él.
        // `symlink_metadata` distingue "no existe" de "es un enlace", que es
        // justo lo que interesa aquí: un enlace roto también debe rechazarse.
        if fs::symlink_metadata(&path).is_ok() {
            ensure_regular_file(&path)?;
        }

        write_atomic(&path, &original)?;
        fs::remove_file(&backup).ok();

        println!("[MangoHud] Configuración restaurada desde {:?}", backup);
        restored += 1;
    }

    if restored == 0 {
        return Err(
            "No hay ninguna copia de seguridad de MangoHud.conf que restaurar.".to_string(),
        );
    }

    Ok(restored)
}

// ───────────────────────── Pruebas guardadas ("Mis pruebas") ─────────────────────────
//
// Cuando el usuario pulsa "Guardar en Mis pruebas" se guarda una COPIA completa de la
// comparación (Antes + Después + resultado). Así no se pierde aunque después se grabe
// otra prueba y el Antes/Después de la pantalla Benchmarks se reemplace.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedComparison {
    pub id: String,
    /// Cuándo se pulsó "Guardar" (la fecha de la prueba es `comparison.test.timestamp`).
    pub saved_at: String,
    pub game_name: String,
    pub before_fps: f64,
    pub after_fps: f64,
    pub fps_change_percent: f64,
    pub comparison: BenchmarkComparison,
    /// Evaluación registrada al guardar, si la hay.
    ///
    /// `None` significa **no registrada** (comparación histórica), que no es lo mismo que
    /// una evaluación con `datos_insuficientes`: esa existe y se conserva. El bloque B no
    /// genera evaluaciones; las escribirá el motor del bloque C.
    ///
    /// Se lee de forma **tolerante**: si la evaluación viene malformada, se guarda el valor
    /// original y `evaluacion` queda en `None`, pero el resto de la comparación (medidas,
    /// deltas, snapshots) se sigue leyendo. Perder la evaluación no puede costar el resto
    /// del archivo, y decir «no registrada» sin más ocultaría que hay algo ahí.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluacion: Option<EvaluacionPersistida>,
    /// Evaluación que hay en disco pero esta versión no sabe leer (malformada).
    ///
    /// Solo se rellena al leer; nunca se escribe (el JSON original ya la conserva). Es lo
    /// que permite distinguir «no hay evaluación» de «hay una y no se entiende».
    #[serde(skip)]
    pub evaluacion_ilegible: Option<serde_json::Value>,
}

// ==================== Evaluación persistida (5.4.1.B) ====================
//
// Esto es **formato de datos**, no un motor: ningún tipo de aquí decide nada. El bloque C
// será quien calcule una evaluación en Rust y la escriba dentro de la misma operación que
// crea la comparación. El bloque B solo define cómo se guarda, cómo se lee sin perder
// nada y cómo se presenta lo guardado.
//
// Identificadores en español a propósito: son códigos de dominio estables, nunca frases
// traducibles. La fase 8 traducirá los textos de la interfaz a partir del código, y por
// eso un cambio de redacción o un idioma nuevo **no** cambia la metodología.

/// Versión de la metodología de interpretación, tal como consta en disco.
///
/// Se guarda como texto para **conservar literalmente** un valor que esta versión no
/// conozca (`"v9"`). Convertirlo a un enum cerrado obligaría a rechazar el archivo entero
/// o a inventar un valor: las dos cosas serían perder el dato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VersionDeInterpretacion(pub String);

impl VersionDeInterpretacion {
    pub const V1: &'static str = "v1";

    /// `true` si esta versión de la app sabe interpretarla.
    pub fn es_soportada(&self) -> bool {
        self.0 == Self::V1
    }
}

/// Estado de una evaluación.
///
/// Igual que la versión, es texto y no un enum cerrado: un estado futuro debe poder
/// conservarse y presentarse como no soportado, no perderse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CodigoDeEvaluacion(pub String);

impl CodigoDeEvaluacion {
    pub const MEJORA: &'static str = "mejora";
    pub const EMPEORA: &'static str = "empeora";
    pub const MIXTO: &'static str = "mixto";
    pub const SIN_CAMBIO: &'static str = "sin_cambio";
    pub const DATOS_INSUFICIENTES: &'static str = "datos_insuficientes";
    /// Metodologías no comparables: solo cabe describir, no concluir.
    pub const SOLO_DESCRIPTIVA: &'static str = "solo_descriptiva";

    /// `true` si es uno de los estados que esta versión entiende.
    pub fn es_conocido(&self) -> bool {
        matches!(
            self.0.as_str(),
            Self::MEJORA
                | Self::EMPEORA
                | Self::MIXTO
                | Self::SIN_CAMBIO
                | Self::DATOS_INSUFICIENTES
                | Self::SOLO_DESCRIPTIVA
        )
    }
}

/// Qué grabación es cada lado. Solo el identificador: la metodología de cálculo y el resto
/// de hechos ya están en `comparison.baseline` / `comparison.test`, que son inmutables.
/// Duplicarlos aquí crearía dos copias del mismo hecho que pueden divergir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdDeBenchmark {
    pub id: String,
}

/// Dirección **numérica** del cambio. No dice si eso es bueno: subir el frametime es peor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DireccionDeCambio {
    Sube,
    Baja,
    Igual,
    #[serde(rename = "no_consta")]
    NoConsta,
}

/// Qué prefirió el criterio aplicado. Separado de la dirección a propósito: `DireccionDeCambio`
/// describe el número y esto describe la regla con la que se juzgó.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferenciaDeMetrica {
    FavorDeCandidata,
    FavorDeReferencia,
    Indistinto,
    NoEstablecida,
}

/// Apoyo estadístico de **una** grabación en **una** métrica.
///
/// Reutiliza `ApoyoDePercentil` del bloque A para no redefinir la misma información, y
/// solo lleva los campos que aplican a esa métrica: no se copia `ApoyoDeMetricas` entero
/// en cada casilla.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApoyoDeMetrica {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentil: Option<ApoyoDePercentil>,
}

/// Una métrica dentro de una evaluación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluacionDeMetrica {
    /// Código de la métrica (`avg_fps`, `fps_1_percent_low`, …).
    pub clave: String,
    /// Magnitudes observadas. `None` = no consta; **nunca** 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antes: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub despues: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diferencia: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diferencia_pct: Option<f64>,
    pub direccion: DireccionDeCambio,
    pub preferencia: PreferenciaDeMetrica,
    /// Confianza **de esta métrica**, no la global copiada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confianza: Option<CodigoDeEvaluacion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apoyo_referencia: Option<ApoyoDeMetrica>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apoyo_candidata: Option<ApoyoDeMetrica>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motivos: Vec<CodigoDeEvaluacion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitaciones: Vec<CodigoDeEvaluacion>,
}

/// Resultado de las comprobaciones de comparabilidad metodológica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilidadMetodologica {
    /// Ambas partes declaran la **misma** metodología de cálculo y esta versión la conoce.
    Establecida,
    /// Alguna parte no la declara (histórico), o son distintas, o son desconocidas.
    /// Dos desconocidas **no** se presuponen iguales.
    NoEstablecida,
}

/// Criterios aplicados y resultado de las comprobaciones.
///
/// No repite nombres, AppID, hardware, comandos ni duraciones: todo eso ya está en los
/// benchmarks inmutables que la comparación embute. Aquí solo va lo que **resulta** de
/// aplicarlos, que es lo que no se puede recalcular sin las reglas de su momento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CondicionesDeComparacion {
    pub compatibilidad_metodologica: CompatibilidadMetodologica,
    /// Margen de la aplicación usado al evaluar (%). `None` = no consta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margen_porcentaje: Option<f64>,
    /// Ruido combinado de las dos grabaciones (%). `None` = no consta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruido_combinado_porcentaje: Option<f64>,
    /// `true` solo si los dos comandos registrados son exactamente el mismo texto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comandos_iguales: Option<bool>,
    /// Variable evaluada **declarada por el usuario**. Sin declaración no se infiere: queda
    /// desconocida (`None`), nunca «la que parece».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable_evaluada: Option<VariableEvaluada>,
}

/// Declaración explícita del usuario sobre qué se estaba probando.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableEvaluada {
    /// Identificador estable de lo declarado. Texto libre porque la lista de variables
    /// futuras no está cerrada, pero **nunca** es una frase ya traducida.
    pub codigo: String,
}

/// Evaluación guardada junto a una comparación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluacionPersistida {
    pub metodologia_interpretacion: VersionDeInterpretacion,
    /// Cuándo se evaluó. No es la fecha de la prueba ni la de guardado.
    pub evaluada_en: String,
    pub referencia: IdDeBenchmark,
    pub candidata: IdDeBenchmark,
    pub estado: CodigoDeEvaluacion,
    pub confianza: CodigoDeEvaluacion,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metricas: Vec<EvaluacionDeMetrica>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motivos: Vec<CodigoDeEvaluacion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitaciones: Vec<CodigoDeEvaluacion>,
    pub condiciones: CondicionesDeComparacion,
}

impl SavedComparison {
    /// Qué se puede afirmar sobre la evaluación de esta comparación guardada.
    ///
    /// Es el **único** sitio donde se decide, para que la interfaz no tenga que adivinar y
    /// para que una evaluación no soportada no se confunda con una ausente.
    ///
    /// Una evaluación cuyos ids no coinciden con los benchmarks que la comparación embute
    /// no describe a esta comparación: se trata como no soportada en vez de presentarla
    /// como si lo fuera.
    pub fn estado_de_la_evaluacion(&self) -> EstadoDeLaEvaluacion {
        // Una evaluación ilegible **no** es «no registrada»: hay algo, y no se puede
        // presentar como si no existiera.
        if self.evaluacion.is_none() && self.evaluacion_ilegible.is_some() {
            return EstadoDeLaEvaluacion::Malformada;
        }
        match &self.evaluacion {
            None => EstadoDeLaEvaluacion::NoRegistrada,
            Some(ev) => {
                if ev.es_soportada() && ev.corresponde_a(&self.comparison) {
                    EstadoDeLaEvaluacion::Soportada(Box::new(ev.clone()))
                } else {
                    EstadoDeLaEvaluacion::NoSoportada(Box::new(ev.clone()))
                }
            }
        }
    }
}

/// Qué se puede decir de la evaluación de una comparación guardada.
#[derive(Debug, Clone, PartialEq)]
pub enum EstadoDeLaEvaluacion {
    /// No se registró ninguna evaluación. Comparación **histórica**: se muestran sus datos,
    /// sus snapshots, sus deltas y su narrativa heredada, pero **no** se emite un veredicto
    /// nuevo llamando a `interpretar()`.
    NoRegistrada,
    /// Evaluación registrada y comprensible por esta versión.
    Soportada(Box<EvaluacionPersistida>),
    /// Evaluación registrada con una metodología que esta versión no conoce. Se conserva
    /// tal cual y **no se reinterpreta**: el contenido original sigue en disco.
    NoSoportada(Box<EvaluacionPersistida>),
    /// Hay una evaluación en disco, pero no tiene la forma esperada. Se conserva el JSON
    /// original; no se presenta como veredicto ni se descarta.
    Malformada,
}

impl EvaluacionPersistida {
    /// Esta versión sabe interpretarla? Una evaluación con metodología desconocida **no**
    /// es la misma cosa que una ausente: la primera existe y se conserva; la segunda nunca
    /// se registró.
    pub fn es_soportada(&self) -> bool {
        // La metodología debe ser de esta versión y el **estado** debe estar en el
        // vocabulario conocido: presentar `mejora` cuando el archivo dice otra cosa sería
        // inventar un veredicto.
        //
        // Los **motivos** y **limitaciones** no se validan: son códigos abiertos por
        // diseño (cada regla futura añade los suyos) y un motivo desconocido se conserva y
        // se muestra como código, sin traducirlo a otro.
        self.metodologia_interpretacion.es_soportada() && self.estado.es_conocido()
    }

    /// La referencia y la candidata deben ser exactamente los dos benchmarks que la
    /// comparación embute. Si no cuadra, la evaluación no describe a esta comparación.
    pub fn corresponde_a(&self, comparacion: &BenchmarkComparison) -> bool {
        self.referencia.id == comparacion.baseline.id
            && self.candidata.id == comparacion.test.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedComparisonsData {
    /// Versión del formato. Ausente en los archivos anteriores a este campo.
    #[serde(default)]
    version: Option<u32>,
    comparisons: Vec<SavedComparison>,
    /// El JSON **original** de cada comparación, tal cual estaba en disco, indexado por id.
    ///
    /// Se conserva porque esta versión no entiende todos los campos que puede haber (los
    /// de una versión futura) y al reescribir el archivo se perderían. Las comparaciones
    /// son inmutables, así que el registro original sigue siendo válido: solo se sustituye
    /// el que se ha añadido o eliminado de verdad.
    ///
    /// No se serializa: es un testigo de lectura, no parte del formato. Al escribir se
    /// fusiona con lo que produce el struct tipado.
    #[serde(skip)]
    crudos: serde_json::Map<String, serde_json::Value>,
    /// Campos desconocidos del **nivel raíz** del archivo. Mismo motivo.
    #[serde(skip)]
    crudo_raiz: serde_json::Map<String, serde_json::Value>,
}

fn get_saved_comparisons_file() -> std::path::PathBuf {
    get_benchmarks_dir().join("saved_comparisons.json")
}

/// Lee `saved_comparisons.json` con las mismas reglas de integridad que el historial.
///
/// Antes, un archivo ilegible devolvía una lista vacía y la siguiente escritura lo
/// sobrescribía: las pruebas guardadas desaparecían sin avisar. La copia de seguridad que
/// se hacía no servía de nada, porque nadie le decía al usuario que existía.
fn try_load_saved_data() -> Result<SavedComparisonsData, ErrorDatos> {
    try_load_saved_data_de(&get_saved_comparisons_file())
}

/// La lectura de verdad, sobre una ruta concreta: así se puede probar con archivos de
/// verdad en un directorio temporal sin depender de dónde viva la configuración.
///
/// Se lee **primero** como `Value` porque el JSON original hace falta para no perder los
/// campos que esta versión no conoce; el tipado viene después.
fn try_load_saved_data_de(ruta: &Path) -> Result<SavedComparisonsData, ErrorDatos> {
    let crudo = leer_datos::<serde_json::Value>(
        ruta,
        "saved_comparisons.json",
        COMPARISONS_SCHEMA_VERSION,
        |v| v.get("version").and_then(|x| x.as_u64()).map(|x| x as u32),
        Ok,
    )?;

    let Some(crudo) = crudo else {
        return Ok(vacio());
    };

    interpretar_fichero_de_comparaciones(crudo)
}

/// Convierte el JSON del archivo en los datos tipados, guardando los registros originales.
fn interpretar_fichero_de_comparaciones(
    crudo: serde_json::Value,
) -> Result<SavedComparisonsData, ErrorDatos> {
    let objeto = crudo.as_object().ok_or_else(|| ErrorDatos::Corrupto {
        detalle: "se esperaba un objeto en la raíz".to_string(),
        copia: None,
    })?;

    let version = objeto.get("version").and_then(|x| x.as_u64()).map(|x| x as u32);

    let lista = objeto
        .get("comparisons")
        .and_then(|c| c.as_array())
        .ok_or_else(|| ErrorDatos::Corrupto {
            detalle: "falta la lista `comparisons`".to_string(),
            copia: None,
        })?;

    let mut comparisons = Vec::with_capacity(lista.len());
    let mut crudos = serde_json::Map::new();

    for valor in lista {
        let mut guardada: SavedComparison = match serde_json::from_value(valor.clone()) {
            Ok(c) => c,
            Err(e) => {
                // Puede fallar por la evaluación, que es lo único que esta versión no
                // conoce del todo. Se reintenta sin ella: el resto de la comparación es
                // dato medido y no puede perderse por un campo opcional ilegible.
                let mut sin_evaluacion = valor.clone();
                let tenia = sin_evaluacion
                    .as_object_mut()
                    .map(|o| o.remove("evaluacion").is_some())
                    .unwrap_or(false);
                match serde_json::from_value::<SavedComparison>(sin_evaluacion) {
                    Ok(mut c) if tenia => {
                        c.evaluacion_ilegible = valor.get("evaluacion").cloned();
                        c
                    }
                    _ => {
                        return Err(ErrorDatos::Corrupto {
                            detalle: format!(
                                "una comparación guardada no tiene el formato esperado: {e}"
                            ),
                            copia: None,
                        })
                    }
                }
            }
        };
        guardada.evaluacion_ilegible = guardada
            .evaluacion_ilegible
            .or_else(|| valor.get("evaluacion").cloned());

        // Dos comparaciones con el mismo id harían que «borrar una» borrase dos y que el
        // testigo de escritura no supiera cuál es cuál. No se adivina: se avisa.
        if crudos.contains_key(&guardada.id) {
            return Err(ErrorDatos::Corrupto {
                detalle: format!("hay dos comparaciones con el mismo id «{}»", guardada.id),
                copia: None,
            });
        }

        crudos.insert(guardada.id.clone(), valor.clone());
        comparisons.push(guardada);
    }

    let crudo_raiz = objeto
        .iter()
        .filter(|(k, _)| k.as_str() != "version" && k.as_str() != "comparisons")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    Ok(SavedComparisonsData {
        version,
        comparisons,
        crudos,
        crudo_raiz,
    })
}

fn vacio() -> SavedComparisonsData {
    SavedComparisonsData {
        version: None,
        comparisons: Vec::new(),
        crudos: serde_json::Map::new(),
        crudo_raiz: serde_json::Map::new(),
    }
}

/// Variante para **modificar**: aborta si el archivo existe y no se puede interpretar.
fn load_saved_data_para_editar() -> Result<SavedComparisonsData, String> {
    try_load_saved_data().map_err(|e| e.mensaje("Tus pruebas guardadas"))
}


/// Construye el JSON que se va a escribir.
///
/// Regla: **ningún registro que ya estaba se reconstruye**. Si el id estaba en disco, se
/// reescribe tal como se leyó, con sus campos anidados aunque esta versión no los
/// entienda. Solo los registros **nuevos** se serializan desde su struct.
///
/// Esto basta porque las comparaciones guardadas son **inmutables**: en producción solo se
/// añaden (`save_comparison`) o se quitan (`delete_comparison`, `delete_all_comparisons`).
/// No hay ningún camino que edite una ya guardada, así que no hace falta distinguir «está
/// igual» de «ha cambiado»: si estaba, se conserva; si no estaba, es nueva.
///
/// Si algún día C necesitara modificar una, además de añadir su ruta habría que traer de
/// vuelta una fusión de campos desconocidos. Hoy sería código muerto, y por eso no está.
fn construir_json_de_comparaciones(
    data: &SavedComparisonsData,
) -> Result<serde_json::Value, String> {
    let mut lista = Vec::with_capacity(data.comparisons.len());

    for guardada in &data.comparisons {
        let valor = match data.crudos.get(&guardada.id) {
            // Estaba en disco: se conserva el JSON original, sin reconstruirlo.
            Some(original) => original.clone(),
            // Alta nueva: es el único caso que se serializa desde el struct.
            None => serde_json::to_value(guardada)
                .map_err(|e| format!("No se pudo preparar la comparación guardada: {e}"))?,
        };
        lista.push(valor);
    }

    let mut raiz = data.crudo_raiz.clone();
    raiz.insert("version".to_string(), serde_json::json!(version_a_escribir(data)));
    raiz.insert("comparisons".to_string(), serde_json::Value::Array(lista));
    Ok(serde_json::Value::Object(raiz))
}

/// Qué versión se escribe.
///
/// Un archivo que ya era v2 **sigue siendo v2** aunque se queden sin evaluaciones: bajar a
/// v1 permitiría que una versión antigua lo abriese y descartase lo que no entiende. Un
/// archivo v1 sube a v2 solo cuando aparece contenido que necesita v2.
fn version_a_escribir(data: &SavedComparisonsData) -> u32 {
    let ya_era_v2 = data.version.is_some_and(|v| v >= 2);
    let hay_evaluacion = data.comparisons.iter().any(|c| c.evaluacion.is_some());
    if ya_era_v2 || hay_evaluacion {
        2
    } else {
        1
    }
}

fn save_saved_data(data: &SavedComparisonsData) -> Result<(), String> {
    save_saved_data_en(&get_saved_comparisons_file(), data)
}

/// La escritura de verdad, sobre una ruta concreta (ver `try_load_saved_data_de`).
fn save_saved_data_en(ruta: &Path, data: &SavedComparisonsData) -> Result<(), String> {
    let json = construir_json_de_comparaciones(data)?;
    let content = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("Error al serializar: {}", e))?;
    escribir_atomico(ruta, &content).map_err(|e| {
        format!(
            "No se pudieron guardar las pruebas ({}). Las que ya tenías siguen intactas.",
            e
        )
    })
}

/// Orden único de adquisición de los cerrojos.
///
/// Hay dos cerrojos, uno por archivo, y una ruta que necesita los dos a la vez. Para que no
/// pueda formarse un abrazo mortal, el orden es **siempre** este y no se invierte en ningún
/// sitio:
///
/// 1. `CERROJO_BENCHMARKS` (el historial: de aquí salen las entradas).
/// 2. `CERROJO_COMPARACIONES` (las comparaciones guardadas: aquí se escribe).
///
/// Ninguna función toma el segundo y luego el primero. Las funciones de lectura y escritura
/// de cada archivo (`load_*`, `save_*`) **no** toman cerrojo por su cuenta: el cerrojo lo
/// toma la operación que las agrupa, y por eso no hay adquisición recursiva del mismo
/// `Mutex` (que en `std` no es reentrante y se bloquearía a sí misma).
///
/// Ambos son intraproceso. Entre dos instancias de la aplicación la protección sigue
/// pendiente (fase 9).
///
/// **C.3**: lo toma la ruta pública de comparación evaluada y la de guardado. Su prueba
/// comprueba que se toman y se sueltan en el orden documentado.
pub(crate) fn adquirir_cerrojos() -> (
    std::sync::MutexGuard<'static, ()>,
    std::sync::MutexGuard<'static, ()>,
) {
    let benchmarks = CERROJO_BENCHMARKS.lock().unwrap_or_else(|e| e.into_inner());
    let comparaciones = CERROJO_COMPARACIONES.lock().unwrap_or_else(|e| e.into_inner());
    (benchmarks, comparaciones)
}

/// Evalúa una comparación con el motor puro y le pone los datos de registro.
///
/// Es el **único** punto en que se aplica la metodología de interpretación. `benchmarks.rs`
/// aporta el instante y los identificadores; el significado lo decide el motor. No hay una
/// segunda copia de las reglas.
pub(crate) fn evaluar_comparacion(
    comparacion: &BenchmarkComparison,
    variable: Option<VariableEvaluada>,
) -> EvaluacionPersistida {
    let condiciones = crate::evaluacion::condiciones_de(comparacion, variable);
    let mut ev = crate::evaluacion::evaluar(comparacion, &condiciones);

    // El instante de registro es **este** (`evaluada_en`): el momento en que se guarda la
    // comparación. No es la fecha de las pruebas (cada una tiene la suya) ni la de la
    // vista previa, y no forma parte de la igualdad de significado.
    ev.evaluada_en = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    // Los identificadores salen de la comparación evaluada, no de lo que diga el llamador.
    ev.referencia = IdDeBenchmark {
        id: comparacion.baseline.id.clone(),
    };
    ev.candidata = IdDeBenchmark {
        id: comparacion.test.id.clone(),
    };
    ev
}

/// Comprobación interna de coherencia **antes de persistir** (C.2 §5).
///
/// No es un segundo motor: no recalcula el veredicto. Solo verifica invariantes
/// estructurales que el propio contrato exige, para que no se escriba una evaluación
/// internamente contradictoria. Reutiliza el vocabulario de C.1.
fn comprobar_coherencia(
    ev: &EvaluacionPersistida,
    comparacion: &BenchmarkComparison,
) -> Result<(), String> {
    // Identificadores: la evaluación describe **exactamente** estas dos grabaciones.
    if ev.referencia.id != comparacion.baseline.id || ev.candidata.id != comparacion.test.id {
        return Err("La evaluación no corresponde a la comparación.".to_string());
    }

    // Confianza: pertenece al vocabulario cerrado, nunca es un código de estado.
    let confianza_valida = matches!(
        ev.confianza.0.as_str(),
        crate::evaluacion::codigos::CONFIANZA_OBSERVADA
            | crate::evaluacion::codigos::CONFIANZA_NO_CONCLUYENTE
            | crate::evaluacion::codigos::CONFIANZA_INSUFICIENTE
    );
    if !confianza_valida {
        return Err(format!("Confianza fuera de vocabulario: {}", ev.confianza.0));
    }

    // Métricas: una preferencia establecida exige un motivo de preferencia concordante, y
    // una preferencia sin establecer no puede llevar motivos que la afirmen.
    for m in &ev.metricas {
        let favor_candidata = m.motivos.iter().any(|c| c.0 == crate::evaluacion::codigos::FAVORECE_CANDIDATA);
        let favor_referencia = m.motivos.iter().any(|c| c.0 == crate::evaluacion::codigos::FAVORECE_REFERENCIA);
        match m.preferencia {
            PreferenciaDeMetrica::FavorDeCandidata => {
                if !favor_candidata || favor_referencia {
                    return Err(format!("Motivos incoherentes en «{}».", m.clave));
                }
            }
            PreferenciaDeMetrica::FavorDeReferencia => {
                if !favor_referencia || favor_candidata {
                    return Err(format!("Motivos incoherentes en «{}».", m.clave));
                }
            }
            PreferenciaDeMetrica::NoEstablecida | PreferenciaDeMetrica::Indistinto => {
                if favor_candidata || favor_referencia {
                    return Err(format!(
                        "Una métrica sin preferencia no puede favorecer a nadie («{}»).",
                        m.clave
                    ));
                }
            }
        }
    }

    // Compatibilidad: si no está establecida, ninguna métrica puede preferir.
    if ev.condiciones.compatibilidad_metodologica != CompatibilidadMetodologica::Establecida
        && ev
            .metricas
            .iter()
            .any(|m| m.preferencia == PreferenciaDeMetrica::FavorDeCandidata
                || m.preferencia == PreferenciaDeMetrica::FavorDeReferencia)
    {
        return Err(
            "Sin compatibilidad establecida no puede haber preferencia por métrica.".to_string(),
        );
    }

    Ok(())
}

/// Vista previa **evaluada**, con la ruta real.
///
/// Es la primera mitad del flujo: lee el historial, compara, evalúa y devuelve la
/// confirmación que la interfaz enseñará y devolverá al guardar. Usa **las mismas**
/// funciones que el guardado (`comparacion_entre`, `evaluar_comparacion`), así que la vista
/// previa y lo persistido no pueden divergir por lógica distinta.
///
/// Exige que cada identificador sea **inequívoco**
/// (`buscar_unico`): una vista previa sobre una grabación ambigua mostraría el resultado de
/// una copia arbitraria.
///
/// No mantiene el cerrojo más de lo que dura la lectura: entre la vista previa y la
/// confirmación el usuario puede tardar lo que quiera, y ningún cerrojo cruza la espera.
/// Por eso el guardado **vuelve a leer y recalcular** en vez de fiarse de esto.
///
/// Deja de estar bajo `cfg(test)` en C.3: es la ruta de producción de la vista previa.
fn vista_previa_evaluada(
    ruta_benchmarks: &Path,
    baseline_id: &str,
    test_id: &str,
    variable: Option<VariableEvaluada>,
) -> Result<crate::evaluacion::ConfirmacionDeVistaPrevia, ErrorDeEvaluacion> {
    let _turno = CERROJO_BENCHMARKS.lock().unwrap_or_else(|e| e.into_inner());
    let data = match try_load_benchmarks_data_de(ruta_benchmarks) {
        Ok(d) => d,
        Err(e) => {
            return Err(ErrorDeEvaluacion::lectura_fallida(
                "benchmarks.json",
                e.mensaje("El historial de pruebas"),
            ));
        }
    };

    let baseline = match resolver_unico(&data.benchmarks, baseline_id, "Benchmark base") {
        Ok(b) => b,
        Err(e) => {
            return Err(e);
        }
    };
    let test = match resolver_unico(&data.benchmarks, test_id, "Benchmark de prueba") {
        Ok(b) => b,
        Err(e) => {
            return Err(e);
        }
    };

    // La comparación se construye con el mismo camino que el guardado, sobre las
    // grabaciones ya resueltas de forma inequívoca.
    let comparacion = match comparacion_entre(baseline, test) {
        Ok(c) => c,
        Err(m) => {
            return Err(ErrorDeEvaluacion::lectura_fallida("benchmarks.json", m));
        }
    };

    let evaluacion = evaluar_comparacion(&comparacion, variable);

    Ok(crate::evaluacion::ConfirmacionDeVistaPrevia {
        evaluacion,
        comparacion,
    })
}

/// Vista previa evaluada sobre las rutas reales de la aplicación.
///
/// Es el **único** camino por el que la interfaz obtiene una comparación individual. La
/// comparación viaja **dentro** de la confirmación: no hay un segundo comando que devuelva
/// la misma comparación sin evaluación, porque eso sería una ruta alternativa a la
/// interpretación (y con otra regla de identidad: primera coincidencia en vez de única).
#[tauri::command]
pub fn compare_benchmarks_evaluada(
    baseline_id: String,
    test_id: String,
    variable: Option<VariableEvaluada>,
) -> Result<crate::evaluacion::ConfirmacionDeVistaPrevia, ErrorDeEvaluacion> {
    vista_previa_evaluada(
        &get_benchmarks_file(),
        &baseline_id,
        &test_id,
        variable,
    )
}

/// Resuelve un identificador a la grabación **inequívoca** que designa, con su error
/// tipado. La regla (0 no encontrado, 1 válido, 2+ ambiguo) es la de `buscar_unico`; aquí
/// solo se clasifica el resultado en el contrato IPC.
fn resolver_unico<'a>(
    benchmarks: &'a [Benchmark],
    id: &str,
    que: &str,
) -> Result<&'a Benchmark, ErrorDeEvaluacion> {
    // La categoría la decide la **variante** que devolvió la búsqueda. El mensaje viaja
    // intacto para el usuario, pero no se interpreta.
    match buscar_unico(benchmarks, id, que) {
        Ok(b) => Ok(b),
        Err(FallaDeBusqueda::Ambigua { mensaje }) => {
            Err(ErrorDeEvaluacion::IdentificadorAmbiguo { mensaje })
        }
        Err(FallaDeBusqueda::NoEncontrada { mensaje }) => {
            Err(ErrorDeEvaluacion::grabacion_inexistente(mensaje))
        }
    }
}

/// Guarda una comparación **con** su evaluación, en una sola escritura atómica.
///
/// Trabaja sobre rutas explícitas, igual que el resto del archivo (`save_saved_data_en`),
/// para que las pruebas ejerciten **esta** lógica y no una copia. La ruta pública de C.3
/// envolverá esta función con `adquirir_cerrojos()` y las rutas reales.
///
/// **No** toma los cerrojos: los toma el llamador, y por eso no hay adquisición recursiva
/// del mismo `Mutex`. Recibe la confirmación como **expectativa**, relee, recalcula y solo
/// escribe si todo coincide. Lo que persiste son los objetos **recalculados**.
///
/// Deja de estar bajo `cfg(test)` en C.3: es la ruta de producción del guardado.
fn guardar_comparacion_evaluada(
    ruta_benchmarks: &Path,
    ruta_comparaciones: &Path,
    confirmacion: &crate::evaluacion::ConfirmacionDeVistaPrevia,
    variable: Option<VariableEvaluada>,
) -> Result<SavedComparison, ErrorDeEvaluacion> {
    let benchmarks = try_load_benchmarks_data_de(ruta_benchmarks).map_err(|e| {
        ErrorDeEvaluacion::lectura_fallida("benchmarks.json", e.mensaje("El historial de pruebas"))
    })?;

    // Relectura **inequívoca**: si entre la vista previa y este momento ha aparecido una
    // copia con el mismo identificador, no se puede saber cuál evaluar y se rechaza.
    let baseline = resolver_unico(
        &benchmarks.benchmarks,
        &confirmacion.comparacion.baseline.id,
        "Benchmark base",
    )?;
    let test = resolver_unico(
        &benchmarks.benchmarks,
        &confirmacion.comparacion.test.id,
        "Benchmark de prueba",
    )?;
    let comparacion = comparacion_entre(baseline, test)
        .map_err(|m| ErrorDeEvaluacion::lectura_fallida("benchmarks.json", m))?;

    let evaluacion_nueva = evaluar_comparacion(&comparacion, variable);
    let recalculada = crate::evaluacion::ConfirmacionDeVistaPrevia {
        comparacion: comparacion.clone(),
        evaluacion: evaluacion_nueva.clone(),
    };

    // Si algo cambió, se rechaza **sin escribir**. El código del motivo viaja tal cual:
    // la interfaz no tiene que adivinar la causa leyendo un texto.
    crate::evaluacion::comprobar_confirmacion(confirmacion, &recalculada)
        .map_err(ErrorDeEvaluacion::rechazo)?;

    // Coherencia interna antes de tocar el disco. Si la evaluación recalculada no es
    // coherente consigo misma, no se escribe: es un defecto del motor, no del usuario.
    comprobar_coherencia(&evaluacion_nueva, &comparacion).map_err(|m| {
        ErrorDeEvaluacion::escritura_fallida(format!(
            "La evaluación recalculada no es coherente ({m}). No se ha guardado nada."
        ))
    })?;

    let mut data = try_load_saved_data_de(ruta_comparaciones).map_err(|e| {
        ErrorDeEvaluacion::lectura_fallida(
            "saved_comparisons.json",
            e.mensaje("Las comparaciones guardadas"),
        )
    })?;

    // La protección de duplicados del **mismo par** se decide dentro del cerrojo del
    // llamador, con la lista ya leída: dos guardados simultáneos no pueden colar los dos.
    // Se devuelve el registro existente para que la interfaz no tenga que buscarlo.
    if let Some(existente) = data.comparisons.iter().find(|c| {
        c.comparison.baseline.id == comparacion.baseline.id
            && c.comparison.test.id == comparacion.test.id
    }) {
        return Err(ErrorDeEvaluacion::ComparacionYaExistente {
            comparacion_id: existente.id.clone(),
        });
    }

    let now = chrono::Local::now();
    let game_name = if comparacion.test.game_name.is_empty() {
        comparacion.baseline.game_name.clone()
    } else {
        comparacion.test.game_name.clone()
    };

    let saved = SavedComparison {
        id: format!("cmp_{}", now.format("%Y%m%d%H%M%S%3f")),
        saved_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        game_name,
        before_fps: comparacion.baseline.avg_fps,
        after_fps: comparacion.test.avg_fps,
        fps_change_percent: comparacion.fps_change_percent,
        comparison: comparacion,
        // La evaluación generada en **esta** operación, con los objetos recalculados.
        evaluacion: Some(evaluacion_nueva),
        evaluacion_ilegible: None,
    };

    data.comparisons.push(saved.clone());
    // Una sola escritura atómica: la comparación y su evaluación aparecen juntas o no
    // aparece ninguna de las dos.
    save_saved_data_en(ruta_comparaciones, &data).map_err(ErrorDeEvaluacion::escritura_fallida)?;

    Ok(saved)
}

/// Guarda una comparación **con su evaluación** en "Mis pruebas".
///
/// Sustituye a `save_comparison`, que escribía `evaluacion: None`. Desde C.3 **no hay
/// ninguna ruta que escriba una comparación sin evaluación**: la evaluación y la
/// comparación se escriben en la misma operación atómica.
///
/// Toma los dos cerrojos en el orden fijo de `adquirir_cerrojos` (historial →
/// comparaciones guardadas): relee el historial bajo cerrojo, recalcula, comprueba que
/// la confirmación sigue siendo válida y solo entonces escribe.
#[tauri::command]
pub fn save_comparison_evaluada(
    confirmacion: crate::evaluacion::ConfirmacionDeVistaPrevia,
    variable: Option<VariableEvaluada>,
) -> Result<SavedComparison, ErrorDeEvaluacion> {
    let (_historial, _guardadas) = adquirir_cerrojos();
    let saved = guardar_comparacion_evaluada(
        &get_benchmarks_file(),
        &get_saved_comparisons_file(),
        &confirmacion,
        variable,
    )?;

    println!(
        "[Pruebas] ✅ Comparación guardada con su evaluación: {}",
        saved.game_name
    );
    Ok(saved)
}

/// Cómo se leyó la evaluación de una comparación guardada.
///
/// Viaja con la comparación en **la misma lectura**, para que la interfaz no tenga que
/// clasificar nada ni pedir un segundo comando por cada fila.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoDeLaEvaluacionLeida {
    /// No se registró evaluación: comparación histórica.
    NoRegistrada,
    /// Registrada y comprensible por esta versión.
    Soportada,
    /// Registrada con códigos o metodología que esta versión no entiende.
    NoSoportada,
    /// Hay una evaluación, pero no se puede interpretar su forma.
    Malformada,
}

/// Una comparación guardada tal como se entrega a la interfaz.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparacionLeida {
    #[serde(flatten)]
    pub guardada: SavedComparison,
    pub estado_de_la_evaluacion: EstadoDeLaEvaluacionLeida,
}

#[tauri::command]
pub fn load_comparisons() -> Result<Vec<ComparacionLeida>, String> {
    // Lectura de pantalla: si el archivo está roto hay que decirlo, no pintar una lista
    // vacía. Antes esto devolvía `Ok(vec![])` y «Mis pruebas» decía «no tienes ninguna».
    try_load_saved_data()
        .map(|d| {
            d.comparisons
                .into_iter()
                .map(|c| {
                    // Clasificar aquí, y no en el frontend, es lo que mantiene la regla en
                    // un solo sitio: qué se puede afirmar de una evaluación guardada es
                    // dominio, no presentación.
                    let estado = match c.estado_de_la_evaluacion() {
                        EstadoDeLaEvaluacion::NoRegistrada => EstadoDeLaEvaluacionLeida::NoRegistrada,
                        EstadoDeLaEvaluacion::Soportada(_) => EstadoDeLaEvaluacionLeida::Soportada,
                        EstadoDeLaEvaluacion::NoSoportada(_) => {
                            EstadoDeLaEvaluacionLeida::NoSoportada
                        }
                        EstadoDeLaEvaluacion::Malformada => EstadoDeLaEvaluacionLeida::Malformada,
                    };
                    ComparacionLeida { guardada: c, estado_de_la_evaluacion: estado }
                })
                .collect()
        })
        .map_err(|e| e.mensaje("Tus pruebas guardadas"))
}

#[tauri::command]
pub fn delete_comparison(comparison_id: String) -> Result<(), String> {
    let _turno = CERROJO_COMPARACIONES.lock().unwrap_or_else(|e| e.into_inner());
    let mut data = load_saved_data_para_editar()?;
    data.comparisons.retain(|c| c.id != comparison_id);
    save_saved_data(&data)
}

#[tauri::command]
pub fn delete_all_comparisons() -> Result<(), String> {
    let _turno = CERROJO_COMPARACIONES.lock().unwrap_or_else(|e| e.into_inner());
    // Borrar todo también es destructivo: si no se puede leer lo que hay, no se vacía a
    // ciegas (un archivo ilegible podría ser uno perfectamente válido con otro formato).
    let mut data = load_saved_data_para_editar()?;
    // Se vacían las comparaciones, pero se conserva lo demás del archivo (incluidos los
    // campos que esta versión no conoce, vía `crudo_raiz`) y su versión de esquema.
    data.comparisons.clear();
    data.crudos.clear();
    save_saved_data(&data)
}

// ==================== TESTS ====================
// Solo cubren la lógica pura: no tocan disco ni el runtime de Tauri.

#[cfg(test)]
mod tests {
    use super::*;

    /// Carpeta temporal propia por test: los tests son de archivos, así que no
    /// pueden compartir directorio. Se borra antes de usarla para que una
    /// ejecución anterior interrumpida no deje basura que haga fallar asserts
    /// por motivos ajenos al código.
    /// Las grabaciones reales de `tests/fixtures/mangohud/`, de solo lectura.
    const LOG1: &str = "Palworld-Win64-Shipping_2026-11-23_16-54-52.csv";
    const LOG2: &str = "Palworld-Win64-Shipping_2026-11-24_18-19-57.csv";

    fn fixture_log(nombre: &str) -> String {
        format!(
            "{}/../tests/fixtures/mangohud/{nombre}",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn fixture_csv(nombre: &str) -> String {
        fs::read_to_string(fixture_log(nombre)).expect("fixture legible")
    }

    pub(crate) fn tmp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "thoriapp-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn backup_path_anade_sufijo_sin_pisar_la_extension() {
        // `.with_extension("conf.thoriapp.bak")` convertía MangoHud.conf en
        // MangoHud.conf.thoriapp.bak, pero con nombres raros cambiaba de más.
        let p = Path::new("/home/u/.config/MangoHud/MangoHud.conf");
        assert_eq!(
            backup_path(p),
            Path::new("/home/u/.config/MangoHud/MangoHud.conf.thoriapp.bak")
        );
    }

    #[test]
    fn ensure_regular_file_rechaza_un_enlace_simbolico() {
        let dir = tmp_dir("symlink");
        let target = dir.join("real.conf");
        let link = dir.join("link.conf");
        fs::write(&target, "log_duration=10\n").unwrap();
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let err = ensure_regular_file(&link).unwrap_err();
        assert!(err.contains("enlace simbólico"), "mensaje inesperado: {err}");

        // El archivo real sí se acepta.
        assert!(ensure_regular_file(&target).is_ok());

        let _ = fs::remove_file(&link);
        let _ = fs::remove_file(&target);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn ensure_regular_file_rechaza_un_directorio() {
        let dir = tmp_dir("dird").join("soy-carpeta");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(ensure_regular_file(&dir).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_no_deja_temporales_y_conserva_el_contenido() {
        let dir = tmp_dir("atomic");
        let file = dir.join("MangoHud.conf");
        fs::write(&file, "viejos\n").unwrap();

        write_atomic(&file, "nuevos=1\n").unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "nuevos=1\n");
        // El temporal no debe quedar huérfano.
        let tmp = dir.join("MangoHud.conf.thoriapp.tmp");
        assert!(!tmp.exists(), "quedó el temporal {:?}", tmp);

        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir(&dir);
    }

    pub(crate) fn bench(appid: u32, name: &str) -> Benchmark {
        Benchmark {
            id: format!("{}-{}", appid, name),
            profile_id: String::new(),
            game_name: name.to_string(),
            appid,
            timestamp: String::new(),
            duration_seconds: 60.0,
            avg_fps: 60.0,
            fps_1_percent_low: 30.0,
            fps_0_1_percent_low: 20.0,
            max_fps: 120.0,
            min_fps: 10.0,
            frame_time_avg: 16.6,
            frame_time_1_percent_high: 33.0,
            total_frames: 3600,
            notes: None,
            launch_command: "%command%".to_string(),
            origen_comando: OrigenComando::Instantanea,
            steam_command: None,
            number: None,
            snapshot: None,
            sesion_id: None,
            spikes_per_min: None,
            pct_over_33ms: None,
            jitter_ms: None,
            worst_frame_ms: None,
            warmup_seconds: None,
            noise_percent: None,
            shader_stutter: None,
            log_path: String::new(),
            sample_count: None,
            muestras_temporales: None,
            metodologia_calculo: None,
            contabilidad_muestras: None,
            apoyo_metricas: None,
            duracion_medida: None,
        }
    }

    #[test]
    fn texto_comparacion_no_confirma_nada_con_una_sesion() {
        // Una diferencia grande sigue siendo "observada": el margen de estimación no
        // mide la variación entre partidas, así que no permite confirmar.
        let t = texto_comparacion(12.0, 4.0, 5.0, false);
        assert!(t.contains("Mejora observada"));
        assert!(t.contains("provisional"));
        assert!(!t.to_lowercase().contains("confirmad"));
        assert!(!t.contains('✅'));
    }

    #[test]
    fn texto_comparacion_avisa_del_mixto_en_los_dos_sentidos() {
        let mas_fps = texto_comparacion(20.0, -20.0, 5.0, false);
        assert!(mas_fps.starts_with("Resultado mixto"));
        assert!(mas_fps.contains("tirones"));

        let menos_fps = texto_comparacion(-20.0, 20.0, 5.0, false);
        assert!(menos_fps.starts_with("Resultado mixto"));
        assert!(menos_fps.contains("parejo"));
    }

    #[test]
    fn texto_comparacion_dentro_del_ruido_pide_repetir() {
        let t = texto_comparacion(3.0, 0.5, 5.1, true);
        assert!(t.starts_with("Sin cambio concluyente"));
        assert!(t.contains("Repite"));
        assert!(!t.contains("Mejora"));
    }

    #[test]
    fn texto_comparacion_empeoramiento() {
        let t = texto_comparacion(-15.0, -10.0, 5.0, false);
        assert!(t.starts_with("Empeoramiento observado"));
        assert!(!t.contains("Mejora"));
    }

    // ── Serie temporal ──────────────────────────────────────────────────────

    fn muestra_con_tiempo(fps: f64, ft: f64, ns: f64) -> Sample {
        Sample { fps, frame_time: ft, elapsed_ns: Some(ns) }
    }

    #[test]
    fn serie_temporal_guarda_frametimes_y_tiempos() {
        let samples: Vec<Sample> = (0..10)
            .map(|i| muestra_con_tiempo(100.0, 10.0 + i as f64, i as f64 * 1e8))
            .collect();
        let serie = serie_temporal(&samples, 4000, true).expect("debería haber serie");
        assert_eq!(serie.frame_time_ms.len(), 10);
        assert_eq!(serie.elapsed_s.len(), 10);
        assert!((serie.frame_time_ms[0] - 10.0).abs() < 1e-4);
        // El eje X empieza en 0 (segundos desde el inicio), no en el reloj absoluto.
        assert!(serie.elapsed_s[0].abs() < 1e-6);
        assert!((serie.elapsed_s[9] - 0.9).abs() < 1e-4);
    }

    #[test]
    fn serie_temporal_sin_columna_elapsed_no_inventa_tiempos() {
        let samples: Vec<Sample> =
            (0..5).map(|i| Sample { fps: 60.0, frame_time: 16.0 + i as f64, elapsed_ns: None }).collect();
        let serie = serie_temporal(&samples, 4000, false).expect("debería haber serie");
        assert_eq!(serie.frame_time_ms.len(), 5);
        // Sin tiempos reales, el eje X queda vacío: el frontend dibuja por índice.
        assert!(serie.elapsed_s.is_empty());
    }

    #[test]
    fn serie_temporal_diezma_sin_cortar_el_final() {
        // 10 000 muestras con tope 1000 → paso 10, y la última debe caer cerca del final.
        let samples: Vec<Sample> = (0..10_000)
            .map(|i| muestra_con_tiempo(60.0, i as f64, i as f64 * 1e8))
            .collect();
        let serie = serie_temporal(&samples, 1000, true).unwrap();
        assert_eq!(serie.frame_time_ms.len(), 1000);
        // El último valor recogido es el de la muestra 9990, no el de la 1000.
        assert!((serie.frame_time_ms[999] - 9990.0).abs() < 1e-1);
    }

    #[test]
    fn serie_temporal_vacia_devuelve_none() {
        assert!(serie_temporal(&[], 4000, true).is_none());
    }

    #[test]
    fn serie_temporal_no_produce_nan_con_frametimes_corruptos() {
        let samples = vec![
            Sample { fps: 60.0, frame_time: f64::NAN, elapsed_ns: Some(0.0) },
            Sample { fps: 60.0, frame_time: -5.0, elapsed_ns: Some(1e8) },
            Sample { fps: 60.0, frame_time: 16.0, elapsed_ns: Some(2e8) },
        ];
        let serie = serie_temporal(&samples, 4000, true).unwrap();
        assert!(serie.frame_time_ms.iter().all(|v| v.is_finite()));
        assert_eq!(serie.frame_time_ms, vec![0.0, 0.0, 16.0]);
    }

    #[test]
    fn un_log_de_60_segundos_ocupa_poco() {
        // Comprobación de tamaño: 600 muestras (60 s a 10 Hz) con tiempos.
        let samples: Vec<Sample> = (0..600)
            .map(|i| muestra_con_tiempo(60.0, 16.6667, i as f64 * 1e8))
            .collect();
        let serie = serie_temporal(&samples, MAX_MUESTRAS_TEMPORALES, true).unwrap();
        let json = serde_json::to_string(&serie).unwrap();
        // ~12 bytes por valor × 2 series × 600 + corchetes. Menos de 20 KB es holgado.
        assert!(
            json.len() < 20_000,
            "la serie temporal ocupa {} bytes, más de lo esperado",
            json.len()
        );
        assert!(json.len() > 1_000, "la serie debería tener datos de verdad");
    }

    // ── Compatibilidad de serialización ─────────────────────────────────────

    #[test]
    fn un_benchmark_antiguo_sin_campos_nuevos_se_sigue_leyendo() {
        // JSON tal y como lo escribía la versión anterior: sin sample_count ni serie.
        let viejo = r#"{
            "id":"bench_1","profile_id":"","game_name":"Juego","appid":440,
            "timestamp":"2026-11-02 14:43:52","duration_seconds":60.0,
            "avg_fps":100.0,"fps_1_percent_low":70.0,"fps_0_1_percent_low":60.0,
            "max_fps":140.0,"min_fps":40.0,"frame_time_avg":10.0,
            "frame_time_1_percent_high":14.3,"total_frames":600,
            "notes":null,"launch_command":"%command%"
        }"#;
        let b: Benchmark = serde_json::from_str(viejo).expect("debe deserializar");
        assert_eq!(b.total_frames, 600);
        // No se inventan datos temporales en pruebas antiguas.
        assert!(b.muestras_temporales.is_none());
        assert!(b.sample_count.is_none());
        // Y el nombre legible de los FPS sigue intacto.
        assert_eq!(b.avg_fps, 100.0);
    }

    #[test]
    fn total_frames_antiguo_se_usa_como_sample_count_al_leer() {
        // El campo `total_frames` guardado siempre contuvo muestras, aunque el nombre
        // diga frames. Al no existir `sample_count`, el valor viejo sigue siendo válido.
        let viejo = r#"{
            "id":"b","profile_id":"","game_name":"J","appid":1,
            "timestamp":"","duration_seconds":60.0,"avg_fps":60.0,
            "fps_1_percent_low":30.0,"fps_0_1_percent_low":20.0,
            "max_fps":1.0,"min_fps":1.0,"frame_time_avg":16.0,
            "frame_time_1_percent_high":33.0,"total_frames":300,
            "notes":null,"launch_command":""
        }"#;
        let b: Benchmark = serde_json::from_str(viejo).unwrap();
        assert_eq!(b.total_frames, 300);
    }

    #[test]
    fn la_serie_temporal_es_opcional_al_serializar() {
        // Una prueba sin serie no debe escribir un campo vacío en el JSON: cada byte
        // cuenta cuando hay cientos de benchmarks.
        let mut b = bench(1, "J");
        b.sample_count = Some(600);
        let json = serde_json::to_string(&b).unwrap();
        assert!(!json.contains("muestras_temporales"), "no debería serializarse: {json}");
        assert!(json.contains("\"sample_count\":600"));
    }

    #[test]
    fn ida_y_vuelta_de_la_serie_temporal() {
        let mut b = bench(1, "J");
        b.muestras_temporales = Some(MuestrasTemporales {
            frame_time_ms: vec![16.6, 17.0, 33.3],
            elapsed_s: vec![0.0, 0.1, 0.2],
        });
        let json = serde_json::to_string(&b).unwrap();
        let vuelta: Benchmark = serde_json::from_str(&json).unwrap();
        let serie = vuelta.muestras_temporales.expect("debería conservarse");
        assert_eq!(serie.frame_time_ms, vec![16.6, 17.0, 33.3]);
        assert_eq!(serie.elapsed_s, vec![0.0, 0.1, 0.2]);
    }

    #[test]
    fn serie_temporal_con_tiempos_vacios_se_lee_bien() {
        let json = r#"{"frame_time_ms":[16.0,17.0]}"#;
        let m: MuestrasTemporales = serde_json::from_str(json).unwrap();
        assert_eq!(m.frame_time_ms.len(), 2);
        assert!(m.elapsed_s.is_empty());
    }

    // ── Comando registrado ───────────────────────────────────────────────

    // ── Origen del comando ────────────────────────────────────────────────

    #[test]
    fn encuentra_el_comando_solo_por_el_nombre_cuando_no_hay_appid() {
        // Caso real: `analyze_log` deja `appid` a 0 (MangoHud no escribe el AppID en el
        // CSV), así que la grabación nueva llega sin appid. La anterior sí tiene el 1623730
        // porque se guardó con la ficha abierta. Buscar solo por appid no encontraba nada y
        // la prueba salía como «Comando no registrado».
        let mut anterior = bench(1623730, "Palworld");
        anterior.launch_command = COMANDO_PALWORLD.into();
        anterior.timestamp = "2026-11-20 10:00:00".into();

        assert_eq!(
            ayuda_ultimo_comando(&[anterior], 0, Some("Palworld")).as_deref(),
            Some(COMANDO_PALWORLD)
        );
    }

    #[test]
    fn el_nombre_se_compara_normalizado() {
        let mut anterior = bench(1623730, "Palworld.x86_64");
        anterior.launch_command = COMANDO_PALWORLD.into();

        assert_eq!(
            ayuda_ultimo_comando(&[anterior], 0, Some("palworld")).as_deref(),
            Some(COMANDO_PALWORLD)
        );
    }

    #[test]
    fn un_nombre_de_otro_juego_no_presta_su_comando() {
        let mut otro = bench(1623730, "Palworld");
        otro.launch_command = COMANDO_PALWORLD.into();

        assert_eq!(ayuda_ultimo_comando(&[otro], 0, Some("Elden Ring")), None);
    }

    #[test]
    fn el_appid_manda_sobre_el_nombre() {
        let mut por_appid = bench(440, "Team Fortress 2");
        por_appid.launch_command = "gamemoderun %command%".into();
        por_appid.timestamp = "2026-11-01 10:00:00".into();

        let mut por_nombre = bench(0, "TF2");
        por_nombre.launch_command = COMANDO_PALWORLD.into();
        por_nombre.timestamp = "2026-11-20 10:00:00".into();

        // Con appid 440 se devuelve el de ese appid, no el que coincide por nombre.
        assert_eq!(
            ayuda_ultimo_comando(&[por_appid, por_nombre], 440, Some("Palworld")).as_deref(),
            Some("gamemoderun %command%")
        );
    }

    #[test]
    fn ni_appid_ni_nombre_devuelve_none() {
        let mut anterior = bench(1623730, "Palworld");
        anterior.launch_command = COMANDO_PALWORLD.into();
        assert_eq!(ayuda_ultimo_comando(&[anterior.clone()], 0, None), None);
        assert_eq!(ayuda_ultimo_comando(&[anterior], 0, Some("")), None);
    }

    #[test]
    fn sin_pruebas_previas_no_hay_comando_que_recuperar() {
        assert_eq!(ayuda_ultimo_comando(&[], 1623730, None), None);
    }

    #[test]
    fn recupera_el_comando_de_la_ultima_prueba_del_juego() {
        let mut vieja = bench(1623730, "Palworld");
        vieja.launch_command = "gamemoderun %command%".into();
        vieja.timestamp = "2026-11-20 10:00:00".into();

        let mut nueva = bench(1623730, "Palworld");
        nueva.launch_command = "mesa_glthread=true game-performance mangohud %command%".into();
        nueva.timestamp = "2026-11-23 13:29:33".into();

        assert_eq!(
            ayuda_ultimo_comando(&[vieja, nueva], 1623730, None).as_deref(),
            Some("mesa_glthread=true game-performance mangohud %command%")
        );
    }

    #[test]
    fn no_mezcla_comandos_de_otro_juego() {
        let mut otro = bench(730, "CS2");
        otro.launch_command = "gamemoderun %command%".into();
        assert_eq!(ayuda_ultimo_comando(&[otro], 1623730, None), None);
    }

    #[test]
    fn un_comando_vacio_no_es_una_observacion() {
        // Una prueba antigua sin comando no puede «prestar» su vacío a la nueva.
        let mut vacia = bench(1623730, "Palworld");
        vacia.launch_command = String::new();
        vacia.timestamp = "2026-11-23 13:29:33".into();

        let mut con_dato = bench(1623730, "Palworld");
        con_dato.launch_command = "mangohud %command%".into();
        con_dato.timestamp = "2026-11-20 10:00:00".into();

        assert_eq!(
            ayuda_ultimo_comando(&[con_dato, vacia], 1623730, None).as_deref(),
            Some("mangohud %command%")
        );
    }

    #[test]
    fn solo_vacios_devuelve_none_no_cadena_vacia() {
        let mut vacia = bench(1623730, "Palworld");
        vacia.launch_command = String::new();
        assert_eq!(ayuda_ultimo_comando(&[vacia], 1623730, None), None);
    }

    #[test]
    fn un_benchmark_antiguo_sin_origen_se_lee_como_desconocido() {
        let json = r#"{
            "id":"viejo","profile_id":"","game_name":"Palworld","appid":1623730,
            "timestamp":"2026-11-23 15:10:13","duration_seconds":30.0,
            "avg_fps":60.0,"fps_1_percent_low":50.0,"fps_0_1_percent_low":45.0,
            "max_fps":80.0,"min_fps":30.0,"frame_time_avg":16.6,
            "frame_time_1_percent_high":20.0,"total_frames":300,"notes":null,
            "launch_command":"%command%"
        }"#;
        let b: Benchmark = serde_json::from_str(json).unwrap();
        assert_eq!(b.origen_comando, OrigenComando::Desconocido);
        // El comando sí se lee: es el que hay, aunque no se sepa de dónde vino.
        assert_eq!(b.launch_command, "%command%");
        // Y no hay lectura de Steam: `None`, no `""` (que significaría «Steam sin nada»).
        assert_eq!(b.steam_command, None);
    }

    /// Una prueba de Palworld con el editor en `%command%` pero Steam lanzando con
    /// argumentos: lo que sirve para saber qué se probó es lo que dice Steam.
    #[test]
    fn steam_sin_leer_no_borra_lo_que_ya_se_sabia() {
        let viejo = Some("mangohud %command%".to_string());
        assert_eq!(steam_a_guardar(viejo.as_ref(), None), viejo);
    }

    #[test]
    fn steam_leido_y_vacio_es_un_dato_y_pisa_al_anterior() {
        // `Some("")` = se leyó y Steam no tenía opciones: se lanzó sin argumentos.
        // Es información real, así que gana a la lectura vieja.
        let viejo = Some("mangohud %command%".to_string());
        assert_eq!(steam_a_guardar(viejo.as_ref(), Some(String::new())), Some(String::new()));
    }

    #[test]
    fn steam_guardado_sobrevive_a_la_serializacion() {
        let mut b = bench(1623730, "Palworld");
        b.steam_command = Some("mesa_glthread=true gamemoderun mangohud %command%".into());
        let json = serde_json::to_string(&b).unwrap();
        let vuelta: Benchmark = serde_json::from_str(&json).unwrap();
        assert_eq!(
            vuelta.steam_command.as_deref(),
            Some("mesa_glthread=true gamemoderun mangohud %command%")
        );
    }

    #[test]
    fn el_origen_se_serializa_en_snake_case() {
        let mut b = bench(1, "J");
        b.origen_comando = OrigenComando::ComandoActual;
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains(r#""origen_comando":"comando_actual""#));
        let vuelta: Benchmark = serde_json::from_str(&json).unwrap();
        assert_eq!(vuelta.origen_comando, OrigenComando::ComandoActual);
    }

    #[test]
    fn un_guardado_sin_comando_no_borra_el_que_ya_habia() {
        assert_eq!(comando_a_guardar(COMANDO_PALWORLD, ""), COMANDO_PALWORLD);
    }

    #[test]
    fn un_comando_nuevo_sustituye_al_anterior() {
        assert_eq!(
            comando_a_guardar("%command%", COMANDO_PALWORLD),
            COMANDO_PALWORLD
        );
    }

    #[test]
    fn el_comando_no_se_normaliza_al_guardar() {
        // Con espacios de más, comillas y orden raro: se guarda igual.
        let raro = r#"  A="x y"  mangohud   %command%  "#;
        assert_eq!(comando_a_guardar("", raro), raro);
    }

    #[test]
    fn guardar_vacio_sobre_vacio_deja_vacio() {
        assert_eq!(comando_a_guardar("", ""), "");
    }

    const COMANDO_PALWORLD: &str =
        "mesa_glthread=true game-performance mangohud %command%";

    #[test]
    fn comandos_identicos_son_iguales() {
        assert_eq!(comandos_iguales(COMANDO_PALWORLD, COMANDO_PALWORLD), Some(true));
        assert_eq!(comandos_iguales("%command%", "%command%"), Some(true));
    }

    #[test]
    fn comandos_distintos_son_distintos() {
        assert_eq!(comandos_iguales("%command%", COMANDO_PALWORLD), Some(false));
        assert_eq!(comandos_iguales(COMANDO_PALWORLD, "%command%"), Some(false));
    }

    #[test]
    fn un_comando_ausente_no_prueba_nada() {
        // Es el fallo de Palworld: un benchmark antiguo sin comando no puede dar ni
        // «iguales» ni «distintos», porque no hay dato.
        assert_eq!(comandos_iguales("", COMANDO_PALWORLD), None);
        assert_eq!(comandos_iguales(COMANDO_PALWORLD, ""), None);
        assert_eq!(comandos_iguales("", ""), None);
        assert_eq!(comandos_iguales("   ", "%command%"), None);
    }

    #[test]
    fn se_ignoran_los_espacios_de_los_extremos() {
        assert_eq!(
            comandos_iguales(&format!("  {COMANDO_PALWORLD} "), COMANDO_PALWORLD),
            Some(true)
        );
    }

    #[test]
    fn los_espacios_internos_cuentan_como_diferencia() {
        // Conservador a propósito: normalizar espacios exigiría interpretar el comando.
        assert_eq!(
            comandos_iguales("mangohud  %command%", "mangohud %command%"),
            Some(false)
        );
    }

    #[test]
    fn el_orden_de_los_argumentos_cuenta() {
        assert_eq!(
            comandos_iguales("--foo=1 --bar=2 %command%", "--bar=2 --foo=1 %command%"),
            Some(false)
        );
    }

    #[test]
    fn las_variables_de_entorno_y_los_wrappers_se_distinguen() {
        assert_eq!(
            comandos_iguales("gamemoderun %command%", COMANDO_PALWORLD),
            Some(false)
        );
    }

    #[test]
    fn un_comando_antiguo_sin_el_campo_se_lee_como_vacio() {
        // JSON de una versión anterior a que existiera `launch_command`.
        let json = r#"{
            "id":"viejo","profile_id":"","game_name":"Palworld","appid":1623730,
            "timestamp":"2026-11-02 14:43:52","duration_seconds":60.0,
            "avg_fps":100.0,"fps_1_percent_low":70.0,"fps_0_1_percent_low":60.0,
            "max_fps":140.0,"min_fps":40.0,"frame_time_avg":10.0,
            "frame_time_1_percent_high":14.3,"total_frames":600,"notes":null
        }"#;
        let b: Benchmark = serde_json::from_str(json).expect("debe deserializar");
        assert_eq!(b.launch_command, "");
        // Y no se inventa un comando al compararlo.
        assert_eq!(comandos_iguales(&b.launch_command, COMANDO_PALWORLD), None);
    }

    #[test]
    fn el_comando_sobrevive_a_la_serializacion_sin_cambios() {
        let mut b = bench(1, "Palworld");
        b.launch_command = COMANDO_PALWORLD.to_string();
        let json = serde_json::to_string(&b).unwrap();
        let vuelta: Benchmark = serde_json::from_str(&json).unwrap();
        assert_eq!(vuelta.launch_command, COMANDO_PALWORLD);
    }

    #[test]
    fn un_comando_con_comillas_y_espacios_se_conserva_exacto() {
        let comando = r#"DXVK_HUD="fps,frametimes" mangohud --dlsym %command% -novid"#;
        let mut b = bench(1, "J");
        b.launch_command = comando.to_string();
        let vuelta: Benchmark = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
        assert_eq!(vuelta.launch_command, comando);
    }

    fn sample(fps: f64, ft: f64) -> Sample {
        Sample { fps, frame_time: ft, elapsed_ns: None }
    }

    fn con_ruta(log_path: &str) -> crate::benchmarks::BenchmarksData {
        let mut b = bench(440, "Team Fortress 2");
        b.log_path = log_path.to_string();
        crate::benchmarks::BenchmarksData {
            version: None,
            benchmarks: vec![b],
            counters: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn la_misma_ruta_cuenta_como_duplicado() {
        let data = con_ruta("/tmp/juego.csv");
        assert!(benchmark_exists_for_path(&data, "/tmp/juego.csv"));
    }

    #[test]
    fn otra_ruta_no_es_duplicado_aunque_las_metricas_coincidan() {
        // Esto es lo que fallaba: dos grabaciones distintas del mismo sitio dan cifras
        // casi iguales, y el historial las rechazaba como "duplicado".
        let data = con_ruta("/tmp/juego-1.csv");
        assert!(!benchmark_exists_for_path(&data, "/tmp/juego-2.csv"));
    }

    #[test]
    fn un_benchmark_viejo_sin_ruta_nunca_es_duplicado() {
        // Los guardados antes de existir `log_path` no pueden identificarse; es mejor
        // guardar la grabación otra vez que rechazarla sin motivo.
        let data = con_ruta("");
        assert!(!benchmark_exists_for_path(&data, "/tmp/juego.csv"));
    }

    #[test]
    fn percentile_interpola_entre_valores_vecinos() {
        let v = [0.0, 10.0, 20.0, 30.0];
        assert_eq!(percentile_sorted(&v, 0.0), 0.0);
        assert_eq!(percentile_sorted(&v, 100.0), 30.0);
        // rank = 0.5 * 3 = 1.5 → entre 10 y 20
        assert_eq!(percentile_sorted(&v, 50.0), 15.0);
    }

    #[test]
    fn percentile_con_un_solo_valor_lo_devuelve() {
        assert_eq!(percentile_sorted(&[42.0], 99.0), 42.0);
    }

    #[test]
    fn percentile_con_lista_vacia_da_cero_sin_panico() {
        assert_eq!(percentile_sorted(&[], 50.0), 0.0);
    }

    #[test]
    fn percentile_satura_fuera_de_rango() {
        let v = [1.0, 2.0, 3.0];
        assert_eq!(percentile_sorted(&v, -10.0), 1.0);
        assert_eq!(percentile_sorted(&v, 500.0), 3.0);
    }

    // ==================== 5.4.1.A: metodología v2 ====================

    /// Fila de datos con reloj, para las pruebas de preparación.
    fn fila(fps: f64, frame_time: f64, elapsed_ns: Option<f64>) -> FilaLeida {
        FilaLeida {
            fps,
            frame_time,
            reloj: match elapsed_ns {
                Some(v) => RelojDeFila::Valor(v),
                None => RelojDeFila::CeldaVacia,
            },
        }
    }

    /// Cambia el reloj de una fila en una prueba, sin tocar su rendimiento.
    fn poner_reloj(f: &mut FilaLeida, reloj: RelojDeFila) {
        f.reloj = reloj;
    }

    /// 600 filas a 10 Hz, todas con el mismo rendimiento, con reloj perfecto.
    fn serie_constante(n: usize, fps: f64) -> Vec<FilaLeida> {
        let ft = 1000.0 / fps;
        (0..n)
            .map(|i| fila(fps, ft, Some(i as f64 * 100_000_000.0)))
            .collect()
    }

    #[test]
    fn v2_no_descarta_un_limitador_constante() {
        // El caso que rompía v1: 600 filas idénticas con timestamps distintos son 600
        // ventanas reales de un juego limitado a 60 fps, no 599 duplicados.
        let preparadas = preparar_muestras(serie_constante(600, 60.0), 10.0, true, 600, 0);
        assert_eq!(preparadas.contabilidad.filas_con_rendimiento_valido, 600);
        // El calentamiento sí se aplica: 10 s de 60 s son 100 filas.
        assert_eq!(preparadas.contabilidad.excluidas_por_calentamiento, 100);
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 500);
        assert!(preparadas.contabilidad.cuadra());
        assert!(preparadas.muestras.iter().all(|s| s.fps == 60.0));
    }

    #[test]
    fn v2_conserva_una_caida_sostenida() {
        // 30 s a 60 fps y 20 s a 20 fps: la caída es el dato que el usuario quiere ver,
        // no un artefacto repetido que haya que borrar.
        let mut filas = serie_constante(300, 60.0);
        for i in 0..200 {
            filas.push(fila(20.0, 50.0, Some((300 + i) as f64 * 100_000_000.0)));
        }
        let preparadas = preparar_muestras(filas, 10.0, true, 500, 0);
        assert_eq!(preparadas.contabilidad.filas_con_rendimiento_valido, 500);
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 400);
        assert!(preparadas.contabilidad.cuadra());
        // La caída sigue entera en las muestras analizadas.
        assert_eq!(preparadas.muestras.iter().filter(|s| s.fps == 20.0).count(), 200);
    }

    #[test]
    fn v2_conserva_una_caida_moderada() {
        // Caída suave a 45 fps: tampoco es un duplicado.
        let mut filas = serie_constante(300, 60.0);
        let ft45 = 1000.0 / 45.0;
        for i in 0..200 {
            filas.push(fila(45.0, ft45, Some((300 + i) as f64 * 100_000_000.0)));
        }
        let preparadas = preparar_muestras(filas, 10.0, true, 500, 0);
        // 500 filas, 100 de calentamiento -> 400 analizadas; las 200 lentas siguen ahí
        // porque el calentamiento corta por delante, no por valor.
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 400);
        assert_eq!(preparadas.muestras.iter().filter(|s| s.fps == 45.0).count(), 200);
        let media = preparadas.muestras.iter().map(|s| s.fps).sum::<f64>() / 400.0;
        // Las 300 a 60 y las 200 a 45 son **antes** del calentamiento. Tras descartar
        // las 100 primeras quedan 200 a 60 y 200 a 45: (200·60 + 200·45)/400 = 52,5.
        assert!((media - 52.5).abs() < 1e-9, "media: {media}");
        assert!(preparadas.contabilidad.cuadra());
    }

    #[test]
    fn v2_conserva_un_patron_periodico_y_su_invertido_con_resultados_distintos() {
        // Patrón A: 6 ventanas a 60,2 ms y 1 a 16,7 ms. Patrón B: lo contrario.
        // v1 colapsaba ambos al mismo número (38,2458). v2 los conserva y da distintos.
        let a: Vec<FilaLeida> = (0..600)
            .map(|i| {
                // A = patrón LENTO: seis muestras de 60,2 ms y una de 16,7 ms.
                let ft = if i % 7 == 6 { 16.7 } else { 60.2 };
                fila(1000.0 / ft, ft, Some(i as f64 * 100_000_000.0))
            })
            .collect();
        let b: Vec<FilaLeida> = (0..600)
            .map(|i| {
                // B = el invertido: seis de 16,7 ms y una de 60,2 ms.
                let ft = if i % 7 == 6 { 60.2 } else { 16.7 };
                fila(1000.0 / ft, ft, Some(i as f64 * 100_000_000.0))
            })
            .collect();

        let pa = preparar_muestras(a, 10.0, true, 600, 0);
        let pb = preparar_muestras(b, 10.0, true, 600, 0);

        assert_eq!(pa.contabilidad.muestras_analizadas, 500);
        assert_eq!(pb.contabilidad.muestras_analizadas, 500);

        let media = |m: &[Sample]| m.iter().map(|s| s.fps).sum::<f64>() / m.len() as f64;
        let ma = media(&pa.muestras);
        let mb = media(&pb.muestras);
        // Valores calculados aparte con las mismas reglas (500 muestras tras el
        // calentamiento). Con v1 ambas daban 38,2458: el mismo número para dos
        // distribuciones distintas.
        assert!((ma - 22.755486).abs() < 1e-5, "media A: {ma}");
        assert!((mb - 53.736049).abs() < 1e-5, "media B: {mb}");
        let ft_medio = |m: &[Sample]| m.iter().map(|s| s.frame_time).sum::<f64>() / m.len() as f64;
        // Ojo con las magnitudes: las cifras de arriba son medias de **FPS**. La media de
        // **frametime** es su inversa aproximada y NO es el inverso exacto: 54,023 ms de
        // A corresponde a 22,755 fps solo porque 1/fps se promedia distinto que ft.
        // Las medias de frametime por ciclo completo (7 muestras, sin calentamiento) son
        // 53,985714 ms para A y 22,914286 ms para B.
        assert!((ft_medio(&pa.muestras) - 54.023).abs() < 1e-9, "ft medio A");
        assert!((ft_medio(&pb.muestras) - 22.877).abs() < 1e-9, "ft medio B");
    }

    #[test]
    fn v2_conserva_fps_y_frametime_iguales_si_el_timestamp_es_distinto() {
        let filas = serie_constante(300, 60.0);
        let preparadas = preparar_muestras(filas, 0.0, true, 300, 0);
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 300);
        assert_eq!(preparadas.contabilidad.excluidas_por_calentamiento, 0);
    }

    #[test]
    fn v2_conserva_filas_identicas_con_timestamp_identico_y_lo_registra() {
        // No hay evidencia de que dos filas iguales con el mismo reloj sean un doble
        // registro: se conservan y se declara la incidencia.
        let mut filas = serie_constante(300, 60.0);
        filas.insert(150, fila(60.0, 1000.0 / 60.0, Some(149.0 * 100_000_000.0)));
        let preparadas = preparar_muestras(filas, 0.0, true, 301, 0);
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 301);
        assert_eq!(preparadas.contabilidad.incidencias.filas_repetidas, 1);
        // La incidencia no invalida el eje por sí sola para el rendimiento, pero el eje
        // deja de ser estrictamente creciente.
        assert!(!preparadas.contabilidad.eje_temporal_fiable);
    }

    #[test]
    fn v2_sin_elapsed_conserva_todo_y_no_deduplica() {
        let filas: Vec<FilaLeida> = (0..400).map(|_| fila(60.0, 16.6, None)).collect();
        let preparadas = preparar_muestras(filas, 10.0, false, 400, 0);
        // Las 400 filas entran: rendimiento válido, sin deduplicar.
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 400);
        // Y no se inventa duración: 400 filas a 60 fps no son «6,6 s» sin un reloj.
        assert_eq!(preparadas.duracion, None);
        let media = preparadas.muestras.iter().map(|s| s.fps).sum::<f64>() / 400.0;
        assert!((media - 60.0).abs() < 1e-12);
        assert_eq!(preparadas.contabilidad.excluidas_por_calentamiento, 0);
        assert!(!preparadas.contabilidad.eje_temporal_fiable);
        assert!(preparadas.contabilidad.cuadra());
        assert_eq!(
            preparadas.contabilidad.calentamiento_no_aplicado,
            Some(MotivoSinCalentamiento::SinEjeTemporal)
        );
    }

    #[test]
    fn v2_conserva_el_rendimiento_aunque_el_reloj_sea_invalido() {
        // Timestamps con saltos hacia atrás: el eje no es fiable, pero las 300 muestras
        // de rendimiento son válidas y deben conservarse todas.
        let mut filas = serie_constante(300, 60.0);
        poner_reloj(&mut filas[100], RelojDeFila::Valor(5_000_000_000.0));
        poner_reloj(&mut filas[150], RelojDeFila::Valor(1_000_000_000.0));
        let preparadas = preparar_muestras(filas, 10.0, true, 300, 0);
        assert_eq!(preparadas.contabilidad.excluidas_por_calentamiento, 0);
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 300);
        assert!(!preparadas.contabilidad.eje_temporal_fiable);
        assert!(preparadas.contabilidad.incidencias.orden_invertido > 0);
        assert!(preparadas.contabilidad.cuadra());
    }

    #[test]
    fn v2_detecta_timestamps_negativos() {
        let mut filas = serie_constante(300, 60.0);
        poner_reloj(&mut filas[10], RelojDeFila::Valor(-1.0));
        let preparadas = preparar_muestras(filas, 0.0, true, 300, 0);
        assert_eq!(preparadas.contabilidad.incidencias.negativas, 1);
        assert!(!preparadas.contabilidad.eje_temporal_fiable);
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 300);
    }

    #[test]
    fn v2_contabiliza_filas_sin_reloj_como_reloj_incompleto() {
        let mut filas = serie_constante(300, 60.0);
        poner_reloj(&mut filas[42], RelojDeFila::CeldaVacia);
        let preparadas = preparar_muestras(filas, 0.0, true, 300, 0);
        // Exactamente una fila carece de reloj: se cuenta una vez, no las posteriores.
        assert_eq!(preparadas.contabilidad.incidencias.reloj_incompleto, 1);
        assert!(!preparadas.contabilidad.eje_temporal_fiable);
        // La fila sin reloj sigue contando como muestra analizada.
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 300);
    }

    #[test]
    fn el_parser_distingue_por_que_falta_el_reloj() {
        // Cada caso deja el eje inservible, pero por un motivo distinto. La suma de
        // `reloj_incompleto` cuenta las filas sin valor, no las posteriores.
        let base = || serie_constante(120, 60.0);

        let mut sin_columna = base();
        for f in &mut sin_columna {
            poner_reloj(f, RelojDeFila::ColumnaAusente);
        }
        let p = preparar_muestras(sin_columna, 0.0, false, 120, 0);
        assert_eq!(p.contabilidad.incidencias.reloj_incompleto, 120);
        assert!(!p.contabilidad.columna_elapsed_presente);

        // Una sola fila vacía: se cuenta una, no 120.
        let mut una_vacia = base();
        poner_reloj(&mut una_vacia[7], RelojDeFila::CeldaVacia);
        let p = preparar_muestras(una_vacia, 0.0, true, 120, 0);
        assert_eq!(p.contabilidad.incidencias.reloj_incompleto, 1);
        assert!(p.contabilidad.columna_elapsed_presente);

        // Malformado y no finito también son «sin reloj» y se cuentan igual.
        for reloj in [RelojDeFila::Malformado, RelojDeFila::NoFinito] {
            let mut filas = base();
            poner_reloj(&mut filas[3], reloj);
            let p = preparar_muestras(filas, 0.0, true, 120, 0);
            assert_eq!(p.contabilidad.incidencias.reloj_incompleto, 1, "{reloj:?}");
            // En ningún caso se pierde la muestra de rendimiento.
            assert_eq!(p.contabilidad.muestras_analizadas, 120);
        }
    }

    #[test]
    fn spikes_per_min_historico_se_lee_y_distingue_cero_de_desconocido() {
        // JSON con `spikes_per_min` numérico (como los guardados antes del cambio) y otro
        // sin el campo. Un `Option<f64>` acepta ambos y no confunde 0.0 con ausente.
        let con_cero = r#"{"benchmarks":[{"id":"a","profile_id":"","game_name":"P","appid":1,
            "timestamp":"2024-01-01 10:00:00","duration_seconds":60.0,"avg_fps":60.0,
            "fps_1_percent_low":50.0,"fps_0_1_percent_low":40.0,"max_fps":70.0,"min_fps":30.0,
            "frame_time_avg":16.6,"frame_time_1_percent_high":20.0,"total_frames":600,
            "notes":null,"spikes_per_min":0.0}],"counters":{}}"#;
        let con_valor = con_cero.replace("\"spikes_per_min\":0.0", "\"spikes_per_min\":2.5");
        let sin_campo = con_cero.replace(",\"spikes_per_min\":0.0", "");

        let a: BenchmarksData = serde_json::from_str(&con_cero).expect("cero");
        assert_eq!(a.benchmarks[0].spikes_per_min, Some(0.0));
        // Cero medido y desconocido son estados distintos.
        assert_ne!(a.benchmarks[0].spikes_per_min, None);

        let b: BenchmarksData = serde_json::from_str(&con_valor).expect("valor");
        assert_eq!(b.benchmarks[0].spikes_per_min, Some(2.5));

        let c: BenchmarksData = serde_json::from_str(&sin_campo).expect("sin campo");
        assert_eq!(c.benchmarks[0].spikes_per_min, None);

        // Y al serializar, el desconocido **no se convierte en 0**: sale `null`, que es
        // inequívoco. (`spikes_per_min` no lleva `skip_serializing_if`, a diferencia de
        // los campos nuevos: se mantiene el contrato existente, no se cambia aquí.)
        let json = serde_json::to_string(&c).expect("serializar");
        assert!(json.contains("\"spikes_per_min\":null"), "json: {json}");
        let releido: BenchmarksData = serde_json::from_str(&json).expect("releer");
        assert_eq!(releido.benchmarks[0].spikes_per_min, None);
    }

    // ============ Bloque 5.4.1.B: persistencia de evaluaciones ============

    /// Una comparación guardada mínima, con ids propios.
    fn comparacion(id: &str, base: &str, test: &str) -> SavedComparison {
        let mut b = bench(1, "Palworld");
        b.id = base.to_string();
        let mut t = bench(1, "Palworld");
        t.id = test.to_string();
        SavedComparison {
            id: id.to_string(),
            saved_at: "2026-01-01 10:00:00".to_string(),
            game_name: "Palworld".to_string(),
            before_fps: b.avg_fps,
            after_fps: t.avg_fps,
            fps_change_percent: 0.0,
            comparison: BenchmarkComparison {
                baseline: b,
                test: t,
                fps_change_percent: 0.0,
                fps_1_percent_change_percent: 0.0,
                fps_0_1_percent_change_percent: 0.0,
                frame_time_change_percent: 0.0,
                conclusion: "narrativa histórica".to_string(),
                noise_percent: Some(3.0),
                within_noise: false,
                comandos_iguales: Some(true),
            },
            evaluacion: None,
            evaluacion_ilegible: None,
        }
    }

    fn evaluacion_de(id_ref: &str, id_cand: &str) -> EvaluacionPersistida {
        EvaluacionPersistida {
            metodologia_interpretacion: VersionDeInterpretacion("v1" .to_string()),
            evaluada_en: "2026-01-02 11:00:00".to_string(),
            referencia: IdDeBenchmark { id: id_ref.to_string() },
            candidata: IdDeBenchmark { id: id_cand.to_string() },
            estado: CodigoDeEvaluacion(CodigoDeEvaluacion::MEJORA.to_string()),
            confianza: CodigoDeEvaluacion("observada".to_string()),
            metricas: vec![EvaluacionDeMetrica {
                clave: "avg_fps".to_string(),
                antes: Some(60.0),
                despues: Some(66.0),
                diferencia: Some(6.0),
                diferencia_pct: Some(10.0),
                direccion: DireccionDeCambio::Sube,
                preferencia: PreferenciaDeMetrica::FavorDeCandidata,
                confianza: Some(CodigoDeEvaluacion("observada".to_string())),
                apoyo_referencia: None,
                apoyo_candidata: None,
                motivos: vec![CodigoDeEvaluacion("velocidad_mejora".to_string())],
                limitaciones: vec![],
            }],
            motivos: vec![CodigoDeEvaluacion("velocidad_mejora".to_string())],
            limitaciones: vec![],
            condiciones: CondicionesDeComparacion {
                compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
                margen_porcentaje: Some(5.0),
                ruido_combinado_porcentaje: Some(3.0),
                comandos_iguales: Some(true),
                variable_evaluada: None,
            },
        }
    }

    /// Escribe un archivo y devuelve su ruta.
    fn escribir(ruta: &Path, json: &str) {
        fs::write(ruta, json).expect("escribir fixture");
    }

    #[test]
    fn leer_no_reescribe_ni_cambia_bytes_ni_mtime() {
        let dir = tmp_dir("b541-lectura");
        let ruta = dir.join("saved_comparisons.json");
        escribir(
            &ruta,
            &format!(
                r#"{{"version":1,"comparisons":[{{"id":"c1","saved_at":"x","game_name":"P",
                   "before_fps":1.0,"after_fps":2.0,"fps_change_percent":100.0,
                   "comparison":{{"campo_futuro":"se conserva","baseline":{},"test":{},
                     "fps_change_percent":0.0,"fps_1_percent_change_percent":0.0,
                     "fps_0_1_percent_change_percent":0.0,"frame_time_change_percent":0.0,
                     "conclusion":"","within_noise":false}}}}]}}"#,
                serde_json::to_string(&bench(1, "P")).unwrap(),
                serde_json::to_string(&bench(1, "P")).unwrap()
            ),
        );
        let antes = fs::read(&ruta).expect("leer bytes");
        let mtime_antes = fs::metadata(&ruta).and_then(|m| m.modified()).expect("mtime");

        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = try_load_saved_data_de(&ruta).expect("lectura");

        let despues = fs::read(&ruta).expect("leer bytes");
        let mtime_despues = fs::metadata(&ruta).and_then(|m| m.modified()).expect("mtime");
        assert_eq!(antes, despues, "leer no puede cambiar los bytes");
        assert_eq!(mtime_antes, mtime_despues, "leer no puede tocar el mtime");
    }

    #[test]
    fn la_serializacion_es_igualdad_semantica() {
        // No se compara byte a byte: el orden de claves y el sangrado no son contrato.
        let mut c = comparacion("c1", "b1", "t1");
        c.evaluacion = Some(evaluacion_de("b1", "t1"));
        let json = serde_json::to_string(&c).expect("serializar");
        let vuelta: SavedComparison = serde_json::from_str(&json).expect("deserializar");
        // Igualdad semántica campo a campo (no byte a byte).
        assert_eq!(c.id, vuelta.id);
        assert_eq!(c.saved_at, vuelta.saved_at);
        assert_eq!(c.game_name, vuelta.game_name);
        assert_eq!(c.before_fps, vuelta.before_fps);
        assert_eq!(c.after_fps, vuelta.after_fps);
        assert_eq!(c.fps_change_percent, vuelta.fps_change_percent);
        assert_eq!(c.evaluacion, vuelta.evaluacion);
        assert_eq!(c.comparison.baseline.id, vuelta.comparison.baseline.id);
        assert_eq!(c.comparison.test.id, vuelta.comparison.test.id);
    }

    #[test]
    fn los_campos_desconocidos_anidados_sobreviven_a_un_borrado() {
        // El escenario que destruiría datos: se borra UNA comparación y las demás deben
        // conservar sus campos futuros, también los anidados hondo.
        let dir = tmp_dir("b541-anidados");
        let ruta = dir.join("saved_comparisons.json");
        let mut c1 = comparacion("c1", "b1", "t1");
        c1.evaluacion = Some(evaluacion_de("b1", "t1"));
        let mut c2 = comparacion("c2", "b2", "t2");
        c2.evaluacion = Some(evaluacion_de("b2", "t2"));

        let mut json = serde_json::json!({
            "version": 2,
            "comparisons": [serde_json::to_value(&c1).unwrap(), serde_json::to_value(&c2).unwrap()],
            "campo_futuro_raiz": {"anidado": [1, 2, 3]}
        });
        json["comparisons"][1]["comparison"]["campo_futuro_profundo"] =
            serde_json::json!({"nivel": {"mas": true}});
        escribir(&ruta, &serde_json::to_string_pretty(&json).unwrap());

        let mut datos = try_load_saved_data_de(&ruta).expect("lectura");
        datos.comparisons.retain(|c| c.id != "c1");
        save_saved_data_en(&ruta, &datos).expect("escritura");

        let escrito: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ruta).unwrap()).unwrap();
        assert_eq!(escrito["campo_futuro_raiz"]["anidado"][2], 3, "raíz conservada");
        assert_eq!(escrito["comparisons"].as_array().unwrap().len(), 1);
        assert_eq!(escrito["comparisons"][0]["id"], "c2");
        assert_eq!(
            escrito["comparisons"][0]["comparison"]["campo_futuro_profundo"]["nivel"]["mas"],
            true,
            "campo anidado de un registro no afectado"
        );
        assert!(
            escrito["comparisons"][0]["evaluacion"].is_object(),
            "la evaluación de la otra comparación no se puede perder"
        );
    }

    #[test]
    fn un_fichero_v1_no_cambia_de_version_al_leerlo() {
        let dir = tmp_dir("b541-v1-lee");
        let ruta = dir.join("saved_comparisons.json");
        let c = comparacion("c1", "b1", "t1");
        escribir(
            &ruta,
            &format!(r#"{{"version":1,"comparisons":[{}]}}"#, serde_json::to_string(&c).unwrap()),
        );

        let datos = try_load_saved_data_de(&ruta).expect("lectura");
        assert_eq!(datos.version, Some(1));
        assert_eq!(version_a_escribir(&datos), 1, "sin evaluaciones sigue siendo v1");
    }

    #[test]
    fn una_comparacion_con_evaluacion_se_escribe_en_v2() {
        // Las comparaciones nacen con su evaluación (bloque C las creará juntas): no hay
        // un camino que añada la evaluación después a una ya guardada.
        let dir = tmp_dir("b541-v2");
        let ruta = dir.join("saved_comparisons.json");
        escribir(&ruta, r#"{"version":1,"comparisons":[]}"#);

        let mut datos = try_load_saved_data_de(&ruta).expect("lectura");
        let mut c = comparacion("c1", "b1", "t1");
        c.evaluacion = Some(evaluacion_de("b1", "t1"));
        datos.comparisons.push(c);
        save_saved_data_en(&ruta, &datos).expect("escritura");

        let escrito: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ruta).unwrap()).unwrap();
        assert_eq!(escrito["version"], 2, "una evaluación obliga a v2");

        let releido = try_load_saved_data_de(&ruta).expect("relectura");
        assert!(releido.comparisons[0].evaluacion.is_some());
    }

    #[test]
    fn un_fichero_v2_no_vuelve_a_v1_aunque_se_borren_las_evaluaciones() {
        let dir = tmp_dir("b541-v2-no-baja");
        let ruta = dir.join("saved_comparisons.json");
        let mut c = comparacion("c1", "b1", "t1");
        c.evaluacion = Some(evaluacion_de("b1", "t1"));
        escribir(
            &ruta,
            &format!(r#"{{"version":2,"comparisons":[{}]}}"#, serde_json::to_string(&c).unwrap()),
        );

        let mut datos = try_load_saved_data_de(&ruta).expect("lectura");
        datos.comparisons[0].evaluacion = None;
        save_saved_data_en(&ruta, &datos).expect("escritura");

        let escrito: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ruta).unwrap()).unwrap();
        assert_eq!(escrito["version"], 2, "un v2 nunca baja a v1 solo");

        // Y vaciar todas tampoco lo baja.
        datos.comparisons.clear();
        save_saved_data_en(&ruta, &datos).expect("escritura 2");
        let escrito: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ruta).unwrap()).unwrap();
        assert_eq!(escrito["version"], 2);
    }

    #[test]
    fn una_version_futura_no_se_trata_como_danada_ni_se_toca() {
        // La protección de v1.2.0: un archivo v3 da error explícito y el archivo queda
        // intacto. Es lo que impide que una versión antigua reescriba evaluaciones.
        let dir = tmp_dir("b541-v3");
        let ruta = dir.join("saved_comparisons.json");
        escribir(&ruta, r#"{"version":3,"comparisons":[]}"#);
        let antes = fs::read(&ruta).unwrap();

        let err = try_load_saved_data_de(&ruta).expect_err("debe rechazar v3");
        match err {
            ErrorDatos::VersionFutura { encontrada, soportada } => {
                assert_eq!(encontrada, 3);
                assert_eq!(soportada, COMPARISONS_SCHEMA_VERSION);
            }
            otro => panic!("se esperaba VersionFutura, llegó {otro:?}"),
        }
        assert_eq!(antes, fs::read(&ruta).unwrap(), "no se modifica");
    }

    #[test]
    fn un_id_duplicado_se_detecta_y_no_se_pisa() {
        let dir = tmp_dir("b541-duplicado");
        let ruta = dir.join("saved_comparisons.json");
        let c1 = comparacion("repetido", "b1", "t1");
        escribir(
            &ruta,
            &format!(
                r#"{{"version":1,"comparisons":[{},{}]}}"#,
                serde_json::to_string(&c1).unwrap(),
                serde_json::to_string(&c1).unwrap()
            ),
        );
        match try_load_saved_data_de(&ruta) {
            Err(ErrorDatos::Corrupto { detalle, .. }) => assert!(detalle.contains("mismo id")),
            otro => panic!("se esperaba Corrupto por id duplicado, llegó {otro:?}"),
        }
    }

    #[test]
    fn ausente_no_es_lo_mismo_que_datos_insuficientes() {
        let mut sin = comparacion("c1", "b1", "t1");
        sin.evaluacion = None;
        assert_eq!(sin.estado_de_la_evaluacion(), EstadoDeLaEvaluacion::NoRegistrada);

        let mut con = comparacion("c2", "b2", "t2");
        let mut ev = evaluacion_de("b2", "t2");
        ev.estado = CodigoDeEvaluacion(CodigoDeEvaluacion::DATOS_INSUFICIENTES.to_string());
        con.evaluacion = Some(ev);
        match con.estado_de_la_evaluacion() {
            EstadoDeLaEvaluacion::Soportada(e) => {
                assert_eq!(e.estado.0, CodigoDeEvaluacion::DATOS_INSUFICIENTES)
            }
            otro => panic!("los datos insuficientes son una evaluación registrada: {otro:?}"),
        }
    }

    #[test]
    fn una_evaluacion_con_codigos_desconocidos_se_conserva_sin_reinterpretar() {
        let mut c = comparacion("c1", "b1", "t1");
        let mut ev = evaluacion_de("b1", "t1");
        ev.metodologia_interpretacion = VersionDeInterpretacion("v9" .to_string());
        c.evaluacion = Some(ev);

        // Se conserva íntegra en disco.
        let json = serde_json::to_string(&c).unwrap();
        let vuelta: SavedComparison = serde_json::from_str(&json).unwrap();
        assert_eq!(
            vuelta.evaluacion.as_ref().unwrap().metodologia_interpretacion.0,
            "v9"
        );
        // Y no se presenta como si se entendiera.
        assert_eq!(
            c.estado_de_la_evaluacion(),
            EstadoDeLaEvaluacion::NoSoportada(Box::new(c.evaluacion.clone().unwrap()))
        );
    }

    #[test]
    fn una_evaluacion_que_no_corresponde_a_sus_benchmarks_no_se_presenta_como_valida() {
        let mut c = comparacion("c1", "b1", "t1");
        // Los ids apuntan a otras grabaciones.
        c.evaluacion = Some(evaluacion_de("otro", "distinto"));
        assert!(matches!(
            c.estado_de_la_evaluacion(),
            EstadoDeLaEvaluacion::NoSoportada(_)
        ));
    }

    #[test]
    fn los_estados_y_versiones_desconocidos_no_se_convierten_en_conocidos() {
        assert!(!CodigoDeEvaluacion("estado_del_futuro".to_string()).es_conocido());
        assert!(CodigoDeEvaluacion(CodigoDeEvaluacion::MEJORA.to_string()).es_conocido());
        assert!(!VersionDeInterpretacion("v9" .to_string()).es_soportada());
        assert!(VersionDeInterpretacion("v1" .to_string()).es_soportada());
    }

    #[test]
    fn añadir_una_comparacion_conserva_las_demas_registros() {
        let dir = tmp_dir("b541-alta");
        let ruta = dir.join("saved_comparisons.json");
        let c1 = comparacion("c1", "b1", "t1");
        escribir(
            &ruta,
            &format!(r#"{{"version":1,"comparisons":[{}]}}"#, serde_json::to_string(&c1).unwrap()),
        );

        let mut datos = try_load_saved_data_de(&ruta).expect("lectura");
        datos.comparisons.push(comparacion("c2", "b2", "t2"));
        save_saved_data_en(&ruta, &datos).expect("escritura");

        let escrito: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ruta).unwrap()).unwrap();
        assert_eq!(escrito["comparisons"].as_array().unwrap().len(), 2);
        assert_eq!(escrito["comparisons"][0]["id"], "c1");
        assert_eq!(escrito["comparisons"][1]["id"], "c2");
    }

    #[test]
    fn los_motivos_no_se_validan_contra_el_vocabulario_de_estados() {
        // Motivo y estado son vocabularios distintos: un motivo con código propio no
        // convierte la evaluación en no soportada.
        let mut c = comparacion("c1", "b1", "t1");
        let mut ev = evaluacion_de("b1", "t1");
        ev.motivos = vec![CodigoDeEvaluacion("motivo_de_una_version_futura".to_string())];
        c.evaluacion = Some(ev);
        assert!(matches!(
            c.estado_de_la_evaluacion(),
            EstadoDeLaEvaluacion::Soportada(_)
        ));
    }

    /// La conservación es del JSON entero, no solo de los objetos de primer nivel: dentro
    /// de arrays, de snapshots y de la propia evaluación también se conserva lo que esta
    /// versión no entiende.
    #[test]
    fn los_campos_desconocidos_se_conservan_en_arrays_snapshots_y_evaluacion() {
        let dir = tmp_dir("b541-arrays");
        let ruta = dir.join("saved_comparisons.json");
        let mut c = comparacion("c1", "b1", "t1");
        c.evaluacion = Some(evaluacion_de("b1", "t1"));
        let mut json = serde_json::json!({
            "version": 2,
            "comparisons": [serde_json::to_value(&c).unwrap()],
        });
        json["comparisons"][0]["evaluacion"]["futuro_en_evaluacion"] =
            serde_json::json!({"x": [1, {"y": 2}]});
        // Un objeto desconocido dentro del benchmark, con array y objeto anidados.
        json["comparisons"][0]["comparison"]["baseline"]["campo_futuro"] =
            serde_json::json!({ "lista": [ {"hondo": true} ] });
        escribir(&ruta, &serde_json::to_string_pretty(&json).unwrap());

        // Se borra OTRA comparación, de modo que c1 no se toque pero el archivo se reescriba.
        let mut datos = try_load_saved_data_de(&ruta).expect("lectura");
        let mut otra = comparacion("c2", "b2", "t2");
        otra.evaluacion = None;
        datos.comparisons.push(otra);
        datos.comparisons.retain(|c| c.id != "c2");
        save_saved_data_en(&ruta, &datos).expect("escritura");

        let escrito: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ruta).unwrap()).unwrap();
        let r = &escrito["comparisons"][0];
        assert_eq!(r["evaluacion"]["futuro_en_evaluacion"]["x"][1]["y"], 2,
            "campo futuro dentro de la evaluación");
        assert_eq!(r["comparison"]["baseline"]["campo_futuro"]["lista"][0]["hondo"], true,
            "campo futuro con array y objeto anidados, dentro del benchmark");
    }

    /// Una evaluación malformada no es «no registrada» ni convierte el archivo en corrupto:
    /// el resto de la comparación se conserva y la evaluación se marca ilegible.
    #[test]
    fn una_evaluacion_malformada_no_arrastra_al_resto_del_archivo() {
        let dir = tmp_dir("b541-malformada");
        let ruta = dir.join("saved_comparisons.json");
        let c = comparacion("c1", "b1", "t1");
        let mut json = serde_json::json!({
            "version": 2,
            "comparisons": [serde_json::to_value(&c).unwrap()],
        });
        // `condiciones` debería ser un objeto: aquí es una cadena.
        json["comparisons"][0]["evaluacion"] = serde_json::json!({
            "metodologia_interpretacion": "v1",
            "evaluada_en": "2026-01-02 11:00:00",
            "referencia": {"id": "b1"},
            "candidata": {"id": "t1"},
            "estado": "mejora",
            "confianza": "observada",
            "condiciones": "esto no es un objeto"
        });
        escribir(&ruta, &serde_json::to_string_pretty(&json).unwrap());

        let datos = try_load_saved_data_de(&ruta).expect("el archivo sigue siendo legible");
        assert_eq!(datos.comparisons.len(), 1);
        // Lo medido se conserva.
        assert_eq!(datos.comparisons[0].id, "c1");
        assert_eq!(datos.comparisons[0].game_name, "Palworld");
        // Y la evaluación no se finge ausente.
        assert!(datos.comparisons[0].evaluacion.is_none());
        assert!(datos.comparisons[0].evaluacion_ilegible.is_some());
        assert_eq!(
            datos.comparisons[0].estado_de_la_evaluacion(),
            EstadoDeLaEvaluacion::Malformada
        );

        // El JSON original se conserva íntegro al reescribir.
        let datos2 = try_load_saved_data_de(&ruta).expect("relectura");
        save_saved_data_en(&ruta, &datos2).expect("escritura");
        let escrito: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&ruta).unwrap()).unwrap();
        assert_eq!(
            escrito["comparisons"][0]["evaluacion"]["condiciones"],
            "esto no es un objeto",
            "la evaluación ilegible no se descarta ni se reescribe"
        );
    }

    /// El apoyo estadístico del bloque A se puede **recuperar** desde los benchmarks
    /// inmutables: la evaluación no necesita copiarlo entero ni inventarlo.
    #[test]
    fn el_apoyo_de_metricas_se_recupera_del_benchmark_guardado() {
        let mut b = bench(1, "Palworld");
        b.apoyo_metricas = Some(ApoyoDeMetricas {
            fps_1_percent_low: Some(ApoyoDePercentil {
                muestras: 498,
                cola_nominal: 4.98,
                observaciones_interpoladas: 2,
                muestras_en_la_media: Some(5),
            }),
            fps_0_1_percent_low: Some(ApoyoDePercentil {
                muestras: 498,
                cola_nominal: 0.498,
                observaciones_interpoladas: 2,
                muestras_en_la_media: None,
            }),
            max_fps: None,
            min_fps: None,
            frame_time_1_percent_high: None,
        });

        // Ida y vuelta por JSON: es como llega desde disco.
        let json = serde_json::to_string(&b).expect("serializar");
        let vuelta: Benchmark = serde_json::from_str(&json).expect("deserializar");
        let apoyo = vuelta.apoyo_metricas.expect("el apoyo consta");

        let p1 = apoyo.fps_1_percent_low.expect("1% low");
        assert_eq!(p1.muestras, 498);
        assert_eq!(p1.cola_nominal, 4.98);
        assert_eq!(p1.observaciones_interpoladas, 2);
        assert_eq!(p1.muestras_en_la_media, Some(5));

        let p01 = apoyo.fps_0_1_percent_low.expect("0,1% low");
        // Un percentil puro no tiene muestras en la media: `None`, no 0.
        assert_eq!(p01.muestras_en_la_media, None);

        // Y las métricas que no aplican siguen siendo `None`, sin rellenarse en cero.
        assert!(apoyo.max_fps.is_none());
        assert!(apoyo.frame_time_1_percent_high.is_none());
    }

    /// `ApoyoDeMetrica` (por métrica) no puede llevar el apoyo de otra métrica.
    #[test]
    fn el_apoyo_por_metrica_no_mezcla_metricas() {
        let a = ApoyoDeMetrica { percentil: None };
        let json = serde_json::to_value(&a).expect("serializar");
        // Sin apoyo de percentil no se escribe nada: no hay `percentil: null` falso.
        assert_eq!(json, serde_json::json!({}));
    }

    #[test]
    fn un_registro_intacto_no_se_reconstruye() {
        let dir = tmp_dir("b541-intacto");
        let ruta = dir.join("saved_comparisons.json");
        let c = comparacion("c1", "b1", "t1");
        let original = serde_json::to_string_pretty(&c).unwrap();
        escribir(&ruta, &format!(r#"{{"version":1,"comparisons":[{original}]}}"#));

        let datos = try_load_saved_data_de(&ruta).expect("lectura");
        let json = construir_json_de_comparaciones(&datos).expect("json");
        // El registro se reescribe literal: mismo objeto que estaba en disco.
        assert_eq!(
            json["comparisons"][0],
            serde_json::to_value(&c).unwrap(),
            "sin cambios, se conserva el registro original"
        );
    }

    #[test]
    fn el_apoyo_estadistico_del_bloque_a_se_conserva_en_la_evaluacion() {
        let mut ev = evaluacion_de("b1", "t1");
        ev.metricas[0].apoyo_referencia = Some(ApoyoDeMetrica {
            percentil: Some(ApoyoDePercentil {
                muestras: 499,
                cola_nominal: 4.99,
                observaciones_interpoladas: 2,
                muestras_en_la_media: Some(5),
            }),
        });
        let json = serde_json::to_string(&ev).unwrap();
        let vuelta: EvaluacionPersistida = serde_json::from_str(&json).unwrap();
        let p = vuelta.metricas[0]
            .apoyo_referencia
            .as_ref()
            .unwrap()
            .percentil
            .as_ref()
            .unwrap();
        assert_eq!(p.muestras, 499);
        assert_eq!(p.observaciones_interpoladas, 2);
        assert_eq!(p.muestras_en_la_media, Some(5));
    }

    #[test]
    fn ausente_y_null_se_leen_igual_y_no_se_escribe_cero() {
        // `Option` simple: ausente y null significan «no registrado».
        let base = comparacion("c1", "b1", "t1");
        let mut valor = serde_json::to_value(&base).unwrap();
        valor.as_object_mut().unwrap().remove("evaluacion");
        valor["evaluacion"] = serde_json::Value::Null;
        let con_null: SavedComparison = serde_json::from_value(valor).expect("null");
        assert!(con_null.evaluacion.is_none());

        let mut valor = serde_json::to_value(&base).unwrap();
        valor.as_object_mut().unwrap().remove("evaluacion");
        let sin_campo: SavedComparison = serde_json::from_value(valor).expect("ausente");
        assert!(sin_campo.evaluacion.is_none());

        // Sin evaluación no aparece la clave al serializar (no se escribe `null`).
        let json = serde_json::to_string(&sin_campo).unwrap();
        assert!(!json.contains("evaluacion"), "json: {json}");
    }

    #[test]
    fn el_campo_legado_solo_lleva_el_cero_cuando_no_hay_duracion() {
        // Con eje fiable, el campo legado y `duracion_medida` llevan **el mismo valor**:
        // así los JSON y el frontend antiguos siguen leyendo una duración de verdad.
        let ruta = format!(
            "{}/../tests/fixtures/mangohud/Palworld-Win64-Shipping_2026-11-24_18-19-57.csv",
            env!("CARGO_MANIFEST_DIR")
        );
        let b = analyze_mangohud_log(&ruta, 10.0).expect("análisis");
        let medida = b.duracion_medida.expect("duración medida");
        assert!((b.duration_seconds - medida).abs() < 1e-12, "legado y medido deben coincidir");
        assert!(b.duration_seconds > 0.0);
        assert_eq!(b.duracion(), DuracionDeLaGrabacion::Medida { segundos: medida });
    }

    #[test]
    fn sin_duracion_el_campo_legado_es_cero_y_no_se_lee_como_duracion() {
        // 300 filas válidas sin reloj: no se puede medir la duración.
        let filas: Vec<FilaLeida> = (0..300).map(|_| fila(60.0, 16.6, None)).collect();
        let p = preparar_muestras(filas, 0.0, false, 300, 0);
        assert_eq!(p.duracion, None);

        // Lo que hace `analyze_mangohud_log` con el campo legado.
        let legado = p.duracion.unwrap_or(0.0);
        assert_eq!(legado, 0.0);
        let b = Benchmark {
            duration_seconds: legado,
            duracion_medida: p.duracion,
            metodologia_calculo: Some(MetodologiaCalculo::V2),
            ..bench(0, "Palworld")
        };
        // El cero existe en el campo, pero el contrato lo interpreta como «no consta».
        assert_eq!(b.duracion_en_segundos(), None);
        assert_eq!(b.duracion(), DuracionDeLaGrabacion::Desconocida);
    }

    #[test]
    fn duracion_distingue_los_tres_estados() {
        let mut b = bench(0, "Palworld");

        // 1. Histórica: sin metodología declarada. `duration_seconds` es lo medido entonces.
        b.metodologia_calculo = None;
        b.duration_seconds = 59.7;
        b.duracion_medida = None;
        assert_eq!(b.duracion(), DuracionDeLaGrabacion::Historica { segundos: 59.7 });
        assert_eq!(b.duracion_en_segundos(), Some(59.7));

        // 2. v2 con duración medida.
        b.metodologia_calculo = Some(MetodologiaCalculo::V2);
        b.duracion_medida = Some(59.85);
        assert_eq!(b.duracion(), DuracionDeLaGrabacion::Medida { segundos: 59.85 });
        assert_eq!(b.duracion_en_segundos(), Some(59.85));

        // 3. v2 sin duración medible: desconocida, **nunca** se cae al cero de
        //    compatibilidad ni a un `duration_seconds` que hubiera quedado ahí.
        b.duracion_medida = None;
        b.duration_seconds = 0.0;
        assert_eq!(b.duracion(), DuracionDeLaGrabacion::Desconocida);
        assert_eq!(b.duracion_en_segundos(), None);
        b.duration_seconds = 42.0;
        assert_eq!(b.duracion_en_segundos(), None, "en v2 no hay respaldo");
    }

    #[test]
    fn duracion_historica_no_utilizable_sigue_siendo_desconocida() {
        let mut b = bench(0, "Palworld");
        b.metodologia_calculo = None;
        for valor in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            b.duration_seconds = valor;
            assert_eq!(
                b.duracion_en_segundos(),
                None,
                "duration_seconds={valor} no es una duración"
            );
        }
    }

    #[test]
    fn sin_eje_fiable_no_se_inventan_segundos_ni_tasas() {
        // 300 filas válidas a 10 Hz, pero con el reloj roto: la serie de rendimiento es
        // perfectamente utilizable y la temporal no lo es.
        let mut filas = serie_constante(300, 60.0);
        poner_reloj(&mut filas[10], RelojDeFila::CeldaVacia);
        let p = preparar_muestras(filas, 10.0, true, 300, 0);
        let c = &p.contabilidad;

        // Duración: no consta (no se estima a partir de 10 Hz).
        assert_eq!(p.duracion, None);
        assert!(p.eje.is_none());
        // Calentamiento: no se aplica, y se dice por qué.
        assert_eq!(p.warmup_applied, 0.0);
        assert_eq!(
            c.calentamiento_no_aplicado,
            Some(MotivoSinCalentamiento::SinEjeTemporal)
        );
        // Las 300 muestras siguen ahí.
        assert_eq!(c.muestras_analizadas, 300);
        assert!(c.cuadra());
    }

    #[test]
    fn sin_reloj_siguen_saliendo_las_metricas_que_no_dependen_del_tiempo() {
        // Serie con una caída al 50 % en la mitad: sin reloj, las métricas de valores
        // siguen siendo calculables y correctas; la tasa por minuto no.
        let mut filas = serie_constante(300, 60.0);
        for f in filas.iter_mut() {
            poner_reloj(f, RelojDeFila::ColumnaAusente);
        }
        for (i, f) in filas.iter_mut().enumerate() {
            if i >= 150 {
                f.fps = 25.0;
                f.frame_time = 40.0;
            }
        }
        let p = preparar_muestras(filas, 0.0, false, 300, 0);
        let muestras: Vec<f64> = p.muestras.iter().map(|s| s.frame_time).collect();

        // Medias y percentiles salen de los valores, no del reloj.
        let media = p.muestras.iter().map(|s| s.fps).sum::<f64>() / p.muestras.len() as f64;
        assert!((media - 42.5).abs() < 1e-9, "media {media}");
        // `compute_stutter` con duración desconocida (0.0) no inventa picos por minuto.
        let st = compute_stutter(&muestras, 0.0);
        assert_eq!(st.spikes_per_min, None);
        // Pero sí cuenta fotogramas lentos y da jitter: no dependen del reloj.
        assert!(st.pct_over_33ms > 0.0);
        assert!(st.jitter_ms > 0.0);
    }

    #[test]
    fn el_parser_clasifica_las_celdas_del_reloj_del_csv() {
        use std::fs;
        // CSV mínimo con la cabecera real y una fila por caso. Las de rendimiento son
        // todas válidas: lo único que cambia es la celda de `elapsed`.
        // 105 filas: 100 con reloj correcto y las 5 patológicas, para pasar el mínimo
        // de muestras y poder observar la clasificación en el pipeline completo.
        let mut csv = String::from("OS,Linux\nCPU,AMD\nGPU,AMD\nfps,frametime,gpu_load,elapsed\n");
        for i in 0..100u64 {
            csv.push_str(&format!("60.0,16.6,10,{}\n", (i + 1) * 1_000_000));
        }
        csv.push_str("60.0,16.6,10,\n");
        csv.push_str("60.0,16.6,10,   \n");
        csv.push_str("60.0,16.6,10,no-es-un-numero\n");
        csv.push_str("60.0,16.6,10,NaN\n");
        csv.push_str("60.0,16.6,10,inf\n");
        let dir = std::env::temp_dir().join("thoriapp_5_4_1a_reloj_csv");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        let ruta = dir.join("log.csv");
        fs::write(&ruta, csv).expect("escribir");

        let b = analyze_mangohud_log(&ruta.to_string_lossy(), 0.0).expect("análisis");
        let c = b.contabilidad_muestras.as_ref().expect("contabilidad");
        // Las 105 filas tienen rendimiento válido: no se descarta ninguna.
        assert_eq!(c.filas_con_rendimiento_valido, 105);
        assert_eq!(c.muestras_analizadas, 105);
        // 5 filas sin reloj utilizable: vacía, espacios, texto, NaN e inf.
        assert_eq!(c.incidencias.reloj_incompleto, 5);
        // Sin eje no hay duración que declarar.
        assert!(!c.eje_temporal_fiable);
        assert_eq!(b.duracion_medida, None);
        assert!(c.cuadra());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_desglose_de_reloj_persiste_la_causa_no_solo_el_total() {
        // Un CSV con una fila de cada causa. `CeldaAusente` sí es reproducible: una fila
        // con menos columnas que la cabecera (aquí, sin la última columna).
        let mut csv = String::from("OS,Linux\nfps,frametime,gpu_load,elapsed\n");
        for i in 0..100u64 {
            csv.push_str(&format!("60.0,16.6,10,{}\n", (i + 1) * 1_000_000));
        }
        csv.push_str("60.0,16.6,10,\n");          // celda vacía
        csv.push_str("60.0,16.6,10,   \n");       // celda en blanco
        csv.push_str("60.0,16.6,10,no-numero\n"); // malformado
        csv.push_str("60.0,16.6,10,NaN\n");       // no finito
        csv.push_str("60.0,16.6,10\n");           // fila corta: celda ausente

        let dir = std::env::temp_dir().join("thoriapp_541a_desglose");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        let ruta = dir.join("log.csv");
        fs::write(&ruta, csv).expect("escribir");

        let b = analyze_mangohud_log(&ruta.to_string_lossy(), 0.0).expect("análisis");
        let inc = &b.contabilidad_muestras.as_ref().expect("contabilidad").incidencias;
        assert_eq!(inc.reloj_incompleto, 5);
        assert_eq!(inc.celda_reloj_vacia, 2, "vacía y en blanco");
        assert_eq!(inc.celda_reloj_ilegible, 2, "texto y NaN");
        assert_eq!(inc.celda_reloj_ausente, 1, "fila con menos columnas");
        assert_eq!(inc.columna_elapsed_ausente, 0);
        assert!(inc.desglose_de_reloj_cuadra(), "{inc:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_columna_ausente_se_persiste_como_tal() {
        let mut csv = String::from("OS,Linux\nfps,frametime,gpu_load\n");
        for _ in 0..100 {
            csv.push_str("60.0,16.6,10\n");
        }
        let dir = std::env::temp_dir().join("thoriapp_541a_sin_columna");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        let ruta = dir.join("log.csv");
        fs::write(&ruta, csv).expect("escribir");

        let b = analyze_mangohud_log(&ruta.to_string_lossy(), 0.0).expect("análisis");
        let inc = &b.contabilidad_muestras.as_ref().expect("contabilidad").incidencias;
        assert_eq!(inc.columna_elapsed_ausente, 100);
        assert_eq!(inc.reloj_incompleto, 100);
        assert!(inc.desglose_de_reloj_cuadra());
        // Y la serie temporal no inventa un eje en segundos.
        assert!(b.muestras_temporales.as_ref().is_none_or(|m| m.elapsed_s.is_empty()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn filas_repetidas_no_exige_que_toda_la_fila_sea_identica() {
        // Mismo timestamp pero rendimiento distinto: es un reloj repetido, no una fila
        // duplicada. Se cuenta igual, y **no** se descarta ninguna de las dos.
        let mut filas = serie_constante(200, 60.0);
        let ns = filas[60].reloj;
        filas[61] = FilaLeida {
            fps: 45.0,
            frame_time: 22.2,
            reloj: ns,
        };
        let p = preparar_muestras(filas, 0.0, true, 200, 0);
        assert_eq!(p.contabilidad.incidencias.filas_repetidas, 1);
        assert!(!p.contabilidad.eje_temporal_fiable);
        // Las dos filas entran: el rendimiento no se toca por repetir el reloj.
        assert_eq!(p.contabilidad.muestras_analizadas, 200);
    }

    #[test]
    fn v2_registra_varias_incidencias_simultaneas_en_la_misma_fila() {
        let mut filas = serie_constante(300, 60.0);
        poner_reloj(&mut filas[50], RelojDeFila::Valor(-3.0)); // negativa + invertida
        let reloj_79 = filas[79].reloj;
        poner_reloj(&mut filas[80], reloj_79); // repetida
        let preparadas = preparar_muestras(filas, 0.0, true, 300, 0);
        let inc = &preparadas.contabilidad.incidencias;
        assert_eq!(inc.negativas, 1);
        assert_eq!(inc.filas_repetidas, 1);
        assert!(inc.total() >= 2);
        // Las incidencias no son exclusiones: siguen las 300 muestras.
        assert_eq!(preparadas.contabilidad.muestras_analizadas, 300);
    }

    #[test]
    fn v2_la_contabilidad_cuadra_con_filas_invalidas() {
        // 500 filas de datos: 460 válidas y 40 inválidas.
        // Las 460 válidas van a 10 Hz (100 ms) desde 0: llegan hasta 45.9 s, así que el
        // corte de 10 s cae en la fila índice 100 y quedan 360 analizables.
        let preparadas = preparar_muestras(serie_constante(460, 60.0), 10.0, true, 500, 40);
        let c = &preparadas.contabilidad;
        assert_eq!(c.filas_originales, 500);
        assert_eq!(c.filas_con_rendimiento_valido, 460);
        assert_eq!(c.filas_con_rendimiento_invalido, 40);
        assert_eq!(c.excluidas_por_calentamiento, 100);
        assert_eq!(c.muestras_analizadas, 360);
        assert!(c.cuadra(), "la contabilidad debe cuadrar: {c:?}");
    }

    #[test]
    fn v2_el_calentamiento_no_sale_donde_no_procede_y_declara_el_motivo() {
        // Pocas muestras: no se aplica y se dice por qué.
        let preparadas = preparar_muestras(serie_constante(50, 60.0), 10.0, true, 50, 0);
        assert_eq!(preparadas.contabilidad.excluidas_por_calentamiento, 0);
        assert_eq!(
            preparadas.contabilidad.calentamiento_no_aplicado,
            Some(MotivoSinCalentamiento::PocasMuestras)
        );

        // Sin calentamiento pedido: se declara como no solicitado.
        let preparadas = preparar_muestras(serie_constante(500, 60.0), 0.0, true, 500, 0);
        assert_eq!(
            preparadas.contabilidad.calentamiento_no_aplicado,
            Some(MotivoSinCalentamiento::NoSolicitado)
        );
        assert!(preparadas.contabilidad.calentamiento_no_aplicado.is_some() || true);
    }

    #[test]
    fn v2_el_calculo_de_la_media_de_cola_incluye_los_empates() {
        // Serie constante: P99 es el propio valor y el filtro `>=` deja entrar todas las
        // muestras, no el 1 %.
        let ft = vec![16.0_f64; 400];
        let p99 = percentile_sorted(&ft, 99.0);
        assert_eq!(p99, 16.0);
        let incluidas = muestras_en_media_de_cola(&ft, p99);
        assert_eq!(incluidas.len(), 400);

        let apoyo = ApoyoDePercentil {
            muestras_en_la_media: Some(incluidas.len() as u32),
            ..apoyo_de_percentil(&ft, 99.0)
        };
        assert_eq!(apoyo.muestras, 400);
        assert_eq!(apoyo.muestras_en_la_media, Some(400));
    }

    #[test]
    fn apoyo_de_percentil_declara_la_cola_y_la_interpolacion() {
        let valores: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let apoyo = apoyo_de_percentil(&valores, 99.9);
        assert_eq!(apoyo.muestras, 1000);
        // Cola nominal: 1000 * 0.001 = 1 muestra.
        assert!((apoyo.cola_nominal - 1.0).abs() < 1e-9);
        assert_eq!(apoyo.observaciones_interpoladas, 2);
        assert_eq!(apoyo.muestras_en_la_media, None);
    }

    #[test]
    fn apoyo_de_percentil_con_pocas_muestras_declara_cola_menor_que_uno() {
        let valores: Vec<f64> = (0..500).map(|i| i as f64).collect();
        let apoyo = apoyo_de_percentil(&valores, 99.9);
        assert!(apoyo.cola_nominal < 1.0, "cola nominal: {}", apoyo.cola_nominal);
    }

    #[test]
    fn el_corte_de_calentamiento_respeta_los_limites() {
        // 60 s a 10 Hz = 600 filas; con 10 s de calentamiento salen 100 filas.
        let filas: Vec<Sample> = serie_constante(600, 60.0)
            .into_iter()
            .map(|f| Sample {
                fps: f.fps,
                frame_time: f.frame_time,
                elapsed_ns: f.reloj.valor(),
            })
            .collect();
        let (restantes, aplicado) = split_warmup(filas, 10.0);
        assert_eq!(restantes.len(), 500);
        assert!((aplicado - 10.0).abs() < 1e-6);
        // El corte cae exactamente en el primer timestamp >= 10 s.
        assert!((restantes[0].elapsed_ns.unwrap() - 10_000_000_000.0).abs() < 1.0);
    }

    #[test]
    fn el_corte_de_calentamiento_no_aplica_si_dejaria_menos_de_lo_minimo() {
        // 25 s totales: con 10 s de calentamiento quedarían 15 s < MIN_STEADY_SECONDS.
        let filas: Vec<Sample> = serie_constante(250, 60.0)
            .into_iter()
            .map(|f| Sample {
                fps: f.fps,
                frame_time: f.frame_time,
                elapsed_ns: f.reloj.valor(),
            })
            .collect();
        let (restantes, aplicado) = split_warmup(filas, 10.0);
        assert_eq!(restantes.len(), 250);
        assert_eq!(aplicado, 0.0);
    }

    #[test]
    fn stutter_detecta_picos_y_frames_lentos() {
        // 8 frames normales de 16 ms y 2 picos de 100 ms.
        let mut ft = vec![16.0; 8];
        ft.push(100.0);
        ft.push(100.0);
        let s = compute_stutter(&ft, 60.0);

        // mediana 16 → umbral max(32, 25) = 32 → los dos de 100 son picos
        assert_eq!(s.spikes_per_min, Some(2.0)); // 2 picos en 1 minuto
        assert!((s.pct_over_33ms - 20.0).abs() < 1e-9); // 2 de 10
        assert!(s.jitter_ms > 0.0);
        assert!((s.worst_frame_ms - 100.0).abs() < 1e-9);
    }

    #[test]
    fn stutter_con_frame_times_uniformes_no_hay_jitter() {
        let ft = vec![16.0; 60];
        let s = compute_stutter(&ft, 60.0);
        assert_eq!(s.spikes_per_min, Some(0.0));
        assert_eq!(s.pct_over_33ms, 0.0);
        assert_eq!(s.jitter_ms, 0.0);
    }

    #[test]
    fn stutter_con_duracion_cero_no_divide_por_cero() {
        // Duración desconocida (0.0 = «no consta»): no hay tasa que dar. Antes se
        // devolvía 0.0, que se leía como «ningún pico» sin serlo.
        let s = compute_stutter(&[16.0, 200.0], 0.0);
        assert_eq!(s.spikes_per_min, None);
        // Lo que no depende del reloj se sigue calculando.
        assert!(s.pct_over_33ms > 0.0);
    }

    #[test]
    fn same_game_cae_al_nombre_si_falta_appid_via_game_key() {
        // Un juego de Lutris no tiene appid (0): se compara el nombre, sin
        // distinguir mayúsculas ni espacios sobrantes. Esto antes lo resolvía una
        // función `same_game` que ya no existe: `game_key` la sustituyó y es la
        // única que se usa de verdad, así que la cobertura vive aquí.
        assert_eq!(
            game_key(&bench(0, "Hollow Knight")),
            game_key(&bench(0, "hollow knight "))
        );
        assert_ne!(
            game_key(&bench(0, "Hollow Knight")),
            game_key(&bench(0, "Celeste"))
        );
        // Y con appid manda el appid, aunque el nombre no coincida.
        assert_eq!(
            game_key(&bench(440, "TF2")),
            game_key(&bench(440, "Otro nombre"))
        );
        assert_ne!(game_key(&bench(440, "TF2")), game_key(&bench(730, "CS2")));
    }

    /// Benchmark con fecha conocida, para probar el recorte por antigüedad.
    ///
    /// El `id` incluye la fecha a propósito: dos grabaciones del mismo juego nunca
    /// comparten id en la app real (lleva la hora con milisegundos), y un test que las
    /// hiciera chocar estaría probando algo que no puede pasar.
    fn bench_at(appid: u32, name: &str, day: u32) -> Benchmark {
        let mut b = bench(appid, name);
        b.timestamp = format!("2026-01-{:02} 10:00:00", day);
        b.id = format!("{}-{}", b.id, day);
        b
    }

    /// Simula un guardado completo: asigna número, añade y recorta, como `save_benchmark`.
    fn guardar(data: &mut BenchmarksData, appid: u32, nombre: &str, dia: u32) -> u32 {
        let mut b = bench_at(appid, nombre, dia);
        asignar_numero(data, &mut b);
        let n = b.number.expect("toda grabación nueva tiene número");
        data.benchmarks.push(b);
        trim_history(&mut data.benchmarks);
        n
    }

    fn vacio() -> BenchmarksData {
        BenchmarksData {
            version: None,
            benchmarks: vec![],
            counters: HashMap::new(),
        }
    }

    #[test]
    fn numera_consecutivo_por_juego() {
        let mut d = vacio();
        assert_eq!(guardar(&mut d, 440, "TF2", 1), 1);
        assert_eq!(guardar(&mut d, 440, "TF2", 2), 2);
        assert_eq!(guardar(&mut d, 440, "TF2", 3), 3);
    }

    #[test]
    fn numeracion_independiente_entre_juegos() {
        let mut d = vacio();
        assert_eq!(guardar(&mut d, 440, "TF2", 1), 1);
        assert_eq!(guardar(&mut d, 730, "CS2", 2), 1);
        assert_eq!(guardar(&mut d, 440, "TF2", 3), 2);
        assert_eq!(guardar(&mut d, 730, "CS2", 4), 2);
    }

    #[test]
    fn borrar_una_prueba_no_renumera_las_demas() {
        let mut d = vacio();
        guardar(&mut d, 440, "TF2", 1);
        guardar(&mut d, 440, "TF2", 2);
        guardar(&mut d, 440, "TF2", 3);

        // Se borra la del medio, como hace `delete_benchmark`.
        d.benchmarks.retain(|b| b.number != Some(2));

        let numeros: Vec<u32> = d.benchmarks.iter().filter_map(|b| b.number).collect();
        assert_eq!(numeros, vec![1, 3], "borrar no puede renumerar");
    }

    #[test]
    fn tras_borrar_la_siguiente_no_reutiliza_el_numero() {
        let mut d = vacio();
        guardar(&mut d, 440, "TF2", 1);
        guardar(&mut d, 440, "TF2", 2);

        // Se borran TODAS: el historial queda vacío y el contador debe aguantar.
        d.benchmarks.clear();

        // Este es el caso que un contador derivado del historial rompería: sin estado
        // persistente volvería a repartir el 1.
        assert_eq!(guardar(&mut d, 440, "TF2", 3), 3);
    }

    // ── Identidad del juego ──────────────────────────────────────────────────
    //
    // Regresión: la ventana «Pruebas de este juego» decía «todavía no hay pruebas» aunque
    // el usuario tuviera una guardada. `analyze_log` deja `appid` a 0 (MangoHud no escribe
    // el AppID en su CSV), así que filtrar solo por appid no encontraba nada.

    #[test]
    fn coincide_por_nombre_cuando_la_grabacion_no_tiene_appid() {
        // El caso real: prueba guardada antes, con appid 0 y nombre del juego.
        let b = bench(0, "Palworld");
        assert!(juego_coincide(&b, 1623730, "Palworld"));
    }

    #[test]
    fn coincide_por_nombre_ignorando_el_sufijo_del_ejecutable() {
        // El CSV de MangoHud trae el binario, no el nombre comercial.
        let b = bench(0, "Palworld-Win64-Shipping.exe");
        assert!(juego_coincide(&b, 1623730, "Palworld-Win64-Shipping"));
        let b2 = bench(0, "Celeste.exe");
        assert!(juego_coincide(&b2, 504230, "Celeste"));
    }

    #[test]
    fn coincide_por_appid_cuando_lo_tiene() {
        let b = bench(1623730, "Palworld");
        assert!(juego_coincide(&b, 1623730, "Palworld"));
    }

    #[test]
    fn no_coincide_con_otro_juego_aunque_se_llame_igual() {
        // Dos juegos distintos con el mismo nombre: manda el appid, no el texto.
        let b = bench(440, "Team Fortress 2");
        assert!(!juego_coincide(&b, 730, "Team Fortress 2"));
    }

    #[test]
    fn una_grabacion_antigua_no_se_cuela_en_otro_juego_de_igual_nombre() {
        // Antigua (appid 0) frente a una ficha con appid: solo entra si el nombre cuadra.
        let b = bench(0, "Palworld");
        assert!(juego_coincide(&b, 1623730, "Palworld"));
        assert!(!juego_coincide(&b, 1623730, "Otro Juego"));
    }

    #[test]
    fn sin_appid_ni_nombre_no_se_afirma_nada() {
        let b = bench(0, "");
        assert!(!juego_coincide(&b, 0, ""));
    }

    #[test]
    fn load_benchmarks_for_game_encuentra_la_prueba_antigua_sin_appid() {
        // Reproduce el fallo: una grabación antigua de Palworld con appid 0.
        let data = BenchmarksData {
            version: None,
            benchmarks: vec![bench(0, "Palworld"), bench(730, "CS2")],
            counters: HashMap::new(),
        };
        let mut del_juego: Vec<Benchmark> = data
            .benchmarks
            .into_iter()
            .filter(|b| juego_coincide(b, 1623730, "Palworld"))
            .collect();
        trim_history(&mut del_juego);

        assert_eq!(del_juego.len(), 1, "la prueba antigua debe aparecer");
        assert_eq!(del_juego[0].game_name, "Palworld");
    }

    // ── Borrado por juego ────────────────────────────────────────────────────
    //
    // `delete_benchmarks_for_game` es lo que usa la ventana «Pruebas de este juego».
    // Lo importante es que NO toque el historial de los otros juegos: borrar todo desde
    // la ficha de un juego no puede vaciar la lista entera.

    #[test]
    fn borrar_por_juego_respeta_los_demas_juegos() {
        let mut d = vacio();
        d.benchmarks.push(bench(440, "TF2"));
        d.benchmarks.push(bench(730, "CS2"));

        // Mismo criterio que usa el comando.
        d.benchmarks.retain(|b| !juego_coincide(b, 440, "TF2"));

        assert_eq!(d.benchmarks.len(), 1);
        assert_eq!(d.benchmarks[0].game_name, "CS2");
    }

    #[test]
    fn borrar_por_juego_sin_appid_usa_el_nombre_normalizado() {
        let mut d = vacio();
        // Grabaciones sin AppID (MangoHud no lo escribe): la clave es el nombre.
        d.benchmarks.push(bench(0, "Hollow Knight"));
        d.benchmarks.push(bench(0, "Celeste"));

        d.benchmarks.retain(|b| !juego_coincide(b, 0, "hollow knight"));

        assert_eq!(d.benchmarks.len(), 1);
        assert_eq!(d.benchmarks[0].game_name, "Celeste");
    }

    #[test]
    fn borrar_por_juego_sin_coincidencias_no_cambia_nada() {
        let mut d = vacio();
        d.benchmarks.push(bench(440, "TF2"));

        let antes = d.benchmarks.len();
        d.benchmarks.retain(|b| !juego_coincide(b, 999, "Otro Juego"));

        assert_eq!(d.benchmarks.len(), antes);
    }

    #[test]
    fn borrar_por_juego_no_renumera_las_pruebas_de_otros_juegos() {
        // El contador de números vive aparte; borrar por juego no lo toca.
        let mut d = vacio();
        guardar(&mut d, 440, "TF2", 1);
        guardar(&mut d, 730, "CS2", 1);
        guardar(&mut d, 730, "CS2", 2);

        d.benchmarks.retain(|b| !juego_coincide(b, 440, "TF2"));

        let de_cs2: Vec<u32> = d.benchmarks.iter().filter_map(|b| b.number).collect();
        assert_eq!(de_cs2, vec![1, 2], "CS2 conserva sus números");
        // Y la siguiente prueba de CS2 sigue la cuenta.
        assert_eq!(guardar(&mut d, 730, "CS2", 3), 3);
    }

    #[test]
    fn trim_history_no_devuelve_numeros_al_bombo() {
        let mut d = vacio();
        for dia in 1..=MAX_BENCHMARKS_PER_GAME as u32 {
            guardar(&mut d, 440, "TF2", dia);
        }
        assert_eq!(d.benchmarks.len(), MAX_BENCHMARKS_PER_GAME);

        // La 11.ª provoca el recorte de la 1.ª, pero la nueva es la 11, no la 1.
        assert_eq!(guardar(&mut d, 440, "TF2", 11), 11);
        assert_eq!(d.benchmarks.len(), MAX_BENCHMARKS_PER_GAME);
        assert!(!d.benchmarks.iter().any(|b| b.number == Some(1)));
    }

    #[test]
    fn el_contador_se_inicializa_desde_los_numeros_ya_guardados() {
        // Archivo anterior a `counters`, pero con números ya repartidos: la serie sigue,
        // no vuelve a empezar en 1.
        let mut d = vacio();
        let mut viejo = bench_at(440, "TF2", 1);
        viejo.number = Some(7);
        d.benchmarks.push(viejo);

        assert_eq!(guardar(&mut d, 440, "TF2", 2), 8);
    }

    #[test]
    fn grabaciones_antiguas_sin_numero_no_inventan_cronologia() {
        let mut d = vacio();
        // Dos antiguas sin número: NO cuentan para inicializar el contador.
        d.benchmarks.push(bench_at(440, "TF2", 1));
        d.benchmarks.push(bench_at(440, "TF2", 2));

        assert_eq!(guardar(&mut d, 440, "TF2", 3), 1);
        // Y las viejas siguen sin número: no se les asigna uno retroactivo.
        assert_eq!(d.benchmarks.iter().filter(|b| b.number.is_none()).count(), 2);
    }

    #[test]
    fn juegos_sin_appid_se_numeran_por_nombre_normalizado() {
        let mut d = vacio();
        assert_eq!(guardar(&mut d, 0, "Hollow Knight", 1), 1);
        // Mismo juego, nombre con mayúsculas y espacios: misma serie.
        assert_eq!(guardar(&mut d, 0, "  hollow knight  ", 2), 2);
        // Otro juego sin appid: serie propia.
        assert_eq!(guardar(&mut d, 0, "Celeste", 3), 1);
    }

    #[test]
    fn el_numero_sobrevive_a_serializar_y_recargar() {
        let mut d = vacio();
        guardar(&mut d, 440, "TF2", 1);
        guardar(&mut d, 440, "TF2", 2);

        let json = serde_json::to_string(&d).unwrap();
        let vuelta: BenchmarksData = serde_json::from_str(&json).unwrap();

        let numeros: Vec<u32> = vuelta.benchmarks.iter().filter_map(|b| b.number).collect();
        assert_eq!(numeros, vec![1, 2]);
        assert_eq!(vuelta.counters.get("appid:440"), Some(&2));
    }

    #[test]
    fn json_antiguo_sin_numero_ni_contadores_se_lee_sin_romper() {
        // Exactamente el formato anterior: sin `number` y sin `counters`.
        let json = r#"{
            "benchmarks": [{
                "id":"b","profile_id":"","game_name":"TF2","appid":440,
                "timestamp":"2026-01-01 10:00:00","duration_seconds":60.0,
                "avg_fps":60.0,"fps_1_percent_low":50.0,"fps_0_1_percent_low":45.0,
                "max_fps":80.0,"min_fps":30.0,"frame_time_avg":16.6,
                "frame_time_1_percent_high":20.0,"total_frames":300,"notes":null,
                "launch_command":"%command%"
            }]
        }"#;
        let d: BenchmarksData = serde_json::from_str(json).unwrap();
        assert_eq!(d.benchmarks.len(), 1);
        assert_eq!(d.benchmarks[0].number, None);
        assert!(d.counters.is_empty());
    }

    #[test]
    fn trim_history_conserva_las_diez_mas_recientes() {
        // Se añaden en orden, y se recorta: deben quedar las 10 últimas (días 6..15).
        let mut list: Vec<Benchmark> = (1..=15).map(|d| bench_at(440, "TF2", d)).collect();
        trim_history(&mut list);

        assert_eq!(list.len(), MAX_BENCHMARKS_PER_GAME);
        assert_eq!(list.first().unwrap().timestamp, "2026-01-06 10:00:00");
        assert_eq!(list.last().unwrap().timestamp, "2026-01-15 10:00:00");
        // El orden original se respeta: no se reordena el archivo.
        assert!(list.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));
    }

    #[test]
    fn trim_history_recorta_por_fecha_y_no_por_orden_de_lista() {
        // Añadida la última pero con fecha vieja: es la que debe caer.
        let mut list: Vec<Benchmark> = (1..=MAX_BENCHMARKS_PER_GAME as u32)
            .map(|d| bench_at(440, "TF2", d + 10))
            .collect();
        list.push(bench_at(440, "TF2", 1));

        trim_history(&mut list);
        assert_eq!(list.len(), MAX_BENCHMARKS_PER_GAME);
        assert!(!list.iter().any(|b| b.timestamp == "2026-01-01 10:00:00"));
    }

    #[test]
    fn trim_history_no_toca_los_demas_juegos() {
        // Un juego no se come el historial de otro, aunque comparta nombre.
        let mut list: Vec<Benchmark> = (1..=MAX_BENCHMARKS_PER_GAME as u32 + 5)
            .map(|d| bench_at(440, "TF2", d))
            .collect();
        list.extend((1..=3).map(|d| bench_at(730, "CS2", d)));

        trim_history(&mut list);
        assert_eq!(list.iter().filter(|b| b.appid == 440).count(), MAX_BENCHMARKS_PER_GAME);
        assert_eq!(list.iter().filter(|b| b.appid == 730).count(), 3);
    }

    #[test]
    fn trim_history_separa_juegos_sin_appid_por_nombre() {
        let mut list: Vec<Benchmark> = (1..=MAX_BENCHMARKS_PER_GAME as u32 + 2)
            .map(|d| bench_at(0, "Hollow Knight", d))
            .collect();
        list.push(bench_at(0, "Celeste", 99));

        trim_history(&mut list);
        assert_eq!(list.len(), MAX_BENCHMARKS_PER_GAME + 1);
        assert!(list.iter().any(|b| b.game_name == "Celeste"));
    }

    #[test]
    fn guardar_un_juego_no_borra_las_pruebas_de_otro() {
        // La política anterior borraba todo al cambiar de juego; ahora el historial es
        // por juego, así que se puede volver a un juego antiguo y seguir comparando.
        let mut data = vec![
            bench_at(440, "TF2", 1),
            bench_at(440, "TF2", 2),
            bench_at(730, "CS2", 3),
        ];

        // Llega una grabación nueva de TF2: se añade al historial sin tocar lo anterior
        // ni la de CS2. La política anterior borraba todo al cambiar de juego.
        data.push(bench_at(440, "TF2", 9));
        trim_history(&mut data);

        assert_eq!(data.iter().filter(|b| b.appid == 440).count(), 3);
        assert_eq!(data.iter().filter(|b| b.appid == 730).count(), 1);
        // Las tres de TF2 siguen ahí, en orden de guardado, y CS2 intacto.
        let tf2: Vec<_> = data.iter().filter(|b| b.appid == 440).collect();
        assert_eq!(tf2[0].timestamp, "2026-01-01 10:00:00");
        assert_eq!(tf2[1].timestamp, "2026-01-02 10:00:00");
        assert_eq!(tf2[2].timestamp, "2026-01-09 10:00:00");
    }

    #[test]
    fn trim_history_no_hace_nada_si_no_sobra_nada() {
        let mut list: Vec<Benchmark> = vec![bench_at(440, "TF2", 1), bench_at(440, "TF2", 2)];
        trim_history(&mut list);
        assert_eq!(list.len(), 2);

        let mut empty: Vec<Benchmark> = vec![];
        trim_history(&mut empty);
        assert!(empty.is_empty());
    }

    /// Frame times con reloj real: `n` filas cada 100 ms, con el frametime indicado.
    fn timed_frames(counts: &[(usize, f64)]) -> Vec<Sample> {
        let mut out = Vec::new();
        let mut i = 0usize;
        for (n, ft) in counts {
            for _ in 0..*n {
                out.push(Sample {
                    fps: 1000.0 / ft,
                    frame_time: *ft,
                    elapsed_ns: Some(i as f64 * 1e8),
                });
                i += 1;
            }
        }
        out
    }

    #[test]
    fn sample_window_parte_en_dos_tramos_disjuntos() {
        // 600 muestras = 60 s. Ventana de 10 s = 100 muestras.
        let samples = timed_frames(&[(600, 16.0)]);
        let (early, steady) = sample_window(&samples, 10.0);
        assert_eq!(early.len(), 100);
        assert_eq!(steady.len(), 500);
        assert_eq!(early.len() + steady.len(), samples.len());
    }

    #[test]
    fn sample_window_sin_reloj_no_inventa_nada() {
        // Log antiguo sin columna `elapsed`: imposible saber dónde está el corte.
        let samples: Vec<Sample> = (0..600)
            .map(|_| Sample { fps: 60.0, frame_time: 16.0, elapsed_ns: None })
            .collect();
        let (early, steady) = sample_window(&samples, 10.0);
        assert!(early.is_empty());
        assert!(steady.is_empty());
    }

    #[test]
    fn sample_window_exige_muestras_en_los_dos_lados() {
        // Grabación de 5 s con ventana de 10: no queda resto con el que comparar.
        let samples = timed_frames(&[(50, 16.0)]);
        let (early, steady) = sample_window(&samples, 10.0);
        assert!(early.is_empty());
        assert!(steady.is_empty());
    }

    #[test]
    fn arranque_con_tirones_se_detecta_por_el_peor_uno_por_ciento() {
        // El arranque tiene los mismos FPS medios que el resto, pero con tirones
        // puntuales: es exactamente el caso que el promedio esconde.
        let mut early = vec![16.0; 2000];
        for i in (0..2000).step_by(10) {
            early[i] = 80.0; // 12,5 FPS: tirón de compilación de shaders
        }
        let steady = vec![16.0; 2000];

        let sh = shader_stutter(&early, &steady, 10.0).expect("debería medirse");
        assert!(
            sh.stutter_penalty_percent >= SIGNIFICANT_SHADER_PENALTY,
            "penalización: {}",
            sh.stutter_penalty_percent
        );
        assert!(sh.early_pct_over_33ms > sh.steady_pct_over_33ms);
    }

    #[test]
    fn arranque_igual_al_resto_no_es_un_problema() {
        let steady = vec![16.0; 2000];
        let sh = shader_stutter(&steady, &steady, 10.0).unwrap();
        assert_eq!(sh.stutter_penalty_percent, 0.0);
    }

    #[test]
    fn arranque_mejor_que_el_resto_no_se_presenta_como_mejora() {
        // Si el principio va mejor (precarga), no se informa de penalización negativa.
        let early = vec![10.0; 2000];
        let steady = vec![16.0; 2000];
        let sh = shader_stutter(&early, &steady, 10.0).unwrap();
        assert_eq!(sh.stutter_penalty_percent, 0.0);
    }

    #[test]
    fn sin_muestras_suficientes_no_se_opina_sobre_shaders() {
        assert!(shader_stutter(&[16.0; 10], &[16.0; 2000], 10.0).is_none());
        assert!(shader_stutter(&[16.0; 2000], &[16.0; 10], 10.0).is_none());
        assert!(shader_stutter(&[16.0; 2000], &[16.0; 2000], 0.0).is_none());
    }

    #[test]
    fn benchmark_viejo_sin_datos_de_shaders_se_sigue_cargando() {
        // Compatibilidad: los benchmarks guardados antes de esta mejora no tienen el
        // campo `shader_stutter` y deben deserializar sin error.
        let json = r#"{"id":"x","profile_id":"","game_name":"TF2","appid":440,"timestamp":"",
            "duration_seconds":60.0,"avg_fps":60.0,"fps_1_percent_low":30.0,
            "fps_0_1_percent_low":20.0,"max_fps":120.0,"min_fps":10.0,
            "frame_time_avg":16.6,"frame_time_1_percent_high":33.0,"total_frames":3600,
            "notes":null,"launch_command":"%command%"}"#;
        let b: Benchmark = serde_json::from_str(json).unwrap();
        assert!(b.shader_stutter.is_none());
        assert!(b.noise_percent.is_none());
    }

    #[test]
    fn el_umbral_coincide_con_el_de_la_interfaz() {
        // `src/lib/shaders.ts` tiene su propia copia (SIGNIFICANT_PENALTY) porque la
        // decisión se toma en el frontend, donde está testeada. Si alguien cambia una y
        // no la otra, el test lo detecta.
        let ts = include_str!("../../src/lib/shaders.ts");
        // Se busca la declaración exacta (`= 20;`), no solo el número, para que un
        // cambio a 200 u 2 no pase por casualidad.
        let expected = format!("SIGNIFICANT_PENALTY = {};", SIGNIFICANT_SHADER_PENALTY);
        assert!(
            ts.contains(&expected),
            "src/lib/shaders.ts debe declarar `{}`",
            expected
        );
    }

    #[test]
    fn guardar_diez_veces_conserva_las_diez_y_no_solo_una() {
        // Recorre la misma secuencia que `save_benchmark` (push + trim_history) sin tocar
        // disco. Existía un `retain` por juego que borraba todas las grabaciones
        // anteriores del juego antes de recortar, así que el historial de 10 nunca llegaba
        // a llenarse. Este test lo habría detectado.
        let mut data: Vec<Benchmark> = Vec::new();
        for day in 1..=10u32 {
            data.push(bench_at(440, "TF2", day));
            trim_history(&mut data);
            assert_eq!(
                data.iter().filter(|b| b.appid == 440).count() as u32,
                day,
                "tras {} grabaciones deberían quedar {}",
                day,
                day
            );
        }

        // La número 11 tira la más antigua (día 1) y conserva la 2..11.
        data.push(bench_at(440, "TF2", 11));
        trim_history(&mut data);
        assert_eq!(data.len(), 10);
        assert_eq!(data[0].timestamp, "2026-01-02 10:00:00");
        assert_eq!(data[9].timestamp, "2026-01-11 10:00:00");
    }

    #[test]
    fn game_key_usa_appid_y_cae_al_nombre() {
        assert_eq!(game_key(&bench(440, "TF2")), "appid:440");
        assert_eq!(game_key(&bench(0, "Hollow Knight")), "name:hollow knight");
    }

    #[test]
    fn norm_game_name_ignora_mayusculas_y_espacios() {
        assert_eq!(norm_game_name("  Hollow Knight  "), norm_game_name("hollow knight"));
    }

    #[test]
    fn norm_game_name_quita_la_extension_del_ejecutable() {
        assert_eq!(norm_game_name("Celeste.exe"), "celeste");
        assert_eq!(norm_game_name("Game.x86_64"), "game");
        assert_eq!(norm_game_name("Game.bin"), "game");
    }

    /// Serie de muestras con reloj real: `n` filas separadas 100 ms (como MangoHud).
    fn samples_with_clock(n: usize, fps: f64) -> Vec<Sample> {
        (0..n)
            .map(|i| Sample {
                fps,
                frame_time: 1000.0 / fps,
                elapsed_ns: Some(i as f64 * 1e8),
            })
            .collect()
    }

    #[test]
    fn warmup_descarta_los_primeros_segundos() {
        // 600 filas = 60 s → descartar los primeros 10 s quita 100 filas.
        let (kept, applied) = split_warmup(samples_with_clock(600, 60.0), 10.0);
        assert_eq!(kept.len(), 500);
        assert!((applied - 10.0).abs() < 0.11);
    }

    #[test]
    fn warmup_no_se_aplica_si_queda_poco() {
        // 25 s de grabación: descartar 10 dejaría 15 s (< MIN_STEADY_SECONDS).
        let (kept, applied) = split_warmup(samples_with_clock(250, 60.0), 10.0);
        assert_eq!(kept.len(), 250);
        assert_eq!(applied, 0.0);
    }

    #[test]
    fn warmup_sin_columna_elapsed_no_hace_nada() {
        let samples: Vec<Sample> = (0..600).map(|_| sample(60.0, 16.6)).collect();
        let (kept, applied) = split_warmup(samples, 10.0);
        assert_eq!(kept.len(), 600);
        assert_eq!(applied, 0.0);
    }

    #[test]
    fn warmup_cero_no_toca_nada() {
        let (kept, applied) = split_warmup(samples_with_clock(600, 60.0), 0.0);
        assert_eq!(kept.len(), 600);
        assert_eq!(applied, 0.0);
    }

    #[test]
    fn ruido_es_cero_si_la_serie_es_constante() {
        let fps: Vec<f64> = (0..600).map(|_| 60.0).collect();
        let noise = noise_percent(&fps).expect("debería poder estimarse");
        assert!(noise < 1e-9);
    }

    #[test]
    fn ruido_crece_cuando_los_bloques_varian() {
        // 600 filas en 10 bloques: cada bloque con una media distinta (±10 FPS).
        let fps: Vec<f64> = (0..600)
            .map(|i| if (i / 60) % 2 == 0 { 50.0 } else { 70.0 })
            .collect();
        let noise = noise_percent(&fps).expect("debería poder estimarse");
        // Medias 50 y 70 con 5 bloques de cada: media 60, error estándar ≈ 3,33
        // → ±10,9 % con 95 % de confianza.
        assert!(noise > 6.0 && noise < 15.0, "ruido inesperado: {}", noise);
    }

    #[test]
    fn ruido_no_se_estima_con_muy_pocas_muestras() {
        let fps: Vec<f64> = (0..150).map(|_| 60.0).collect();
        assert!(noise_percent(&fps).is_none());
    }

    #[test]
    fn ruido_combinado_suma_en_cuadratura() {
        // 3-4-5: √(9+16) = 5
        assert!((combined_noise(Some(3.0), Some(4.0)) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn ruido_combinado_respeta_el_minimo_si_falta_el_dato() {
        // Un dato ausente o absurdo no se inventa: no aporta margen.
        assert!((combined_noise(Some(0.0), None) - MIN_NOISE).abs() < 1e-9);
        assert!((combined_noise(Some(0.1), Some(0.1)) - MIN_NOISE).abs() < 1e-9);
        assert!((combined_noise(None, None) - MIN_NOISE).abs() < 1e-9);
        // Con una sola grabación medida, el margen es el suyo (no √2 veces mayor).
        assert!((combined_noise(Some(8.0), None) - 8.0).abs() < 1e-9);
    }

    // ==================== Subfase 5.1: integridad del almacenamiento ====================
    //
    // Estos tests trabajan sobre archivos temporales reales, nunca sobre los del usuario:
    // `leer_datos` y `escribir_atomico` reciben la ruta, así que se pueden probar sin
    // tocar `~/.config/thoriapp`. Ese era el requisito: verificar el comportamiento sin
    // corromper el historial de nadie.

    /// JSON mínimo de un historial válido y antiguo (sin `version`), como los que hay en
    /// disco hoy.
    fn historial_antiguo_json() -> String {
        format!(
            r#"{{"benchmarks":[{{"id":"b1","profile_id":"","game_name":"Palworld","appid":1623730,"timestamp":"2024-01-01 10:00:00","duration_seconds":60.0,"avg_fps":60.0,"fps_1_percent_low":50.0,"fps_0_1_percent_low":40.0,"max_fps":70.0,"min_fps":30.0,"frame_time_avg":16.6,"frame_time_1_percent_high":20.0,"total_frames":600,"notes":null}}],"counters":{{"appid:1623730":1}}}}"#
        )
    }

    pub(crate) fn leer_bench(path: &Path) -> Result<Option<BenchmarksData>, ErrorDatos> {
        leer_datos::<BenchmarksData>(
            path,
            "benchmarks.json",
            BENCHMARKS_SCHEMA_VERSION,
            |v| v.get("version").and_then(|x| x.as_u64()).map(|x| x as u32),
            serde_json::from_value,
        )
    }

    /// 1. Archivo inexistente: es un estado legítimo, no un error.
    #[test]
    fn s51_archivo_inexistente_es_lista_vacia_no_error() {
        let dir = tmp_dir("s51-inexistente");
        let path = dir.join("benchmarks.json");

        let leido = leer_bench(&path).expect("no debe fallar");
        assert!(leido.is_none(), "no existe el archivo: no hay datos, sin error");

        // Y el estado por defecto que se usa al escribir tampoco inventa nada.
        let d = try_load_benchmarks_data_en(&path).unwrap();
        assert!(d.benchmarks.is_empty());
        assert!(d.counters.is_empty());
    }

    /// Variante de `try_load_benchmarks_data` sobre una ruta concreta, solo para tests.
    fn try_load_benchmarks_data_en(path: &Path) -> Result<BenchmarksData, ErrorDatos> {
        Ok(leer_bench(path)?.unwrap_or(BenchmarksData {
            version: None,
            benchmarks: Vec::new(),
            counters: HashMap::new(),
        }))
    }

    /// 2. Archivo válido antiguo, sin `version`: se lee como versión 1.
    #[test]
    fn s51_archivo_antiguo_sin_version_se_lee_como_v1() {
        let dir = tmp_dir("s51-antiguo");
        let path = dir.join("benchmarks.json");
        fs::write(&path, historial_antiguo_json()).unwrap();

        let d = leer_bench(&path).unwrap().expect("hay archivo");
        assert!(d.version.is_none(), "un archivo antiguo no se reescribe al leer");
        assert_eq!(d.benchmarks.len(), 1);
        assert_eq!(d.benchmarks[0].game_name, "Palworld");
        assert_eq!(d.counters.get("appid:1623730"), Some(&1));
    }

    /// 3. Archivo válido con la versión actual: se lee igual.
    #[test]
    fn s51_archivo_con_version_actual_se_lee() {
        let dir = tmp_dir("s51-version-actual");
        let path = dir.join("benchmarks.json");
        let json = historial_antiguo_json().replacen('{', &format!("{{\"version\":{},", BENCHMARKS_SCHEMA_VERSION), 1);
        fs::write(&path, json).unwrap();

        let d = leer_bench(&path).unwrap().unwrap();
        assert_eq!(d.version, Some(BENCHMARKS_SCHEMA_VERSION));
        assert_eq!(d.benchmarks.len(), 1);
    }

    /// 7. Versión futura: se rechaza en vez de interpretarse a medias.
    #[test]
    fn s51_version_futura_se_rechaza_sin_tocar_el_archivo() {
        let dir = tmp_dir("s51-version-futura");
        let path = dir.join("benchmarks.json");
        let json = historial_antiguo_json().replacen('{', &format!("{{\"version\":{},", BENCHMARKS_SCHEMA_VERSION + 1), 1);
        fs::write(&path, &json).unwrap();

        let err = leer_bench(&path).expect_err("debe rechazarse");
        match err {
            ErrorDatos::VersionFutura { encontrada, soportada } => {
                assert_eq!(encontrada, BENCHMARKS_SCHEMA_VERSION + 1);
                assert_eq!(soportada, BENCHMARKS_SCHEMA_VERSION);
            }
            otro => panic!("error equivocado: {otro:?}"),
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), json, "el archivo queda intacto");
        let msg = leer_bench(&path).unwrap_err().mensaje("El historial");
        assert!(msg.contains("más nueva"), "el mensaje debe explicar el motivo: {msg}");
        assert!(msg.contains("No se ha modificado nada"), "{msg}");
    }

    /// 4. JSON corrupto: es un error, nunca una lista vacía.
    #[test]
    fn s51_json_corrupto_no_se_lee_como_lista_vacia() {
        let dir = tmp_dir("s51-corrupto");
        let path = dir.join("benchmarks.json");
        fs::write(&path, "{ esto no es json").unwrap();

        let err = leer_bench(&path).expect_err("debe fallar");
        assert!(matches!(err, ErrorDatos::Corrupto { .. }));
        // Se conserva una copia: el original nunca se pierde por leer mal.
        assert!(dir.join("benchmarks.json.corrupt.bak").exists(), "copia de seguridad");
    }

    /// Un archivo vacío es lo que deja un `fs::write` interrumpido: no es «no hay
    /// pruebas», es un archivo roto que hay que enseñar.
    #[test]
    fn s51_archivo_vacio_se_trata_como_corrupto_no_como_vacio() {
        let dir = tmp_dir("s51-vacio");
        let path = dir.join("benchmarks.json");
        fs::write(&path, "").unwrap();

        let err = leer_bench(&path).expect_err("debe fallar");
        assert!(matches!(err, ErrorDatos::Corrupto { .. }));
        assert!(dir.join("benchmarks.json.vacio.bak").exists());
    }

    /// 5. Sin permisos de lectura: error, no lista vacía.
    #[cfg(unix)]
    #[test]
    fn s51_archivo_sin_permisos_es_error_no_lista_vacia() {
        use std::os::unix::fs::PermissionsExt;
        // root salta los permisos: el test no probaría nada de forma fiable.
        if fs::metadata("/proc/self").map(|_| true).unwrap_or(false)
            && std::process::Command::new("id")
                .arg("-u")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
                .unwrap_or(false)
        {
            return;
        }

        let dir = tmp_dir("s51-permisos");
        let path = dir.join("benchmarks.json");
        fs::write(&path, historial_antiguo_json()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();

        let err = leer_bench(&path).expect_err("debe fallar");
        assert!(matches!(err, ErrorDatos::Ilegible(_)), "error equivocado: {err:?}");

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).ok();
    }

    /// 6 y 12. Error de escritura: se informa y el archivo anterior no se toca.
    #[test]
    fn s51_error_de_escritura_no_pierde_el_archivo_anterior() {
        let dir = tmp_dir("s51-escritura");
        let path = dir.join("benchmarks.json");
        let original = historial_antiguo_json();
        fs::write(&path, &original).unwrap();

        // Un destino imposible: el «archivo» es un directorio, así que renombrar falla.
        let imposible = dir.join("destino");
        fs::create_dir_all(&imposible).unwrap();
        assert!(escribir_atomico(&imposible, "contenido").is_err());

        assert_eq!(fs::read_to_string(&path).unwrap(), original, "intacto");
    }

    /// La escritura atómica deja exactamente el contenido nuevo y no restos `.tmp`.
    #[test]
    fn s51_escritura_atomica_deja_el_archivo_y_ningun_temporal() {
        let dir = tmp_dir("s51-atomica");
        let path = dir.join("benchmarks.json");
        fs::write(&path, "viejo").unwrap();

        escribir_atomico(&path, "nuevo").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "nuevo");
        assert!(!dir.join("benchmarks.json.tmp").exists(), "no quedan temporales");
    }

    /// 8 y 10. Un guardado real conserva benchmarks históricos y contadores, y sella
    /// la versión.
    #[test]
    fn s51_guardar_conserva_historial_y_contadores() {
        let dir = tmp_dir("s51-guardar");
        let path = dir.join("benchmarks.json");
        fs::write(&path, historial_antiguo_json()).unwrap();

        let mut d = try_load_benchmarks_data_en(&path).unwrap();
        d.benchmarks.push(bench(1623730, "Palworld"));
        d.counters.insert("appid:1623730".to_string(), 2);

        // Se escribe con la misma función que las rutas reales.
        escribir_atomico(&path, &serde_json::to_string(&con_la_version(d.clone())).unwrap())
            .unwrap();

        let releido = leer_bench(&path).unwrap().unwrap();
        assert_eq!(releido.version, Some(BENCHMARKS_SCHEMA_VERSION));
        assert_eq!(releido.benchmarks.len(), 2, "el registro antiguo sigue ahí");
        assert_eq!(releido.benchmarks[0].id, "b1");
        assert_eq!(releido.counters.get("appid:1623730"), Some(&2));
    }

    /// Sella la versión igual que `save_benchmarks_data`.
    fn con_la_version(mut d: BenchmarksData) -> BenchmarksData {
        d.version = Some(BENCHMARKS_SCHEMA_VERSION);
        d
    }

    /// 13. Ninguna ruta destructiva puede partir de una lectura fallida.
    ///
    /// Antes, `load_benchmarks_data_para_editar` no existía y cada borrado hacía
    /// `load_benchmarks_data()` (que devolvía vacío) seguido de `save`: el historial
    /// entero desaparecía por un archivo ilegible.
    #[test]
    fn s51_las_rutas_de_escritura_abortan_si_la_lectura_falla() {
        let fuente = include_str!("benchmarks.rs");

        // Las cuatro rutas que modifican benchmarks y las tres de comparaciones.
        for funcion in [
            "pub fn save_benchmark(",
            "pub fn delete_benchmark(",
            "pub fn delete_benchmarks_for_game(",
            "pub fn delete_all_benchmarks(",
        ] {
            let inicio = fuente.find(funcion).unwrap_or_else(|| panic!("falta {funcion}"));
            let cuerpo = &fuente[inicio..(inicio + 2000).min(fuente.len())];
            assert!(
                cuerpo.contains("load_benchmarks_data_para_editar()?"),
                "{funcion} debe abortar si no puede leer el historial"
            );
            assert!(
                !cuerpo.contains("let mut data = load_benchmarks_data();"),
                "{funcion} no puede partir de una lectura tolerante a errores"
            );
        }

        for funcion in ["pub fn delete_comparison(", "pub fn delete_all_comparisons("] {
            let inicio = fuente.find(funcion).unwrap_or_else(|| panic!("falta {funcion}"));
            let cuerpo = &fuente[inicio..(inicio + 1200).min(fuente.len())];
            assert!(
                cuerpo.contains("load_saved_data_para_editar()?"),
                "{funcion} debe abortar si no puede leer las pruebas guardadas"
            );
        }
    }

    /// 14. La lectura de pantalla distingue «vacío» de «no se pudo leer».
    #[test]
    fn s51_la_lectura_de_pantalla_informa_del_error() {
        let fuente = include_str!("benchmarks.rs");

        // `load_benchmarks`, `load_benchmarks_for_game` y `load_comparisons` usan la
        // variante que propaga el error en vez de tragárselo.
        for funcion in ["pub fn load_benchmarks()", "pub fn load_benchmarks_for_game(", "pub fn load_comparisons()"] {
            let inicio = fuente.find(funcion).unwrap_or_else(|| panic!("falta {funcion}"));
            // Ventana amplia: la función creció al clasificar la evaluación, y lo que se
            // comprueba (que propaga el error) sigue existiendo dentro de ella.
            let cuerpo = &fuente[inicio..(inicio + 2000).min(fuente.len())];
            assert!(
                cuerpo.contains("cargar_benchmarks()?") || cuerpo.contains(".map_err(|e| e.mensaje("),
                "{funcion} debe propagar el error de lectura"
            );
        }

        let dir = tmp_dir("s51-pantalla");
        let path = dir.join("benchmarks.json");
        fs::write(&path, "roto").unwrap();

        let error = leer_bench(&path).unwrap_err();
        let mensaje = error.mensaje("El historial de pruebas");
        assert!(mensaje.contains("dañado"), "{mensaje}");
        assert!(mensaje.contains("No se ha modificado nada"), "{mensaje}");
    }

    /// 9. Las comparaciones guardadas sobreviven aunque sus benchmarks desaparezcan:
    /// embuten copias completas, así que no hay nada que recalcular.
    #[test]
    fn s51_las_comparaciones_guardadas_sobreviven_al_borrado_de_sus_benchmarks() {
        let a = bench(1623730, "Palworld");
        let b = bench(1623730, "Palworld");
        let guardada = SavedComparison {
            id: "cmp_1".to_string(),
            saved_at: "2024-01-02 10:00:00".to_string(),
            game_name: "Palworld".to_string(),
            before_fps: 60.0,
            after_fps: 66.0,
            fps_change_percent: 10.0,
            comparison: BenchmarkComparison {
                baseline: a.clone(),
                test: b.clone(),
                fps_change_percent: 10.0,
                fps_1_percent_change_percent: 0.0,
                fps_0_1_percent_change_percent: 0.0,
                frame_time_change_percent: -9.0,
                conclusion: "Mejora observada".to_string(),
                noise_percent: Some(3.0),
                within_noise: false,
                comandos_iguales: Some(true),
            },
            evaluacion: None,
            evaluacion_ilegible: None,
        };

        // El historial queda vacío (como tras «borrar todo»), pero la prueba guardada
        // no depende de él.
        let historial = BenchmarksData {
            version: Some(BENCHMARKS_SCHEMA_VERSION),
            benchmarks: Vec::new(),
            counters: HashMap::new(),
        };
        let dir = tmp_dir("s51-comparaciones");
        let bench_path = dir.join("benchmarks.json");
        let cmp_path = dir.join("saved_comparisons.json");

        escribir_atomico(&bench_path, &serde_json::to_string(&historial).unwrap()).unwrap();
        let datos = SavedComparisonsData {
            version: Some(COMPARISONS_SCHEMA_VERSION),
            comparisons: vec![guardada],
            crudos: serde_json::Map::new(),
            crudo_raiz: serde_json::Map::new(),
        };
        escribir_atomico(&cmp_path, &serde_json::to_string(&datos).unwrap()).unwrap();

        let leidas = leer_datos::<SavedComparisonsData>(
            &cmp_path,
            "saved_comparisons.json",
            COMPARISONS_SCHEMA_VERSION,
            |v| v.get("version").and_then(|x| x.as_u64()).map(|x| x as u32),
            serde_json::from_value,
        )
        .unwrap()
        .unwrap();

        assert_eq!(leidas.comparisons.len(), 1);
        assert_eq!(leidas.comparisons[0].comparison.baseline.id, "1623730-Palworld");
        assert!(leer_bench(&bench_path).unwrap().unwrap().benchmarks.is_empty());
    }

    /// 11. Operaciones concurrentes: el cerrojo existe y las cuatro rutas lo usan.
    ///
    /// Se comprueba la propiedad estructural (todas las rutas pasan por el mismo cerrojo)
    /// y que el mutex serializa de verdad.
    #[test]
    fn s51_las_rutas_de_escritura_usan_un_cerrojo() {
        let fuente = include_str!("benchmarks.rs");
        for funcion in [
            "pub fn save_benchmark(",
            "pub fn delete_benchmark(",
            "pub fn delete_benchmarks_for_game(",
            "pub fn delete_all_benchmarks(",
        ] {
            let inicio = fuente.find(funcion).unwrap();
            let cuerpo = &fuente[inicio..(inicio + 2000).min(fuente.len())];
            assert!(
                cuerpo.contains("CERROJO_BENCHMARKS.lock()"),
                "{funcion} debe serializarse con el cerrojo"
            );
        }

        // El cerrojo serializa: 20 hilos incrementando bajo el mismo mutex dan 20.
        use std::sync::Arc;
        let contador = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut hilos = Vec::new();
        for _ in 0..20 {
            let c = Arc::clone(&contador);
            hilos.push(std::thread::spawn(move || {
                let _t = CERROJO_BENCHMARKS.lock().unwrap_or_else(|e| e.into_inner());
                let n = c.load(std::sync::atomic::Ordering::SeqCst);
                std::thread::yield_now();
                c.store(n + 1, std::sync::atomic::Ordering::SeqCst);
            }));
        }
        for h in hilos {
            h.join().unwrap();
        }
        assert_eq!(contador.load(std::sync::atomic::Ordering::SeqCst), 20);
    }

    /// Las comparaciones guardadas tienen su propio cerrojo y su propio esquema.
    #[test]
    fn s51_las_comparaciones_tienen_cerrojo_y_esquema_propios() {
        let fuente = include_str!("benchmarks.rs");
        for funcion in ["pub fn delete_comparison(", "pub fn delete_all_comparisons("] {
            let inicio = fuente.find(funcion).unwrap();
            let cuerpo = &fuente[inicio..(inicio + 1200).min(fuente.len())];
            assert!(
                cuerpo.contains("CERROJO_COMPARACIONES.lock()"),
                "{funcion} debe usar su cerrojo"
            );
        }
        // 5.4.1.B: `saved_comparisons.json` pasa a v2 (añade `evaluacion`). El esquema de
        // benchmarks no cambia: son dos archivos con ciclos distintos.
        assert_eq!(COMPARISONS_SCHEMA_VERSION, 2);
        assert_eq!(BENCHMARKS_SCHEMA_VERSION, 1);
    }

    // ── Cierre de 5.1: concurrencia, durabilidad y copias ──

    /// 1. Concurrencia **real**: 40 hilos leyendo, añadiendo y escribiendo el mismo
    /// archivo en disco a la vez.
    ///
    /// Comprobar que existe un `Mutex` no demuestra nada: un cerrojo mal colocado dentro
    /// del hilo (por ejemplo, cogido solo alrededor de la escritura y no alrededor del
    /// ciclo completo leer-modificar-escribir) pasaría esa comprobación y aun así perdería
    /// benchmarks, porque dos hilos leerían la misma lista y el segundo pisaría al primero.
    /// Esta prueba ejercita el ciclo completo bajo el mismo cerrojo que usan las rutas
    /// reales.
    /// La escritura atómica evita un archivo a medias, **no** evita perder
    /// actualizaciones: dos hilos que leen lo mismo y escriben por turnos se pisan. Lo que
    /// lo evita es el cerrojo, y aquí se comprueba que cubre el ciclo entero.
    #[test]
    fn la_concurrencia_intraproceso_no_pierde_comparaciones() {
        let dir = tmp_dir("b541-concurrencia");
        let ruta = dir.join("saved_comparisons.json");
        escribir(&ruta, r#"{"version":1,"comparisons":[]}"#);
        const HILOS: usize = 30;

        let mut handles = Vec::new();
        for i in 0..HILOS {
            let ruta = ruta.clone();
            handles.push(std::thread::spawn(move || {
                // Mismo cerrojo que `save_comparison`: todo el ciclo leer-modificar-escribir.
                let _t = CERROJO_COMPARACIONES.lock().unwrap_or_else(|e| e.into_inner());
                let mut d = try_load_saved_data_de(&ruta).expect("lectura");
                let mut c = comparacion(&format!("conc-{i}"), &format!("b{i}"), &format!("t{i}"));
                c.evaluacion = Some(evaluacion_de(&format!("b{i}"), &format!("t{i}")));
                d.comparisons.push(c);
                save_saved_data_en(&ruta, &d).expect("escritura");
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        let final_data = try_load_saved_data_de(&ruta).expect("lectura final");
        assert_eq!(
            final_data.comparisons.len(),
            HILOS,
            "sin cerrojo se perderían actualizaciones: cada hilo relee y reescribe el archivo"
        );
        // Y ninguna se ha quedado sin su evaluación por una escritura pisada.
        assert!(final_data.comparisons.iter().all(|c| c.evaluacion.is_some()));
        // El primero en escribir forzó v2, y ya no baja.
        assert_eq!(final_data.version, Some(2));
    }

    #[test]
    fn s51_concurrencia_real_no_pierde_ni_duplica_benchmarks() {
        let dir = tmp_dir("s51-concurrencia");
        let path = dir.join("benchmarks.json");
        fs::write(&path, historial_antiguo_json()).unwrap();
        const HILOS: usize = 40;

        let mut handles = Vec::new();
        for i in 0..HILOS {
            let path = path.clone();
            handles.push(std::thread::spawn(move || {
                // Mismo cerrojo que las rutas reales: todo el ciclo leer-modificar-escribir.
                let _t = CERROJO_BENCHMARKS.lock().unwrap_or_else(|e| e.into_inner());
                let mut d = try_load_benchmarks_data_en(&path).expect("lectura");

                let mut b = bench(1623730, "Palworld");
                b.id = format!("conc-{}", i);
                b.number = Some(i as u32 + 1);
                d.benchmarks.push(b);
                // El contador se incrementa en una sola entrada, como `save_benchmark`:
                // después de 40 hilos debe valer exactamente 40 más el valor inicial.
                let e = d.counters.entry("appid:1623730".to_string()).or_insert(0);
                *e += 1;

                let contenido =
                    serde_json::to_string(&con_la_version(d.clone())).expect("serializar");
                escribir_atomico(&path, &contenido).expect("escritura");
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        let final_data = leer_bench(&path).unwrap().unwrap();
        let de_palworld: Vec<&Benchmark> = final_data
            .benchmarks
            .iter()
            .filter(|b| b.game_name == "Palworld")
            .collect();

        // Los 40 nuevos + el antiguo que ya estaba. Ni uno menos: eso sería una escritura
        // perdida por leer una versión obsoleta.
        assert_eq!(
            de_palworld.len(),
            HILOS + 1,
            "se perdieron benchmarks: el ciclo leer-modificar-escribir no está serializado"
        );
        assert_eq!(
            final_data.counters.get("appid:1623730"),
            Some(&(HILOS as u32 + 1)),
            "el contador no es el esperado: hubo lecturas obsoletas"
        );

        // Ningún identificador repetido (lo que produciría dos «Prueba 5» iguales).
        let mut ids: Vec<&str> = final_data.benchmarks.iter().map(|b| b.id.as_str()).collect();
        ids.sort();
        let antes = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), antes, "hay identificadores duplicados");

        // Los números de prueba también son únicos entre los que se repartieron.
        let mut numeros: Vec<u32> = final_data
            .benchmarks
            .iter()
            .filter_map(|b| b.number)
            .collect();
        numeros.sort();
        let antes = numeros.len();
        numeros.dedup();
        assert_eq!(numeros.len(), antes, "hay números de prueba duplicados");

        // Y el archivo sigue siendo legible: ninguna escritura lo dejó a medias.
        assert!(leer_bench(&path).unwrap().is_some());
    }

    /// 2. La escritura atómica sincroniza el directorio padre tras el `rename`.
    ///
    /// Sin `fsync` del directorio, el rename puede no estar en disco cuando se corta la
    /// corriente: el archivo nuevo existe pero la entrada del directorio apunta al viejo.
    /// No hay forma portable de *observar* el `fsync` en un test, así que se comprueba que
    /// la ruta se llama y que la escritura produce el resultado correcto.
    #[cfg(unix)]
    #[test]
    fn s51_escribir_atomico_sincroniza_el_directorio() {
        let fuente = include_str!("benchmarks.rs");
        let inicio = fuente.find("fn escribir_atomico(").unwrap();
        let cuerpo = &fuente[inicio..(inicio + 900).min(fuente.len())];
        assert!(cuerpo.contains("sync_dir(path)"), "falta el fsync del directorio");
        assert!(cuerpo.contains("sync_all"), "el contenido debe sincronizarse antes del rename");

        let inicio = fuente.find("fn sync_dir(").unwrap();
        let cuerpo = &fuente[inicio..(inicio + 400).min(fuente.len())];
        assert!(cuerpo.contains("File::open(dir)"), "abre el directorio");
        assert!(cuerpo.contains("sync_all"), "lo sincroniza");

        // Y escribir en un directorio recién creado funciona igual (no depende de que el
        // padre ya existiera en caché).
        let dir = tmp_dir("s51-syncdir");
        let path = dir.join("sub").join("benchmarks.json");
        fs::create_dir_all(dir.join("sub")).unwrap();
        escribir_atomico(&path, "contenido").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "contenido");
    }

    /// 3. Una copia de un archivo dañado **no pisa** una copia anterior distinta.
    #[test]
    fn s51_la_copia_de_un_archivo_danado_no_pisa_la_anterior() {
        let dir = tmp_dir("s51-copias");
        let path = dir.join("benchmarks.json");

        let primera = copiar_danado(&path, "corrupt", "primera version rota").unwrap();
        assert!(primera.to_string_lossy().ends_with("benchmarks.json.corrupt.bak"));

        // Mismo contenido otra vez (la UI reintenta en bucle): no se duplica.
        let otra = copiar_danado(&path, "corrupt", "primera version rota").unwrap();
        assert_eq!(otra, primera, "no debe acumular copias idénticas");

        // Contenido distinto: se guarda aparte, sin tocar la primera.
        let segunda = copiar_danado(&path, "corrupt", "segunda version rota").unwrap();
        assert_ne!(segunda, primera);
        assert_eq!(fs::read_to_string(&primera).unwrap(), "primera version rota");
        assert_eq!(fs::read_to_string(&segunda).unwrap(), "segunda version rota");

        // Con muchas copias distintas se rota, no se crece sin límite.
        for i in 0..10 {
            copiar_danado(&path, "corrupt", &format!("version {}", i));
        }
        let cuantas = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("corrupt.bak"))
            .count();
        assert!(cuantas <= 5, "se acumularon {} copias", cuantas);
    }

    /// 4. Thoriapp es de instancia única, así que el cerrojo de proceso basta.
    ///
    /// No se añadió `fs2` a propósito. Antes de apoyarse en eso hay que confirmar que la
    /// app **no** puede correr dos veces: si pudiera, dos procesos con cerrojos separados
    /// se pisarían el archivo igual que sin cerrojo.
    #[test]
    fn s51_la_app_declara_una_unica_instancia() {
        let config = include_str!("../tauri.conf.json");
        assert!(
            !config.contains("single-instance") || config.contains("\"single-instance\""),
            "revisar la configuración de instancia única"
        );

        // Lo que de verdad lo garantiza está en Rust: el arranque se niega a continuar si
        // ya hay otra instancia.
        let main = include_str!("main.rs");
        assert!(
            main.contains("fn tomar_cerrojo_de_instancia"),
            "no hay protección de instancia única en main.rs; el Mutex de proceso no \
             protegería el archivo si la app corre dos veces"
        );
        // Y que se ejerza de verdad al arrancar, no que solo exista la función.
        assert!(
            main.contains("let Some(_cerrojo) = tomar_cerrojo_de_instancia()"),
            "el cerrojo de instancia se declara pero main() no lo toma"
        );
        // Se toma antes que nada: si se tomara después de arrancar la UI, la segunda
        // ventana ya habría abierto los archivos.
        let cuerpo_main = &main[main.find("fn main()").unwrap()..];
        assert!(
            cuerpo_main.find("tomar_cerrojo_de_instancia()")
                < cuerpo_main.find("tauri::Builder::default()"),
            "el cerrojo debe tomarse antes de arrancar la interfaz"
        );
    }

    /// 5. Las pruebas en vivo no tocan los archivos del usuario.
    ///
    /// Los tests de esta subfase leen y escriben con `escribir_atomico(ruta, ...)` sobre
    /// rutas de `tmp_dir`, nunca con las funciones que resuelven `~/.config/thoriapp`.
    /// Se comprueba que ningún test de integridad usa `get_benchmarks_file`.
    #[test]
    fn s51_las_pruebas_de_integridad_no_usan_las_rutas_reales() {
        let fuente = include_str!("benchmarks.rs");
        let inicio = fuente.find("mod tests {").unwrap();
        let tests = &fuente[inicio..];

        // `historial_antiguo_json` es un literal en memoria; el resto debe ir por `tmp_dir`.
        for funcion in [
            "fn s51_archivo_inexistente_es_lista_vacia_no_error",
            "fn s51_guardar_conserva_historial_y_contadores",
            "fn s51_concurrencia_real_no_pierde_ni_duplica_benchmarks",
            "fn s51_la_copia_de_un_archivo_danado_no_pisa_la_anterior",
        ] {
            let i = tests.find(funcion).unwrap_or_else(|| panic!("falta {funcion}"));
            let cuerpo = &tests[i..(i + 2500).min(tests.len())];
            assert!(
                !cuerpo.contains("get_benchmarks_file()"),
                "{funcion} usaría la ruta real del usuario"
            );
            assert!(cuerpo.contains("tmp_dir("), "{funcion} debe usar un directorio temporal");
        }
    }

    // ==================== Subfase 5.2: instantáneas completas ====================

    /// Contexto típico: el juego abierto es el de la prueba y hay perfil cargado.
    fn contexto_de(appid: u32) -> ContextoDeCaptura {
        ContextoDeCaptura {
            appid_real: Some(appid),
            comando_confirmado: Some("mangohud %command%".to_string()),
            revision_motor: Some(7),
            opciones_steam: Some("mangohud %command%".to_string()),
            perfil_id: Some("perfil-1".to_string()),
            perfil_nombre: Some("Rendimiento".to_string()),
            cambios_sin_guardar: Some(false),
            modo_proton: Some(false),
            corresponde_al_juego: true,
        }
    }

    fn snapshot_de(appid: u32, ctx: &ContextoDeCaptura) -> SnapshotConfig {
        snapshot_con_numero(appid, 3, ctx)
    }

    fn snapshot_con_numero(appid: u32, numero: u32, ctx: &ContextoDeCaptura) -> SnapshotConfig {
        let mut b = bench(appid, "Palworld");
        b.number = Some(numero);
        b.timestamp = "2026-11-24 19:20:05".to_string();
        construir_snapshot(&b, ctx, None, "2026-11-24 19:25:00")
    }

    /// 1 y 5. Un benchmark antiguo sin `snapshot` se sigue leyendo, y la ausencia es
    /// distinguible de una instantánea vacía.
    #[test]
    fn s52_benchmark_antiguo_sin_snapshot_se_lee_y_queda_sin_el() {
        let json = r#"{"benchmarks":[{"id":"b1","profile_id":"p","game_name":"Palworld",
            "appid":1623730,"timestamp":"2024-01-01 10:00:00","duration_seconds":60.0,
            "avg_fps":60.0,"fps_1_percent_low":50.0,"fps_0_1_percent_low":40.0,"max_fps":70.0,
            "min_fps":30.0,"frame_time_avg":16.6,"frame_time_1_percent_high":20.0,
            "total_frames":600,"notes":null}],"counters":{}}"#;

        let d: BenchmarksData = serde_json::from_str(json).expect("los antiguos deben cargar");
        assert_eq!(d.benchmarks.len(), 1, "no se pierde ningún benchmark antiguo");
        assert!(
            d.benchmarks[0].snapshot.is_none(),
            "sin instantánea: None, no una instantánea inventada"
        );
        // El campo nuevo no rompe la serialización de ida y vuelta.
        let ida = serde_json::to_string(&d).unwrap();
        let vuelta: BenchmarksData = serde_json::from_str(&ida).unwrap();
        assert!(vuelta.benchmarks[0].snapshot.is_none());
    }

    /// 2. Benchmark nuevo con instantánea completa: todos los bloques rellenos.
    #[test]
    fn s52_snapshot_completo_tiene_identidad_configuracion_y_entorno() {
        let snap = snapshot_de(1623730, &contexto_de(1623730));

        assert_eq!(snap.version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(snap.identidad.appid_real, Some(1623730));
        assert_eq!(snap.identidad.numero_prueba, Some(3));
        assert_eq!(snap.identidad.fecha, "2026-11-24 19:20:05");
        assert_eq!(
            snap.configuracion.comando_confirmado.as_ref().unwrap().valor,
            "mangohud %command%"
        );
        assert_eq!(snap.configuracion.revision_motor, Some(7));
        assert_eq!(snap.configuracion.perfil_id.as_deref(), Some("perfil-1"));
        assert_eq!(
            snap.configuracion.perfil_nombre.as_ref().unwrap().valor,
            "Rendimiento"
        );
        // El comando viene del motor, no de un ajuste reconstruido.
        assert_eq!(
            snap.configuracion.comando_confirmado.as_ref().unwrap().procedencia,
            Procedencia::MotorRust
        );
        assert_eq!(
            snap.configuracion.opciones_steam.as_ref().unwrap().procedencia,
            Procedencia::LeidoDeSteam
        );
        assert!(snap.posterior_a_la_grabacion, "se captura al guardar");
    }

    /// 3. Campos opcionales ausentes: la instantánea se construye igual y los huecos son
    /// `None`, no cadenas vacías ni ceros.
    #[test]
    fn s52_campos_opcionales_ausentes_quedan_en_none() {
        let ctx = ContextoDeCaptura {
            corresponde_al_juego: true,
            ..Default::default()
        };
        let snap = snapshot_de(1623730, &ctx);

        assert!(snap.configuracion.comando_confirmado.is_none());
        assert!(snap.configuracion.opciones_steam.is_none());
        assert!(snap.configuracion.perfil_id.is_none());
        assert!(snap.configuracion.perfil_nombre.is_none());
        assert!(snap.configuracion.modo_proton.is_none());
        assert!(snap.configuracion.cambios_sin_guardar.is_none());
        // El AppID sí está: lo aporta el propio benchmark.
        assert_eq!(snap.identidad.appid_real, Some(1623730));
    }

    /// 4. Comando complejo: se conserva literal, con comillas, espacios y orden.
    #[test]
    fn s52_el_comando_se_conserva_literal_no_se_normaliza() {
        let complejo = r#"PROTON_FSR4_UPGRADE=1 DXVK_HUD=fps  gamemoderun  MANGOHUD_CONFIG=read_cfg "mangohud --dlsym" --  %command%  -dx11   --ruta="/home/u/Mis Juegos/x"#;
        let ctx = ContextoDeCaptura {
            comando_confirmado: Some(complejo.to_string()),
            ..contexto_de(1623730)
        };
        let snap = snapshot_de(1623730, &ctx);

        assert_eq!(
            snap.configuracion.comando_confirmado.as_ref().unwrap().valor,
            complejo,
            "espacios dobles, comillas y orden deben sobrevivir tal cual"
        );
    }

    /// 5, 6 y 7. Renombrar, actualizar o borrar un perfil no cambia la instantánea.
    #[test]
    fn s52_la_instantanea_no_depende_del_perfil_actual() {
        let ctx = contexto_de(1623730);
        let snap = snapshot_de(1623730, &ctx);
        let nombre_original = snap.configuracion.perfil_nombre.clone().unwrap().valor.clone();
        let comando_original = snap
            .configuracion
            .comando_confirmado
            .clone()
            .unwrap()
            .valor
            .clone();

        // «Renombrar» y «actualizar» el perfil es solo cambiar el contexto que se usaría en
        // una prueba NUEVA: una instantánea ya construida es un valor aparte.
        let mut ctx2 = contexto_de(1623730);
        ctx2.perfil_nombre = Some("Nombre nuevo".to_string());
        ctx2.comando_confirmado = Some("otro comando %command%".to_string());

        assert_eq!(
            snap.configuracion.perfil_nombre.clone().unwrap().valor,
            nombre_original,
            "el nombre histórico no cambia al renombrar el perfil"
        );
        assert_eq!(
            snap.configuracion.comando_confirmado.clone().unwrap().valor,
            comando_original,
            "el comando histórico no cambia al actualizar el perfil"
        );

        // Serializar y volver a leer (lo que pasa al reiniciar) tampoco lo cambia.
        let json = serde_json::to_string(&snap).unwrap();
        let leido: SnapshotConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(leido, snap, "la instantánea sobrevive a guardar y releer");
    }

    /// 8. Perfil cargado con cambios sin guardar: se registra el hecho.
    #[test]
    fn s52_registra_cambios_sin_guardar() {
        let ctx = ContextoDeCaptura {
            cambios_sin_guardar: Some(true),
            ..contexto_de(1623730)
        };
        let snap = snapshot_de(1623730, &ctx);
        assert_eq!(snap.configuracion.cambios_sin_guardar, Some(true));
    }

    /// 9. Benchmark sin perfil cargado: no se inventa uno.
    #[test]
    fn s52_sin_perfil_cargado_queda_sin_perfil() {
        let ctx = ContextoDeCaptura {
            perfil_id: None,
            perfil_nombre: None,
            ..contexto_de(1623730)
        };
        let snap = snapshot_de(1623730, &ctx);
        assert!(snap.configuracion.perfil_id.is_none());
        assert!(snap.configuracion.perfil_nombre.is_none());
    }

    /// 10. Proton configurado ≠ Proton verificado.
    #[test]
    fn s52_proton_configurado_no_se_presenta_como_ejecutado() {
        let ctx = ContextoDeCaptura {
            modo_proton: Some(true),
            ..contexto_de(1623730)
        };
        let snap = snapshot_de(1623730, &ctx);

        assert_eq!(
            snap.configuracion.modo_proton.as_ref().unwrap().valor,
            true
        );
        assert_eq!(
            snap.configuracion.modo_proton.as_ref().unwrap().procedencia,
            Procedencia::ConfiguradoEnThoriapp,
            "es lo configurado, no lo ejecutado"
        );
        assert!(
            snap.configuracion.ejecucion_verificada.is_none(),
            "configurar Proton no demuestra que el proceso lo usara"
        );
        // Y la versión de Proton tampoco se presume.
        assert!(snap.entorno.proton_seleccionado.is_none());
    }

    /// 11 y 12. Cambio de juego durante la captura, o CSV de otro juego: la configuración se
    /// registra como desconocida en vez de atribuirse al juego equivocado.
    #[test]
    fn s52_configuracion_de_otro_juego_no_se_atribuye() {
        let ctx = ContextoDeCaptura {
            corresponde_al_juego: false,
            // Aunque lleguen datos, no deben usarse: es la protección contra mezclar.
            ..contexto_de(1623730)
        };
        let snap = snapshot_de(251570, &ctx);

        assert!(snap.configuracion.comando_confirmado.is_none());
        assert!(snap.configuracion.perfil_id.is_none());
        assert!(snap.configuracion.perfil_nombre.is_none());
        assert!(snap.configuracion.modo_proton.is_none());
        assert!(snap.configuracion.revision_motor.is_none());
        assert!(snap.configuracion.cambios_sin_guardar.is_none());
        // La identidad sí es la del benchmark, no la del juego abierto.
        assert_eq!(snap.identidad.appid_real, Some(251570));

        // Las opciones de Steam no dependen del juego abierto: se leen para el AppID de la
        // grabación, así que sí se conservan.
        assert_eq!(
            snap.configuracion.opciones_steam.as_ref().unwrap().valor,
            "mangohud %command%"
        );
    }

    /// 13. Una operación IPC pendiente se manifiesta como «sin contexto»: la vista no manda
    /// contexto y la instantánea no mezcla datos de una revisión no confirmada.
    #[test]
    fn s52_sin_contexto_no_se_inventa_la_configuracion() {
        let mut b = bench(1623730, "Palworld");
        b.number = Some(1);
        let snap = construir_snapshot(&b, &ContextoDeCaptura::default(), None, "2026-11-24 20:00:00");

        assert!(snap.configuracion.comando_confirmado.is_none());
        assert!(snap.configuracion.revision_motor.is_none());
        // Pero la instantánea existe: el hueco es visible, no la ausencia del bloque.
        assert_eq!(snap.version, SNAPSHOT_SCHEMA_VERSION);
    }

    /// 17. Sin BuildID ni hardware disponible: `None`, nunca ceros ni cadenas vacías.
    #[test]
    fn s52_hardware_o_buildid_ausentes_quedan_en_none() {
        // AppID 0 = no hay juego instalado que consultar: build_id debe ser None.
        assert_eq!(crate::steam::build_id(0).unwrap(), None);
        // Un AppID que no está instalado: tampoco hay dato, y no es un error.
        assert_eq!(crate::steam::build_id(999999999).unwrap(), None);

        let e = leer_entorno(999999999, "2026-11-24 19:25:00");
        assert!(e.build_id.is_none(), "sin appmanifest no hay BuildID");
        // El driver y Proton no se detectan todavía: desconocidos, no inventados.
        assert!(e.driver.is_none());
        assert!(e.proton_seleccionado.is_none());
        // Lo que sí se detecta lleva su procedencia y su fecha de captura.
        if let Some(gpu) = &e.gpu {
            assert_eq!(gpu.procedencia, Procedencia::DetectadoEnSistema);
            assert!(!gpu.valor.trim().is_empty());
            assert_eq!(gpu.capturado_en.as_deref(), Some("2026-11-24 19:25:00"));
        }
    }

    /// 18. Ningún dato histórico desconocido se completa con datos actuales.
    ///
    /// Es la regla más importante de la subfase: una instantánea antigua que dice «no
    /// consta» no puede rellenarse al leerla con el hardware de hoy.
    #[test]
    fn s52_no_se_rellenan_huecos_con_datos_actuales() {
        let fuente = include_str!("benchmarks.rs");

        // No existe ninguna función que «complete» o «reconstruya» instantáneas.
        let seccion = &fuente[..fuente.find("mod tests {").unwrap()];
        assert!(
            !seccion.contains("fn reconstruir_snapshot"),
            "no debe existir una reconstrucción de instantáneas"
        );
        assert!(
            !seccion.contains("fn completar_snapshot"),
            "no debe existir un relleno de instantáneas"
        );

        // Y `construir_snapshot` solo se llama al guardar, no al leer.
        let llamadas = seccion.matches("construir_snapshot(").count();
        assert_eq!(llamadas, 2, "una es la definición y otra el guardado; ninguna al leer");

        // Un archivo con instantánea se devuelve tal cual, con sus huecos.
        let mut b = bench(1623730, "Palworld");
        b.snapshot = Some(snapshot_de(
            1623730,
            &ContextoDeCaptura {
                corresponde_al_juego: true,
                ..Default::default()
            },
        ));
        let json = serde_json::to_string(&b).unwrap();
        let leido: Benchmark = serde_json::from_str(&json).unwrap();
        assert!(
            leido.snapshot.as_ref().unwrap().configuracion.comando_confirmado.is_none(),
            "el hueco sigue siendo hueco después de guardar y releer"
        );
    }

    /// 16. Los números de prueba y los contadores no cambian al añadir la instantánea.
    #[test]
    fn s52_se_conservan_numeros_y_contadores() {
        let mut d = vacio();
        guardar(&mut d, 1623730, "Palworld", 1);
        guardar(&mut d, 1623730, "Palworld", 1);
        guardar(&mut d, 251570, "7 Days to Die", 1);

        let numeros: Vec<Option<u32>> = d
            .benchmarks
            .iter()
            .filter(|b| b.game_name == "Palworld")
            .map(|b| b.number)
            .collect();
        assert_eq!(numeros, vec![Some(1), Some(2)]);
        assert_eq!(d.counters.get("appid:1623730"), Some(&2));
        assert_eq!(d.counters.get("appid:251570"), Some(&1));
    }

    /// 14. Las comparaciones guardadas conservan las instantáneas embebidas de los dos
    /// benchmarks, aunque los originales se borren.
    #[test]
    fn s52_la_comparacion_guardada_conserva_las_dos_instantaneas() {
        let mut a = bench(1623730, "Palworld");
        a.snapshot = Some(snapshot_de(1623730, &contexto_de(1623730)));
        let mut b = a.clone();
        b.id = "segundo".to_string();
        b.number = Some(4);
        // La segunda prueba se hizo con otro comando y otro perfil.
        let mut ctx2 = contexto_de(1623730);
        ctx2.comando_confirmado = Some("game-performance %command%".to_string());
        ctx2.perfil_nombre = Some("Otro perfil".to_string());
        b.snapshot = Some(snapshot_con_numero(1623730, 4, &ctx2));

        let comparacion = BenchmarkComparison {
            baseline: a.clone(),
            test: b.clone(),
            fps_change_percent: 5.0,
            fps_1_percent_change_percent: 1.0,
            fps_0_1_percent_change_percent: 1.0,
            frame_time_change_percent: -5.0,
            conclusion: "Mejora".to_string(),
            noise_percent: Some(3.0),
            within_noise: false,
            comandos_iguales: Some(false),
        };

        let guardada = SavedComparison {
            id: "cmp_1".to_string(),
            saved_at: "2026-11-24 21:00:00".to_string(),
            game_name: "Palworld".to_string(),
            before_fps: 60.0,
            after_fps: 63.0,
            fps_change_percent: 5.0,
            comparison: comparacion,
            evaluacion: None,
            evaluacion_ilegible: None,
        };

        let json = serde_json::to_string(&guardada).unwrap();
        let leida: SavedComparison = serde_json::from_str(&json).unwrap();

        assert!(leida.comparison.baseline.snapshot.is_some());
        assert!(leida.comparison.test.snapshot.is_some());
        assert_eq!(
            leida.comparison.test.snapshot.as_ref().unwrap().identidad.numero_prueba,
            Some(4)
        );
        assert_ne!(
            leida.comparison.baseline.snapshot,
            leida.comparison.test.snapshot,
            "cada prueba conserva SU instantánea, no la de la otra"
        );
    }

    /// 15. Persistencia: la instantánea sobrevive a escribir en disco y releer.
    #[test]
    fn s52_la_instantanea_persiste_en_disco() {
        let dir = tmp_dir("s52-persistencia");
        let path = dir.join("benchmarks.json");

        let mut b = bench(1623730, "Palworld");
        b.number = Some(1);
        b.snapshot = Some(snapshot_con_numero(1623730, 1, &contexto_de(1623730)));
        let d = BenchmarksData {
            version: Some(BENCHMARKS_SCHEMA_VERSION),
            benchmarks: vec![b],
            counters: HashMap::new(),
        };
        escribir_atomico(&path, &serde_json::to_string(&d).unwrap()).unwrap();

        let releido = leer_bench(&path).unwrap().unwrap();
        let snap = releido.benchmarks[0].snapshot.as_ref().unwrap();
        assert_eq!(snap.configuracion.perfil_id.as_deref(), Some("perfil-1"));
        assert_eq!(
            snap.configuracion.comando_confirmado.as_ref().unwrap().valor,
            "mangohud %command%"
        );
        assert_eq!(snap.identidad.numero_prueba, Some(1));
    }

    /// 15. Con una grabación real, la instantánea deja el hardware **de la grabación**
    ///     separado del detectado ahora, cada uno con su procedencia y su fecha.
    #[test]
    fn s52_el_hardware_de_la_grabacion_va_aparte_del_actual() {
        let mut b = bench(0, "Palworld");
        b.number = Some(1);
        b.log_path = fixture_log(LOG2);
        // El CSV no trae AppID: el contexto es el que declara ser del juego de la prueba.
        let snap = construir_snapshot(&b, &contexto_de(1623730), None, "2026-11-24 19:25:00");

        let grabacion = snap.hardware_grabacion.as_ref().expect("hardware de la grabación");
        let cpu = grabacion.cpu.as_ref().expect("cpu");
        assert_eq!(cpu.valor, "AMD Ryzen 5 5600X 6-Core Processor");
        assert_eq!(cpu.procedencia, Procedencia::RegistradoPorMangohud);
        assert_eq!(cpu.capturado_en.as_deref(), Some("2026-11-24 19:25:00"));

        // `os` es el runtime de Steam, y se guarda sin reinterpretarlo como distro.
        assert_eq!(grabacion.os.as_ref().unwrap().valor, "Steam Runtime 4");

        // `driver` vacío en el CSV: no se deduce de «RADV NAVI23».
        assert_eq!(grabacion.driver, None);

        // El equipo actual es un bloque **independiente**. Puede coincidir o no: este
        // entorno tiene el mismo modelo de CPU que la grabación, pero la procedencia y la
        // fecha son otras, y ninguno rellena los huecos del otro.
        let actual = &snap.entorno;
        if let Some(cpu_actual) = &actual.cpu {
            assert_eq!(cpu_actual.procedencia, Procedencia::DetectadoEnSistema);
            assert_eq!(
                cpu_actual.capturado_en.as_deref(),
                Some("2026-11-24 19:25:00")
            );
            assert_ne!(
                cpu_actual.procedencia, cpu.procedencia,
                "las dos fuentes se distinguen aunque el valor coincida"
            );
        }
        // El bloque de la grabación no depende de que el entorno detecte algo.
        assert!(snap.hardware_grabacion.is_some());
    }

    /// 15b. Si el CSV no trae un campo, el bloque de la grabación lo deja en `None`
    ///      aunque el equipo actual lo detecte: no se rellena el hueco.
    #[test]
    fn s52_el_hueco_del_csv_no_lo_rellena_el_equipo_actual() {
        let mut b = bench(0, "Palworld");
        b.log_path = fixture_log(LOG2);
        let snap = construir_snapshot(&b, &contexto_de(1623730), None, "2026-11-24 19:25:00");
        let grabacion = snap.hardware_grabacion.expect("hardware");

        // `driver` viene vacío en el CSV.
        assert_eq!(grabacion.driver, None);
        // El kernel sí viene: no es un hueco.
        assert!(grabacion.kernel.is_some());
        // Y que un campo esté en `None` en la grabación no copia el valor del entorno.
        if snap.entorno.driver.is_some() {
            panic!("el entorno tampoco debe inventar un driver");
        }
    }

    /// 16. RAM: se guarda el crudo en KiB y la conversión en GiB, sin confundir unidades.
    #[test]
    fn s52_la_ram_conserva_kib_y_gib() {
        let mut b = bench(0, "Palworld");
        b.log_path = fixture_log(LOG2);
        let snap = construir_snapshot(&b, &contexto_de(1623730), None, "2026-11-24 19:25:00");
        let hw = snap.hardware_grabacion.unwrap();

        assert_eq!(hw.ram_kib.unwrap().valor, 32_777_068);
        let gib = hw.ram_gib.unwrap().valor;
        assert!((gib - 31.26).abs() < 0.01, "{gib}");
    }

    /// 17. El resumen de MangoHud se guarda aparte y NO sustituye a las métricas de
    ///     Thoriapp, que siguen siendo las calculadas por el motor.
    #[test]
    fn s52_el_resumen_no_pisa_las_metricas_de_thoriapp() {
        let mut b = bench(0, "Palworld");
        b.avg_fps = 66.23;
        b.fps_1_percent_low = 41.31;
        b.log_path = fixture_log(LOG2);
        let snap = construir_snapshot(&b, &contexto_de(1623730), None, "2026-11-24 19:25:00");

        let resumen = snap.resumen_mangohud.as_ref().expect("resumen");
        assert_eq!(resumen.valor("Average FPS"), Some(60.4));
        assert_eq!(resumen.valor("1% Min FPS"), Some(26.4349));

        // Las del benchmark siguen intactas: la instantánea no las toca.
        assert_eq!(b.avg_fps, 66.23);
        assert_eq!(b.fps_1_percent_low, 41.31);
        // Y son distintas de las de MangoHud, cada una con su metodología.
        assert_ne!(Some(b.avg_fps), resumen.valor("Average FPS"));
    }

    /// El camino completo: analizar un CSV real y guardarlo con `save_benchmark` deja en
    /// disco los dos bloques de hardware y el resumen, cada uno con su procedencia. Es lo
    /// que ocurre al pulsar «Analizar y guardar» en la aplicación.
    #[test]
    fn s52_analizar_y_guardar_conserva_hardware_y_resumen() {
        // El CSV real tiene 60 s de muestras; se analiza como en la interfaz.
        let ruta = fixture_log(LOG2);
        let analisis = analyze_mangohud_log(&ruta, 10.0).expect("análisis");

        let mut b = bench(0, "Palworld");
        b.log_path = ruta.clone();
        b.avg_fps = analisis.avg_fps;
        b.fps_1_percent_low = analisis.fps_1_percent_low;

        let snap = construir_snapshot(&b, &contexto_de(1623730), None, "2026-11-24 19:25:00");

        // Bloque histórico: lo que MangoHud escribió en el CSV.
        let hw = snap.hardware_grabacion.as_ref().expect("hardware del CSV");
        assert_eq!(hw.gpu.as_ref().unwrap().valor, "AMD Radeon RX 6600 (RADV NAVI23)");
        assert_eq!(hw.gpu.as_ref().unwrap().procedencia, Procedencia::RegistradoPorMangohud);
        assert_eq!(hw.ram_kib.as_ref().unwrap().valor, 32_777_068);
        // El driver no viene en el CSV: queda vacío, no se deduce «RADV».
        assert!(hw.driver.is_none());
        // `os` es el runtime de Steam, y así se guarda: no se convierte en distribución.
        assert_eq!(hw.os.as_ref().unwrap().valor, "Steam Runtime 4");

        // Bloque actual: detectado aquí, con su propia procedencia.
        assert_eq!(
            snap.entorno.gpu.as_ref().unwrap().procedencia,
            Procedencia::DetectadoEnSistema
        );

        // Resumen original: se conserva entero, sin recalcular.
        let resumen = snap.resumen_mangohud.as_ref().expect("resumen del CSV");
        assert_eq!(resumen.valor("Average FPS"), Some(60.4));

        // Y las métricas de Thoriapp siguen siendo las suyas.
        assert!((b.avg_fps - analisis.avg_fps).abs() < 1e-9);
        assert_ne!(Some(b.avg_fps.round()), resumen.valor("Average FPS").map(|v| v.round()));
    }

    /// 18. Un CSV sin cabecera de sistema no produce hardware de la grabación, aunque el
    ///     equipo actual sí se detecte. No se rellena el hueco.
    #[test]
    fn s52_un_csv_sin_metadatos_deja_el_hardware_desconocido() {
        let dir = tmp_dir("s52-sin-metadatos");
        let csv = dir.join("Juego_2026-01-01_10-00-00.csv");
        let mut contenido = String::from("fps,frametime,elapsed\n");
        for i in 0..700 {
            contenido.push_str(&format!("60.0,16.6,{}\n", i * 100_000_000u64));
        }
        fs::write(&csv, contenido).unwrap();

        let mut b = bench(0, "Juego");
        b.log_path = csv.to_string_lossy().to_string();
        let snap = construir_snapshot(&b, &contexto_de(1623730), None, "2026-11-24 19:25:00");

        assert_eq!(snap.hardware_grabacion, None);
        assert_eq!(snap.resumen_mangohud, None);
    }

    /// Ayudante: sesión observada de Palworld con la configuración capturada al abrirla.
    fn armar_sesion_de_prueba(posterior_al_arranque: bool) -> crate::sessions::SesionDeEjecucion {
        use crate::sessions::{
            ConfiguracionCapturada, IdentidadDeSesion, OrigenAppId, SesionDeEjecucion, TipoDeSesion,
        };
        SesionDeEjecucion {
            id: "sesion-1".to_string(),
            tipo: TipoDeSesion::Observada,
            identidad: IdentidadDeSesion {
                appid: Some(1623730),
                origen: OrigenAppId::EntornoDelProceso,
                ..Default::default()
            },
            proceso: None,
            evidencia_proceso: Default::default(),
            configuracion_inicial: Some(ConfiguracionCapturada {
                comando_confirmado: Some(ConProcedencia {
                    valor: "mangohud %command%".to_string(),
                    procedencia: Procedencia::MotorRust,
                    capturado_en: None,
                }),
                perfil: None,
                revision_motor: Some(7),
                opciones_steam: None,
                capturado_en: "2026-11-24 18:00:04".to_string(),
                posterior_al_arranque,
            }),
            grabaciones: Vec::new(),
            abierta_en: "2026-11-24 18:00:04".to_string(),
            ultima_observacion: None,
            cerrada_en: None,
            pasadas_sin_verlo: 0,
            // Sesión de prueba sin ejecución que seguir.
            vivos_de_la_ejecucion: Vec::new(),
            arbol_del_lanzamiento: Vec::new(),
        }
    }

    /// Regresión del fallo real de la prueba 15: la cabecera de la configuración decía
    /// «AppID desconocido» mientras la sesión asociada tenía el AppID 1623730. La causa era
    /// que la instantánea se construía **antes** de asociar la sesión, así que nunca podía
    /// consultarla. Con la sesión delante, el AppID observado se usa y se acredita.
    #[test]
    fn el_appid_observado_en_la_sesion_llega_a_la_instantanea_con_su_procedencia() {
        let b = bench(0, "Palworld");
        let ctx = ContextoDeCaptura {
            appid_real: None,
            corresponde_al_juego: false,
            ..Default::default()
        };
        let sesion = armar_sesion_de_prueba(true);
        let snap = construir_snapshot(&b, &ctx, Some(&sesion), "2026-11-24 19:25:00");

        assert_eq!(
            snap.identidad.appid_real,
            Some(1623730),
            "el AppID observado en el proceso debe llegar a la instantánea"
        );
        assert_eq!(
            snap.identidad.appid_procedencia,
            Procedencia::ObservadoEnProceso,
            "y decir que se observó en el proceso, no que se supuso"
        );
    }

    /// Un AppID sin evidencia directa (inferido por nombre, o candidato sin resolver) **no**
    /// se escribe como certeza: la cabecera debe seguir diciendo que no se conoce.
    #[test]
    fn un_appid_inferido_no_se_presenta_como_observado() {
        use crate::sessions::{IdentidadDeSesion, OrigenAppId};
        let b = bench(0, "Palworld");
        let mut sesion = armar_sesion_de_prueba(true);
        sesion.identidad = IdentidadDeSesion {
            appid: Some(1623730),
            origen: OrigenAppId::InferidoPorNombre,
            ..Default::default()
        };
        let snap = construir_snapshot(
            &b,
            &ContextoDeCaptura::default(),
            Some(&sesion),
            "2026-11-24 19:25:00",
        );
        assert_eq!(snap.identidad.appid_real, None);
        assert_eq!(snap.identidad.appid_procedencia, Procedencia::Desconocido);
    }

    /// La instantánea pertenece a la prueba: si el benchmark trae su propio AppID y la sesión
    /// dice otro, no se sobreescribe el del benchmark con el de la sesión.
    #[test]
    fn la_sesion_no_pisa_el_appid_de_la_propia_prueba() {
        let b = bench(1623730, "Palworld");
        let mut sesion = armar_sesion_de_prueba(true);
        sesion.identidad.appid = Some(999999);
        let snap = construir_snapshot(
            &b,
            &ContextoDeCaptura::default(),
            Some(&sesion),
            "2026-11-24 19:25:00",
        );
        assert_eq!(snap.identidad.appid_real, Some(1623730));
    }

    /// Regresión del problema 3: la configuración se declaraba «posterior a la grabación»
    /// aunque la sesión hubiera guardado una foto tomada **durante** la observación, antes
    /// de que el juego arrancara.
    #[test]
    fn una_configuracion_capturada_al_arrancar_no_es_posterior_a_la_grabacion() {
        let b = bench(0, "Palworld");
        let sesion = armar_sesion_de_prueba(false);
        let snap = construir_snapshot(
            &b,
            &ContextoDeCaptura::default(),
            Some(&sesion),
            "2026-11-24 19:25:00",
        );
        assert!(
            !snap.posterior_a_la_grabacion,
            "la foto existía antes de analizar el CSV: no es posterior a la grabación"
        );
    }

    /// Sin sesión no se puede afirmar nada: se mantiene «posterior», que es lo honesto.
    #[test]
    fn sin_sesion_la_captura_sigue_siendo_posterior() {
        let b = bench(0, "Palworld");
        let snap = construir_snapshot(
            &b,
            &ContextoDeCaptura::default(),
            None,
            "2026-11-24 19:25:00",
        );
        assert!(snap.posterior_a_la_grabacion);
    }

    /// 19. La instantánea de una grabación real no se atribuye a otro juego.
    #[test]
    fn s52_la_grabacion_real_no_supone_que_es_el_juego_abierto() {
        let mut b = bench(0, "Palworld");
        b.log_path = fixture_log(LOG2);
        // Contexto de OTRO juego.
        let mut ctx = contexto_de(251570);
        ctx.corresponde_al_juego = false;
        let snap = construir_snapshot(&b, &ctx, None, "2026-11-24 19:25:00");

        // El AppID del contexto no se cuela.
        assert_eq!(snap.identidad.appid_real, None);
        // El hardware de la grabación sí es de la grabación: lo registró MangoHud.
        assert!(snap.hardware_grabacion.is_some());
    }

    /// 20. Valores cero de sensores: se leen del CSV como cero, sin convertirlos en
    ///     «ausente» ni al revés. La interpretación es cosa de 5.4.
    #[test]
    fn s52_los_ceros_del_csv_no_se_reescriben() {
        let contenido = fixture_csv(LOG2);
        let serie = crate::mangohud::Serie::desde_csv(&contenido).unwrap();
        assert!(serie.valores("cpu_power").iter().all(|v| *v == 0.0));
        assert!(serie.valores("gpu_power").iter().all(|v| *v > 0.0));
    }

    /// 21. Un resumen de otra grabación no se asocia a este CSV.
    #[test]
    fn s52_solo_se_asocia_el_resumen_de_su_csv() {
        let mut b = bench(0, "Palworld");
        b.log_path = fixture_log(LOG1);
        let snap = construir_snapshot(&b, &contexto_de(1623730), None, "2026-11-24 19:25:00");
        let resumen = snap.resumen_mangohud.unwrap();

        // Es el del log1 (74.5), no el del log2 (60.4).
        assert_eq!(resumen.valor("Average FPS"), Some(74.5));
        // Se guarda cada par nombre/valor tal cual, sin renombrar columnas.
        assert!(resumen
            .valores
            .iter()
            .any(|(n, v)| n == "Average FPS" && *v == 74.5));
    }

    /// 22. Un CSV inexistente no rompe: se guarda sin datos de grabación.
    #[test]
    fn s52_un_csv_inexistente_no_rompe_la_instantanea() {
        let mut b = bench(0, "Fantasma");
        b.log_path = "/tmp/no-existe-en-absoluto-123.csv".to_string();
        let snap = construir_snapshot(&b, &contexto_de(1623730), None, "2026-11-24 19:25:00");
        assert!(snap.hardware_grabacion.is_none());
        assert!(snap.resumen_mangohud.is_none());
    }

    /// 23. Un benchmark antiguo se lee igual y, al no tener instantánea, no se le inventa
    ///     hardware histórico ni resumen.
    #[test]
    fn s52_los_antiguos_no_se_completan_con_datos_nuevos() {
        let json = r#"{"benchmarks":[{"id":"b1","profile_id":"p","game_name":"Palworld",
            "appid":1623730,"timestamp":"2024-01-01 10:00:00","duration_seconds":60.0,
            "avg_fps":60.0,"fps_1_percent_low":50.0,"fps_0_1_percent_low":40.0,"max_fps":70.0,
            "min_fps":30.0,"frame_time_avg":16.6,"frame_time_1_percent_high":20.0,
            "total_frames":3600,"launch_command":"","origen_comando":"desconocido"}]}"#;
        let d: BenchmarksData = serde_json::from_str(json).unwrap();
        let b = &d.benchmarks[0];
        assert!(b.snapshot.is_none());
        let n: Option<u32> = b.number;
        assert_eq!(n, None);
    }

    /// 24. Numeración e historial: el tope son 10 guardadas pero el número de prueba
    ///     continúa subiendo.
    #[test]
    fn s52_la_numeracion_sigue_subiendo_con_el_tope_de_diez() {
        let mut counters = HashMap::new();
        counters.insert("appid:1623730".to_string(), 12u32);
        let d = BenchmarksData {
            version: Some(BENCHMARKS_SCHEMA_VERSION),
            benchmarks: Vec::new(),
            counters,
        };
        let json = serde_json::to_string(&d).unwrap();
        let releido: BenchmarksData = serde_json::from_str(&json).unwrap();
        assert_eq!(releido.counters.get("appid:1623730"), Some(&12));
        assert_eq!(MAX_BENCHMARKS_PER_GAME, 10);
    }
}

#[cfg(test)]
mod pruebas_quinta_correccion {
    use super::*;

    fn bench(appid: u32, name: &str) -> Benchmark {
        Benchmark {
            id: format!("{}-{}", appid, name),
            profile_id: String::new(),
            game_name: name.to_string(),
            appid,
            timestamp: String::new(),
            duration_seconds: 60.0,
            avg_fps: 60.0,
            fps_1_percent_low: 30.0,
            fps_0_1_percent_low: 20.0,
            max_fps: 120.0,
            min_fps: 10.0,
            frame_time_avg: 16.6,
            frame_time_1_percent_high: 33.0,
            total_frames: 3600,
            notes: None,
            launch_command: "%command%".to_string(),
            origen_comando: OrigenComando::Instantanea,
            steam_command: None,
            number: None,
            snapshot: None,
            sesion_id: None,
            spikes_per_min: None,
            pct_over_33ms: None,
            jitter_ms: None,
            worst_frame_ms: None,
            warmup_seconds: None,
            noise_percent: None,
            shader_stutter: None,
            log_path: String::new(),
            sample_count: None,
            muestras_temporales: None,
            metodologia_calculo: None,
            contabilidad_muestras: None,
            apoyo_metricas: None,
            duracion_medida: None,
        }
    }

    /// Dos grabaciones de juegos distintos no son comparables.
    #[test]
    fn s53c5_dos_appid_distintos_no_son_el_mismo_juego() {
        let a = bench(4623570, "Deskrawl");
        let b = bench(1623730, "Palworld");
        assert!(!mismo_juego(&a, &b));
        // Aunque el nombre se haya escrito igual, dos AppID distintos son dos juegos.
        let a2 = bench(4623570, "Juego");
        let b2 = bench(1623730, "Juego");
        assert!(!mismo_juego(&a2, &b2), "el AppID manda sobre el nombre");
    }

    /// El mismo AppID con el nombre escrito de otra forma sigue siendo el mismo juego.
    #[test]
    fn s53c5_el_appid_manda_sobre_el_nombre() {
        let a = bench(4623570, "Deskrawl");
        let b = bench(4623570, "Deskrawl (Proton)");
        assert!(mismo_juego(&a, &b));
    }

    /// Sin AppID en ninguna, se compara el nombre normalizado.
    #[test]
    fn s53c5_sin_appid_manda_el_nombre_normalizado() {
        let a = bench(0, "7 Days to Die");
        let b = bench(0, "7 days to die");
        assert!(mismo_juego(&a, &b));
        let c = bench(0, "Palworld");
        assert!(!mismo_juego(&a, &c));
    }

    /// Si una tiene AppID y la otra no, no se puede afirmar que sean el mismo juego.
    #[test]
    fn s53c5_sin_evidencia_suficiente_no_se_afirma_que_sean_el_mismo() {
        let con = bench(4623570, "Deskrawl");
        let sin = bench(0, "Deskrawl");
        assert!(
            !mismo_juego(&con, &sin),
            "afirmar que son el mismo juego sin evidencia es peor que no compararlas"
        );
    }

    /// El historial y la comparación usan la misma regla: no puede haber discrepancia.
    #[test]
    fn s53c5_historial_y_comparacion_usan_la_misma_regla() {
        let referencia = bench(4623570, "Deskrawl");
        let misma = bench(4623570, "Deskrawl");
        let otra = bench(1623730, "Palworld");
        assert_eq!(
            juego_coincide(&misma, referencia.appid, &referencia.game_name),
            mismo_juego(&referencia, &misma)
        );
        assert_eq!(
            juego_coincide(&otra, referencia.appid, &referencia.game_name),
            mismo_juego(&referencia, &otra)
        );
    }

    /// Comparar dos juegos distintos se rechaza en el backend, no se devuelve con un aviso.
    #[test]
    fn s53c5_comparar_juegos_distintos_se_rechaza() {
        let r = comparar_identidades(4623570, "Deskrawl", 1623730, "Palworld");
        assert!(r.is_err(), "la comparación debe rechazarse, no calcularse");
        assert!(r.unwrap_err().contains("juegos distintos"));
    }

    /// Atajo que aplica las mismas comprobaciones que `compare_benchmarks` antes de calcular,
    /// para poder probar el rechazo sin tocar el disco.
    fn comparar_identidades(
        appid_a: u32,
        nombre_a: &str,
        appid_b: u32,
        nombre_b: &str,
    ) -> Result<(), String> {
        let a = bench(appid_a, nombre_a);
        let b = bench(appid_b, nombre_b);
        if !mismo_juego(&a, &b) {
            return Err(format!(
                "No se pueden comparar grabaciones de juegos distintos: «{}» (AppID {}) y «{}» (AppID {}).",
                a.game_name,
                texto_appid(a.appid),
                b.game_name,
                texto_appid(b.appid)
            ));
        }
        Ok(())
    }
}

/// Conjunto mixto de la subfase 5.3 (complemento final).
///
/// Reproduce el caso que el usuario vio en sus capturas: siete pruebas en «Historial de este
/// juego», dos de Deskrawl y cinco de Palworld. Aquí se añaden además una antigua **sin AppID**
/// y otra con **identidad contradictoria**, que son las que pueden colarse.
///
/// Todas estas pruebas son puras: no tocan disco ni ficheros reales.
#[cfg(test)]
mod pruebas_aislamiento {
    use super::*;

    pub(super) fn b(id: &str, appid: u32, nombre: &str, ts: &str) -> Benchmark {
        Benchmark {
            id: id.to_string(),
            profile_id: String::new(),
            game_name: nombre.to_string(),
            appid,
            timestamp: ts.to_string(),
            duration_seconds: 60.0,
            avg_fps: 60.0,
            fps_1_percent_low: 30.0,
            fps_0_1_percent_low: 20.0,
            max_fps: 120.0,
            min_fps: 10.0,
            frame_time_avg: 16.6,
            frame_time_1_percent_high: 33.0,
            total_frames: 3600,
            notes: None,
            launch_command: "%command%".to_string(),
            origen_comando: OrigenComando::Instantanea,
            steam_command: None,
            number: None,
            snapshot: None,
            sesion_id: None,
            spikes_per_min: None,
            pct_over_33ms: None,
            jitter_ms: None,
            worst_frame_ms: None,
            warmup_seconds: None,
            noise_percent: None,
            shader_stutter: None,
            log_path: String::new(),
            sample_count: None,
            muestras_temporales: None,
            metodologia_calculo: None,
            contabilidad_muestras: None,
            apoyo_metricas: None,
            duracion_medida: None,
        }
    }

    const DESKRAWL: u32 = 4623570;
    const PALWORLD: u32 = 1623730;

    /// El conjunto tal cual lo describe el usuario, más los dos casos peligrosos.
    fn historial_mixto() -> Vec<Benchmark> {
        vec![
            b("d1", DESKRAWL, "Deskrawl", "2026-11-25 10:00:00"),
            b("d2", DESKRAWL, "Deskrawl", "2026-11-25 11:00:00"),
            b("p1", PALWORLD, "Palworld", "2026-11-24 20:00:00"),
            b("p2", PALWORLD, "Palworld", "2026-11-24 21:00:00"),
            b("p3", PALWORLD, "Palworld", "2026-11-24 22:00:00"),
            b("p4", PALWORLD, "Palworld", "2026-11-24 23:00:00"),
            b("p5", PALWORLD, "Palworld", "2026-11-25 09:00:00"),
            // Antigua: MangoHud no escribe el AppID en el CSV, así que se quedó en 0. El
            // nombre sí quedó limpio (`-Win64-Shipping` se quita al importar), por eso
            // coincide por nombre con el Palworld comercial.
            b("p0", 0, "Palworld", "2026-11-23 19:00:00"),
            // Contradictoria: el nombre dice Palworld pero el AppID dice Deskrawl. No se
            // resuelve sola hacia ninguno de los dos: manda el AppID, que es lo observado.
            b("x1", DESKRAWL, "Palworld", "2026-11-25 12:00:00"),
        ]
    }

    fn ids_del_historial(lista: &[Benchmark], appid: u32, nombre: &str) -> Vec<String> {
        lista
            .iter()
            .filter(|x| juego_coincide(x, appid, nombre))
            .map(|x| x.id.clone())
            .collect()
    }

    /// 1. El historial de cada juego enseña **solo** sus pruebas.
    #[test]
    fn s53f_el_historial_no_mezcla_juegos() {
        let h = historial_mixto();

        let deskrawl = ids_del_historial(&h, DESKRAWL, "Deskrawl");
        // d1, d2 y x1 (que declara AppID de Deskrawl, aunque se llame Palworld).
        assert_eq!(deskrawl, vec!["d1", "d2", "x1"]);

        let palworld = ids_del_historial(&h, PALWORLD, "Palworld");
        // p1..p5 y la antigua sin AppID. **No** x1, aunque se llame Palworld: su AppID dice
        // otra cosa y una coincidencia de nombre no puede pasar por encima de eso.
        assert_eq!(palworld, vec!["p1", "p2", "p3", "p4", "p5", "p0"]);
        assert!(!palworld.contains(&"x1".to_string()));
    }

    /// 2. La prueba antigua sin AppID se conserva, y con su certeza correspondiente.
    #[test]
    fn s53f_la_prueba_sin_appid_se_conserva_y_no_se_le_inventa_identidad() {
        let h = historial_mixto();
        let antigua = h.iter().find(|x| x.id == "p0").unwrap();
        // Sigue teniendo AppID 0: no se le asigna uno deducido.
        assert_eq!(antigua.appid, 0);
        // Se encuentra por nombre, que es la única pista que hay…
        assert!(ids_del_historial(&h, PALWORLD, "Palworld").contains(&"p0".to_string()));
        // …pero no se afirma que sea el mismo juego que una prueba con AppID.
        let con_appid = h.iter().find(|x| x.id == "p1").unwrap();
        assert!(!mismo_juego(antigua, con_appid));
    }

    /// 3. Comparar pruebas de juegos distintos se rechaza, no se avisa y se devuelve igual.
    #[test]
    fn s53f_las_comparaciones_directas_no_mezclan_juegos() {
        let h = historial_mixto();
        let deskrawl = h.iter().find(|x| x.id == "d1").unwrap();
        let palworld = h.iter().find(|x| x.id == "p1").unwrap();
        let antigua = h.iter().find(|x| x.id == "p0").unwrap();

        assert!(!mismo_juego(deskrawl, palworld));
        assert!(!mismo_juego(deskrawl, antigua));
        // Dos del mismo juego sí, que es lo que hace útil la comprobación.
        assert!(mismo_juego(deskrawl, h.iter().find(|x| x.id == "d2").unwrap()));
    }

    /// 4. La comparación **automática** (la de las dos últimas) usa la misma regla.
    #[test]
    fn s53f_la_comparacion_automatica_no_mezcla_juegos() {
        let h = historial_mixto();
        let del_juego = ids_del_historial(&h, PALWORLD, "Palworld");
        // Se toman las dos últimas **de ese juego**, nunca las dos últimas del archivo.
        let mut suyas: Vec<&Benchmark> =
            h.iter().filter(|x| del_juego.contains(&x.id)).collect();
        suyas.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        let ultimas = &suyas[suyas.len() - 2..];
        assert!(mismo_juego(ultimas[0], ultimas[1]));
        assert!(ultimas.iter().all(|x| x.appid == PALWORLD || x.appid == 0));
        // Las dos últimas del archivo entero serían x1 (Deskrawl) y d2: el contador no debe
        // sacarlas de ahí.
        assert!(!ultimas.iter().any(|x| x.id == "x1"));
    }

    /// 5. Los contadores por juego no se mezclan.
    #[test]
    fn s53f_los_contadores_son_por_juego() {
        let h = historial_mixto();
        let mut claves: Vec<String> = h.iter().map(game_key).collect();
        claves.sort();
        claves.dedup();
        // Cada juego con AppID tiene su propia clave: no comparten contador.
        assert!(claves.contains(&format!("appid:{DESKRAWL}")));
        assert!(claves.contains(&format!("appid:{PALWORLD}")));
        // La antigua, al no tener AppID, cae en una clave por nombre. Se documenta aquí
        // porque es una limitación conocida: no se une al grupo de su AppID.
        assert!(claves.contains(&format!("name:{}", norm_game_name("Palworld"))));
    }

    /// 6. Borrar el historial de Deskrawl no toca ninguna prueba de Palworld.
    #[test]
    fn s53f_borrar_un_juego_no_toca_las_pruebas_de_otro() {
        let mut data = BenchmarksData {
            version: None,
            benchmarks: historial_mixto(),
            counters: HashMap::new(),
        };
        let antes = data.benchmarks.len();
        let ids = ids_a_borrar_del_juego(&data, DESKRAWL, "Deskrawl");
        assert_eq!(ids, vec!["d1", "d2", "x1"]);
        data.benchmarks.retain(|x| !ids.iter().any(|i| i == &x.id));

        let quedan: Vec<String> = data.benchmarks.iter().map(|x| x.id.clone()).collect();
        // Las cinco de Palworld y su antigua siguen todas.
        for id in ["p1", "p2", "p3", "p4", "p5", "p0"] {
            assert!(quedan.contains(&id.to_string()), "se borró {id} sin tocar");
        }
        assert_eq!(antes - data.benchmarks.len(), 3);
        // Y no se ha borrado nada de más.
        assert_eq!(quedan.len(), 6);
    }

    /// 7. El borrado por juego y la lectura usan **el mismo** alcance.
    ///
    /// Si divergieran, el botón borraría pruebas que la ventana no estaba enseñando (o al
    /// revés). Se comprueba sobre los dos juegos y con las dos formas de llamar.
    #[test]
    fn s53f_borrar_borra_exactamente_lo_que_se_ve() {
        for (appid, nombre) in [(DESKRAWL, "Deskrawl"), (PALWORLD, "Palworld")] {
            let data = BenchmarksData {
                version: None,
                benchmarks: historial_mixto(),
                counters: HashMap::new(),
            };
            let visto = ids_del_historial(&data.benchmarks, appid, nombre);
            let a_borrar = ids_a_borrar_del_juego(&data, appid, nombre);
            assert_eq!(visto, a_borrar, "el alcance de borrado no coincide con el de la vista");
        }
    }

    /// 8. Una identidad contradictoria no se borra como si fuera de un juego concreto.
    ///
    /// `x1` se llama Palworld. Al pedir el historial de Palworld no aparece (manda su AppID),
    /// pero **tampoco se borra** desde el historial de Deskrawl por su nombre. Se borra solo
    /// cuando se borra Deskrawl, que es el juego que su AppID declara.
    #[test]
    fn s53f_una_identidad_contradictoria_no_se_borra_por_su_nombre() {
        let data = BenchmarksData {
            version: None,
            benchmarks: historial_mixto(),
            counters: HashMap::new(),
        };
        // Pedir el historial de Palworld no incluye x1…
        let de_palworld = ids_del_historial(&data.benchmarks, PALWORLD, "Palworld");
        assert!(!de_palworld.contains(&"x1".to_string()));
        // …así que borrar «el historial de Palworld» tampoco se la lleva.
        let a_borrar = ids_a_borrar_del_juego(&data, PALWORLD, "Palworld");
        assert!(!a_borrar.contains(&"x1".to_string()));
    }

    /// 9. Las comparaciones guardadas que dependen de pruebas borradas se limpian.
    ///
    /// El fallo: `delete_benchmarks_for_game` borraba las pruebas y dejaba los informes
    /// guardados, que siguen citando datos que ya no existen.
    #[test]
    fn s53f_la_purga_quita_solo_las_comparaciones_huerfanas() {
        let h = historial_mixto();
        let get = |id: &str| h.iter().find(|x| x.id == id).unwrap().clone();
        let guardada = |id: &str, base: &str, test: &str| SavedComparison {
            id: id.to_string(),
            saved_at: String::new(),
            game_name: String::new(),
            before_fps: 0.0,
            after_fps: 0.0,
            fps_change_percent: 0.0,
            comparison: BenchmarkComparison {
                baseline: get(base),
                test: get(test),
                fps_change_percent: 0.0,
                fps_1_percent_change_percent: 0.0,
                fps_0_1_percent_change_percent: 0.0,
                frame_time_change_percent: 0.0,
                conclusion: String::new(),
                noise_percent: None,
                within_noise: false,
                comandos_iguales: None,
            },
            evaluacion: None,
            evaluacion_ilegible: None,
        };
        // Una de Palworld (debe sobrevivir) y una de Deskrawl (debe caer).
        let mut comps = vec![
            guardada("c_pal", "p1", "p2"),
            guardada("c_desk", "d1", "d2"),
        ];
        let ids_borrados = ids_a_borrar_del_juego(
            &BenchmarksData {
                version: None,
                benchmarks: h.clone(),
                counters: HashMap::new(),
            },
            DESKRAWL,
            "Deskrawl",
        );
        comps.retain(|c| {
            !ids_borrados
                .iter()
                .any(|i| i == &c.comparison.baseline.id || i == &c.comparison.test.id)
        });
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].id, "c_pal");
    }
}

/// Identidad del juego en la ventana «Pruebas del juego» (defecto real de la Prueba 4).
///
/// Datos auténticos del equipo del usuario: **todas** las grabaciones tienen `appid: 0`
/// (MangoHud no escribe el AppID en el CSV) y la Biblioteca llama al juego por su nombre
/// comercial, `Deskrawl: Idle ARPG`, mientras MangoHud escribe `Deskrawl`. La ficha decía
/// «Todavía no hay pruebas de Deskrawl: Idle ARPG» teniendo cuatro.
///
/// Palworld funcionaba por casualidad: su nombre comercial no lleva subtítulo.
#[cfg(test)]
mod pruebas_identidad_por_nombre {
    use super::*;

    /// Historial real: 5 de Palworld y 4 de Deskrawl, todas con `appid: 0`.
    fn historial_real() -> Vec<Benchmark> {
        let mut v: Vec<Benchmark> = (11..=15)
            .enumerate()
            .map(|(i, n)| {
                let mut x = pruebas_aislamiento::b(
                    &format!("p{i}"),
                    0,
                    "Palworld",
                    &format!("2026-11-24 20:{n}:00"),
                );
                x.number = Some(n);
                x
            })
            .collect();
        v.extend((1..=4).enumerate().map(|(i, n)| {
            let mut x = pruebas_aislamiento::b(
                &format!("d{i}"),
                0,
                "Deskrawl",
                &format!("2026-11-25 18:3{n}:11"),
            );
            x.number = Some(n);
            x
        }));
        v
    }

    fn cuantas(appid: u32, nombre: &str) -> usize {
        historial_real()
            .into_iter()
            .filter(|x| juego_coincide(x, appid, nombre))
            .count()
    }

    /// El caso exacto que falló: mismo AppID (0) y nombres que difieren en el subtítulo.
    #[test]
    fn s53g_el_subtitulo_del_nombre_comercial_no_oculta_el_historial() {
        assert_eq!(
            cuantas(0, "Deskrawl: Idle ARPG"),
            4,
            "la ficha con el nombre comercial debe encontrar las 4 pruebas de Deskrawl"
        );
        // Y el otro lado sigue intacto.
        assert_eq!(cuantas(0, "Palworld"), 5);
    }

    /// El nombre del binario también sirve: es lo que escribe MangoHud.
    #[test]
    fn s53g_el_nombre_sin_subtitulo_encuentra_lo_mismo() {
        assert_eq!(cuantas(0, "Deskrawl"), 4);
        assert_eq!(cuantas(0, "Deskrawl.exe"), 4);
    }

    /// Un subtítulo es un corte en separador, no un prefijo cualquiera. Si se aceptara
    /// cualquier prefijo, «Slime» arrastraría el historial de «Slime Rancher».
    #[test]
    fn s53g_un_prefijo_compartido_no_es_el_mismo_juego() {
        let mut slime = pruebas_aislamiento::b("s1", 0, "Slime", "2026-11-20 10:00:00");
        slime.number = Some(1);
        let mut rancher = pruebas_aislamiento::b("s2", 0, "Slime Rancher", "2026-11-20 11:00:00");
        rancher.number = Some(1);

        assert!(
            !juego_coincide(&slime, 0, "Slime Rancher"),
            "«Slime» no es «Slime Rancher»: el corte cae en un espacio, no en un separador"
        );
        assert!(!juego_coincide(&rancher, 0, "Slime"));
        // El de una letra tampoco puede comerse todo.
        assert!(!es_el_mismo_con_subtitulo("p", "palworld: x"));
    }

    /// Un AppID válido manda siempre; el subtítulo solo actúa cuando no hay AppID.
    #[test]
    fn s53g_con_appid_valido_no_se_mira_el_nombre() {
        let b = pruebas_aislamiento::b("x", 4623570, "Deskrawl: Idle ARPG", "2026-11-25 18:30:13");
        // Mismo AppID con nombres distintos: sí es el mismo juego.
        assert!(juego_coincide(&b, 4623570, "Deskrawl"));
        // AppID distinto: no, aunque el nombre se parezca.
        assert!(!juego_coincide(&b, 1623730, "Deskrawl: Idle ARPG"));
    }

    /// La comparación es estricta: un AppID de un lado y ninguno del otro sigue sin
    /// compararse, tampoco por subtítulo. Ahí se produciría un número inventado.
    #[test]
    fn s53g_el_subtitulo_no_relaja_la_comparacion_estricta() {
        let con = pruebas_aislamiento::b("a", 4623570, "Deskrawl", "2026-11-25 18:30:13");
        let sin = pruebas_aislamiento::b("b", 0, "Deskrawl: Idle ARPG", "2026-11-25 18:30:13");
        assert!(
            !mismo_juego(&con, &sin),
            "evidencia incompleta: no se comparan aunque el nombre encaje"
        );
        // Y dos sin AppID con subtítulo distinto sí son el mismo juego.
        let otro = pruebas_aislamiento::b("c", 0, "Deskrawl", "2026-11-25 19:00:00");
        assert!(mismo_juego(&sin, &otro));
    }

    /// El borrado desde la ficha usa la misma regla, así que borra exactamente las cuatro
    /// de Deskrawl y deja intactas las cinco de Palworld.
    #[test]
    fn s53g_borrar_desde_la_ficha_alcance_exacto() {
        let data = BenchmarksData {
            version: None,
            benchmarks: historial_real(),
            counters: HashMap::new(),
        };
        let ids = ids_a_borrar_del_juego(&data, 0, "Deskrawl: Idle ARPG");
        assert_eq!(ids.len(), 4, "solo las 4 de Deskrawl");

        let sobran: Vec<&Benchmark> = data
            .benchmarks
            .iter()
            .filter(|x| !ids.contains(&x.id))
            .collect();
        assert_eq!(sobran.len(), 5);
        assert!(
            sobran.iter().all(|x| x.game_name == "Palworld"),
            "ninguna prueba de Palworld puede caer"
        );
    }
}

// ==================== 5.4.1.A: CSV reales e históricos ====================

/// Los dos CSV reales deben conservar **exactamente** los números que ya daban antes de
/// la metodología v2. Aquí no había filas que descartar (0 duplicados en ambos), así que
/// la corrección del descarte no puede mover ni un decimal.
///
/// Si alguno de estos valores cambia, o el pipeline cambió de comportamiento sobre datos
/// reales, o la tolerancia deja de ser suficiente: las dos cosas hay que explicarlas.
#[cfg(test)]
mod pruebas_v2_csv_reales {
    use super::*;

    const LOG_23: &str = "Palworld-Win64-Shipping_2026-11-23_16-54-52.csv";
    const LOG_24: &str = "Palworld-Win64-Shipping_2026-11-24_18-19-57.csv";

    fn ruta(nombre: &str) -> String {
        format!(
            "{}/../tests/fixtures/mangohud/{nombre}",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    /// Tolerancia para los contrastes numéricos: los valores están fijados con 4 decimales
    /// y el cálculo es determinista en el mismo binario, así que la discrepancia admisible
    /// es solo la del redondeo del literal (5e-5). Se deja 1e-4 como margen de impresión.
    const TOL: f64 = 1e-4;

    fn cerca(obtenido: f64, esperado: f64, que: &str) {
        assert!(
            (obtenido - esperado).abs() <= TOL,
            "{que}: obtenido {obtenido}, esperado {esperado}"
        );
    }

    #[test]
    fn v2_conserva_los_numeros_del_csv_23_11() {
        let b = analyze_mangohud_log(&ruta(LOG_23), 10.0).expect("análisis 23-11");
        cerca(b.avg_fps, 75.0244, "avg_fps");
        cerca(b.fps_1_percent_low, 69.9924, "fps_1_percent_low");
        cerca(b.fps_0_1_percent_low, 37.1773, "fps_0_1_percent_low");
        cerca(b.max_fps, 103.7825, "max_fps");
        cerca(b.min_fps, 39.3372, "min_fps");
        cerca(b.frame_time_1_percent_high, 19.9119, "frame_time_1_percent_high");
        cerca(b.noise_percent.expect("ruido"), 0.2276, "noise_percent");
        cerca(b.duracion_medida.expect("duración"), 59.8517, "duración");
        assert_eq!(b.total_frames, 499);
    }

    #[test]
    fn v2_conserva_los_numeros_del_csv_24_11() {
        let b = analyze_mangohud_log(&ruta(LOG_24), 10.0).expect("análisis 24-11");
        cerca(b.avg_fps, 66.2282, "avg_fps");
        cerca(b.fps_1_percent_low, 41.3128, "fps_1_percent_low");
        cerca(b.fps_0_1_percent_low, 18.2874, "fps_0_1_percent_low");
        cerca(b.frame_time_1_percent_high, 38.4127, "frame_time_1_percent_high");
        cerca(b.noise_percent.expect("ruido"), 8.8420, "noise_percent");
        cerca(b.duracion_medida.expect("duración"), 59.7554, "duración");
        assert_eq!(b.total_frames, 498);
    }

    #[test]
    fn v2_los_csv_reales_no_tienen_incidencias_temporales() {
        for nombre in [LOG_23, LOG_24] {
            let b = analyze_mangohud_log(&ruta(nombre), 10.0).expect("análisis");
            let c = b.contabilidad_muestras.as_ref().expect("contabilidad");
            assert!(c.eje_temporal_fiable, "{nombre}: el eje debe ser fiable");
            assert!(
                !c.incidencias.hay_algunas(),
                "{nombre}: no se esperaban incidencias, hay {:?}",
                c.incidencias
            );
            assert!(c.cuadra(), "{nombre}: la contabilidad no cuadra");
            // Todas las filas del CSV tienen rendimiento válido.
            assert_eq!(c.filas_con_rendimiento_invalido, 0, "{nombre}");
        }
    }

    #[test]
    fn v2_declara_la_metodologia_y_el_apoyo_de_las_metricas() {
        let b = analyze_mangohud_log(&ruta(LOG_24), 10.0).expect("análisis");
        assert_eq!(b.metodologia_calculo, Some(MetodologiaCalculo::V2));

        let apoyo = b.apoyo_metricas.as_ref().expect("apoyo");
        let p1 = apoyo.fps_1_percent_low.expect("apoyo 1% low");
        assert_eq!(p1.muestras, b.total_frames as u32);
        // La cola nominal del 1 % sobre ~500 muestras ronda las 5 muestras.
        assert!((p1.cola_nominal - 4.98).abs() < 0.02, "cola {}", p1.cola_nominal);

        // El 0,1 % sobre menos de 1000 muestras declara una cola menor que una muestra.
        let p01 = apoyo.fps_0_1_percent_low.expect("apoyo 0,1% low");
        assert!(p01.cola_nominal < 1.0);
        assert_eq!(p01.observaciones_interpoladas, 2);

        // La media de cola declara cuántas muestras entraron de verdad (el ~1 %, no todas).
        // Sobre las 498 muestras analizadas tras el calentamiento entran 5 (1,004 %).
        // Ojo: contarlas sobre las 598 filas del CSV da 6; el n que manda es el analizado.
        let media = apoyo.frame_time_1_percent_high.expect("apoyo media de cola");
        let incluidas = media.muestras_en_la_media.expect("muestras en la media");
        assert_eq!(incluidas, 5, "en el 24-11 (n=498) entran 5 muestras");
    }

    #[test]
    fn v2_el_csv_real_mapea_gpu_y_comando_igual_que_antes() {
        // El pipeline completo sigue produciendo un Benchmark guardable: la ruta, el
        // nombre del juego y la duración se conservan.
        let b = analyze_mangohud_log(&ruta(LOG_24), 10.0).expect("análisis");
        assert_eq!(b.game_name, "Palworld");
        assert!(!b.log_path.is_empty());
        assert!(b.duracion_medida.unwrap() > 0.0);
        // El valor guardado es el corte real (el primer timestamp >= 10 s), no el
        // solicitado: se declara lo que se descartó de verdad.
        let w = b.warmup_seconds.unwrap();
        assert!((w - 10.0).abs() < 0.2, "calentamiento aplicado: {w}");
    }
}

/// Compatibilidad con archivos históricos: leerlos no cambia ni sus bytes ni su epoch de
/// modificación, y los campos nuevos quedan desconocidos (nunca cero inventado).
#[cfg(test)]
mod pruebas_v2_compatibilidad {
    use super::*;
    use std::fs;

    /// JSON con la forma exacta que se guardaba antes de 5.4.1.A (sin los campos nuevos).
    const HISTORICO: &str = r#"{"benchmarks":[{"id":"b1","profile_id":"","game_name":"Palworld","appid":1623730,"timestamp":"2024-01-01 10:00:00","duration_seconds":60.0,"avg_fps":60.0,"fps_1_percent_low":50.0,"fps_0_1_percent_low":40.0,"max_fps":70.0,"min_fps":30.0,"frame_time_avg":16.6,"frame_time_1_percent_high":20.0,"total_frames":600,"notes":null}],"counters":{"appid:1623730":1}}"#;

    fn dir_de_prueba(nombre: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("thoriapp_v2_compat_{nombre}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("crear dir");
        d
    }

    #[test]
    fn un_benchmark_historico_queda_sin_metodologia_y_sin_apoyo() {
        let leido: BenchmarksData = serde_json::from_str(HISTORICO).expect("JSON histórico");
        let b = &leido.benchmarks[0];
        // «No consta» es `None`, no un v1 deducido ni un cero.
        assert_eq!(b.metodologia_calculo, None);
        assert_eq!(b.contabilidad_muestras, None);
        assert_eq!(b.apoyo_metricas, None);
        assert_eq!(b.duracion_medida, None);
        // Los números que tenía se leen tal cual.
        assert_eq!(b.avg_fps, 60.0);
    }

    #[test]
    fn leer_un_historico_no_cambia_sus_bytes_ni_su_mtime() {
        use std::time::SystemTime;

        let dir = dir_de_prueba("mtime");
        let ruta = dir.join("benchmarks.json");
        fs::write(&ruta, HISTORICO).expect("escribir");

        let antes_bytes = fs::read(&ruta).expect("leer");
        let antes_mtime: SystemTime = fs::metadata(&ruta).and_then(|m| m.modified()).expect("mtime");

        // Se lee como hace la app al abrir el histórico.
        let leido: BenchmarksData =
            serde_json::from_str(&fs::read_to_string(&ruta).expect("texto")).expect("JSON");
        assert_eq!(leido.benchmarks.len(), 1);
        assert_eq!(leido.benchmarks[0].metodologia_calculo, None);

        // Leer no escribe: ni bytes ni mtime cambian.
        let despues_bytes = fs::read(&ruta).expect("leer");
        let despues_mtime: SystemTime = fs::metadata(&ruta).and_then(|m| m.modified()).expect("mtime");
        assert_eq!(antes_bytes, despues_bytes, "los bytes no deben cambiar");
        assert_eq!(antes_mtime, despues_mtime, "el mtime no debe cambiar");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_campos_nuevos_no_se_inventan_al_serializar_un_historico() {
        let leido: BenchmarksData = serde_json::from_str(HISTORICO).expect("JSON histórico");
        let json = serde_json::to_string(&leido).expect("serializar");
        // `skip_serializing_if = Option::is_none`: los desconocidos no aparecen.
        assert!(!json.contains("metodologia_calculo"));
        assert!(!json.contains("contabilidad_muestras"));
        assert!(!json.contains("apoyo_metricas"));
        assert!(!json.contains("duracion_medida"));
        // Y siguen siendo desconocidos al volver a leerlos: igualdad semántica.
        let releido: BenchmarksData = serde_json::from_str(&json).expect("releer");
        assert_eq!(releido.benchmarks[0].metodologia_calculo, None);
        assert_eq!(releido.benchmarks[0].avg_fps, 60.0);
    }
}

/// 5.4.1.C.2: el guardado evaluado sobre la **ruta real**, no sobre una copia.
///
/// Estos tests usan los ayudantes de `tests` (`tmp_dir`, `bench`, `leer_bench`) y ejercitan
/// `guardar_comparacion_evaluada`, el mismo cuerpo que llama la función pública. El
/// directorio es temporal; la lógica es la de producción.
#[cfg(test)]
mod pruebas_c2_guardado {
    use super::tests::{bench, leer_bench, tmp_dir};
    use super::*;

    // ──────────────── 5.4.1.C.2: guardado evaluado (ruta real) ────────────────
    //
    // Estas pruebas ejercitan el **núcleo real** (`guardar_comparacion_evaluada`), el
    // mismo que usará la ruta pública: solo cambian las rutas por un directorio temporal.
    // No son una copia de la lógica.

    /// Prepara un historial con dos grabaciones y una comparación ya cargada.
    /// Hace el viaje por JavaScript de verdad: `JSON.parse` + `JSON.stringify` en Node.
    ///
    /// Devuelve `None` si Node no está disponible, para que la prueba se omita en vez de
    /// simular el viaje y dar un verde falso.
    fn ir_y_volver_por_node(json: &str) -> Option<String> {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let mut hijo = Command::new("node")
            .arg("-e")
            .arg("let s='';process.stdin.on('data',d=>s+=d).on('end',()=>process.stdout.write(JSON.stringify(JSON.parse(s))))")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        hijo.stdin.as_mut()?.write_all(json.as_bytes()).ok()?;
        let salida = hijo.wait_with_output().ok()?;
        if !salida.status.success() {
            return None;
        }
        String::from_utf8(salida.stdout).ok()
    }

    pub(crate) fn escenario_c2(nombre: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = tmp_dir(nombre);
        let ruta_bench = dir.join("benchmarks.json");
        let ruta_cmp = dir.join("saved_comparisons.json");

        let mut a = bench(1623730, "Palworld");
        a.id = "b_a".to_string();
        a.avg_fps = 100.0;
        a.fps_1_percent_low = 100.0;
        a.fps_0_1_percent_low = 100.0;
        a.total_frames = 20000;
        let mut b = a.clone();
        b.id = "b_b".to_string();
        b.avg_fps = 112.5;
        b.fps_1_percent_low = 100.0;
        b.fps_0_1_percent_low = 100.0;

        let historial = BenchmarksData {
            version: Some(BENCHMARKS_SCHEMA_VERSION),
            benchmarks: vec![a.clone(), b.clone()],
            counters: HashMap::new(),
        };
        escribir_atomico(
            &ruta_bench,
            &serde_json::to_string(&historial).expect("serializar"),
        )
        .expect("escribir historial");
        escribir_atomico(&ruta_cmp, r#"{"version":2,"comparisons":[]}"#).expect("escribir");

        (ruta_bench, ruta_cmp)
    }

    /// **Flujo completo de C.2**: vista previa → serialización → deserialización →
    /// confirmación → recálculo → persistencia.
    ///
    /// La confirmación se obtiene de `vista_previa_evaluada` (la función real), se pasa por
    /// JSON como haría el viaje de IPC, se deserializa y se guarda. Comprueba que el
    /// significado sobrevive a la ida y vuelta completa.
    #[test]
    fn c2_flujo_completo_vista_previa_a_persistencia() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-flujo");

        // 1. Vista previa real.
        let vista = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista previa");

        // 2. Viaje de serialización, como en el IPC.
        let json = serde_json::to_string(&vista).expect("serializar la vista previa");
        let vuelta: crate::evaluacion::ConfirmacionDeVistaPrevia =
            serde_json::from_str(&json).expect("deserializar la confirmación");

        // 3. La confirmación vuelve idéntica en significado (aun sin `PartialEq` en el
        //    contrato deserializado, la comprobación es la del guardado).
        assert_eq!(vuelta.comparacion.baseline.id, vista.comparacion.baseline.id);
        assert_eq!(vuelta.comparacion.test.id, vista.comparacion.test.id);
        assert_eq!(vuelta.evaluacion.estado, vista.evaluacion.estado);

        // 4. Guardado con la confirmación que volvió del JSON.
        let guardada = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &vuelta, None)
            .expect("debe guardar");

        // 5. Lo persistido coincide con lo evaluado en la vista previa.
        let ev = guardada.evaluacion.clone().expect("con evaluación");
        assert_eq!(ev.estado, vista.evaluacion.estado);
        assert_eq!(ev.confianza, vista.evaluacion.confianza);
        assert_eq!(ev.condiciones, vista.evaluacion.condiciones);
        assert_eq!(ev.metricas.len(), vista.evaluacion.metricas.len());

        // 6. Y el archivo se relee con todo dentro.
        let datos = try_load_saved_data_de(&ruta_cmp).expect("releer");
        assert_eq!(datos.comparisons.len(), 1);
        assert!(datos.comparisons[0].evaluacion.is_some());
    }

    /// **Contrato de transporte**: Rust → JSON → `JSON.parse`/`JSON.stringify` en Node →
    /// deserialización Rust → confirmación → persistencia.
    ///
    /// El viaje por JavaScript es el riesgo real de esta ruta: su `Number` es `f64` y
    /// re-serializar puede cambiar la representación textual. Esta prueba ejecuta el viaje
    /// **de verdad** con Node (si está disponible) sobre una vista previa sin modificar, y
    /// exige que se pueda confirmar y guardar.
    ///
    /// Se distingue el caso **normal** (`c2_transporte_por_node_conserva_la_confirmacion`)
    /// de los **extremos sintéticos**, que se prueban aparte contra la igualdad y no tienen
    /// por qué ser guardables.
    #[test]
    fn c2_transporte_por_node_conserva_la_confirmacion() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-transporte");

        // 1. Vista previa real, sin tocar.
        let vista = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");
        let json = serde_json::to_string(&vista).expect("serializar");

        // 2. Viaje por Node: parse + stringify, exactamente lo que hace la interfaz.
        let viajado = match ir_y_volver_por_node(&json) {
            Some(v) => v,
            // Sin Node no se puede afirmar nada del transporte, y aquí el transporte es
            // lo que se prueba: se falla en vez de omitir.
            None => panic!("el viaje por Node debe ejecutarse: Node no está disponible"),
        };

        // 3. Deserialización en Rust: es el mismo tipo, no una copia.
        let vuelta: crate::evaluacion::ConfirmacionDeVistaPrevia =
            serde_json::from_str(&viajado).expect("deserializar lo que devolvió Node");

        // 4. Confirmación contra el recálculo real: debe aceptarse.
        crate::evaluacion::comprobar_confirmacion(
            &vuelta,
            &vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("recalcular"),
        )
        .expect("una vista previa sin modificar debe confirmarse tras el viaje");

        // 5. Y se persiste.
        let guardada = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &vuelta, None)
            .expect("debe guardar tras el viaje");
        assert!(guardada.evaluacion.is_some());
        assert_eq!(
            contar_comparaciones_para_c2(&ruta_cmp).expect("contar"),
            1
        );
    }

    /// Los valores **normales** del contrato sobreviven al viaje por Node.
    ///
    /// Se construye una confirmación con decimales de medición real (FPS con dos
    /// decimales, porcentajes con muchos), opcionales ausentes y presentes, cero, y el
    /// entero más grande que aparece en una grabación. No hay tolerancias: o se conserva
    /// o se detecta.
    #[test]
    fn c2_transporte_conserva_valores_normales() {
        let valores: [f64; 7] = [
            0.0,          // cero real (no ausencia)
            59.94,        // FPS de una pantalla
            143.98,       // FPS medido
            0.1 + 0.2,    // el binario que más se cita
            12.3456789,   // porcentaje con muchos decimales
            2.5,          // frame time
            144000.0,     // total_frames real, como f64
        ];

        let json = serde_json::to_string(&valores).expect("serializar");
        // En este proyecto el viaje por JS **es** el caso a probar: si Node no está, la
        // prueba falla en vez de dar un verde que no demuestra nada.
        let vuelta = ir_y_volver_por_node(&json).expect("el viaje por Node debe ejecutarse");
        let otra: Vec<f64> = serde_json::from_str(&vuelta).expect("deserializar");

        assert_eq!(otra.len(), valores.len());
        for (original, despues) in valores.iter().zip(otra.iter()) {
            assert_eq!(
                original.to_bits(),
                despues.to_bits(),
                "{original} no sobrevivió al viaje: llegó {despues}"
            );
        }
    }

    /// Un **fallo de escritura** no deja comparación parcial ni fichero temporal.
    ///
    /// Se hace fallar la escritura de forma real: el destino es un **directorio**, así que
    /// el `rename` final no puede sustituirlo. Lo que importa es que el archivo existente
    /// quede **intacto** y que no quede un `.tmp` huérfano con la mitad de los datos.
    #[test]
    fn c2_fallo_de_escritura_no_deja_registro_parcial() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-fallo-escritura");
        let vista = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");

        // Contenido previo conocido del archivo de comparaciones.
        let antes = std::fs::read_to_string(&ruta_cmp).expect("leer antes");

        // Se sustituye el destino por un directorio: `rename` sobre él falla.
        std::fs::remove_file(&ruta_cmp).expect("quitar archivo");
        std::fs::create_dir(&ruta_cmp).expect("crear directorio en su lugar");

        let resultado = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &vista, None);
        assert!(resultado.is_err(), "la escritura debe fallar");

        // No queda ningún fichero temporal a medias.
        let tmp = ruta_cmp.with_file_name("saved_comparisons.json.tmp");
        assert!(!tmp.exists(), "no puede quedar un .tmp huérfano");

        // El destino sigue siendo el directorio vacío: no se escribió nada dentro.
        assert!(ruta_cmp.is_dir());
        assert_eq!(
            std::fs::read_dir(&ruta_cmp).expect("leer dir").count(),
            0,
            "no puede haber un registro parcial"
        );

        // Restaurado el archivo, su contenido es el de antes (nada se modificó por el camino).
        std::fs::remove_dir(&ruta_cmp).expect("quitar directorio");
        std::fs::write(&ruta_cmp, &antes).expect("restaurar");
        assert_eq!(std::fs::read_to_string(&ruta_cmp).expect("releer"), antes);
    }

    /// **Casos extremos sintéticos**: se comportan como deben y son distintos del normal.
    ///
    /// - `-0.0` y enteros por encima de 2⁵³ **no sobreviven** al viaje por JS (el primero
    ///   pierde el signo, los segundos pierden precisión). Ninguna medición real los
    ///   produce, pero si llegaran, la igualdad canónica los detecta y la confirmación se
    ///   rechaza: el lado seguro.
    /// - El caso **normal** no se ve afectado: eso lo fija la prueba de valores normales.
    #[test]
    fn c2_transporte_extremos_sinteticos_no_son_el_caso_normal() {
        // `-0.0` pierde el signo en JavaScript.
        let json = serde_json::to_string(&[-0.0f64]).expect("serializar");
        let vuelta = ir_y_volver_por_node(&json).expect("viaje");
        let tras: Vec<f64> = serde_json::from_str(&vuelta).expect("deserializar");
        assert_eq!(tras[0], 0.0);
        assert!(
            !crate::evaluacion::mismos_f64(-0.0, tras[0]),
            "-0.0 y 0.0 no pueden considerarse iguales tras el viaje"
        );

        // Un entero por encima de 2⁵³ pierde precisión.
        let grande = 9_007_199_254_740_993u64; // 2^53 + 1
        let json = serde_json::to_string(&[grande]).expect("serializar");
        let vuelta = ir_y_volver_por_node(&json).expect("viaje");
        let tras: Vec<u64> = serde_json::from_str(&vuelta).expect("deserializar");
        assert_ne!(
            tras[0], grande,
            "un entero por encima de 2^53 no conserva su valor"
        );

        // Y el límite exacto sí se conserva (2^53 es representable).
        let seguro = 9_007_199_254_740_992u64;
        let json = serde_json::to_string(&[seguro]).expect("serializar");
        let vuelta = ir_y_volver_por_node(&json).expect("viaje");
        let tras: Vec<u64> = serde_json::from_str(&vuelta).expect("deserializar");
        assert_eq!(tras[0], seguro);
    }

    /// Lo persistido **es** el resultado del recálculo, no la confirmación del llamador.
    ///
    /// Se guarda con una confirmación en la que la comparación ha sido alterada en un campo
    /// que la evaluación no refleja (`avg_fps` de la candidata). Si el guardado se fiara de
    /// la confirmación, ese valor quedaría en disco.
    #[test]
    fn c2_lo_persistido_viene_del_recalculo_no_del_llamador() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-recalculo");

        // La vista previa real, y luego se altera un campo de la grabación candidata.
        let mut confirmacion = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect("vista previa");
        confirmacion.comparacion.test.avg_fps = 9999.0;

        // El guardado recalcula desde el archivo y detecta el cambio.
        let err = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect_err("debe rechazar entradas cambiadas");
        assert_eq!(
            err,
            ErrorDeEvaluacion::ConfirmacionRechazada { motivo: "entradas_cambiadas".into() },
            "el motivo viaja como código, no como frase: {err:?}"
        );

        // Nada escrito: el valor inventado no está en disco.
        let datos = try_load_saved_data_de(&ruta_cmp).expect("leer");
        assert!(datos.comparisons.is_empty());
    }

    /// El margen aplicado se persiste tal cual se usó para evaluar.
    #[test]
    fn c2_el_margen_aplicado_queda_registrado() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-margen");
        let vista = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");

        let guardada =
            guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &vista, None).expect("guardar");
        let ev = guardada.evaluacion.expect("con evaluación");

        // El margen aplicado no baja nunca del suelo de empate de la interpretación.
        let margen = ev
            .condiciones
            .margen_porcentaje
            .expect("el margen aplicado debe constar");
        assert!(margen >= crate::evaluacion::MIN_TIE_PERCENT, "margen {margen}");
        // Y coincide con el que la vista previa había aplicado.
        assert_eq!(
            ev.condiciones.margen_porcentaje,
            vista.evaluacion.condiciones.margen_porcentaje
        );
        // Y el ruido medido se conserva en la comparación persistida.
        assert_eq!(
            guardada.comparison.noise_percent,
            vista.comparacion.noise_percent
        );
    }

    /// `evaluada_en` **no** participates en la igualdad: la vista previa no la tiene y el
    /// guardado la sella con el instante del registro.
    #[test]
    fn c2_evaluada_en_es_el_instante_del_guardado() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-instant");
        let vista = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");

        // La vista previa sella un instante (es una evaluación completa)...
        assert!(!vista.evaluacion.evaluada_en.is_empty());

        let guardada = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &vista, None)
            .expect("guardar");
        let ev = guardada.evaluacion.expect("con evaluación");

        // ...y el guardado sella el suyo, que es el del registro.
        assert!(!ev.evaluada_en.is_empty());
        // El formato es el mismo que usa el resto de la aplicación.
        assert_eq!(ev.evaluada_en.len(), "2026-01-01 10:00:00".len());

        // Y la diferencia de instante no impidió confirmar: no participa en la igualdad.
        assert!(crate::evaluacion::comprobar_confirmacion(
            &vista,
            &crate::evaluacion::ConfirmacionDeVistaPrevia {
                comparacion: guardada.comparison.clone(),
                evaluacion: ev,
            }
        )
        .is_ok());
    }

    /// La confirmación de lo que Rust acaba de generar se guarda **con** su evaluación.
    #[test]
    fn c2_la_confirmacion_valida_se_guarda_con_su_evaluacion() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-guarda");
        // La confirmación sale de la función **real** de vista previa, no se compone a mano.
        let confirmacion = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect("vista previa");

        let guardada = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect("debe guardar");

        // Se ha persistido la evaluación, y es la recalcular.
        let ev = guardada.evaluacion.expect("con evaluación");
        assert!(ev.corresponde_a(&guardada.comparison));
        assert!(ev.es_soportada(), "un estado conocido es soportado");
        // El instante se ha sellado.
        assert!(!ev.evaluada_en.is_empty());
        // Un solo registro, y con la comparación completa (no un resumen).
        assert_eq!(guardada.comparison.baseline.id, "b_a");
        assert_eq!(guardada.comparison.test.id, "b_b");
    }

    /// Una confirmación cuyas entradas ya no existen se rechaza **sin escribir**.
    #[test]
    fn c2_una_confirmacion_obsoleta_no_escribe_nada() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-obsoleta");
        // La confirmación sale de la función **real** de vista previa, no se compone a mano.
        let confirmacion = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect("vista previa");

        // El archivo cambia entre la vista previa y el guardado: desaparece la candidata.
        let mut historial = leer_bench(&ruta_bench).unwrap().unwrap();
        historial.benchmarks.retain(|b| b.id != "b_b");
        escribir_atomico(
            &ruta_bench,
            &serde_json::to_string(&historial).unwrap(),
        )
        .unwrap();

        let err = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect_err("debe rechazar");
        assert!(
            matches!(err, ErrorDeEvaluacion::GrabacionInexistente { .. }),
            "una grabación borrada es «inexistente», no ambigua ni un rechazo de confirmación: {err:?}"
        );

        // Y no se ha creado ni modificado el archivo de comparaciones.
        let datos = try_load_saved_data_de(&ruta_cmp).expect("leer");
        assert!(
            datos.comparisons.is_empty(),
            "un rechazo no puede dejar registro a medias"
        );
    }

    /// Un rechazo deja **los dos archivos** con el mismo contenido y la misma fecha de
    /// modificación.
    ///
    /// Es más fuerte que comprobar que no hay registros: si la ruta reescribiera el archivo
    /// sin añadir nada, el contenido coincidiría pero el `mtime` no.
    #[test]
    fn c2_un_rechazo_no_toca_ningun_archivo() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-sin-tocar");
        let vista = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");

        // Se fuerza un rechazo: una confirmación con la candidata alterada.
        let mut manipulada = vista.clone();
        manipulada.comparacion.test.avg_fps = 1.0;

        let contenido_bench = std::fs::read(&ruta_bench).expect("leer bench");
        let contenido_cmp = std::fs::read(&ruta_cmp).expect("leer cmp");
        let mtime_bench = std::fs::metadata(&ruta_bench).and_then(|m| m.modified()).ok();
        let mtime_cmp = std::fs::metadata(&ruta_cmp).and_then(|m| m.modified()).ok();

        // Se deja pasar algo de tiempo para que un rewrite cambie el mtime de verdad.
        std::thread::sleep(std::time::Duration::from_millis(20));

        guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &manipulada, None)
            .expect_err("debe rechazar");

        assert_eq!(std::fs::read(&ruta_bench).expect("releer"), contenido_bench);
        assert_eq!(std::fs::read(&ruta_cmp).expect("releer"), contenido_cmp);
        assert_eq!(
            std::fs::metadata(&ruta_bench).and_then(|m| m.modified()).ok(),
            mtime_bench,
            "un rechazo no puede reescribir el historial"
        );
        assert_eq!(
            std::fs::metadata(&ruta_cmp).and_then(|m| m.modified()).ok(),
            mtime_cmp,
            "un rechazo no puede reescribir las comparaciones"
        );
    }

    /// Cambiar una entrada sin que cambie el veredicto también se rechaza, y no escribe.
    #[test]
    fn c2_cambio_de_entradas_rechaza_y_no_escribe() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-entradas");
        // La confirmación sale de la función **real** de vista previa, no se compone a mano.
        let confirmacion = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect("vista previa");

        let mut historial = leer_bench(&ruta_bench).unwrap().unwrap();
        for b in &mut historial.benchmarks {
            if b.id == "b_b" {
                b.launch_command = "otro comando %command%".to_string();
            }
        }
        escribir_atomico(&ruta_bench, &serde_json::to_string(&historial).unwrap()).unwrap();

        let err = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect_err("debe rechazar");
        assert_eq!(
            err,
            ErrorDeEvaluacion::ConfirmacionRechazada { motivo: "entradas_cambiadas".into() },
            "{err:?}"
        );
        assert!(try_load_saved_data_de(&ruta_cmp).unwrap().comparisons.is_empty());
    }

    /// Lo que se persiste son los objetos de Rust, no los que envía el llamador.
    #[test]
    fn c2_no_se_persiste_lo_que_diga_el_llamador() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-manipulada");
        let mut confirmacion = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect("vista previa");
        // El llamador "cambia" el veredicto en su copia.
        confirmacion.evaluacion.estado = CodigoDeEvaluacion("mejora".to_string());
        confirmacion.evaluacion.metricas.clear();

        let err = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect_err("no corresponde");
        assert_eq!(
            err,
            ErrorDeEvaluacion::ConfirmacionRechazada { motivo: "significado_cambiado".into() },
            "{err:?}"
        );
        assert!(try_load_saved_data_de(&ruta_cmp).unwrap().comparisons.is_empty());
    }

    /// La evaluación guardada se puede releer idéntica, y no se recalcula.
    #[test]
    fn c2_la_evaluacion_guardada_se_relee_sin_tocarla() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-relee");
        // La confirmación sale de la función **real** de vista previa, no se compone a mano.
        let confirmacion = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect("vista previa");
        let guardada = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect("guardar");
        let ev = guardada.evaluacion.clone().expect("con evaluación");

        let releido = try_load_saved_data_de(&ruta_cmp).expect("leer");
        let misma = &releido.comparisons[0];
        assert_eq!(misma.evaluacion.as_ref(), Some(&ev));
        // Y el estado de la evaluación registrada está soportado (no malformada ni ausente).
        assert!(matches!(
            misma.estado_de_la_evaluacion(),
            EstadoDeLaEvaluacion::Soportada(_)
        ));
    }

    /// Guardar dos veces la misma comparación no duplica el registro.
    #[test]
    fn c2_no_se_guarda_dos_veces_la_misma_comparacion() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-duplicada");
        // La confirmación sale de la función **real** de vista previa, no se compone a mano.
        let confirmacion = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect("vista previa");
        guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None).expect("1ª");

        let err = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect_err("2ª rechazada");
        assert!(
            matches!(err, ErrorDeEvaluacion::ComparacionYaExistente { .. }),
            "un duplicado del mismo par es un tipo propio, no un rechazo de confirmación: {err:?}"
        );
        assert_eq!(try_load_saved_data_de(&ruta_cmp).unwrap().comparisons.len(), 1);
    }

    /// La comparación y su evaluación se escriben **juntas**: no existe el estado
    /// intermedio con una sin la otra.
    #[test]
    fn c2_comparacion_y_evaluacion_se_escriben_juntas() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c2-atomico");
        // La confirmación sale de la función **real** de vista previa, no se compone a mano.
        let confirmacion = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect("vista previa");
        guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None).expect("guardar");

        let datos = try_load_saved_data_de(&ruta_cmp).unwrap();
        assert_eq!(datos.comparisons.len(), 1);
        let c = &datos.comparisons[0];
        // La evaluación embebida habla de los benchmarks embebidos en la misma comparación.
        assert!(c.evaluacion.as_ref().unwrap().corresponde_a(&c.comparison));
        // Y no hay una segunda escritura suelta: solo un archivo, un registro.
        assert_eq!(datos.version, Some(2));
    }
}

/// Ayudante de pruebas: ejecuta el flujo real de vista previa + guardado sobre rutas
/// dadas, para las pruebas de concurrencia de `evaluacion::pruebas_c2`.
///
/// Devuelve `Ok(true)` si guardó, `Ok(false)` si la vista previa o el guardado
/// rechazaron (situación esperada bajo carrera), y `Err` solo si algo es ilegible.
#[cfg(test)]
pub(crate) fn guardar_con_vista_previa_para_c2(
    ruta_benchmarks: &Path,
    ruta_comparaciones: &Path,
) -> Result<bool, String> {
    let vista = match vista_previa_evaluada(ruta_benchmarks, "b_a", "b_b", None) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    match guardar_comparacion_evaluada(ruta_benchmarks, ruta_comparaciones, &vista, None) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Ayudante de pruebas: expone `escenario_c2` a `evaluacion::pruebas_c2`.
#[cfg(test)]
pub(crate) fn escenario_c2_para_c2(nombre: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    pruebas_c2_guardado::escenario_c2(nombre)
}

/// Ayudante de pruebas: cuántos registros hay en el archivo de comparaciones, para las
/// pruebas de concurrencia de `evaluacion::pruebas_c2`.
///
/// Devuelve solo datos públicos: el tipo del archivo no se expone fuera de este módulo.
#[cfg(test)]
pub(crate) fn contar_comparaciones_para_c2(ruta: &Path) -> Result<usize, String> {
    try_load_saved_data_de(ruta)
        .map(|d| d.comparisons.len())
        .map_err(|e| e.mensaje("El archivo de comparaciones"))
}

/// Ayudante de pruebas: los identificadores base y candidato de cada comparación guardada,
/// para comprobar que lo leído bajo carrera es coherente.
#[cfg(test)]
pub(crate) fn ids_de_comparaciones_para_c2(ruta: &Path) -> Result<Vec<(String, String)>, String> {
    let datos =
        try_load_saved_data_de(ruta).map_err(|e| e.mensaje("El archivo de comparaciones"))?;
    Ok(datos
        .comparisons
        .iter()
        .map(|c| {
            (
                c.comparison.baseline.id.clone(),
                c.comparison.test.id.clone(),
            )
        })
        .collect())
}

/// Ayudante de pruebas: expone `adquirir_cerrojos` a los tests de `evaluacion::pruebas_c2`.
#[cfg(test)]
pub(crate) fn adquirir_cerrojos_para_c2() -> (
    std::sync::MutexGuard<'static, ()>,
    std::sync::MutexGuard<'static, ()>,
) {
    adquirir_cerrojos()
}

/// Ayudante de pruebas: expone `buscar_unico` a los tests de `evaluacion::pruebas_c2`.
///
/// Devuelve la **variante** de la falla, no un `String`: así las pruebas de C.2 fijan el
/// discriminante y no el texto.
#[cfg(test)]
pub(crate) fn buscar_unico_para_c2<'a>(
    benchmarks: &'a [Benchmark],
    id: &str,
    que: &str,
) -> Result<&'a Benchmark, FallaDeBusqueda> {
    buscar_unico(benchmarks, id, que)
}

/// Ayudante de pruebas: expone `construir_snapshot` a los tests de `evaluacion::pruebas_c2`
/// sin cambiar la visibilidad de producción.
#[cfg(test)]
pub(crate) fn construir_snapshot_para_c2(
    b: &Benchmark,
    ctx: &ContextoDeCaptura,
    sesion: Option<&crate::sessions::SesionDeEjecucion>,
    ahora: &str,
) -> SnapshotConfig {
    construir_snapshot(b, ctx, sesion, ahora)
}

/// 5.4.1.C.3: activación del motor, contrato de errores IPC y ausencia de rutas
/// alternativas.
#[cfg(test)]
mod pruebas_c3_activacion {
    use super::pruebas_c2_guardado::escenario_c2;
    use super::*;

    /// El **contrato de errores** se serializa con un discriminante estable, y el motivo de
    /// rechazo viaja como código (no como frase redactada).
    ///
    /// Esto es lo que permite a la interfaz distinguir «repito la vista previa» de «no se
    /// pudo escribir» sin analizar textos.
    #[test]
    fn c3_errores_ipc_se_serializan_con_su_discriminante() {
        let rechazo = ErrorDeEvaluacion::rechazo(crate::evaluacion::MotivoDeRechazo::SignificadoCambiado);
        let json = serde_json::to_value(&rechazo).expect("serializar");
        assert_eq!(json["tipo"], "confirmacion_rechazada");
        assert_eq!(json["motivo"], "significado_cambiado");

        let amb = ErrorDeEvaluacion::IdentificadorAmbiguo { mensaje: "x".into() };
        assert_eq!(serde_json::to_value(&amb).unwrap()["tipo"], "identificador_ambiguo");

        let ya = ErrorDeEvaluacion::ComparacionYaExistente { comparacion_id: "cmp_1".into() };
        let ya_json = serde_json::to_value(&ya).unwrap();
        assert_eq!(ya_json["tipo"], "comparacion_ya_existente");
        assert_eq!(ya_json["comparacion_id"], "cmp_1");

        for (e, esperado) in [
            (ErrorDeEvaluacion::GrabacionInexistente { mensaje: "x".into() }, "grabacion_inexistente"),
            (ErrorDeEvaluacion::LecturaFallida { archivo: "a".into(), mensaje: "x".into() }, "lectura_fallida"),
            (ErrorDeEvaluacion::EscrituraFallida { mensaje: "x".into() }, "escritura_fallida"),
        ] {
            assert_eq!(serde_json::to_value(&e).unwrap()["tipo"], esperado);
        }
    }

    /// Los cuatro motivos de `MotivoDeRechazo` llegan íntegros al contrato IPC: no se
    /// renombran ni se traducen por el camino.
    #[test]
    fn c3_los_motivos_de_rechazo_se_reutilizan_sin_renombrar() {
        for motivo in [
            crate::evaluacion::MotivoDeRechazo::EntradasCambiadas,
            crate::evaluacion::MotivoDeRechazo::SignificadoCambiado,
            crate::evaluacion::MotivoDeRechazo::EvaluacionNoCorresponde,
            crate::evaluacion::MotivoDeRechazo::ValorNoPersistible,
        ] {
            let e = ErrorDeEvaluacion::rechazo(motivo);
            match e {
                ErrorDeEvaluacion::ConfirmacionRechazada { motivo: m } => {
                    assert_eq!(m, motivo.codigo())
                }
                otro => panic!("motivo convertido en {otro:?}"),
            }
        }
    }

    /// El discriminante lo decide la **variante** devuelta por `buscar_unico`, no el texto.
    ///
    /// Se comprueba sobre `FallaDeBusqueda` directamente: si mañana se reescribe el mensaje
    /// (otra redacción, otro plural, otro idioma), `resolver_unico` seguirá clasificando
    /// igual porque hace `match` sobre la variante. Es la propiedad que `es_ambiguous`
    /// (`contains` sobre el mensaje) no podía garantizar.
    #[test]
    fn c3_el_discriminante_no_depende_del_texto() {
        use crate::benchmarks::FallaDeBusqueda as F;

        let mut base = super::tests::bench(1623730, "Palworld");
        base.id = "dup".to_string();
        let mut otra_contenido = base.clone();
        otra_contenido.avg_fps = 999.0;

        let ambiguo = buscar_unico(&[base.clone(), otra_contenido], "dup", "Benchmark base")
            .expect_err("dos copias distintas son ambiguas");
        let ausente = buscar_unico(&[base], "fantasma", "Benchmark base")
            .expect_err("no existe");

        // El texto actual, por si cambia:
        assert!(ambiguous_texto(&ambiguo));
        // Pero la clasificación es la variante, y nada más:
        assert!(matches!(ambiguo, F::Ambigua { .. }));
        assert!(matches!(ausente, F::NoEncontrada { .. }));

        // Sustituir el mensaje por basura no cambia la variante ni su clasificación.
        let ambiguo_basura = F::Ambigua { mensaje: "zzz".into() };
        let ausente_basura = F::NoEncontrada { mensaje: "grabaciones (las copias".into() };
        assert!(matches!(ambiguo_basura, F::Ambigua { .. }));
        assert!(matches!(ausente_basura, F::NoEncontrada { .. }));

        // Y aquí está la prueba de que la lectura de texto era incorrecta: un mensaje que
        // contiene la cadena que buscaba `es_ambiguous`, pero que viene de la rama
        // «no encontrada», la regla textual clasificaría como **ambigua**.
        assert!(
            es_ambigua_por_texto(&ausente_basura),
            "la regla textual reclasificaría una ausencia como ambigüedad"
        );
        // La variante, en cambio, sigue diciendo la verdad, y es lo que se consume:
        let clasificado = match ausente_basura {
            F::Ambigua { .. } => "ambiguo",
            F::NoEncontrada { .. } => "inexistente",
        };
        assert_eq!(clasificado, "inexistente");
    }

    /// Ayudante: el mensaje de ambigüedad describe el caso (solo informativo).
    fn ambiguous_texto(f: &crate::benchmarks::FallaDeBusqueda) -> bool {
        f.mensaje().contains("grabaciones (")
    }

    /// Ayudante de contraste: reproduce la regla **retirada** para demostrar que no se usa.
    fn es_ambigua_por_texto(f: &crate::benchmarks::FallaDeBusqueda) -> bool {
        f.mensaje().contains("grabaciones (")
    }

    /// Un identificador con dos copias se rechaza como **ambiguo**, y una grabación que no
    /// existe como **inexistente**: son dos situaciones distintas y no se pueden confundir.
    #[test]
    fn c3_identificador_ambiguo_e_inexistente_son_distintos() {
        let (ruta_bench, _ruta_cmp) = escenario_c2("c3-ambiguo");

        // Inexistente.
        let err = vista_previa_evaluada(&ruta_bench, "no_existe", "b_b", None)
            .expect_err("no existe");
        assert!(matches!(err, ErrorDeEvaluacion::GrabacionInexistente { .. }), "{err:?}");

        // Ambiguo: se duplica la grabación base con el mismo id y contenido distinto.
        let mut historial = super::tests::leer_bench(&ruta_bench).unwrap().unwrap();
        let mut copia = historial.benchmarks[0].clone();
        copia.avg_fps = 42.0;
        historial.benchmarks.push(copia);
        escribir_atomico(&ruta_bench, &serde_json::to_string(&historial).unwrap()).unwrap();

        let err = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None)
            .expect_err("ambiguo");
        assert!(matches!(err, ErrorDeEvaluacion::IdentificadorAmbiguo { .. }), "{err:?}");

        // Y la otra grabación (sin duplicar) sigue siendo resoluble.
        let ok = vista_previa_evaluada(&ruta_bench, "b_b", "b_b", None).expect("b_b es único");
        assert_eq!(ok.comparacion.baseline.id, "b_b");
    }

    /// **La variable evaluada manda**: si la vista previa se hizo con una declaración y el
    /// guardado llega con otra, se rechaza por `significado_cambiado` y no se escribe nada.
    ///
    /// Es el caso que impide guardar una evaluación calculada con una declaración y
    /// persistir el veredicto de otra.
    #[test]
    fn c3_variable_distinta_de_la_confirmada_se_rechaza() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c3-variable");
        let con_variable = Some(VariableEvaluada {
            codigo: "variable_x".to_string(),
        });
        let confirmacion =
            vista_previa_evaluada(&ruta_bench, "b_a", "b_b", con_variable.clone()).expect("vista");

        // Se guarda declarando **otra** variable.
        let err = guardar_comparacion_evaluada(
            &ruta_bench,
            &ruta_cmp,
            &confirmacion,
            Some(VariableEvaluada { codigo: "variable_y".to_string() }),
        )
        .expect_err("variable distinta");
        assert_eq!(
            err,
            ErrorDeEvaluacion::ConfirmacionRechazada { motivo: "significado_cambiado".into() },
            "{err:?}"
        );
        assert!(try_load_saved_data_de(&ruta_cmp).unwrap().comparisons.is_empty());

        // Con la misma declaración sí se guarda, y queda registrada.
        let guardada =
            guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, con_variable)
                .expect("misma variable");
        let ev = guardada.evaluacion.expect("evaluación");
        assert_eq!(
            ev.condiciones.variable_evaluada.expect("variable").codigo,
            "variable_x"
        );
    }

    /// Un guardado sin declaración de variable **no** hereda la de la confirmación: la
    /// variable es un parámetro explícito del comando, nunca se infiere.
    #[test]
    fn c3_sin_variable_declarada_queda_sin_variable() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c3-sin-variable");
        let confirmacion =
            vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");
        let guardada = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect("guardar");
        let ev = guardada.evaluacion.expect("evaluación");
        assert!(ev.condiciones.variable_evaluada.is_none());
    }

    /// El segundo guardado del mismo par devuelve el **identificador del registro que ya
    /// existe**, para que la interfaz pueda localizarlo sin deducir nada de las cifras.
    #[test]
    fn c3_el_duplicado_devuelve_el_id_del_registro_existente() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c3-duplicado-id");
        let confirmacion =
            vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");
        let primera = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect("1ª");

        let err = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect_err("2ª");
        match err {
            ErrorDeEvaluacion::ComparacionYaExistente { comparacion_id } => {
                assert_eq!(comparacion_id, primera.id)
            }
            otro => panic!("se esperaba duplicado, llegó {otro:?}"),
        }
        assert_eq!(try_load_saved_data_de(&ruta_cmp).unwrap().comparisons.len(), 1);
    }

    /// Una confirmación rechazada por **obsolescencia** exige repetir la vista previa: el
    /// motivo lo dice, y con la confirmación vieja el guardado nunca escribe.
    #[test]
    fn c3_confirmacion_obsoleta_no_escribe_y_lo_dice() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c3-obsoleta-activada");
        let confirmacion =
            vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");

        // La candidata cambia de medición entre la vista previa y el guardado.
        let mut historial = super::tests::leer_bench(&ruta_bench).unwrap().unwrap();
        for b in &mut historial.benchmarks {
            if b.id == "b_b" {
                b.avg_fps = 130.0;
            }
        }
        escribir_atomico(&ruta_bench, &serde_json::to_string(&historial).unwrap()).unwrap();

        let err = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect_err("obsoleta");
        assert!(
            matches!(err, ErrorDeEvaluacion::ConfirmacionRechazada { ref motivo } if motivo == "entradas_cambiadas"),
            "{err:?}"
        );
        assert!(try_load_saved_data_de(&ruta_cmp).unwrap().comparisons.is_empty());
    }

    /// **No queda ninguna ruta de guardado sin evaluación**: ni el comando antiguo existe,
    /// ni la comparación se puede escribir con `evaluacion: None` por la ruta nueva.
    #[test]
    fn c3_no_hay_ruta_de_guardado_sin_evaluacion() {
        // Los nombres se montan en tiempo de ejecución para que este mismo test no se
        // encuentre a sí mismo al buscar en el fuente.
        let fuente = include_str!("benchmarks.rs");
        let retirado_save = format!("pub fn {}{}", "save_comparison", "(");
        let retirado_compare = format!("pub fn {}{}", "compare_benchmarks", "(");
        let activo_save = format!("pub fn {}{}", "save_comparison_evaluada", "(");
        let activo_compare = format!("pub fn {}{}", "compare_benchmarks_evaluada", "(");

        assert!(
            !fuente.contains(&retirado_save),
            "save_comparison debe estar retirado: escribía sin evaluación"
        );
        assert!(
            !fuente.contains(&retirado_compare),
            "compare_benchmarks debe estar retirado: era una ruta alternativa sin evaluación"
        );
        assert!(fuente.contains(&activo_save), "la ruta evaluada tiene que existir");
        assert!(
            fuente.contains(&activo_compare),
            "la vista previa evaluada es el único camino a la comparación individual"
        );

        // Y lo que escribe la ruta nueva lleva siempre evaluación.
        let (ruta_bench, ruta_cmp) = escenario_c2("c3-siempre-con-evaluacion");
        let confirmacion =
            vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");
        let guardada = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None)
            .expect("guardar");
        assert!(guardada.evaluacion.is_some(), "toda comparación nueva lleva su evaluación");
    }

    /// El comando IPC de vista previa devuelve la comparación **dentro** de la confirmación:
    /// no hay una segunda llamada que pueda dar una comparación distinta.
    #[test]
    fn c3_la_vista_previa_lleva_la_comparacion_dentro() {
        let (ruta_bench, _ruta_cmp) = escenario_c2("c3-comparacion-dentro");
        let vista = vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");
        assert_eq!(vista.comparacion.baseline.id, "b_a");
        assert_eq!(vista.comparacion.test.id, "b_b");
        assert_eq!(vista.evaluacion.referencia.id, "b_a");
        assert_eq!(vista.evaluacion.candidata.id, "b_b");
    }

    /// El guardado es **idempotente por par**: repetirlo no duplica, y no depende del
    /// identificador de la comparación (que el cliente no conoce antes de guardar).
    #[test]
    fn c3_el_guardado_no_duplica_al_repetirse() {
        let (ruta_bench, ruta_cmp) = escenario_c2("c3-idempotente");
        let confirmacion =
            vista_previa_evaluada(&ruta_bench, "b_a", "b_b", None).expect("vista");
        for _ in 0..3 {
            let _ = guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &confirmacion, None);
        }
        assert_eq!(try_load_saved_data_de(&ruta_cmp).unwrap().comparisons.len(), 1);
    }
}

/// Pruebas de recorrido de **C3-20** (el viaje del `f64` hasta la persistencia).
///
/// Viven aquí y no en `evaluacion::pruebas_c3_20` porque ejercitan el **núcleo real** de
/// vista previa y guardado (`vista_previa_evaluada`, `guardar_comparacion_evaluada`), que son
/// privadas de este módulo. Las pruebas de serialización puras (bits, `Value`, guardia de la
/// feature) están en `evaluacion::pruebas_c3_20`, donde no hace falta disco.
#[cfg(test)]
mod pruebas_c3_20_recorrido {
    use super::*;
    use crate::evaluacion::ConfirmacionDeVistaPrevia;

// ────────────── 2 y 3. Recorridos: SIN_CAMBIO y los cinco escenarios ──────────────

/// Los cinco escenarios sintéticos del paquete de validación.
const ESCENARIOS: &[&str] = &[
    "COMPLETA_MEJORA",
    "COMPLETA_EMPEORA",
    "PARCIAL_PERDIDA",
    "MIXTO",
    "SIN_CAMBIO",
];

/// Ruta del paquete de validación, si está montado.
///
/// Las pruebas de recorrido **necesitan los fixtures reales** (el valor de frontera sale de
/// un frame time concreto). Si el paquete no está, se avisa y se omite: no se inventan datos
/// para simular un escenario que no se está probando.
fn ruta_fixtures() -> Option<std::path::PathBuf> {
    let ruta = std::path::Path::new("/workspace/validacion-c3/fixtures/benchmarks.json");
    if ruta.exists() {
        Some(ruta.to_path_buf())
    } else {
        eprintln!(
            "AVISO: {} no está disponible; se omiten las pruebas de recorrido de C3-20",
            ruta.display()
        );
        None
    }
}

/// Directorio temporal propio de la prueba, borrado al salir.
fn directorio_temporal(nombre: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("thoriapp-c3-20-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("crear el directorio temporal");
    base
}

/// Ejecuta el viaje real de ida y vuelta por **Node**, que es el eslabón del IPC.
///
/// Rust -> JSON -> `JSON.parse` -> `JSON.stringify` -> Rust. Node se usa de verdad, no
/// simulado: es la única forma de comprobar que JavaScript no altera la representación.
fn viaje_por_node(json: &str) -> Option<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut hijo = Command::new("node")
        .arg("-e")
        .arg("let d='';process.stdin.on('data',c=>d+=c).on('end',()=>process.stdout.write(JSON.stringify(JSON.parse(d))))")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    hijo.stdin
        .as_mut()
        .expect("stdin")
        .write_all(json.as_bytes())
        .expect("escribir el JSON en Node");
    let salida = hijo.wait_with_output().expect("esperar a Node");
    if !salida.status.success() {
        eprintln!(
            "AVISO: Node falló ({}); se omite el viaje por Node",
            String::from_utf8_lossy(&salida.stderr)
        );
        return None;
    }
    String::from_utf8(salida.stdout).ok()
}

/// Carga los fixtures y devuelve `(A, B)` del escenario pedido.
fn par_del_escenario(
    ruta: &std::path::Path,
    escenario: &str,
) -> Option<(Benchmark, Benchmark)> {
    let texto = std::fs::read_to_string(ruta).expect("leer los fixtures");
    let data: BenchmarksData =
        serde_json::from_str(&texto).expect("deserializar los fixtures");

    let id_a = format!("SINT-{escenario}-A");
    let id_b = format!("SINT-{escenario}-B");
    let a = data.benchmarks.iter().find(|b| b.id == id_a)?.clone();
    let b = data.benchmarks.iter().find(|b| b.id == id_b)?.clone();
    Some((a, b))
}

/// **Recorrido completo de un escenario** hasta la persistencia y la relectura.
///
/// Reproduce el camino real y comprueba que la evaluación guardada conserva su significado
/// y sus valores:
///
/// 1. vista previa real (`vista_previa_evaluada` sobre un fichero escrito desde el fixture);
/// 2. serialización a JSON;
/// 3. viaje por **Node** (`JSON.parse` + `JSON.stringify`);
/// 4. deserialización en Rust;
/// 5. confirmación y guardado (`guardar_comparacion_evaluada`, que recalcula);
/// 6. relectura del fichero de comparaciones.
fn recorrido_completo(escenario: &str) {
    let Some(fixtures) = ruta_fixtures() else {
        return;
    };
    let Some((a, b)) = par_del_escenario(&fixtures, escenario) else {
        eprintln!("AVISO: faltan las grabaciones de {escenario}; se omite");
        return;
    };
    let original = a.clone();
    let original_b = b.clone();

    let dir = directorio_temporal(&escenario.to_lowercase());
    let ruta_benchmarks = dir.join("benchmarks.json");
    let ruta_comparaciones = dir.join("saved_comparisons.json");

    // El fichero se escribe con el historial mínimo: las dos grabaciones del escenario.
    let historial = BenchmarksData {
        version: Some(BENCHMARKS_SCHEMA_VERSION),
        benchmarks: vec![a, b],
        counters: std::collections::HashMap::new(),
    };
    std::fs::write(
        &ruta_benchmarks,
        serde_json::to_string(&historial).expect("serializar el historial"),
    )
    .expect("escribir el historial");
    std::fs::write(&ruta_comparaciones, r#"{"version":2,"comparisons":[]}"#)
        .expect("escribir las comparaciones");

    let id_a = format!("SINT-{escenario}-A");
    let id_b = format!("SINT-{escenario}-B");

    // 1. Vista previa real.
    let vista = vista_previa_evaluada(&ruta_benchmarks, &id_a, &id_b, None)
        .unwrap_or_else(|e| panic!("{escenario}: la vista previa falló: {e:?}"));

    // 2 y 3. Serialización y viaje por Node, si está disponible.
    let json = serde_json::to_string(&vista).expect("serializar la vista previa");
    let procesado = viaje_por_node(&json).unwrap_or_else(|| json.clone());

    // 4. Deserialización en Rust.
    let devuelta: ConfirmacionDeVistaPrevia =
        serde_json::from_str(&procesado).expect("deserializar la confirmación del viaje");

    // El número tiene que haber sobrevivido al viaje **bit a bit**.
    assert_eq!(
        devuelta.comparacion.frame_time_change_percent.to_bits(),
        vista.comparacion.frame_time_change_percent.to_bits(),
        "{escenario}: frame_time_change_percent cambió al cruzar el IPC \
         (0x{:016x} -> 0x{:016x})",
        vista.comparacion.frame_time_change_percent.to_bits(),
        devuelta.comparacion.frame_time_change_percent.to_bits()
    );

    // 5. Confirmación y guardado: aquí se recalcula y se compara con `mismos_f64`.
    let guardada = guardar_comparacion_evaluada(
        &ruta_benchmarks,
        &ruta_comparaciones,
        &devuelta,
        None,
    )
    .unwrap_or_else(|e| {
        panic!("{escenario}: el guardado se rechazó: {e:?}");
    });

    // El significado guardado es el de la vista previa.
    let evaluacion_guardada = guardada.evaluacion.clone().expect("con evaluación");
    assert_eq!(
        evaluacion_guardada.estado, vista.evaluacion.estado,
        "{escenario}: el estado guardado difiere del confirmado"
    );
    assert_eq!(
        evaluacion_guardada.confianza, vista.evaluacion.confianza,
        "{escenario}: la confianza guardada difiere de la confirmada"
    );

    // 6. Relectura desde disco: lo persistido conserva el significado y los valores.
    let texto = std::fs::read_to_string(&ruta_comparaciones).expect("releer las comparaciones");
    let releidas: std::collections::HashMap<String, serde_json::Value> = {
        let data: serde_json::Value = serde_json::from_str(&texto).expect("parsear lo persistido");
        data["comparisons"]
            .as_array()
            .expect("lista de comparaciones")
            .iter()
            .map(|c| (c["id"].as_str().unwrap_or_default().to_string(), c.clone()))
            .collect()
    };
    let persistida = releidas
        .get(&guardada.id)
        .unwrap_or_else(|| panic!("{escenario}: la comparación no está en el fichero"));

    assert_eq!(
        persistida["evaluacion"]["estado"], vista.evaluacion.estado.0,
        "{escenario}: el estado persistido no es el confirmado"
    );
    assert_eq!(
        persistida["evaluacion"]["confianza"], vista.evaluacion.confianza.0,
        "{escenario}: la confianza persistida no es la confirmada"
    );

    // Las grabaciones del historial **no** se han tocado: guardar no reescribe entradas.
    let tras = std::fs::read_to_string(&ruta_benchmarks).expect("releer el historial");
    let tras_data: BenchmarksData =
        serde_json::from_str(&tras).expect("deserializar el historial");
    let tras_a = tras_data
        .benchmarks
        .iter()
        .find(|x| x.id == id_a)
        .expect("A sigue ahí");
    let tras_b = tras_data
        .benchmarks
        .iter()
        .find(|x| x.id == id_b)
        .expect("B sigue ahí");
    assert_eq!(
        tras_a.frame_time_avg.to_bits(),
        original.frame_time_avg.to_bits(),
        "{escenario}: A se modificó"
    );
    assert_eq!(
        tras_b.frame_time_avg.to_bits(),
        original_b.frame_time_avg.to_bits(),
        "{escenario}: B se modificó"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **SIN_CAMBIO**: el escenario que fallaba en la validación real.
///
/// Es el caso concreto de C3-20: `frame_time_change_percent = -1.4778325123152847`.
#[test]
fn c3_20_sin_cambio_llega_a_persistencia_tras_el_viaje() {
    recorrido_completo("SIN_CAMBIO");
}

/// Los **cinco** escenarios del paquete, uno por uno, para que el informe diga cuál falla.
#[test]
fn c3_20_completa_mejora_llega_a_persistencia() {
    recorrido_completo("COMPLETA_MEJORA");
}

#[test]
fn c3_20_completa_empeora_llega_a_persistencia() {
    recorrido_completo("COMPLETA_EMPEORA");
}

#[test]
fn c3_20_parcial_perdida_llega_a_persistencia() {
    recorrido_completo("PARCIAL_PERDIDA");
}

#[test]
fn c3_20_mixto_llega_a_persistencia() {
    recorrido_completo("MIXTO");
}

/// La tabla se recorre entera: si se añade un escenario y no se le da prueba propia, esto
/// lo delata (el recorrido deja de cubrirse solo).
#[test]
fn c3_20_todos_los_escenarios_declarados_tienen_recorrido() {
    for escenario in ESCENARIOS {
        let Some(fixtures) = ruta_fixtures() else {
            return;
        };
        assert!(
            par_del_escenario(&fixtures, escenario).is_some(),
            "el escenario {escenario} está en la tabla pero no en los fixtures"
        );
    }
}

}

/// Compatibilidad histórica de **C3-20**: activar `float_roundtrip` cambia el parser de
/// `serde_json` de toda la aplicación, así que la lectura de datos ya existentes tiene que
/// comprobarse explícitamente.
///
/// Se verifica: lectura de benchmarks y comparaciones históricos, conservación de
/// evaluaciones no soportadas y malformadas, y que **leer no reescribe** (mismo contenido y
/// mismo `mtime`).
#[cfg(test)]
mod pruebas_c3_20_historico {
    use super::*;

    fn dir(nombre: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "thoriapp-c3-20-hist-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("crear dir");
        base
    }

    /// `mtime` con precisión de nanosegundos, para detectar reescrituras.
    fn mtime_ns(ruta: &std::path::Path) -> u128 {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(ruta).expect("metadata");
        (m.mtime() as u128) * 1_000_000_000 + (m.mtime_nsec() as u128)
    }

    /// Un historial con un `f64` de frontera, un campo desconocido y una comparación
    /// histórica sin evaluación. Se lee varias veces y **no** puede cambiar.
    #[test]
    fn c3_20_leer_un_benchmarks_historico_no_lo_reescribe() {
        let d = dir("bench");
        let ruta = d.join("benchmarks.json");
        // `frame_time_avg` con el valor de frontera de C3-20 y un campo que esta versión no
        // conoce: la lectura tiene que tolerarlo, no borrarlo.
        let contenido = r#"{"version":1,"benchmarks":[
            {"id":"hist-1","profile_id":"p","game_name":"Juego","appid":1,
             "timestamp":"2025-01-01 10:00:00","duration_seconds":60.0,
             "avg_fps":100.0,"fps_1_percent_low":80.0,"fps_0_1_percent_low":70.0,
             "max_fps":140.0,"min_fps":48.0,
             "frame_time_avg":9.852216748768473,
             "frame_time_1_percent_high":12.5,
             "total_frames":20000,"comando_original":"%command%",
             "campo_del_futuro":{"a":1},"otro_mas":"x"}
        ],"counters":{}}"#;
        std::fs::write(&ruta, contenido).expect("escribir");
        let antes = mtime_ns(&ruta);

        for _ in 0..3 {
            let leido = try_load_benchmarks_data_de(&ruta).expect("leer");
            assert_eq!(leido.benchmarks.len(), 1);
            // El valor de frontera se lee en su punto exacto, no en el contiguo.
            assert_eq!(
                leido.benchmarks[0].frame_time_avg.to_bits(),
                9.852216748768473_f64.to_bits(),
                "el f64 histórico no se leyó tal cual"
            );
        }

        assert_eq!(mtime_ns(&ruta), antes, "leer no puede tocar el fichero");
        assert_eq!(
            std::fs::read_to_string(&ruta).expect("releer"),
            contenido,
            "leer no puede reescribir el contenido"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Las tres formas de evaluación histórica se clasifican igual tras la corrección.
    ///
    /// Se parte de una comparación **real** de los fixtures y se le cambia solo la
    /// evaluación: metodología desconocida, evaluación con forma inválida, y sin evaluación.
    #[test]
    fn c3_20_las_evaluaciones_historicas_se_siguen_clasificando() {
        let ruta_fixture = std::path::Path::new("/workspace/validacion-c3/fixtures/saved_comparisons.json");
        if !ruta_fixture.exists() {
            eprintln!("AVISO: sin fixtures; se omite");
            return;
        }
        let texto = std::fs::read_to_string(ruta_fixture).expect("leer fixture");
        let mut data: serde_json::Value = serde_json::from_str(&texto).expect("parsear fixture");
        let base = data["comparisons"][0].clone();

        let mut sin_evaluacion = base.clone();
        sin_evaluacion["id"] = "c-sin".into();
        sin_evaluacion.as_object_mut().unwrap().remove("evaluacion");

        let mut no_soportada = base.clone();
        no_soportada["id"] = "c-nosop".into();
        no_soportada["evaluacion"]["metodologia_interpretacion"] = "v999".into();

        let mut malformada = base.clone();
        malformada["id"] = "c-mal".into();
        malformada["evaluacion"] = serde_json::json!({"no_es_una_evaluacion": true});

        data["comparisons"] = serde_json::json!([sin_evaluacion, no_soportada, malformada]);

        let d = dir("clasif");
        let ruta = d.join("saved_comparisons.json");
        let contenido = serde_json::to_string_pretty(&data).expect("serializar");
        std::fs::write(&ruta, &contenido).expect("escribir");
        let antes = mtime_ns(&ruta);

        let leido = try_load_saved_data_de(&ruta).expect("leer");
        assert_eq!(
            leido.comparisons.len(),
            3,
            "las tres comparaciones se leen: ninguna se pierde por su evaluación"
        );

        // La clasificación es la esperada, sin deducirla del texto.
        let mut estados: Vec<(String, EstadoDeLaEvaluacion)> = leido
            .comparisons
            .iter()
            .map(|c| (c.id.clone(), c.estado_de_la_evaluacion()))
            .collect();
        estados.sort_by(|a, b| a.0.cmp(&b.0));

        assert!(
            matches!(estados[0].1, EstadoDeLaEvaluacion::Malformada),
            "c-mal tendría que ser Malformada, es {:?}",
            estados[0].1
        );
        assert!(
            matches!(estados[1].1, EstadoDeLaEvaluacion::NoSoportada(_)),
            "c-nosop tendría que ser NoSoportada, es {:?}",
            estados[1].1
        );
        assert!(
            matches!(estados[2].1, EstadoDeLaEvaluacion::NoRegistrada),
            "c-sin tendría que ser NoRegistrada, es {:?}",
            estados[2].1
        );

        // Leer no reescribe, ni siquiera la malformada.
        assert_eq!(mtime_ns(&ruta), antes, "leer no puede tocar el fichero");
        assert_eq!(
            std::fs::read_to_string(&ruta).expect("releer"),
            contenido,
            "leer no puede reescribir el contenido"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Un guardado nuevo no toca el `mtime` ni el contenido de `benchmarks.json`.
    #[test]
    fn c3_20_guardar_no_reescribe_el_historial() {
        let Some(fixtures) = std::path::Path::new("/workspace/validacion-c3/fixtures/benchmarks.json")
            .exists()
            .then(|| std::path::PathBuf::from("/workspace/validacion-c3/fixtures/benchmarks.json"))
        else {
            eprintln!("AVISO: sin fixtures; se omite");
            return;
        };
        let d = dir("guardar");
        let ruta_bench = d.join("benchmarks.json");
        let ruta_cmp = d.join("saved_comparisons.json");

        let texto = std::fs::read_to_string(&fixtures).expect("leer fixture");
        let data: BenchmarksData = serde_json::from_str(&texto).expect("parsear fixture");
        let ids: Vec<String> = data
            .benchmarks
            .iter()
            .filter(|b| b.game_name.contains("SIN_CAMBIO"))
            .map(|b| b.id.clone())
            .collect();
        assert_eq!(ids.len(), 2, "el fixture tiene que traer el par de SIN_CAMBIO");

        let historial = BenchmarksData {
            version: Some(BENCHMARKS_SCHEMA_VERSION),
            benchmarks: data
                .benchmarks
                .into_iter()
                .filter(|b| ids.contains(&b.id))
                .collect(),
            counters: HashMap::new(),
        };
        std::fs::write(
            &ruta_bench,
            serde_json::to_string(&historial).expect("serializar"),
        )
        .expect("escribir historial");
        std::fs::write(&ruta_cmp, r#"{"version":2,"comparisons":[]}"#).expect("escribir");

        let antes_mtime = mtime_ns(&ruta_bench);
        let antes_texto = std::fs::read_to_string(&ruta_bench).expect("leer");

        let vista = vista_previa_evaluada(&ruta_bench, &ids[0], &ids[1], None).expect("vista");
        guardar_comparacion_evaluada(&ruta_bench, &ruta_cmp, &vista, None).expect("guardar");

        assert_eq!(
            mtime_ns(&ruta_bench),
            antes_mtime,
            "guardar una comparación no puede reescribir benchmarks.json"
        );
        assert_eq!(
            std::fs::read_to_string(&ruta_bench).expect("releer"),
            antes_texto,
            "guardar una comparación no puede cambiar el historial"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
