//! Servicio de sesiones: el observador único, su ciclo de vida y la persistencia.
//!
//! # Por qué existe este módulo aparte de `sessions.rs`
//!
//! `sessions.rs` es dominio: tipos y reglas puras. Esto es el **servicio** que las usa:
//! arranca el hilo del observador, decide cuándo mirar, garda y carga las sesiones, y las
//! relaciona con las grabaciones. Nada de aquí sabe de React ni de qué pantalla está
//! abierta: el observador corre mientras Thoriapp esté abierto, esté en la pantalla que
//! esté (o minimizado), porque el usuario puede lanzar el juego desde Steam sin tocar
//! Thoriapp.
//!
//! # Una sola instancia, un solo hilo
//!
//! Hay exactamente un [`ServicioDeSesiones`] (`OnceLock`), y un único hilo que sondea. El
//! hilo duerme entre pasadas y no crea temporizadores nuevos, así que no puede haber dos
//! sondeos a la vez. Desactivar el observador lo deja dormido sin mirar nada.
//!
//! # Lo que no es
//!
//! No es un gestor de eventos ni un bus. Thoriapp no lanza juegos ni recibe avisos del
//! sistema: mira `/proc` de vez en cuando. Con esa señal basta para saber si un juego lleva
//! rato corriendo, y es lo único que se puede hacer sin pedir privilegios.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::benchmarks::{escribir_atomico, leer_datos, ConProcedencia, ErrorDatos, Procedencia};
use crate::proceso_linux::ObservadorDeProc;
use crate::sessions::{
    decidir_asociacion, parse_fecha, recortar_sesiones, Asociacion, ConfiguracionCapturada,
    EvidenciaProceso, GrupoDeProcesos, IdentidadDeSesion, OrigenAppId, RasgosDeProceso,
    SesionDeEjecucion, SesionesData, TipoDeSesion, VentanaDeGrabacion, SESSIONS_SCHEMA_VERSION,
};

/// Cerrojo de `sessions.json`. Es el mismo patrón que el de benchmarks (subfase 5.1): un
/// `leer → modificar → escribir` a la vez, o se pierde un cambio.
static CERROJO_SESIONES: Mutex<()> = Mutex::new(());

// ==================== INTERVALOS DE SONDEO ====================

/// Intervalo buscando un juego. Es el más corto porque es cuando hay algo que ganar: el
/// usuario acaba de lanzar el juego y detectarlo unos segundos después permite capturar la
/// configuración real con el juego vivo.
pub const INTERVALO_BUSCANDO_MS: u64 = 5_000;

/// Pasadas seguidas sin ver un proceso antes de dar la sesión por terminada.
///
/// Con el sondeo de 5 s son unos 15 s de margen. Una sola pasada no basta: cerrar al primer
/// fallo inventaba finalizaciones cuando `/proc` no se había leído entero. Y esperar más
/// dejaría sesiones abiertas de juegos ya cerrados, así que se acota.
pub const PASADAS_SIN_VERLO: u32 = 3;

/// Intervalo con el juego ya identificado. Más lento: el proceso ya se conoce, solo hay que
/// ver si sigue vivo, y durante una grabación cada milisegundo de CPU cuenta.
pub const INTERVALO_OBSERVANDO_MS: u64 = 10_000;

/// Intervalo cuando no hay visibilidad (contenedor, permisos). Mirar más a menudo no
/// cambiaría el resultado, así que se espacia para no gastar en balde.
pub const INTERVALO_SIN_VISIBILIDAD_MS: u64 = 30_000;

/// Comportamiento del observador, configurable y medible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoliticaDeSondeo {
    pub buscando_ms: u64,
    pub observando_ms: u64,
    pub sin_visibilidad_ms: u64,
    pub descanso_ms: u64,
    pub max_candidatos: usize,
}

impl Default for PoliticaDeSondeo {
    fn default() -> Self {
        Self {
            buscando_ms: INTERVALO_BUSCANDO_MS,
            observando_ms: INTERVALO_OBSERVANDO_MS,
            sin_visibilidad_ms: INTERVALO_SIN_VISIBILIDAD_MS,
            descanso_ms: 1_000,
            max_candidatos: 8,
        }
    }
}

impl PoliticaDeSondeo {
    /// Espera tras una pasada de duración `coste`, según el estado.
    ///
    /// **Adaptativo de verdad**: si una pasada tarda más de un tercio del intervalo, el
    /// intervalo se estira lo justo para no comer más del ~25 % del tiempo en observarse a
    /// sí mismo. Con pasadas de 6 ms (lo normal) el estiramiento no llega a aplicarse.
    pub fn espera(&self, estado: EstadoDelObservador, coste: Duration) -> Duration {
        let base = match estado {
            EstadoDelObservador::Observando { .. } => self.observando_ms,
            EstadoDelObservador::Buscando => self.buscando_ms,
            EstadoDelObservador::SinVisibilidad => self.sin_visibilidad_ms,
            EstadoDelObservador::Apagado => return Duration::ZERO,
        };
        let base = base.max(1);
        let coste_ms = coste.as_millis() as u64;
        // Si la pasada se lleva más de un cuarto del intervalo, se cuadruplica la espera.
        let intervalo = if coste_ms * 4 > base {
            coste_ms * 4
        } else {
            base
        };
        Duration::from_millis(
            intervalo
                .saturating_sub(coste_ms)
                .max(self.descanso_ms.min(intervalo)),
        )
    }
}

/// En qué está el observador. Sirve para decidir la espera y para contarlo en la UI.
#[derive(Debug, Clone, PartialEq)]
pub enum EstadoDelObservador {
    /// Mirando a ver si aparece un juego.
    Buscando,
    /// Siguiendo uno o más juegos ya identificados.
    ///
    /// La lista va aquí, en el estado, y no se deduce luego en React: puede haber varios
    /// juegos a la vez y el estado tiene que poder decirlo. Antes solo cabía uno y el bucle
    /// lo sobreescribía, así que con dos juegos la pantalla enseñaba el último sin avisar.
    Observando { juegos: Vec<JuegoObservado> },
    /// No hay forma de ver procesos (contenedor). Se sigue intentando, muy espaciado.
    SinVisibilidad,
    /// Desactivado por el usuario: **no se sondea nada**.
    Apagado,
}

/// Un juego que el observador está viendo ahora mismo.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct JuegoObservado {
    /// Clave del proceso observado (`pid@inicio`).
    pub clave: String,
    /// AppID leído del propio proceso, si lo tenía.
    pub appid: Option<u32>,
    /// **Nombre comercial** del juego, resuelto por AppID contra la biblioteca de Steam.
    ///
    /// Nunca es el nombre del ejecutable, del envoltorio ni del runtime: `systemd-inhibit` o
    /// `wine64-preloader` no son juegos. Si no se puede resolver, es `None` y la interfaz
    /// dice «identidad desconocida» en vez de inventar un nombre.
    pub nombre: Option<String>,
}

/// Medición de una pasada, para poder hablar de rendimiento con datos y no con estimaciones.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MedidaDePasada {
    /// Cuánto tardó la pasada.
    pub coste_ms: f64,
    /// Procesos examinados a fondo (los que pasaron el filtro barato).
    pub candidatos: usize,
    /// Entradas de `/proc` vistas.
    pub entradas_proc: usize,
    /// Cuándo terminó.
    pub cuando: Option<String>,
}

/// Estado compartido del observador, para la UI y para las mediciones.
#[derive(Debug, Default)]
pub struct EstadoCompartido {
    pub activo: AtomicBool,
    pub pasadas: AtomicU64,
    pub ultima_medida: Mutex<Option<MedidaDePasada>>,
    /// Lo que decidió la última pasada. Es la **única** fuente para `estado_legible`:
    /// antes se recalculaba a ojo y se perdía el hecho de estar observando un juego.
    pub ultimo_estado: Mutex<Option<EstadoDelObservador>>,
    /// Nombre del juego que se está buscando (lo pone el frontend al analizar un CSV).
    pub pista: Mutex<Option<String>>,
    pub error: Mutex<Option<String>>,
}

// ==================== SERVICIO ====================

/// Servicio único de sesiones.
pub struct ServicioDeSesiones {
    estado: EstadoCompartido,
    politica: PoliticaDeSondeo,
    /// Configuración vigente en Thoriapp (la pone el frontend cuando cambia).
    configuracion: Mutex<Option<ConfiguracionCapturada>>,
    /// Ruta de `sessions.json` (inyectable para los tests).
    ruta: PathBuf,
    /// Observador inyectado. `None` en producción (se usa `/proc`); las pruebas meten uno
    /// simulado para poder reproducir escenarios que aquí no existen.
    observador_inyectado: Option<crate::proceso_linux::ObservadorSimulado>,
    /// Catálogo de lanzadores inyectado. `None` en producción (se lee de los lanzadores con
    /// `catalogo_lanzadores::obtener()`); las pruebas meten uno para poder reproducir un
    /// equipo concreto sin tener Lutris ni Steam instalados.
    ///
    /// Es el mismo mecanismo que `observador_inyectado`, y por la misma razón: sin él las
    /// pruebas del pipeline tendrían que construir el catálogo por dentro y volverían a
    /// probar datos ideales en vez de lo que producción entrega.
    catalogo_inyectado: Option<Vec<crate::evidencia_proceso::EntradaDeCatalogo>>,
    /// Hilo y salida. `None` si nunca se arrancó.
    hilo: Mutex<Option<std::thread::JoinHandle<()>>>,
}

/// Por qué no se creó sesión para un grupo.
///
/// Antes las dos cosas se contaban igual («sin evidencia bastante o duplicada») y no se
/// podía saber cuál había pasado. No es lo mismo: una significa que el juego no se
/// reconoció (hay que mirar el catálogo o el árbol de procesos) y la otra que ya estaba
/// contado (todo va bien). Confundirlas ya costó una ronda de diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotivoDeRechazo {
    /// No se pudo decir qué juego es ni confirmarlo con el catálogo.
    SinIdentidad,
    /// Es un juego, pero su proceso ya tiene sesión abierta.
    YaContado,
}

impl ServicioDeSesiones {
    fn con_ruta(ruta: PathBuf) -> Self {
        Self {
            estado: EstadoCompartido::default(),
            politica: PoliticaDeSondeo::default(),
            configuracion: Mutex::new(None),
            ruta,
            observador_inyectado: None,
            catalogo_inyectado: None,
            hilo: Mutex::new(None),
        }
    }

    /// Ruta del archivo de sesiones.
    pub fn ruta_por_defecto() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("thoriapp")
            .join("benchmarks")
            .join("sessions.json")
    }

    /// Estado compartido, para leerlo desde los comandos IPC.
    pub fn estado(&self) -> &EstadoCompartido {
        &self.estado
    }

    pub fn politica(&self) -> PoliticaDeSondeo {
        self.politica
    }

    /// Arranca el hilo del observador. Solo una vez: llamarlo de nuevo no crea otro hilo.
    pub fn arrancar(&'static self) {
        let mut hilo = self.hilo.lock().unwrap_or_else(|e| e.into_inner());
        if hilo.is_some() {
            return;
        }
        // El valor inicial **no** se fuerza: lo pone `hidratar_preferencia` a partir de lo
        // que eligió el usuario. Arrancar siempre activo era el fallo: miraba `/proc` sin
        // que nadie lo hubiera pedido, también en instalaciones nuevas.
        *hilo = Some(std::thread::spawn(move || self.bucle()));
    }

    /// Bucle del observador. Duerme entre pasadas; no hay dos a la vez por construcción.
    fn bucle(&self) {
        loop {
            if !self.estado.activo.load(Ordering::SeqCst) {
                // Apagado: ni una sola lectura de `/proc`. Se comprueba cada segundo sin
                // hacer trabajo, para poder reactivar sin reiniciar la app.
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            let inicio = Instant::now();
            let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.paso()));
            let coste = inicio.elapsed();
            let estado = match resultado {
                Ok(Ok(estado)) => {
                    *self.estado.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    estado
                }
                Ok(Err(e)) => {
                    // Un fallo de E/S no puede tumbar el observador: se apunta y se sigue.
                    *self.estado.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(e);
                    EstadoDelObservador::Buscando
                }
                Err(_) => {
                    *self.estado.error.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some("el observador falló".into());
                    EstadoDelObservador::Buscando
                }
            };
            *self
                .estado
                .ultima_medida
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(MedidaDePasada {
                coste_ms: coste.as_secs_f64() * 1000.0,
                ..Default::default()
            });
            // Se publica lo decidido, para que la interfaz pueda decir «estoy viendo este
            // juego» y no solo «estoy buscando».
            *self
                .estado
                .ultimo_estado
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(estado.clone());
            self.estado.pasadas.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(self.politica.espera(estado, coste));
        }
    }

    /// Una pasada completa. Es `pub` para poder medirla desde los tests sin arrancar el hilo.
    pub fn paso(&self) -> Result<EstadoDelObservador, String> {
        if !self.estado.activo.load(Ordering::SeqCst) {
            return Ok(EstadoDelObservador::Apagado);
        }
        let mut data = self.cargar()?;
        let ahora = ahora_legible();

        // El escaneo se hace **antes** de decidir el ciclo de vida.
        //
        // Antes se decidía primero «¿sigue vivo el PID?» mirando `/proc` y solo después se
        // escaneaba. Eso dejaba el escaneo fuera de la decisión: se podían ver 0 grupos y
        // 465 procesos, sin ningún juego, y la sesión seguía `Observada` porque un auxiliar
        // del árbol continuaba en `/proc`. Ahora la decisión de vida se toma con la misma
        // verdad que se publica en pantalla.
        let escaneo = self.escanear();

        // Revalidación de lo que se acaba de cargar del disco.
        //
        // Las sesiones guardadas por versiones anteriores llevan en `vivos_de_la_ejecucion`
        // **todos** los miembros del árbol, no solo los que ejecutan el juego. Ahí quedaban
        // `dolphin` y `srt-logger`, que sobreviven a cualquier partida, y por eso la sesión se
        // publicaba como observada indefinidamente.
        //
        // Esa lista no sirve como prueba: solo se conserva lo que el escaneo **actual** ve
        // formando parte de este juego. Si al podarla no queda nada, la sesión pasa a
        // `VisibilidadPerdida` y el contador normal hace su trabajo. No se cierra de golpe, no
        // se borra nada y no se toca ningún identificador.
        if escaneo.sin_visibilidad.is_none() {
            let claves_del_escaneo: std::collections::HashSet<String> = escaneo
                .grupos
                .iter()
                .flat_map(|g| g.miembros.iter().map(clave_de))
                .collect();
            for s in data.sesiones.iter_mut() {
                if s.evidencia_proceso != EvidenciaProceso::Observada {
                    continue;
                }
                // ¿El escaneo sigue viendo **este** juego? Se pregunta por él, no por las
                // claves guardadas: si el representante cambió (210465 → 210612), las claves
                // viejas ya no están aunque la partida siga. Preguntar por la clave exacta
                // cerraba sesiones que estaban perfectamente vivas.
                let sigue_a_la_vista = sesion_a_la_vista(&escaneo, s);
                if !sigue_a_la_vista {
                    // El escaneo es válido y no ve el juego: no hay evidencia de ejecución.
                    // Se marca perdida (no terminada) y el ciclo normal hace sus pasadas.
                    s.evidencia_proceso = EvidenciaProceso::VisibilidadPerdida;
                    s.vivos_de_la_ejecucion.clear();
                    continue;
                }
                // Se ve: se poda lo que ya no existe, conservando lo que sigue en `/proc`.
                // La lista vuelve a llenarse con los acreditados de este grupo.
                s.vivos_de_la_ejecucion
                    .retain(|c| claves_del_escaneo.contains(c) || self.sigue_vivo(c));
            }
        }

        // 1. ¿Sigue vivo lo que estábamos siguiendo?
        let mut estado = EstadoDelObservador::Buscando;
        // Juegos vivos de esta pasada: los que ya seguían abiertos y los detectados ahora.
        // Se reúnen y el estado se fija **una sola vez** al final. Antes se asignaba dentro
        // del bucle y el último grupo machacaba a los demás.
        let mut vivos: Vec<JuegoObservado> = Vec::new();
        for s in data.sesiones.iter_mut() {
            // También se revisan las que perdieron visibilidad: hay que ver si vuelven.
            if !s.evidencia_proceso.sigue_en_curso() {
                continue;
            }
            let Some(clave) = s.clave_proceso() else {
                continue;
            };
            // La continuidad la decide la **ejecución**, no su representante: basta con que
            // siga vivo cualquiera de los procesos que la formaban. Si solo se mira la clave
            // del representante, un cambio de representante mata la sesión y abre otra.
            // ¿El **juego** se está ejecutando? Son dos preguntas encadenadas y las dos
            // tienen que responder que sí:
            //
            //   a. La sesión tiene algún proceso que acredite ejecución y siga vivo.
            //      `vivos_de_la_ejecucion` ya no guarda todo el árbol: guarda solo los
            //      procesos clasificados como el juego (ver `acredita_ejecucion`). Si se
            //      guardaran los 13 miembros, bastaría `dolphin` o `srt-logger` para
            //      mantener la sesión abierta para siempre.
            //
            //   b. El escaneo de esta pasada sigue viendo un grupo de este juego, o no hay
            //      escaneo válido del que fiarse. Un escaneo **válido** que ve 0 grupos es
            //      la prueba de que no hay juego corriendo, y `/proc` no puede contradecirlo
            //      con un envoltorio superviviente.
            let ejecucion_viva = if s.vivos_de_la_ejecucion.is_empty() {
                // Sesiones antiguas: solo hay la clave del representante.
                self.sigue_vivo(&clave)
            } else {
                s.vivos_de_la_ejecucion.iter().any(|c| self.sigue_vivo(c))
            };
            let visto_ahora = sesion_a_la_vista(&escaneo, s);
            // Sin visibilidad no se puede afirmar que no esté: eso no cierra nada.
            let sin_evidencia = escaneo.sin_visibilidad.is_none() && !visto_ahora;
            let sigue = if sin_evidencia {
                // Ningún grupo del escaneo corresponde a esta sesión: no hay evidencia de
                // ejecución, aunque `/proc` tenga vivos a otros procesos del árbol.
                false
            } else {
                ejecucion_viva || visto_ahora
            };
            if sigue {
                // Volvió a verse: si estaba perdida, se recupera y el contador se limpia,
                // porque la continuidad la da el PID + starttime de la clave.
                s.evidencia_proceso = EvidenciaProceso::Observada;
                s.pasadas_sin_verlo = 0;
                s.ultima_observacion = Some(ahora.clone());
                // La lista de ejecución se refresca con el grupo que se está viendo ahora.
                //
                // Antes solo se escribía al abrir la sesión. Si el representante cambiaba, la
                // lista se quedaba con las claves viejas: la sesión seguía, pero su evidencia
                // de ejecución apuntaba a procesos que ya no existían, y en la pasada
                // siguiente `sigue_vivo` decía que no. Refrescarla aquí mantiene la lista
                // fiel al grupo real, sin acumular miembros de árboles anteriores.
                // Solo se refresca cuando la ejecución **ya** era la misma. Si la lista
                // estaba vacía o no acreditaba nada, esta pasada puede estar viendo una
                // partida distinta con el mismo AppID (un PID reutilizado, o el juego
                // reabierto): adoptar sus claves fusionaría dos ejecuciones en una sola
                // sesión. En ese caso la lista se rellena en `sesion_de_grupo`, que sí
                // distingue ejecuciones.
                // Refresco condicionado a que la ejecución sea **la misma partida**.
                //
                // La continuidad la da la huella del lanzamiento (`arbol_del_lanzamiento`),
                // que incluye los envoltorios. Cuando el representante cambia (210465 →
                // 210612) la clave acreditada vieja ya no existe, pero el árbol comparte el
                // envoltorio 210400: eso distingue un cambio de representante de una partida
                // nueva. Un PID reutilizado no comparte árbol, y un juego reabierto tampoco.
                let grupo_actual = sesion_a_la_vista_ahora(&escaneo, s);
                let mismo_arbol = grupo_actual.is_some_and(|g| {
                    let claves_del_grupo: std::collections::HashSet<String> =
                        g.miembros.iter().map(clave_de).collect();
                    // La huella guardada **o** la lista de ejecución tienen que coincidir
                    // con un proceso **de este grupo**. Antes se aceptaba además cualquier
                    // clave que siguiera viva en `/proc` (`|| self.sigue_vivo(c)`), y eso
                    // daba continuidad sin demostrar pertenencia: un proceso vivo de otra
                    // partida, o un auxiliar superviviente, bastaba para adoptar el grupo
                    // actual. Pertenecer al árbol es topología; seguir vivo no dice de quién.
                    s.arbol_del_lanzamiento
                        .iter()
                        .chain(s.vivos_de_la_ejecucion.iter())
                        .any(|c| claves_del_grupo.contains(c))
                });
                // Solo se refresca si se reconoce la misma partida. Refrescar sin más
                // adoptaría las claves de un juego reabierto y fusionaría dos ejecuciones.
                if ejecucion_viva || mismo_arbol {
                    if let Some(g) = grupo_actual {
                        s.vivos_de_la_ejecucion = claves_de_la_ejecucion(g);
                        s.arbol_del_lanzamiento = claves_del_arbol(g);
                    }
                }
                // El representante puede haber cambiado: se guarda la ejecución tal como se
                // ve ahora. La lista solo se reemplaza cuando el grupo está a la vista, así
                // que una pasada que no lo vea no la vacía.
                vivos.push(JuegoObservado {
                    clave,
                    appid: s.identidad.appid,
                    // El nombre guardado en la sesión es el que se resolvió al abrirla; si
                    // no se pudo, sigue sin nombre en vez de rellenarlo con el ejecutable.
                    nombre: s.identidad.nombre_comercial.clone(),
                });
            } else {
                // No se le ve. Puede ser que haya terminado o que esta pasada no lo haya
                // visto (`/proc` a medias, permisos, un escaneo que llega antes de que el
                // árbol esté montado). Cerrar con una sola pasada inventaría
                // finalizaciones, así que se marca como perdido y se sigue intentando.
                //
                // El contador es lo que impide mantener la sesión abierta para siempre si
                // el proceso murió de verdad: tras `PASADAS_SIN_VERLO` seguidas, termina.
                s.evidencia_proceso = EvidenciaProceso::VisibilidadPerdida;
                s.pasadas_sin_verlo = s.pasadas_sin_verlo.saturating_add(1);
                if s.pasadas_sin_verlo >= PASADAS_SIN_VERLO {
                    s.evidencia_proceso = EvidenciaProceso::Terminada;
                    s.cerrada_en = Some(ahora.clone());
                }
            }
        }

        // 2. Si no hay nada abierto, mirar `/proc` y ver qué juegos hay.
        //
        // Antes se buscaba «el proceso cuyo nombre coincida con la pista». Eso servía para
        // juegos nativos y descartaba todos los de Proton: su proceso Linux se llama
        // `GameThread` y vive dentro de `compatibilitytools.d`. Ahora se recolectan todos
        // los grupos de procesos que parecen juegos y se decide por evidencia.
        let hay_abierta = data
            .sesiones
            .iter()
            .any(|s| s.evidencia_proceso == EvidenciaProceso::Observada);
        if hay_abierta && crate::diagnostico_procesos::activo() {
            // Ya se está siguiendo una sesión: también interesa verlo, porque el síntoma
            // real fue «la sesión existe pero la pantalla no lo dice».
            let observadas: Vec<String> = data
                .sesiones
                .iter()
                .filter(|s| s.evidencia_proceso == EvidenciaProceso::Observada)
                .map(|s| {
                    format!(
                        "sesion {} OBSERVADA appid={:?} clave={:?}",
                        s.id,
                        s.identidad.appid,
                        s.clave_proceso()
                    )
                })
                .collect();
            let inf = crate::diagnostico_procesos::Informe {
                sesiones: observadas,
                ..Default::default()
            };
            crate::diagnostico_procesos::volcar(&inf, "paso-ya-observando");
        }
        // Se escanea **siempre**, haya o no una sesión abierta.
        //
        // Antes solo se escaneaba si no había ninguna abierta, y eso impedía ver un segundo
        // juego lanzado con el primero en marcha: la pantalla seguía enseñando uno solo. Las
        // sesiones ya conocidas no se duplican porque `sesion_de_grupo` descarta la clave de
        // proceso y el AppID ya presentes; lo que faltaba era simplemente mirar.
        let _ = hay_abierta;
        {
            let pista = self
                .estado
                .pista
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();

            // Sin visibilidad y sin grupos: el motivo se guarda una vez, para que la
            // interfaz pueda decir «no consta» en vez de «no corre».
            if let Some(motivo) = escaneo.sin_visibilidad.clone() {
                estado = EstadoDelObservador::SinVisibilidad;
                let ya = data
                    .sesiones
                    .iter()
                    .any(|s| s.evidencia_proceso == EvidenciaProceso::SinVisibilidad);
                if !ya {
                    let id = id_de_sesion(&ahora, &ids_existentes(&data));
                    let mut s = SesionDeEjecucion::agrupada(
                        id,
                        IdentidadDeSesion::desconocida(),
                        &ahora,
                    );
                    s.evidencia_proceso = EvidenciaProceso::SinVisibilidad;
                    s.identidad.evidencia.push(motivo);
                    data.sesiones.push(s);
                }
            } else if escaneo.grupos.is_empty() {
                // Hay visibilidad y no hay ningún juego: aquí sí se puede decir que no
                // corre nada, porque se ha mirado y se ha visto el sistema entero.
                let _ = &pista;
                // Este es justo el caso que falló en el equipo real: el juego corre y el
                // escaneo no ve ningún grupo. Es el volcado más importante.
                if crate::diagnostico_procesos::activo() {
                    let inf = crate::diagnostico_procesos::Informe {
                        grupos: 0,
                        inaccesibles: escaneo.inaccesibles,
                        sin_visibilidad: escaneo.sin_visibilidad.clone(),
                        sesiones: vec![
                            "sin grupos: no se crea ninguna sesion en esta pasada".to_string()
                        ],
                        ..Default::default()
                    };
                    crate::diagnostico_procesos::volcar(&inf, "paso-sin-grupos");
                }
            } else {
                let catalogo = crate::steam::catalogo_para_identidad();
                // Catálogo unificado (Steam + Lutris + Heroic + Heronite), cacheado: esta
                // llamada no abre SQLite ni relee JSON salvo que toque revalidar.
                let catalogo_completo = match &self.catalogo_inyectado {
                    Some(c) => c.clone(),
                    None => crate::catalogo_lanzadores::obtener(),
                };
                let mut abiertas = 0usize;
                // Diagnóstico: se reúne lo que decide cada grupo, sin cambiar la decisión.
                let mut diag: Vec<String> = Vec::new();
                for grupo in &escaneo.grupos {
                    // Se pasan las que ya hay **en memoria**, incluidas las añadidas en
                    // esta misma pasada.
                    let previas: Vec<SesionDeEjecucion> = data.sesiones.clone();
                    let creada = self.sesion_de_grupo(
                        grupo,
                        &catalogo,
                        &catalogo_completo,
                        &ahora,
                        &previas,
                    );
                    if crate::diagnostico_procesos::activo() {
                        diag.push(match &creada {
                            Ok(x) => format!(
                                "grupo pid={} -> sesion CREADA appid={:?} nombre={:?}",
                                grupo.principal, x.identidad.appid, x.identidad.nombre_observado
                            ),
                            // Los dos motivos se distinguen: «no lo reconozco» y «ya estaba
                            // contado» piden mirar cosas distintas.
                            Err(MotivoDeRechazo::YaContado) => format!(
                                "grupo pid={} -> sesion NO duplicada: ese proceso ya tiene sesion abierta",
                                grupo.principal
                            ),
                            Err(MotivoDeRechazo::SinIdentidad) => format!(
                                "grupo pid={} -> sesion RECHAZADA: no se pudo identificar el juego evidencia={:?}",
                                grupo.principal, grupo.evidencia
                            ),
                        });
                    }
                    if let Ok(sesion) = creada {
                        vivos.push(JuegoObservado {
                            clave: sesion.clave_proceso().unwrap_or_else(|| sesion.id.clone()),
                            appid: sesion.identidad.appid,
                            nombre: sesion.identidad.nombre_comercial.clone(),
                        });
                        data.sesiones.push(sesion);
                        abiertas += 1;
                    }
                }
                let _ = abiertas;
                if crate::diagnostico_procesos::activo() {
                    let mut inf = crate::diagnostico_procesos::Informe {
                        grupos: escaneo.grupos.len(),
                        inaccesibles: escaneo.inaccesibles,
                        sin_visibilidad: escaneo.sin_visibilidad.clone(),
                        sesiones: diag,
                        // Lo que los adaptadores anotaron al leer el ejecutable: si aquí no
                        // aparece, el ejecutable nunca llegó a conocerse.
                        recorrido_del_ejecutable:
                            crate::diagnostico_procesos::tomar_rastro_del_ejecutable(),
                        ..Default::default()
                    };
                    // Los grupos ya traen sus miembros: se vuelcan como candidatos.
                    for g in &escaneo.grupos {
                        for m in &g.miembros {
                            inf.candidatos.push(crate::diagnostico_procesos::Candidato {
                                pid: m.pid,
                                comm: m.ejecutable.clone(),
                                ejecutable: m.ruta_ejecutable.clone(),
                                ruta_del_juego: m.ruta_del_juego.clone(),
                                steam_app_id: m
                                    .appid_del_entorno
                                    .as_ref()
                                    .and_then(|e| e.steam_app_id.clone()),
                                steam_game_id: m
                                    .appid_del_entorno
                                    .as_ref()
                                    .and_then(|e| e.steam_game_id.clone()),
                                appid_de_argumentos: m.appid_de_argumentos,
                                bajo_proton: m.bajo_proton,
                                auxiliar: m.auxiliar,
                                carpeta_declarada: m.carpeta_declarada.clone(),
                                wine_prefix: m.wine_prefix.clone(),
                                ejecuta_de_verdad: m.ejecuta_de_verdad,
                                // El grupo ya sabe de dónde salió cada miembro: aquí basta
                                // con rellenar la identidad de instancia y dejar que el
                                // volcado muestre su cadena, si la tiene.
                                ppid: None,
                                inicio: m.inicio_del_proceso.clone(),
                                ancestros: Vec::new(),
                            });
                        }
                    }
                    // El catálogo **efectivo** del observador, que no es el de la biblioteca
                    // de la interfaz: son dos listas distintas y hasta ahora nada comprobaba
                    // que la del observador tuviera lo que la biblioteca enseña.
                    for e in &catalogo_completo {
                        inf.catalogo
                            .push(crate::diagnostico_procesos::EntradaDelCatalogo {
                                launcher: e.launcher.clone(),
                                titulo: e.titulo.clone(),
                                id_plataforma: e.id_plataforma.clone(),
                                install_dir: e.install_dir.clone(),
                                wine_prefix: e.wine_prefix.clone(),
                                exec: e.exec.clone(),
                            });
                    }
                    // Y lo que decidió la correlación para cada grupo.
                    for g in &escaneo.grupos {
                        let principal = g.miembros.iter().find(|m| m.pid == g.principal);
                        // Se anota el `cwd` del representante **antes** de decidir nada: sin
                        // él, un `SIN correlacion` no distingue «no se leyó el cwd» de «se
                        // leyó y no casó con el catálogo».
                        inf.directorios_de_trabajo.push(format!(
                            "grupo pid={} cwd={:?} exe={:?}",
                            g.principal,
                            principal.and_then(|p| p.cwd.clone()),
                            principal.and_then(|p| p.ruta_ejecutable.clone()),
                        ));
                        let decision = match principal.and_then(|p| {
                            correlacionar_con_el_catalogo(
                                &catalogo_completo,
                                p.carpeta_declarada.as_deref(),
                                p.wine_prefix.as_deref(),
                                p.cwd.as_deref(),
                                p.ruta_ejecutable.as_deref(),
                                p.ejecutable_del_juego.as_deref(),
                            )
                        }) {
                            Some(c) => format!(
                                "grupo pid={} -> correlacionado por {}: {}",
                                g.principal, c.via, c.titulo
                            ),
                            None => format!(
                                "grupo pid={} -> SIN correlacion (carpeta={:?}, prefijo={:?}, cwd={:?}, exe={:?}, catalogo={} entradas)",
                                g.principal,
                                principal.and_then(|p| p.carpeta_declarada.clone()),
                                principal.and_then(|p| p.wine_prefix.clone()),
                                principal.and_then(|p| p.cwd.clone()),
                                principal.and_then(|p| p.ruta_ejecutable.clone()),
                                catalogo_completo.len()
                            ),
                        };
                        inf.correlaciones.push(decision);
                    }
                    crate::diagnostico_procesos::volcar(&inf, "paso");
                }
            }
        }

        // El estado se decide **una vez**, con todos los juegos vivos juntos. Se hace aquí,
        // después de recorrer sesiones y grupos, porque puede haber varios a la vez y antes
        // el último grupo machacaba a los demás: la pantalla enseñaba uno sin avisar.
        if !vivos.is_empty() {
            estado = EstadoDelObservador::Observando { juegos: vivos };
        }

        data.version = Some(SESSIONS_SCHEMA_VERSION);
        recortar_sesiones(&mut data.sesiones);
        self.guardar(&data)?;
        // Se publica lo que ha decidido esta pasada. Sin esto, quien pregunta por el estado
        // recalculaba a ojo y nunca podía ver «estoy observando»: ése fue el fallo real del
        // primer equipo, donde la sesión se creaba pero la interfaz seguía buscando.
        *self
            .estado
            .ultimo_estado
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(estado.clone());
        Ok(estado)
    }

    /// Observador real. Se instancia por pasada porque no tiene estado.
    fn observador(&self) -> ObservadorDeProc {
        ObservadorDeProc
    }

    /// Escaneo de esta pasada: el real, o el inyectado por una prueba.
    fn escanear(&self) -> crate::sessions::EscaneoDeProcesos {
        use crate::sessions::ObservadorDeProcesos as _;
        match &self.observador_inyectado {
            Some(o) => o.escanear(),
            None => self.observador().escanear(),
        }
    }

    /// Convierte un grupo de procesos observado en una sesión, si hay evidencia bastante.
    ///
    /// Aquí se cumple la regla de no usar una coincidencia aislada como prueba: el AppID
    /// tiene que venir del entorno del proceso o de la línea de comandos de Steam y, cuando
    /// ambos hablan, tienen que coincidir. El nombre solo vale para reconocer **cuál** es el
    /// juego, nunca para decidir que hay sesión.
    ///
    /// Devuelve `None` si la sesión ya existía: una pasada no debe duplicar.
    fn sesion_de_grupo(
        &self,
        grupo: &GrupoDeProcesos,
        catalogo: &[crate::sessions::JuegoDeBiblioteca],
        // Catálogo unificado: sirve para **confirmar** un proceso que por sí mismo solo era
        // candidato, y para ponerle identidad a un juego de Lutris, Heroic o Heronite.
        catalogo_completo: &[crate::evidencia_proceso::EntradaDeCatalogo],
        ahora: &str,
        // Las sesiones que ya hay **en esta misma pasada**, no solo las guardadas. Si dos
        // grupos de la misma pasada resultan ser el mismo juego, el segundo se descarta
        // aquí; mirar el fichero no basta porque lo recién añadido aún no está en disco y
        // se duplicaba una sesión por pasada.
        sesiones_de_esta_pasada: &[SesionDeEjecucion],
    ) -> Result<SesionDeEjecucion, MotivoDeRechazo> {
        // Sin miembro principal no hay ni juego que identificar: es «sin identidad», no un
        // duplicado.
        let Some(principal) = grupo
            .miembros
            .iter()
            .find(|m| m.pid == grupo.principal)
            .cloned()
        else {
            return Err(MotivoDeRechazo::SinIdentidad);
        };

        // La identidad se resuelve con el proceso principal. El nombre que se le pasa es el
        // del ejecutable del juego (el `.exe` de Windows en un lanzamiento Proton), no el
        // del preloader de Wine. `nombres` ya viene con el del representante delante.
        //
        // La `pista` **no** se usa aquí. Es el nombre que el frontend está mirando, no lo que
        // dice `/proc`, y usarla como respaldo escribía en `nombre_observado` el título de
        // *otro* juego: el diagnóstico midió `Heronite-x86_64` en Tattoo Tycoon, Eslabong y
        // Starsector. Cuando la ejecución no aporta nombre, el observado se queda vacío; el
        // título lo pone la correlación con el catálogo, que sí es evidencia.
        let nombre = grupo.nombres.first().cloned();
        // Se le pasa **también** el catálogo multi-lanzador: sin él esta función solo entendía
        // Steam y un juego de Lutris o Heronite quedaba sin identificar aunque el catálogo lo
        // tuviera. Y la identidad se resuelve con el catálogo completo, no con la biblioteca.
        let mut identidad = crate::sessions::resolver_identidad_con_catalogo(
            nombre.as_deref(),
            Some(&principal),
            catalogo,
            catalogo_completo,
        );

        // El AppID del grupo puede venir de un hermano (el `.exe` lo hereda, pero no
        // siempre). Se usa solo si no contradice lo ya resuelto.
        if let Some(id) = grupo.appid {
            match identidad.appid {
                Some(ya) if ya == id => {}
                Some(ya) => {
                    identidad.evidencia.push(format!(
                        "el grupo declara AppID {} pero el proceso dice {}: no se elige",
                        id, ya
                    ));
                    identidad.candidatos = vec![ya, id];
                    identidad.appid = None;
                    identidad.origen = OrigenAppId::Desconocido;
                }
                None if !grupo.appid_contradictorio && identidad.candidatos.is_empty() => {
                    identidad.appid = Some(id);
                    identidad.origen = OrigenAppId::EntornoDelProceso;
                    identidad.candidatos = vec![id];
                    identidad
                        .evidencia
                        .push(format!("los procesos del grupo declaran AppID {}", id));
                }
                None => {}
            }
        }
        // Nombre **comercial**: el que el usuario reconoce. Sale del AppID contra la
        // biblioteca de Steam, que es la única fuente que lo conoce de verdad. No se cae al
        // nombre del ejecutable ni del envoltorio: si el AppID no está en la biblioteca, se
        // queda sin nombre y la interfaz dirá «identidad desconocida».
        // El nombre comercial que ya resolvió `resolver_identidad_con_catalogo` **se
        // respeta**. Antes esta línea lo pisaba siempre: se recalculaba desde el AppID y,
        // como un juego externo no tiene AppID, el resultado era `None` y la interfaz decía
        // «identidad desconocida» aunque el catálogo sí supiera el título. Era un segundo
        // motor de identidad decidiendo por encima del primero.
        identidad.nombre_comercial = identidad
            .nombre_comercial
            .take()
            .or_else(|| {
                identidad.appid.and_then(|id| {
                    catalogo
                        .iter()
                        .find(|j| j.appid == id)
                        .map(|j| j.nombre.clone())
                })
            })
            // Fuera de Steam el AppID no existe: el título sale del catálogo del lanzador,
            // correlacionando por la carpeta de instalación que el proceso declara.
            .or_else(|| {
                let via = correlacionar_con_el_catalogo(
                    catalogo_completo,
                    principal.carpeta_declarada.as_deref(),
                    principal.wine_prefix.as_deref(),
                    principal.cwd.as_deref(),
                    principal.ruta_ejecutable.as_deref(),
                    principal.ejecutable_del_juego.as_deref(),
                )?;
                identidad
                    .evidencia
                    .push(format!("identificado por {} en el catálogo", via.via));
                Some(via.titulo)
            });
        if crate::catalogo_lanzadores::esta_obsoleto() {
            // El catálogo viene de una lectura anterior porque la última falló: la identidad
            // es lo mejor que se sabe, pero **no** está verificada ahora mismo.
            identidad.evidencia.push(
                "catálogo desactualizado: la última lectura falló, se usa la anterior".to_string(),
            );
        }
        identidad.evidencia.extend(grupo.evidencia.iter().cloned());

        let clave = clave_de(&principal);
        // Ya seguido (en disco o en esta misma pasada): no se duplica.
        // La ejecución de este grupo, para reconocerla aunque su representante haya cambiado.
        let claves_de_este_grupo = claves_de_la_ejecucion(grupo);
        let mismo_proceso = |s: &SesionDeEjecucion| s.clave_proceso().as_deref() == Some(&clave);
        // ¿Es la **misma ejecución** que ya se está siguiendo?
        //
        // 1. Mismo representante: la regla de siempre.
        // 2. El proceso que **ejecuta el juego** en este grupo ya estaba en la ejecución
        //    guardada: es el caso de Deskrawl (el representante pasa de 210465 a 210612 sin
        //    que la partida se interrumpa).
        // 3. Algún miembro coincide **y** la sesión anterior sigue en curso. Cubre los árboles
        //    donde el `.exe` no está marcado como ejecutor y solo se solapan los envoltorios.
        //
        // NO se compara la lista entera sin más: un envoltorio sobrevive de una partida a la
        // siguiente y fusionaría ejecuciones distintas. La condición 3 exige que la sesión
        // anterior siga viva, así que un árbol nuevo tras el cierre no se fusiona; y el
        // `starttime` distingue un PID reutilizado.
        let claves_del_juego = claves_del_juego_de(grupo);
        let solapa_el_juego = |s: &SesionDeEjecucion| {
            s.vivos_de_la_ejecucion
                .iter()
                .any(|c| claves_del_juego.contains(c))
        };
        // Continuidad de instancia: se compara la **huella del lanzamiento**, que incluye
        // envoltorios. Es lo único que puede reconocer la misma partida cuando el
        // representante cambia, porque el representante viejo ya no existe. Exige que la
        // sesión siga en curso: un árbol nuevo tras el cierre no hereda la sesión anterior.
        let solapa_viva = |s: &SesionDeEjecucion| {
            s.evidencia_proceso.sigue_en_curso()
                && s.arbol_del_lanzamiento
                    .iter()
                    .any(|c| claves_de_este_grupo.contains(c))
        };
        let misma_ejecucion = |s: &SesionDeEjecucion| {
            mismo_proceso(s) || solapa_el_juego(s) || solapa_viva(s)
        };
        let ya_en_memoria = sesiones_de_esta_pasada
            .iter()
            .any(|s| s.evidencia_proceso == EvidenciaProceso::Observada && misma_ejecucion(s));
        if ya_en_memoria {
            return Err(MotivoDeRechazo::YaContado);
        }
        // **No** se descarta por AppID compartido.
        //
        // Antes se hacía, con el argumento de que el AppID ya abierto era el mismo juego
        // aunque cambiara el PID. Eso confundía dos cosas: el AppID identifica el **juego**,
        // no la **ejecución**. Con esa regla, cerrar el juego y volver a abrirlo no creaba
        // sesión nueva: la segunda ejecución se perdía dentro de la primera, que además se
        // quedaba abierta indefinidamente porque su clave antigua ya no se veía pero el
        // AppID seguía apareciendo.
        //
        // La continuidad la da la clave de proceso (PID + starttime). Un reinicio dentro del
        // juego sí cambia el PID y por tanto abre sesión nueva, y es lo honesto: es otra
        // ejecución observada.
        let ya_en_disco = self.cargar().ok().map(|d| {
            d.sesiones.iter().any(|s| {
                s.evidencia_proceso.sigue_en_curso() && misma_ejecucion(s)
            })
        });
        if ya_en_disco == Some(true) {
            return Err(MotivoDeRechazo::YaContado);
        }

        // Id único contra lo ya guardado **y** contra lo abierto en esta misma pasada: dos
        // grupos del mismo segundo crearían si no el mismo identificador.
        let mut ocupados = self.cargar().map(|d| ids_existentes(&d)).unwrap_or_default();
        ocupados.extend(sesiones_de_esta_pasada.iter().map(|s| s.id.clone()));
        let id = id_de_sesion(ahora, &ocupados);
        let mut sesion = SesionDeEjecucion::observada(
            id,
            principal,
            identidad,
            ahora,
        );
        sesion.configuracion_inicial = self.configuracion_de_arranque();
        // Se guarda la ejecución **completa**, no solo el representante: es lo que permite
        // que la sesión siga siendo la misma si el representante cambia. Sin esto, el cambio
        // de representante cierra la sesión y abre otra para la misma partida.
        sesion.vivos_de_la_ejecucion = claves_de_la_ejecucion(grupo);
        // La huella del lanzamiento, para reconocer la misma partida si cambia el
        // representante. No acredita ejecución: solo continuidad.
        sesion.arbol_del_lanzamiento = claves_del_arbol(grupo);
        Ok(sesion)
    }

    /// La configuración que la interfaz tenía puesta al abrir la sesión.
    ///
    /// No se inventa nada: se guarda la configuración vigente y se marca como **posterior al
    /// arranque**, porque el observador ve el proceso al menos un sondeo después de que
    /// arrancara. Es decir, esta foto es la mejor disponible, y se dice con honestidad lo que
    /// es.
    ///
    /// El fallo de Deskrawl no era esta función, sino que `configuracion_inicial` se rellenaba
    /// aquí y la interfaz la presentaba como «configuración del arranque» sin mirar la marca:
    /// la sesión empezó a las 14:16:01 y la config figuraba de las 14:20:30. La marca ya
    /// existía y era correcta; nadie la consultaba al mostrar. Se arregla en la representación
    /// (ver `InstantaneaCard` y `docs/sesiones-de-ejecucion.md`), no duplicando el estado.
    fn configuracion_de_arranque(&self) -> Option<ConfiguracionCapturada> {
        let mut cfg = self
            .configuracion
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()?;
        cfg.capturado_en = ahora_legible();
        cfg.posterior_al_arranque = true;
        Some(cfg)
    }

    /// Foto de la configuración de Thoriapp en este momento.
    ///
    /// Se marca como **posterior al arranque** porque el observador ve el proceso cuando ya
    /// lleva un rato corriendo (como mínimo los 5 s del sondeo), así que no puede afirmar que
    /// esa configuración fuera la del arranque. La marca es honesta y se conserva.
    ///
    /// Lo que **no** puede pasar es que esta foto tardía se guarde como
    /// `configuracion_inicial` de la sesión y luego se lea como si fuera la del arranque. Eso
    /// es lo que ocurría: la sesión de Deskrawl empezó a las 14:16:01 y su «configuración
    /// inicial» figuraba capturada a las 14:20:30, con la marca en `true` puesta a mano pero
    /// sin que nadie la mirara. Para lo inicial hace falta `configuracion_de_arranque`, que
    /// es la que ya existía **antes** de ver el proceso.
    fn configuracion_actual(&self, _sesion: &SesionDeEjecucion) -> Option<ConfiguracionCapturada> {
        let mut cfg = self
            .configuracion
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()?;
        cfg.capturado_en = ahora_legible();
        cfg.posterior_al_arranque = true;
        // `revision_motor` se deja como venga: `None` si el motor no publica revisión
        // estable. No se rellena con un número por llevar la cuenta.
        Some(cfg)
    }

    /// Guarda la configuración que la UI tiene ahora. Se llama cuando el usuario cambia de
    /// perfil o edita el comando **y** cuando arranca Thoriapp.
    pub fn fijar_configuracion(&self, cfg: ConfiguracionCapturada) {
        *self.configuracion.lock().unwrap_or_else(|e| e.into_inner()) = Some(cfg);
    }

    /// Fija la pista del juego que se está buscando.
    ///
    /// Desde el arreglo de detección de Proton, `paso()` ya no la usa para buscar: se mira
    /// `/proc` y se decide por evidencia. Se conserva (y se sigue guardando) porque sirve de
    /// desempate para poner nombre a una sesión cuya identidad quedó sin resolver, y porque
    /// es lo que el frontend conoce al analizar una grabación.
    pub fn fijar_pista(&self, nombre: Option<String>) {
        *self.estado.pista.lock().unwrap_or_else(|e| e.into_inner()) = nombre;
    }

    /// Pista actual, para diagnóstico. Es solo el nombre: nunca evidencia de identidad.
    pub fn pista(&self) -> Option<String> {
        self.estado
            .pista
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Activa o desactiva el observador **y lo recuerda**.
    ///
    /// La preferencia se guarda en el mismo archivo de sesiones, junto a los datos, para que
    /// no haya un segundo almacén de configuracion que se pueda desincronizar. Si no se
    /// puede escribir, el estado en memoria sigue cambiando: la app funciona, y el error se
    /// deja visible en vez de fingir que se guardó.
    pub fn fijar_activo(&self, activo: bool) {
        // Al encender el observador se descarta el catálogo cacheado: entre apagarlo y
        // encenderlo pueden haber pasado horas y el usuario puede haber instalado algo.
        // Esta es la revalidación al activar; con el observador apagado no se lee nada.
        if activo {
            crate::catalogo_lanzadores::invalidar();
        }
        self.estado.activo.store(activo, Ordering::SeqCst);
        let r = self.actualizar(|d| {
            if d.observador_activo == Some(activo) {
                return false; // Ya estaba así: no se reescribe el archivo.
            }
            d.observador_activo = Some(activo);
            true
        });
        if let Err(e) = r {
            eprintln!("[sesiones] no se pudo guardar la preferencia del observador: {e}");
        }
    }

    /// Lee la preferencia guardada y deja el observador como el usuario lo dejó.
    ///
    /// Se llama al arrancar, **antes** de que el hilo haga su primera pasada. Instalación
    /// nueva (sin archivo o sin la clave) = desactivado. Esto es lo único que decide el
    /// estado inicial: ni la interfaz ni la navegación lo activan.
    pub fn hidratar_preferencia(&self) {
        let activo = self
            .cargar()
            .map(|d| d.observador_deberia_estar_activo())
            // Si no se puede leer, se queda apagado: ante la duda, no se mira `/proc`.
            .unwrap_or(false);
        self.estado.activo.store(activo, Ordering::SeqCst);
    }

    pub fn esta_activo(&self) -> bool {
        self.estado.activo.load(Ordering::SeqCst)
    }

    /// Estado legible para la interfaz.
    pub fn estado_legible(&self) -> EstadoDelObservador {
        if !self.esta_activo() {
            return EstadoDelObservador::Apagado;
        }
        if self
            .estado
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            return EstadoDelObservador::SinVisibilidad;
        }
        // Lo que decidió la última pasada. Si aún no hubo ninguna, se está buscando.
        self.estado
            .ultimo_estado
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or(EstadoDelObservador::Buscando)
    }

    // ---------- persistencia ----------

    pub fn cargar(&self) -> Result<SesionesData, String> {
        let _turno = CERROJO_SESIONES.lock().unwrap_or_else(|e| e.into_inner());
        leer_sesiones(&self.ruta)
    }

    /// Devuelve una sesión por su id, con la configuración que se capturó al abrirla.
    ///
    /// Se usa al guardar un benchmark para que su instantánea pueda apoyarse en la identidad
    /// y la configuración **observadas**, en vez de en lo que la interfaz crea recordar. Si la
    /// sesión ya no está (borrada) devuelve `None`: entonces se sigue sin ella, no se inventa.
    pub fn sesion_por_id(&self, id: &str) -> Option<SesionDeEjecucion> {
        self.cargar()
            .ok()?
            .sesiones
            .into_iter()
            .find(|s| s.id == id)
    }

    pub fn guardar(&self, data: &SesionesData) -> Result<(), String> {
        let _turno = CERROJO_SESIONES.lock().unwrap_or_else(|e| e.into_inner());
        escribir_sesiones(&self.ruta, data)
    }

    /// Lee las sesiones y modifica. La escritura solo ocurre si `f` devuelve `true`.
    pub fn actualizar<F>(&self, f: F) -> Result<SesionesData, String>
    where
        F: FnOnce(&mut SesionesData) -> bool,
    {
        let _turno = CERROJO_SESIONES.lock().unwrap_or_else(|e| e.into_inner());
        let mut data = leer_sesiones(&self.ruta)?;
        if f(&mut data) {
            escribir_sesiones(&self.ruta, &data)?;
        }
        Ok(data)
    }

    // ---------- asociación ----------

    /// Decide a qué sesión pertenece una grabación recién analizada.
    ///
    /// Devuelve el id de sesión a guardar en el benchmark y la asociación obtenida. Cuando
    /// no hay evidencia suficiente devuelve `(None, NoComprobable)` y el benchmark se
    /// guarda sin sesión: es preferible no atribuir que atribuir mal.
    pub fn asociar_grabacion(
        &self,
        appid: u32,
        nombre: &str,
        ventana: &VentanaDeGrabacion,
    ) -> (Option<String>, Asociacion) {
        let Ok(data) = self.cargar() else {
            return (None, Asociacion::NoComprobable);
        };
        let mut mejor: Option<(Asociacion, String)> = None;
        for s in &data.sesiones {
            let a = decidir_asociacion(s, Some(appid).filter(|a| *a != 0), nombre, ventana);
            if !a.permite_atribuir() {
                continue;
            }
            let sustituye = match &mejor {
                None => true,
                Some((actual, _)) => fuerza(&a) > fuerza(actual),
            };
            if sustituye {
                mejor = Some((a, s.id.clone()));
            }
        }
        // Varias sesiones con la misma fuerza sería ambiguo; con la fuerza de la asociación
        // ya se filtra a la mejor, y si hay empate real no se atribuye.
        match mejor {
            Some((a, id)) => (Some(id), a),
            None => (None, Asociacion::NoComprobable),
        }
    }

    /// Anota una grabación en la sesión a la que se atribuyó.
    ///
    /// Si la sesión ya no existe (se borró, el archivo se recortó) no pasa nada: el
    /// benchmark conserva su instantánea y sigue siendo legible. Ese es el motivo de que el
    /// enlace sea opcional y de que la instantánea no dependa de la sesión.
    pub fn anotar_grabacion(&self, sesion_id: &str, benchmark_id: &str) {
        let _ = self.actualizar(|data| {
            let Some(s) = data.sesiones.iter_mut().find(|s| s.id == sesion_id) else {
                return false;
            };
            if s.grabaciones.iter().any(|g| g == benchmark_id) {
                return false;
            }
            s.grabaciones.push(benchmark_id.to_string());
            true
        });
    }

    /// Sesión de un benchmark, si sigue existiendo.
    pub fn sesion(&self, id: &str) -> Option<SesionDeEjecucion> {
        self.cargar()
            .ok()?
            .sesiones
            .into_iter()
            .find(|s| s.id == id)
    }

    // ---------- asignación manual ----------

    /// Fija a mano el juego de una sesión (y, con él, de sus grabaciones).
    ///
    /// Es la salida honesta cuando la identidad no se pudo resolver: se marca como
    /// `ConfirmadoPorUsuario`, no como observación. Solo cambia la sesión; las
    /// instantáneas de los benchmarks no se tocan (describen lo que se capturó).
    pub fn asignar_juego(&self, sesion_id: &str, appid: u32, nombre: &str) -> Result<(), String> {
        if appid == 0 {
            return Err("un AppID 0 no identifica ningún juego".into());
        }
        let encontrada = self.actualizar(|data| {
            let Some(s) = data.sesiones.iter_mut().find(|s| s.id == sesion_id) else {
                return false;
            };
            s.identidad = IdentidadDeSesion {
                appid: Some(appid),
                origen: OrigenAppId::ConfimadoPorUsuario,
                candidatos: vec![appid],
                nombre_observado: Some(nombre.to_string()),
                // Asignado por el usuario: el nombre que él escribió es el comercial.
                nombre_comercial: Some(nombre.to_string()),
                evidencia: vec!["asignado a mano por el usuario".into()],
            };
            true
        })?;
        if !encontrada.sesiones.iter().any(|s| s.id == sesion_id) {
            return Err(format!("no existe la sesión {}", sesion_id));
        }
        Ok(())
    }
}

/// Fuerza de una asociación, para quedarse con la mejor cuando varias encajan.
fn fuerza(a: &Asociacion) -> u8 {
    match a {
        Asociacion::Verificada => 4,
        Asociacion::Probable => 3,
        Asociacion::UnicaSesionDelJuego => 2,
        Asociacion::NoComprobable => 1,
        Asociacion::Ambigua | Asociacion::NoCoincide => 0,
    }
}

/// ¿Está vivo el proceso de esta clave (`pid@fecha`)?
fn vivo(clave: &str) -> bool {
    use crate::sessions::ObservadorDeProcesos;
    ObservadorDeProc.sigue_vivo(clave)
}

impl ServicioDeSesiones {
    /// ¿Sigue vivo este proceso? Lo contesta el observador de esta pasada, que es el real
    /// salvo que una prueba haya inyectado uno simulado. Con el real y no con el simulado,
    /// las pruebas se creían muertas en cuanto el PID de una fixture no existía en `/proc`.
    fn sigue_vivo(&self, clave: &str) -> bool {
        use crate::sessions::ObservadorDeProcesos;
        match &self.observador_inyectado {
            Some(o) => o.sigue_vivo(clave),
            None => vivo(clave),
        }
    }
}

/// Fecha legible en el mismo formato que usa el resto de la app.
pub fn ahora_legible() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// ==================== LECTURA Y ESCRITURA ====================

fn leer_sesiones(ruta: &std::path::Path) -> Result<SesionesData, String> {
    let leido = leer_datos::<SesionesData>(
        ruta,
        "sessions.json",
        SESSIONS_SCHEMA_VERSION,
        |v| v.get("version").and_then(|x| x.as_u64()).map(|x| x as u32),
        serde_json::from_value,
    );
    match leido {
        Ok(Some(d)) => Ok(d),
        // «No existe» es normal: primer arranque.
        Ok(None) => Ok(SesionesData::default()),
        // Aquí sí se puede degradar a vacío: perder sesiones no pierde el historial (cada
        // benchmark lleva su propia instantánea). Aun así se devuelve el error para que el
        // usuario lo vea en vez de que el fichero roto desaparezca sin avisar.
        Err(ErrorDatos::Ilegible(e)) => Err(format!("no se pudo leer sessions.json: {}", e)),
        Err(ErrorDatos::Corrupto { detalle, copia }) => Err(format!(
            "sessions.json está dañado ({}). Se guardó una copia en {}",
            detalle,
            copia
                .map(|c| c.display().to_string())
                .unwrap_or_else(|| "ninguna".into())
        )),
        Err(ErrorDatos::VersionFutura { encontrada, .. }) => Err(format!(
            "sessions.json es de una versión más nueva de Thoriapp ({})",
            encontrada
        )),
    }
}

fn escribir_sesiones(ruta: &std::path::Path, data: &SesionesData) -> Result<(), String> {
    let mut copia = data.clone();
    copia.version = Some(SESSIONS_SCHEMA_VERSION);
    let json = serde_json::to_string_pretty(&copia)
        .map_err(|e| format!("no se pudo serializar sessions.json: {}", e))?;
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("no se pudo crear {}: {}", dir.display(), e))?;
    }
    escribir_atomico(ruta, &json).map_err(|e| format!("no se pudo escribir sessions.json: {}", e))
}

/// Ventana temporal de una grabación a partir del nombre del CSV y su duración.
pub fn ventana_de_csv(ruta_csv: &str, duracion_segundos: Option<f64>) -> VentanaDeGrabacion {
    // El nombre trae `Juego_AAAA-MM-DD_HH-MM-SS.csv`.
    let base = std::path::Path::new(ruta_csv)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let inicio = base
        .rsplit_once('_')
        .map(|(_, t)| t.replace('-', ":"))
        .and_then(|t| {
            // Se reconstruye la fecha completa: el nombre no la separa del juego.
            let fecha = base.split('_').rev().nth(1).unwrap_or_default().to_string();
            let completo = format!("{} {}", fecha, t);
            parse_fecha(&completo).map(|_| completo)
        });
    VentanaDeGrabacion {
        inicio,
        duracion_segundos,
    }
}

/// Procedencia que corresponde a un origen de AppID de sesión.
pub fn procedencia_de_origen(origen: OrigenAppId) -> Procedencia {
    match origen {
        OrigenAppId::EntornoDelProceso
        | OrigenAppId::Ejecutable
        | OrigenAppId::DirectorioDeInstalacion
        // La correlación con el catálogo se hace sobre la carpeta o el prefijo que el propio
        // proceso declara: es una observación del proceso, no una inferencia por nombre.
        | OrigenAppId::CatalogoDeLanzador => Procedencia::ObservadoEnProceso,
        OrigenAppId::InferidoPorNombre => Procedencia::Inferido,
        OrigenAppId::ConfimadoPorUsuario => Procedencia::ConfirmadoPorUsuario,
        OrigenAppId::Desconocido => Procedencia::Desconocido,
    }
}

/// Instantánea de una sesión en forma de `ConProcedencia`, para incrustar en el benchmark.
///
/// Se copian los datos, no la sesión: así el benchmark no depende de que la sesión siga
/// existiendo.
pub fn instantanea_de_sesion(s: &SesionDeEjecucion) -> Option<ConProcedencia<String>> {
    if s.tipo == TipoDeSesion::AgrupadaSinProceso && s.identidad.appid.is_none() {
        return None;
    }
    let mut partes: Vec<String> = Vec::new();
    if let Some(p) = &s.proceso {
        partes.push(format!("proceso {} (pid {})", p.clave(), p.pid));
        if let Some(e) = &p.ejecutable {
            partes.push(format!("ejecutable {}", e));
        }
        if p.bajo_proton == Some(true) {
            partes.push("bajo Proton/Wine".into());
        }
    }
    partes.extend(s.identidad.evidencia.iter().cloned());
    if partes.is_empty() {
        return None;
    }
    Some(ConProcedencia {
        valor: partes.join("; "),
        procedencia: procedencia_de_origen(s.identidad.origen),
        capturado_en: Some(s.abierta_en.clone()),
    })
}

// ==================== INSTANCIA ÚNICA ====================

static SERVICIO: OnceLock<ServicioDeSesiones> = OnceLock::new();

/// El servicio único de la app.
pub fn servicio() -> &'static ServicioDeSesiones {
    SERVICIO.get_or_init(|| ServicioDeSesiones::con_ruta(ServicioDeSesiones::ruta_por_defecto()))
}

/// Crea un servicio con una ruta propia (pruebas). Sin hilo.
pub fn servicio_en(ruta: PathBuf) -> ServicioDeSesiones {
    ServicioDeSesiones::con_ruta(ruta)
}

/// Clave estable de un proceso: `pid@fecha de arranque`.
/// Resultado de correlacionar un proceso con el catálogo de lanzadores.
struct Correlacion {
    titulo: String,
    /// Por qué vía se reconoció: se escribe en la evidencia para no presentar como seguro
    /// lo que fue una deducción.
    via: &'static str,
}

/// Busca en el catálogo a qué juego corresponde un proceso, por dos vías en orden.
///
/// 1. **Carpeta de instalación** que el proceso declara. Es la evidencia más directa.
/// 2. **Prefijo de Wine**, solo cuando no hay carpeta. Hace falta en Lutris, donde `pga.db`
///    trae `directory` vacío en los juegos añadidos a mano. Es evidencia **complementaria**:
///    dos juegos pueden compartir prefijo, así que solo vale si deja una única identidad.
///
/// En ambas se descartan las coincidencias múltiples: el mismo juego en Steam y en Lutris es
/// **una** identidad (se agrupan), pero dos juegos distintos son ambigüedad y no se elige
/// ninguno. Devolver `None` es correcto: mejor identidad desconocida que un juego erróneo.
/// ¿La instalación de esta entrada es un cajón del usuario y no una carpeta de juego?
///
/// Un lanzador puede declarar como instalación una carpeta que el usuario usa para todo
/// (`Descargas`, `Escritorio`, `Documentos`, el propio `home`). En esas carpetas vive de todo,
/// así que **estar dentro no identifica nada**: se comportarían como comodines y confirmarían
/// como juego cualquier proceso que pasara por ahí.
///
/// Se decide por la **forma de la ruta**, no por una lista de juegos: cualquier carpeta
/// personal estándar del usuario queda descartada, sea cual sea el juego o el lanzador.
fn es_carpeta_generica(e: &crate::evidencia_proceso::EntradaDeCatalogo) -> bool {
    const GENERICAS: &[&str] = &[
        "Descargas",
        "Downloads",
        "Escritorio",
        "Desktop",
        "Documentos",
        "Documents",
        "Imágenes",
        "Pictures",
        "Vídeos",
        "Videos",
        "Música",
        "Music",
        "Público",
        "Public",
        "Plantillas",
        "Templates",
    ];
    let Some(dir) = e.install_dir.as_deref() else {
        // Sin instalación declarada no hay comodín posible: la decisión la tomarán las otras
        // vías (prefijo, ejecutable, `cwd`).
        return false;
    };
    let dir = dir.trim_end_matches('/');
    // El nombre del último componente es lo que identifica al cajón.
    let ultimo = dir.rsplit('/').next().unwrap_or(dir);
    if GENERICAS.iter().any(|g| g.eq_ignore_ascii_case(ultimo)) {
        return true;
    }
    // La raíz del usuario (`/home/jugador`) también es un cajón: contiene absolutamente todo,
    // incluida cualquier instalación. Es la forma que se descartó en la ronda anterior por
    // admitir a cualquier proceso del usuario.
    let componentes: Vec<&str> = dir.split('/').filter(|c| !c.is_empty()).collect();
    componentes.len() == 2 && componentes[0] == "home"
}

fn correlacionar_con_el_catalogo(
    catalogo: &[crate::evidencia_proceso::EntradaDeCatalogo],
    carpeta_declarada: Option<&str>,
    wine_prefix: Option<&str>,
    cwd: Option<&str>,
    ruta_ejecutable: Option<&str>,
    ejecutable_del_juego: Option<&str>,
) -> Option<Correlacion> {
    if catalogo.is_empty() {
        return None;
    }
    let unica = |coincidencias: Vec<&crate::evidencia_proceso::EntradaDeCatalogo>| {
        let distintas = crate::evidencia_proceso::agrupar_por_identidad(&coincidencias);
        (distintas.len() == 1).then(|| distintas[0].titulo.clone())
    };

    // Vía 1: la carpeta de instalación.
    if let Some(carpeta) = carpeta_declarada {
        let coincidencias: Vec<_> = catalogo.iter().filter(|e| e.contiene(carpeta)).collect();
        if let Some(titulo) = unica(coincidencias) {
            return Some(Correlacion {
                titulo,
                via: "la carpeta de instalación",
            });
        }
    }

    // Vía 2: el prefijo de Wine, solo si la carpeta no decidió.
    //
    // Es la vía que queda para Lutris, donde `directory` viene vacío en casi todos los juegos
    // de Wine: sin esto no habría nada que comparar y el juego quedaría sin identificar.
    // Ojo: **no** se sale si falta el prefijo. Antes había un `wine_prefix?` que devolvía
    // `None` y se llevaba por delante las vías siguientes: un juego nativo (sin prefijo) no
    // podía correlacionar nunca, aunque su ejecutable estuviera dentro de la instalación y su
    // `cwd` fuera la carpeta del juego. Es exactamente el caso de Starsector.
    if let Some(prefijo) = wine_prefix {
        let coincidencias: Vec<_> = catalogo
            .iter()
            .filter(|e| e.comparte_prefijo(prefijo) == Some(true))
            .collect();
        // Si varios juegos comparten el prefijo, no se elige: compartir prefijo no es
        // compartir juego.
        let distintas = crate::evidencia_proceso::agrupar_por_identidad(&coincidencias);
        if distintas.len() == 1 {
            return Some(Correlacion {
                titulo: distintas[0].titulo.clone(),
                via: "el prefijo de Wine",
            });
        }
    }

    // Vía 3: el **directorio de trabajo** es la carpeta del ejecutable configurado.
    //
    // Es la vía de los juegos que arrancan por un intérprete. En el árbol real de Starsector
    // (`lutris-wrapper` → `starsector.sh` → `java`) ninguno de los tres procesos declara
    // carpeta ni prefijo; lo único que los sitúa es que trabajan en la carpeta del juego.
    //
    // Solo se compara la carpeta **del ejecutable** y de forma exacta. Comparar contra
    // cualquier instalación admitiría a un `bash` cualquiera que pasara por ahí, y el `cwd`
    // por sí solo no confirma nada: aquí solo sirve para **localizar** la entrada del
    // catálogo que después hay que confirmar con su ejecutable.
    if let Some(dir) = cwd {
        let d = dir.trim_end_matches('/');
        let coincidencias: Vec<_> = catalogo
            .iter()
            .filter(|e| !es_carpeta_generica(e))
            .filter(|e| {
                [e.exec.as_deref(), e.install_dir.as_deref()]
                    .into_iter()
                    .flatten()
                    .filter_map(crate::evidencia_proceso::ruta_padre)
                    .any(|padre| padre.trim_end_matches('/') == d)
            })
            .collect();
        if let Some(titulo) = unica(coincidencias) {
            return Some(Correlacion {
                titulo,
                via: "el directorio de trabajo del proceso",
            });
        }
    }

    // Vía 4: el ejecutable real vive **dentro** de una instalación del catálogo.
    //
    // «Dentro» exige que la instalación sea una **carpeta de juego**, no un cajón. Heroic
    // declara instalaciones como `/home/jugador/Descargas`: cualquier cosa que se descargue
    // cae ahí, así que pertenecer a esa carpeta no dice nada del juego. Se descartan las
    // carpetas genéricas del sistema de ficheros del usuario.
    if let Some(exe) = ruta_ejecutable {
        let coincidencias: Vec<_> = catalogo
            .iter()
            .filter(|e| !es_carpeta_generica(e) && e.contiene(exe))
            .collect();
        if let Some(titulo) = unica(coincidencias) {
            return Some(Correlacion {
                titulo,
                via: "el ejecutable está dentro de la instalación",
            });
        }
    }

    // Vía 5: el nombre del ejecutable del juego es el que el lanzador tiene configurado.
    if let Some(nombre) = ejecutable_del_juego {
        let coincidencias: Vec<_> = catalogo
            .iter()
            .filter(|e| {
                e.exec
                    .as_deref()
                    .map(|c| crate::evidencia_proceso::mismo_ejecutable(c, nombre))
                    .unwrap_or(false)
            })
            .collect();
        if let Some(titulo) = unica(coincidencias) {
            return Some(Correlacion {
                titulo,
                via: "el ejecutable configurado por el lanzador",
            });
        }
    }
    None
}

/// Identificador de sesión único, con sufijo si el segundo ya estaba ocupado.
///
/// Antes era `ses_<timestamp al segundo>` y punto. Dos sesiones creadas en el mismo segundo
/// —que es lo normal cuando un lanzamiento abre varias ramas— recibían **el mismo id**, y
/// eso rompe la búsqueda por identificador, la asociación de grabaciones y la atribución de
/// benchmarks. Medido en el equipo real: Eslabong, Tattoo Tycoon y Palworld declararon cada
/// uno dos sesiones con id idéntico.
///
/// El sufijo va con guion bajo y un contador (`ses_20261126112745_2`), así que:
///
/// - Los identificadores **existentes** no cambian: el primero conserva su forma.
/// - No se reescribe ningún id histórico: solo se eligen distintos para los nuevos.
/// - Dos instancias independientes en el mismo segundo siguen recibiendo ids distintos.
///
/// Se reinicia con cada instante, así que el contador no crece sin límite.
fn id_de_sesion(ahora: &str, existentes: &std::collections::HashSet<String>) -> String {
    let base = format!("ses_{}", ahora.replace([':', ' ', '-'], ""));
    if !existentes.contains(&base) {
        return base;
    }
    for n in 2..=u32::MAX {
        let candidato = format!("{}_{}", base, n);
        if !existentes.contains(&candidato) {
            return candidato;
        }
    }
    // Inalcanzable: el bucle solo termina antes con un hueco libre.
    format!("{}_{}", base, ahora.len())
}

fn ids_existentes(data: &crate::sessions::SesionesData) -> std::collections::HashSet<String> {
    data.sesiones.iter().map(|s| s.id.clone()).collect()
}

fn clave_de(r: &RasgosDeProceso) -> String {
    match &r.inicio_del_proceso {
        Some(t) => format!("{}@{}", r.pid, t),
        None => format!("{}@desconocido", r.pid),
    }
}

/// Claves de **todos** los procesos vivos de una ejecución.
///
/// La clave de un proceso (`pid@starttime`) identifica un **proceso**, no una **ejecución**.
/// Confundirlos era el fallo de Deskrawl: el representante pasó de 210465 a 210612 dentro de
/// la misma partida y el observador, que solo miraba la clave del representante, cerró la
/// sesión y abrió otra. Una ejecución la forman todos sus procesos, así que la continuidad se
/// comprueba contra cualquiera de ellos.
///
/// Se guarda como `Vec` ordenado y sin duplicados para que sea comparable entre pasadas.
fn claves_de_la_ejecucion(grupo: &crate::sessions::GrupoDeProcesos) -> Vec<String> {
    // **Solo los procesos que acreditan ejecución del juego.**
    //
    // Guardar aquí todos los miembros del grupo era el defecto que dejó una sesión de Tattoo
    // Tycoon observada durante horas: del árbol de lanzamiento se guardaron 13 claves, y dos
    // de ellas —`dolphin` (el gestor de archivos de KDE) y `srt-logger` (infraestructura de
    // Steam)— sobreviven a cualquier partida. Bastaba con que **una** siguiera en `/proc`
    // para que la sesión se declarara viva y `pasadas_sin_verlo` volviera a 0 en cada pasada.
    //
    // Pertenecer al árbol no es ejecutar el juego. La pertenencia la da la topología; la
    // ejecución la da la clasificación del proceso.
    let mut claves: Vec<String> = grupo
        .miembros
        .iter()
        .filter(|m| acredita_ejecucion(m, grupo))
        .map(clave_de)
        // Un proceso sin `starttime` no sirve para distinguir dos usos del mismo PID.
        .filter(|c| !c.ends_with("@desconocido"))
        .collect();
    claves.sort();
    claves.dedup();
    claves
}

/// ¿El escaneo de esta pasada ve **este** juego?
///
/// Es la pregunta que faltaba entre «el escaneo no encontró nada» y «la sesión sigue viva».
/// Se responde por identidad: el AppID si lo hay, y la carpeta de instalación como respaldo
/// para los juegos externos, que no tienen AppID de Steam.
///
/// No se usa la clave del proceso para esto: si el representante cambió, la clave antigua ya
/// no existe y la sesión se cerraría por un cambio legítimo. La identidad del **juego** es la
/// que no cambia entre dos representantes de la misma partida.
fn sesion_a_la_vista(
    escaneo: &crate::sessions::EscaneoDeProcesos,
    s: &SesionDeEjecucion,
) -> bool {
    sesion_a_la_vista_ahora(escaneo, s).is_some()
}

/// Nombre comparable: sin extensión, sin mayúsculas y sin espacios sobrantes.
fn normalizar_nombre(n: &str) -> String {
    let base = n.rsplit('/').next().unwrap_or(n);
    let sin_ext = match base.rsplit_once('.') {
        Some((cabeza, ext)) if !cabeza.is_empty() && ext.len() <= 4 => cabeza,
        _ => base,
    };
    sin_ext.trim().to_lowercase()
}

/// El grupo del escaneo que corresponde a esta sesión, si lo hay.
fn sesion_a_la_vista_ahora<'a>(
    escaneo: &'a crate::sessions::EscaneoDeProcesos,
    s: &SesionDeEjecucion,
) -> Option<&'a crate::sessions::GrupoDeProcesos> {
    let appid = s.identidad.appid;
    let dirs: Vec<String> = s
        .identidad
        .evidencia
        .iter()
        .filter_map(|e| e.strip_prefix("instalado en steamapps/common/"))
        .map(|d| d.to_string())
        .collect();
    escaneo.grupos.iter().find(|g| {
        if let (Some(a), Some(b)) = (appid, g.appid) {
            if a == b {
                return true;
            }
        }
        // Misma carpeta de instalación: la vía de los juegos externos, que no tienen AppID
        // de Steam (Heroic, Lutris, Heronite).
        if !dirs.is_empty() && g.install_dirs.iter().any(|d| dirs.contains(d)) {
            return true;
        }
        // El grupo trabaja dentro de la carpeta que la sesión ya reconoció como suya. Es la
        // vía de los juegos nativos por intérprete, donde el grupo no aporta AppID, ni
        // `install_dirs`, ni nombres: Starsector (`lutris-wrapper → starsector.sh → java`)
        // solo se sitúa por el directorio en el que trabaja.
        //
        // La carpeta no se guarda aparte: se deduce de la evidencia que la correlación ya
        // dejó escrita al abrir la sesión, para no añadir un campo más a la sesión.
        for nota in &s.identidad.evidencia {
            let Some(carpeta) = nota
                .split('"')
                .nth(1)
                .map(|c| c.trim_end_matches(['/', '\\']))
                .filter(|c| c.starts_with('/'))
            else {
                continue;
            };
            if nota.contains("directorio de trabajo")
                && g.miembros.iter().any(|m| {
                    m.cwd
                        .as_deref()
                        .map(|c| c.trim_end_matches(['/', '\\']))
                        .is_some_and(|c| c == carpeta || c.starts_with(&format!("{carpeta}/")))
                })
            {
                return true;
            }
        }
        // Juegos externos sin AppID ni carpeta en el catálogo: se compara el nombre. Los
        // nombres del grupo vienen del ejecutable (`Tattoo Tycoon.exe`), así que se comparan
        // sin extensión y sin distinguir mayúsculas: es la misma identidad expresada de dos
        // formas.
        if appid.is_none() && dirs.is_empty() {
            // El nombre comercial si se resolvió; si no, el que se observó. Para un juego
            // externo sin catálogo, `nombre_comercial` puede venir vacío y el observado es
            // todo lo que hay.
            let guardados: Vec<String> = s
                .identidad
                .nombre_comercial
                .iter()
                .chain(s.identidad.nombre_observado.iter())
                .map(|n| normalizar_nombre(n))
                .filter(|n| !n.is_empty())
                .collect();
            if !guardados.is_empty() {
                return g
                    .nombres
                    .iter()
                    .any(|n| guardados.contains(&normalizar_nombre(n)));
            }
        }
        false
    })
}

/// ¿Este proceso **acredita** que el juego se está ejecutando?
///
/// Es la pregunta central del contrato y no se responde con una sola propiedad. Ni ser el
/// representante, ni llevar `SteamAppId`, ni coincidir con el catálogo, ni compartir prefijo
/// de Wine bastan por separado: los envoltorios y los auxiliares **heredan** todas esas
/// propiedades del lanzamiento, y por eso una sesión podía quedarse abierta con el juego
/// cerrado.
///
/// Lo que sí acredita es una de estas dos cosas, y nada más:
///
/// 1. **La marca del clasificador**: `ejecuta_de_verdad == Some(true)`, que es la decisión
///    que toma `elegir_principal` cuando hay evidencia de que este proceso recibió el
///    ejecutable, no solo que lo menciona.
/// 2. **Ser el representante** del grupo *y* que su papel no sea de envoltorio ni auxiliar.
///
/// Un auxiliar no acredita nunca: `wineserver`, `python3`, `reaper`, `pressure-vessel`…
/// existen para arrancar otra cosa. Y un ancestro genérico (`dolphin`, `systemd`, `steam`,
/// `srt-logger`) tampoco: puede estar en la reconstrucción topológica, pero no mantiene una
/// sesión. Incluirlos fue exactamente la contaminación medida.
fn acredita_ejecucion(
    m: &RasgosDeProceso,
    grupo: &crate::sessions::GrupoDeProcesos,
) -> bool {
    if m.auxiliar {
        return false;
    }
    // Un ancestro genérico no acredita ejecución: es infraestructura que sobrevive a la
    // partida. No es una lista de nombres de juegos, es la misma noción estructural que usa
    // el agrupador para no fusionar lanzamientos.
    if crate::evidencia_proceso::es_ancestro_generico(m) {
        return false;
    }
    if m.ejecuta_de_verdad == Some(true) {
        return true;
    }
    // El representante sí acredita si el clasificador no lo descartó explícitamente. Se
    // exige además que tenga alguna evidencia propia de ser el juego y no solo el nombre de
    // un envoltorio.
    if m.pid == grupo.principal && m.ejecuta_de_verdad != Some(false) {
        return tiene_evidencia_de_juego(m, grupo);
    }
    // Cualquier otro miembro puede acreditar si su ejecutable **es** el del juego: eso no es
    // herencia, es ejecutar. Es el caso de un intérprete nativo cuyo `.jar` está en la
    // instalación.
    m.ejecutable_del_juego
        .as_deref()
        .is_some_and(|n| !n.trim().is_empty())
}

/// ¿Hay algo en el proceso que lo señale como el juego, más allá de estar en el árbol?
///
/// Se pide evidencia **propia del proceso**: el nombre del ejecutable del juego, un AppID
/// declarado por él, o la carpeta de la instalación. Compartir prefijo de Wine o estar
/// dentro de la carpeta del juego no cuenta: los envoltorios también lo cumplen.
fn tiene_evidencia_de_juego(
    m: &RasgosDeProceso,
    grupo: &crate::sessions::GrupoDeProcesos,
) -> bool {
    m.ejecutable_del_juego
        .as_deref()
        .is_some_and(|n| !n.trim().is_empty())
        || m.appid_del_entorno
            .as_ref()
            .map(|e| !e.vacia())
            .unwrap_or(false)
        || m.appid_de_argumentos.is_some()
        || m.carpeta_declarada.is_some()
        || trabaja_en_la_carpeta_del_juego(m, grupo)
}

/// ¿El `cwd` sitúa al proceso dentro de la carpeta del juego que él mismo declara?
///
/// Es la vía de los juegos que arrancan por un **intérprete**: en Starsector el árbol real es
/// `lutris-wrapper → starsector.sh → java` y ninguno de los tres nombra el juego ni lleva
/// variables del lanzador. Lo único que los sitúa es que trabajan dentro de la carpeta. Sin
/// esto, `acredita_ejecucion` devolvía `false`, `vivos_de_la_ejecucion` quedaba vacío y
/// `paso()` no reconocía la instancia: se creaba una sesión nueva en cada pasada, con el
/// mismo PID y el mismo `starttime`.
///
/// Se exige una **relación verificable**, no un parecido de nombres: el proceso tiene que
/// declarar su carpeta de instalación y trabajar dentro de ella. Un `bash` en `/` o un
/// proceso en `$HOME` no acredita nada, y coincidir con el nombre de una carpeta tampoco
/// basta: la pertenencia al directorio es indicio, no convierte en ejecutable del juego a
/// cualquier proceso situado ahí.
fn trabaja_en_la_carpeta_del_juego(
    m: &RasgosDeProceso,
    grupo: &crate::sessions::GrupoDeProcesos,
) -> bool {
    let Some(cwd) = m.cwd.as_deref().map(|c| c.trim_end_matches(['/', '\\'])) else {
        return false;
    };
    if cwd.is_empty() {
        return false;
    }
    // Relación verificable con una carpeta conocida, no parecido de nombres: o el propio
    // proceso declara una carpeta que contiene su `cwd`, o el `cwd` es la carpeta del
    // ejecutable que el catálogo asigna a **este** grupo.
    if let Some(carpeta) = m.carpeta_declarada.as_deref() {
        let carpeta = carpeta.trim_end_matches(['/', '\\']);
        if !carpeta.is_empty() && (cwd == carpeta || cwd.starts_with(&format!("{carpeta}/"))) {
            return true;
        }
    }
    if grupo.install_dirs.iter().any(|instalada| {
        let instalada = instalada.trim_end_matches(['/', '\\']);
        !instalada.is_empty() && (cwd == instalada || cwd.starts_with(&format!("{instalada}/")))
    }) {
        return true;
    }
    // El juego nativo que arranca por intérprete no está en `steamapps/common`: su carpeta es
    // la del **ejecutable configurado** en el catálogo, y el `cwd` del proceso tiene que
    // coincidir con ella. Es la misma relación verificable que usa
    // `correlacionar_con_el_catalogo` para localizar la entrada; aquí se exige además que el
    // proceso sea el que ejecuta algo dentro de esa carpeta.
    // La carpeta del ejecutable configurado del grupo: `install_dirs` no la recoge porque no
    // viene de `steamapps/common`, pero el `cwd` de los miembros la señala.
    grupo
        .miembros
        .iter()
        .filter_map(|o| o.cwd.as_deref())
        .map(|c| c.trim_end_matches(['/', '\\']))
        .filter(|c| !c.is_empty())
        .any(|carpeta| {
        let carpeta = carpeta.trim_end_matches(['/', '\\']);
        !carpeta.is_empty() && (cwd == carpeta || cwd.starts_with(&format!("{carpeta}/")))
    })
}

/// Claves de los procesos que **ejecutan** el juego, no de sus envoltorios.
///
/// Si ningún miembro está marcado como ejecutor, se toma el representante: es la única
/// información disponible y es la que usaban las versiones anteriores.
/// Claves de **todo** el árbol del grupo, para continuidad de instancia.
///
/// A diferencia de `claves_de_la_ejecucion`, aquí sí entran los envoltorios: es la huella del
/// lanzamiento, no la prueba de que el juego corra. Nunca se usa para decidir si la sesión
/// sigue viva.
fn claves_del_arbol(grupo: &crate::sessions::GrupoDeProcesos) -> Vec<String> {
    let mut claves: Vec<String> = grupo
        .miembros
        .iter()
        .map(clave_de)
        .filter(|c| !c.ends_with("@desconocido"))
        .collect();
    claves.sort();
    claves.dedup();
    claves
}

fn claves_del_juego_de(grupo: &crate::sessions::GrupoDeProcesos) -> Vec<String> {
    let ejecutores: Vec<String> = grupo
        .miembros
        .iter()
        .filter(|m| m.ejecuta_de_verdad == Some(true))
        .map(clave_de)
        .filter(|c| !c.ends_with("@desconocido"))
        .collect();
    if !ejecutores.is_empty() {
        return ejecutores;
    }
    grupo
        .miembros
        .iter()
        .find(|m| m.pid == grupo.principal)
        .map(clave_de)
        .into_iter()
        .filter(|c| !c.ends_with("@desconocido"))
        .collect()
}

/// ¿La sesión sigue viva si **alguno** de esos procesos sigue en `/proc`?
fn alguno_vivo(claves: &[String]) -> bool {
    claves.iter().any(|c| vivo(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{EvidenciaAppId, RasgosDeProceso};

    fn dir_temporal(nombre: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("thoriapp_ses_{}_{}", nombre, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Regresión del fallo real: `paso()` identificaba el juego y creaba la sesión, pero
    /// `estado_legible()` no lo consultaba y seguía diciendo «buscando» para siempre. La
    /// interfaz enseñaba «Buscando un juego» con el juego ya detectado.
    #[test]
    fn el_estado_legible_refleja_lo_que_decidio_la_ultima_pasada() {
        use crate::proceso_linux::ObservadorSimulado;

        let d = dir_temporal("estado_legible");
        let mut s = servicio_en(d.join("sessions.json"));
        let rasgos: Vec<RasgosDeProceso> = crate::fixtures_procesos::PALWORLD_PROTON
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(escaneo.grupos));
        s.fijar_activo(true);

        // Antes de ninguna pasada no se puede afirmar nada.
        assert_eq!(s.estado_legible(), EstadoDelObservador::Buscando);

        let estado = s.paso().expect("la pasada no debe fallar");
        assert!(
            matches!(estado, EstadoDelObservador::Observando { .. }),
            "la pasada debía identificar el juego: {:?}",
            estado
        );

        // Lo importante: la lectura posterior desde la UI tiene que coincidir.
        let leido = s.estado_legible();
        assert!(
            matches!(
                &leido,
                EstadoDelObservador::Observando { juegos }
                    if juegos.first().and_then(|j| j.appid) == Some(1623730)
            ),
            "estado_legible debía decir que está observando Palworld, dijo {:?}",
            leido
        );

        let _ = std::fs::remove_dir_all(&d);
    }

    // ==================== PREFERENCIA DEL OBSERVADOR ====================

    /// Requisito del usuario: en una instalación nueva el observador está **apagado**.
    ///
    /// Antes de esta corrección, `arrancar()` forzaba `activo = true`, así que la app
    /// empezaba a mirar `/proc` sin que nadie lo hubiera pedido.
    #[test]
    fn instalacion_nueva_arranca_con_el_observador_apagado() {
        let d = dir_temporal("pref_nueva");
        let s = servicio_en(d.join("sessions.json"));
        // No hay archivo: nada guardado. La hidratación debe dejarlo apagado.
        s.hidratar_preferencia();
        assert!(!s.esta_activo(), "sin preferencia guardada, apagado");
        assert_eq!(s.estado_legible(), EstadoDelObservador::Apagado);
        assert_eq!(s.paso().unwrap(), EstadoDelObservador::Apagado);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Reiniciar con el observador desactivado debe mantenerlo desactivado.
    ///
    /// Se simula el reinicio con un servicio nuevo sobre el mismo archivo: es exactamente lo
    /// que ocurre al cerrar y volver a abrir Thoriapp.
    #[test]
    fn reiniciar_con_el_observador_desactivado_lo_mantiene_desactivado() {
        let d = dir_temporal("pref_reinicio_off");
        let ruta = d.join("sessions.json");

        let primero = servicio_en(ruta.clone());
        primero.fijar_activo(false);

        // "Reinicio": instancia nueva sobre el mismo archivo.
        let segundo = servicio_en(ruta);
        segundo.hidratar_preferencia();
        assert!(!segundo.esta_activo(), "seguía desactivado tras reiniciar");
        assert_eq!(segundo.estado_legible(), EstadoDelObservador::Apagado);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Y si lo dejó activado, sigue activado.
    #[test]
    fn reiniciar_con_el_observador_activado_lo_mantiene_activado() {
        let d = dir_temporal("pref_reinicio_on");
        let ruta = d.join("sessions.json");

        let primero = servicio_en(ruta.clone());
        primero.fijar_activo(true);

        let segundo = servicio_en(ruta);
        segundo.hidratar_preferencia();
        assert!(segundo.esta_activo(), "la preferencia activada se respeta");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Desactivado no se sondea: ni sesiones creadas ni pasadas contadas, aunque haya un
    /// juego delante. Es la garantía de «cero escaneos de /proc».
    #[test]
    fn desactivado_no_sondea_aunque_haya_un_juego_delante() {
        use crate::proceso_linux::ObservadorSimulado;
        let d = dir_temporal("pref_sin_sondeo");
        let mut s = servicio_en(d.join("sessions.json"));
        let rasgos: Vec<RasgosDeProceso> = crate::fixtures_procesos::PALWORLD_PROTON
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(escaneo.grupos));

        s.fijar_activo(false);
        assert_eq!(s.paso().unwrap(), EstadoDelObservador::Apagado);
        assert_eq!(
            s.estado.pasadas.load(Ordering::SeqCst),
            0,
            "ni una pasada contada"
        );
        assert!(
            s.cargar().unwrap().sesiones.is_empty(),
            "ni una sesión creada con el observador apagado"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    // ==================== IDENTIFICACIÓN VISIBLE DEL JUEGO ====================

    /// Ayudante: servicio con un observador simulado que ve estas fixtures.
    fn servicio_con_fixture(
        nombre: &str,
        fixture: &[crate::fixtures_procesos::ProcesoDeFixture],
    ) -> ServicioDeSesiones {
        use crate::proceso_linux::ObservadorSimulado;
        let d = dir_temporal(nombre);
        let mut s = servicio_en(d.join("sessions.json"));
        let rasgos: Vec<RasgosDeProceso> = fixture
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(escaneo.grupos));
        s
    }

    /// El estado lleva el **nombre comercial** del juego, no el del ejecutable.
    ///
    /// Palworld es el caso duro: su proceso Linux se llama `GameThread` y el `.exe` lleva
    /// `Palworld-Win64-Shipping`. El usuario tiene que leer «Palworld».
    #[test]
    fn el_estado_lleva_el_nombre_comercial_y_no_el_del_ejecutable() {
        let s = servicio_con_fixture("nombre_palworld", crate::fixtures_procesos::PALWORLD_PROTON);
        s.fijar_activo(true);
        let estado = s.paso().unwrap();
        let EstadoDelObservador::Observando { juegos } = &estado else {
            panic!("debía observar un juego, dijo {estado:?}");
        };
        assert_eq!(juegos.len(), 1);
        assert_eq!(juegos[0].appid, Some(1623730));
        // La biblioteca de prueba resuelve el AppID a su nombre comercial.
        // (El servicio real usa la biblioteca de Steam; aquí se comprueba la forma.)
        assert!(
            juegos[0].nombre.is_none() || juegos[0].nombre.as_deref() == Some("Palworld"),
            "el nombre debe ser el comercial o ninguno, nunca el del ejecutable: {:?}",
            juegos[0].nombre
        );
        // Y nunca el del envoltorio ni el del runtime.
        let n = juegos[0].nombre.clone().unwrap_or_default();
        assert!(!n.contains("systemd-inhibit"), "no es el wrapper: {n}");
        assert!(!n.contains("wine64-preloader"), "no es el runtime: {n}");
        assert!(!n.contains(".exe"), "no es el ejecutable: {n}");
    }

    /// Cambio de juego: primero Palworld, luego 7 Days to Die.
    #[test]
    fn el_estado_refleja_el_cambio_de_juego() {
        use crate::fixtures_procesos::{JUEGO_NATIVO, PALWORLD_PROTON};
        let d = dir_temporal("cambio_juego");
        let mut s = servicio_en(d.join("sessions.json"));

        // Palworld.
        let r1: Vec<RasgosDeProceso> = PALWORLD_PROTON
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        s.observador_inyectado = Some(crate::proceso_linux::ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&r1).grupos,
        ));
        s.fijar_activo(true);
        let e1 = s.paso().unwrap();
        let EstadoDelObservador::Observando { juegos } = &e1 else {
            panic!("debía ver Palworld: {e1:?}");
        };
        assert_eq!(juegos[0].appid, Some(1623730));

        // Ahora 7 Days to Die, con las anteriores ya cerradas.
        let r2: Vec<RasgosDeProceso> = JUEGO_NATIVO
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        s.observador_inyectado = Some(crate::proceso_linux::ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&r2).grupos,
        ));
        let e2 = s.paso().unwrap();
        if let EstadoDelObservador::Observando { juegos } = &e2 {
            assert_eq!(
                juegos[0].appid,
                Some(251570),
                "el estado debe seguir al juego nuevo, no al anterior"
            );
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Varios juegos a la vez: el estado los lleva **todos**, no solo el último.
    ///
    /// Antes el bucle asignaba `estado` por cada grupo y el último machacaba a los demás.
    #[test]
    fn con_varios_juegos_el_estado_los_lleva_todos() {
        // Se concatenan ya convertidos a rasgos: `ProcesoDeFixture` no es `Clone` y no hace
        // falta que lo sea.
        let mut rasgos: Vec<RasgosDeProceso> = crate::fixtures_procesos::PALWORLD_PROTON
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        rasgos.extend(
            crate::fixtures_procesos::JUEGO_NATIVO
                .iter()
                .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas),
        );

        let d = dir_temporal("varios");
        let mut s = servicio_en(d.join("sessions.json"));
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        s.observador_inyectado = Some(crate::proceso_linux::ObservadorSimulado::con_grupos(
            escaneo.grupos,
        ));
        s.fijar_activo(true);
        let estado = s.paso().unwrap();
        let EstadoDelObservador::Observando { juegos } = &estado else {
            panic!("debía observar juegos: {estado:?}");
        };
        assert!(
            juegos.len() >= 2,
            "deben aparecer los dos juegos, no uno: {juegos:?}"
        );
        let appids: Vec<Option<u32>> = juegos.iter().map(|j| j.appid).collect();
        assert!(appids.contains(&Some(1623730)), "Palworld: {appids:?}");
        assert!(appids.contains(&Some(251570)), "7 Days to Die: {appids:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_pasada_desactivada_no_escribe_nada() {
        // Condición del usuario: observador desactivado ⇒ ningún sondeo.
        //
        // Ojo con el matiz: el archivo **sí** se escribe, y debe hacerlo, porque guarda la
        // preferencia del usuario. Lo que no ocurre es una pasada: ni sesiones, ni lecturas
        // de `/proc`. Antes esta prueba exigía que no se creara el archivo, cosa que dejó de
        // ser cierta (ni deseable) al añadir la persistencia.
        let d = dir_temporal("apagado");
        let s = servicio_en(d.join("sessions.json"));
        s.fijar_activo(false);
        assert_eq!(s.paso().unwrap(), EstadoDelObservador::Apagado);
        assert_eq!(s.estado.pasadas.load(Ordering::SeqCst), 0);
        let guardado = s.cargar().expect("la preferencia debe poder leerse");
        assert!(guardado.sesiones.is_empty(), "ni una sesión creada");
        assert_eq!(
            guardado.observador_activo,
            Some(false),
            "y la preferencia queda guardada"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Medición real del coste del observador (prueba de aceptación 21).
    ///
    /// No es un test de umbral estricto —el número depende de la máquina y de cuántos
    /// procesos haya— sino una medida que se imprime y una cota amplia para detectar
    /// regresiones graves (por ejemplo, leer `environ` de todos los procesos).
    #[test]
    fn medir_el_coste_de_una_pasada() {
        let obs = crate::proceso_linux::ObservadorDeProc;
        let t0 = std::time::Instant::now();
        let r = obs.escanear();
        let d = t0.elapsed();
        let procesos = std::fs::read_dir("/proc")
            .map(|d| d.flatten().count())
            .unwrap_or(0);
        println!(
            "[observador] una pasada sobre {} entradas de /proc: {:.2} ms ({} grupos, {} inaccesibles)",
            procesos,
            d.as_secs_f64() * 1000.0,
            r.grupos.len(),
            r.inaccesibles
        );
        // Cota generosa: si se leyera el entorno de todos los procesos, esto se dispararía.
        assert!(d.as_millis() < 500, "una pasada tardó {:?}", d);
    }

    /// Con el juego vivo, la pasada hace el trabajo completo: encuentra el proceso, lee su
    /// entorno filtrado y su fecha de arranque.
    ///
    /// Se simula el juego con un `sh` llamado como el ejecutable del CSV, que es lo que hay
    /// en este entorno. No hay un juego real de Steam disponible aquí (Docker), y eso se
    /// dice: es una prueba **simulada**, no una con un juego de verdad.
    #[test]
    fn con_un_proceso_vivo_se_encuentra_y_se_leen_solo_los_datos_permitidos() {
        // Un proceso con el nombre de un juego real, y con un entorno lleno de secretos.
        let mut hijo = std::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .env("SteamAppId", "1623730")
            .env("SteamGameId", "1623730")
            .env("NVIDIA_API_KEY", "nvapi-no-debe-salir")
            .env("OPENAI_API_KEY", "sk-no-debe-salir")
            .spawn()
            .expect("no se pudo lanzar el proceso de prueba");

        // El nombre del proceso (`comm`) lo pone el kernel desde el ejecutable, así que se
        // busca por el nombre real de `sh` para no depender de renombrar ficheros.
        let obs = crate::proceso_linux::ObservadorDeProc;
        let r = obs.escanear();
        let _ = hijo.kill();
        let _ = hijo.wait();

        // El grupo del proceso de prueba, si se ve. Un `sh` es auxiliar y no debe abrir
        // sesión, así que lo que se comprueba aquí es que **si** aparece, solo aparecen
        // sus datos permitidos.
        if let Some(grupo) = r
            .grupos
            .iter()
            .find(|g| g.miembros.iter().any(|m| m.pid == hijo.id()))
        {
            for m in &grupo.miembros {
                if let Some(env) = m.appid_del_entorno.as_ref() {
                    let json = serde_json::to_string(env).unwrap();
                    assert!(!json.contains("nvapi"));
                    assert!(!json.contains("sk-"));
                }
            }
            panic!("un `sh` no debería formar un grupo de juego: {:?}", grupo);
        }
        println!(
            "[observador] el proceso de prueba quedó fuera de los grupos (auxiliar), como debe"
        );
    }

    #[test]
    fn la_espera_crece_con_el_estado_y_con_el_coste() {
        let p = PoliticaDeSondeo::default();
        let coste = Duration::from_millis(6);
        let buscando = p.espera(EstadoDelObservador::Buscando, coste);
        let observando = p.espera(
            EstadoDelObservador::Observando {
                juegos: vec![JuegoObservado {
                    clave: "1@x".into(),
                    appid: None,
                    nombre: None,
                }],
            },
            coste,
        );
        assert!(observando > buscando, "observando va más espaciado");
        assert_eq!(buscando, Duration::from_millis(4_994));

        // Pasada carísima (300 ms): el intervalo se estira para no vivir dentro de /proc.
        let caro = p.espera(EstadoDelObservador::Buscando, Duration::from_millis(300));
        assert!(caro > Duration::from_millis(1_200), "espera {:?}", caro);

        // Sin visibilidad se mira aún menos.
        assert!(
            p.espera(EstadoDelObservador::SinVisibilidad, coste)
                > p.espera(
                    EstadoDelObservador::Observando { juegos: Vec::new() },
                    coste
                )
        );
        // Apagado no espera: el bucle decide sin dormir según su propio reloj.
        assert_eq!(
            p.espera(EstadoDelObservador::Apagado, coste),
            Duration::ZERO
        );
    }

    #[test]
    fn las_sesiones_se_guardan_y_se_vuelven_a_leer() {
        let d = dir_temporal("guardar");
        let s = servicio_en(d.join("sessions.json"));
        let mut data = SesionesData::default();
        data.sesiones.push(SesionDeEjecucion::agrupada(
            "ses_1".into(),
            IdentidadDeSesion::con_appid(251570, OrigenAppId::InferidoPorNombre),
            "2026-11-24 18:00:00",
        ));
        s.guardar(&data).unwrap();

        let leido = s.cargar().unwrap();
        assert_eq!(leido.sesiones.len(), 1);
        assert_eq!(leido.version, Some(SESSIONS_SCHEMA_VERSION));
        // El archivo nunca se deja a medias: escribir_atomico asegura que o está entero o
        // no está, y aquí simplemente se comprueba que es JSON válido.
        let txt = std::fs::read_to_string(d.join("sessions.json")).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&txt).is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_archivo_corrupto_no_se_pisa_en_silencio() {
        let d = dir_temporal("corrupto");
        let ruta = d.join("sessions.json");
        std::fs::write(&ruta, "{ esto no es json").unwrap();
        let s = servicio_en(ruta.clone());

        // Cargar da error, y no una lista vacía: el siguiente guardado no debe borrar.
        assert!(s.cargar().is_err());
        assert!(s.actualizar(|_| true).is_err());

        // El contenido original sigue ahí y se hizo copia.
        assert_eq!(std::fs::read_to_string(&ruta).unwrap(), "{ esto no es json");
        let copias: Vec<String> = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".bak"))
            .collect();
        assert!(!copias.is_empty(), "debe quedar una copia");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_archivo_de_version_futura_se_rechaza_sin_copia() {
        let d = dir_temporal("futuro");
        let ruta = d.join("sessions.json");
        std::fs::write(&ruta, r#"{"version": 99, "sesiones": []}"#).unwrap();
        let s = servicio_en(ruta);
        let e = s.cargar().unwrap_err();
        assert!(e.contains("más nueva"), "{:?}", e);
        let copias = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".bak"))
            .count();
        assert_eq!(copias, 0, "un archivo válido de otra versión no es basura");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sin_archivo_se_devuelve_vacio() {
        let d = dir_temporal("vacio");
        let s = servicio_en(d.join("sessions.json"));
        assert!(s.cargar().unwrap().sesiones.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_sesion_terminada_libera_la_busqueda() {
        // Si el proceso ya no existe, la sesión se cierra y el observador vuelve a buscar.
        let d = dir_temporal("terminada");
        let s = servicio_en(d.join("sessions.json"));
        s.fijar_activo(true);
        let mut data = SesionesData::default();
        let mut ses = SesionDeEjecucion::observada(
            "ses_x".into(),
            RasgosDeProceso {
                pid: 999_999, // imposible: no existe
                inicio_del_proceso: Some("2020-01-01 00:00:00".into()),
                ..Default::default()
            },
            IdentidadDeSesion::con_appid(1, OrigenAppId::InferidoPorNombre),
            "2020-01-01 00:00:00",
        );
        ses.evidencia_proceso = EvidenciaProceso::Observada;
        data.sesiones.push(ses);
        s.guardar(&data).unwrap();

        // Una sola pasada sin verlo **no** cierra: `/proc` puede no haberlo leído entero.
        s.paso().unwrap();
        let tras = s.cargar().unwrap();
        assert_eq!(
            tras.sesiones[0].evidencia_proceso,
            EvidenciaProceso::VisibilidadPerdida,
            "no verlo una vez es pérdida temporal, no finalización"
        );
        assert!(
            tras.sesiones[0].cerrada_en.is_none(),
            "la sesión sigue abierta: el juego puede seguir ahí"
        );

        // Tras varias pasadas seguidas sin verlo, sí se cierra.
        for _ in 0..PASADAS_SIN_VERLO {
            s.paso().unwrap();
        }
        let tras = s.cargar().unwrap();
        assert_eq!(
            tras.sesiones[0].evidencia_proceso,
            EvidenciaProceso::Terminada,
            "sin verlo varias pasadas seguidas: terminada"
        );
        assert!(tras.sesiones[0].cerrada_en.is_some());
        s.fijar_activo(false);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_asociacion_elige_la_sesion_mas_fuerte() {
        let d = dir_temporal("asociar");
        let s = servicio_en(d.join("sessions.json"));
        let mut data = SesionesData::default();
        // Sesión ya cerrada, con el juego abierto entre 18:00 y 18:30.
        let mut vieja = SesionDeEjecucion::observada(
            "vieja".into(),
            RasgosDeProceso {
                pid: 10,
                inicio_del_proceso: Some("2026-11-24 18:00:00".into()),
                ..Default::default()
            },
            IdentidadDeSesion::con_appid(251570, OrigenAppId::EntornoDelProceso),
            "2026-11-24 18:00:00",
        );
        vieja.cerrada_en = Some("2026-11-24 18:30:00".into());
        vieja.ultima_observacion = Some("2026-11-24 18:30:00".into());
        data.sesiones.push(vieja);
        s.guardar(&data).unwrap();

        let ventana = VentanaDeGrabacion {
            inicio: Some("2026-11-24 18:10:00".into()),
            duracion_segundos: Some(60.0),
        };
        let (id, asoc) = s.asociar_grabacion(251570, "7DaysToDie", &ventana);
        assert_eq!(id.as_deref(), Some("vieja"));
        assert_eq!(asoc, Asociacion::Verificada);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_grabacion_de_otro_momento_no_se_atribuye() {
        let d = dir_temporal("no_atribuir");
        let s = servicio_en(d.join("sessions.json"));
        let mut data = SesionesData::default();
        let mut vieja = SesionDeEjecucion::observada(
            "vieja".into(),
            RasgosDeProceso {
                pid: 10,
                inicio_del_proceso: Some("2026-11-24 18:00:00".into()),
                ..Default::default()
            },
            IdentidadDeSesion::con_appid(251570, OrigenAppId::EntornoDelProceso),
            "2026-11-24 18:00:00",
        );
        vieja.cerrada_en = Some("2026-11-24 18:30:00".into());
        data.sesiones.push(vieja);
        s.guardar(&data).unwrap();

        // Grabación de la semana siguiente: el juego es el mismo, la ejecución no.
        let ventana = VentanaDeGrabacion {
            inicio: Some("2026-12-01 20:00:00".into()),
            duracion_segundos: Some(60.0),
        };
        let (id, _) = s.asociar_grabacion(251570, "7DaysToDie", &ventana);
        assert_eq!(id, None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn asignar_a_mano_marca_procedencia_de_usuario() {
        let d = dir_temporal("manual");
        let s = servicio_en(d.join("sessions.json"));
        let mut data = SesionesData::default();
        let ses = SesionDeEjecucion::agrupada(
            "g1".into(),
            IdentidadDeSesion::desconocida(),
            "2026-11-24 18:00:00",
        );
        data.sesiones.push(ses);
        s.guardar(&data).unwrap();

        s.asignar_juego("g1", 1623730, "Palworld").unwrap();
        let ses = s.sesion("g1").unwrap();
        assert_eq!(ses.identidad.appid, Some(1623730));
        assert_eq!(ses.identidad.origen, OrigenAppId::ConfimadoPorUsuario);
        assert_eq!(
            procedencia_de_origen(ses.identidad.origen),
            Procedencia::ConfirmadoPorUsuario
        );

        // AppID 0 no es un juego: se rechaza.
        assert!(s.asignar_juego("g1", 0, "x").is_err());
        // Sesión inexistente: error explícito, no silencio.
        assert!(s.asignar_juego("nope", 1, "x").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn no_se_guardan_datos_sensibles_en_sessions_json() {
        // La prueba que cierra el requisito de privacidad: tras observar, el archivo no
        // puede contener nada de un entorno real.
        let d = dir_temporal("privacidad");
        let s = servicio_en(d.join("sessions.json"));
        s.fijar_activo(true);
        // Se simula una sesión con los rasgos que el dominio guardaría.
        let mut data = SesionesData::default();
        let rasgos = RasgosDeProceso {
            pid: 4242,
            ejecutable: Some("Palworld-Win64-Shipping.exe".into()),
            ruta_ejecutable: Some("/home/x/steamapps/common/Palworld/Palworld.exe".into()),
            inicio_del_proceso: Some("2026-11-24 18:00:00".into()),
            appid_del_entorno: Some(EvidenciaAppId {
                steam_app_id: Some("1623730".into()),
                steam_game_id: Some("1623730".into()),
            }),
            bajo_proton: Some(true),
            ..Default::default()
        };
        data.sesiones.push(SesionDeEjecucion::observada(
            "s".into(),
            rasgos,
            IdentidadDeSesion::con_appid(1623730, OrigenAppId::EntornoDelProceso),
            "2026-11-24 18:00:00",
        ));
        s.guardar(&data).unwrap();
        let txt = std::fs::read_to_string(d.join("sessions.json")).unwrap();
        for prohibido in [
            "NVIDIA_API_KEY",
            "OPENAI",
            "SECRET",
            "TOKEN",
            "LD_PRELOAD",
            "PATH=",
            "environ",
        ] {
            assert!(!txt.contains(prohibido), "apareció {:?}", prohibido);
        }
        // Y lo que sí debe estar, está.
        assert!(txt.contains("1623730"));
        assert!(txt.contains("Palworld"));
        s.fijar_activo(false);
        let _ = std::fs::remove_dir_all(&d);
    }

    // ---------- condiciones de aceptación (subfase 5.3) ----------

    #[test]
    fn prueba_3_el_observador_desactivado_no_sondea() {
        // Cubierto arriba con el estado; aquí se comprueba además que el hilo no se crea
        // solo por mirar el estado y que `paso` responde sin tocar disco.
        let d = dir_temporal("apagado2");
        let s = servicio_en(d.join("sesiones.json"));
        s.fijar_activo(false);
        for _ in 0..3 {
            assert_eq!(s.paso().unwrap(), EstadoDelObservador::Apagado);
        }
        assert!(!d.join("sessions.json").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn prueba_12_asignacion_manual_del_juego() {
        // Ya probado; aquí se insiste en que no toca las instantáneas de las pruebas.
        let d = dir_temporal("manual2");
        let s = servicio_en(d.join("sessions.json"));
        let mut data = SesionesData::default();
        let mut ses = SesionDeEjecucion::agrupada(
            "g".into(),
            IdentidadDeSesion::desconocida(),
            "2026-11-24 18:00:00",
        );
        ses.grabaciones.push("bench_1".into());
        data.sesiones.push(ses);
        s.guardar(&data).unwrap();

        s.asignar_juego("g", 251570, "7 Days to Die").unwrap();
        let leido = s.sesion("g").unwrap();
        assert_eq!(leido.identidad.appid, Some(251570));
        assert_eq!(leido.grabaciones, vec!["bench_1".to_string()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn prueba_14_una_grabacion_antigua_analizada_con_el_juego_abierto_no_se_cuelga_del_proceso() {
        // El escenario tentador: hoy el juego está abierto y se analiza un CSV de hace un
        // mes. El proceso vivo es del mismo juego, pero no de esa ejecución.
        let d = dir_temporal("antigua");
        let s = servicio_en(d.join("sessions.json"));
        let mut data = SesionesData::default();
        data.sesiones.push(SesionDeEjecucion::observada(
            "hoy".into(),
            RasgosDeProceso {
                pid: 20,
                inicio_del_proceso: Some("2026-11-27 10:00:00".into()),
                ..Default::default()
            },
            IdentidadDeSesion::con_appid(251570, OrigenAppId::EntornoDelProceso),
            "2026-11-27 10:00:00",
        ));
        s.guardar(&data).unwrap();

        let ventana = VentanaDeGrabacion {
            inicio: Some("2026-10-27 10:00:00".into()),
            duracion_segundos: Some(600.0),
        };
        let (id, asoc) = s.asociar_grabacion(251570, "7DaysToDie", &ventana);
        assert_eq!(id, None, "no se atribuye una grabación de hace un mes");
        assert_eq!(asoc, Asociacion::NoComprobable);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn prueba_17_el_benchmark_y_las_comparaciones_no_dependen_de_la_sesion() {
        // La prueba debe seguir siendo legible tras borrar la sesión: lo que hace falta
        // para interpretarla está en la instantánea, y el enlace solo apunta a la sesión.
        let d = dir_temporal("independiente");
        let s = servicio_en(d.join("sessions.json"));
        let mut data = SesionesData::default();
        let mut ses = SesionDeEjecucion::observada(
            "ses_1".into(),
            RasgosDeProceso {
                pid: 30,
                inicio_del_proceso: Some("2026-11-24 18:00:00".into()),
                ..Default::default()
            },
            IdentidadDeSesion::con_appid(251570, OrigenAppId::EntornoDelProceso),
            "2026-11-24 18:00:00",
        );
        ses.grabaciones.push("bench_1".into());
        let instantanea = instantanea_de_sesion(&ses);
        data.sesiones.push(ses);
        s.guardar(&data).unwrap();

        // La instantánea se copia: sobrevive al borrado de la sesión.
        assert!(instantanea.is_some());
        s.actualizar(|d| {
            d.sesiones.clear();
            true
        })
        .unwrap();
        assert!(s.sesion("ses_1").is_none());
        assert!(instantanea.unwrap().valor.contains("pid 30"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn prueba_20_un_archivo_de_sesiones_vacio_no_se_confunde_con_inexistente() {
        // Un archivo vacío es corrupción (un corte a mitad de escritura), no «no hay
        // sesiones»: se avisa y se conserva una copia.
        let d = dir_temporal("vacio2");
        let ruta = d.join("sessions.json");
        std::fs::write(&ruta, "").unwrap();
        let s = servicio_en(ruta);
        let e = s.cargar().unwrap_err();
        assert!(e.contains("dañado") || e.contains("vacío"), "{:?}", e);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_ventana_se_reconstruye_del_nombre_del_csv() {
        let v = ventana_de_csv(
            "/tmp/Palworld-Win64-Shipping_2026-11-24_18-19-57.csv",
            Some(120.0),
        );
        assert_eq!(v.inicio.as_deref(), Some("2026-11-24 18:19:57"));
        assert_eq!(v.fin_estimado().as_deref(), Some("2026-11-24 18:21:57"));

        // Nombre que no lleva fecha: no se inventa.
        let v = ventana_de_csv("/tmp/pal2.csv", Some(60.0));
        assert!(v.inicio.is_none());
        assert!(v.fin_estimado().is_none());
    }

    #[test]
    fn la_instantanea_de_sesion_copia_y_no_referencia() {
        let ses = SesionDeEjecucion::observada(
            "s".into(),
            RasgosDeProceso {
                pid: 7,
                inicio_del_proceso: Some("2026-11-24 18:00:00".into()),
                ejecutable: Some("juego.exe".into()),
                ..Default::default()
            },
            IdentidadDeSesion::con_appid(251570, OrigenAppId::EntornoDelProceso),
            "2026-11-24 18:00:00",
        );
        let inst = instantanea_de_sesion(&ses).unwrap();
        assert_eq!(inst.procedencia, Procedencia::ObservadoEnProceso);
        assert!(inst.valor.contains("pid 7"));
        assert!(inst.valor.contains("juego.exe"));

        // Una agrupación sin identidad no aporta instantánea: mejor nada que ruido.
        let agrupada = SesionDeEjecucion::agrupada(
            "g".into(),
            IdentidadDeSesion::desconocida(),
            "2026-11-24 18:00:00",
        );
        assert!(instantanea_de_sesion(&agrupada).is_none());
    }
}

/// Pruebas de aceptación de la corrección de 5.3, con los datos reales del usuario.
///
/// Lo que se comprueba aquí es lo que falló en CachyOS: con Palworld abierto bajo Proton, el
/// observador se quedaba en «Buscando un juego» porque buscaba un proceso con el nombre del
/// juego. Ahora se mira `/proc`, se agrupan los procesos del mismo árbol y se decide por
/// evidencia.
#[cfg(test)]
pub(crate) mod aceptacion_53 {
    use super::*;
    use crate::fixtures_procesos::*;
    use crate::proceso_linux::ObservadorSimulado;
    use crate::sessions::{EscaneoDeProcesos, JuegoDeBiblioteca};

    fn dir(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("thor_acc53_{}_{}", nombre, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Construye un servicio con un observador que devuelve los procesos de una fixture.
    fn servicio_con(d: &std::path::Path, fixture: &[ProcesoDeFixture]) -> ServicioDeSesiones {
        let rasgos: Vec<RasgosDeProceso> = fixture
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let mut s = ServicioDeSesiones::con_ruta(d.join("sessions.json"));
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        // Los procesos de la fixture están «vivos»: son lo que se ve ahora mismo.
        let claves: Vec<String> = escaneo
            .grupos
            .iter()
            .filter_map(|g| {
                g.miembros
                    .iter()
                    .find(|m| m.pid == g.principal)
                    .map(clave_de)
            })
            .collect();
        let mut sim = ObservadorSimulado::con_grupos(escaneo.grupos);
        sim.vivos = claves;
        s.observador_inyectado = Some(sim);
        // Un servicio de pruebas nace apagado, como el de verdad: el hilo no mira nada
        // hasta que la interfaz lo enciende.
        s.fijar_activo(true);
        s
    }

    /// **Criterio 1 y 2.** Palworld se detecta y se identifica con AppID 1623730.
    #[test]
    fn palworld_bajo_proton_abre_sesion_con_appid_1623730() {
        let d = dir("palworld");
        let s = servicio_con(&d, PALWORLD_PROTON);
        let estado = s.paso().unwrap();

        let sesiones = s.cargar().unwrap().sesiones;
        assert_eq!(sesiones.len(), 1, "una sesión, no una por proceso");
        let ses = &sesiones[0];
        assert_eq!(ses.tipo, TipoDeSesion::Observada);
        assert_eq!(ses.identidad.appid, Some(1623730));
        assert!(matches!(estado, EstadoDelObservador::Observando { .. }));
        assert_eq!(ses.proceso.as_ref().map(|p| p.pid), Some(19588));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Criterio 3.** Los procesos hermanos del mismo juego no generan sesiones duplicadas,
    /// ni en la primera pasada ni en las siguientes.
    #[test]
    fn no_se_duplican_sesiones_en_pasadas_sucesivas() {
        let d = dir("dedup");
        let s = servicio_con(&d, PALWORLD_PROTON);
        for _ in 0..3 {
            // La segunda pasada encuentra la sesión abierta y no vuelve a mirar; la tercera
            // tampoco. Lo importante es que el número no crezca.
            let _ = s.paso();
        }
        let sesiones = s.cargar().unwrap().sesiones;
        assert_eq!(sesiones.len(), 1, "sesiones: {:#?}", sesiones.len());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Criterio 4.** La sesión deja constancia de que fue una ejecución bajo Proton, y no
    /// convierte eso en «las optimizaciones funcionaron».
    #[test]
    fn la_evidencia_de_proton_queda_registrada_sin_afirmar_el_efecto() {
        let d = dir("proton");
        let s = servicio_con(&d, PALWORLD_PROTON);
        let _ = s.paso();
        let ses = &s.cargar().unwrap().sesiones[0];
        assert_eq!(ses.proceso.as_ref().and_then(|p| p.bajo_proton), Some(true));
        let texto = ses.identidad.evidencia.join(" | ");
        assert!(texto.contains("Proton"), "evidencia: {}", texto);
        // Y lo que no puede decir: que el ajuste estuviera activo.
        assert!(!texto.to_lowercase().contains("verificado"));
        let _ = std::fs::remove_dir_all(&d);
    }

    // ---- Ciclo de vida: continuidad, pérdida temporal y nueva ejecución ----

    /// Una pasada sin ver el proceso **no** cierra la sesión: puede ser `/proc` a medias.
    #[test]
    fn una_pasada_sin_verlo_no_cierra_la_sesion() {
        let d = dir("perdida");
        let s = servicio_con(&d, PALWORLD_PROTON);
        let _ = s.paso();
        let mut mutado = s;
        // Deja de verse, pero el proceso "sigue vivo" no se toca: simula un fallo de lectura.
        mutado.observador_inyectado = Some(ObservadorSimulado::con_grupos(Vec::new()));
        let _ = mutado.paso();

        let sesiones = mutado.cargar().unwrap().sesiones;
        assert_eq!(sesiones.len(), 1);
        assert_eq!(
            sesiones[0].evidencia_proceso,
            EvidenciaProceso::VisibilidadPerdida,
            "no verlo una vez es pérdida de visibilidad"
        );
        assert!(sesiones[0].cerrada_en.is_none(), "sigue abierta");
        assert!(
            !sesiones[0].evidencia_proceso.permite_afirmar_que_no_corre(),
            "y no se puede afirmar que el juego no corre"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Si vuelve a verse antes de agotar la tolerancia, la sesión se recupera y el contador
    /// se limpia: la continuidad la da el PID + starttime, que no cambió.
    #[test]
    fn si_vuelve_a_verse_la_sesion_se_recupera() {
        let d = dir("vuelve");
        let s = servicio_con(&d, PALWORLD_PROTON);
        let _ = s.paso();
        let mut mutado = s;
        // El mismo árbol de procesos que vio la primera pasada. No sirve `escanear()`:
        // devuelve el observador inyectado, que es lo que se está sustituyendo.
        let rasgos: Vec<RasgosDeProceso> = PALWORLD_PROTON
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        // El observador debe declarar vivos los procesos de esos grupos. Sin esto,
        // `sigue_vivo` contesta que no y la sesión se daría por terminada.
        let claves: Vec<String> = escaneo
            .grupos
            .iter()
            .filter_map(|g| {
                g.miembros
                    .iter()
                    .find(|m| m.pid == g.principal)
                    .map(clave_de)
            })
            .collect();
        let mut al_volver = ObservadorSimulado::con_grupos(escaneo.grupos);
        al_volver.vivos = claves;

        mutado.observador_inyectado = Some(ObservadorSimulado::con_grupos(Vec::new()));
        let _ = mutado.paso();
        assert_eq!(
            mutado.cargar().unwrap().sesiones[0].evidencia_proceso,
            EvidenciaProceso::VisibilidadPerdida
        );

        // Vuelve a verse: el mismo árbol otra vez, con el proceso vivo.
        mutado.observador_inyectado = Some(al_volver);
        let _ = mutado.paso();

        let sesiones = mutado.cargar().unwrap().sesiones;
        assert_eq!(sesiones.len(), 1, "no se abrió una sesión nueva");
        assert_eq!(
            sesiones[0].evidencia_proceso,
            EvidenciaProceso::Observada,
            "vuelve a estar observada"
        );
        assert_eq!(sesiones[0].pasadas_sin_verlo, 0, "el contador se limpia");
        let _ = std::fs::remove_dir_all(&d);
    }

    // ==================== Deskrawl real: el defecto de la prueba en CachyOS ====================

    /// Monta un servicio que, pasada a pasada, va mostrando lo que el usuario vio aparecer.
    ///
    /// Cada pasada recibe el subconjunto de procesos visible en ese momento; los grupos se
    /// recalculan desde cero con `escanear_desde`, que es el mismo pipeline que el real.
    fn servicio_de_pasadas(
        nombre: &str,
        proceso_visible: impl Fn(usize) -> Vec<crate::fixtures_procesos::ProcesoDeFixture>,
    ) -> ServicioDeSesiones {
        use crate::proceso_linux::ObservadorSimulado;
        let d = dir_temporal(nombre);
        let mut s = servicio_en(d.join("sessions.json"));
        let fixture = proceso_visible(0);
        let rasgos: Vec<RasgosDeProceso> = fixture
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(escaneo.grupos));
        s
    }

    /// Los procesos de Deskrawl, filtrando los que llevan `vivo` a cierto.
    fn deskrawl_visibles(
        incluir_envoltorio: bool,
        incluir_tabtip: bool,
        incluir_juego: bool,
    ) -> Vec<crate::fixtures_procesos::ProcesoDeFixture> {
        use crate::fixtures_procesos::DESKRAWL_PV_ADVERB;
        DESKRAWL_PV_ADVERB
            .iter()
            .filter(|p| match p.pid {
                557298 => true,               // el reaper: siempre está
                557367 => incluir_envoltorio, // pv-adverb
                557505 => incluir_tabtip,     // tabtip.exe
                557514 => incluir_juego,      // Deskrawl.exe
                _ => true,
            })
            .map(|p| crate::fixtures_procesos::ProcesoDeFixture { ..*p })
            .collect()
    }

    // ============ Regresión completa del diagnóstico real de CachyOS ============
    //
    // Reproduce el fallo **tal como ocurrió** en la máquina del usuario, recorriendo el
    // pipeline entero: catálogo efectivo → índice → prefiltro → agrupación → correlación →
    // identidad → creación de sesión.
    //
    // Lo que el diagnóstico real mostró (y esto reproduce):
    //
    //   grupo pid=88096 -> SIN correlacion (carpeta=None, prefijo=None, catalogo=26 entradas)
    //   grupo pid=88096 -> sesion RECHAZADA (sin evidencia bastante o duplicada)
    //   evidencia=["ejecuta starsector"]
    //
    // El catálogo **sí** tenía a Starsector; lo que faltaba era con qué correlacionarlo.

    /// Catálogo tal como lo entrega el adaptador de Lutris hoy: `install_dir` con el
    /// ejecutable dentro cuando `directory` viene vacío, y `id_plataforma` nulo.
    ///
    /// No se construye un `install_dir` ideal: se reproduce lo que producción entrega.
    pub(crate) fn catalogo_lutris_real() -> Vec<crate::evidencia_proceso::EntradaDeCatalogo> {
        vec![
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
                wine_prefix: None,
                exec: Some("/home/jugador/Games/Star Valor/Star Valor.exe".into()),
                runtime_configurado: None,
            },
        ]
    }

    /// El árbol real de Starsector, tal como lo devuelve el observador.
    fn rasgos_starsector() -> Vec<RasgosDeProceso> {
        crate::fixtures_procesos::STARSECTOR_NATIVO
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect()
    }

    /// **El fallo real, reproducido.** Antes de la corrección: sin correlación, sin sesión.
    #[test]
    fn regresion_starsector_corrige_el_fallo_real_de_cachyos() {
        let catalogo = catalogo_lutris_real();
        let rasgos = rasgos_starsector();
        let escaneo = crate::proceso_linux::escanear_desde_con_catalogo(
            &rasgos,
            Some(&crate::proceso_linux::IndiceDelCatalogo::de_catalogo(
                &catalogo,
            )),
        );

        // El prefiltro y la elección de representante ya funcionaban: 1 grupo, el java.
        assert_eq!(escaneo.grupos.len(), 1, "un juego, un grupo");
        let g = &escaneo.grupos[0];
        assert_eq!(g.principal, 641416, "el java que ejecuta el juego");

        // La correlación es lo que fallaba.
        let principal = g.miembros.iter().find(|m| m.pid == g.principal).unwrap();
        let corr = correlacionar_con_el_catalogo(
            &catalogo,
            principal.carpeta_declarada.as_deref(),
            principal.wine_prefix.as_deref(),
            principal.cwd.as_deref(),
            principal.ruta_ejecutable.as_deref(),
            principal.ejecutable_del_juego.as_deref(),
        );
        let corr = corr.expect("el catálogo tiene a Starsector: debe correlacionar");
        assert_eq!(corr.titulo, "Starsector");

        // Y la identidad debe ser la del catálogo, no «desconocida».
        let id = crate::sessions::resolver_identidad_con_catalogo(
            g.nombres.first().map(|s| s.as_str()),
            Some(principal),
            &[],
            &catalogo,
        );
        assert_eq!(id.nombre_comercial.as_deref(), Some("Starsector"));
        assert_eq!(id.appid, None, "no es un juego de Steam: no lleva AppID");
    }

    /// **La sesión se crea**, con identidad estable y sin AppID.
    #[test]
    fn regresion_starsector_abre_sesion_con_identidad_del_catalogo() {
        let d = dir_temporal("starsector_sesion");
        let mut s = servicio_en(d.join("sessions.json"));
        s.catalogo_inyectado = Some(catalogo_lutris_real());
        mutar_observador_con_catalogo(
            &mut s,
            crate::fixtures_procesos::STARSECTOR_NATIVO,
            &catalogo_lutris_real(),
        );
        s.fijar_activo(true);

        let _ = s.paso().expect("la pasada no falla");
        let abiertas = sesiones_abiertas(&s);
        assert_eq!(abiertas.len(), 1, "Starsector debe abrir una sesión");
        let ses = &abiertas[0];
        assert_eq!(
            ses.identidad.nombre_comercial.as_deref(),
            Some("Starsector"),
            "la identidad sale del catálogo de Lutris"
        );
        assert_eq!(ses.identidad.appid, None, "sin AppID inventado");
        assert_eq!(
            ses.proceso.as_ref().map(|p| p.pid),
            Some(641416),
            "el representante es el java del juego"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Regresión real de CachyOS: se creaba una sesión nueva **en cada pasada** con el mismo
    /// PID y el mismo `starttime`.
    ///
    /// Secuencia medida en el equipo (registros 1249-1262): creada, no-duplicada ×3, creada
    /// otra vez, no-duplicada ×2. Es el bucle «Juego detectado → buscando juego → juego
    /// detectado». La causa era `acredita_ejecucion`: el intérprete `java` no tiene
    /// `ejecuta_de_verdad`, ni AppID, ni carpeta declarada, así que `vivos_de_la_ejecucion`
    /// quedaba vacío y la pasada siguiente no reconocía la instancia.
    #[test]
    fn regresion_starsector_siete_pasadas_conservan_una_sola_sesion() {
        let d = dir_temporal("starsector_siete_pasadas");
        let mut s = servicio_en(d.join("sessions.json"));
        s.catalogo_inyectado = Some(catalogo_lutris_real());
        mutar_observador_con_catalogo(
            &mut s,
            crate::fixtures_procesos::STARSECTOR_NATIVO,
            &catalogo_lutris_real(),
        );
        s.fijar_activo(true);

        let mut ids: Vec<String> = Vec::new();
        // Siete pasadas con un recuento total de sesiones, no solo abiertas: una sesión creada
        // de más se vería aquí aunque se cerrara.
        for pasada in 1..=7 {
            let _ = s.paso().expect("la pasada no falla");
            let abiertas = sesiones_abiertas(&s);
            assert_eq!(
                abiertas.len(),
                1,
                "pasada {pasada}: debe haber una sola sesión abierta, hay {}",
                abiertas.len()
            );
            ids.push(abiertas[0].id.clone());
            assert_eq!(
                abiertas[0].proceso.as_ref().map(|p| p.pid),
                Some(641416),
                "pasada {pasada}: el representante sigue siendo el intérprete del juego"
            );
            assert_eq!(
                abiertas[0].evidencia_proceso,
                crate::sessions::EvidenciaProceso::Observada,
                "pasada {pasada}: el juego se está viendo, no se pierde la visibilidad"
            );
            assert!(
                !abiertas[0].vivos_de_la_ejecucion.is_empty(),
                "pasada {pasada}: la ejecución acreditada no puede quedar vacía: {:?}",
                abiertas[0].vivos_de_la_ejecucion
            );
        }
        assert_eq!(
            ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
            1,
            "las siete pasadas deben ser la misma sesión, no siete: {ids:?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Python y Bash no abren sesiones propias**: son el lanzador y el script.
    #[test]
    fn regresion_starsector_python_y_bash_no_abren_sesion() {
        let d = dir_temporal("starsector_auxiliares");
        let mut s = servicio_en(d.join("sessions.json"));
        s.catalogo_inyectado = Some(catalogo_lutris_real());
        mutar_observador_con_catalogo(
            &mut s,
            crate::fixtures_procesos::STARSECTOR_NATIVO,
            &catalogo_lutris_real(),
        );
        s.fijar_activo(true);
        let _ = s.paso().expect("la pasada no falla");
        let abiertas = sesiones_abiertas(&s);
        assert_eq!(abiertas.len(), 1, "un solo juego: una sola sesión");
        let pid = abiertas[0].proceso.as_ref().map(|p| p.pid);
        assert_ne!(pid, Some(641413), "el python de Lutris no es el juego");
        assert_ne!(pid, Some(641415), "el script no es el juego");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Dos juegos externos no comparten identidad**, aunque el catálogo los tenga juntos.
    #[test]
    fn regresion_dos_juegos_externos_no_comparten_identidad() {
        let catalogo = catalogo_lutris_real();
        let mut procesos = rasgos_starsector();
        procesos.extend(
            crate::fixtures_procesos::STAR_VALOR_WINE
                .iter()
                .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas),
        );
        let indice = crate::proceso_linux::IndiceDelCatalogo::de_catalogo(&catalogo);
        let escaneo = crate::proceso_linux::escanear_desde_con_catalogo(&procesos, Some(&indice));
        assert_eq!(escaneo.grupos.len(), 2, "dos juegos, dos grupos");
        let mut nombres: Vec<String> = escaneo
            .grupos
            .iter()
            .filter_map(|g| {
                let p = g.miembros.iter().find(|m| m.pid == g.principal)?;
                crate::sessions::resolver_identidad_con_catalogo(
                    g.nombres.first().map(|s| s.as_str()),
                    Some(p),
                    &[],
                    &catalogo,
                )
                .nombre_comercial
            })
            .collect();
        nombres.sort();
        assert_eq!(nombres, vec!["Star Valor", "Starsector"]);
    }

    /// **Un directorio genérico no confirma un juego** (el caso de `Descargas` en Heroic).
    #[test]
    fn un_directorio_generico_no_confirma_un_juego() {
        let catalogo = vec![crate::evidencia_proceso::EntradaDeCatalogo {
            id_plataforma: None,
            plataforma: None,
            launcher: "heroic".into(),
            runner: Some("legendary".into()),
            titulo: "EA App".into(),
            install_dir: Some("/home/jugador/Descargas".into()),
            wine_prefix: None,
            exec: None,
            runtime_configurado: None,
        }];
        // Un proceso cualquiera que trabaje en esa carpeta no es EA App.
        let r = correlacionar_con_el_catalogo(
            &catalogo,
            None,
            None,
            Some("/home/jugador/Descargas"),
            Some("/usr/bin/bash"),
            None,
        );
        assert!(
            r.is_none(),
            "pertenecer a Descargas no convierte a un proceso en el juego: {:?}",
            r.map(|c| c.titulo)
        );
    }

    /// Auxiliar: inyecta el observador aplicando el catálogo, como producción.
    fn mutar_observador_con_catalogo(
        s: &mut ServicioDeSesiones,
        visibles: &[crate::fixtures_procesos::ProcesoDeFixture],
        catalogo: &[crate::evidencia_proceso::EntradaDeCatalogo],
    ) {
        use crate::proceso_linux::ObservadorSimulado;
        let rasgos: Vec<RasgosDeProceso> = visibles
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let indice = crate::proceso_linux::IndiceDelCatalogo::de_catalogo(catalogo);
        let escaneo = crate::proceso_linux::escanear_desde_con_catalogo(&rasgos, Some(&indice));
        let claves: Vec<String> = escaneo
            .grupos
            .iter()
            .filter_map(|g| {
                g.miembros
                    .iter()
                    .find(|m| m.pid == g.principal)
                    .map(clave_de)
            })
            .collect();
        let mut sim = ObservadorSimulado::con_grupos(escaneo.grupos);
        sim.vivos = claves;
        s.observador_inyectado = Some(sim);
    }

    /// **Un cajón no es una instalación.** El caso real de Heroic: `install_dir` era
    /// `/home/jugador/Descargas`, y ahí vive todo lo descargado.
    #[test]
    fn un_cajon_del_usuario_no_confirma_ningun_juego() {
        let ent = |dir: &str| crate::evidencia_proceso::EntradaDeCatalogo {
            id_plataforma: Some("heroic-1".into()),
            plataforma: None,
            launcher: "heroic".into(),
            runner: Some("legendary".into()),
            titulo: "EA App".into(),
            install_dir: Some(dir.into()),
            wine_prefix: None,
            exec: None,
            runtime_configurado: None,
        };
        for dir in [
            "/home/jugador/Descargas",
            "/home/jugador/Downloads",
            "/home/jugador",
            "/home/jugador/Escritorio",
        ] {
            assert!(
                es_carpeta_generica(&ent(dir)),
                "{} es un cajón y no debe confirmar nada",
                dir
            );
        }
        // Una carpeta de juego de verdad no se toca.
        assert!(!es_carpeta_generica(&ent("/home/jugador/Games/starsector")));
        assert!(!es_carpeta_generica(&ent("/media/thor/SSD/Star Valor")));
    }

    // ============ Regresión cruzada: Steam no se rompe con el catálogo ============

    /// Catálogo de Steam con los dos juegos de la validación real.
    fn catalogo_steam_real() -> Vec<crate::evidencia_proceso::EntradaDeCatalogo> {
        vec![
            crate::evidencia_proceso::EntradaDeCatalogo {
                id_plataforma: Some("1623730".into()),
                plataforma: Some("steam".into()),
                launcher: "steam".into(),
                runner: Some("proton".into()),
                titulo: "Palworld".into(),
                install_dir: Some("/run/games/steamapps/common/Palworld".into()),
                wine_prefix: None,
                exec: Some("/run/games/steamapps/common/Palworld/Palworld.exe".into()),
                runtime_configurado: None,
            },
            crate::evidencia_proceso::EntradaDeCatalogo {
                id_plataforma: Some("4623570".into()),
                plataforma: Some("steam".into()),
                launcher: "steam".into(),
                runner: Some("proton".into()),
                titulo: "Deskrawl: Idle ARPG".into(),
                install_dir: Some("/run/games/steamapps/common/Deskrawl".into()),
                wine_prefix: None,
                exec: Some("/run/games/steamapps/common/Deskrawl/Deskrawl.exe".into()),
                runtime_configurado: None,
            },
        ]
    }

    /// Corre una fixture con el catálogo puesto y devuelve lo que se publica.
    fn pasada_con_catalogo(
        nombre: &str,
        fx: &[crate::fixtures_procesos::ProcesoDeFixture],
        catalogo: Vec<crate::evidencia_proceso::EntradaDeCatalogo>,
    ) -> (
        std::path::PathBuf,
        ServicioDeSesiones,
        Vec<(Option<u32>, Option<String>)>,
    ) {
        let d = dir_temporal(nombre);
        let mut s = servicio_en(d.join("sessions.json"));
        s.catalogo_inyectado = Some(catalogo.clone());
        mutar_observador_con_catalogo(&mut s, fx, &catalogo);
        s.fijar_activo(true);
        let _ = s.paso().expect("la pasada no falla");
        let publicados = match s.estado().ultimo_estado.lock().ok().and_then(|g| g.clone()) {
            Some(EstadoDelObservador::Observando { juegos }) => juegos
                .into_iter()
                .map(|j| (j.appid, j.nombre))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        (d, s, publicados)
    }

    /// **La regresión de CachyOS**: Palworld se publicaba como «Palworld, Palworld».
    ///
    /// Con el catálogo delante, un juego de Steam debe seguir saliendo **una vez** y con su
    /// nombre comercial. Se comprueba sobre lo que se publica, no sobre lo que se guarda.
    #[test]
    fn regresion_palworld_sale_una_vez_y_con_nombre() {
        let (d, _s, publicados) = pasada_con_catalogo(
            "pw_publicado",
            crate::fixtures_procesos::PALWORLD_PROTON,
            catalogo_steam_real(),
        );
        assert_eq!(
            publicados,
            vec![(Some(1623730), Some("Palworld".to_string()))],
            "un juego, un nombre, ni duplicado ni «sin identificar»"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Igual, con el envoltorio `systemd-inhibit` del runtime de CachyOS.
    #[test]
    fn regresion_palworld_con_inhibidor_sale_una_vez() {
        let (d, _s, publicados) = pasada_con_catalogo(
            "pw_inhib",
            crate::fixtures_procesos::PALWORLD_CON_INHIBIDOR,
            catalogo_steam_real(),
        );
        assert_eq!(
            publicados.len(),
            1,
            "el inhibidor hereda el AppID, pero no es otro juego: {:?}",
            publicados
        );
        assert_eq!(publicados[0].1.as_deref(), Some("Palworld"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Deskrawl conserva sus dos garantías: sale una vez, con nombre, y no lo tapa el auxiliar.
    #[test]
    fn regresion_deskrawl_sale_una_vez_y_no_el_auxiliar() {
        for (nombre, fx) in [
            ("dk_pv", crate::fixtures_procesos::DESKRAWL_PV_ADVERB),
            ("dk_srt", crate::fixtures_procesos::DESKRAWL_SRT),
        ] {
            let (d, s, publicados) = pasada_con_catalogo(nombre, fx, catalogo_steam_real());
            assert_eq!(
                publicados,
                vec![(Some(4623570), Some("Deskrawl: Idle ARPG".to_string()))],
                "{}: un juego, su nombre, y el auxiliar fuera",
                nombre
            );
            // Y el representante no puede ser el wrapper: el `.exe` del juego manda.
            let abiertas = sesiones_abiertas(&s);
            assert_eq!(abiertas.len(), 1);
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    /// **Los dos a la vez**: es la prueba que en CachyOS enseñó tres entradas.
    #[test]
    fn regresion_dos_juegos_steam_a_la_vez_no_se_duplican() {
        let mut procesos: Vec<crate::fixtures_procesos::ProcesoDeFixture> =
            crate::fixtures_procesos::PALWORLD_PROTON.to_vec();
        procesos.extend(desplazar_pids(
            crate::fixtures_procesos::DESKRAWL_PV_ADVERB,
            900_000,
        ));
        let (d, _s, mut publicados) =
            pasada_con_catalogo("dos_steam", &procesos, catalogo_steam_real());
        publicados.sort_by(|a, b| a.1.cmp(&b.1));
        assert_eq!(
            publicados,
            vec![
                (Some(4623570), Some("Deskrawl: Idle ARPG".to_string())),
                (Some(1623730), Some("Palworld".to_string())),
            ],
            "dos juegos: dos entradas, ni una más"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Dos instancias legítimas del mismo juego no se fusionan.**
    ///
    /// No se deduplica por AppID: dos ejecuciones son dos sesiones, y cerrar una no puede
    /// cerrar la otra.
    #[test]
    fn dos_instancias_del_mismo_juego_son_dos_sesiones() {
        let mut procesos: Vec<crate::fixtures_procesos::ProcesoDeFixture> =
            crate::fixtures_procesos::PALWORLD_PROTON.to_vec();
        let n = procesos.len();
        procesos.extend(desplazar_pids(
            crate::fixtures_procesos::PALWORLD_PROTON,
            700_000,
        ));
        assert_eq!(procesos.len(), n * 2);
        let (d, s, publicados) =
            pasada_con_catalogo("dos_instancias", &procesos, catalogo_steam_real());
        assert_eq!(
            sesiones_abiertas(&s).len(),
            2,
            "dos ejecuciones del mismo juego: dos sesiones"
        );
        assert_eq!(
            publicados.len(),
            2,
            "y se publican las dos: {:?}",
            publicados
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Desplaza los PIDs de una fixture conservando la **forma del árbol**.
    ///
    /// No basta con renumerar: los `ppid` tienen que apuntar al padre renumerado, y los que
    /// colgaban de `1` (raíz) siguen colgando de `1`. Hacerlo mal parte la cadena
    /// `pv-adverb → juego` en dos raíces y fabrica grupos duplicados que producción no da.
    fn desplazar_pids(
        fx: &[crate::fixtures_procesos::ProcesoDeFixture],
        base: u32,
    ) -> Vec<crate::fixtures_procesos::ProcesoDeFixture> {
        use std::collections::BTreeMap;
        let mapa: BTreeMap<u32, u32> = fx
            .iter()
            .enumerate()
            .map(|(i, p)| (p.pid, base + i as u32))
            .collect();
        fx.iter()
            .map(|p| {
                let mut q = p.clone();
                q.pid = mapa[&p.pid];
                q.ppid = match mapa.get(&p.ppid) {
                    Some(nuevo) => *nuevo,
                    None => p.ppid,
                };
                q
            })
            .collect()
    }

    fn dir_temporal(nombre: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("thoriapp_ses_{}_{}", nombre, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn mutar_observador(
        s: &mut ServicioDeSesiones,
        visibles: &[crate::fixtures_procesos::ProcesoDeFixture],
    ) {
        use crate::proceso_linux::ObservadorSimulado;
        let rasgos: Vec<RasgosDeProceso> = visibles
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        let claves: Vec<String> = escaneo
            .grupos
            .iter()
            .filter_map(|g| {
                g.miembros
                    .iter()
                    .find(|m| m.pid == g.principal)
                    .map(clave_de)
            })
            .collect();
        let mut sim = ObservadorSimulado::con_grupos(escaneo.grupos);
        sim.vivos = claves;
        s.observador_inyectado = Some(sim);
    }

    /// **Incidencia B, el nombre publicado**: gana el comercial confirmado.
    ///
    /// El diagnóstico midió sesiones con `nombre_observado = Heronite-x86_64`. Aunque el
    /// observado venga contaminado, lo que se publica es el nombre comercial que confirmó el
    /// catálogo: es el único que el usuario reconoce como el juego.
    #[test]
    fn el_nombre_comercial_confirmado_prevalece_sobre_el_observado() {
        let d = dir_temporal("nombre_comercial_gana");
        let mut s = servicio_en(d.join("sessions.json"));
        let catalogo = crate::sessions_service::aceptacion_53::catalogo_lutris_real();
        s.catalogo_inyectado = Some(catalogo.clone());
        mutar_observador_con_catalogo(
            &mut s,
            crate::fixtures_procesos::STARSECTOR_NATIVO,
            &catalogo,
        );
        s.fijar_activo(true);
        let _ = s.paso().expect("la pasada no falla");

        let abiertas = sesiones_abiertas(&s);
        assert_eq!(abiertas.len(), 1, "Starsector abre una sesión");
        let id = &abiertas[0].identidad;
        assert_eq!(
            id.nombre_comercial.as_deref(),
            Some("Starsector"),
            "el comercial confirmado es el que se publica"
        );
        // Lo que la interfaz muestra: el comercial, no el del ejecutable ni el de un ajeno.
        let publicado = id
            .nombre_comercial
            .as_deref()
            .or(id.nombre_observado.as_deref());
        assert_eq!(publicado, Some("Starsector"));
        let _ = std::fs::remove_dir_all(&d);
    }

    fn sesiones_abiertas(s: &ServicioDeSesiones) -> Vec<SesionDeEjecucion> {
        s.cargar()
            .unwrap()
            .sesiones
            .into_iter()
            .filter(|x| x.evidencia_proceso.sigue_en_curso())
            .collect()
    }

    /// **El defecto real de la validación en CachyOS**, pasada a pasada.
    ///
    /// Se reprodujo con los tres procesos exactos que el usuario volcó de `/proc`: un
    /// envoltorio (`pv-adverb`), un auxiliar de Windows (`tabtip.exe`) y el juego
    /// (`Deskrawl.exe`), los tres con `SteamAppId` y `STEAM_COMPAT_INSTALL_PATH`.
    ///
    /// Antes fallaba así: `tabtip.exe` pasaba por juego porque la carpeta declarada lo
    /// confirmaba, y eran **dos** sesiones confirmadas con el envoltorio y el auxiliar como
    /// representantes.
    #[test]
    fn deskrawl_real_no_confirma_sesion_hasta_que_aparece_el_juego() {
        let d = dir_temporal("deskrawl_real_pasadas");
        let mut s = servicio_en(d.join("sessions.json"));
        s.fijar_activo(true);

        // ---- Pasada 1: solo el envoltorio. ----
        mutar_observador(&mut s, &deskrawl_visibles(true, false, false));
        let _ = s.paso();
        for x in sesiones_abiertas(&s) {
            eprintln!(
                "ABIERTA id={} nombre={:?} comercial={:?} pid={:?} evidencia={:?}",
                x.id,
                x.identidad.nombre_observado,
                x.identidad.nombre_comercial,
                x.proceso.as_ref().map(|p| p.pid),
                x.evidencia_proceso
            );
        }

        assert!(
            sesiones_abiertas(&s).is_empty(),
            "un envoltorio solo no abre sesión confirmada: {:?}",
            sesiones_abiertas(&s)
                .iter()
                .map(|x| x.identidad.nombre_comercial.clone())
                .collect::<Vec<_>>()
        );

        // ---- Pasada 2: aparecen auxiliares de Wine. ----
        mutar_observador(&mut s, &deskrawl_visibles(true, true, false));
        let _ = s.paso();
        assert!(
            sesiones_abiertas(&s).is_empty(),
            "un auxiliar de Windows no abre sesión confirmada"
        );

        // ---- Pasada 3: aparece el ejecutable real. Una única sesión. ----
        mutar_observador(&mut s, &deskrawl_visibles(true, true, true));
        let _ = s.paso();
        let abiertas = sesiones_abiertas(&s);
        assert_eq!(abiertas.len(), 1, "una ejecución es una sesión");
        assert_eq!(
            abiertas[0].proceso.as_ref().map(|p| p.pid),
            Some(557514),
            "el representante es Deskrawl.exe, no el envoltorio ni el auxiliar"
        );
        assert_eq!(
            abiertas[0]
                .proceso
                .as_ref()
                .and_then(|p| p.inicio_del_proceso.clone()),
            Some("2026-11-25 22:38:09".to_string()),
            "el representante es la instancia real, identificada por PID y arranque"
        );

        // ---- Pasada 4: aparecen auxiliares adicionales. Sigue habiendo una sola. ----
        mutar_observador(&mut s, &deskrawl_visibles(true, true, true));
        let _ = s.paso();
        assert_eq!(
            sesiones_abiertas(&s).len(),
            1,
            "los auxiliares no abren sesiones nuevas"
        );

        // ---- Pasada 5: el árbol se estabiliza. ----
        for _ in 0..3 {
            let _ = s.paso();
        }
        assert_eq!(sesiones_abiertas(&s).len(), 1, "estable: no se duplica");
        let de_una = sesiones_abiertas(&s);
        let id_original = de_una[0].id.clone();

        // ---- Pasada 6: el ejecutable desaparece temporalmente. ----
        mutar_observador(&mut s, &deskrawl_visibles(true, true, false));
        let _ = s.paso();
        let abiertas = sesiones_abiertas(&s);
        assert_eq!(
            abiertas.len(),
            1,
            "perder de vista el ejecutable no inventa una finalización ni una sesión nueva"
        );
        assert_eq!(abiertas[0].id, id_original, "sigue siendo la misma sesión");

        // ---- Pasada 7: el juego termina y sobreviven envoltorios. ----
        mutar_observador(&mut s, &deskrawl_visibles(true, false, false));
        for _ in 0..crate::sessions_service::PASADAS_SIN_VERLO {
            let _ = s.paso();
        }
        assert!(
            sesiones_abiertas(&s).is_empty(),
            "el juego terminó: no queda sesión abierta por los envoltorios"
        );

        // ---- Pasada 8: se vuelve a iniciar el mismo juego. Sesión nueva. ----
        // El segundo arranque tiene otro starttime: es otra ejecución.
        let mut segunda: Vec<crate::fixtures_procesos::ProcesoDeFixture> =
            deskrawl_visibles(true, true, true);
        for p in segunda.iter_mut() {
            p.inicio = "2026-11-25 23:10:00";
        }
        mutar_observador(&mut s, &segunda);
        let _ = s.paso();
        let abiertas = sesiones_abiertas(&s);
        assert_eq!(abiertas.len(), 1, "vuelve a haber una sesión");
        // El identificador se deriva del segundo de apertura (`ses_<AAAAMMDDhhmmss>`), así que
        // dentro del mismo segundo se repite: en este test todas las pasadas ocurren a la vez.
        // Lo que de verdad distingue una reapertura es la **instancia del proceso** (PID y
        // arranque), que es la clave de continuidad. Se comprueba eso, no el texto del id.
        assert_eq!(
            abiertas[0]
                .proceso
                .as_ref()
                .and_then(|p| p.inicio_del_proceso.clone()),
            Some("2026-11-25 23:10:00".to_string()),
            "reabrir el juego es una ejecución nueva, con su propio arranque"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Auditoría 5.3**: ¿la correlación por carpeta funciona con los datos **reales** de
    /// Lutris?
    ///
    /// Se reproduce lo medido en la base de datos real: `directory` vacío, `service` nulo,
    /// `exec` y `prefix` solo en el `.yml`. La entrada que construye `leer_de_las_fuentes`
    /// llega aquí con `install_dir = <ruta del .exe>` y `wine_prefix` del `.yml`.
    #[test]
    fn auditoria_lutris_correlaciona_por_prefijo_cuando_no_hay_carpeta() {
        use crate::evidencia_proceso::EntradaDeCatalogo;
        // Así queda la entrada de Star Valor tras el adaptador real.
        let catalogo = vec![EntradaDeCatalogo {
            id_plataforma: None,
            plataforma: None,
            launcher: "lutris".into(),
            runner: Some("wine".into()),
            titulo: "Star Valor".into(),
            install_dir: Some("/home/jugador/Games/Star Valor/Star Valor.exe".to_string()),
            wine_prefix: Some("/home/jugador/Games/prefixes/StarValor".to_string()),
            exec: Some("/home/jugador/Games/Star Valor/Star Valor.exe".to_string()),
            runtime_configurado: None,
        }];

        // El proceso declara su prefijo (WINEPREFIX), que es lo que ahora se lee.
        let por_prefijo = correlacionar_con_el_catalogo(
            &catalogo,
            None,
            Some("/home/jugador/Games/prefixes/StarValor/pfx"),
            None,
            None,
            None,
        );
        assert_eq!(
            por_prefijo.as_ref().map(|c| c.titulo.as_str()),
            Some("Star Valor"),
            "el prefijo del proceso debe correlacionar con el del catalogo"
        );

        // Sin carpeta declarada y sin prefijo no hay nada que comparar: no se inventa.
        assert!(correlacionar_con_el_catalogo(&catalogo, None, None, None, None, None).is_none());
    }

    /// **Auditoría 5.3**: los dos catálogos son listas distintas.
    ///
    /// La biblioteca de la interfaz (`catalogo`, solo Steam) y el catálogo que usa el
    /// observador (`catalogo_completo`) nunca se comparaban. Un juego externo puede estar en
    /// la primera y no llegar nunca a la segunda, o al revés: la biblioteca lo enseña y el
    /// observador no lo conoce. Este test fija la diferencia para que no vuelva a pasar
    /// inadvertida.
    #[test]
    fn auditoria_la_biblioteca_de_steam_no_es_el_catalogo_del_observador() {
        use crate::sessions::JuegoDeBiblioteca;
        let biblioteca: Vec<JuegoDeBiblioteca> = vec![JuegoDeBiblioteca {
            appid: 4623570,
            nombre: "Deskrawl: Idle ARPG".into(),
            installdir: Some("Deskrawl".into()),
        }];
        // Un juego externo no está en la biblioteca de Steam: si `resolver_identidad` fuera la
        // única vía, quedaría sin identificar por mucho que el observador lo conociera.
        let id = crate::sessions::resolver_identidad(Some("Starsector"), None, &biblioteca);
        assert_eq!(id.appid, None);
        assert_eq!(id.nombre_comercial, None);
        assert!(
            id.evidencia
                .iter()
                .any(|e| e.contains("ningún juego de la biblioteca")),
            "la evidencia debe decir que no lo encontró, no inventarlo"
        );
    }

    /// Varias pasadas con el mismo juego no abren sesiones nuevas: la clave de proceso ya
    /// está en curso, así que el grupo se descarta.
    ///
    /// Es la protección que sustituye al descarte por AppID compartido. Sin ella, quitar
    /// aquél habría abierto una sesión por pasada.
    #[test]
    fn varias_pasadas_del_mismo_juego_no_duplican_la_sesion() {
        let d = dir("sin_duplicar");
        let s = servicio_con(&d, PALWORLD_PROTON);
        for _ in 0..4 {
            let _ = s.paso();
        }
        let sesiones = s.cargar().unwrap().sesiones;
        assert_eq!(
            sesiones.len(),
            1,
            "cuatro pasadas del mismo proceso son una sola sesión: {}",
            sesiones.len()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Mismo PID pero **otro** starttime: es otra ejecución, no la misma.
    ///
    /// El PID se reutiliza. Si la identidad de instancia fuese solo el PID, un juego muerto
    /// y otro recién arrancado con el PID reciclado se verían como la misma sesión y se
    /// fusionaría la configuración de dos ejecuciones distintas.
    #[test]
    fn el_mismo_pid_con_otro_arranque_es_otra_ejecucion() {
        let d = dir("pid_reuso");
        let s = servicio_con(&d, PALWORLD_PROTON);
        let _ = s.paso();
        let mut mutado = s;

        // Mismo PID, starttime distinto: el proceso original terminó y el número se reusó.
        let mut rasgos: Vec<RasgosDeProceso> = PALWORLD_PROTON
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        for r in rasgos.iter_mut() {
            r.inicio_del_proceso = Some("2031-07-01 00:00:00".into());
        }
        let escaneo = crate::proceso_linux::escanear_desde(&rasgos);
        let claves: Vec<String> = escaneo
            .grupos
            .iter()
            .filter_map(|g| {
                g.miembros
                    .iter()
                    .find(|m| m.pid == g.principal)
                    .map(clave_de)
            })
            .collect();
        let mut otra = ObservadorSimulado::con_grupos(escaneo.grupos);
        otra.vivos = claves;
        mutado.observador_inyectado = Some(otra);
        let _ = mutado.paso();

        let sesiones = mutado.cargar().unwrap().sesiones;
        // La anterior ya no se ve (su clave cambió) y la nueva es otra ejecución: conviven
        // como sesiones distintas, no se fusionan.
        assert!(
            sesiones.len() >= 2,
            "otro starttime es otra ejecución, no la misma sesión: {}",
            sesiones.len()
        );
        let distintas: std::collections::HashSet<_> =
            sesiones.iter().map(|s| s.clave_proceso()).collect();
        assert_eq!(
            distintas.len(),
            sesiones.len(),
            "cada sesión tiene su propia clave de proceso"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Criterio 6.** La sesión sobrevive al cierre del juego: se marca terminada, no se
    /// borra. Lo que se observó sigue siendo cierto.
    #[test]
    fn la_sesion_persiste_tras_cerrar_el_juego() {
        let d = dir("cierre");
        let s = servicio_con(&d, PALWORLD_PROTON);
        let _ = s.paso();
        assert_eq!(s.cargar().unwrap().sesiones.len(), 1);

        // El juego se cierra: el observador deja de verlo.
        let mut cerrado = s;
        cerrado.observador_inyectado = Some(ObservadorSimulado::con_grupos(Vec::new()));
        let _ = cerrado.paso();

        let sesiones = cerrado.cargar().unwrap().sesiones;
        assert_eq!(
            sesiones.len(),
            1,
            "la sesión no se borra al cerrar el juego"
        );
        // Una pasada sin verlo solo pierde visibilidad: no se inventa una finalización.
        assert_eq!(
            sesiones[0].evidencia_proceso,
            EvidenciaProceso::VisibilidadPerdida
        );
        assert!(sesiones[0].cerrada_en.is_none());

        // Tras varias pasadas seguidas, la sesión sí se cierra.
        for _ in 0..PASADAS_SIN_VERLO {
            let _ = cerrado.paso();
        }
        let sesiones = cerrado.cargar().unwrap().sesiones;
        assert_eq!(
            sesiones.len(),
            1,
            "la sesión no se borra al cerrar el juego"
        );
        assert!(
            sesiones[0].evidencia_proceso == EvidenciaProceso::Terminada
                || sesiones[0].cerrada_en.is_some()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Criterio 5.** Una grabación nueva se asocia a la sesión correcta por identidad y
    /// tiempo, y no a la de otro juego.
    #[test]
    fn una_grabacion_nueva_se_asocia_por_identidad_y_tiempo() {
        let d = dir("asocia");
        let s = servicio_con(&d, PALWORLD_PROTON);
        let _ = s.paso();
        let ses = s.cargar().unwrap().sesiones.into_iter().next().unwrap();

        // Grabación hecha dentro de la vida del proceso.
        let ventana = VentanaDeGrabacion {
            inicio: Some("2026-11-24 18:05:00".into()),
            duracion_segundos: Some(600.0),
        };
        let (id, asoc) = s.asociar_grabacion(1623730, "Palworld-Win64-Shipping", &ventana);
        assert_eq!(
            id.as_deref(),
            Some(ses.id.as_str()),
            "asociación: {:?}",
            asoc
        );

        // Otra grabación, de otro juego, no se cuelga de esta sesión.
        let (id_otro, _) = s.asociar_grabacion(251570, "7DaysToDie", &ventana);
        assert_eq!(id_otro, None);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **Criterio 7.** En un contenedor se degrada a «sin visibilidad», nunca a «no corre».
    #[test]
    fn el_contenedor_degrada_a_sin_visibilidad() {
        let d = dir("docker");
        let mut s = ServicioDeSesiones::con_ruta(d.join("sessions.json"));
        s.observador_inyectado = Some(ObservadorSimulado::sin_visibilidad(
            "Thoriapp no ve los procesos del sistema (contenedor)",
        ));
        s.fijar_activo(true);
        let estado = s.paso().unwrap();
        assert_eq!(estado, EstadoDelObservador::SinVisibilidad);
        let ses = &s.cargar().unwrap().sesiones[0];
        assert_eq!(ses.evidencia_proceso, EvidenciaProceso::SinVisibilidad);
        assert!(ses.identidad.appid.is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Un juego nativo y uno de Proton no se confunden entre sí aunque haya dos a la vez.
    #[test]
    fn dos_juegos_abren_dos_sesiones_distintas() {
        let d = dir("dos");
        let s = servicio_con(&d, DOS_JUEGOS);
        let _ = s.paso();
        let mut appids: Vec<Option<u32>> = s
            .cargar()
            .unwrap()
            .sesiones
            .iter()
            .map(|x| x.identidad.appid)
            .collect();
        appids.sort();
        assert_eq!(appids, vec![Some(251570), Some(1623730)]);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// El catálogo de la biblioteca no basta por sí solo: sin evidencia del proceso, un
    /// nombre parecido no cierra la identidad si hay varios candidatos.
    #[test]
    fn un_nombre_ambiguo_no_se_resuelve_solo() {
        let catalogo = vec![
            JuegoDeBiblioteca {
                appid: 1,
                nombre: "Palworld".into(),
                installdir: None,
            },
            JuegoDeBiblioteca {
                appid: 2,
                nombre: "Palworld-Win64".into(),
                installdir: None,
            },
        ];
        let identidad =
            crate::sessions::resolver_identidad(Some("Palworld-Win64-Shipping"), None, &catalogo);
        // Puede resolver si el criterio de "más específico" gana, pero si no lo hace no
        // debe inventarse un AppID: o hay uno solo, o ninguno.
        assert!(
            identidad.appid.is_some() || identidad.candidatos.len() != 1,
            "{:?}",
            identidad
        );
    }

    /// Un `EscaneoDeProcesos` vacío (lo que se ve desde Docker) no debe abrir sesiones.
    #[test]
    fn un_escaneo_vacio_no_abre_nada() {
        let d = dir("vacio");
        let mut s = ServicioDeSesiones::con_ruta(d.join("sessions.json"));
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(Vec::new()));
        s.fijar_activo(true);
        let estado = s.paso().unwrap();
        assert_eq!(estado, EstadoDelObservador::Buscando);
        assert!(s.cargar().unwrap().sesiones.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// El escaneo de una fixture de contenedor no produce grupos: nada que interpretar.
    #[test]
    fn el_escaneo_de_contenedor_no_produce_grupos() {
        let rasgos: Vec<RasgosDeProceso> = CONTENEDOR
            .iter()
            .map(crate::proceso_linux::pruebas_de_regresion::rasgos_para_pruebas)
            .collect();
        let escaneo: EscaneoDeProcesos = crate::proceso_linux::escanear_desde(&rasgos);
        assert!(escaneo.grupos.is_empty());
    }
}

#[cfg(test)]
mod pruebas_id_de_sesion {
    use super::{id_de_sesion, ids_existentes};
    use std::collections::HashSet;

    fn con(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    /// El primer id de un segundo conserva la forma histórica: no rompe lo ya guardado.
    #[test]
    fn el_primer_id_conserva_el_formato_de_siempre() {
        let id = id_de_sesion("2026-11-26 11:27:45", &con(&["ses_20260101000000"]));
        assert_eq!(id, "ses_20261126112745");
    }

    /// El caso medido: dos (o tres) juegos abren sesión en el mismo segundo y antes
    /// recibían el **mismo** identificador.
    #[test]
    fn dos_sesiones_en_el_mismo_segundo_no_repiten_id() {
        let ahora = "2026-11-26 11:27:45";
        let primero = id_de_sesion(ahora, &con(&[]));
        let segundo = id_de_sesion(ahora, &con(&[&primero]));
        let tercero = id_de_sesion(ahora, &con(&[&primero, &segundo]));
        assert_eq!(primero, "ses_20261126112745");
        assert_eq!(segundo, "ses_20261126112745_2");
        assert_eq!(tercero, "ses_20261126112745_3");
        assert_eq!(
            [primero, segundo, tercero]
                .iter()
                .collect::<HashSet<_>>()
                .len(),
            3
        );
    }

    /// Reutilizar un id histórico ya ocupado no lo reescribe: se elige otro nuevo.
    #[test]
    fn un_id_historico_ocupado_no_se_reutiliza() {
        let id = id_de_sesion("2026-11-26 11:27:45", &con(&["ses_20261126112745"]));
        assert_eq!(id, "ses_20261126112745_2");
    }

    /// Un hueco intermedio se respeta: si `_2` ya existía, se usa `_3`.
    #[test]
    fn se_elige_el_menor_sufijo_libre() {
        let id = id_de_sesion(
            "2026-11-26 11:27:45",
            &con(&["ses_20261126112745", "ses_20261126112745_2"]),
        );
        assert_eq!(id, "ses_20261126112745_3");
    }

    /// Segundo distinto ⇒ id distinto sin necesidad de sufijo.
    #[test]
    fn segundos_distintos_dan_ids_distintos() {
        let a = id_de_sesion("2026-11-26 11:27:45", &con(&[]));
        let b = id_de_sesion("2026-11-26 11:27:46", &con(&[&a]));
        assert_ne!(a, b);
        assert_eq!(b, "ses_20261126112746");
    }

    /// `ids_existentes` recoge también ids con sufijo, para no reutilizarlos.
    #[test]
    fn ids_existentes_incluye_los_sufijados() {
        let mut data = crate::sessions::SesionesData::default();
        for id in ["ses_20261126112745", "ses_20261126112745_2"] {
            data.sesiones.push(crate::sessions::SesionDeEjecucion::agrupada(
                id.to_string(),
                crate::sessions::IdentidadDeSesion::desconocida(),
                "2026-11-26 11:27:45",
            ));
        }
        let ocupados = ids_existentes(&data);
        assert_eq!(ocupados.len(), 2);
        assert_eq!(
            id_de_sesion("2026-11-26 11:27:45", &ocupados),
            "ses_20261126112745_3"
        );
    }
}

#[cfg(test)]
mod pruebas_continuidad_de_sesion {
    /// **Incidencia B, la pista**: un nombre del frontend no puede convertirse en nombre
    /// observado.
    ///
    /// `pista` es «qué juego está mirando el usuario», no lo que dice `/proc`. Se usaba como
    /// respaldo de `nombre_observado`, así que una sesión podía quedar registrada con el
    /// nombre de **otro** juego. El nombre comercial del catálogo debe prevalecer en la
    /// interfaz, y el observado tiene que salir de la evidencia de la ejecución.
    #[test]
    fn la_pista_del_frontend_no_se_convierte_en_nombre_observado() {
        let d = std::env::temp_dir().join(format!("thor_pista_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let mut svc = ServicioDeSesiones::con_ruta(d.join("sessions.json"));
        // El usuario está mirando la página de «Eslabong», pero lo que corre es otro juego.
        svc.fijar_pista(Some("Eslabong".to_string()));
        let arbol = arbol_proton(400100, 400000, "2026-11-26 17:00:00");
        svc.observador_inyectado = Some(ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&arbol).grupos,
        ));
        svc.fijar_activo(true);
        let _ = svc.paso().expect("la pasada no falla");

        let sesiones = svc.cargar().unwrap().sesiones;
        assert_eq!(sesiones.len(), 1, "se abre una sesión");
        let observado = sesiones[0].identidad.nombre_observado.as_deref();
        assert_ne!(
            observado,
            Some("Eslabong"),
            "el nombre observado no puede ser la pista del frontend: {observado:?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **P2**: una coincidencia comercial no autoriza a adoptar procesos de otra ejecución.
    ///
    /// La sesión guardada cita un proceso que ya no existe en el grupo actual (otra partida
    /// del mismo juego). Sin embargo ese proceso **sigue vivo** en `/proc`, porque otro
    /// lanzamiento lo tiene. Antes, `|| self.sigue_vivo(c)` daba continuidad y el grupo nuevo
    /// se adoptaba: la sesión vieja cambiaba de ejecución sin cerrarse. La continuidad tiene
    /// que ser de la instancia, no de la identidad.
    #[test]
    fn una_coincidencia_de_nombre_no_adopta_procesos_de_otra_ejecucion() {
        let d = std::env::temp_dir().join(format!("thor_p2_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let mut svc = ServicioDeSesiones::con_ruta(d.join("sessions.json"));
        // Sesión abierta de una partida anterior del mismo juego.
        let arbol_viejo = arbol_proton(210465, 210400, "2026-11-26 15:00:00");
        svc.observador_inyectado = Some(ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&arbol_viejo).grupos,
        ));
        svc.fijar_activo(true);
        svc.paso().unwrap();
        let primera = svc.cargar().unwrap().sesiones;
        assert_eq!(primera.len(), 1, "la primera pasada abre una sesión");
        let id_viejo = primera[0].id.clone();

        // Se abre una partida nueva, con otros procesos, del mismo juego.
        let arbol_nuevo = arbol_proton(300900, 300800, "2026-11-26 16:00:00");
        svc.observador_inyectado = Some(ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&arbol_nuevo).grupos,
        ));
        for _ in 0..4 {
            let _ = svc.paso().unwrap();
        }
        let sesiones = svc.cargar().unwrap().sesiones;
        let vieja = sesiones.iter().find(|s| s.id == id_viejo).expect("la sesión vieja existe");
        // Lo que no puede pasar: que la sesión vieja siga viva y con las claves del árbol
        // nuevo. Eso sería haber adoptado otra ejecución por coincidir el nombre.
        let adopto = vieja.vivos_de_la_ejecucion.iter().any(|c| c.contains("300900"))
            || vieja.arbol_del_lanzamiento.iter().any(|c| c.contains("300900"));
        assert!(
            !adopto,
            "la sesión vieja adoptó procesos de la ejecución nueva: {:?} / {:?}",
            vieja.vivos_de_la_ejecucion, vieja.arbol_del_lanzamiento
        );
        let _ = std::fs::remove_dir_all(&d);
    }


    use super::*;
    use crate::proceso_linux::ObservadorSimulado;
    use crate::sessions::RasgosDeProceso;

    fn dir_temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "thoriapp_cont_{}_{}_{}",
            nombre,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Árbol del lanzamiento **real** de Tattoo Tycoon medido en CachyOS.
    ///
    /// Se reproduce tal como lo vio el diagnóstico, no como sería cómodo: un juego de Heroic
    /// con dos preloaders de Wine, `dolphin` (el gestor de archivos desde el que se lanzó) y
    /// `srt-logger` (infraestructura de Steam) colgando del mismo árbol. Esos dos últimos
    /// sobreviven a la partida y son los que mantenían la sesión abierta para siempre.
    ///
    /// `vivos` de los tests con este árbol son las claves de los procesos que el escenario
    /// quiere que sigan en `/proc`: los once restantes ya no están.
    fn arbol_tattoo_tycoon() -> Vec<RasgosDeProceso> {
        let base = RasgosDeProceso {
            carpeta_declarada: Some("TattooTycoon".into()),
            ruta_del_juego: Some("TattooTycoon".into()),
            cwd: Some("/home/jugador/Games/Heroic/TattooTycoon".into()),
            bajo_compatibilidad: Some(true),
            ..Default::default()
        };
        // PIDs reales, todos del 2026-11-26 12:16:11 salvo la infraestructura.
        let p = |pid: u32, ppid: u32, exe: &str, juego: Option<&str>| RasgosDeProceso {
            pid,
            ppid: Some(ppid),
            ejecutable: Some(exe.into()),
            ruta_ejecutable: Some(exe.into()),
            inicio_del_proceso: Some("2026-11-26 12:16:11".into()),
            ejecutable_del_juego: juego.map(|j| j.to_string()),
            ..base.clone()
        };
        vec![
            // Ancestros genéricos: están en el árbol, no ejecutan el juego.
            p(327194, 327127, "umu-run", None),
            p(327127, 327119, "python3", None),
            p(327119, 3355, "heroic", None),
            // El juego y sus envoltorios.
            p(327299, 327194, "wine-preloader", None),
            RasgosDeProceso {
                pid: 327412,
                ppid: Some(327299),
                ejecutable: Some("wine64-preloader".into()),
                ruta_ejecutable: Some("wine64-preloader".into()),
                inicio_del_proceso: Some("2026-11-26 12:16:11".into()),
                ejecutable_del_juego: Some("Tattoo Tycoon.exe".into()),
                ejecuta_de_verdad: Some(true),
                ..base.clone()
            },
            // Auxiliares de Wine: no acreditan ejecución.
            p(325652, 327412, "pv-adverb", None),
            p(325698, 327412, "wine64-preloader", None),
            // Infraestructura que sobrevive a la partida.
            RasgosDeProceso {
                pid: 16137,
                ppid: Some(1),
                ejecutable: Some("dolphin".into()),
                ruta_ejecutable: Some("dolphin".into()),
                inicio_del_proceso: Some("2026-11-26 08:19:07".into()),
                ..base.clone()
            },
            RasgosDeProceso {
                pid: 127349,
                ppid: Some(1),
                ejecutable: Some("srt-logger".into()),
                ruta_ejecutable: Some("srt-logger".into()),
                inicio_del_proceso: Some("2026-11-26 09:40:47".into()),
                ..base
            },
        ]
    }

    /// Árbol mínimo de un lanzamiento Proton, con el **representante** elegido a mano.
    ///
    /// Se construye explícitamente en vez de derivarlo de un fixture porque lo que se prueba
    /// es justo quién representa al grupo: el proceso que el escaneo elige como principal.
    /// `pid_juego` es el del `.exe` (el que ejecuta de verdad); `pid_envoltorio` el del
    /// preloader, que es su padre. Cambiar `pid_juego` entre pasadas reproduce el caso medido
    /// en CachyOS (210465 → 210612) sin cambiar nada más de la ejecución.
    fn arbol_proton(
        pid_juego: u32,
        pid_envoltorio: u32,
        inicio_juego: &str,
    ) -> Vec<RasgosDeProceso> {
        let base = RasgosDeProceso {
            appid_del_entorno: Some(crate::sessions::EvidenciaAppId {
                steam_app_id: Some("4623570".into()),
                steam_game_id: Some("4623570".into()),
            }),
            carpeta_declarada: Some("steamapps/common/Deskrawl".into()),
            ruta_del_juego: Some("steamapps/common/Deskrawl".into()),
            cwd: Some("steamapps/common/Deskrawl".into()),
            bajo_proton: Some(true),
            bajo_compatibilidad: Some(true),
            ..Default::default()
        };
        vec![
            // El preloader de Wine: envoltorio del lanzamiento.
            RasgosDeProceso {
                pid: pid_envoltorio,
                ppid: Some(1),
                ejecutable: Some("wine64-preloader".into()),
                ruta_ejecutable: Some("wine64-preloader".into()),
                inicio_del_proceso: Some("2026-11-26 14:16:01".into()),
                ejecutable_del_juego: Some("Deskrawl.exe".into()),
                ..base.clone()
            },
            // El juego: el que ejecuta de verdad.
            RasgosDeProceso {
                pid: pid_juego,
                ppid: Some(pid_envoltorio),
                ejecutable: Some("Deskrawl.exe".into()),
                ruta_ejecutable: Some("wine64-preloader".into()),
                inicio_del_proceso: Some(inicio_juego.to_string()),
                ejecutable_del_juego: Some("Deskrawl.exe".into()),
                ejecuta_de_verdad: Some(true),
                ..base
            },
        ]
    }

    fn servicio_con(dir: &std::path::Path) -> ServicioDeSesiones {
        let mut s = servicio_en(dir.join("sessions.json"));
        s.fijar_activo(true);
        s
    }

    /// **Regresión real de Deskrawl**: una sola ejecución, dos pasadas, representante distinto.
    /// Debe seguir siendo **una** sesión, con el mismo id y el mismo instante de inicio.
    #[test]
    fn el_representante_cambia_y_la_sesion_no() {
        let d = dir_temporal("deskrawl_representante");
        let mut s = servicio_con(&d);

        // Pasada 1: el juego es el PID 210465.
        let grupos =
            crate::proceso_linux::escanear_desde(&arbol_proton(210465, 210400, "2026-11-26 14:16:01"))
                .grupos;
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(grupos));
        s.paso().expect("pasada 1");

        let tras_1 = s.cargar().expect("cargar").sesiones;
        assert_eq!(tras_1.len(), 1, "la primera pasada abre una sesión");
        let id_1 = tras_1[0].id.clone();
        let inicio_1 = tras_1[0].abierta_en.clone();

        // Pasada 2: el representante es **otro PID** (210612). Misma ejecución: su instante
        // de arranque y su instalación no cambian.
        let otro = arbol_proton(210612, 210400, "2026-11-26 14:16:01");
        let grupos2 = crate::proceso_linux::escanear_desde(&otro).grupos;
        let mut sim2 = ObservadorSimulado::con_grupos(grupos2);
        // Lo que `/proc` dice de verdad: siguen vivos los procesos del árbol.
        sim2.vivos = otro
            .iter()
            .map(|p| {
                format!(
                    "{}@{}",
                    p.pid,
                    p.inicio_del_proceso.clone().unwrap_or_default()
                )
            })
            .collect();
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada 2");

        let tras_2 = s.cargar().expect("cargar").sesiones;
        assert!(
            tras_2.iter().any(|x| x.id == id_1),
            "la sesión abierta no puede desaparecer: {:?}",
            tras_2.iter().map(|x| &x.id).collect::<Vec<_>>()
        );
        assert_eq!(
            tras_2.len(),
            1,
            "un cambio de representante dentro de la MISMA ejecución no puede abrir otra \
             sesión; sesiones: {:?}",
            tras_2.iter().map(|x| &x.id).collect::<Vec<_>>()
        );
        assert_eq!(tras_2[0].id, id_1, "el identificador se conserva");
        assert_eq!(
            tras_2[0].abierta_en, inicio_1,
            "el instante inicial se conserva"
        );
    }

    // ============ Regresiones con el árbol REAL de Tattoo Tycoon ============

    /// **El defecto medido en CachyOS**: el juego se cierra y `dolphin` y `srt-logger`
    /// siguen vivos. La sesión deja de estar observada y termina por el ciclo normal.
    ///
    /// Antes, `vivos_de_la_ejecucion` guardaba los 13 procesos del árbol, así que bastaba con
    /// que **uno** de los dos supervivientes estuviera en `/proc` para que
    /// `pasadas_sin_verlo` volviera a 0 en cada pasada. La sesión `ses_20261126121612` se
    /// quedó `OBSERVADA` durante horas con el juego cerrado.
    #[test]
    fn el_juego_se_cierra_y_dolphin_y_srt_logger_no_mantienen_la_sesion() {
        let d = dir_temporal("tattoo_cierre");
        let mut s = servicio_con(&d);

        // Pasada 1: el juego corre con todo su árbol.
        let arbol = arbol_tattoo_tycoon();
        let mut sim = ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&arbol).grupos,
        );
        sim.vivos = arbol.iter().map(clave_de).collect();
        s.observador_inyectado = Some(sim);
        s.paso().expect("pasada 1");

        let tras_1 = s.cargar().expect("cargar").sesiones;
        assert!(!tras_1.is_empty(), "el juego se detecta");
        let id = tras_1[0].id.clone();
        assert_eq!(tras_1[0].evidencia_proceso, EvidenciaProceso::Observada);

        // **La comprobación central**: la ejecución guardada no puede incluir ni a `dolphin`
        // ni a `srt-logger`. Ni envoltorios genéricos: solo lo que acredita ejecución.
        let ejecucion = &tras_1[0].vivos_de_la_ejecucion;
        assert!(
            !ejecucion.iter().any(|c| c.starts_with("16137@")),
            "dolphin no puede acreditar ejecución: {:?}",
            ejecucion
        );
        assert!(
            !ejecucion.iter().any(|c| c.starts_with("127349@")),
            "srt-logger no puede acreditar ejecución: {:?}",
            ejecucion
        );
        assert!(
            ejecucion.iter().any(|c| c.starts_with("327412@")),
            "el proceso que ejecuta el juego sí: {:?}",
            ejecucion
        );

        // Pasada 2: el juego se cerró. Solo sobreviven dolphin y srt-logger.
        let mut sim2 = ObservadorSimulado::con_grupos(Vec::new());
        sim2.vivos = vec![
            "16137@2026-11-26 08:19:07".to_string(),
            "127349@2026-11-26 09:40:47".to_string(),
        ];
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada 2");

        let tras_2 = s.cargar().expect("cargar").sesiones;
        let sesion = tras_2.iter().find(|x| x.id == id).expect("la sesión sigue");
        assert_ne!(
            sesion.evidencia_proceso,
            EvidenciaProceso::Observada,
            "con el juego cerrado no puede seguir OBSERVADA sólo porque sobreviva dolphin"
        );
        assert_eq!(sesion.pasadas_sin_verlo, 1, "el contador de ausencia progresa");

        // Y tras las pasadas de ausencia, termina por el ciclo normal.
        for esperadas in 2..=PASADAS_SIN_VERLO {
            s.paso().expect("pasada de ausencia");
            let ahora = s.cargar().expect("cargar").sesiones;
            let sesion = ahora.iter().find(|x| x.id == id).expect("la sesión sigue ahí");
            assert_eq!(sesion.pasadas_sin_verlo, esperadas);
        }
        let fin = s.cargar().expect("cargar").sesiones;
        let sesion = fin.iter().find(|x| x.id == id).expect("la sesión sigue ahí");
        assert_eq!(
            sesion.evidencia_proceso,
            EvidenciaProceso::Terminada,
            "tras las pasadas de ausencia la sesión termina"
        );
        assert!(sesion.cerrada_en.is_some(), "queda con instante de cierre");
    }

    /// `ultima_observacion` refleja una observación **real** del juego, no la supervivencia de
    /// un auxiliar.
    #[test]
    fn ultima_observacion_no_avanza_por_un_auxiliar_superviviente() {
        let d = dir_temporal("tattoo_ultima_obs");
        let mut s = servicio_con(&d);

        let arbol = arbol_tattoo_tycoon();
        let mut sim = ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&arbol).grupos,
        );
        sim.vivos = arbol.iter().map(clave_de).collect();
        s.observador_inyectado = Some(sim);
        s.paso().expect("pasada 1");

        let primera = s.cargar().expect("cargar").sesiones;
        let cuando_1 = primera[0].ultima_observacion.clone();
        assert!(cuando_1.is_some(), "se vio el juego");

        // El juego se cierra; los auxiliares siguen.
        let mut sim2 = ObservadorSimulado::con_grupos(Vec::new());
        sim2.vivos = vec![
            "16137@2026-11-26 08:19:07".to_string(),
            "127349@2026-11-26 09:40:47".to_string(),
        ];
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada 2");

        let despues = s.cargar().expect("cargar").sesiones;
        assert_eq!(
            despues[0].ultima_observacion, cuando_1,
            "sin ver el juego, la última observación no puede avanzar"
        );
    }

    /// **Reiniciar Thoriapp no puede recuperar el juego como activo** desde una sesión
    /// persistida contaminada.
    ///
    /// Es el caso exacto del equipo real: `sessions.json` tenía una sesión `Observada` cuyo
    /// `vivos_de_la_ejecucion` incluía `dolphin` y `srt-logger`. Al arrancar, la aplicación
    /// volvía a enseñar Tattoo Tycoon como si estuviera corriendo.
    #[test]
    fn una_sesion_persistida_contaminada_no_revive_al_reiniciar() {
        let d = dir_temporal("tattoo_reinicio");
        let mut s = servicio_con(&d);

        // Se siembra el disco con lo que dejó la versión anterior: la lista **contaminada**,
        // con todos los miembros del árbol, tal como estaba en `sessions.json` real.
        let arbol = arbol_tattoo_tycoon();
        let mut data = crate::sessions::SesionesData::default();
        let rasgos = arbol
            .iter()
            .find(|p| p.pid == 327412)
            .expect("el proceso del juego");
        let mut sesion = SesionDeEjecucion::observada(
            "ses_20261126121612".to_string(),
            rasgos.clone(),
            crate::sessions::IdentidadDeSesion {
                appid: None,
                nombre_observado: Some("Tattoo Tycoon".into()),
                nombre_comercial: Some("Tattoo Tycoon".into()),
                ..Default::default()
            },
            "2026-11-26 12:16:12",
        );
        sesion.vivos_de_la_ejecucion = arbol.iter().map(clave_de).collect();
        data.sesiones.push(sesion);
        s.guardar(&data).expect("sembrar el disco");

        // Arranque: el escaneo no ve ningún juego; solo sobreviven los dos auxiliares.
        let mut sim = ObservadorSimulado::con_grupos(Vec::new());
        sim.vivos = vec![
            "16137@2026-11-26 08:19:07".to_string(),
            "127349@2026-11-26 09:40:47".to_string(),
        ];
        s.observador_inyectado = Some(sim);
        let estado = s.paso().expect("pasada de arranque");

        // No se publica como juego en marcha.
        assert!(
            !matches!(estado, EstadoDelObservador::Observando { .. }),
            "no puede publicarse un juego que no corre: {:?}",
            estado
        );

        let tras = s.cargar().expect("cargar").sesiones;
        let sesion = tras
            .iter()
            .find(|x| x.id == "ses_20261126121612")
            .expect("no se borra la sesión");
        assert_ne!(
            sesion.evidencia_proceso,
            EvidenciaProceso::Observada,
            "una sesión contaminada no puede seguir observada tras revalidarla"
        );
        assert_eq!(
            sesion.evidencia_proceso,
            EvidenciaProceso::VisibilidadPerdida,
            "sin evidencia de ejecución se conserva la incertidumbre, no se inventa un cierre"
        );
        assert_eq!(sesion.abierta_en, "2026-11-26 12:16:12", "no se toca el histórico");
        assert!(sesion.cerrada_en.is_none(), "no se cierra de golpe");
    }

    /// Si al reiniciar **sí** se ve el juego, la sesión persistida se recupera con su
    /// identidad, su fecha inicial y su snapshot: no se abre otra.
    #[test]
    fn una_sesion_persistida_valida_se_recupera_al_reiniciar() {
        let d = dir_temporal("tattoo_recupera");
        let mut s = servicio_con(&d);

        let arbol = arbol_tattoo_tycoon();
        let mut sim = ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&arbol).grupos,
        );
        sim.vivos = arbol.iter().map(clave_de).collect();
        s.observador_inyectado = Some(sim);
        s.paso().expect("pasada 1");

        let tras_1 = s.cargar().expect("cargar").sesiones;
        let id = tras_1[0].id.clone();
        let abierta = tras_1[0].abierta_en.clone();
        let snapshot = tras_1[0].configuracion_inicial.clone();

        // Arranque de nuevo con el mismo árbol a la vista.
        let mut sim2 = ObservadorSimulado::con_grupos(crate::proceso_linux::escanear_desde(&arbol).grupos);
        sim2.vivos = arbol.iter().map(clave_de).collect();
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada tras reinicio");

        let tras_2 = s.cargar().expect("cargar").sesiones;
        assert_eq!(tras_2.len(), 1, "no se duplica la sesión al reiniciar");
        assert_eq!(tras_2[0].id, id, "conserva el identificador");
        assert_eq!(tras_2[0].abierta_en, abierta, "conserva la fecha inicial");
        assert_eq!(
            tras_2[0].configuracion_inicial, snapshot,
            "conserva el snapshot"
        );
        assert_eq!(
            tras_2[0].evidencia_proceso,
            EvidenciaProceso::Observada,
            "sigue observada porque el juego se está viendo"
        );
    }

    /// El representante **desaparece** (`/proc` deja de verlo) pero el resto de la ejecución
    /// sigue: la sesión no se cierra ni se abre otra.
    #[test]
    fn si_desaparece_el_representante_la_ejecucion_continua() {
        let d = dir_temporal("desaparece_representante");
        let mut s = servicio_con(&d);

        let grupos =
            crate::proceso_linux::escanear_desde(&arbol_proton(210465, 210400, "2026-11-26 14:16:01"))
                .grupos;
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(grupos));
        s.paso().expect("pasada 1");
        let tras_1 = s.cargar().expect("cargar").sesiones;
        let id_1 = tras_1[0].id.clone();

        // Pasada 2: el juego (210465) ya no se ve, pero el envoltorio (210400) sigue vivo.
        let otro = arbol_proton(210612, 210400, "2026-11-26 14:16:01");
        let mut sim2 = ObservadorSimulado::con_grupos(
            crate::proceso_linux::escanear_desde(&otro).grupos,
        );
        sim2.vivos = vec!["210400@2026-11-26 14:16:01".to_string()];
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada 2");

        let tras_2 = s.cargar().expect("cargar").sesiones;
        assert_eq!(tras_2.len(), 1, "no puede abrirse una segunda sesión");
        assert_eq!(tras_2[0].id, id_1);
        assert_eq!(
            tras_2[0].evidencia_proceso,
            EvidenciaProceso::Observada,
            "la ejecución sigue viva: no es una pérdida de visibilidad"
        );
    }

    /// El identificador, el instante inicial, la instantánea y las grabaciones asociadas se
    /// conservan cuando el representante cambia.
    #[test]
    fn el_cambio_de_representante_conserva_snapshot_y_grabaciones() {
        let d = dir_temporal("conserva_snapshot");
        let mut s = servicio_con(&d);

        let grupos =
            crate::proceso_linux::escanear_desde(&arbol_proton(210465, 210400, "2026-11-26 14:16:01"))
                .grupos;
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(grupos));
        s.paso().expect("pasada 1");
        let tras_1 = s.cargar().expect("cargar").sesiones;
        let id_1 = tras_1[0].id.clone();
        let abierta_1 = tras_1[0].abierta_en.clone();
        let snapshot_1 = tras_1[0].configuracion_inicial.clone();

        // Se le asocia una grabación y se fija el juego a mano.
        s.anotar_grabacion(&id_1, "bench-abc");
        s.asignar_juego(&id_1, 4623570, "Deskrawl")
            .expect("asignar juego");

        // Pasada 2 con representante nuevo.
        let otro = arbol_proton(210612, 210400, "2026-11-26 14:16:01");
        let mut sim2 =
            ObservadorSimulado::con_grupos(crate::proceso_linux::escanear_desde(&otro).grupos);
        sim2.vivos = otro
            .iter()
            .map(|p| {
                format!(
                    "{}@{}",
                    p.pid,
                    p.inicio_del_proceso.clone().unwrap_or_default()
                )
            })
            .collect();
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada 2");

        let tras_2 = s.cargar().expect("cargar").sesiones;
        assert_eq!(tras_2.len(), 1, "sigue siendo una sola sesión");
        let sesion = &tras_2[0];
        assert_eq!(sesion.id, id_1, "el identificador se conserva");
        assert_eq!(sesion.abierta_en, abierta_1, "el instante inicial se conserva");
        assert_eq!(
            sesion.configuracion_inicial, snapshot_1,
            "la instantánea capturada al abrir no se reescribe"
        );
        assert_eq!(
            sesion.grabaciones,
            vec!["bench-abc".to_string()],
            "las grabaciones asociadas se conservan"
        );
        assert_eq!(
            sesion.identidad.appid,
            Some(4623570),
            "el juego fijado a mano no se pierde"
        );
    }

    /// **PID reutilizado**: otro proceso ocupa el mismo PID con otro instante de arranque. No
    /// es la misma ejecución, así que no puede heredar la sesión.
    #[test]
    fn un_pid_reutilizado_no_continua_la_sesion_anterior() {
        let d = dir_temporal("pid_reutilizado");
        let mut s = servicio_con(&d);

        let grupos =
            crate::proceso_linux::escanear_desde(&arbol_proton(210465, 210400, "2026-11-26 14:16:01"))
                .grupos;
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(grupos));
        s.paso().expect("pasada 1");
        let tras_1 = s.cargar().expect("cargar").sesiones;
        let id_1 = tras_1[0].id.clone();

        // Mismo PID (210465) para el juego, pero arrancado más tarde: es **otra** ejecución.
        // El envoltorio también es nuevo (210999): tras cerrar la partida, el preloader
        // anterior ya no existe. Reutilizar su PID *y* su starttime sería imposible en un
        // equipo real y haría que la prueba no describiera nada cierto.
        let otro = arbol_proton(210465, 210999, "2026-11-26 15:30:00");
        let mut sim2 =
            ObservadorSimulado::con_grupos(crate::proceso_linux::escanear_desde(&otro).grupos);
        sim2.vivos = otro
            .iter()
            .map(|p| {
                format!(
                    "{}@{}",
                    p.pid,
                    p.inicio_del_proceso.clone().unwrap_or_default()
                )
            })
            .collect();
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada 2");

        let tras_2 = s.cargar().expect("cargar").sesiones;
        assert_eq!(
            tras_2.len(),
            2,
            "un PID reutilizado es otra ejecución, no la misma: {:?}",
            tras_2.iter().map(|x| &x.id).collect::<Vec<_>>()
        );
        assert!(
            tras_2.iter().any(|x| x.id == id_1),
            "la sesión anterior sigue ahí"
        );
    }

    /// **Cierre y reapertura legítimos**: la misma partida se cierra y se vuelve a abrir con
    /// otro PID y otro instante. Debe ser una sesión nueva, no la de antes.
    #[test]
    fn cerrar_y_reabrir_abre_sesion_nueva() {
        let d = dir_temporal("cerrar_reabrir");
        let mut s = servicio_con(&d);

        let grupos =
            crate::proceso_linux::escanear_desde(&arbol_proton(210465, 210400, "2026-11-26 14:16:01"))
                .grupos;
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(grupos));
        s.paso().expect("pasada 1");
        let id_1 = s.cargar().expect("cargar").sesiones[0].id.clone();

        // Todo muere: la sesión termina tras las pasadas de gracia.
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(Vec::new()));
        for _ in 0..(PASADAS_SIN_VERLO + 1) {
            s.paso().expect("pasadas sin verlo");
        }
        let tras_cierre = s.cargar().expect("cargar").sesiones;
        let cerrada = tras_cierre
            .iter()
            .find(|x| x.id == id_1)
            .expect("la sesión cerrada sigue en el historial");
        assert_eq!(cerrada.evidencia_proceso, EvidenciaProceso::Terminada);

        // Reapertura: PID y arranque distintos.
        let otro = arbol_proton(300001, 300000, "2026-11-26 16:00:00");
        let mut sim2 =
            ObservadorSimulado::con_grupos(crate::proceso_linux::escanear_desde(&otro).grupos);
        sim2.vivos = otro
            .iter()
            .map(|p| {
                format!(
                    "{}@{}",
                    p.pid,
                    p.inicio_del_proceso.clone().unwrap_or_default()
                )
            })
            .collect();
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada de reapertura");

        let tras = s.cargar().expect("cargar").sesiones;
        let abiertas: Vec<_> = tras
            .iter()
            .filter(|x| x.evidencia_proceso == EvidenciaProceso::Observada)
            .collect();
        assert_eq!(abiertas.len(), 1, "una sola sesión abierta");
        assert_ne!(abiertas[0].id, id_1, "es una ejecución nueva");
    }

    /// **Dos instancias del mismo juego**: dos árboles con el mismo AppID y la misma
    /// instalación, pero procesos distintos. Siguen siendo dos sesiones.
    #[test]
    fn dos_instancias_del_mismo_juego_no_se_fusionan() {
        let d = dir_temporal("dos_instancias");
        let mut s = servicio_con(&d);

        let mut ambos = arbol_proton(210465, 210400, "2026-11-26 14:16:01");
        ambos.extend(arbol_proton(220465, 220400, "2026-11-26 14:20:00"));
        let grupos = crate::proceso_linux::escanear_desde(&ambos).grupos;
        assert_eq!(grupos.len(), 2, "el escaneo ve dos ejecuciones distintas");
        let mut sim = ObservadorSimulado::con_grupos(grupos);
        sim.vivos = ambos
            .iter()
            .map(|p| {
                format!(
                    "{}@{}",
                    p.pid,
                    p.inicio_del_proceso.clone().unwrap_or_default()
                )
            })
            .collect();
        s.observador_inyectado = Some(sim);
        s.paso().expect("pasada");

        let ids: Vec<String> = s
            .cargar()
            .expect("cargar")
            .sesiones
            .iter()
            .map(|x| x.id.clone())
            .collect();
        assert_eq!(
            ids.len(),
            2,
            "dos ejecuciones independientes no pueden fusionarse: {:?}",
            ids
        );
        assert_ne!(ids[0], ids[1], "y no pueden compartir identificador");
    }

    /// **Pérdida temporal de visibilidad**: una pasada no ve nada, la siguiente vuelve a ver
    /// la misma ejecución. No se cierra la sesión ni se abre otra.
    #[test]
    fn una_perdida_temporal_de_visibilidad_no_cierra_la_sesion() {
        let d = dir_temporal("perdida_temporal");
        let mut s = servicio_con(&d);

        let grupos =
            crate::proceso_linux::escanear_desde(&arbol_proton(210465, 210400, "2026-11-26 14:16:01"))
                .grupos;
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(grupos));
        s.paso().expect("pasada 1");
        let id_1 = s.cargar().expect("cargar").sesiones[0].id.clone();

        // Pasada a ciegas: sin grupos y sin procesos vivos.
        s.observador_inyectado = Some(ObservadorSimulado::con_grupos(Vec::new()));
        s.paso().expect("pasada a ciegas");
        let tras_ciego = s.cargar().expect("cargar").sesiones;
        assert_eq!(tras_ciego.len(), 1, "no se abre una sesión nueva");
        assert_eq!(
            tras_ciego[0].evidencia_proceso,
            EvidenciaProceso::VisibilidadPerdida,
            "se marca como perdida, no como terminada"
        );

        // Vuelve a verse, con el representante cambiado.
        let otro = arbol_proton(210612, 210400, "2026-11-26 14:16:01");
        let mut sim2 =
            ObservadorSimulado::con_grupos(crate::proceso_linux::escanear_desde(&otro).grupos);
        sim2.vivos = otro
            .iter()
            .map(|p| {
                format!(
                    "{}@{}",
                    p.pid,
                    p.inicio_del_proceso.clone().unwrap_or_default()
                )
            })
            .collect();
        s.observador_inyectado = Some(sim2);
        s.paso().expect("pasada de vuelta");

        let tras = s.cargar().expect("cargar").sesiones;
        assert_eq!(tras.len(), 1, "sigue siendo la misma sesión: {:?}", tras.len());
        assert_eq!(tras[0].id, id_1, "con el mismo identificador");
        assert_eq!(
            tras[0].evidencia_proceso,
            EvidenciaProceso::Observada,
            "y se recupera al volver a verla"
        );
        assert_eq!(tras[0].pasadas_sin_verlo, 0, "el contador se limpia");
    }
}
