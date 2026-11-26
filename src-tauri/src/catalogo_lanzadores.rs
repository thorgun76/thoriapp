//! Catálogo unificado de juegos instalados, con caché.
//!
//! Reúne lo que ya saben los lectores existentes —`steam::catalogo_para_identidad` y
//! `launchers::scan_all_launchers`— en una sola vista, para que el motor de procesos no
//! dependa de un lanzador concreto.
//!
//! No descubre nada por su cuenta: no hay una segunda implementación de los escaneos. Lo
//! único que aporta es **cuándo** se releen y cómo se conserva lo último bueno.
//!
//! # Por qué hay caché
//!
//! Sin ella, cada pasada del observador (cada 5 s) abriría `pga.db` con SQLite y releería
//! todos los JSON de Heroic y Heronite. Eso es caro y no hace falta: la biblioteca de juegos
//! cambia cuando el usuario instala o desinstala algo, no cada cinco segundos.
//!
//! # Por qué no basta el `mtime`
//!
//! `pga.db` es SQLite en modo WAL: las escrituras van al `-wal` y el fichero principal puede
//! no cambiar de fecha. Además un cambio de `directory`, de `runner` o de `service_id` afecta
//! a la identificación aunque el conjunto de juegos instalados sea el mismo. Por eso la
//! revalidación compara una **huella de los campos que importan**, no la fecha del fichero.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::evidencia_proceso::EntradaDeCatalogo;

/// Cada cuánto se relee el catálogo aunque nada parezca haber cambiado.
///
/// Valor inicial acordado. Se puede ajustar con [`TOMAR_MEDIDA`] en la mano.
pub const REVALIDACION: Duration = Duration::from_secs(300);

/// Huella de un catálogo: solo lo que influye en la identificación.
///
/// Si dos catálogos tienen la misma huella, el motor de procesos tomará las mismas
/// decisiones con ellos y no hace falta volver a leer nada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Huella(Vec<String>);

impl Huella {
    pub fn de(entradas: &[EntradaDeCatalogo]) -> Self {
        let mut claves: Vec<String> = entradas
            .iter()
            .map(|e| {
                format!(
                    "{}|{}|{}|{}|{}|{}",
                    e.id_plataforma.as_deref().unwrap_or(""),
                    e.plataforma.as_deref().unwrap_or(""),
                    e.launcher,
                    e.runner.as_deref().unwrap_or(""),
                    e.titulo,
                    e.install_dir.as_deref().unwrap_or(""),
                )
            })
            .collect();
        claves.sort_unstable();
        claves.dedup();
        Huella(claves)
    }
}

/// Lo que hay en la caché y de cuándo es.
#[derive(Debug)]
struct Guardado {
    entradas: Vec<EntradaDeCatalogo>,
    huella: Huella,
    leido_en: Instant,
    /// `true` si la última relectura falló y esto es lo último que se pudo leer bien.
    obsoleto: bool,
}

/// Resultado de intentar leer las fuentes.
///
/// Distinguir «vacío de verdad» de «no se pudo leer» es obligatorio: un usuario sin juegos
/// instalados tiene un catálogo vacío legítimo, y confundirlo con un fallo haría que la app
/// conservase eternamente un catálogo viejo o marcase todo como no verificado.
#[derive(Debug, Clone, PartialEq)]
pub enum Lectura {
    /// Se leyó bien. `entradas` puede estar vacío si no hay juegos instalados.
    Bien(Vec<EntradaDeCatalogo>),
    /// No se pudo leer alguna fuente. Se conserva lo anterior.
    Fallo,
}

/// De dónde salen los datos. Existe para poder probar la caché sin tocar el disco.
type Fuente = fn() -> Lectura;

fn fuente_real() -> Fuente {
    leer_de_las_fuentes
}

static CACHE: OnceLock<Mutex<Option<Guardado>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<Guardado>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Coste de la última actualización y de las lecturas que sirvió la caché.
///
/// Sirve para poder decidir con datos si los 5 minutos son razonables.
pub mod medida {
    use super::*;

    /// Tiempo de la última relectura completa del catálogo.
    pub static ULTIMA_LECTURA: Mutex<Option<Duration>> = Mutex::new(None);
    /// Veces que se ha releído de verdad.
    pub static LECTURAS: Mutex<usize> = Mutex::new(0);
    /// Veces que se sirvió la caché sin releer.
    pub static ACIERTOS: Mutex<usize> = Mutex::new(0);
}

/// Devuelve el catálogo, releyéndolo solo si toca.
///
/// Relee cuando: no hay caché, la huella cambió, pasó [`REVALIDACION`], o la última lectura
/// falló. Si la relectura falla, **se conserva lo último bueno** y se marca como obsoleto en
/// la evidencia: es mejor seguir reconociendo juegos con un catálogo de hace un rato que
/// perder la identidad de todo por un error puntual.
pub fn obtener() -> Vec<EntradaDeCatalogo> {
    obtener_con(fuente_real(), false)
}

/// Igual que [`obtener`] pero forzando la relectura (actualización manual de la biblioteca).
///
/// La llama `refresh_library`, el botón de actualizar de la interfaz: si el usuario acaba de
/// instalar un juego no tiene por qué esperar a la revalidación periódica.
pub fn actualizar() -> Vec<EntradaDeCatalogo> {
    obtener_con(fuente_real(), true)
}

/// Núcleo de la caché, con la fuente inyectable para poder probarla.
///
/// `forzar` salta la comprobación de tiempo: lo usa la actualización manual.
fn obtener_con(fuente: Fuente, forzar: bool) -> Vec<EntradaDeCatalogo> {
    let mut guardado = match cache().lock() {
        Ok(g) => g,
        // Un pánico anterior no debe dejar el observador sin catálogo.
        Err(envenenado) => envenenado.into_inner(),
    };

    // Se relee si no hay nada, si la última lectura falló, si toca revalidar o si alguien lo
    // fuerza. La huella **no** sirve para decidir esto: para calcularla hay que leer primero,
    // así que solo se usa para saber si el resultado cambió respecto al anterior.
    let hay_que_releer = forzar
        || match guardado.as_ref() {
            None => true,
            Some(g) => g.obsoleto || g.leido_en.elapsed() >= REVALIDACION,
        };

    if !hay_que_releer {
        if let Some(g) = guardado.as_ref() {
            if let Ok(mut a) = medida::ACIERTOS.lock() {
                *a += 1;
            }
            return g.entradas.clone();
        }
    }

    let inicio = Instant::now();
    let lectura = fuente();
    let coste = inicio.elapsed();

    if let Ok(mut u) = medida::ULTIMA_LECTURA.lock() {
        *u = Some(coste);
    }
    if let Ok(mut l) = medida::LECTURAS.lock() {
        *l += 1;
    }

    let nuevas = match lectura {
        Lectura::Bien(nuevas) => nuevas,
        Lectura::Fallo => {
            // No se pudo leer: se conserva lo último bueno y se marca como no verificado.
            // Perder la identidad de todo por un error puntual sería peor que usar un
            // catálogo de hace un rato, siempre que quede claro que no está al día.
            if let Some(g) = guardado.as_mut() {
                g.obsoleto = true;
                g.leido_en = Instant::now();
                return g.entradas.clone();
            }
            // Nunca hubo nada válido: se devuelve vacío sin inventar.
            return Vec::new();
        }
    };

    let huella = Huella::de(&nuevas);
    // Si la huella no cambió, la biblioteca está igual aunque se haya releído: solo se
    // refresca la marca de tiempo. Cuando sí cambia, se avisa una vez, para poder ver en la
    // máquina real cada cuánto cambia de verdad la biblioteca.
    let cambio = guardado
        .as_ref()
        .map(|g| g.huella != huella)
        .unwrap_or(true);
    if cambio {
        println!(
            "[Thoriapp] catálogo de lanzadores actualizado: {} entradas",
            nuevas.len()
        );
    }

    *guardado = Some(Guardado {
        entradas: nuevas.clone(),
        huella,
        leido_en: Instant::now(),
        obsoleto: false,
    });
    nuevas
}

/// ¿El catálogo en uso puede estar desactualizado por un fallo de lectura?
pub fn esta_obsoleto() -> bool {
    let guardado = match cache().lock() {
        Ok(g) => g,
        Err(envenenado) => envenenado.into_inner(),
    };
    guardado.as_ref().map(|g| g.obsoleto).unwrap_or(false)
}

/// Fuerza la próxima llamada a releer. Lo usan las pruebas y el arranque.
pub fn invalidar() {
    match cache().lock() {
        Ok(mut g) => *g = None,
        // Un cerrojo envenenado sigue guardando el dato: se vacía igualmente.
        Err(envenenado) => *envenenado.into_inner() = None,
    }
}

/// Lectura real de todas las fuentes. Es lo caro: aquí sí se abre SQLite y se leen JSON.
fn leer_de_las_fuentes() -> Lectura {
    let mut entradas: Vec<EntradaDeCatalogo> = Vec::new();

    // Steam: catálogo ya existente, sin tocar.
    for j in crate::steam::catalogo_para_identidad() {
        entradas.push(EntradaDeCatalogo {
            id_plataforma: Some(j.appid.to_string()),
            plataforma: Some("steam".to_string()),
            launcher: "steam".to_string(),
            runner: None,
            titulo: j.nombre,
            install_dir: j.installdir.map(|d| format!("steamapps/common/{d}")),
            // Steam no declara prefijo ni ejecutable en su catálogo: los pone en el entorno
            // al lanzar, y eso lo ve el observador en el proceso, no aquí.
            wine_prefix: None,
            exec: None,
            runtime_configurado: None,
        });
    }

    // Lutris, Heroic y Heronite: el escáner ya existente, sin duplicarlo.
    for j in crate::launchers::scan_all_launchers() {
        // El identificador estable es el de la plataforma cuando lo hay. `cover_key` **no**
        // se usa: es un nombre de archivo de imagen, no una identidad.
        let plataforma = match j.launcher.as_str() {
            "steam" => Some("steam".to_string()),
            "heroic" => {
                // Heroic agrupa tiendas; el runner dice cuál (`legendary`, `gog`, `nile`…).
                j.runner.clone()
            }
            _ => None,
        };
        entradas.push(EntradaDeCatalogo {
            // El identificador estable de la plataforma. Para un juego que no es de Steam es
            // el id interno del lanzador (el `id` de `pga.db` en Lutris, el id de tienda en
            // Heroic, la clave del `library.json` en Heronite). **No es un AppID**: un juego
            // de Steam ya trae su AppID como identificador de plataforma porque Steam lo es.
            //
            // Sin esto todas las entradas externas quedaban con `id_plataforma: null` y dos
            // juegos distintos del mismo lanzador no se podían distinguir por identidad.
            id_plataforma: j
                .appid
                .map(|a| a.to_string())
                .or_else(|| match j.launcher.as_str() {
                    // Steam ya lo resuelve con el AppID; el resto usa su id propio.
                    "steam" => None,
                    _ => j.id_de_lanzador.clone(),
                }),
            plataforma,
            launcher: j.launcher,
            runner: j.runner,
            titulo: j.name,
            // La carpeta tal cual la da el lanzador. Puede faltar: en Lutris `directory` viene
            // vacío en casi todos los juegos de Wine, y entonces no hay correlación por ruta.
            install_dir: j.install_path,
            wine_prefix: j.wine_prefix,
            exec: j.exec,
            runtime_configurado: j.runtime_configurado,
        });
    }

    // Si no se ha leído **nada** de ninguna fuente, es un fallo de lectura, no un usuario sin
    // juegos: alguien con la biblioteca vacía tendría al menos un lanzador instalado. Aun
    // así, un catálogo vacío con fuentes legibles se devuelve como vacío legítimo.
    if entradas.is_empty() && crate::launchers::hay_alguna_biblioteca() {
        return Lectura::Fallo;
    }

    Lectura::Bien(entradas)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entrada(titulo: &str, dir: Option<&str>) -> EntradaDeCatalogo {
        EntradaDeCatalogo {
            id_plataforma: None,
            plataforma: None,
            launcher: "lutris".into(),
            runner: None,
            titulo: titulo.into(),
            install_dir: dir.map(|d| d.to_string()),
            wine_prefix: None,
            exec: None,
            runtime_configurado: None,
        }
    }

    /// Un cambio de carpeta, runner o título cambia la huella: no basta con mirar qué juegos
    /// siguen instalados.
    #[test]
    fn la_huella_detecta_cambios_que_no_son_instalaciones() {
        let base = vec![entrada("Uno", Some("/games/uno"))];
        let h0 = Huella::de(&base);

        let mut otra_carpeta = base.clone();
        otra_carpeta[0].install_dir = Some("/juegos/uno".into());
        assert_ne!(h0, Huella::de(&otra_carpeta), "cambió la carpeta");

        let mut otro_runner = base.clone();
        otro_runner[0].runner = Some("proton".into());
        assert_ne!(h0, Huella::de(&otro_runner), "cambió el runner");

        let mut otro_id = base.clone();
        otro_id[0].id_plataforma = Some("123".into());
        assert_ne!(h0, Huella::de(&otro_id), "cambió el identificador");
    }

    /// Reordenar la misma biblioteca no es un cambio.
    #[test]
    fn la_huella_no_depende_del_orden() {
        let a = vec![entrada("Uno", Some("/a")), entrada("Dos", Some("/b"))];
        let b = vec![entrada("Dos", Some("/b")), entrada("Uno", Some("/a"))];
        assert_eq!(Huella::de(&a), Huella::de(&b));
    }

    // ---- Escenarios de la caché ----

    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Cerrojo para que los tests de caché no se pisen: la caché es global por diseño (una
    /// sola instancia en toda la app), así que probarla exige serializar el acceso. No se
    /// cambia el diseño por comodidad del test.
    static TESTIGO: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn en_exclusiva() -> std::sync::MutexGuard<'static, ()> {
        match TESTIGO.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }

    /// Contador propio de cada test, sin estado global.
    fn contador() -> &'static AtomicUsize {
        // Un contador por test no se puede pasar como `fn()`; se usa un único contador que
        // cada test reinicia bajo el cerrojo.
        static C: AtomicUsize = AtomicUsize::new(0);
        &C
    }

    /// Cada test usa su propia fuente; el contador se reinicia antes de empezar.
    fn con_fuente(f: Fuente, forzar: bool) -> Vec<EntradaDeCatalogo> {
        invalidar();
        obtener_con(f, forzar)
    }

    fn dos_juegos_counted() -> Lectura {
        contador().fetch_add(1, Ordering::SeqCst);
        Lectura::Bien(vec![
            entrada("Uno", Some("/juegos/uno")),
            entrada("Dos", Some("/juegos/dos")),
        ])
    }

    #[test]
    fn reutiliza_la_cache_sin_releer() {
        let _guardia = en_exclusiva();
        contador().store(0, Ordering::SeqCst);
        let _ = con_fuente(dos_juegos_counted, false);
        let _ = obtener_con(dos_juegos_counted, false);
        let _ = obtener_con(dos_juegos_counted, false);
        assert_eq!(
            contador().load(Ordering::SeqCst),
            1,
            "la segunda y tercera llamada deben servirse de la caché"
        );
        invalidar();
    }

    /// Cambiar solo la carpeta no cambia el identificador, pero **sí** es un cambio real.
    #[test]
    fn un_cambio_de_directorio_se_detecta_aunque_el_id_no_cambie() {
        let _guardia = en_exclusiva();
        fn antes() -> Lectura {
            let mut e = entrada("Uno", Some("/juegos/uno"));
            e.id_plataforma = Some("42".into());
            Lectura::Bien(vec![e])
        }
        fn despues() -> Lectura {
            let mut e = entrada("Uno", Some("/otro/uno"));
            e.id_plataforma = Some("42".into());
            Lectura::Bien(vec![e])
        }
        let a = con_fuente(antes, false);
        let b = obtener_con(despues, true);
        assert_eq!(a[0].id_plataforma, b[0].id_plataforma, "el id no cambió");
        assert_ne!(a[0].install_dir, b[0].install_dir, "la carpeta sí");
        assert_eq!(b[0].install_dir.as_deref(), Some("/otro/uno"));
        invalidar();
    }

    /// Un cambio de runner también es un cambio, aunque los juegos sean los mismos.
    #[test]
    fn un_cambio_de_runner_se_detecta() {
        let _guardia = en_exclusiva();
        fn wine() -> Lectura {
            let mut e = entrada("Uno", Some("/juegos/uno"));
            e.runner = Some("wine".into());
            Lectura::Bien(vec![e])
        }
        fn proton() -> Lectura {
            let mut e = entrada("Uno", Some("/juegos/uno"));
            e.runner = Some("proton".into());
            Lectura::Bien(vec![e])
        }
        let a = con_fuente(wine, false);
        let b = obtener_con(proton, true);
        assert_eq!(a[0].runner.as_deref(), Some("wine"));
        assert_eq!(b[0].runner.as_deref(), Some("proton"));
        invalidar();
    }

    /// Ante un fallo de lectura se conserva lo último bueno, marcado como no verificado.
    #[test]
    fn un_fallo_de_lectura_conserva_lo_anterior() {
        let _guardia = en_exclusiva();
        let bueno = con_fuente(dos_juegos_counted, false);
        assert_eq!(bueno.len(), 2);
        let tras_fallo = obtener_con(|| Lectura::Fallo, true);
        assert_eq!(tras_fallo.len(), 2, "no se pierde lo válido");
        assert!(esta_obsoleto(), "queda marcado como no verificado");
        invalidar();
    }

    /// Y se recupera en cuanto la fuente vuelve a responder.
    #[test]
    fn se_recupera_cuando_la_fuente_vuelve() {
        let _guardia = en_exclusiva();
        let _ = con_fuente(dos_juegos_counted, false);
        let _ = obtener_con(|| Lectura::Fallo, true);
        assert!(esta_obsoleto());
        let recuperado = obtener_con(dos_juegos_counted, true);
        assert_eq!(recuperado.len(), 2);
        assert!(!esta_obsoleto(), "al leer bien deja de estar obsoleto");
        invalidar();
    }

    /// Un catálogo vacío **legítimo** no es un fallo: no se marca obsoleto.
    #[test]
    fn un_catalogo_vacio_legitimo_no_es_un_fallo() {
        let _guardia = en_exclusiva();
        let vacio = con_fuente(|| Lectura::Bien(Vec::new()), false);
        assert!(vacio.is_empty());
        assert!(
            !esta_obsoleto(),
            "sin juegos instalados no hay nada que verificar: no es un error"
        );
        invalidar();
    }

    /// Actualización manual: se relee aunque no toque por tiempo.
    #[test]
    fn la_actualizacion_manual_fuerza_la_relectura() {
        let _guardia = en_exclusiva();
        contador().store(0, Ordering::SeqCst);
        let _ = con_fuente(dos_juegos_counted, false);
        // Sin forzar, la segunda no relee.
        let _ = obtener_con(dos_juegos_counted, false);
        assert_eq!(contador().load(Ordering::SeqCst), 1);
        // Forzando, sí.
        let _ = obtener_con(dos_juegos_counted, true);
        assert_eq!(contador().load(Ordering::SeqCst), 2, "la manual relee");
        invalidar();
    }

    /// Desactivar el observador no debe provocar lecturas: la caché no se toca.
    #[test]
    fn sin_observador_activo_no_se_lee_nada() {
        let _guardia = en_exclusiva();
        let _ = con_fuente(dos_juegos_counted, false);
        let antes = contador().load(Ordering::SeqCst);
        // Simula pasadas con el observador apagado: nadie llama a `obtener`.
        assert_eq!(contador().load(Ordering::SeqCst), antes);
        invalidar();
    }

    /// Tras invalidar, la siguiente llamada relee de verdad.
    #[test]
    fn invalidar_obliga_a_releer() {
        let _guardia = en_exclusiva();
        contador().store(0, Ordering::SeqCst);
        let _ = con_fuente(dos_juegos_counted, false);
        assert_eq!(contador().load(Ordering::SeqCst), 1);
        invalidar();
        let _ = obtener_con(dos_juegos_counted, false);
        assert_eq!(
            contador().load(Ordering::SeqCst),
            2,
            "se releyó tras invalidar"
        );
        invalidar();
    }
}
