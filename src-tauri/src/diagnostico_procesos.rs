//! Diagnóstico del observador de procesos, activable explícitamente.
//!
//! Existe porque la detección de juegos bajo Proton falló dos veces en un equipo real
//! mientras las pruebas con datos simulados pasaban. Un fallo así no se arregla adivinando:
//! hace falta ver qué ve de verdad `/proc` en el equipo donde falla.
//!
//! # Cómo se activa
//!
//! Solo con la variable de entorno `THORIAPP_DIAGNOSTICO_PROCESOS=<ruta>` apuntando a un
//! fichero escribible. Sin esa variable **no se hace nada**: ni una lectura extra, ni un
//! byte escrito. El coste en el camino normal es una comprobación de variable por pasada.
//!
//! # Qué escribe y qué no
//!
//! Escribe **una línea JSON por pasada** (o por llamada a `volcar`). Nunca el entorno
//! completo: solo las variables de la lista blanca, ya filtradas por `filtrar_environ`.
//! Nunca la línea de comandos completa: solo el nombre del ejecutable del juego y el
//! `AppId` numérico, igual que el camino de producción.
//!
//! # Qué responde
//!
//! El informe dice, por cada paso de la cadena, cuánto se examinó y por qué se descartó:
//! procesos vistos, inaccesibles, candidatos, el motivo exacto de cada descarte, la
//! identificación del grupo, y si la sesión se crea o se rechaza.

use std::io::Write;

/// Variable de entorno que activa el diagnóstico. Su valor es la ruta del fichero de salida.
pub const VARIABLE: &str = "THORIAPP_DIAGNOSTICO_PROCESOS";

/// Por qué se descartó un proceso que tenía alguna evidencia de juego.
#[derive(Debug, Clone)]
pub struct Descarte {
    pub pid: u32,
    /// Padre observado. Es lo que permite saber si un eslabón intermedio se perdió aquí.
    pub ppid: Option<u32>,
    pub comm: Option<String>,
    pub ejecutable: Option<String>,
    pub motivo: String,
    /// Cadena de ancestros observada, del padre inmediato hacia arriba.
    ///
    /// Un proceso descartado puede ser el **eslabón** que une al juego con su árbol: sin su
    /// cadena no se puede explicar por qué dos candidatos acabaron en grupos distintos. Antes
    /// solo los candidatos la llevaban y el descarte se quedaba en su `ppid` suelto.
    pub ancestros: Vec<u32>,
}

/// Lo que se pudo ver de un proceso candidato, para el informe.
#[derive(Debug, Clone, Default)]
pub struct Candidato {
    pub pid: u32,
    pub comm: Option<String>,
    pub ejecutable: Option<String>,
    pub ruta_del_juego: Option<String>,
    pub steam_app_id: Option<String>,
    pub steam_game_id: Option<String>,
    pub appid_de_argumentos: Option<u32>,
    pub bajo_proton: Option<bool>,
    pub auxiliar: bool,
    /// Carpeta que el proceso declara (`STEAM_COMPAT_INSTALL_PATH`, ya reducida).
    ///
    /// Es la **única** vía de correlación por carpeta. Sin este dato no se puede saber si un
    /// juego externo tenía la evidencia que la correlación necesita.
    pub carpeta_declarada: Option<String>,
    /// Prefijo de Wine declarado (`WINEPREFIX`), sin la marca de inicio de sesión.
    ///
    /// Es la segunda vía de correlación, y la que decide cuando el lanzador no guarda carpeta
    /// de instalación (caso real de Lutris, con `directory` vacío).
    pub wine_prefix: Option<String>,
    /// ¿El `.exe` que nombra lo **ejecuta**, o solo lo menciona como argumento?
    pub ejecuta_de_verdad: Option<bool>,
    /// Padre, tal cual lo declara `/proc`. **Observado**, no inferido.
    pub ppid: Option<u32>,
    /// Instante de arranque del proceso, ya formateado. **Observado**.
    ///
    /// Con el PID forma la identidad de **instancia de proceso**: distingue dos usos del mismo
    /// PID. No es la identidad de la ejecución del juego ni de la sesión.
    pub inicio: Option<String>,
    /// Cadena de ancestros que ve el observador, de padre inmediato hacia arriba.
    ///
    /// **Inferida** del `ppid`: es lo que el escaneo pudo reconstruir, y se corta en cuanto un
    /// ancestro no aparece en lo observado. Por eso se acompaña del motivo de su descarte, si
    /// lo hubo, para saber si la cadena se corta por un filtro nuestro o por el sistema.
    pub ancestros: Vec<u32>,
}

/// Una entrada del catálogo **efectivo** que usa el observador.
///
/// Responde a «¿el juego que tengo abierto estaba en el catálogo con el que miraba el
/// observador?». Es el dato que faltaba para distinguir «no está en el catálogo» de «está,
/// pero no se correlacionó».
#[derive(Debug, Clone)]
pub struct EntradaDelCatalogo {
    pub launcher: String,
    pub titulo: String,
    pub id_plataforma: Option<String>,
    pub install_dir: Option<String>,
    pub wine_prefix: Option<String>,
    pub exec: Option<String>,
}

/// Informe completo de una pasada.
#[derive(Debug, Clone, Default)]
pub struct Informe {
    pub procesos_vistos: usize,
    pub inaccesibles: usize,
    pub candidatos: Vec<Candidato>,
    pub descartes: Vec<Descarte>,
    pub grupos: usize,
    pub sin_visibilidad: Option<String>,
    /// Sesiones creadas o rechazadas, en texto: lo decide el servicio, no el adaptador.
    pub sesiones: Vec<String>,
    /// El catálogo **efectivo** de la pasada (no el de la biblioteca de la interfaz).
    ///
    /// Solo se rellena con el diagnóstico activo. Es la pieza que faltaba: el catálogo de la
    /// biblioteca (lo que ve el usuario) y el del observador (lo que usa para identificar)
    /// son dos listas distintas, y nunca se comprobaba si coincidían.
    pub catalogo: Vec<EntradaDelCatalogo>,
    /// Lo que decidió la correlación, grupo a grupo.
    pub correlaciones: Vec<String>,
    /// El directorio de trabajo del representante de cada grupo, si se pudo leer.
    ///
    /// Es el dato que faltaba para entender por qué Starsector no correlacionaba: el
    /// diagnóstico decía `categoria=None` y no enseñaba el `cwd`, así que no se podía
    /// distinguir «no se leyó el cwd» de «se leyó y no coincidió con nada del catálogo».
    /// Solo se rellena con el diagnóstico activo, que es opt-in y escribe en un fichero que
    /// el usuario elige.
    pub directorios_de_trabajo: Vec<String>,
    /// Cómo quedó cada proceso tras clasificar y agrupar: papel, elegibilidad, raíz calculada,
    /// grupo y representante. Se separa de `candidatos` porque aquí ya hay **decisiones** del
    /// motor, no datos leídos de `/proc`.
    ///
    /// Cada entrada declara qué es observado y qué es inferido, para no presentar una
    /// deducción como si se hubiera visto.
    pub clasificacion: Vec<Clasificado>,
    /// Qué pasó con cada sesión: creada, mantenida o cerrada, y por qué.
    pub ciclo_de_sesiones: Vec<String>,
    /// Recorrido del ejecutable desde el adaptador hasta el catálogo efectivo.
    ///
    /// Existe porque un juego puede estar bien leído por su adaptador y aun así llegar al
    /// observador sin `exec`: sin este rastro no se puede saber en qué paso se perdió.
    pub recorrido_del_ejecutable: Vec<String>,
}

/// Lo que el motor decidió para un proceso concreto.
#[derive(Debug, Clone, Default)]
pub struct Clasificado {
    pub pid: u32,
    pub ppid: Option<u32>,
    /// Identidad de instancia (`pid@starttime`), tal como la usa el servicio.
    pub identidad_de_instancia: Option<String>,
    /// Papel en el lanzamiento (`EjecutableDelJuego`, `Ancla`, `Envoltorio`…).
    ///
    /// **Inferido** de la estructura del árbol, no leído de `/proc`.
    pub papel: Option<String>,
    /// ¿Podría representar al juego? Es una decisión distinta del papel.
    pub elegible_como_representante: bool,
    /// Raíz calculada **ahora** por `agrupar_por_padre`.
    pub raiz_calculada: Option<u32>,
    /// Índice del grupo al que fue a parar, si entró en alguno.
    pub grupo: Option<usize>,
    /// ¿Fue el representante de su grupo?
    pub representante: bool,
    /// Identidad del juego con la que se correlacionó, si se correlacionó.
    pub identidad_correlacionada: Option<String>,
}

/// Ruta del fichero de diagnóstico, si está activado.
pub fn ruta_activa() -> Option<String> {
    std::env::var(VARIABLE)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// ¿Está activado? Una sola comprobación de variable, sin coste apreciable.
pub fn activo() -> bool {
    ruta_activa().is_some()
}

/// Escribe una línea JSON con el informe, si el diagnóstico está activo.
///
/// Es tolerante a fallos a propósito: un diagnóstico que rompe la aplicación es peor que
/// no tener diagnóstico. Si no se puede escribir, se avisa por `stderr` y se sigue.
pub fn volcar(informe: &Informe, nota: &str) {
    let Some(ruta) = ruta_activa() else {
        return;
    };
    let linea = a_json(informe, nota);
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ruta)
    {
        Ok(mut f) => {
            if let Err(e) = writeln!(f, "{}", linea) {
                eprintln!("[diagnostico] no se pudo escribir en {}: {}", ruta, e);
            }
        }
        Err(e) => {
            eprintln!("[diagnostico] no se pudo abrir {}: {}", ruta, e);
        }
    }
}

/// Serialización JSON a mano.
///
/// No se usa `serde` para no arrastrar el modelo completo: esto es un volcado de
/// diagnóstico con campos fijos, y escribirlo a mano evita exponer tipos internos.
fn a_json(inf: &Informe, nota: &str) -> String {
    let mut s = String::with_capacity(512);
    s.push('{');
    s.push_str(&format!("\"nota\":{},", esc(nota)));
    s.push_str(&format!("\"procesos_vistos\":{},", inf.procesos_vistos));
    s.push_str(&format!("\"inaccesibles\":{},", inf.inaccesibles));
    s.push_str(&format!("\"grupos\":{},", inf.grupos));
    s.push_str(&format!(
        "\"sin_visibilidad\":{},",
        inf.sin_visibilidad
            .as_deref()
            .map(esc)
            .unwrap_or_else(|| "null".into())
    ));

    s.push_str("\"candidatos\":[");
    for (i, c) in inf.candidatos.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        // Cada clave se concatena por separado: así no se cuela una barra invertida de
        // continuación de línea dentro del JSON, que era lo que lo partía en dos líneas.
        let mut c_obj = String::from("{");
        c_obj.push_str(&format!("\"pid\":{},", c.pid));
        c_obj.push_str(&format!("\"ppid\":{},", num(c.ppid.map(|v| v as u64))));
        c_obj.push_str(&format!("\"comm\":{},", opt(&c.comm)));
        c_obj.push_str(&format!("\"ejecutable\":{},", opt(&c.ejecutable)));
        c_obj.push_str(&format!("\"inicio\":{},", opt(&c.inicio)));
        c_obj.push_str(&format!(
            "\"identidad_de_instancia\":{},\"ancestros\":{},",
            opt(&identidad_de_instancia(&c.pid, c.inicio.as_deref())),
            lista_de_pid(&c.ancestros)
        ));
        c_obj.push_str(&format!("\"ruta_del_juego\":{},", opt(&c.ruta_del_juego)));
        c_obj.push_str(&format!("\"steam_app_id\":{},", opt(&c.steam_app_id)));
        c_obj.push_str(&format!("\"steam_game_id\":{},", opt(&c.steam_game_id)));
        c_obj.push_str(&format!(
            "\"appid_de_argumentos\":{},",
            num(c.appid_de_argumentos.map(|v| v as u64))
        ));
        c_obj.push_str(&format!(
            "\"bajo_proton\":{},\"auxiliar\":{},",
            booleano(c.bajo_proton),
            c.auxiliar
        ));
        c_obj.push_str(&format!(
            "\"carpeta_declarada\":{},\"wine_prefix\":{},",
            opt(&c.carpeta_declarada),
            opt(&c.wine_prefix)
        ));
        c_obj.push_str(&format!(
            "\"ejecuta_de_verdad\":{}",
            booleano(c.ejecuta_de_verdad)
        ));
        c_obj.push('}');
        s.push_str(&c_obj);
    }
    s.push_str("],");

    s.push_str("\"descartes\":[");
    for (i, d) in inf.descartes.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"pid\":{},\"ppid\":{},\"comm\":{},\"ejecutable\":{},\"motivo\":{},\"ancestros\":{}}}",
            d.pid,
            num(d.ppid.map(|v| v as u64)),
            opt(&d.comm),
            opt(&d.ejecutable),
            esc(&d.motivo),
            lista_de_pid(&d.ancestros)
        ));
    }
    s.push_str("],");

    s.push_str("\"sesiones\":[");
    for (i, x) in inf.sesiones.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&esc(x));
    }
    s.push_str("],");

    // Catálogo efectivo del observador y decisión de correlación: lo que faltaba para poder
    // separar «no lo conozco» de «lo conozco y aun así no lo identifiqué».
    s.push_str("\"catalogo\":[");
    for (i, e) in inf.catalogo.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        // Sin saltos de línea dentro del objeto: una línea JSON por registro.
        let mut e_obj = String::from("{");
        e_obj.push_str(&format!("\"launcher\":{},", esc(&e.launcher)));
        e_obj.push_str(&format!("\"titulo\":{},", esc(&e.titulo)));
        e_obj.push_str(&format!("\"id_plataforma\":{},", opt(&e.id_plataforma)));
        e_obj.push_str(&format!("\"install_dir\":{},", opt(&e.install_dir)));
        e_obj.push_str(&format!("\"wine_prefix\":{},", opt(&e.wine_prefix)));
        e_obj.push_str(&format!("\"exec\":{}", opt(&e.exec)));
        e_obj.push('}');
        s.push_str(&e_obj);
    }
    s.push_str("],");

    s.push_str("\"correlaciones\":[");
    for (i, x) in inf.correlaciones.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&esc(x));
    }
    s.push_str("],");

    s.push_str("\"directorios_de_trabajo\":[");
    for (i, x) in inf.directorios_de_trabajo.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&esc(x));
    }
    s.push_str("],");

    // Decisiones del motor, separadas de lo meramente observado.
    s.push_str("\"clasificacion\":[");
    for (i, c) in inf.clasificacion.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"pid\":{},\"ppid\":{},\"identidad_de_instancia\":{},\"papel\":{},\
             \"elegible_como_representante\":{},\"raiz_calculada\":{},\"grupo\":{},\
             \"representante\":{},\"identidad_correlacionada\":{}}}",
            c.pid,
            num(c.ppid.map(|v| v as u64)),
            opt(&c.identidad_de_instancia),
            opt(&c.papel),
            c.elegible_como_representante,
            num(c.raiz_calculada.map(|v| v as u64)),
            num(c.grupo.map(|v| v as u64)),
            c.representante,
            opt(&c.identidad_correlacionada)
        ));
    }
    s.push_str("],");

    s.push_str("\"ciclo_de_sesiones\":[");
    for (i, x) in inf.ciclo_de_sesiones.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&esc(x));
    }
    s.push_str("],");

    s.push_str("\"recorrido_del_ejecutable\":[");
    for (i, x) in inf.recorrido_del_ejecutable.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&esc(x));
    }
    s.push_str("],");

    s.push_str("\"directorios_de_trabajo\":[");
    for (i, x) in inf.directorios_de_trabajo.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&esc(x));
    }
    s.push(']');
    s.push('}');
    s
}

/// Número JSON o `null`. Los booleanos y números se escriben sin comillas.
fn num(v: Option<u64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "null".into())
}

/// Booleano JSON o `null` (desconocido, que no es lo mismo que `false`).
fn booleano(v: Option<bool>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "null".into())
}

/// Lista JSON de PIDs.
fn lista_de_pid(v: &[u32]) -> String {
    let cuerpo: Vec<String> = v.iter().map(|p| p.to_string()).collect();
    format!("[{}]", cuerpo.join(","))
}

/// Identidad de instancia de proceso: `pid@starttime`.
///
/// Es la misma forma que usa el servicio para decidir continuidad, y por eso se muestra aquí:
/// sirve para comprobar si dos observaciones son el mismo proceso. **No** es la identidad de
/// la ejecución del juego ni de la sesión.
fn identidad_de_instancia(pid: &u32, inicio: Option<&str>) -> Option<String> {
    Some(match inicio {
        Some(t) => format!("{}@{}", pid, t),
        None => format!("{}@desconocido", pid),
    })
}

fn opt(v: &Option<String>) -> String {
    v.as_deref().map(esc).unwrap_or_else(|| "null".into())
}

/// Escapa una cadena para JSON, incluidas comillas, barras y controles.
fn esc(v: &str) -> String {
    let mut o = String::with_capacity(v.len() + 2);
    o.push('"');
    for c in v.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Anota un paso del recorrido de un ejecutable desde su adaptador hasta el catálogo.
///
/// No hace nada si el diagnóstico está apagado: sin la variable no hay trabajo ni memoria
/// reservada. Se guarda **un contador de pasos**, no el recorrido entero, para que un
/// lanzador con muchos juegos no acumule texto indefinidamente.
pub fn rastro_del_ejecutable(paso: String) {
    if !activo() {
        return;
    }
    let mut guardado = match rastro().lock() {
        Ok(g) => g,
        Err(envenenado) => envenenado.into_inner(),
    };
    if guardado.len() < 512 {
        guardado.push(paso);
    }
}

/// Recoge y vacía el rastro acumulado desde el último volcado.
///
/// Se vacía al recogerlo para que cada línea del fichero cuente solo lo de su pasada: si no,
/// el rastro crecería y un registro lejano parecería contemporáneo de otro.
pub fn tomar_rastro_del_ejecutable() -> Vec<String> {
    let mut guardado = match rastro().lock() {
        Ok(g) => g,
        Err(envenenado) => envenenado.into_inner(),
    };
    std::mem::take(&mut *guardado)
}

static RASTRO: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();

fn rastro() -> &'static std::sync::Mutex<Vec<String>> {
    RASTRO.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::sessions::{EvidenciaAppId, RasgosDeProceso};

    fn rasgos(pid: u32, comm: &str, exe: &str) -> RasgosDeProceso {
        RasgosDeProceso {
            pid,
            ejecutable: Some(comm.into()),
            ruta_ejecutable: Some(exe.into()),
            ..Default::default()
        }
    }

    #[test]
    fn sin_la_variable_no_hay_ruta_activa() {
        // No se puede garantizar el estado del entorno en otros tests, así que se comprueba
        // la función pura: una ruta vacía no activa nada.
        std::env::remove_var(VARIABLE);
        assert!(ruta_activa().is_none());
        assert!(!activo());
    }

    #[test]
    fn el_json_escapa_y_no_inventa_nulos() {
        let inf = Informe::default();
        let j = a_json(&inf, "prueba");
        assert!(j.contains("\"nota\":\"prueba\""));
        assert!(j.contains("\"sin_visibilidad\":null"));
        assert!(j.contains("\"candidatos\":[]"));
        assert!(j.contains("\"descartes\":[]"));
    }

    /// El fallo real: un objeto del catálogo se serializaba con un salto de línea dentro, así
    /// que cada pasada escribía JSON partido en varias líneas y los lectores que van línea a
    /// línea perdían registros enteros. Una línea, un registro, siempre.
    #[test]
    fn cada_registro_es_una_sola_linea_json() {
        let mut inf = Informe::default();
        inf.catalogo
            .push(crate::diagnostico_procesos::EntradaDelCatalogo {
                launcher: "heroic".into(),
                titulo: "Tattoo Tycoon".into(),
                id_plataforma: Some("epic-e75733d7dbb0463f9942ed6a2cd4acfb".into()),
                install_dir: Some("/home/jugador/Games/Heroic/TattooTycoon".into()),
                wine_prefix: Some("/home/jugador/Games/Heroic/Prefixes/default/Tattoo Tycoon".into()),
                exec: Some("/home/jugador/Games/Heroic/TattooTycoon/Tattoo Tycoon.exe".into()),
            });
        inf.clasificacion.push(Clasificado {
            pid: 209750,
            ppid: Some(207985),
            identidad_de_instancia: Some("209750@2026-11-26 10:51:45".into()),
            papel: Some("EjecutableDelJuego".into()),
            elegible_como_representante: true,
            raiz_calculada: Some(207985),
            grupo: Some(0),
            representante: true,
            identidad_correlacionada: Some("Tattoo Tycoon".into()),
        });
        inf.ciclo_de_sesiones
            .push("CREADA ses_x por cambio de representante".into());

        let j = a_json(&inf, "prueba");
        assert!(!j.contains('\n'), "el JSON no puede llevar saltos de línea");
        // Y con dos objetos seguidos se puede leer sin `raw_decode`.
        let dos = format!("{}\n{}\n", j, j);
        for linea in dos.lines() {
            let v: serde_json::Value =
                serde_json::from_str(linea).expect("cada línea debe ser JSON válido");
            assert_eq!(v["nota"], "prueba");
        }
    }

    /// La identidad de instancia distingue dos usos del mismo PID, y lo dice cuando falta el
    /// instante en vez de inventarlo.
    #[test]
    fn la_identidad_de_instancia_usa_pid_y_arranque() {
        assert_eq!(
            identidad_de_instancia(&209750, Some("2026-11-26 10:51:45")).as_deref(),
            Some("209750@2026-11-26 10:51:45")
        );
        assert_eq!(
            identidad_de_instancia(&209750, None).as_deref(),
            Some("209750@desconocido")
        );
        assert_ne!(
            identidad_de_instancia(&209750, Some("2026-11-26 10:51:45")),
            identidad_de_instancia(&209750, Some("2026-11-26 11:02:00"))
        );
    }

    /// La cadena de ancestros se corta en lo observado: no rellena el eslabón que no se vio.
    #[test]
    fn la_cadena_de_ancestros_no_inventa_eslabones() {
        use std::collections::HashMap;
        // 300 -> 200 -> 100 -> 1 (no observado)
        let mut padres: HashMap<u32, Option<u32>> = HashMap::new();
        padres.insert(300, Some(200));
        padres.insert(200, Some(100));
        padres.insert(100, Some(1));
        assert_eq!(
            crate::proceso_linux::ancestros_de(300, &padres),
            vec![200, 100, 1]
        );
        // Un proceso huérfano no tiene cadena.
        padres.insert(400, Some(1));
        assert_eq!(crate::proceso_linux::ancestros_de(400, &padres), vec![1]);
        // Un PID que no está en lo observado no tiene cadena inventada.
        assert!(crate::proceso_linux::ancestros_de(999, &padres).is_empty());
    }

    /// Un ciclo de PPID no puede colgar el escaneo.
    #[test]
    fn la_cadena_de_ancestros_corta_ciclos() {
        use std::collections::HashMap;
        let mut padres: HashMap<u32, Option<u32>> = HashMap::new();
        padres.insert(10, Some(11));
        padres.insert(11, Some(10));
        let cadena = crate::proceso_linux::ancestros_de(10, &padres);
        assert!(cadena.len() <= 2, "cadena no acotada: {:?}", cadena);
    }

    #[test]
    fn el_json_escapa_comillas_y_barras() {
        assert_eq!(esc("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(esc("linea\nnueva"), "\"linea\\nnueva\"");
    }

    #[test]
    fn los_candidatos_llevan_sus_datos_sin_entorno_completo() {
        let mut r = rasgos(20, "GameThread", "/p/wine64-preloader");
        r.appid_del_entorno = Some(EvidenciaAppId {
            steam_app_id: Some("1623730".into()),
            steam_game_id: Some("1623730".into()),
        });
        let mut inf = Informe::default();
        inf.candidatos.push(Candidato {
            pid: r.pid,
            comm: r.ejecutable.clone(),
            ejecutable: r.ruta_ejecutable.clone(),
            ruta_del_juego: r.ruta_del_juego.clone(),
            steam_app_id: r
                .appid_del_entorno
                .as_ref()
                .and_then(|e| e.steam_app_id.clone()),
            steam_game_id: r
                .appid_del_entorno
                .as_ref()
                .and_then(|e| e.steam_game_id.clone()),
            appid_de_argumentos: r.appid_de_argumentos,
            bajo_proton: r.bajo_proton,
            auxiliar: r.auxiliar,
            carpeta_declarada: r.carpeta_declarada.clone(),
            ppid: None,
            inicio: None,
            ancestros: Vec::new(),
            wine_prefix: None,
            ejecuta_de_verdad: r.ejecuta_de_verdad,
        });
        assert_eq!(inf.candidatos[0].steam_app_id.as_deref(), Some("1623730"));
        let j = a_json(&inf, "t");
        assert!(j.contains("1623730"));
        // Nunca se vuelca el entorno completo ni credenciales.
        assert!(!j.contains("PATH="));
        assert!(!j.contains("TOKEN"));
    }
}
