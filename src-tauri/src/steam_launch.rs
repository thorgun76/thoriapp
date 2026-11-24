//! Lectura de las opciones de lanzamiento que Steam tiene configuradas.
//!
//! ## Qué hace y qué no
//!
//! Steam guarda las opciones de cada juego en `userdata/<id>/config/localconfig.vdf`:
//!
//! ```text
//! "UserLocalConfigStore"
//! {
//!     "Software"
//!     {
//!         "Valve"
//!         {
//!             "Steam"
//!             {
//!                 "apps"
//!                 {
//!                     "1623730"
//!                     {
//!                         "LaunchOptions"    "mesa_glthread=true mangohud %command%"
//!                     }
//!                 }
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! Este módulo **solo lee**. No escribe nada en Steam, nunca. El motivo está en la memoria
//! del proyecto: Steam reescribe `LaunchOptions` al cerrarse y pisaba lo que escribíamos, así
//! que espiar el fichero y escribir en él garantizaría perder el cambio del usuario en la
//! siguiente partida. Importar en Thoriapp (copiar el texto a su propio editor) sí es fiable.
//!
//! ## Lo que se conserva
//!
//! El valor se devuelve **tal cual está en el fichero**, sin tokenizar, sin reordenar y sin
//! quitar `mangohud` ni los wrappers. Es la única forma de que «lo que Steam tiene» y «lo
//! que Thoriapp registra en una grabación» coincidan carácter a carácter.

use serde::Serialize;
use std::path::PathBuf;

/// Opciones de lanzamiento encontradas en Steam para un juego.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OpcionesSteam {
    /// Texto exacto de `LaunchOptions`. `""` cuando Steam la tiene vacía o no aparece:
    /// en ese caso Steam usa el ejecutable del juego sin nada más.
    pub opciones: String,
    /// De qué cuenta salen (`userdata/<id>`), para poder decirlo en la interfaz.
    pub cuenta: String,
}

/// Todas las carpetas `userdata` de todas las instalaciones de Steam del sistema.
///
/// Hay tres rutas habituales (nativa, `~/.steam/steam` y el Flatpak) y puede haber varias
/// cuentas en cada una. Se devuelven todas: el juego puede estar en cualquiera.
fn raices_steam() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let mut raices = vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ];
    // Respeta `STEAM_COMPAT_CLIENT_INSTALL_PATH` si está definida (Steam Deck, Flatpak).
    if let Ok(p) = std::env::var("STEAM_COMPAT_CLIENT_INSTALL_PATH") {
        if !p.is_empty() {
            raices.insert(0, PathBuf::from(p));
        }
    }
    raices.sort();
    raices.dedup();
    raices
}

/// Ids de cuenta que tienen un `localconfig.vdf`, con su ruta.
fn cuentas() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for raiz in raices_steam() {
        let userdata = raiz.join("userdata");
        let Ok(rd) = std::fs::read_dir(&userdata) else { continue };
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().to_string();
            // `0` es la cuenta anónima: no tiene configuración real.
            if id == "0" || !id.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let cfg = e.path().join("config/localconfig.vdf");
            if cfg.is_file() {
                out.push((id, cfg));
            }
        }
    }
    out
}

/// Extrae el bloque `"<appid>" { ... }` dentro de la sección `apps` del fichero.
///
/// No se usa un parser de VDF completo a propósito: `localconfig.vdf` puede pesar varios
/// megabytes y trae claves repetidas, escapes y anidamiento arbitrario. Un recorrido de
/// llaves con profundidad hace falta igualmente para no confundir el `"12345"` de `apps`
/// con un `"12345"` que aparezca en otra sección, y es exactamente lo que hay aquí.
///
/// Devuelve el texto del bloque (sin las llaves exteriores).
fn bloque_de_app(texto: &str, appid: u32) -> Option<String> {
    let objetivo = appid.to_string();
    let mut profundidad = 0usize;
    let mut en_apps = false;
    // Profundidad a la que está abierta la sección `apps`.
    let mut prof_apps = 0usize;
    let mut prof_bloque: Option<usize> = None;
    let mut inicio = 0usize;

    let mut resto = texto;
    let mut offset = 0usize;

    while let Some(pos) = siguiente_token(resto) {
        let (ini, fin, es_llave_abre, es_llave_cierra, comilla) = pos;
        let token_ini = offset + ini;
        let token_fin = offset + fin;

        if es_llave_abre {
            profundidad += 1;
            resto = &resto[fin..];
            offset = token_fin;
            continue;
        }
        if es_llave_cierra {
            // ¿Se cierra el bloque del appid que estábamos leyendo?
            if let Some(p) = prof_bloque {
                if profundidad == p {
                    let contenido = &texto[inicio..token_ini];
                    return Some(contenido.to_string());
                }
            }
            if en_apps && profundidad == prof_apps {
                en_apps = false;
            }
            profundidad = profundidad.saturating_sub(1);
            resto = &resto[fin..];
            offset = token_fin;
            continue;
        }

        if let Some(clave) = comilla {
            // ¿Abre la sección `apps`? La clave va seguida de `{`.
            if clave == "apps" {
                let tras = texto[token_fin..].trim_start();
                if tras.starts_with('{') {
                    en_apps = true;
                    prof_apps = profundidad + 1;
                }
            } else if en_apps && profundidad == prof_apps && clave == objetivo {
                let tras = texto[token_fin..].trim_start();
                if tras.starts_with('{') {
                    // El contenido empieza tras la llave de apertura.
                    let pos_llave = token_fin + (texto[token_fin..].len() - tras.len()) + 1;
                    inicio = pos_llave;
                    prof_bloque = Some(profundidad + 1);
                }
            }
        }

        resto = &resto[fin..];
        offset = token_fin;
    }
    None
}

/// Siguiente token significativo del VDF: `{`, `}` o una clave entre comillas.
///
/// Devuelve `(inicio, fin, es_abre, es_cierra, clave)` en índices del `str` recibido.
fn siguiente_token(s: &str) -> Option<(usize, usize, bool, bool, Option<String>)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => return Some((i, i + 1, true, false, None)),
            b'}' => return Some((i, i + 1, false, true, None)),
            b'"' => {
                let mut j = i + 1;
                let mut clave = String::new();
                while j < bytes.len() {
                    if bytes[j] == b'\\' && j + 1 < bytes.len() {
                        clave.push(bytes[j + 1] as char);
                        j += 2;
                        continue;
                    }
                    if bytes[j] == b'"' {
                        return Some((i, j + 1, false, false, Some(clave)));
                    }
                    clave.push(bytes[j] as char);
                    j += 1;
                }
                return None;
            }
            _ => i += 1,
        }
    }
    None
}

/// Valor de `LaunchOptions` dentro del bloque de un juego.
fn launch_options_en(bloque: &str) -> Option<String> {
    let mut resto = bloque;
    let mut offset = 0usize;
    while let Some((ini, fin, abre, cierra, clave)) = siguiente_token(resto) {
        let _ = (abre, cierra);
        if let Some(k) = clave {
            if k.eq_ignore_ascii_case("LaunchOptions") {
                // El valor es la siguiente cadena entre comillas.
                let tras = &resto[fin..];
                if let Some(p) = tras.find('"') {
                    let mut j = p + 1;
                    let b = tras.as_bytes();
                    let mut valor = String::new();
                    while j < b.len() {
                        if b[j] == b'\\' && j + 1 < b.len() {
                            valor.push(b[j + 1] as char);
                            j += 2;
                            continue;
                        }
                        if b[j] == b'"' {
                            return Some(valor);
                        }
                        valor.push(b[j] as char);
                        j += 1;
                    }
                }
            }
        }
        resto = &resto[fin..];
        offset += fin;
        let _ = (ini, offset);
    }
    None
}

/// Opciones de lanzamiento de `appid` según Steam, o `None` si no se puede saber.
///
/// `None` significa «no se puede leer»: no hay Steam, no hay `localconfig.vdf`, o el juego
/// no aparece. Es importante no devolver `""` en ese caso, porque `""` significaría «Steam
/// las tiene vacías», que es una afirmación distinta.
pub fn leer_opciones(appid: u32) -> Option<OpcionesSteam> {
    leer_en(&cuentas(), appid)
}

/// Igual que [`leer_opciones`], pero con las cuentas ya resueltas (para tests).
fn leer_en(cuentas: &[(String, PathBuf)], appid: u32) -> Option<OpcionesSteam> {
    for (cuenta, ruta) in cuentas {
        let Ok(texto) = std::fs::read_to_string(ruta) else { continue };
        let Some(bloque) = bloque_de_app(&texto, appid) else { continue };
        // El juego aparece en esta cuenta. Si no tiene `LaunchOptions`, Steam la tiene
        // vacía para él: eso es una respuesta, no un fallo.
        return Some(OpcionesSteam {
            opciones: launch_options_en(&bloque).unwrap_or_default(),
            cuenta: cuenta.clone(),
        });
    }
    None
}

/// Nombre del fichero de una cuenta concreta (para tests).
#[cfg(test)]
fn ruta_config(dir: &std::path::Path, cuenta: &str) -> PathBuf {
    dir.join("userdata").join(cuenta).join("config/localconfig.vdf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// El nombre del CSV de MangoHud es el del ejecutable; el de Steam, el comercial.
    #[test]
    fn norm_nombre_iguala_ejecutable_y_nombre_comercial() {
        assert_eq!(norm_nombre("Palworld.exe"), norm_nombre("Palworld"));
        assert_eq!(norm_nombre("Game.x86_64"), norm_nombre("game"));
        assert_eq!(norm_nombre("  Celeste  "), "celeste");
    }

    /// Con AppID no se busca por nombre: si `localconfig` no tiene el juego, no hay dato.
    #[test]
    fn leer_opciones_juego_sin_appid_resoluble_devuelve_none() {
        // Nombre que no está en ninguna biblioteca de este equipo.
        assert_eq!(
            leer_opciones_juego(None, Some("JuegoQueNoExiste__xyz")),
            None
        );
        assert_eq!(leer_opciones_juego(None, None), None);
        // AppID 0 no se usa como identidad: es «no se sabe», no un juego real.
        assert_eq!(leer_opciones_juego(Some(0), None), None);
    }

    /// Dos juegos instalados que encajan igual de bien no se adivinan.
    ///
    /// El nombre del CSV puede ser prefijo del comercial, así que si Steam tiene
    /// "Palworld" y "Palworld-Win64" el nombre "Palworld-Win64-Shipping" encaja con los
    /// dos. Atribuir la prueba al juego equivocado es peor que no tener el dato, y por eso
    /// se exige coincidencia única.
    #[test]
    fn nombre_ambiguo_no_se_resuelve() {
        let base = std::env::temp_dir().join(format!("thoriapp_ambi_{}", std::process::id()));
        let steam = base.join(".local/share/Steam");
        fs::create_dir_all(steam.join("steamapps")).unwrap();
        for (id, nombre) in [("1", "Palworld"), ("2", "Palworld-Win64")] {
            fs::write(
                steam.join(format!("steamapps/appmanifest_{id}.acf")),
                format!(
                    "\"AppState\"\n{{\n\t\"appid\"\t\t\"{id}\"\n\t\"name\"\t\t\"{nombre}\"\n}}\n"
                ),
            )
            .unwrap();
        }

        // `HOME` es global al proceso: el guardián evita que otro test la reescriba a la
        // vez y que este lea su biblioteca de Steam en lugar de la temporal.
        let r = {
            let _entorno = crate::test_entorno::HomeTemporal::nuevo(&base);
            appid_por_nombre("Palworld-Win64-Shipping")
        };
        let _ = fs::remove_dir_all(&base);

        assert_eq!(r, None);
    }

    /// Cadena completa: CSV con AppID 0 y nombre de ejecutable → Steam → argumentos.    ///
    /// Es el caso real que dejaba las pruebas con `%command%`: MangoHud nombra el fichero
    /// `Palworld-Win64-Shipping` y el CSV no trae AppID, así que sin resolver el nombre el
    /// guardado no tenía forma de saber qué se había ejecutado.
    #[test]
    fn resuelve_por_nombre_cuando_el_appid_es_cero() {
        // HOME propio para no depender de la Steam real del equipo. Y un guardián, porque
        // `HOME` es global al proceso y `launchers.rs` también la reescribe: sin él este
        // test fallaba uno de cada cinco arranques leyendo la biblioteca del otro test.
        let base = std::env::temp_dir().join(format!("thoriapp_steam_{}", std::process::id()));
        let steam = base.join(".local/share/Steam");
        fs::create_dir_all(steam.join("steamapps")).unwrap();
        fs::create_dir_all(steam.join("userdata/123/config")).unwrap();
        fs::write(
            steam.join("steamapps/appmanifest_1623730.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"1623730\"\n\t\"name\"\t\t\"Palworld\"\n}\n",
        )
        .unwrap();
        fs::write(
            steam.join("userdata/123/config/localconfig.vdf"),
            VDF,
        )
        .unwrap();

        let r = {
            let _entorno = crate::test_entorno::HomeTemporal::nuevo(&base);
            leer_opciones_juego(Some(0), Some("Palworld-Win64-Shipping"))
        };
        let _ = fs::remove_dir_all(&base);

        assert_eq!(
            r.map(|o| o.opciones).as_deref(),
            Some("mesa_glthread=true game-performance mangohud %command%")
        );
    }

    /// Fragmento realista de `localconfig.vdf`.
    const VDF: &str = r#"
"UserLocalConfigStore"
{
	"WebStorage"		"12345"
	"Software"
	{
		"Valve"
		{
			"Steam"
			{
				"apps"
				{
					"730"
					{
						"LaunchOptions"		"gamemoderun mangohud %command%"
					}
					"1623730"
					{
						"LaunchOptions"		"mesa_glthread=true game-performance mangohud %command%"
						"LastPlayed"		"1762088632"
					}
					"440"
					{
						"LastPlayed"		"1762000000"
					}
				}
				"RecentApps"
				{
					"730"		"1"
				}
			}
		}
	}
}
"#;

    #[test]
    fn lee_las_opciones_del_juego_correcto() {
        let bloque = bloque_de_app(VDF, 1623730).expect("debería encontrar el bloque");
        assert_eq!(
            launch_options_en(&bloque).as_deref(),
            Some("mesa_glthread=true game-performance mangohud %command%")
        );
    }

    #[test]
    fn no_confunde_appids_de_otras_secciones() {
        // `RecentApps` también tiene "730": si el recorrido no respetara `apps`, el
        // bloque leído podría ser el de otra sección.
        let bloque = bloque_de_app(VDF, 730).unwrap();
        assert_eq!(launch_options_en(&bloque).as_deref(), Some("gamemoderun mangohud %command%"));
    }

    #[test]
    fn un_juego_sin_launch_options_devuelve_nada_aqui() {
        let bloque = bloque_de_app(VDF, 440).unwrap();
        assert_eq!(launch_options_en(&bloque), None);
    }

    #[test]
    fn un_appid_que_no_esta_devuelve_none() {
        assert!(bloque_de_app(VDF, 999999).is_none());
    }

    #[test]
    fn el_texto_se_devuelve_tal_cual_sin_tocar() {
        let vdf = r#"
"Software" { "Valve" { "Steam" { "apps" { "1" {
    "LaunchOptions"    "DXVK_HUD=\"fps,frametimes\"  --foo=1 --bar=2 %command%"
} } } } }
"#;
        let bloque = bloque_de_app(vdf, 1).unwrap();
        let v = launch_options_en(&bloque).unwrap();
        // Comillas escapadas, dobles espacios y orden: intactos.
        assert_eq!(v, r#"DXVK_HUD="fps,frametimes"  --foo=1 --bar=2 %command%"#);
    }

    #[test]
    fn un_valor_vacio_es_una_respuesta_no_un_fallo() {
        let vdf = r#""apps" { "5" { "LaunchOptions" "" } }"#;
        let bloque = bloque_de_app(vdf, 5).unwrap();
        assert_eq!(launch_options_en(&bloque).as_deref(), Some(""));
    }

    #[test]
    fn un_fichero_corrupto_no_paniquea() {
        // Sin llaves de cierre: el recorrido debe terminar sin entrar en bucle.
        assert!(bloque_de_app("\"apps\" { \"1\" { \"LaunchOptions\" \"x\"", 1).is_none());
        assert!(bloque_de_app("", 1).is_none());
        assert!(bloque_de_app("}{", 1).is_none());
    }

    #[test]
    fn el_appid_escrito_con_otra_seccion_antes_no_rompe() {
        let vdf = r#"
"apps" { "10" { "LaunchOptions" "viejo %command%" } }
"other" { "apps" { "10" { "LaunchOptions" "no deberia ganar" } } }
"#;
        let bloque = bloque_de_app(vdf, 10).unwrap();
        // Gana el primero dentro de `apps` (que es como lo escribe Steam).
        assert_eq!(launch_options_en(&bloque).as_deref(), Some("viejo %command%"));
    }

    #[test]
    fn sin_steam_no_se_inventa_nada() {
        let tmp = std::env::temp_dir().join(format!("thoriapp-steam-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).ok();
        // No hay `userdata`, así que no hay cuentas.
        assert!(leer_en(&[], 730).is_none());
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn lee_de_la_primera_cuenta_que_tiene_el_juego() {
        let tmp = std::env::temp_dir().join(format!("thoriapp-steam2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);

        let ruta = ruta_config(&tmp, "11111");
        fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        fs::write(&ruta, VDF).unwrap();

        let cuentas = vec![("11111".to_string(), ruta)];
        let op = leer_en(&cuentas, 1623730).expect("debería leerla");
        assert_eq!(op.cuenta, "11111");
        assert!(op.opciones.contains("mesa_glthread=true"));

        // Un juego que no está en el fichero: no se inventa.
        assert!(leer_en(&cuentas, 4242).is_none());

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn un_fichero_ilegible_no_paniquea() {
        let cuenta = ("1".to_string(), PathBuf::from("/ruta/que/no/existe/localconfig.vdf"));
        assert!(leer_en(&[cuenta], 730).is_none());
    }
}

/// Nombre comparable: minúsculas y sin extensión de ejecutable.
///
/// MangoHud nombra el CSV con el **ejecutable** (`Palworld-Win64-Shipping`, `Game.exe`),
/// mientras que Steam guarda el nombre comercial ("Palworld"). Tienen que poder compararse.
fn norm_nombre(n: &str) -> String {
    let lower = n.trim().to_lowercase();
    for sufijo in [".x86_64", ".x86", ".exe", ".bin"] {
        if let Some(sin) = lower.strip_suffix(sufijo) {
            return sin.trim().to_string();
        }
    }
    lower
}

/// AppID de un juego por su nombre, buscando en la biblioteca instalada de Steam.
///
/// Hace falta porque al analizar un CSV de MangoHud el AppID llega **siempre a 0**: el
/// fichero no lo trae, solo el nombre del ejecutable. Sin resolverlo no se puede saber qué
/// opciones de Steam se usaron, que es justo el dato que la app quiere registrar.
///
/// Solo vale una coincidencia **única**: si dos juegos instalados normalizan al mismo
/// nombre, se devuelve `None` en vez de adivinar (atribuir la prueba al juego equivocado es
/// peor que no tener el dato).
pub fn appid_por_nombre(nombre: &str) -> Option<u32> {
    let objetivo = norm_nombre(nombre);
    if objetivo.is_empty() {
        return None;
    }
    // Coincidencia en cualquiera de las dos direcciones: MangoHud suele añadir el sufijo
    // del ejecutable (`Palworld-Win64-Shipping`), pero también hay nombres donde Steam
    // guarda el título completo y el fichero el corto. Se exige que el nombre más corto
    // sea **prefijo** del largo y que empiece en el mismo sitio, para no casar por una
    // palabra suelta que aparezca en medio.
    let candidatos: Vec<u32> = crate::steam::scan_library()
        .into_iter()
        .filter(|g| {
            let n = norm_nombre(&g.name);
            !n.is_empty()
                && (objetivo.starts_with(&n) || n.starts_with(&objetivo))
        })
        .map(|g| g.appid)
        .collect();
    match candidatos.as_slice() {
        [unico] => Some(*unico),
        _ => None,
    }
}

/// Opciones de Steam para un juego identificado por AppID **o** por nombre.
///
/// Es la puerta que usa la grabación de benchmarks: el AppID puede llegar a 0 (lo habitual
/// al analizar un CSV) y entonces se intenta resolver por nombre. Devuelve `None` solo
/// cuando de verdad no se puede saber nada.
pub fn leer_opciones_juego(appid: Option<u32>, nombre: Option<&str>) -> Option<OpcionesSteam> {
    if let Some(id) = appid.filter(|id| *id != 0) {
        if let Some(o) = leer_opciones(id) {
            return Some(o);
        }
    }
    let id = appid_por_nombre(nombre?)?;
    leer_opciones(id)
}

