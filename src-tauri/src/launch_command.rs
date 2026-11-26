//! Motor del comando de lanzamiento: analizar, conservar y editar sin destruir.
//!
//! ## El problema que resuelve
//!
//! «Traer a Thoriapp» copiaba el texto de Steam a `manual_command`, y `launchLine()` lo
//! devolvía antes de mirar los ajustes. Resultado: tras importar, pulsar un ajuste no
//! cambiaba nada y el resumen de arriba se vaciaba. La única salida era `Restablecer`,
//! que borraba todo.
//!
//! La tentación era mapear el comando de Steam a los toggles del catálogo. No sirve:
//! `games.json` tiene 25 valores únicos y los comandos reales llevan cosas que no están
//! ahí (`mesa_glthread=true`, `-novid`, `DXVK_ASYNC=1`…). Convertir por nombre y
//! descartar el resto sería exactamente el borrado silencioso que hay que evitar.
//!
//! ## La idea
//!
//! El comando **no se reconstruye nunca desde sus piezas**. Se parte del texto original y
//! se aplican reemplazos sobre tramos concretos ([`Span`]): lo que no se toca se copia
//! byte a byte, con sus espacios, sus comillas y su orden. Con las mismas piezas y sin
//! cambios, el resultado es el original carácter a carácter (lo comprueba
//! `sin_cambios_devuelve_el_comando_exacto`).
//!
//! ## Qué NO hace este módulo
//!
//! - **No ejecuta nada.** No hay `std::process` en el archivo: un test lo verifica.
//! - **No lee Steam.** El texto llega ya leído; aquí solo se analiza.
//! - **No conoce Tauri, React ni el catálogo de juegos.** El catálogo se pasa como
//!   `&[Definicion]`, así que el motor se puede probar sin `games.json` y sin base de datos.
//!
//! ## Cómo se usa (stateless)
//!
//! ```text
//! analizar(texto, catalogo)      -> Analisis      (una vez, al importar)
//! serializar(analisis, cambios)  -> String        (cada vez que cambia un ajuste)
//! ```
//!
//! ```text
//! // Fase 2 (UI) usará además `ajustes_activos` y `cambios_para`, que ya están probadas.
//! ```
//!
//! No hay caché ni identificadores de sesión: el `Analisis` es un valor serializable que
//! el llamador guarda y devuelve. Para un comando de lanzamiento (cientos de bytes) el
//! coste de reenviarlo es irrelevante, y se evita toda una clase de errores: caché
//! obsoleta, identificador caducado, estado del backend que no coincide con la UI.

use serde::{Deserialize, Serialize};

// ==================== MODELO ====================

/// Tramo de bytes de [`Analisis::original`].
///
/// Bytes, no caracteres: se usa para `String::replace_range`/slicing, que trabajan en
/// índices de byte. El lexer solo produce fronteras en límites de carácter UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub inicio: usize,
    pub fin: usize,
}

impl Span {
    pub fn new(inicio: usize, fin: usize) -> Self {
        Self { inicio, fin }
    }

    /// Texto de este tramo dentro de `texto`. Nunca entra en pánico por índices raros: si
    /// el span no cae en una frontera de carácter, devuelve `""` en vez de abortar.
    ///
    /// La usan los tests para comprobar que los spans apuntan al texto exacto; la UI
    /// recibe el `texto` ya resuelto en cada [`Segmento`].
    #[allow(dead_code)]
    pub fn recortar<'a>(&self, texto: &'a str) -> &'a str {
        texto.get(self.inicio..self.fin).unwrap_or("")
    }
}

/// Qué papel juega un token en la línea de Steam.
///
/// El orden importa: la variante `Orden` se usa para comparar posiciones relativas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rol {
    /// Variable de entorno (`FOO=bar`) antes de `%command%`.
    Env,
    /// Argumento suelto del lanzador que Thoriapp gestiona (`-novid`, `RADV_PERFTEST=sam`).
    ///
    /// Es un ajuste reconocido que no envuelve nada: se coloca junto al resto de argumentos
    /// del lanzador (delante del `%command%`) y se puede activar y quitar como cualquier
    /// otro. Existe para que una recomendación del catálogo efectivo sea un ajuste más y no
    /// un texto que el frontend tenga que pegar a mano.
    Arg,
    /// Envoltorio conocido (`mangohud`, `gamemoderun`…) antes de `%command%`.
    Wrapper,
    /// El marcador `%command%`.
    Marcador,
    /// Ejecutable propio entre los wrappers y el marcador (raro, pero existe:
    /// `mangohud ./juego %command%`).
    Ejecutable,
    /// Argumento propio de un wrapper: `-W 1920` en `gamescope -W 1920 -- %command%`.
    ///
    /// Nunca es del juego ni se edita. Existe porque sin él no se sabe dónde acaba el
    /// wrapper y empieza lo envuelto, y eso es justo lo que decide si un ajuste se mide a
    /// sí mismo o al juego.
    ArgWrapper,
    /// Argumento del juego, después de `%command%`. Nunca lo toca Thoriapp.
    ArgJuego,
}

/// Quién manda sobre este segmento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Gestion {
    /// Thoriapp sabe qué ajuste es y puede activarlo/desactivarlo.
    ///
    /// `clave` es el identificador de la definición del catálogo (su `value`, que es
    /// único dentro de un juego). `indice` es la posición en `rules.params`, que es lo que
    /// la UI ya usa para `enabled_params`.
    Thoriapp { clave: String, indice: usize },
    /// Se conserva tal cual: Thoriapp no lo entiende o no debe tocarlo.
    Conservado { razon: Conservado },
    /// El `%command%`.
    Marcador,
}

/// Por qué un segmento se conserva en vez de gestionarse.
///
/// Se guarda el motivo (y no solo un `bool`) para poder explicarlo en la UI: «no lo
/// conocemos» y «es ambiguo» piden acciones distintas del usuario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "motivo", rename_all = "snake_case")]
pub enum Conservado {
    /// No está en el catálogo de este juego.
    Desconocido,
    /// Está en el catálogo, pero el mismo valor define más de un ajuste y elegir uno
    /// apagaría el equivocado. Con los `indices` candidatos para que la UI pregunte.
    Ambiguo { candidatos: Vec<usize> },
    /// Repite un valor que ya aparece antes: este segundo no se gestiona para no
    /// actuar dos veces sobre lo mismo.
    Duplicado { de_span: Span },
    /// Argumento del juego: va después de `%command%`, es cosa del juego.
    ArgJuego,
    /// Hay algo cerca que impide tocar el comando con seguridad (comillas sin cerrar,
    /// más de un `%command%`…).
    NoSeguro { detalle: String },
}

/// Un token del comando, con su posición exacta en el original.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segmento {
    pub span: Span,
    /// Texto exacto del token, **con comillas si las había**.
    pub texto: String,
    /// El mismo token sin las comillas exteriores. Es lo que se compara con el catálogo.
    ///
    /// Sin esto, `"PROTON_X=1"` (con comillas) no se reconocería como el ajuste
    /// `PROTON_X=1` y se trataría como desconocido; y con esto se compararía mal el
    /// texto exacto que hay que escribir de vuelta.
    pub valor: String,
    pub rol: Rol,
    pub gestion: Gestion,
    /// `false` si el token tenía una comilla sin cerrar. Un token así no se puede
    /// reescribir sin adivinar dónde acaba.
    pub cerrado: bool,
}

/// Qué impide (o complica) editar el comando.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Aviso {
    /// No hay `%command%`. Steam no sabría qué lanzar: es una línea inválida.
    SinMarcador,
    /// Hay más de un `%command%`: no se sabe cuál es el bueno.
    MarcadorRepetido { posiciones: Vec<Span> },
    /// Comilla sin cerrar: el resto de la línea es ambiguo.
    ComillaSinCerrar { span: Span, comilla: String },
    /// El mismo valor aparece dos veces. El segundo no se gestiona.
    Duplicado { span: Span, valor: String },
    /// Dos valores distintos para la misma variable (`DXVK_HUD=fps` y `DXVK_HUD=full`).
    ///
    /// No se resuelve solo: cualquiera de los dos podría ser el que el usuario quiere.
    Contradiccion { key: String, spans: Vec<Span> },
    /// Valor que encaja con más de un ajuste del catálogo.
    Ambiguo {
        span: Span,
        valor: String,
        candidatos: Vec<usize>,
    },
    /// Hay un ajuste del catálogo activable que **no** es el mismo que el del comando
    /// aunque comparta el nombre de la variable (`PROTON_X=1` no es `PROTON_X=2`).
    ///
    /// Existe para que «coincide el nombre» no se tome por «es el mismo ajuste».
    MismoNombreDistintoValor { key: String, valor: String },
}

impl Aviso {
    /// Los avisos que impiden editar con seguridad (no solo avisar).
    pub fn impide_editar(&self) -> bool {
        matches!(
            self,
            Aviso::SinMarcador | Aviso::MarcadorRepetido { .. } | Aviso::ComillaSinCerrar { .. }
        )
    }

    /// ¿Este aviso significa que la línea no está bien escrita?
    ///
    /// Ojo: es la gramática, no la ejecutabilidad. Un duplicado o una contradicción se
    /// escriben bien y se lanzan; simplemente son ambiguos para Thoriapp.
    pub fn es_error_de_sintaxis(&self) -> bool {
        matches!(self, Aviso::ComillaSinCerrar { .. })
    }
}

/// Resultado de analizar un comando. Es lo que se guarda y se reenvía a [`serializar`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Analisis {
    /// Texto exacto de partida. Inmutable.
    pub original: String,
    pub segmentos: Vec<Segmento>,
    pub avisos: Vec<Aviso>,
    /// `false` si algún aviso impide editar; entonces la UI muestra el original y explica
    /// por qué, sin reconstruirlo nunca.
    pub editable: bool,
    /// Cuántos segmentos reconoce Thoriapp y cuántos conserva, para el mensaje de
    /// confirmación de la importación.
    pub reconocidos: usize,
    pub conservados: usize,
    /// `true` si la línea está escrita en la gramática que el motor entiende por completo.
    ///
    /// Es una dimensión aparte de `editable` y de `conservados`, y las tres no coinciden:
    ///
    /// | caso | conservable | editable | sintaxis válida |
    /// |---|---|---|---|
    /// | comando normal | sí | sí | sí |
    /// | `mangohud ./juego %command%` | sí (ejecutable propio) | sí | sí |
    /// | `FOO="sin cerrar %command%` | sí | no | **no** |
    /// | `%command% %command%` | sí | no | **no** |
    /// | `mystery-wrap %command%` | sí | sí | sí (wrapper desconocido, pero bien escrito) |
    ///
    /// La validación para lanzar es la de [`validar`], que es otra cosa más: comprueba que
    /// la línea se pueda ejecutar de verdad. Una línea con comilla sin cerrar se conserva
    /// íntegra pero **no** se ofrece como lista para lanzar.
    pub sintaxis_valida: bool,
    /// Por qué la sintaxis no es válida, si no lo es.
    pub motivo_sintaxis: Option<String>,
}

impl Analisis {
    /// Índices del catálogo que el comando ya tiene activos.
    ///
    /// La UI lo usará como estado inicial de los toggles al importar: así lo que se ve en
    /// la lista coincide con lo que hay en la línea desde el primer momento.
    ///
    /// `dead_code` hasta la fase 2, que es quien la consume; se implementa aquí para que
    /// la regla de «qué cuenta como activo» viva junto al análisis y no se duplique en TS.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ajustes_activos(&self) -> Vec<usize> {
        self.segmentos
            .iter()
            .filter_map(|s| match &s.gestion {
                Gestion::Thoriapp { indice, .. } => Some(*indice),
                _ => None,
            })
            .collect()
    }

    /// Comprueba que el análisis se sostiene: segmentos en orden, dentro del texto y con
    /// los spans apuntando al token que dicen.
    ///
    /// La UI reenvía el `Analisis` por IPC, así que al entrar hay que verificarlo: un
    /// análisis manipulado o viejo no puede hacer que se edite el comando equivocado. Se
    /// expone para que la puerta esté en el mismo sitio que las demás reglas.
    pub fn validar_coherencia(&self) -> Result<(), ErrorAnalisis> {
        analisis_coherente(self)
    }

    /// Segmentos cubiertos por un ajuste del catálogo, en orden. Es la entrada de
    /// [`cambios_para`].
    fn spans_de(&self, valor: &str) -> Vec<Span> {
        self.segmentos
            .iter()
            .filter(|s| matches!(&s.gestion, Gestion::Thoriapp { clave, .. } if clave == valor))
            .map(|s| s.span)
            .collect()
    }
}

// ==================== CATÁLOGO DE ENTRADA ====================

/// Una entrada del catálogo que el motor debe conocer.
///
/// Es una copia deliberada y mínima de `rules::Param`: el motor no depende del módulo de
/// reglas (ni de `games.json`) para poder probarse solo. `main.rs` la construye desde las
/// reglas reales, que siguen siendo la única fuente de verdad de los ajustes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definicion {
    /// Posición en `rules.params`. Es lo que usa `enabled_params` del perfil.
    pub indice: usize,
    /// Texto del parámetro tal cual va en el comando (`mangohud`, `PROTON_X=1`).
    pub valor: String,
    /// `env`, `wrapper` o `config`. Los `config` no van en el comando y no se analizan.
    pub kind: String,
    /// Solo tiene efecto bajo Proton/Wine (`PROTON_*`, `WINE_*`, `DXVK_*`…).
    ///
    /// Va aquí, y no se deduce en cada sitio, para que la interfaz no tenga que mantener
    /// su propia lista de prefijos: la que tenía en TypeScript se dejó fuera `WINE_*` y
    /// pintaba esas tarjetas como aplicables en juegos nativos.
    pub proton_only: bool,
}

/// Tipo de token que corresponde a una definición, según su `kind`.
fn rol_de_definicion(kind: &str) -> Option<Rol> {
    match kind {
        "env" => Some(Rol::Env),
        "wrapper" => Some(Rol::Wrapper),
        // Un argumento suelto del lanzador (`-novid`, `RADV_PERFTEST=sam` cuando se ofrece
        // como recomendación). No envuelve nada ni ejecuta nada: se coloca al final, junto a
        // los argumentos del juego, y así una recomendación puede activarse y quitarse como
        // cualquier otro ajuste.
        "arg" => Some(Rol::Arg),
        // `config` es un ajuste dentro del juego («usa FSR en Calidad»): su texto lleva
        // espacios y nunca aparece en la línea de lanzamiento.
        _ => None,
    }
}

// ==================== LEXER ====================

/// Un token del lexer, antes de clasificarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    span: Span,
    /// Texto con comillas.
    texto: String,
    /// Texto sin las comillas exteriores.
    valor: String,
    /// `false` si quedó una comilla abierta.
    cerrado: bool,
}

/// ¿Es un espacio en blanco para el separador de tokens?
///
/// Se usa `char::is_whitespace`, que cubre espacio, tabulador y los espacios raros que se
/// cuelan al copiar y pegar. **No** se separa por `\n`: un salto de línea es una segunda
/// orden y `validar` lo rechaza antes de llegar aquí.
fn es_separador(c: char) -> bool {
    c.is_whitespace() && c != '\n' && c != '\r'
}

/// Parte el comando en tokens respetando comillas y escapes, **sin perder posiciones**.
///
/// ## Por qué no `split_whitespace`
///
/// `env "FOO=a b" mangohud %command%` dividido por espacios da 5 tokens y tres son basura;
/// los espacios dentro de comillas son parte del valor. Y para poder editar sin destruir
/// hace falta el offset de cada token, que `split` no da.
///
/// ## Reglas
///
/// - `"` y `'` agrupan; el separador dentro de ellas no separa.
/// - `\` escapa el carácter siguiente (fuera y dentro de comillas dobles), así que
///   `"a \"b\""` es un token cerrado con un valor que contiene comillas.
/// - Un token puede mezclar tramos entrecomillados y sueltos (`--x="a b"`), que es como
///   los construye mucha gente y como los guarda Steam.
/// - Una comilla sin cerrar no aborta: se marca `cerrado: false` y se consume hasta el
///   final. El análisis lo convierte en un aviso que bloquea la edición.
///
/// Es una función pura sobre bytes para no romper UTF-8 a mitad de carácter.
fn tokenizar(texto: &str) -> Vec<Token> {
    // Se itera por caracteres (`char_indices`) y no por bytes: un byte suelto de un
    // acento o de un emoji no es un `char` válido y se corrompería al copiarlo.
    let mut tokens = Vec::new();
    // Posición en bytes donde empieza el siguiente carácter pendiente.
    let mut i = 0_usize;

    while let Some((_, c)) = texto[i..].char_indices().next() {
        if es_separador(c) {
            i += c.len_utf8();
            continue;
        }

        let inicio = i;
        let mut valor = String::new();
        let mut comilla_abierta: Option<char> = None;
        let mut cerrado = true;

        while let Some((_, c)) = texto[i..].char_indices().next() {
            if let Some(q) = comilla_abierta {
                // Dentro de comillas SIMPLES no hay escapes: todo es literal hasta la
                // comilla de cierre. Es la semántica del shell y la que entiende Steam; antes
                // se escapaba también ahí, así que `'a\\b'` valía `ab` en vez de `a\\b`.
                if c == '\\' && q == '"' {
                    // Dentro de comillas dobles la barra SOLO escapa a `$`, `` ` ``, `"`, `\\`
                    // y el salto de línea. En cualquier otro caso la barra es un carácter
                    // literal que se conserva: `"a\xb"` vale `a\xb`, no `axb`.
                    //
                    // Antes se comía el carácter siguiente fuera cual fuera, así que
                    // `DXVK_HUD="a\xb"` se leía como `a xb` y el valor se corrompía al
                    // analizar y al volver a escribir. Verificado contra bash y `shlex`.
                    let siguiente = texto[i + c.len_utf8()..].chars().next();
                    let escapable = matches!(siguiente, Some('$' | '`' | '"' | '\\'));
                    if escapable {
                        i += c.len_utf8();
                        match siguiente {
                            Some(sig) => {
                                valor.push(sig);
                                i += sig.len_utf8();
                            }
                            None => valor.push('\\'),
                        }
                    } else {
                        valor.push('\\');
                        i += c.len_utf8();
                    }
                    continue;
                }
                if c == q {
                    comilla_abierta = None;
                    i += c.len_utf8();
                    continue;
                }
                valor.push(c);
                i += c.len_utf8();
                continue;
            }

            // Fuera de comillas.
            if es_separador(c) {
                break;
            }
            if c == '"' || c == '\'' {
                comilla_abierta = Some(c);
                i += c.len_utf8();
                continue;
            }
            if c == '\\' {
                // Fuera de comillas la barra sí escapa al separador (para meter un espacio
                // en un token) y a los caracteres especiales. Ante `\\n` literal, que no es
                // un salto, también se conserva la barra.
                let siguiente = texto[i + c.len_utf8()..].chars().next();
                match siguiente {
                    Some(sig) if sig.is_whitespace() || !sig.is_alphanumeric() => {
                        i += c.len_utf8() + sig.len_utf8();
                        // Una barra ante un salto de línea real es una continuación, no un
                        // carácter del valor.
                        if sig != '\n' && sig != '\r' {
                            valor.push(sig);
                        }
                    }
                    _ => {
                        valor.push('\\');
                        i += c.len_utf8();
                    }
                }
                continue;
            }
            valor.push(c);
            i += c.len_utf8();
        }

        if comilla_abierta.is_some() {
            cerrado = false;
        }

        tokens.push(Token {
            span: Span::new(inicio, i),
            texto: texto[inicio..i].to_string(),
            valor,
            cerrado,
        });
    }

    tokens
}

// ==================== CLASIFICACIÓN ====================

/// Clave de una asignación de entorno: lo que va antes del primer `=` sin comillas.
///
/// Se calcula sobre el valor ya sin comillas exteriores. `""` si no es una asignación.
fn clave_env(valor: &str) -> Option<&str> {
    if valor == "%command%" {
        return None;
    }
    let (k, _) = valor.split_once('=')?;
    if k.is_empty() {
        return None;
    }
    Some(k)
}

/// Analiza un comando contra un catálogo.
///
/// ## Orden de reconocimiento
///
/// 1. `%command%` localiza el marcador; lo de antes es cabeza, lo de después es juego.
/// 2. En la cabeza, un token con `=` es `env`, uno que esté en el catálogo como `wrapper`
///    o en la lista de envoltorios conocidos es `wrapper`, y el primero que no sea ninguna
///    de las dos cosas es el ejecutable.
/// 3. Después del marcador, todo es `ArgJuego` y se conserva.
///
/// ## Qué es «reconocido»
///
/// No basta con que el texto coincida: el `kind` del catálogo tiene que corresponder al
/// rol del token. Un ajuste `env` cuyo valor aparezca como argumento del juego no es ese
/// ajuste. Y si el mismo valor está en dos definiciones, es ambiguo y se conserva.
pub fn analizar(texto: &str, catalogo: &[Definicion]) -> Analisis {
    let tokens = tokenizar(texto);
    let mut avisos: Vec<Aviso> = Vec::new();

    // ---- el marcador ----
    let posiciones: Vec<Span> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.valor == "%command%")
        .map(|(_, t)| t.span)
        .collect();

    if posiciones.is_empty() {
        avisos.push(Aviso::SinMarcador);
    } else if posiciones.len() > 1 {
        avisos.push(Aviso::MarcadorRepetido {
            posiciones: posiciones.clone(),
        });
    }
    let indice_marcador = tokens.iter().position(|t| t.valor == "%command%");

    // Comillas sin cerrar: se avisa de todas, no solo de la primera.
    for t in &tokens {
        if !t.cerrado {
            let comilla = if t.texto.contains('"') { "\"" } else { "'" };
            avisos.push(Aviso::ComillaSinCerrar {
                span: t.span,
                comilla: comilla.to_string(),
            });
        }
    }

    // ---- posición relativa al marcador ----
    let mut segmentos: Vec<Segmento> = Vec::with_capacity(tokens.len());
    let mut visto_ejecutable = false;
    // Hay un wrapper cuyos argumentos propios aún no han terminado (ni con `--` ni porque
    // empiece otro wrapper).
    let mut wrapper_abierto = false;

    for (i, t) in tokens.iter().enumerate() {
        let antes_del_marcador = match indice_marcador {
            Some(m) => i < m,
            // Sin marcador, todo se trata como cabeza: es una línea rota y lo importante es
            // describirla, no clasificarla con optimismo.
            None => true,
        };

        let rol = if t.valor == "%command%" {
            Rol::Marcador
        } else if !antes_del_marcador {
            Rol::ArgJuego
        } else if clave_env(&t.valor).is_some() {
            Rol::Env
        } else if catalogo
            .iter()
            .any(|d| d.kind == "wrapper" && d.valor == t.valor)
        {
            // Un wrapper que admite opciones propias abre un tramo: lo que venga detrás son
            // sus argumentos hasta el `--`.
            wrapper_abierto = regla_wrapper(&t.valor).map(|r| r.acepta_argumentos).unwrap_or(false);
            Rol::Wrapper
        } else if catalogo.iter().any(|d| d.kind == "arg" && d.valor == t.valor) {
            // Argumento gestionado que no envuelve nada: una recomendación del catálogo
            // efectivo. Va antes del marcador y se reconoce por su texto exacto.
            Rol::Arg
        } else if wrapper_abierto {
            // Estamos dentro de los argumentos propios de un wrapper (`gamescope -W 1920`):
            // son suyos hasta el `--` o hasta el siguiente wrapper.
            if t.valor == "--" {
                wrapper_abierto = false;
            }
            Rol::ArgWrapper
        } else if !visto_ejecutable {
            visto_ejecutable = true;
            Rol::Ejecutable
        } else {
            // Argumentos del ejecutable antes del marcador (`mangohud --x %command%`).
            // No son del catálogo: se conservan como si fueran del juego.
            Rol::ArgJuego
        };

        segmentos.push(Segmento {
            span: t.span,
            texto: t.texto.clone(),
            valor: t.valor.clone(),
            rol,
            gestion: Gestion::Conservado {
                razon: Conservado::Desconocido,
            },
            cerrado: t.cerrado,
        });
    }

    // ---- reconocimiento contra el catálogo ----
    let mut ya_visto: Vec<(String, Span)> = Vec::new();
    for seg in segmentos.iter_mut() {
        match seg.rol {
            Rol::Marcador => {
                seg.gestion = Gestion::Marcador;
                continue;
            }
            Rol::ArgJuego => {
                seg.gestion = Gestion::Conservado {
                    razon: Conservado::ArgJuego,
                };
                continue;
            }
            // Los argumentos propios de un wrapper ni se reconocen ni se tocan.
            Rol::ArgWrapper => {
                seg.gestion = Gestion::Conservado {
                    razon: Conservado::Desconocido,
                };
                continue;
            }
            Rol::Env | Rol::Wrapper | Rol::Arg | Rol::Ejecutable => {}
        }

        let candidatos: Vec<usize> = catalogo
            .iter()
            .filter(|d| d.valor == seg.valor && rol_de_definicion(&d.kind) == Some(seg.rol))
            .map(|d| d.indice)
            .collect();

        // Un ejecutable nunca se gestiona: la app no lanza binarios propios.
        if seg.rol == Rol::Ejecutable || candidatos.is_empty() {
            seg.gestion = Gestion::Conservado {
                razon: Conservado::Desconocido,
            };
            continue;
        }

        if let Some((_, de_span)) = ya_visto.iter().find(|(v, _)| *v == seg.valor) {
            let de_span = *de_span;
            avisos.push(Aviso::Duplicado {
                span: seg.span,
                valor: seg.valor.clone(),
            });
            seg.gestion = Gestion::Conservado {
                razon: Conservado::Duplicado { de_span },
            };
            continue;
        }
        ya_visto.push((seg.valor.clone(), seg.span));

        if candidatos.len() > 1 {
            // Mismo valor, varias definiciones: NO se elige por el usuario. Elegir mal
            // apagaría un ajuste que el usuario quiere y activaría otro al guardar.
            avisos.push(Aviso::Ambiguo {
                span: seg.span,
                valor: seg.valor.clone(),
                candidatos: candidatos.clone(),
            });
            seg.gestion = Gestion::Conservado {
                razon: Conservado::Ambiguo { candidatos },
            };
            continue;
        }

        let indice = candidatos[0];
        let clave = catalogo
            .iter()
            .find(|d| d.indice == indice)
            .map(|d| d.valor.clone())
            .unwrap_or_else(|| seg.valor.clone());
        seg.gestion = Gestion::Thoriapp { clave, indice };
    }

    // ---- contradicciones: misma variable, valores distintos ----
    avisos.extend(detectar_contradicciones(&segmentos));

    // ---- mismo nombre, distinto valor ----
    avisos.extend(detectar_mismo_nombre(&segmentos, catalogo));

    let editable = !avisos.iter().any(Aviso::impide_editar);

    // La sintaxis es otra cosa que la editabilidad. Una comilla sin cerrar rompe las dos,
    // pero un marcador repetido o ausente solo impide editar: la línea se escribe bien
    // (aunque Steam no sabría qué hacer con ella). Y un duplicado no rompe ninguna de las
    // dos, solo la desambigua Thoriapp.
    let error_sintaxis = avisos.iter().find(|a| a.es_error_de_sintaxis());
    let motivo_sintaxis = error_sintaxis.map(|a| match a {
        Aviso::ComillaSinCerrar { comilla, .. } => format!(
            "hay una comilla {comilla} sin cerrar; el comando se conserva tal cual, pero no \
             se puede dar por bueno para lanzar"
        ),
        otro => format!("{otro:?}"),
    });
    let sintaxis_valida = error_sintaxis.is_none();

    let reconocidos = segmentos
        .iter()
        .filter(|s| matches!(s.gestion, Gestion::Thoriapp { .. }))
        .count();
    let conservados = segmentos
        .iter()
        .filter(|s| matches!(s.gestion, Gestion::Conservado { .. }))
        .count();

    Analisis {
        original: texto.to_string(),
        segmentos,
        avisos,
        editable,
        reconocidos,
        conservados,
        sintaxis_valida,
        motivo_sintaxis,
    }
}

/// Dos valores distintos para la misma clave de entorno.
///
/// No se marca como error del comando (Steam lo acepta: gana el último), sino como algo
/// que Thoriapp no puede ordenar solo: activar el ajuste A y dejar el B puesto es una
/// contradicción que el usuario debe ver.
fn detectar_contradicciones(segmentos: &[Segmento]) -> Vec<Aviso> {
    let mut por_clave: Vec<(String, Vec<(String, Span)>)> = Vec::new();

    for seg in segmentos.iter().filter(|s| s.rol == Rol::Env) {
        let Some(clave) = clave_env(&seg.valor) else {
            continue;
        };
        let resto = seg.valor.split_once('=').map(|(_, v)| v).unwrap_or("");
        match por_clave.iter_mut().find(|(k, _)| k == clave) {
            Some((_, lista)) => lista.push((resto.to_string(), seg.span)),
            None => por_clave.push((clave.to_string(), vec![(resto.to_string(), seg.span)])),
        }
    }

    por_clave
        .into_iter()
        .filter_map(|(key, valores)| {
            // Valores distintos que aparecen de verdad. Antes se calculaban y no se usaban:
            // `DXVK_HUD=fps` repetido igual salía como contradicción, que no lo es.
            let mut distintos: Vec<&str> = Vec::new();
            for (v, _) in &valores {
                if !distintos.contains(&v.as_str()) {
                    distintos.push(v.as_str());
                }
            }
            if distintos.len() < 2 {
                return None;
            }
            Some(Aviso::Contradiccion {
                key,
                // Solo los tramos con valores distintos: los repetidos iguales ya salen
                // como `Duplicado`, y señalar ambos sería avisar dos veces de lo mismo.
                spans: valores
                    .iter()
                    .filter(|(v, _)| distintos.contains(&v.as_str()))
                    .map(|(_, s)| *s)
                    .collect(),
            })
        })
        .collect()
}

/// El comando lleva `PROTON_X=1` y el catálogo tiene `PROTON_X=2`: **no** son el mismo
/// ajuste aunque compartan el nombre de la variable.
///
/// Sin esto, la UI podría pintar el ajuste `PROTON_X=2` como activo porque «ya está
/// PROTON_X», y apagarlo borraría un valor que el usuario no eligió.
fn detectar_mismo_nombre(segmentos: &[Segmento], catalogo: &[Definicion]) -> Vec<Aviso> {
    let mut avisos = Vec::new();
    for seg in segmentos.iter().filter(|s| s.rol == Rol::Env) {
        let Some(clave) = clave_env(&seg.valor) else {
            continue;
        };
        for d in catalogo.iter().filter(|d| d.kind == "env") {
            if d.valor == seg.valor || clave_env(&d.valor) != Some(clave) {
                continue;
            }
            // Si ese otro valor TAMBIÉN está en el comando, no es «otro ajuste que
            // chocaría»: ya está puesto, y lo que hay es una contradicción real (que
            // `detectar_contradicciones` informa con sus dos spans). Reportarlo aquí
            // duplicaría el aviso y, peor, señalaría como «conflicto latente» un ajuste
            // que el usuario tiene puesto a propósito.
            let ya_presente = segmentos.iter().any(|s| s.valor == d.valor);
            if ya_presente {
                continue;
            }
            avisos.push(Aviso::MismoNombreDistintoValor {
                key: clave.to_string(),
                valor: d.valor.clone(),
            });
        }
    }
    avisos
}

// ==================== ESTADO POR AJUSTE ====================
//
// El análisis dice qué hay en el comando. La UI necesita, además, un estado por cada
// ajuste del catálogo que responda «¿lo tengo puesto?» sin mentir cuando la respuesta no
// es un simple sí o no. Antes, un ajuste que aparecía dos veces y solo una era gestionable
// se podía pintar como desactivado aunque quedara una aparición activa en la línea.

/// Estado de un ajuste del catálogo respecto al comando analizado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstadoAjuste {
    /// Índice del ajuste en el catálogo. Va explícito para que la UI no tenga que
    /// deducirlo por la posición en la lista.
    ///
    /// **No es una identidad**: dos catálogos distintos (el del juego y el efectivo con
    /// recomendaciones) dan índices distintos para el mismo ajuste, y el hueco lo ocupa
    /// otro. Para identificar un ajuste se usa `valor`.
    pub indice: usize,
    /// Texto exacto del ajuste en el comando (`mangohud`, `RADV_PERFTEST=sam`).
    ///
    /// Es la identidad de verdad: no cambia al reordenar el catálogo, ni al añadir o quitar
    /// recomendaciones, ni entre el catálogo del juego y el efectivo. La UI pinta esto, así
    /// que nunca enseña un índice interno.
    pub valor: String,
    /// Qué clase de pieza es (`env`, `wrapper`, `arg`), para que la UI no la adivine.
    pub kind: String,
    /// Si el ajuste solo tiene efecto bajo Proton/Wine.
    ///
    /// Lo decide **Rust** (`rules::is_proton_only`, la única lista de prefijos) y viaja por
    /// IPC para que la interfaz no vuelva a deducirlo por su cuenta. Mantener una copia de
    /// la regla en TypeScript fue lo que dejó `WINE_FULLSCREEN_FSR=1` pintada como aplicable
    /// en un juego nativo mientras el motor la rechazaba.
    pub proton_only: bool,
    #[serde(flatten)]
    pub situacion: Situacion,
}

/// En qué situación está un ajuste respecto al comando.
///
/// Cinco casos y no un booleano, porque «activado» a medias es una mentira: un ajuste que
/// aparece dos veces y solo una es gestionable se pintaba como desactivado aunque quedara
/// una aparición viva en la línea.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "estado", rename_all = "snake_case")]
pub enum Situacion {
    /// No aparece en el comando.
    Inactivo,
    /// Aparece y su aparición está gestionada: activarlo o desactivarlo hace lo esperado.
    Activo { span: Span },
    /// Aparece, pero no todas sus apariciones se pueden tocar (una es duplicado de otra).
    ///
    /// `gestionadas` es dónde sí se puede actuar. La UI debe pintarlo activo **y** advertir
    /// de que queda otra aparición que Thoriapp no controla: desactivarlo la dejaría puesta,
    /// y eso es justo lo que no puede parecer.
    Parcial {
        gestionadas: Vec<Span>,
        conservadas: Vec<Span>,
    },
    /// Aparece, pero Thoriapp no puede gestionarla (ambiguo). No se puede activar ni
    /// desactivar sin que el usuario decida.
    Ambiguo {
        spans: Vec<Span>,
        candidatos: Vec<usize>,
    },
    /// El comando tiene otro valor para la misma variable: activar este ajuste crearía una
    /// contradicción, y el que está puesto no es este.
    Conflicto {
        spans: Vec<Span>,
        /// Valores del comando que ocupan la misma variable.
        en_comando: Vec<String>,
    },
}

/// Estado de cada ajuste del catálogo respecto a un comando analizado.
///
/// Devuelve un vector paralelo a `catalogo`. Es lo que la UI usará para pintar la lista de
/// ajustes sin deducir nada por su cuenta.
pub fn estado_de_ajustes(analisis: &Analisis, catalogo: &[Definicion]) -> Vec<EstadoAjuste> {
    catalogo
        .iter()
        .map(|d| {
            // 1) Apariciones gestionadas de este ajuste.
            let gestionadas: Vec<Span> = analisis
                .segmentos
                .iter()
                .filter(|s| matches!(&s.gestion, Gestion::Thoriapp { indice, .. } if *indice == d.indice))
                .map(|s| s.span)
                .collect();

            // 2) Apariciones del mismo texto que Thoriapp conserva (duplicados).
            let conservadas: Vec<Span> = analisis
                .segmentos
                .iter()
                .filter(|s| {
                    s.valor == d.valor
                        && matches!(s.gestion, Gestion::Conservado { .. })
                        && s.rol != Rol::Marcador
                })
                .map(|s| s.span)
                .collect();

            let situacion = if !gestionadas.is_empty() {
                if conservadas.is_empty() {
                    Situacion::Activo {
                        span: gestionadas[0],
                    }
                } else {
                    Situacion::Parcial {
                        gestionadas,
                        conservadas,
                    }
                }
            } else if let Some((spans, candidatos)) = aparicion_ambigua(analisis, d) {
                Situacion::Ambiguo { spans, candidatos }
            } else if let Some((spans, en_comando)) = conflicto_con(analisis, d) {
                Situacion::Conflicto { spans, en_comando }
            } else {
                Situacion::Inactivo
            };

            // El índice va DENTRO del estado. Antes se alineaban por posición con el
            // catálogo, así que la UI tenía que suponer que ese orden no cambia nunca: al
            // reordenar `rules.params`, los estados habrían apuntado a otro ajuste sin que
            // nada fallara.
            EstadoAjuste {
                indice: d.indice,
                valor: d.valor.clone(),
                kind: d.kind.clone(),
                proton_only: d.proton_only,
                situacion,
            }
        })
        .collect()
}

/// Apariciones del valor de este ajuste que Thoriapp conserva por ser ambiguas.
fn aparicion_ambigua(analisis: &Analisis, d: &Definicion) -> Option<(Vec<Span>, Vec<usize>)> {
    let ambiguas: Vec<&Segmento> = analisis
        .segmentos
        .iter()
        .filter(|s| matches!(&s.gestion, Gestion::Conservado { razon: Conservado::Ambiguo { .. } }))
        .filter(|s| s.valor == d.valor)
        .collect();
    if ambiguas.is_empty() {
        return None;
    }
    let candidatos = ambiguas
        .iter()
        .find_map(|s| match &s.gestion {
            Gestion::Conservado {
                razon: Conservado::Ambiguo { candidatos },
            } => Some(candidatos.clone()),
            _ => None,
        })
        .unwrap_or_default();
    if !candidatos.contains(&d.indice) {
        return None;
    }
    Some((ambiguas.iter().map(|s| s.span).collect(), candidatos))
}

/// Apariciones de la misma variable de entorno con otro valor.
fn conflicto_con(analisis: &Analisis, d: &Definicion) -> Option<(Vec<Span>, Vec<String>)> {
    let clave = clave_env(&d.valor)?;
    let otros: Vec<&Segmento> = analisis
        .segmentos
        .iter()
        .filter(|s| s.rol == Rol::Env)
        .filter(|s| clave_env(&s.valor) == Some(clave) && s.valor != d.valor)
        .collect();
    if otros.is_empty() {
        return None;
    }
    Some((
        otros.iter().map(|s| s.span).collect(),
        otros.iter().map(|s| s.valor.clone()).collect(),
    ))
}

// ==================== ESCAPADO DE VALORES NUEVOS ====================

/// Rechazo al escapar un valor del catálogo para meterlo en el comando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValorNoSerializable {
    /// Vacío: no es un argumento.
    Vacio,
    /// Lleva un carácter que `validar` rechaza (encadenamiento, redirección…). No se
    /// escapa: aunque se pudiera entrecomillar, el comando resultante seguiría siendo una
    /// orden encadenada, que es lo que hay que evitar.
    CaracterProhibido(char),
}

impl std::fmt::Display for ValorNoSerializable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValorNoSerializable::Vacio => write!(f, "el valor está vacío"),
            ValorNoSerializable::CaracterProhibido(c) => {
                write!(f, "el valor lleva «{c}», que encadena órdenes")
            }
        }
    }
}

/// ¿Este carácter obliga a entrecomillar el valor?
///
/// Espacio, tabulador y comillas, obviamente. También los comodines y el `~`: sin comillas,
/// el lanzador los expandiría, así que el valor que llega al juego no sería el del catálogo.
fn necesita_comillas(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\'' | '\\' | '*' | '?' | '[' | ']' | '{' | '}' | '~' | '#' | '!' | '(' | ')'
        )
}

/// Escribe un valor del catálogo de forma que el lexer lo devuelva idéntico.
///
/// ## Por qué existe
///
/// Antes los ajustes se insertaban tal cual (`valor.clone()`). Funcionaba solo porque los
/// 25 valores de `games.json` no llevan espacios. En cuanto un ajuste llevara uno
/// (`MANGOHUD_CONFIG="fps_limit=0,position=top-left"`) el comando salía partido en dos
/// argumentos. El motor no puede depender de que el catálogo sea benigno.
///
/// ## Qué sintaxis se usa
///
/// La del lexer del propio motor, que es la que entiende Steam: comillas dobles con `\`
/// como escape de `\` y `"`. Se eligen dobles porque Steam y los wrappers las tratan igual
/// en todas las plataformas, y porque los valores del catálogo pueden llevar `'` sin
/// tratarse.
///
/// ## Qué NO se hace
///
/// No se escapa un carácter prohibido (`;`, `$`, `|`…): se **rechaza**. Entrecomillarlos
/// daría un texto que `validar` aceptaría pero que seguiría siendo una orden encadenada en
/// un uso descuidado; ante la duda, el motor no construye ese comando.
///
/// Garantía que cumplen los tests: si esto devuelve `Ok(v)` y `v` se analiza, aparece
/// **exactamente un** segmento con `valor == original_del_catalogo`.
pub fn escapar_valor(valor: &str) -> Result<String, ValorNoSerializable> {
    if valor.is_empty() {
        return Err(ValorNoSerializable::Vacio);
    }
    if let Some(bad) = valor.chars().find(|c| CARACTERES_PROHIBIDOS.contains(c)) {
        return Err(ValorNoSerializable::CaracterProhibido(bad));
    }

    if !valor.chars().any(necesita_comillas) {
        return Ok(valor.to_string());
    }

    let mut salida = String::with_capacity(valor.len() + 2);
    salida.push('"');
    for c in valor.chars() {
        if c == '"' || c == '\\' {
            salida.push('\\');
        }
        salida.push(c);
    }
    salida.push('"');
    Ok(salida)
}

// ==================== EDICIÓN ====================

/// Un cambio pedido sobre el comando.
///
/// La identidad es `valor`, no `indice`. Un índice posicional no sirve: el mismo ajuste
/// tiene uno distinto en el catálogo del juego y en el efectivo (el que añade las
/// recomendaciones), así que el frontend y Rust podían estar hablando de ajustes diferentes
/// y el motor rechazaba el cambio con «ese ajuste no pertenece a este juego».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cambio {
    /// Índice en el catálogo con el que se analizó. Se conserva para poder informar, pero
    /// **no se usa para decidir** qué ajuste es.
    pub indice: usize,
    /// Texto exacto del ajuste. Es lo que identifica el cambio.
    pub valor: String,
    /// `true` para que esté en el comando, `false` para que no.
    pub activo: bool,
}

/// Traduce los ajustes activos deseados a cambios concretos sobre el comando analizado.
///
/// Compara el conjunto pedido con lo que el análisis ya reconoce, y solo emite cambios
/// para las diferencias. Los índices que no se pueden gestionar (ambiguos, duplicados,
/// argumentos del juego) se ignoran: ya estaban puestos y no se quitan.
///
/// Se calcula **sin mirar el texto**: solo con el `Analisis`, así que la UI puede
/// reutilizarlo tal cual al serializar.
///
/// La UI no implementa esta comparación: se la pide a `launch_command_changes`. Que el
/// criterio de «esto ya está puesto» viva en un solo sitio es lo que evita que activar y
/// desactivar dos veces deje el comando con un ajuste duplicado.
pub fn cambios_para(analisis: &Analisis, catalogo: &[Definicion], activos: &[usize]) -> Vec<Cambio> {
    // El índice pedido se traduce a su valor **con el mismo catálogo** con el que se
    // analizó. A partir de ahí todo se compara por valor.
    let valor_de = |indice: usize| catalogo.iter().find(|d| d.indice == indice).map(|d| d.valor.clone());
    let pedidos: Vec<String> = activos.iter().filter_map(|i| valor_de(*i)).collect();

    let mut cambios: Vec<Cambio> = Vec::new();

    // Apagar lo que sobra.
    for seg in &analisis.segmentos {
        if let Gestion::Thoriapp { indice, clave } = &seg.gestion {
            if pedidos.contains(clave) {
                continue;
            }
            if cambios.iter().any(|c| c.valor == *clave) {
                continue;
            }
            cambios.push(Cambio {
                indice: *indice,
                valor: clave.clone(),
                activo: false,
            });
        }
    }

    // Encender lo que falta.
    for indice in activos {
        let Some(valor) = valor_de(*indice) else {
            continue;
        };
        let ya_esta = analisis
            .segmentos
            .iter()
            .any(|s| matches!(&s.gestion, Gestion::Thoriapp { clave, .. } if *clave == valor));
        if ya_esta || cambios.iter().any(|c| c.valor == valor) {
            continue;
        }
        cambios.push(Cambio {
            indice: *indice,
            valor,
            activo: true,
        });
    }

    cambios
}

/// ¿Es este análisis coherente consigo mismo?
///
/// Lo que llega por IPC no es de fiar: la UI puede mandar un `Analisis` manipulado, antiguo o
/// corrompido. Un span fuera del texto, desordenado o que no cae en un límite de carácter
/// haría que `serializar` cortara por un sitio imposible. `str::get` ya devuelve `None` en
/// vez de entrar en pánico, pero con un span válido-pero-absurdo se generaría un comando
/// distinto del original sin avisar.
///
/// Por eso se comprueba antes de tocar nada: si no cuadra, se tratará como no editable y
/// se devolverá el original intacto.
fn analisis_coherente(analisis: &Analisis) -> Result<(), ErrorAnalisis> {
    let t = &analisis.original;
    let mut anterior_fin = 0_usize;
    for (i, seg) in analisis.segmentos.iter().enumerate() {
        let span = seg.span;
        // Dentro del texto.
        if span.inicio > span.fin || span.fin > t.len() {
            return Err(ErrorAnalisis::SpanFueraDeRango {
                segmento: i,
                inicio: span.inicio,
                fin: span.fin,
                longitud: t.len(),
            });
        }
        // En frontera de carácter: si cae a mitad de un carácter multibyte, recortar
        // entraría en pánico.
        if !t.is_char_boundary(span.inicio) || !t.is_char_boundary(span.fin) {
            return Err(ErrorAnalisis::SpanEnMitadDeCaracter {
                segmento: i,
                inicio: span.inicio,
                fin: span.fin,
            });
        }
        // En orden y sin solaparse.
        if span.inicio < anterior_fin {
            return Err(ErrorAnalisis::SpansDesordenados {
                segmento: i,
                inicio: span.inicio,
                fin_anterior: anterior_fin,
            });
        }
        anterior_fin = span.fin;
        // El texto del segmento tiene que ser exactamente el del original: si no, quien
        // llama el IPC lo manipuló y no se puede confiar en el resto.
        if span.recortar(t) != seg.texto {
            return Err(ErrorAnalisis::TextoNoCoincide {
                segmento: i,
                esperado: span.recortar(t).to_string(),
                recibido: seg.texto.clone(),
            });
        }
    }
    Ok(())
}

/// El análisis recibido no cuadra consigo mismo.
///
/// Estos errores no deberían ocurrir nunca si el modelo viene de [`analizar`]; existen
/// porque el modelo viaja por IPC y el proceso de la UI puede mandar cualquier cosa. Antes
/// se devolvía el original en silencio, que para quien llama era indistinguible de «no hubo
/// cambios»: un éxito falso. Ahora se devuelve el original **y** el motivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ErrorAnalisis {
    /// Un `Span` se sale del texto.
    SpanFueraDeRango {
        segmento: usize,
        inicio: usize,
        fin: usize,
        longitud: usize,
    },
    /// Un `Span` cae en mitad de un carácter multibyte.
    SpanEnMitadDeCaracter {
        segmento: usize,
        inicio: usize,
        fin: usize,
    },
    /// Los segmentos no van en orden o se solapan.
    SpansDesordenados {
        segmento: usize,
        inicio: usize,
        fin_anterior: usize,
    },
    /// El `texto` de un segmento no es el que hay en `original` entre sus offsets.
    TextoNoCoincide {
        segmento: usize,
        esperado: String,
        recibido: String,
    },
    /// Un `Cambio` pide un ajuste que no está en el catálogo pasado.
    IndiceDesconocido { indice: usize },
}

impl std::fmt::Display for ErrorAnalisis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorAnalisis::SpanFueraDeRango {
                segmento,
                inicio,
                fin,
                longitud,
            } => write!(
                f,
                "el segmento {segmento} tiene el tramo {inicio}..{fin}, fuera del texto de \
                 {longitud} bytes"
            ),
            ErrorAnalisis::SpanEnMitadDeCaracter {
                segmento,
                inicio,
                fin,
            } => write!(
                f,
                "el segmento {segmento} ({inicio}..{fin}) corta un carácter multibyte"
            ),
            ErrorAnalisis::SpansDesordenados {
                segmento,
                inicio,
                fin_anterior,
            } => write!(
                f,
                "el segmento {segmento} empieza en {inicio}, antes de que acabe el anterior \
                 ({fin_anterior})"
            ),
            ErrorAnalisis::TextoNoCoincide {
                segmento,
                esperado,
                recibido,
            } => write!(
                f,
                "el segmento {segmento} dice ser {recibido:?} pero en el original hay \
                 {esperado:?}"
            ),
            ErrorAnalisis::IndiceDesconocido { indice } => {
                write!(f, "el ajuste {indice} no está en el catálogo")
            }
        }
    }
}

impl std::error::Error for ErrorAnalisis {}

/// Serializa el comando aplicando `cambios`, conservando intacto todo lo demás.
///
/// ## Cómo
///
/// 1. Se ordenan los reemplazos por offset y se aplican **de atrás hacia delante**, así
///    los spans de los cambios no se desplazan.
/// 2. Lo que no cae dentro de un reemplazo se copia literalmente de `original`.
/// 3. Los ajustes nuevos se insertan en el sitio canónico según su tipo: env y wrappers
///    antes del `%command%` (env antes que wrappers, como han ido siempre en Thoriapp),
///    argumentos después.
///
/// Si `cambios` no toca nada, el resultado es `original` exacto.
///
/// ## Cuándo NO se toca nada
///
/// - Si el análisis no es editable (comillas sin cerrar, `%command%` ausente o repetido) se
///   devuelve `original` **sin aplicar nada**, aunque lleguen cambios. Reconstruir a medias
///   un comando que no se entiende es justo lo que hay que evitar.
/// - Un `indice` que no está en el catálogo pasado aquí se ignora: quien llama debe usar el
///   mismo catálogo con el que analizó.
#[allow(dead_code)]
pub fn serializar(analisis: &Analisis, catalogo: &[Definicion], cambios: &[Cambio]) -> String {
    // Se descarta el error a propósito: quien quiera distinguir «no cambió nada» de «los
    // datos de IPC están corruptos» tiene [`intentar_serializar`].
    //
    // El IPC usa `intentar_serializar`. Esta variante se queda para los tests y para quien
    // solo quiera el texto: el motor no obliga a manejar el error si no interesa.
    match intentar_serializar(analisis, catalogo, cambios) {
        Ok(texto) => texto,
        Err(_) => analisis.original.clone(),
    }
}

/// Borra un segmento del comando y devuelve el texto resultante.
///
/// Solo acepta segmentos **conservados por desconocidos**: es lo que la UI ofrece quitar de
/// la tarjeta de argumentos conservados. Un envoltorio, un ajuste reconocido o un argumento
/// del juego no se tocan: borrarlos cambiaría cómo arranca el juego sin que se vea.
///
/// El corte se hace por el span real del análisis, no buscando el texto: un argumento puede
/// aparecer repetido (o dentro de otro) y `replace` se llevaría el equivocado.
pub fn quitar_segmento(analisis: &Analisis, indice: usize) -> Result<String, ErrorAnalisis> {
    analisis_coherente(analisis)?;

    // El índice es la posición en `segmentos`, que es lo que la UI pinta y manda de vuelta.
    let segmento = analisis
        .segmentos
        .get(indice)
        .ok_or(ErrorAnalisis::IndiceDesconocido { indice })?;

    if !matches!(
        segmento.gestion,
        Gestion::Conservado {
            razon: Conservado::Desconocido
        }
    ) {
        return Err(ErrorAnalisis::IndiceDesconocido { indice });
    }

    let original = &analisis.original;
    let desde = segmento.span.inicio;
    let hasta = segmento.span.fin;
    if desde > hasta || hasta > original.len() {
        return Err(ErrorAnalisis::SpanFueraDeRango {
            segmento: indice,
            inicio: desde,
            fin: hasta,
            longitud: original.len(),
        });
    }

    // Se arrastra también un espacio para no dejar dos seguidos: el shell lo leería como
    // un argumento vacío. Se prefiere el de la izquierda (así el segmento desaparece con
    // su separación y el resto no se mueve), y solo si no lo hay se quita el de la derecha,
    // que es lo que pasa cuando el segmento es el primero del comando.
    let mut desde = desde;
    let mut hasta = hasta;
    if desde > 0 && original.as_bytes()[desde - 1] == b' ' {
        while desde > 0 && original.as_bytes()[desde - 1] == b' ' {
            desde -= 1;
        }
    } else {
        while hasta < original.len() && original.as_bytes()[hasta] == b' ' {
            hasta += 1;
        }
    }

    let mut texto = String::with_capacity(original.len());
    texto.push_str(&original[..desde]);
    texto.push_str(&original[hasta..]);
    Ok(texto)
}

/// Como [`serializar`], pero distingue el éxito del fallo.
///
/// Se devuelve el original en los dos casos en que no se reconstruye nada, pero con
/// significados distintos:
///
/// - `Ok(original)` — no había nada que aplicar (sin cambios o análisis no editable). Es un
///   éxito: el comando sigue como estaba.
/// - `Err(_)` — los datos no cuadran (spans imposibles, texto manipulado, índice que no está
///   en el catálogo). Se conserva el original **y** se informa. Antes este caso devolvía el
///   original como si todo hubiera ido bien, y la UI no podía saber que su estado y el del
///   motor se habían separado.
pub fn intentar_serializar(
    analisis: &Analisis,
    catalogo: &[Definicion],
    cambios: &[Cambio],
) -> Result<String, ErrorAnalisis> {
    // Datos de IPC: si el modelo no cuadra con su propio texto, no se reconstruye nada.
    analisis_coherente(analisis)?;

    // Un cambio que pide un ajuste inexistente es un fallo, no un no-op: si se ignorara, la
    // UI creería que el ajuste está puesto y el comando no lo tendría.
    //
    // La comprobación es **por valor**, no por índice: el índice que manda el frontend sale
    // de otro catálogo (el del juego sin recomendaciones) y no tiene por qué coincidir con
    // el de aquí. Comparar índices era lo que producía «ese ajuste no pertenece a este
    // juego» al activar una recomendación.
    for cambio in cambios {
        let existe = catalogo
            .iter()
            .any(|d| d.valor == cambio.valor && d.indice == cambio.indice);
        let existe_por_valor = existe || catalogo.iter().any(|d| d.valor == cambio.valor);
        if !existe_por_valor {
            return Err(ErrorAnalisis::IndiceDesconocido {
                indice: cambio.indice,
            });
        }
    }

    if !analisis.editable || cambios.is_empty() {
        return Ok(analisis.original.clone());
    }

    let original = &analisis.original;

    // ---- 1) qué segmentos desaparecen ----
    let mut borrar: Vec<Span> = Vec::new();
    for cambio in cambios.iter().filter(|c| !c.activo) {
        borrar.extend(analisis.spans_de(&cambio.valor));
    }

    // El `--` de un wrapper es suyo: si se quita el wrapper, el separador se va con él.
    // Dejarlo suelto producía `gamemoderun -- mangohud %command%`, que al reanalizarse
    // saldría como argumento del juego. Es un caso que solo aparece al editar, porque en un
    // comando bien escrito un `--` no está nunca sin su programa delante.
    for span in borrar.clone() {
        if let Some(seg) = analisis
            .segmentos
            .iter()
            .find(|s| s.span.inicio == span.inicio && s.span.fin == span.fin)
        {
            if seg.rol != Rol::Wrapper {
                continue;
            }
            // El `--` va inmediatamente detrás del programa del wrapper.
            let siguiente = analisis
                .segmentos
                .iter()
                .find(|s| s.span.inicio >= seg.span.fin);
            if let Some(sig) = siguiente {
                if sig.valor == "--" && sig.rol == Rol::ArgWrapper {
                    borrar.push(sig.span);
                }
            }
        }
    }
    let borrado = |span: &Span| {
        borrar
            .iter()
            .any(|b| b.inicio == span.inicio && b.fin == span.fin)
    };

    // ---- 2) qué ajustes nuevos entran ----
    // Un índice que ya está en el comando no se duplica.
    let mut env_nuevos: Vec<(usize, String)> = Vec::new();
    let mut wrapper_nuevos: Vec<(usize, String)> = Vec::new();
    let mut arg_nuevos: Vec<(usize, String)> = Vec::new();
    // Determinismo: se recorren los cambios en ORDEN DE CATÁLOGO, no en el orden en que
    // lleguen. Si no, dos ajustes activados a la vez salían en el orden del array y
    // activados en dos pasos salían en orden canónico (env antes que wrapper), así que el
    // mismo estado final daba dos comandos distintos. Lo encontró una prueba generativa.
    let mut activos: Vec<&Cambio> = cambios.iter().filter(|c| c.activo).collect();
    // Orden canónico del comando: primero los `env`, después los wrappers, después los
    // args; y dentro de cada grupo, por índice de catálogo para que sea estable.
    let rango_rol = |valor: &str| -> u8 {
        match catalogo
            .iter()
            .find(|d| d.valor == valor)
            .and_then(|d| rol_de_definicion(&d.kind))
        {
            Some(Rol::Env) => 0,
            Some(Rol::Wrapper) => 1,
            Some(_) => 2,
            None => 3,
        }
    };
    activos.sort_by_key(|c| (rango_rol(&c.valor), c.indice));

    for cambio in activos {
        let ya_esta = analisis.segmentos.iter().any(
            |s| matches!(&s.gestion, Gestion::Thoriapp { clave, .. } if *clave == cambio.valor),
        );
        if ya_esta {
            continue;
        }
        // Se busca por **valor**: el índice que trae el cambio puede venir de otro catálogo
        // (el del juego sin recomendaciones) y apuntar a otro ajuste.
        let Some(d) = catalogo.iter().find(|d| d.valor == cambio.valor) else {
            // Valor que no está en el catálogo con el que se analizó: se ignora en vez de
            // inventarse un texto.
            continue;
        };
        let Some(rol) = rol_de_definicion(&d.kind) else {
            continue;
        };
        // Se escapa SIEMPRE, aunque el valor no lleve espacios: no se depende de que el
        // catálogo sea benigno (ver `escapar_valor`).
        let Ok(texto) = escapar_valor(&d.valor) else {
            continue;
        };
        match rol {
            Rol::Env => env_nuevos.push((d.indice, texto)),
            Rol::Wrapper => wrapper_nuevos.push((d.indice, texto)),
            _ => arg_nuevos.push((d.indice, texto)),
        }
    }

    // ---- 3) colocación de los ajustes nuevos ----
    //
    // Un wrapper nuevo NO se mete «donde quepa»: se coloca por su capa de precedencia
    // respecto a los wrappers vivos, y si no hay hueco seguro **no se inserta**. El orden
    // de dos wrappers no es conmutable: `gamescope -- mangohud %command%` mide el cliente de
    // gamescope y `mangohud gamescope -- %command%` mide el compositor. Para una app de
    // optimización y benchmarking, dejar que el orden de dos pulsaciones cambie eso es un
    // fallo, no una preferencia.
    let colocacion = colocar_nuevos(
        analisis,
        &env_nuevos,
        &wrapper_nuevos,
        &arg_nuevos,
    );
    #[cfg(test)]
    eprintln!("COLOCACION antes_de={:?} al_final={:?}", colocacion.antes_de, colocacion.al_final);

    // ---- 4) una sola pasada sobre el original ----
    //
    // Se recorre el texto **token a token** en orden, y entre token y token se copia el
    // hueco original. Antes de hacerlo se decide si el hueco es un límite de borrado, que
    // es el único sitio donde se toca el espaciado. Así:
    //
    //  - lo que no se edita sale byte a byte, con sus espacios dobles incluidos;
    //  - al borrar un token no quedan dos espacios ni un espacio antes del `%command%`;
    //  - no hay aritmética de offsets desplazados (el bug de `"PROTON_X=1  mangohud   gamemoderun"`
    //    → `"PROTON_X=1 % command%"`: el segundo borrado se aplicaba sobre una posición ya
    //    desplazada y partía el marcador).
    //
    // Los segmentos nuevos van «pegados» a un segmento del original: se escriben justo
    // antes de él. Así no hace falta inventar offsets.
    let mut salida = String::with_capacity(original.len() + 64);
    let mut ultimo_fin = 0_usize;

    for (i, seg) in analisis.segmentos.iter().enumerate() {
        let hueco = original.get(ultimo_fin..seg.span.inicio).unwrap_or("");
        let va_borrado = borrado(&seg.span);
        let anterior_borrado = i
            .checked_sub(1)
            .and_then(|j| analisis.segmentos.get(j))
            .map(|s| borrado(&s.span))
            .unwrap_or(false);

        if va_borrado {
            ultimo_fin = seg.span.fin;
            continue;
        }

        let al_principio = salida.trim().is_empty();
        // Qué hueco toca delante de este token:
        //  - si el anterior se borró, basta con un separador simple (nunca doble);
        //  - si no, el original tal cual (aunque tenga tres espacios).
        //
        // Salvo que haya algo que insertar delante: entonces se escribe el hueco normal y
        // los nuevos tokens con un espacio entre ellos.
        let inserta_aqui = colocacion.delante_de(seg.span.inicio);

        if !al_principio {
            let hueco_a_escribir = if anterior_borrado { " " } else { hueco };
            salida.push_str(hueco_a_escribir);
        }

        if !inserta_aqui.is_empty() {
            if !al_principio {
                // Ya hay un separador del hueco.
            }
            salida.push_str(&inserta_aqui.join(" "));
            if !seg.texto.is_empty() {
                salida.push(' ');
            }
        }

        salida.push_str(&seg.texto);
        ultimo_fin = seg.span.fin;
    }

    // Lo que no encontró hueco (solo puede ser el bloque suelto del final: args y lo que
    // no se pudo colocar con seguridad).
    if !colocacion.al_final.is_empty() {
        if !salida.is_empty() && !salida.ends_with(' ') {
            salida.push(' ');
        }
        salida.push_str(&colocacion.al_final.join(" "));
    }

    Ok(salida)
}

/// Resultado de colocar los ajustes nuevos sobre un análisis.
///
/// Los `String` son el texto exacto que hay que escribir (ya escapado). Se guardan por
/// posición de inserción: «esto va justo antes del segmento que empieza en el byte N».
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Colocacion {
    antes_de: Vec<(usize, Vec<String>)>,
    al_final: Vec<String>,
}

impl Colocacion {
    fn delante_de(&self, inicio: usize) -> &[String] {
        self.antes_de
            .iter()
            .find(|(pos, _)| *pos == inicio)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }
}

/// Coloca los ajustes nuevos respetando la precedencia de los wrappers.
///
/// ## El concepto que faltaba: la zona envuelta
///
/// Un wrapper no ocupa un token: ocupa **un tramo**. `gamescope -W 1920 -H 1080 -f -- %command%`
/// es: programa (`gamescope`), sus opciones (`-W 1920 -H 1080 -f`), separador (`--`) y, a
/// partir de ahí, lo envuelto. Meter mangohud «delante de gamescope» da
/// `mangohud gamescope -- %command%`, que mide el compositor; meterlo **detrás del `--`** da
/// `gamescope -- mangohud %command%`, que mide el cliente. Son cosas distintas y la regla
/// tiene que decir cuál se quiere.
///
/// Por eso cada wrapper se representa por su *inicio* y, si lleva `--`, por la posición
/// **tras** el separador, que es donde entra un wrapper más interior.
///
/// ## Reglas de colocación
///
/// - **Lo que ya está no se mueve** (ni wrappers desconocidos).
/// - Un wrapper nuevo entra **inmediatamente a la izquierda** de lo que debe envolver:
///   - si su capa es mayor que la de un wrapper vivo (más interior), entra detrás del
///     separador `--` de ese wrapper si lo tiene, y si no, justo delante de él;
///   - si su capa es menor (más exterior), entra delante de ese wrapper.
/// - Si no se puede determinar el hueco —misma capa con otro nombre, o un wrapper del que
///   no hay regla—, **no se inserta** y se informa.
fn colocar_nuevos(
    analisis: &Analisis,
    env_nuevos: &[(usize, String)],
    wrapper_nuevos: &[(usize, String)],
    arg_nuevos: &[(usize, String)],
) -> Colocacion {
    let mut colocacion = Colocacion::default();

    /// Un wrapper que ya está en el comando, con su tramo.
    struct Vivo {
        nombre: String,
        /// Byte donde empieza el programa envoltorio.
        inicio: usize,
        /// Byte donde empieza lo que envuelve: tras el `--` si lo lleva, o donde empieza el
        /// siguiente token si no.
        zona_envuelta: usize,
        regla: Option<&'static ReglaWrapper>,
    }

    let segmentos = &analisis.segmentos;
    let mut vivos: Vec<Vivo> = Vec::new();
    for (i, seg) in segmentos.iter().enumerate() {
        if seg.rol != Rol::Wrapper {
            continue;
        }
        // Lo envuelto empieza en el primer token que ya no es del wrapper: ni sus
        // argumentos propios ni el separador `--`.
        let zona_envuelta = segmentos
            .iter()
            .skip(i + 1)
            .find(|s| !matches!(s.rol, Rol::ArgWrapper))
            .map(|s| s.span.inicio)
            .unwrap_or(seg.span.fin);
        vivos.push(Vivo {
            nombre: seg.valor.clone(),
            inicio: seg.span.inicio,
            zona_envuelta,
            regla: regla_wrapper(&seg.valor),
        });
    }

    // Punto de referencia general: delante del marcador.
    let marcador = segmentos
        .iter()
        .find(|s| s.rol == Rol::Marcador)
        .map(|s| s.span.inicio);

    // --- env: lo más exterior de todo lo que Thoriapp coloca ---
    // Van delante del primer wrapper vivo si lo hay (un env afecta a todos los procesos),
    // y si no, delante del marcador.
    if !env_nuevos.is_empty() {
        let pos = vivos
            .iter()
            .map(|v| v.inicio)
            .min()
            .or(marcador);
        match pos {
            Some(p) => {
                let mut lista: Vec<&(usize, String)> = env_nuevos.iter().collect();
                lista.sort_by_key(|(i, _)| *i);
                let textos: Vec<String> = lista.into_iter().map(|(_, t)| t.clone()).collect();
                colocar_en(&mut colocacion, p, textos, 0);
            }
            None => {
                let mut lista: Vec<&(usize, String)> = env_nuevos.iter().collect();
                lista.sort_by_key(|(i, _)| *i);
                colocacion.al_final.extend(lista.into_iter().map(|(_, t)| t.clone()));
            }
        }
    }

    // --- wrappers nuevos: uno a uno, contra los vivos ---
    let mut ordenados: Vec<&(usize, String)> = wrapper_nuevos.iter().collect();
    ordenados.sort_by_key(|(i, _)| *i);

    for (_, texto) in ordenados {
        let nombre = texto.trim_matches('"');
        let Some(regla) = regla_wrapper(nombre) else {
            // Sin regla no se sabe dónde va: no se mete a ciegas.
            continue;
        };

        // Wrapper vivo de capa MENOR (más exterior) más cercano por la derecha: nos
        // metemos dentro de él, si admite algo dentro.
        let mut pos: Option<usize> = None;

        // Un wrapper de capa MENOR es más exterior: si existe, nosotros entramos dentro de
        // él, en su zona envuelta (detrás de su `--`).
        let exterior = vivos
            .iter()
            .filter(|v| v.regla.map(|r| r.capa < regla.capa).unwrap_or(false))
            .min_by_key(|v| regla.capa - v.regla.map(|r| r.capa).unwrap_or(0));
        if let Some(v) = exterior {
            pos = Some(v.zona_envuelta);
        }

        // Si no nos envuelve nadie, nos ponemos por FUERA del más interior que haya: delante
        // de su inicio. `mangohud` (capa 2) con `gamescope` (capa 3) vivo va delante, y si
        // gamescope tuviera zona envuelta, sería dentro de ella por el paso anterior.
        if pos.is_none() {
            if let Some(v) = vivos
                .iter()
                .filter(|v| v.regla.map(|r| r.capa > regla.capa).unwrap_or(true))
                .min_by_key(|v| v.regla.map(|r| r.capa).unwrap_or(u8::MAX))
            {
                pos = Some(v.inicio);
            }
        }

        // 3) Si no hay wrappers vivos, delante del marcador.
        let pos = match pos.or(marcador) {
            Some(p) => p,
            None => continue,
        };

        // Misma capa con otro nombre ya presente: el orden relativo no lo dicen las reglas.
        // En vez de elegir uno, no se inserta.
        let choca = vivos
            .iter()
            .any(|v| v.regla.map(|r| r.capa) == Some(regla.capa) && v.nombre != nombre);
        if choca {
            continue;
        }

        // Si justo a la izquierda hay un wrapper con separador `--`, este va detrás de él:
        // correcto. Si el punto cae delante de un wrapper más interior, no hace falta `--`.
        let mut texto_final = texto.clone();

        // Un wrapper con `--` que quede a la izquierda de su zona envuelta necesita que su
        // separador exista. Si el original no lo traía (p. ej. `gamescope -f %command%`),
        // hay que añadirlo al insertar algo dentro; si no, lo nuevo quedaría como opción de
        // gamescope y no como comando envuelto.
        let zona_de_un_solo_token = vivos.iter().any(|v| {
            v.zona_envuelta == pos
                && v.regla.map(|r| r.acepta_argumentos).unwrap_or(false)
                && !segmentos.iter().any(|s| {
                    s.valor == "--" && s.span.inicio > v.inicio && s.span.inicio < v.zona_envuelta
                })
        });
        if zona_de_un_solo_token {
            texto_final = format!("-- {texto_final}");
        }

        colocar_en(&mut colocacion, pos, vec![texto_final], regla.capa);
    }

    // --- args gestionados: delante del marcador, junto a los demás args del lanzador ---
    //
    // Van aquí y no en `al_final` porque `al_final` escribe **después** de todo, es decir
    // detrás del `%command%`, y ahí el argumento pasaría a ser argumento del juego en vez
    // de opción del lanzador (`%command% -novid` lanzaría el juego con `-novid`, que es otra
    // cosa). Delante del marcador es donde Steam los lee como opciones del lanzamiento.
    let mut args: Vec<&(usize, String)> = arg_nuevos.iter().collect();
    args.sort_by_key(|(i, _)| *i);
    if !args.is_empty() {
        let textos: Vec<String> = args.into_iter().map(|(_, t)| t.clone()).collect();
        match marcador {
            Some(p) => colocar_en(&mut colocacion, p, textos, 8),
            // Sin marcador no hay línea válida: se dejan al final para no inventar uno.
            None => colocacion.al_final.extend(textos),
        }
    }

    colocacion
}

/// Añade `textos` para que se escriban justo antes del byte `pos`, ordenados por `capa`.
fn colocar_en(colocacion: &mut Colocacion, pos: usize, textos: Vec<String>, capa: u8) {
    match colocacion.antes_de.iter_mut().find(|(p, _)| *p == pos) {
        Some((_, existentes)) => {
            let mut pares: Vec<(u8, String)> = existentes
                .drain(..)
                .map(|t| (pares_capa(&t), t))
                .collect();
            pares.extend(textos.into_iter().map(|t| (capa, t)));
            // Estable por capa: dentro de la misma capa se respeta el orden de llegada.
            pares.sort_by_key(|(c, _)| *c);
            existentes.extend(pares.into_iter().map(|(_, t)| t));
        }
        None => colocacion.antes_de.push((pos, textos)),
    }
}

/// Capa de un texto ya colocado, para poder reordenar de forma estable.
///
/// Un `env` (`FOO=1`) va siempre por fuera de cualquier wrapper: no ejecuta nada, solo pone
/// variables en el entorno, y eso tiene que valer también para el envoltorio. Por eso su
/// capa es `0` y no `u8::MAX`.
fn pares_capa(texto: &str) -> u8 {
    if clave_env(texto.trim_start_matches('"')).is_some() {
        return 0;
    }
    let limpio = texto.trim_start_matches('-').trim_start_matches(' ');
    regla_wrapper(limpio.split(' ').next().unwrap_or(limpio))
        .map(|r| r.capa)
        .unwrap_or(u8::MAX)
}

// ==================== VALIDACIÓN ====================

/// Máximo de caracteres de un comando de lanzamiento. Coincide con el límite que ya usaba
/// `profiles.rs` para los comandos de perfil.
pub const MAX_LONGITUD: usize = 2000;

/// Motivo por el que un comando no se puede usar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotivoRechazo {
    /// Código estable para que la UI decida qué enseñar sin comparar textos.
    ///
    /// Valores posibles: `vacio`, `demasiado_largo`, `encadenamiento`, `sintaxis`,
    /// `sin_marcador`, `lanzador_desconocido`, `marcador_repetido`.
    pub codigo: String,
    /// Texto legible en español.
    pub mensaje: String,
}

impl MotivoRechazo {
    fn nuevo(codigo: &str, mensaje: impl Into<String>) -> Self {
        Self {
            codigo: codigo.to_string(),
            mensaje: mensaje.into(),
        }
    }
}

/// Ejecutables que Thoriapp acepta como primera palabra de un comando.
///
/// `manual_command` no lo escribe la app: llega de un perfil importado, que es un archivo
/// cualquiera. Tratarlo como una orden a ejecutar es el puente «texto importado →
/// instrucciones ejecutables» que hay que cerrar.
pub const LANZADORES_PERMITIDOS: [&str; 19] = [
    "gamemoderun",
    "mangohud",
    "gamescope",
    "prime-run",
    "vkbasalt",
    "obs-gamecapture",
    "game-performance",
    "scx_lavd",
    "WINEDLLOVERRIDES",
    "steam",
    "steam-native",
    "steam-runtime",
    "protontricks",
    "umu-run",
    "proton",
    "wine",
    "wine64",
    "winetricks",
    "env",
];

// ==================== WRAPPERS: POSICIÓN Y PRECEDENCIA ====================
//
// Un wrapper NO es conmutable con otro. `gamescope -- mangohud %command%` y
// `mangohud gamescope -- %command%` no hacen lo mismo: en el primero MangoHud instrumenta
// el cliente que lanza gamescope; en el segundo, MangoHud envuelve al compositor. Thoriapp
// optimiza y mide, así que no puede dejar que el orden de dos pulsaciones produzca
// configuraciones distintas sin avisar.
//
// Las reglas de esta tabla son la única fuente de verdad para colocar un wrapper nuevo.
// Solo cubren los wrappers que el catálogo de Thoriapp puede meter en un comando; un
// wrapper desconocido no se toca nunca.

/// Dónde tiene que ir un wrapper respecto a los demás.
///
/// El valor es la **capa** de precedencia: menor número es más exterior (más a la
/// izquierda). Dos wrappers de la misma capa no se reordenan entre sí: si ya estaban,
/// se respeta su orden; si ambos son nuevos, se avisa de que el orden no está garantizado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReglaWrapper {
    /// Nombre del programa envoltorio.
    pub nombre: &'static str,
    /// Capa de precedencia (menor = más exterior).
    pub capa: u8,
    /// ¿Acepta argumentos propios antes del comando envuelto?
    ///
    /// `true` significa que, si va a la izquierda de otro wrapper, **debe** llevar un
    /// separador `--` para que sus opciones no se coman el wrapper siguiente.
    pub acepta_argumentos: bool,
    /// ¿Su `<programa>` es el propio lanzador (Steam, Proton, Wine)?
    ///
    /// Un lanzador de sistema termina la cadena: nada de Thoriapp va a su derecha.
    pub es_lanzador_de_sistema: bool,
}

/// Tabla de precedencias. Menor capa = más exterior.
///
/// El criterio es el que aplican los propios proyectos cuando se combinan:
///
/// - `gamemoderun` primero: sube prioridad y `nice` del proceso, y eso tiene que abarcar
///   todo lo que se lance después, incluidos compositores.
/// - `scx_lavd` / `game-performance` / `prime-run`: gobernadores de CPU y selección de GPU.
///   Van fuera de la instrumentación para que la medición los incluya.
/// - `mangohud`: instrumenta el proceso que envuelve. Thoriapp lo quiere **por dentro** de
///   `gamescope` (`gamescope -- mangohud %command%`), porque así se mide el juego y no el
///   compositor. `mangohud gamescope -- %command%` mide gamescope, que no es lo que
///   interesa a una herramienta de optimización. Por eso su capa es MAYOR que la de
///   gamescope.
/// - `gamescope`: compositor. Sus opciones propias (`-W`, `-H`) preceden al comando
///   envuelto y necesitan `--` cuando hay algo detrás.
/// - `vkbasalt` / `obs-gamecapture`: capas de gráficos y de captura, cerca del juego.
/// - `env`: utilidad que ejecuta el comando que le sigue; lo más interior.
pub const REGLAS_WRAPPER: &[ReglaWrapper] = &[
    ReglaWrapper { nombre: "gamemoderun", capa: 0, acepta_argumentos: false, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "scx_lavd", capa: 1, acepta_argumentos: false, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "game-performance", capa: 1, acepta_argumentos: false, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "prime-run", capa: 1, acepta_argumentos: false, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "gamescope", capa: 2, acepta_argumentos: true, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "mangohud", capa: 3, acepta_argumentos: false, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "vkbasalt", capa: 4, acepta_argumentos: false, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "obs-gamecapture", capa: 4, acepta_argumentos: false, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "env", capa: 5, acepta_argumentos: false, es_lanzador_de_sistema: false },
    ReglaWrapper { nombre: "steam-native", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
    ReglaWrapper { nombre: "steam-runtime", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
    ReglaWrapper { nombre: "steam", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
    ReglaWrapper { nombre: "umu-run", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
    ReglaWrapper { nombre: "proton", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
    ReglaWrapper { nombre: "wine", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
    ReglaWrapper { nombre: "wine64", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
    ReglaWrapper { nombre: "winetricks", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
    ReglaWrapper { nombre: "protontricks", capa: 9, acepta_argumentos: false, es_lanzador_de_sistema: true },
];

/// La regla de un wrapper, si Thoriapp sabe colocarlo.
pub fn regla_wrapper(nombre: &str) -> Option<&'static ReglaWrapper> {
    REGLAS_WRAPPER.iter().find(|r| r.nombre == nombre)
}

/// Caracteres que permiten encadenar órdenes, redirigir o sustituir texto por la salida
/// de otro programa. Ninguno tiene uso legítimo en una línea de Steam.
pub const CARACTERES_PROHIBIDOS: [char; 9] = [';', '|', '&', '$', '`', '<', '>', '\n', '\r'];

/// ¿Se puede usar este comando tal cual?
///
/// Deliberadamente conservadora: ante la duda, rechaza. **No** sanea el texto (sanear un
/// comando de shell es una trampa: siempre queda un caso raro); solo verifica que tiene la
/// forma de una línea de lanzamiento de Steam.
///
/// Es la ÚNICA implementación del proyecto: `profiles.rs` llama aquí en vez de tener su
/// propia copia (antes había dos y ya dijeron cosas distintas).
pub fn validar(comando: &str) -> Result<(), MotivoRechazo> {
    let texto = comando.trim();

    if texto.is_empty() {
        return Err(MotivoRechazo::nuevo("vacio", "El comando está vacío."));
    }

    if texto.chars().count() > MAX_LONGITUD {
        return Err(MotivoRechazo::nuevo(
            "demasiado_largo",
            format!("El comando es demasiado largo (máximo {MAX_LONGITUD} caracteres)."),
        ));
    }

    if let Some(bad) = texto.chars().find(|c| CARACTERES_PROHIBIDOS.contains(c)) {
        let mostrado = if bad == '\n' || bad == '\r' {
            "un salto de línea".to_string()
        } else {
            format!("«{bad}»")
        };
        return Err(MotivoRechazo::nuevo(
            "encadenamiento",
            format!(
                "El comando lleva {mostrado}, que sirve para encadenar órdenes. Thoriapp solo \
                 acepta una orden de lanzamiento."
            ),
        ));
    }

    let tokens = tokenizar(texto);

    // Comillas sin cerrar: la línea se conserva íntegra al importarla, pero no se puede
    // dar por buena para lanzar. Steam la ejecutaría con un significado que ni el usuario
    // ni Thoriapp han pedido, así que aquí se rechaza explícitamente en vez de dejarla
    // pasar «porque tiene pinta de comando».
    if let Some(t) = tokens.iter().find(|t| !t.cerrado) {
        let comilla = if t.texto.contains('"') { '"' } else { '\'' };
        return Err(MotivoRechazo::nuevo(
            "sintaxis",
            format!(
                "El comando tiene una comilla {comilla} sin cerrar. Se conserva tal cual, pero \
                 no se puede usar para lanzar hasta arreglarlo."
            ),
        ));
    }

    // El ejecutable está ANTES de `%command%`: detrás solo van argumentos del juego
    // (`-novid`, `-dx11`…) que no son programas. Buscar en toda la línea daba falsos
    // positivos con cualquier bandera.
    let marcadores = tokens.iter().filter(|t| t.valor == "%command%").count();
    let hay_marcador = marcadores > 0;

    // Dos marcadores: Steam los sustituye los dos y el comando lanzado cambia de forma
    // imprevisible. No es una línea válida de lanzamiento.
    if marcadores > 1 {
        return Err(MotivoRechazo::nuevo(
            "marcador_repetido",
            "El comando lleva %command% más de una vez. Steam sustituiría el marcador en cada \
             sitio, así que el programa se lanzaría de forma imprevisible."
        ));
    }
    let cabeza: &[Token] = match tokens.iter().position(|t| t.valor == "%command%") {
        Some(i) => &tokens[..i],
        // Sin marcador no es una línea de Steam válida, pero se valida igual: el comando
        // empieza directamente por el programa.
        None => &tokens[..],
    };

    // Las asignaciones de entorno (`FOO=bar`) no son el programa.
    let Some(primero) = cabeza.iter().find(|t| clave_env(&t.valor).is_none()) else {
        if hay_marcador {
            // Solo variables de entorno + `%command%`: no hay programa propio que validar.
            return Ok(());
        }
        return Err(MotivoRechazo::nuevo(
            "sin_marcador",
            "El comando no lleva %command%, así que Steam no sabría qué lanzar.",
        ));
    };

    let programa = primero
        .valor
        .rsplit('/')
        .next()
        .unwrap_or(&primero.valor)
        .to_string();

    if !LANZADORES_PERMITIDOS.contains(&programa.as_str()) {
        return Err(MotivoRechazo::nuevo(
            "lanzador_desconocido",
            format!(
                "«{programa}» no es un lanzador conocido de Thoriapp. Solo se aceptan envoltorios \
                 de juegos (gamemoderun, mangohud, gamescope…) y el marcador %command%."
            ),
        ));
    }

    // Rutas absolutas y `~`: un comando legítimo se resuelve por PATH. Una ruta concreta
    // apunta a un binario arbitrario del disco.
    if let Some(ruta) = cabeza
        .iter()
        .find(|t| t.valor.starts_with('/') || t.valor.starts_with('~'))
    {
        return Err(MotivoRechazo::nuevo(
            "ruta_absoluta",
            format!(
                "El comando usa una ruta del sistema («{}»). Thoriapp solo acepta órdenes que se \
                 resuelven por PATH para no lanzar un binario cualquiera.",
                ruta.valor
            ),
        ));
    }

    Ok(())
}

// ==================== TESTS ====================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- quitar argumentos conservados ----
    //
    // Es lo único que la UI ofrece borrar desde la tarjeta de argumentos. El criterio de
    // «esto se puede quitar» tiene que vivir aquí y no en el frontend: si React decidiera
    // por su cuenta, dos pantallas podrían ofrecer borrar cosas distintas.

    fn analisis_de(texto: &str) -> Analisis {
        analizar(texto, &catalogo())
    }

    #[test]
    fn se_puede_quitar_un_argumento_desconocido() {
        let a = analisis_de("mangohud --foo %command%");
        let indice = a
            .segmentos
            .iter()
            .position(|s| s.texto == "--foo")
            .expect("el argumento desconocido tiene que estar en el análisis");

        let texto = quitar_segmento(&a, indice).expect("quitar un desconocido es seguro");

        assert_eq!(texto, "mangohud %command%");
    }

    #[test]
    fn quitar_un_argumento_del_medio_no_deja_un_hueco() {
        // Dos espacios seguidos serían un argumento vacío para el shell. Se quita uno de
        // los dos desconocidos que envuelven el marcador, no uno de después: los de
        // después de `%command%` son del juego y no se tocan.
        let a = analisis_de("--uno --dos %command%");
        let indice = a
            .segmentos
            .iter()
            .position(|s| s.texto == "--uno")
            .expect("el argumento tiene que estar");

        let texto = quitar_segmento(&a, indice).expect("es seguro quitarlo");

        assert!(!texto.contains("  "), "no puede quedar un hueco doble: {texto:?}");
        assert_eq!(texto, "--dos %command%");
    }

    #[test]
    fn no_se_quita_un_ajuste_que_thoriapp_gestiona() {
        // Quitarlo por aquí saltaría los toggles: el ajuste tiene que apagarse desde su
        // interruptor, no borrando texto.
        let a = analisis_de("mangohud %command%");
        let indice = a
            .segmentos
            .iter()
            .position(|s| s.texto == "mangohud")
            .expect("mangohud está");

        assert!(quitar_segmento(&a, indice).is_err());
    }

    #[test]
    fn no_se_quita_un_argumento_del_juego() {
        // Va después de `%command%`: es del juego, no de Thoriapp.
        let a = analisis_de("%command% -novid");
        let indice = a
            .segmentos
            .iter()
            .position(|s| s.texto == "-novid")
            .expect("el argumento del juego está");

        assert!(quitar_segmento(&a, indice).is_err());
    }

    #[test]
    fn un_indice_que_no_existe_da_error_en_vez_de_tocar_nada() {
        let a = analisis_de("%command%");
        assert!(quitar_segmento(&a, 99).is_err());
    }

    // ---- aceptación: el recorrido completo que hace el usuario ----
    //
    // El caso del informe. No es una prueba unitaria de una función: encadena análisis →
    // cambios → serialización → reanálisis, que es exactamente lo que hace la app al pulsar
    // un interruptor. Si algo se rompiera en el camino (duplicados, argumentos perdidos,
    // orden cambiado), aquí se vería.

    #[test]
    fn aceptacion_importar_desactivar_reactivar_y_restaurar() {
        let texto = "AMD_VULKAN_ICD=RADV game-performance mangohud %command%";
        // `AMD_VULKAN_ICD=RADV` no está en el catálogo de prueba: se comporta como
        // argumento conservado, que es el caso interesante.
        let mut a = analisis_de(texto);
        assert_eq!(a.original, texto);

        // Los ajustes que el motor reconoce dentro de ese texto.
        let activos: Vec<usize> = a
            .segmentos
            .iter()
            .filter_map(|s| match s.gestion {
                Gestion::Thoriapp { indice, .. } => Some(indice),
                _ => None,
            })
            .collect();
        assert!(activos.contains(&0), "game-performance tiene que reconocerse");
        assert!(activos.contains(&2), "mangohud tiene que reconocerse");

        // 1) Desactivar MangoHud.
        let sin_hud: Vec<usize> = activos.iter().copied().filter(|i| *i != 2).collect();
        let cambios = cambios_para(&a, &catalogo(), &sin_hud);
        let paso1 = intentar_serializar(&a, &catalogo(), &cambios).expect("serializa");
        assert!(!paso1.contains("mangohud"), "MangoHud ya no está: {paso1:?}");
        assert!(paso1.contains("game-performance"), "lo demás se queda: {paso1:?}");
        assert!(
            paso1.contains("AMD_VULKAN_ICD=RADV"),
            "el argumento desconocido no se pierde: {paso1:?}"
        );
        assert!(paso1.contains("%command%"), "el marcador sigue: {paso1:?}");

        // 2) Volver a activarlo: tiene que quedar **una** vez, no dos.
        a = analisis_de(&paso1);
        let con_hud = vec![0, 2];
        let paso2 = intentar_serializar(&a, &catalogo(), &cambios_para(&a, &catalogo(), &con_hud))
            .expect("serializa");
        assert_eq!(
            paso2.matches("mangohud").count(),
            1,
            "reactivar no puede duplicar: {paso2:?}"
        );

        // 3) Añadir un ajuste que no estaba.
        a = analisis_de(&paso2);
        let con_extra = vec![0, 2, 3];
        let paso3 = intentar_serializar(&a, &catalogo(), &cambios_para(&a, &catalogo(), &con_extra))
            .expect("serializa");
        assert!(paso3.contains("DXVK_HUD=fps"), "el ajuste nuevo entra: {paso3:?}");
        assert!(
            paso3.contains("AMD_VULKAN_ICD=RADV"),
            "y el desconocido sigue ahí: {paso3:?}"
        );

        // 4) Restaurar el importado es volver al texto exacto, no a uno equivalente.
        a = analisis_de(&paso3);
        let restaurado = intentar_serializar(&a, &catalogo(), &cambios_para(&a, &catalogo(), &[0, 2, 3]))
            .expect("serializa");
        assert_eq!(restaurado, paso3, "con los mismos ajustes no cambia nada");
    }

    #[test]
    fn aceptacion_los_argumentos_del_juego_no_se_mueven_en_todo_el_recorrido() {
        // Un caso con comillas, variables y argumentos tras el marcador, que es donde más
        // fácil es que un serializador descuidado reordene o pierda algo.
        let texto = r#"gamemoderun mangohud "VAR=a b" %command% -novid --foo"#;
        let mut a = analisis_de(texto);

        for activos in [vec![2], vec![], vec![0, 2]] {
            let siguiente = intentar_serializar(&a, &catalogo(), &cambios_para(&a, &catalogo(), &activos))
                .expect("serializa");
            assert!(
                siguiente.ends_with("%command% -novid --foo"),
                "los argumentos del juego se quedan al final y en orden: {siguiente:?}"
            );
            assert!(
                siguiente.contains(r#""VAR=a b""#),
                "las comillas del original se conservan: {siguiente:?}"
            );
            a = analisis_de(&siguiente);
        }
    }

    #[test]
    fn un_env_nuevo_se_inserta_delante_del_wrapper() {
        let catalogo = vec![
            Definicion { indice: 0, valor: "mangohud".into(), kind: "wrapper".into(), proton_only: false },
            Definicion { indice: 1, valor: "RADV_PERFTEST=sam".into(), kind: "env".into(), proton_only: false },
        ];
        let a = analizar("mangohud %command%", &catalogo);
        let r = intentar_serializar(&a, &catalogo, &[cambio(&catalogo, 1, true)]);
        assert_eq!(r.unwrap(), "RADV_PERFTEST=sam mangohud %command%");
    }

    #[test]
    fn volcar_json_del_contrato_real() {
        let catalogo = vec![Definicion { indice: 0, valor: "game-performance".into(), kind: "wrapper".into(), proton_only: false }];
        let a = analizar("RADV_PERFTEST=sam game-performance %command%", &catalogo);
        println!("JSONSEG={}", serde_json::to_string(&a.segmentos).unwrap());
    }

    #[test]
    fn repro_pantalla_negra_7dtd() {
        let catalogo_real: Vec<Definicion> = crate::rules::for_appid(251570)
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| Definicion { indice: i, valor: p.value.clone(), kind: p.kind.clone(), proton_only: false })
            .collect();
        let texto = "RADV_PERFTEST=sam PROTON_USE_NTSYNC=1 game-performance mangohud %command%";
        let a = analizar(texto, &catalogo_real);
        for s in &a.segmentos {
            println!("SEG {:?} {:?} texto={:?}", s.rol, s.gestion, s.texto);
        }
        for e in estado_de_ajustes(&a, &catalogo_real) {
            println!("ESTADO {e:?}");
        }
        println!("--- serializar[] ---");
        println!("{:?}", intentar_serializar(&a, &catalogo_real, &[]));
    }

    #[test]
    fn aceptacion_un_comando_con_sintaxis_rota_no_se_da_por_bueno() {
        // Comilla sin cerrar: se conserva el texto, pero no se presenta como lanzable.
        let a = analisis_de(r#"mangohud "sin cerrar %command%"#);
        assert!(!a.sintaxis_valida);
        assert!(a.motivo_sintaxis.is_some());
        assert!(validar(&a.original).is_err(), "no se puede validar como lanzable");
        // Y aun así el texto no se ha tocado.
        assert_eq!(a.original, r#"mangohud "sin cerrar %command%"#);
    }

    // ---- catálogo de prueba ----
    //
    // No se usa `games.json` a propósito: el motor tiene que poder probarse sin el
    // catálogo real, y si mañana cambia un juego estos tests no deben moverse.

    /// Construye un `Cambio` a partir del índice del catálogo, como hace el frontend.
    ///
    /// El motor compara por `valor` (el índice cambia entre el catálogo del juego y el
    /// efectivo), así que la prueba tiene que rellenarlo con el valor real.
    fn cambio(catalogo: &[Definicion], indice: usize, activo: bool) -> Cambio {
        Cambio {
            indice,
            valor: catalogo
                .iter()
                .find(|d| d.indice == indice)
                .map(|d| d.valor.clone())
                .unwrap_or_default(),
            activo,
        }
    }

    fn catalogo() -> Vec<Definicion> {
        vec![
            Definicion {
                indice: 0,
                valor: "game-performance".into(),
                kind: "wrapper".into(),
                proton_only: false,
            },
            Definicion {
                indice: 1,
                valor: "PROTON_LOCAL_SHADER_CACHE=1".into(),
                kind: "env".into(),
                proton_only: false,
            },
            Definicion {
                indice: 2,
                valor: "mangohud".into(),
                kind: "wrapper".into(),
                proton_only: false,
            },
            Definicion {
                indice: 3,
                valor: "DXVK_HUD=fps".into(),
                kind: "env".into(),
                proton_only: false,
            },
            Definicion {
                indice: 4,
                valor: "RADV_PERFTEST=gpl".into(),
                kind: "env".into(),
                proton_only: false,
            },
            Definicion {
                indice: 5,
                valor: "DLSS en Calidad".into(),
                kind: "config".into(),
                proton_only: false,
            },
            // Mismo valor en dos definiciones: caso ambiguo.
            Definicion {
                indice: 6,
                valor: "PROTON_ENABLE_WAYLAND=1".into(),
                kind: "env".into(),
                proton_only: false,
            },
            Definicion {
                indice: 7,
                valor: "PROTON_ENABLE_WAYLAND=1".into(),
                kind: "env".into(),
                proton_only: false,
            },
            // Misma variable, otro valor: no es el mismo ajuste.
            Definicion {
                indice: 8,
                valor: "MANGOHUD_CONFIG=fps_limit=0".into(),
                kind: "env".into(),
                proton_only: false,
            },
        ]
    }

    fn cat() -> Vec<Definicion> {
        catalogo()
    }

    /// Analiza y serializa con los ajustes indicados, en un solo paso.
    fn con_activos(texto: &str, activos: &[usize]) -> String {
        let c = cat();
        let a = analizar(texto, &c);
        let cambios = cambios_para(&a, &catalogo(), &activos);
        serializar(&a, &c, &cambios)
    }

    fn segmentos_de<'a>(a: &'a Analisis, rol: Rol) -> Vec<&'a Segmento> {
        a.segmentos.iter().filter(|s| s.rol == rol).collect()
    }

    // ==================== WRAPPERS: POSICIÓN Y PRECEDENCIA ====================

    /// Catálogo con los wrappers que tienen regla de precedencia.
    fn cat_wrappers() -> Vec<Definicion> {
        let mut c = catalogo();
        let mut siguiente = 100;
        for nombre in [
            "gamescope",
            "gamemoderun",
            "prime-run",
            "vkbasalt",
            "env",
            "scx_lavd",
        ] {
            c.push(Definicion {
                indice: siguiente,
                valor: nombre.into(),
                kind: "wrapper".into(),
                proton_only: false,
            });
            siguiente += 1;
        }
        c
    }

    /// Índice del catálogo para un nombre de envoltorio en `cat_wrappers`.
    fn idx(nombre: &str) -> usize {
        cat_wrappers()
            .iter()
            .find(|d| d.valor == nombre)
            .unwrap()
            .indice
    }

    /// Añade los ajustes indicados a lo que la línea ya tenga activo.
    ///
    /// Es el caso real: el usuario importa un comando de Steam y enciende un toggle más.
    /// A diferencia de `con_activos`, no apaga nada de lo que hubiera.
    fn anadiendo(texto: &str, anadir: &[usize]) -> String {
        let c = cat_wrappers();
        let a = analizar(texto, &c);
        let mut lista: Vec<Cambio> = (0..c.len())
            .map(|i| cambio(&c, c[i].indice, false))
            .collect();
        for ind in a.ajustes_activos() {
            if let Some(x) = lista.iter_mut().find(|c| c.indice == ind) {
                x.activo = true;
            }
        }
        for ind in anadir {
            if let Some(x) = lista.iter_mut().find(|c| c.indice == *ind) {
                x.activo = true;
            }
        }
        serializar(&a, &c, &lista)
    }

    #[test]
    fn mangohud_se_anade_por_dentro_de_gamescope() {
        // Lo que interesa medir es el juego, no el compositor. Con `--` de por medio,
        // mangohud va detrás del separador.
        assert_eq!(
            anadiendo("gamescope -W 1920 -H 1080 -f -- %command%", &[idx("mangohud")]),
            "gamescope -W 1920 -H 1080 -f -- mangohud %command%"
        );
    }

    #[test]
    fn gamescope_se_anade_por_fuera_de_mangohud() {
        assert_eq!(
            anadiendo("mangohud %command%", &[idx("gamescope")]),
            "gamescope mangohud %command%"
        );
    }

    #[test]
    fn gamemoderun_va_por_fuera_de_todo() {
        assert_eq!(
            anadiendo("mangohud %command%", &[idx("gamemoderun")]),
            "gamemoderun mangohud %command%"
        );
        assert_eq!(
            anadiendo("gamescope -- %command%", &[idx("gamemoderun")]),
            "gamemoderun gamescope -- %command%"
        );
        assert_eq!(
            anadiendo("gamemoderun %command%", &[idx("mangohud")]),
            "gamemoderun mangohud %command%"
        );
    }

    #[test]
    fn el_orden_de_dos_wrappers_no_depende_del_orden_de_pulsacion() {
        // Este es el motivo de todo el modelo de capas: si pulsar A y luego B diera un
        // comando distinto de pulsar B y luego A, la app produciría dos configuraciones
        // distintas sin avisar, y una de ellas mediría el compositor en vez del juego.
        let a_b = anadiendo("%command%", &[idx("mangohud"), idx("gamescope")]);
        let b_a = anadiendo("%command%", &[idx("gamescope"), idx("mangohud")]);
        assert_eq!(a_b, b_a);
        assert_eq!(a_b, "gamescope mangohud %command%");
    }

    #[test]
    fn gamescope_y_mangohud_no_son_equivalentes() {
        // Las dos líneas son válidas y se analizan bien, pero NO son la misma configuración:
        // una mide el juego y la otra el compositor. El motor tiene que distinguirlas.
        let c = cat_wrappers();
        let dentro = analizar("gamescope -- mangohud %command%", &c);
        let fuera = analizar("mangohud gamescope -- %command%", &c);
        assert!(dentro.editable && fuera.editable);
        assert_ne!(dentro.original, fuera.original);
        // Y al serializar sin cambios cada uno se queda como estaba: no se normaliza nada.
        assert_eq!(serializar(&dentro, &c, &[]), "gamescope -- mangohud %command%");
        assert_eq!(serializar(&fuera, &c, &[]), "mangohud gamescope -- %command%");
        // Las capas confirman que uno envuelve al otro.
        assert!(regla_wrapper("gamescope").unwrap().capa < regla_wrapper("mangohud").unwrap().capa);
    }

    #[test]
    fn los_argumentos_de_gamescope_no_son_del_juego_ni_se_tocan() {
        let c = cat_wrappers();
        let a = analizar("gamescope -W 1920 -H 1080 -f -- %command% -novid", &c);
        let arg_wrapper = segmentos_de(&a, Rol::ArgWrapper);
        assert_eq!(
            arg_wrapper.iter().map(|s| s.texto.as_str()).collect::<Vec<_>>(),
            vec!["-W", "1920", "-H", "1080", "-f", "--"]
        );
        // `-novid` sí es del juego.
        assert_eq!(
            segmentos_de(&a, Rol::ArgJuego)
                .iter()
                .map(|s| s.texto.as_str())
                .collect::<Vec<_>>(),
            vec!["-novid"]
        );
    }

    #[test]
    fn si_gamescope_no_trae_separador_se_anade_al_meter_algo_dentro() {
        // `gamescope -f mangohud %command%` haría que mangohud fuese una opción de gamescope.
        // Al insertar dentro sin `--` previo, el motor pone el separador.
        assert_eq!(
            anadiendo("gamescope -f %command%", &[idx("mangohud")]),
            "gamescope -f -- mangohud %command%"
        );
    }

    #[test]
    fn un_wrapper_desconocido_no_se_toca_ni_lo_atraviesa_lo_nuevo() {
        // `mystery-wrap` no está en la tabla: no se sabe qué envuelve. Lo que se añade se
        // coloca contando con él como caja negra, sin reordenarlo.
        let c = cat_wrappers();
        let a = analizar("mystery-wrap %command%", &c);
        assert_eq!(serializar(&a, &c, &[]), "mystery-wrap %command%");
        // Y sigue delante: el motor no lo reordena.
        let con_mango = anadiendo("mystery-wrap %command%", &[idx("mangohud")]);
        assert!(
            con_mango.starts_with("mystery-wrap") || con_mango.contains("mystery-wrap %command%"),
            "el wrapper desconocido no se mueve: {con_mango}"
        );
    }

    #[test]
    fn el_json_del_analisis_tiene_la_forma_que_espera_el_frontend() {
        // El frontend tiene un espejo de estos tipos en `src/lib/motorComando.ts`. Si aquí
        // cambia una etiqueta, allí el TypeScript sigue compilando y la UI lee `undefined`
        // sin quejarse: este test fija los nombres para que el cambio sea ruidoso.
        let c = cat_wrappers();
        let a = analizar(r#"DXVK_HUD=fps mangohud %command% -novid"#, &c);
        let json = serde_json::to_value(&a).unwrap();

        for campo in [
            "original",
            "segmentos",
            "avisos",
            "editable",
            "reconocidos",
            "conservados",
            "sintaxis_valida",
            "motivo_sintaxis",
        ] {
            assert!(json.get(campo).is_some(), "falta el campo `{campo}` en Analisis");
        }

        let seg = &json["segmentos"][0];
        for campo in ["span", "texto", "valor", "rol", "gestion", "cerrado"] {
            assert!(seg.get(campo).is_some(), "falta `{campo}` en Segmento");
        }
        assert_eq!(seg["rol"], "env");
        assert_eq!(seg["gestion"]["tipo"], "thoriapp");

        let wrap = json["segmentos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["rol"] == "wrapper")
            .unwrap();
        assert_eq!(wrap["gestion"]["tipo"], "thoriapp");
    }

    #[test]
    fn el_json_de_los_avisos_usa_las_etiquetas_del_frontend() {
        let c = cat_wrappers();
        let a = analizar(r#"FOO="sin cerrar %command%"#, &c);
        let json = serde_json::to_value(&a).unwrap();
        let aviso = json["avisos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["tipo"] == "comilla_sin_cerrar")
            .expect("tiene que haber un aviso de comilla sin cerrar");
        assert!(aviso.get("comilla").is_some());
        assert!(aviso.get("span").is_some());
        assert_eq!(json["sintaxis_valida"], false);
    }

    #[test]
    fn el_json_de_los_estados_usa_las_etiquetas_del_frontend() {
        let c = cat_wrappers();
        let a = analizar("DXVK_HUD=fps DXVK_HUD=fps %command%", &c);
        let estados = estado_de_ajustes(&a, &c);
        let json = serde_json::to_value(&estados).unwrap();
        let parcial = json
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["estado"] == "parcial")
            .unwrap_or_else(|| panic!("el duplicado tiene que dar estado parcial: {json}"));
        assert!(parcial.get("indice").is_some());
        assert!(parcial.get("gestionadas").is_some());
        assert!(parcial.get("conservadas").is_some());

        // Y el inactivo también lleva índice.
        let inactivo = json
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["estado"] == "inactivo")
            .expect("tiene que haber ajustes inactivos");
        assert!(inactivo.get("indice").is_some());
    }

    #[test]
    fn el_json_del_error_usa_las_etiquetas_del_frontend() {
        let e = ErrorAnalisis::IndiceDesconocido { indice: 7 };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["tipo"], "indice_desconocido");
        assert_eq!(json["indice"], 7);

        let e = ErrorAnalisis::TextoNoCoincide {
            segmento: 0,
            esperado: "a".into(),
            recibido: "b".into(),
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["tipo"], "texto_no_coincide");
    }

    // ==================== SINTAXIS, EDICIÓN Y VALIDACIÓN ====================

    #[test]
    fn una_comilla_sin_cerrar_se_conserva_pero_no_es_sintaxis_valida() {
        let c = cat_wrappers();
        let texto = r#"FOO="sin cerrar %command%"#;
        let a = analizar(texto, &c);
        // Se conserva íntegro.
        assert_eq!(a.original, texto);
        assert_eq!(serializar(&a, &c, &[]), texto);
        // Pero no es válido ni editable.
        assert!(!a.sintaxis_valida);
        assert!(!a.editable);
        assert!(a.motivo_sintaxis.is_some());
        // Y no se ofrece como comando para lanzar: la validación lo rechaza por gramática,
        // no por lanzador desconocido.
        let rechazo = validar(texto).unwrap_err();
        assert_eq!(rechazo.codigo, "sintaxis");
    }

    #[test]
    fn la_sintaxis_valida_no_es_lo_mismo_que_ser_editable() {
        let c = cat_wrappers();
        // Sin `%command%` la línea está bien escrita aunque no se pueda editar.
        let sin_marcador = analizar("mangohud -f", &c);
        assert!(!sin_marcador.editable, "no se puede editar sin marcador");
        assert!(
            sin_marcador.sintaxis_valida,
            "pero la sintaxis sí es válida: las comillas están bien"
        );
        assert!(sin_marcador.motivo_sintaxis.is_none());

        // Con duplicado, se puede editar y la sintaxis es válida; solo es ambiguo.
        let duplicado = analizar("DXVK_HUD=fps DXVK_HUD=fps %command%", &c);
        assert!(duplicado.editable);
        assert!(duplicado.sintaxis_valida);
    }

    #[test]
    fn un_wrapper_desconocido_no_rompe_la_sintaxis() {
        let c = cat_wrappers();
        let a = analizar("mystery-wrap %command%", &c);
        assert!(a.sintaxis_valida, "es un comando bien escrito");
        assert!(a.editable);
        assert_eq!(a.conservados, 1, "el wrapper se conserva, no se reconoce");
    }

    #[test]
    fn las_comillas_dobles_solo_escapan_lo_que_escapa_bash() {
        // Comprobado contra bash y `shlex`: dentro de `"..."` la barra solo escapa `$`,
        // `` ` ``, `"`, `\` y el salto de línea. Ante cualquier otra letra se conserva.
        let casos = [
            (r#"A="a\x""#, "A=a\\x"),
            (r#"A="a\$b""#, "A=a$b"),
            (r#"A="a\\b""#, "A=a\\b"),
            (r#"A="a\`b""#, "A=a`b"),
        ];
        for (texto, esperado) in casos {
            let t = tokenizar(texto);
            assert_eq!(t[0].valor, esperado, "al analizar {texto}");
        }
    }

    #[test]
    fn una_barra_antes_de_una_letra_se_conserva_al_ida_y_a_la_vuelta() {
        // El bug: el lexer se comía la barra y el valor quedaba `ax`. Al volver a escribir
        // el comando salía otro distinto del que había puesto el usuario.
        let c = cat_wrappers();
        let texto = r#"DXVK_HUD="a\x" %command%"#;
        let a = analizar(texto, &c);
        assert_eq!(serializar(&a, &c, &[]), texto);

        // Y sigue reconociéndose tras escribirlo de nuevo.
        let generado = format!("{} %command%", escapar_valor("DXVK_HUD=a\\x").unwrap());
        let re = analizar(&generado, &c);
        assert_eq!(
            re.segmentos[0].valor, r"DXVK_HUD=a\x",
            "el valor tiene que sobrevivir al viaje"
        );
    }

    #[test]
    fn las_comillas_simples_no_tienen_escapes() {
        // En `'...'` bash no interpreta la barra. `'a\'` es un error de comilla sin cerrar,
        // no `a'`.
        let t = tokenizar(r#"'a\b'"#);
        assert_eq!(t[0].valor, r"a\b");
        assert!(t[0].cerrado);
    }

    // ==================== ERRORES ESTRUCTURADOS ====================

    #[test]
    fn un_span_imposible_devuelve_error_y_conserva_el_original() {
        let c = cat_wrappers();
        let mut a = analizar("mangohud %command%", &c);
        a.segmentos[0].span.fin = 9_999;

        let r = intentar_serializar(&a, &c, &[]);
        assert!(
            matches!(r, Err(ErrorAnalisis::SpanFueraDeRango { .. })),
            "tiene que ser un error, no un éxito silencioso: {r:?}"
        );
        // Y el comando sigue siendo el original.
        assert_eq!(serializar(&a, &c, &[]), "mangohud %command%");
    }

    #[test]
    fn un_span_a_mitad_de_caracter_devuelve_error() {
        let c = cat_wrappers();
        let mut a = analizar("ñandú %command%", &c);
        // `ñ` ocupa 2 bytes: cortar en el byte 1 parte el carácter.
        a.segmentos[0].span.inicio = 1;

        let r = intentar_serializar(&a, &c, &[]);
        assert!(
            matches!(r, Err(ErrorAnalisis::SpanEnMitadDeCaracter { .. })),
            "{r:?}"
        );
        assert_eq!(serializar(&a, &c, &[]), "ñandú %command%");
    }

    #[test]
    fn un_texto_manipulado_devuelve_error() {
        let c = cat_wrappers();
        let mut a = analizar("mangohud %command%", &c);
        // El IPC dice que este segmento gestiona otra cosa.
        a.segmentos[0].texto = "mangohud --arbitrary".into();

        let r = intentar_serializar(&a, &c, &[]);
        assert!(
            matches!(r, Err(ErrorAnalisis::TextoNoCoincide { .. })),
            "no se puede declarar como gestionado algo que no está en el original: {r:?}"
        );
    }

    #[test]
    fn un_indice_que_no_esta_en_el_catalogo_devuelve_error() {
        let c = cat_wrappers();
        let a = analizar("%command%", &c);
        // Valor inexistente: `intentar_serializar` valida por valor, que es la identidad
        // real. Un índice suelto ya no basta para pedir un cambio.
        let cambios = [Cambio {
            indice: 424242,
            valor: "NO_EXISTE=1".into(),
            activo: true,
        }];

        let r = intentar_serializar(&a, &c, &cambios);
        assert!(
            matches!(r, Err(ErrorAnalisis::IndiceDesconocido { indice: 424242 })),
            "{r:?}"
        );
        // Ignorarlo en silencio haría que la UI mostrara un ajuste activo que no está.
        assert_eq!(serializar(&a, &c, &cambios), "%command%");
    }

    #[test]
    fn sin_cambios_es_un_exito_no_un_error() {
        let c = cat_wrappers();
        let a = analizar("mangohud %command%", &c);
        assert_eq!(
            intentar_serializar(&a, &c, &[]),
            Ok("mangohud %command%".to_string())
        );
    }

    #[test]
    fn ningun_wrapper_nuevo_queda_detras_de_otro_por_orden_de_llegada() {
        // Se añaden todos los wrappers del catálogo de uno en uno, en el peor orden posible
        // (el inverso al de precedencia), y se comprueba que el resultado es el mismo que
        // añadirlos todos de golpe y que ninguno acaba fuera de su capa.
        let nombres = ["gamescope", "mangohud", "gamemoderun", "prime-run"];
        let todos: Vec<usize> = nombres.iter().map(|n| idx(n)).collect();

        let de_golpe = anadiendo("%command%", &todos);
        let mut en_serie = "%command%".to_string();
        for nombre in nombres.iter().rev() {
            en_serie = anadiendo(&en_serie, &[idx(nombre)]);
        }

        assert_eq!(
            en_serie, de_golpe,
            "el orden de pulsación no debe cambiar el comando resultante"
        );

        // Y cada uno aparece una sola vez.
        for nombre in nombres {
            assert_eq!(
                en_serie.matches(nombre).count(),
                1,
                "`{nombre}` tiene que aparecer exactamente una vez en {en_serie}"
            );
        }
    }

    #[test]
    fn quitar_un_wrapper_no_mueve_a_los_demas() {
        // Desactivar un ajuste no puede reordenar el resto: es el comando del usuario.
        let c = cat_wrappers();
        let original = "gamemoderun gamescope -- mangohud %command%";
        let a = analizar(original, &c);
        let activos = a.ajustes_activos();

        // Se apaga solo `gamescope`.
        let cambios: Vec<Cambio> = cat_wrappers()
            .iter()
            .map(|d| Cambio {
                indice: d.indice,
                valor: d.valor.clone(),
                activo: activos.contains(&d.indice) && d.valor != "gamescope",
            })
            .collect();
        let resultado = serializar(&a, &c, &cambios);

        assert_eq!(resultado, "gamemoderun mangohud %command%");
        // Gamemoderun sigue delante y mangohud detrás, en su orden original.
        let pos_gm = resultado.find("gamemoderun").unwrap();
        let pos_mh = resultado.find("mangohud").unwrap();
        assert!(pos_gm < pos_mh, "no se reordena lo que ya estaba: {resultado}");
    }

    #[test]
    fn los_argumentos_del_juego_nunca_se_reordenan_ni_se_pierden() {
        let c = cat_wrappers();
        let original = "%command% -novid -dx11 --foo=\"a b\"";
        let a = analizar(original, &c);
        assert_eq!(serializar(&a, &c, &[]), original);

        // Ni siquiera al añadir ajustes: los args del usuario se quedan detrás intactos.
        let con_mango = anadiendo(original, &[idx("mangohud")]);
        assert!(
            con_mango.ends_with("-novid -dx11 --foo=\"a b\""),
            "los argumentos del juego tienen que quedar al final, tal cual: {con_mango}"
        );
    }

    #[test]
    fn la_tabla_de_reglas_cubre_todos_los_lanzadores_permitidos() {
        // Si mañana se añade un lanzador a `LANZADORES_PERMITIDOS` y se olvida su capa, el
        // motor lo trataría como «sin regla» y no lo colocaría nunca. Este test lo detecta.
        //
        // `WINEDLLOVERRIDES` es la excepción conocida y documentada: no es un programa que
        // envuelva, es una variable (`WINEDLLOVERRIDES=x`), y por eso su lugar es la rama de
        // `env`, no la de wrappers. Fue heredada de `profiles.rs`, donde estaba mezclada con
        // los lanzadores por error.
        for w in LANZADORES_PERMITIDOS {
            if w == "WINEDLLOVERRIDES" {
                assert!(
                    regla_wrapper(w).is_none(),
                    "WINEDLLOVERRIDES es una variable de entorno, no un wrapper"
                );
                continue;
            }
            assert!(
                regla_wrapper(w).is_some(),
                "falta la regla de precedencia para `{w}`"
            );
        }
    }

    // ==================== LEXER ====================

    #[test]
    fn el_lexer_parte_por_espacios_simples() {
        let t = tokenizar("mesa_glthread=true mangohud %command% -novid");
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].valor, "mesa_glthread=true");
        assert_eq!(t[2].valor, "%command%");
        assert_eq!(t[3].valor, "-novid");
    }

    #[test]
    fn el_lexer_respeta_las_comillas_dobles() {
        let t = tokenizar(r#"env "FOO=a b" mangohud %command%"#);
        assert_eq!(t.len(), 4, "los espacios dentro de comillas no separan");
        assert_eq!(t[1].valor, "FOO=a b");
        // El texto conserva las comillas: es lo que se copia de vuelta.
        assert_eq!(t[1].texto, r#""FOO=a b""#);
    }

    #[test]
    fn el_lexer_respeta_las_comillas_simples_y_mezcladas() {
        let t = tokenizar("--ruta='a b' --x=\"c d\" %command%");
        assert_eq!(t[0].valor, "--ruta=a b");
        assert_eq!(t[1].valor, "--x=c d");
    }

    #[test]
    fn el_lexer_entiende_los_escapes() {
        let t = tokenizar(r#"FOO="a \"b\" c" %command%"#);
        assert_eq!(t[0].valor, r#"FOO=a "b" c"#);
        assert_eq!(t[0].texto, r#"FOO="a \"b\" c""#);
        assert!(t[0].cerrado);
    }

    #[test]
    fn el_lexer_marca_la_comilla_sin_cerrar() {
        let t = tokenizar(r#"FOO="sin cerrar %command%"#);
        assert_eq!(t.len(), 1);
        assert!(!t[0].cerrado);
    }

    #[test]
    fn el_lexer_no_parte_caracteres_utf8() {
        // Un acento dentro de comillas: si se avanzara byte a byte se rompería.
        let t = tokenizar(r#""camión"="sí" %command%"#);
        assert_eq!(t[0].valor, "camión=sí");
    }

    #[test]
    fn los_spans_apuntan_al_texto_exacto() {
        let texto = "  mangohud   %command%  -novid ";
        let t = tokenizar(texto);
        assert_eq!(t.len(), 3);
        for tok in &t {
            assert_eq!(tok.span.recortar(texto), tok.texto);
        }
        assert_eq!(t[0].span, Span::new(2, 10));
    }

    // ==================== ROUND-TRIP EXACTO ====================

    #[test]
    fn sin_cambios_devuelve_el_comando_exacto() {
        // Con `cambios_para` no hay diferencia entre lo que hay y lo que se pide: el
        // resultado tiene que ser idéntico, no solo equivalente.
        for texto in [
            "%command%",
            "mangohud %command%",
            "PROTON_LOCAL_SHADER_CACHE=1 gamemoderun %command% -novid",
            "env \"FOO=a  b\" mangohud %command%",
            "  mangohud    %command%   -dx11  ",
            "mesa_glthread=true DXVK_ASYNC=1 mangohud %command% +fps_max 144",
            "gamescope -W 2560 -H 1440 -- %command%",
        ] {
            let c = cat();
            let a = analizar(texto, &c);
            let activos = a.ajustes_activos();
            let cambios = cambios_para(&a, &catalogo(), &activos);
            assert!(cambios.is_empty(), "no debería haber cambios en {texto:?}");
            assert_eq!(
                serializar(&a, &c, &cambios),
                texto,
                "round-trip roto en {texto:?}"
            );
        }
    }

    #[test]
    fn sin_cambios_conserva_los_espacios_multiples() {
        // El espaciado raro es del usuario: reconstruir con `join(' ')` lo perdería.
        let texto = "mangohud    %command%    -novid";
        let c = cat();
        let a = analizar(texto, &c);
        assert_eq!(serializar(&a, &c, &[]), texto);
    }

    #[test]
    fn desactivar_y_volver_a_activar_devuelve_el_original() {
        // Ida y vuelta completa: quitar `mangohud` y volver a ponerlo debe dejar la misma
        // línea (mismo orden, mismos espacios), no una reordenada.
        let texto = "PROTON_LOCAL_SHADER_CACHE=1 mangohud %command% -novid";
        let c = cat();
        let a = analizar(texto, &c);

        let sin_mangohud = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: false,
            }],
        );
        assert_eq!(sin_mangohud, "PROTON_LOCAL_SHADER_CACHE=1 %command% -novid");

        // Volver a activarlo añade `mangohud` antes del marcador.
        let otra_vez = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: true,
            }],
        );
        assert_eq!(
            otra_vez,
            "PROTON_LOCAL_SHADER_CACHE=1 mangohud %command% -novid"
        );
    }

    // ==================== ARGUMENTOS DESCONOCIDOS ====================

    #[test]
    fn conserva_los_argumentos_que_no_conoce() {
        let c = cat();
        let a = analizar("mesa_glthread=true mangohud %command% -novid", &c);
        let desconocidos: Vec<&str> = a
            .segmentos
            .iter()
            .filter(|s| matches!(s.gestion, Gestion::Conservado { .. }))
            .map(|s| s.valor.as_str())
            .collect();
        assert!(desconocidos.contains(&"mesa_glthread=true"));
        assert_eq!(a.reconocidos, 1, "solo `mangohud` se reconoce");
    }

    #[test]
    fn los_argumentos_del_juego_nunca_se_tocan() {
        // `-novid` y `+fps_max 300` van después de `%command%`: son del juego.
        let c = cat();
        let a = analizar("mangohud %command% -novid +fps_max 300", &c);
        for s in segmentos_de(&a, Rol::ArgJuego) {
            assert!(matches!(s.gestion, Gestion::Conservado { .. }));
        }
        // Quitar un ajuste no puede llevarse los args.
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "%command% -novid +fps_max 300");
    }

    #[test]
    fn un_valor_del_catalogo_tras_el_marcador_sigue_siendo_del_juego() {
        // `mangohud` después de `%command%` es un argumento del juego, no el wrapper.
        let c = cat();
        let a = analizar("%command% mangohud", &c);
        let seg = segmentos_de(&a, Rol::ArgJuego);
        assert_eq!(seg.len(), 1);
        assert!(matches!(seg[0].gestion, Gestion::Conservado { .. }));
    }

    // ==================== VARIABLES DE ENTORNO ====================

    #[test]
    fn reconoce_las_variables_de_entorno_del_catalogo() {
        let c = cat();
        let a = analizar("PROTON_LOCAL_SHADER_CACHE=1 %command%", &c);
        assert_eq!(a.ajustes_activos(), vec![1]);
    }

    #[test]
    fn una_asignacion_vacia_de_clave_no_es_env() {
        let c = cat();
        let a = analizar("=raro %command%", &c);
        // No tiene clave: se trata como ejecutable del comando, no como env.
        assert_eq!(segmentos_de(&a, Rol::Env).len(), 0);
    }

    #[test]
    fn reconoce_el_env_aunque_venga_entrecomillado() {
        // `"PROTON_X=1"` es el mismo ajuste que `PROTON_X=1`.
        let c = cat();
        let a = analizar("\"PROTON_LOCAL_SHADER_CACHE=1\" %command%", &c);
        assert_eq!(a.ajustes_activos(), vec![1]);
    }

    // ==================== WRAPPERS Y ANIDAMIENTO ====================

    #[test]
    fn reconoce_varios_wrappers() {
        let c = cat();
        let a = analizar("game-performance mangohud %command%", &c);
        assert_eq!(a.ajustes_activos(), vec![0, 2]);
    }

    #[test]
    fn respeta_el_orden_de_los_wrappers_al_desactivar() {
        // `game-performance mangohud` y `mangohud game-performance` no son lo mismo en
        // cuanto a intención, pero el que queda debe conservar su sitio.
        let c = cat();
        let a = analizar("game-performance mangohud %command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 0,
                valor: "game-performance".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "mangohud %command%");
    }

    #[test]
    fn congela_la_posicion_de_un_wrapper_anidado() {
        // `gamescope -- %command%` no es del catálogo pero el marcador sigue detrás.
        let c = cat();
        let a = analizar("gamescope -W 2560 -- mangohud %command%", &c);
        assert!(a
            .segmentos
            .iter()
            .any(|s| s.valor == "gamescope" && s.rol == Rol::Ejecutable));
        assert_eq!(a.ajustes_activos(), vec![2], "mangohud sí se reconoce");
    }

    #[test]
    fn un_wrapper_que_no_esta_en_el_catalogo_no_se_gestiona() {
        // Está en `KNOWN_WRAPPERS` del sistema pero no es ajuste de este juego.
        let c = cat();
        let a = analizar("prime-run %command%", &c);
        assert!(a
            .segmentos
            .iter()
            .any(|s| s.valor == "prime-run" && s.rol == Rol::Ejecutable));
    }

    // ==================== DUPLICADOS ====================

    #[test]
    fn un_ajuste_repetido_se_gestiona_una_sola_vez() {
        let c = cat();
        let a = analizar("mangohud mangohud %command%", &c);
        assert_eq!(a.ajustes_activos(), vec![2], "solo el primero se gestiona");
        assert!(a
            .avisos
            .iter()
            .any(|x| matches!(x, Aviso::Duplicado { .. })));
        // Quitar el ajuste quita el gestionado, no los dos.
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "mangohud %command%");
    }

    // ==================== CONFLICTOS ====================

    #[test]
    fn detecta_dos_valores_distintos_de_la_misma_variable() {
        let c = cat();
        let a = analizar("DXVK_HUD=fps DXVK_HUD=full %command%", &c);
        let conflictos: Vec<&Aviso> = a
            .avisos
            .iter()
            .filter(|x| matches!(x, Aviso::Contradiccion { .. }))
            .collect();
        assert_eq!(conflictos.len(), 1);
        if let Aviso::Contradiccion { key, spans } = conflictos[0] {
            assert_eq!(key, "DXVK_HUD");
            assert_eq!(spans.len(), 2);
        }
    }

    #[test]
    fn mismo_nombre_distinto_valor_no_es_el_mismo_ajuste() {
        // El catálogo tiene `MANGOHUD_CONFIG=fps_limit=0`; el comando trae otro valor.
        // No se marca el ajuste como activo por compartir el nombre de la variable.
        let c = cat();
        let a = analizar("MANGOHUD_CONFIG=fps_limit=60 %command%", &c);
        assert!(a.ajustes_activos().is_empty());
        assert!(a.avisos.iter().any(
            |x| matches!(x, Aviso::MismoNombreDistintoValor { key, .. } if key == "MANGOHUD_CONFIG")
        ));
    }

    #[test]
    fn un_valor_ambiguo_no_se_elige_por_el_usuario() {
        let c = cat();
        let a = analizar("PROTON_ENABLE_WAYLAND=1 %command%", &c);
        assert!(a.ajustes_activos().is_empty(), "no se elige una de las dos");
        assert!(a
            .avisos
            .iter()
            .any(|x| matches!(x, Aviso::Ambiguo { candidatos, .. } if candidatos.len() == 2)));
        // Y se conserva: activar/desactivar otros ajustes no lo toca.
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: true,
            }],
        );
        assert_eq!(r, "PROTON_ENABLE_WAYLAND=1 mangohud %command%");
    }

    // ==================== PARÁMETROS ANTES Y DESPUÉS DEL MARCADOR ====================

    #[test]
    fn anadir_un_ajuste_lo_pone_antes_del_marcador() {
        let c = cat();
        let a = analizar("%command% -novid", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: true,
            }],
        );
        assert_eq!(r, "mangohud %command% -novid");
    }

    #[test]
    fn anadir_un_env_va_antes_que_el_wrapper() {
        let c = cat();
        let a = analizar("%command%", &c);
        let r = serializar(
            &a,
            &c,
            &[
                Cambio {
                    indice: 2,
                valor: "mangohud".into(),
                    activo: true,
                },
                Cambio {
                    indice: 1,
                valor: "PROTON_LOCAL_SHADER_CACHE=1".into(),
                    activo: true,
                },
            ],
        );
        assert_eq!(r, "PROTON_LOCAL_SHADER_CACHE=1 mangohud %command%");
    }

    #[test]
    fn los_config_del_catalogo_no_entran_en_el_comando() {
        // `DLSS en Calidad` es un ajuste dentro del juego: su texto rompería la línea.
        let c = cat();
        let a = analizar("%command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 5,
                valor: "DLSS en Calidad".into(),
                activo: true,
            }],
        );
        assert_eq!(r, "%command%");
    }

    // ==================== ESTRUCTURAS DE SHELL COMPLEJAS ====================

    #[test]
    fn no_edita_si_hay_una_comilla_sin_cerrar() {
        let c = cat();
        let texto = "mangohud \"sin cerrar %command%";
        let a = analizar(texto, &c);
        assert!(!a.editable);
        assert!(a
            .avisos
            .iter()
            .any(|x| matches!(x, Aviso::ComillaSinCerrar { .. })));
        // Aunque lleguen cambios, el texto sale intacto.
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: false,
            }],
        );
        assert_eq!(r, texto);
    }

    #[test]
    fn no_edita_si_falta_el_marcador() {
        let c = cat();
        let texto = "mangohud -novid";
        let a = analizar(texto, &c);
        assert!(!a.editable);
        assert!(a.avisos.iter().any(|x| matches!(x, Aviso::SinMarcador)));
        assert_eq!(
            serializar(
                &a,
                &c,
                &[Cambio {
                    indice: 2,
                valor: "mangohud".into(),
                    activo: false
                }]
            ),
            texto
        );
    }

    #[test]
    fn no_edita_si_hay_dos_marcadores() {
        let c = cat();
        let texto = "%command% %command%";
        let a = analizar(texto, &c);
        assert!(!a.editable);
        assert!(a
            .avisos
            .iter()
            .any(|x| matches!(x, Aviso::MarcadorRepetido { .. })));
        assert_eq!(
            serializar(
                &a,
                &c,
                &[Cambio {
                    indice: 2,
                valor: "mangohud".into(),
                    activo: true
                }]
            ),
            texto
        );
    }

    #[test]
    fn un_hueco_con_muchos_espacios_se_colapsa_al_borrar() {
        // Al quitar `mangohud` no deben quedar dos espacios seguidos.
        let c = cat();
        let a = analizar("gamemoderun   mangohud %command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: false,
            }],
        );
        assert!(!r.contains("  "), "quedaron espacios dobles: {r:?}");
    }

    #[test]
    fn borrar_el_ultimo_token_no_deja_espacio_final() {
        let c = cat();
        // `%command% mangohud` → mangohud es arg del juego, así que no se gestiona; se
        // prueba borrando un wrapper antes del marcador.
        let a = analizar("mangohud %command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "%command%");
        assert_eq!(r.trim(), r, "no hay espacios sobrantes");
    }

    // ==================== VALIDACIÓN ====================

    #[test]
    fn valida_un_comando_normal() {
        assert!(validar("%command%").is_ok());
        assert!(validar("mangohud %command% -novid").is_ok());
        assert!(validar("FOO=1 gamemoderun %command%").is_ok());
        assert!(validar("gamescope -W 1920 %command%").is_ok());
    }

    #[test]
    fn rechaza_el_encadenamiento_de_ordenes() {
        for malo in [
            "mangohud %command%; rm -rf /",
            "mangohud %command% | tee /tmp/x",
            "mangohud %command% && echo",
            "mangohud $(whoami) %command%",
            "mangohud `id` %command%",
            "mangohud %command% > /tmp/out",
            "mangohud %command%\nrm -rf /",
        ] {
            let r = validar(malo);
            assert!(r.is_err(), "debería rechazar {malo:?}");
            assert_eq!(r.unwrap_err().codigo, "encadenamiento");
        }
    }

    #[test]
    fn rechaza_un_lanzador_desconocido() {
        let r = validar("./juego %command%");
        assert_eq!(r.unwrap_err().codigo, "lanzador_desconocido");
    }

    #[test]
    fn rechaza_rutas_absolutas() {
        let r = validar("/usr/bin/mangohud %command%");
        assert_eq!(r.unwrap_err().codigo, "ruta_absoluta");
    }

    // ---- Fase 1.1: escapado de valores nuevos ----

    #[test]
    fn escapar_valor_no_toca_lo_que_no_necesita_comillas() {
        for v in ["PROTON_X=1", "mangohud", "DXVK_HUD=fps", "-dx11"] {
            assert_eq!(escapar_valor(v).unwrap(), v, "no debería cambiar {v}");
        }
    }

    #[test]
    fn escapar_valor_entrecomilla_lo_que_lleva_espacios() {
        assert_eq!(escapar_valor("A=b c").unwrap(), "\"A=b c\"");
        assert_eq!(
            escapar_valor("MANGOHUD_CONFIG=fps_limit=0,gpu_stats").unwrap(),
            "MANGOHUD_CONFIG=fps_limit=0,gpu_stats",
            "las comas no necesitan comillas"
        );
    }

    #[test]
    fn escapar_valor_escapa_comillas_y_barras() {
        assert_eq!(escapar_valor("A=\"x\"").unwrap(), "\"A=\\\"x\\\"\"");
        assert_eq!(escapar_valor("A=b\\c").unwrap(), "\"A=b\\\\c\"");
    }

    #[test]
    fn escapar_valor_entrecomilla_comodines_y_tilde() {
        // Sin comillas, el lanzador expandiría el comodín y el juego no vería el valor.
        assert_eq!(escapar_valor("A=*.txt").unwrap(), "\"A=*.txt\"");
        assert_eq!(escapar_valor("A=~/x").unwrap(), "\"A=~/x\"");
    }

    #[test]
    fn escapar_valor_rechaza_lo_que_encadena_ordenes() {
        for v in ["A=b;rm", "A=$(id)", "A=b|c", "A=b&c", "A=b>c"] {
            assert!(
                matches!(
                    escapar_valor(v),
                    Err(ValorNoSerializable::CaracterProhibido(_))
                ),
                "debería rechazar {v}"
            );
        }
    }

    #[test]
    fn escapar_valor_rechaza_el_vacio() {
        assert_eq!(escapar_valor(""), Err(ValorNoSerializable::Vacio));
    }

    #[test]
    fn un_ajuste_con_espacios_entra_como_un_solo_argumento() {
        // El caso que antes se rompía: el valor se insertaba tal cual y partía el comando.
        let c = vec![Definicion {
            indice: 0,
            valor: "MANGOHUD_CONFIG=fps_limit=0, position=top-left".into(),
            kind: "env".into(),
                proton_only: false,
            }];
        let a = analizar("%command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 0,
                valor: "MANGOHUD_CONFIG=fps_limit=0, position=top-left".into(),
                activo: true,
            }],
        );
        assert_eq!(
            r,
            "\"MANGOHUD_CONFIG=fps_limit=0, position=top-left\" %command%"
        );

        // Y al reanalizar sigue siendo UN argumento, no dos.
        let a2 = analizar(&r, &c);
        assert_eq!(a2.segmentos.len(), 2);
        assert_eq!(
            a2.segmentos[0].valor,
            "MANGOHUD_CONFIG=fps_limit=0, position=top-left"
        );
        assert_eq!(
            a2.ajustes_activos(),
            vec![0],
            "tiene que reconocerse de vuelta"
        );
    }

    #[test]
    fn un_ajuste_rechazado_no_entra_en_el_comando() {
        // Un valor con `;` no se mete ni escapado: el comando saldría encadenado.
        let c = vec![Definicion {
            indice: 0,
            valor: "A=1;rm -rf /".into(),
            kind: "env".into(),
                proton_only: false,
            }];
        let a = analizar("%command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 0,
                valor: "game-performance".into(),
                activo: true,
            }],
        );
        assert_eq!(r, "%command%", "no se añade nada");
        assert!(validar(&r).is_ok(), "y el comando sigue siendo válido");
    }

    // ---- Fase 1.1: equivalencia textual vs semántica ----

    #[test]
    fn una_asignacion_tras_el_marcador_es_un_arg_del_juego() {
        // `DXVK_HUD=fps` antes del marcador es un env; detrás, un argumento del juego.
        // No basta con que el texto coincida: manda el rol.
        let c = cat();
        let a = analizar("DXVK_HUD=fps %command% DXVK_HUD=fps", &c);
        let roles: Vec<Rol> = a.segmentos.iter().map(|s| s.rol.clone()).collect();
        assert_eq!(roles, vec![Rol::Env, Rol::Marcador, Rol::ArgJuego]);
        assert_eq!(
            a.ajustes_activos(),
            vec![3],
            "solo el de delante se gestiona (DXVK_HUD=fps)"
        );
    }

    #[test]
    fn dos_tokens_con_comillas_son_equivalentes_a_uno_sin_ellas() {
        // `"a b"` es UN argumento cuyo valor es `a b`. `a b` son DOS.
        let c = cat();
        let uno = analizar("--x=\"a b\" %command%", &c);
        let dos = analizar("--x=a b %command%", &c);
        assert_eq!(uno.segmentos.len(), 2, "el entrecomillado es un solo token");
        assert_eq!(dos.segmentos.len(), 3, "sin comillas son dos");
        assert_eq!(uno.segmentos[0].valor, "--x=a b");
        assert_eq!(dos.segmentos[0].valor, "--x=a");
        assert_eq!(dos.segmentos[1].valor, "b");
    }

    #[test]
    fn las_comillas_no_se_saltan_el_limite_entre_argumentos() {
        // Un valor NO puede salir a buscar el espacio de fuera de sus comillas.
        let c = cat();
        let a = analizar("A=\"x y\" B=z %command%", &c);
        let valores: Vec<&str> = a.segmentos.iter().map(|s| s.valor.as_str()).collect();
        assert_eq!(valores, vec!["A=x y", "B=z", "%command%"]);
    }

    #[test]
    fn el_mismo_valor_entrecomillado_o_no_es_el_mismo_ajuste() {
        // Equivalencia semántica: `MANGOHUD_CONFIG="fps_limit=0"` y `MANGOHUD_CONFIG=fps_limit=0`
        // producen el mismo valor, así que son el mismo ajuste.
        let c = vec![Definicion {
            indice: 0,
            valor: "MANGOHUD_CONFIG=fps_limit=0".into(),
            kind: "env".into(),
                proton_only: false,
            }];
        for texto in [
            "MANGOHUD_CONFIG=fps_limit=0 %command%",
            "MANGOHUD_CONFIG=\"fps_limit=0\" %command%",
            "\"MANGOHUD_CONFIG=fps_limit=0\" %command%",
        ] {
            let a = analizar(texto, &c);
            assert_eq!(a.ajustes_activos(), vec![0], "no reconoce {texto}");
            assert_eq!(a.segmentos[0].valor, "MANGOHUD_CONFIG=fps_limit=0");
        }
    }

    #[test]
    fn una_barra_solo_escapa_dentro_de_comillas_dobles() {
        // Semántica del lexer: `\"` es una comilla literal dentro de comillas dobles;
        // entre comillas simples, `\` es literal.
        let c = cat();
        // Dentro de comillas simples el `\` es literal: `'a \ b'` vale `a \ b`.
        // Dentro de dobles, `\"` es una comilla literal: `"c \" d"` vale `c " d`.
        let a = analizar(r#"'a \ b' "c \" d" %command%"#, &c);
        assert_eq!(a.segmentos[0].valor, "a \\ b");
        assert_eq!(a.segmentos[1].valor, "c \" d");
    }

    #[test]
    fn los_limites_de_token_se_respetan_con_escapes() {
        // `a\ b` es UN token con un espacio escapado, no dos.
        let c = cat();
        let a = analizar("A=a\\ b %command%", &c);
        assert_eq!(a.segmentos.len(), 2);
        assert_eq!(a.segmentos[0].valor, "A=a b");
        assert_eq!(a.segmentos[0].texto, "A=a\\ b");
    }

    // ---- Fase 1.1: estado por ajuste ----

    #[test]
    fn estado_activo_inactivo_y_conflicto() {
        let c = cat();
        let a = analizar("DXVK_HUD=fps %command%", &c);
        let estados = estado_de_ajustes(&a, &c);
        // catálogo de tests: 0 game-performance, 1 PROTON_LOCAL_SHADER_CACHE=1,
        // 2 mangohud, 3 DXVK_HUD=fps
        assert!(
            matches!(estados[0].situacion, Situacion::Inactivo),
            "{:?}",
            estados[0].situacion
        );
        assert!(
            matches!(estados[1].situacion, Situacion::Inactivo),
            "{:?}",
            estados[1].situacion
        );
        assert!(
            matches!(estados[2].situacion, Situacion::Inactivo),
            "{:?}",
            estados[2].situacion
        );
        assert!(
            matches!(estados[3].situacion, Situacion::Activo { .. }),
            "{:?}",
            estados[3].situacion
        );
    }

    #[test]
    fn estado_conflicto_cuando_la_variable_esta_ocupada_por_otro_valor() {
        let c = cat();
        // El catálogo tiene MANGOHUD_CONFIG=fps_limit=0 (índice 8); el comando trae otro valor.
        let a = analizar("MANGOHUD_CONFIG=fps_limit=60 %command%", &c);
        let estados = estado_de_ajustes(&a, &c);
        match &estados[8].situacion {
            Situacion::Conflicto { en_comando, .. } => {
                assert_eq!(
                    en_comando,
                    &vec!["MANGOHUD_CONFIG=fps_limit=60".to_string()]
                );
            }
            otro => panic!("se esperaba conflicto, salió {otro:?}"),
        }
    }

    #[test]
    fn estado_parcial_cuando_un_duplicado_no_se_gestiona() {
        let c = cat();
        // El ajuste aparece dos veces: la primera se gestiona, la segunda es duplicado.
        let a = analizar("mangohud mangohud %command%", &c);
        let estados = estado_de_ajustes(&a, &c);
        match &estados[2].situacion {
            Situacion::Parcial {
                gestionadas,
                conservadas,
            } => {
                assert_eq!(gestionadas.len(), 1);
                assert_eq!(
                    conservadas.len(),
                    1,
                    "queda una aparición viva que Thoriapp no controla"
                );
            }
            otro => panic!("se esperaba parcial, salió {otro:?}"),
        }
    }

    #[test]
    fn estado_ambiguo_cuando_el_valor_no_tiene_definicion_unica() {
        // Dos definiciones con el mismo valor: no se puede elegir una.
        let c = vec![
            Definicion {
                indice: 0,
                valor: "PROTON_X=1".into(),
                kind: "env".into(),
                proton_only: false,
            },
            Definicion {
                indice: 1,
                valor: "PROTON_X=1".into(),
                kind: "env".into(),
                proton_only: false,
            },
        ];
        let a = analizar("PROTON_X=1 %command%", &c);
        let estados = estado_de_ajustes(&a, &c);
        for e in &estados {
            assert!(
                matches!(&e.situacion, Situacion::Ambiguo { candidatos, .. } if candidatos.len() == 2),
                "se esperaba ambiguo, salió {e:?}"
            );
        }
    }

    #[test]
    fn un_ajuste_activo_no_se_marca_como_conflicto() {
        let c = cat();
        let a = analizar("DXVK_HUD=fps %command%", &c);
        let estados = estado_de_ajustes(&a, &c);
        assert!(
            !matches!(estados[3].situacion, Situacion::Conflicto { .. }),
            "el ajuste que está puesto no puede ser un conflicto consigo mismo"
        );
        assert!(
            !a.avisos
                .iter()
                .any(|x| matches!(x, Aviso::MismoNombreDistintoValor { .. })),
            "avisos: {:?}",
            a.avisos
        );
    }

    // ---- Fase 1.1: datos de IPC ----

    #[test]
    fn un_analisis_incoherente_no_reconstruye_nada() {
        // La UI podría mandar un modelo manipulado: span fuera del texto.
        let c = cat();
        let mut a = analizar("mangohud %command%", &c);
        a.segmentos[0].span = Span {
            inicio: 0,
            fin: 9999,
        };
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 0,
                valor: "game-performance".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "mangohud %command%", "debe devolver el original");
    }

    #[test]
    fn un_span_que_no_cae_en_limite_de_caracter_no_hace_panico() {
        let c = cat();
        let mut a = analizar("ñandú %command%", &c);
        // Cortar por la mitad de un carácter UTF-8.
        a.segmentos[0].span = Span { inicio: 0, fin: 1 };
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 0,
                valor: "game-performance".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "ñandú %command%");
    }

    #[test]
    fn un_span_solapado_no_hace_panico() {
        let c = cat();
        let mut a = analizar("mangohud gamemoderun %command%", &c);
        a.segmentos[1].span.inicio = 0; // solapado con el anterior
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 0,
                valor: "game-performance".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "mangohud gamemoderun %command%");
    }

    #[test]
    fn un_indice_de_catalogo_inexistente_se_ignora() {
        let c = cat();
        let a = analizar("%command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 999,
                valor: "".into(),
                activo: true,
            }],
        );
        assert_eq!(r, "%command%");
    }

    #[test]
    fn acepta_solo_variables_y_el_marcador() {
        // Sin ejecutable propio no hay nada que validar como programa.
        assert!(validar("PROTON_X=1 %command%").is_ok());
    }

    #[test]
    fn rechaza_sin_marcador_si_no_hay_ejecutable_permitido() {
        let r = validar("-novid");
        assert!(r.is_err());
    }

    #[test]
    fn rechaza_los_comandos_demasiado_largos() {
        let largo = format!("%command% {}", "a".repeat(MAX_LONGITUD));
        assert_eq!(validar(&largo).unwrap_err().codigo, "demasiado_largo");
    }

    #[test]
    fn rechaza_el_comando_vacio() {
        assert_eq!(validar("   ").unwrap_err().codigo, "vacio");
    }

    // ==================== SEGURIDAD: NO EJECUTA NADA ====================

    #[test]
    fn el_motor_no_ejecuta_comandos() {
        // Candado del requisito «no ejecutar durante el análisis»: si alguien añade un
        // `std::process::Command` a este módulo, este test falla y obliga a revisarlo.
        let fuente = include_str!("launch_command.rs");
        // Se busca el texto por partes para que este mismo test no se detecte a sí.
        let import = concat!("use std::", "process");
        let constructor = concat!("Command", "::new");
        assert!(
            !fuente.contains(import),
            "launch_command.rs no debe importar std::process"
        );
        assert!(
            !fuente.contains(constructor),
            "launch_command.rs no debe construir Command"
        );
    }

    // ==================== CONTRATO CON EL RESTO DEL PROYECTO ====================

    #[test]
    fn el_buffer_de_enfrentamiento_con_la_lista_de_wrappers_de_preflight_no_esta_vacia() {
        // El motor clasifica wrappers por el catálogo del juego, pero `validar` usa
        // `LANZADORES_PERMITIDOS`. Si alguien quita un wrapper de `preflight::KNOWN_WRAPPERS`
        // y no de aquí, un comando legítimo pasaría a ser rechazado.
        for w in crate::preflight::KNOWN_WRAPPERS {
            assert!(
                LANZADORES_PERMITIDOS.contains(&w),
                "el wrapper {w} está en preflight pero no en LANZADORES_PERMITIDOS"
            );
        }
    }

    // ==================== REGRESIONES DE SERIALIZACIÓN ====================
    //
    // Estas pruebas nacen de un bug real encontrado al escribir el motor: al borrar dos
    // tokens a la vez se aplicaban offsets ya desplazados y el comando salía partido
    // (`"PROTON_X=1 % command%"`). La causa era calcular posiciones en el texto resultante
    // en vez de recorrer el original en una sola pasada.

    #[test]
    fn borrar_dos_tokens_a_la_vez_no_parte_el_comando() {
        let c = cat();
        let a = analizar(
            "PROTON_LOCAL_SHADER_CACHE=1  mangohud   game-performance %command% -novid",
            &c,
        );
        let r = serializar(
            &a,
            &c,
            &[
                Cambio {
                    indice: 2,
                valor: "mangohud".into(),
                    activo: false,
                },
                Cambio {
                    indice: 0,
                valor: "game-performance".into(),
                    activo: false,
                },
            ],
        );
        assert_eq!(r, "PROTON_LOCAL_SHADER_CACHE=1 %command% -novid");
        assert!(r.contains("%command%"), "el marcador no se parte: {r:?}");
    }

    #[test]
    fn borrar_un_token_del_medio_deja_un_solo_espacio() {
        let c = cat();
        let a = analizar(
            "mangohud  PROTON_LOCAL_SHADER_CACHE=1   game-performance %command%",
            &c,
        );
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 1,
                valor: "PROTON_LOCAL_SHADER_CACHE=1".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "mangohud game-performance %command%");
    }

    #[test]
    fn borrar_todo_menos_el_marcador_deja_el_marcador_limpio() {
        let c = cat();
        let a = analizar("mangohud game-performance %command%", &c);
        let r = serializar(
            &a,
            &c,
            &[
                Cambio {
                    indice: 2,
                valor: "mangohud".into(),
                    activo: false,
                },
                Cambio {
                    indice: 0,
                valor: "game-performance".into(),
                    activo: false,
                },
            ],
        );
        assert_eq!(r, "%command%");
    }

    #[test]
    fn no_toca_el_espaciado_de_las_zonas_que_no_edita() {
        let c = cat();
        // Dos espacios entre los dos wrappers: son del usuario y siguen ahí.
        let a = analizar("mangohud  PROTON_LOCAL_SHADER_CACHE=1 %command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 3,
                valor: "DXVK_HUD=fps".into(),
                activo: false,
            }],
        );
        assert_eq!(r, "mangohud  PROTON_LOCAL_SHADER_CACHE=1 %command%");
    }

    #[test]
    fn respeta_los_espacios_dentro_de_las_comillas_al_editar() {
        let c = cat();
        let a = analizar("mangohud \"a  b\" %command%", &c);
        let r = serializar(
            &a,
            &c,
            &[Cambio {
                indice: 2,
                valor: "mangohud".into(),
                activo: false,
            }],
        );
        assert_eq!(
            r, "\"a  b\" %command%",
            "el valor conserva sus dos espacios"
        );
    }

    #[test]
    fn el_orden_canonico_al_anadir_es_env_luego_wrappers() {
        let c = cat();
        let a = analizar("mangohud %command%", &c);
        let r = serializar(
            &a,
            &c,
            &[
                Cambio {
                    indice: 0,
                valor: "game-performance".into(),
                    activo: true,
                },
                Cambio {
                    indice: 1,
                valor: "PROTON_LOCAL_SHADER_CACHE=1".into(),
                    activo: true,
                },
            ],
        );
        // Fase 1.1: el env nuevo va DELANTE del wrapper. Antes salía
        // `mangohud PROTON_LOCAL_SHADER_CACHE=1 game-performance %command%`, donde el env
        // quedaba aplicado solo al wrapper y no al juego.
        //
        // `mangohud` ya estaba en el comando y se queda donde estaba (el motor no mueve lo
        // del usuario); el bloque nuevo —el env y `game-performance`— entra delante del
        // primer wrapper vivo, ordenado por índice de catálogo.
        assert_eq!(
            r,
            "PROTON_LOCAL_SHADER_CACHE=1 game-performance mangohud %command%"
        );
    }

    #[test]
    fn los_lanzadores_permitidos_no_se_repiten() {
        let mut v = LANZADORES_PERMITIDOS.to_vec();
        let antes = v.len();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), antes, "hay lanzadores repetidos en la lista");
    }
}

// ==================== PROPIEDADES (property-based) ====================
//
// Los tests de arriba fijan casos concretos; estos comprueban que las invariantes se
// sostienen sobre entradas arbitrarias. `proptest` es dependencia **de desarrollo**: no
// entra en el binario.
//
// Estrategias: se generan piezas con forma de comando (env, wrappers, args, comillas,
// escapes, espacios) en vez de `String` arbitrarios, porque un texto basura aleatorio
// solo comprueba que no hay pánico, no que la lógica es correcta. Para el no-pánico sí se
// usa `String` puro.

#[cfg(test)]
mod propiedades {
    use super::*;
    use proptest::prelude::*;

    /// `Cambio` a partir del índice del catálogo. El motor identifica por `valor`.
    fn cambio(catalogo: &[Definicion], indice: usize, activo: bool) -> Cambio {
        Cambio {
            indice,
            valor: catalogo
                .iter()
                .find(|d| d.indice == indice)
                .map(|d| d.valor.clone())
                .unwrap_or_default(),
            activo,
        }
    }

    // ---- estrategias ----

    /// Catálogo estable para todas las propiedades.
    fn cat() -> Vec<Definicion> {
        vec![
            Definicion {
                indice: 0,
                valor: "mangohud".into(),
                kind: "wrapper".into(),
                proton_only: false,
            },
            Definicion {
                indice: 1,
                valor: "gamemoderun".into(),
                kind: "wrapper".into(),
                proton_only: false,
            },
            Definicion {
                indice: 2,
                valor: "PROTON_X=1".into(),
                kind: "env".into(),
                proton_only: false,
            },
            Definicion {
                indice: 3,
                valor: "DXVK_HUD=fps".into(),
                kind: "env".into(),
                proton_only: false,
            },
            Definicion {
                indice: 4,
                valor: "RADV_PERFTEST=gpl".into(),
                kind: "env".into(),
                proton_only: false,
            },
        ]
    }

    fn pieza_desconocida() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("mesa_glthread=true".to_string()),
            Just("DXVK_ASYNC=1".to_string()),
            Just("prime-run".to_string()),
            Just("WINEDLLOVERRIDES=x".to_string()),
        ]
    }

    /// Un token con comillas y escapes, del conjunto que el motor soporta.
    fn token_comillado() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("\"a b\"".to_string()),
            Just("'c d'".to_string()),
            Just("\"a \\\"b\\\" c\"".to_string()),
            Just("--x=\"p q\"".to_string()),
            Just("\"con  dos  espacios\"".to_string()),
        ]
    }

    fn token_valido() -> impl Strategy<Value = String> {
        prop_oneof![
            pieza_desconocida(),
            token_comillado(),
            Just("mangohud".to_string()),
            Just("gamemoderun".to_string()),
            Just("PROTON_X=1".to_string()),
            Just("-novid".to_string()),
            Just("+fps_max".to_string()),
            Just("300".to_string()),
        ]
    }

    /// Comando con forma real: [piezas] %command% [args].
    fn comando() -> impl Strategy<Value = String> {
        (
            prop::collection::vec(token_valido(), 0..5),
            prop::collection::vec(token_valido(), 0..4),
        )
            .prop_map(|(antes, despues)| {
                let mut s = antes.join(" ");
                if !s.is_empty() {
                    s.push(' ');
                }
                s.push_str("%command%");
                for d in despues {
                    s.push(' ');
                    s.push_str(&d);
                }
                s
            })
    }

    // ---- 1. round-trip exacto sin modificaciones ----

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..Default::default() })]

        #[test]
        fn prop_sin_cambios_devuelve_el_original(texto in comando()) {
            let c = cat();
            let a = analizar(&texto, &c);
            let activos = a.ajustes_activos();
            let cambios = cambios_para(&a, &c, &activos);
            prop_assert!(cambios.is_empty(), "no debería haber diferencias en {:?}", texto);
            prop_assert_eq!(serializar(&a, &c, &cambios), texto.clone());
        }

        // ---- 8. conservación semántica tras serializar y volver a analizar ----

        #[test]
        fn prop_reanalizar_da_los_mismos_segmentos(texto in comando()) {
            let c = cat();
            let a1 = analizar(&texto, &c);
            let activos = a1.ajustes_activos();
            let salida = serializar(&a1, &c, &cambios_para(&a1, &c, &activos));
            let a2 = analizar(&salida, &c);
            // Mismos valores, mismo orden, mismas gestión.
            let v1: Vec<(&str, &Rol)> = a1.segmentos.iter().map(|s| (s.valor.as_str(), &s.rol)).collect();
            let v2: Vec<(&str, &Rol)> = a2.segmentos.iter().map(|s| (s.valor.as_str(), &s.rol)).collect();
            prop_assert_eq!(v1, v2);
            prop_assert_eq!(a1.ajustes_activos(), a2.ajustes_activos());
        }

        // ---- 2. conservación de argumentos desconocidos ----

        #[test]
        fn prop_los_desconocidos_sobreviven_a_cualquier_cambio(texto in comando(), cambios in prop::collection::vec((0usize..5, any::<bool>()), 1..6)) {
            let c = cat();
            let a = analizar(&texto, &c);
            let lista: Vec<Cambio> = cambios
                .into_iter()
                .map(|(i, activo)| cambio(&c, i, activo))
                .collect();
            let salida = serializar(&a, &c, &lista);

            // Los tokens desconocidos que había al principio siguen todos en la salida.
            for seg in a.segmentos.iter().filter(|s| {
                matches!(s.gestion, Gestion::Conservado { .. }) && s.rol == Rol::Env
            }) {
                prop_assert!(
                    salida.contains(&seg.texto),
                    "se perdió {:?} de {:?} -> {:?}",
                    seg.texto, texto, salida
                );
            }
        }

        // ---- 3. integridad de los argumentos del juego ----

        #[test]
        fn prop_los_args_del_juego_nunca_se_tocan(texto in comando(), cambios in prop::collection::vec((0usize..5, any::<bool>()), 1..6)) {
            let c = cat();
            let a = analizar(&texto, &c);
            let lista: Vec<Cambio> = cambios
                .into_iter()
                .map(|(i, activo)| cambio(&c, i, activo))
                .collect();
            let salida = serializar(&a, &c, &lista);

            let args: Vec<String> = a
                .segmentos
                .iter()
                .filter(|s| s.rol == Rol::ArgJuego)
                .map(|s| s.texto.clone())
                .collect();
            // Aparecen en el mismo orden y sin que se cuele nada entre medias.
            let mut cursor = 0;
            for arg in &args {
                match salida[cursor..].find(arg.as_str()) {
                    Some(pos) => cursor += pos + arg.len(),
                    None => prop_assert!(false, "se perdió el arg {:?} en {:?}", arg, salida),
                }
            }
        }

        // ---- 4. eliminación de múltiples segmentos ----

        #[test]
        fn prop_desactivar_todo_deja_un_comando_valido(texto in comando()) {
            let c = cat();
            let a = analizar(&texto, &c);
            let lista: Vec<Cambio> = (0..c.len()).map(|i| cambio(&c, i, false)).collect();
            let salida = serializar(&a, &c, &lista);
            // Sigue habiendo exactamente un `%command%` y nada partido.
            prop_assert_eq!(salida.matches("%command%").count(), 1, "comando: {:?}", salida);
            // Sin espacios dobles FUERA de comillas: dentro de un valor entrecomillado
            // (`"con  dos  espacios"`) son parte del argumento y tienen que seguir ahí.
            let mut en_comilla = false;
            let mut previo_espacio = false;
            for ch in salida.chars() {
                match ch {
                    '"' => { en_comilla = !en_comilla; previo_espacio = false; }
                    ' ' if !en_comilla => {
                        prop_assert!(!previo_espacio, "espacios dobles fuera de comillas: {:?}", salida);
                        previo_espacio = true;
                    }
                    _ => previo_espacio = false,
                }
            }
        }

        // ---- 5. modificaciones consecutivas ----

        /// Dos pulsaciones sobre ajustes **distintos** dan lo mismo de golpe que en serie
        /// cuando ningún ajuste se solapa consigo mismo.
        ///
        /// ## Alcance real (fase 1.1)
        ///
        /// La propiedad completa —«el comando final solo depende del conjunto de ajustes,
        /// no del orden de las pulsaciones»— **no se cumple y no puede cumplirse** sin
        /// violar un requisito superior: los ajustes que ya estaban en el comando se quedan
        /// en su sitio, porque reordenar la línea del usuario es justo lo que no puede pasar.
        ///
        /// Ejemplo: con `%command%` y dos wrappers nuevos (`mangohud` índice 1 primero,
        /// `gamemoderun` índice 0 después) el resultado es `gamemoderun mangohud` (bloque
        /// ordenado); pero activándolos en serie cada uno se coloca en su hueco al llegar y
        /// queda `mangohud gamemoderun`. Ambos son comandos válidos y equivalentes en
        /// semántica; solo cambia el orden de dos wrappers, que es conmutativo en la práctica.
        ///
        /// Lo que SÍ se garantiza y aquí se comprueba:
        ///  - un `env` nunca acaba detrás de un wrapper (el fallo de semántica real);
        ///  - el conjunto de ajustes presentes es el mismo por los dos caminos;
        ///  - el `%command%` y los args del juego no se duplican ni se pierden.
        #[test]
        fn prop_aplicar_cambios_en_serie_deja_el_mismo_conjunto_de_ajustes(
            texto in comando(),
            i1 in 0usize..5, a1 in any::<bool>(),
            i2 in 0usize..5, a2 in any::<bool>(),
        ) {
            prop_assume!(i1 != i2);
            let c = cat();
            let a = analizar(&texto, &c);
            // Si alguno de los dos YA está en el comando en una posición concreta, el
            // orden de los dos caminos no tiene por qué coincidir: el motor no mueve lo que
            // ya estaba. La propiedad solo aplica a ajustes nuevos.
            prop_assume!(!a.ajustes_activos().contains(&i1));
            prop_assume!(!a.ajustes_activos().contains(&i2));
            // Dos ajustes del MISMO rol insertados en serie se colocan cada uno al llegar
            // (el bloque crece por la izquierda), así que su orden relativo sigue a las
            // pulsaciones. El conjunto es el mismo, el orden interno no. Se exige la
            // propiedad fuerte solo entre roles distintos, que es donde importaba.
            prop_assume!(c.iter().find(|d| d.indice == i1).map(|d| d.kind.clone())
                != c.iter().find(|d| d.indice == i2).map(|d| d.kind.clone()));

            let de_golpe = serializar(
                &a,
                &c,
                &[cambio(&c, i1, a1), cambio(&c, i2, a2)],
            );
            let paso1 = serializar(&a, &c, &[cambio(&c, i1, a1)]);
            let en_serie = serializar(
                &analizar(&paso1, &c),
                &c,
                &[cambio(&c, i2, a2)],
            );

            // Mismo conjunto de ajustes activos por los dos caminos.
            prop_assert_eq!(
                analizar(&de_golpe, &c).ajustes_activos(),
                analizar(&en_serie, &c).ajustes_activos(),
                "de golpe {:?} vs en serie {:?}",
                de_golpe,
                en_serie
            );
            // Y en ninguno se pierde el marcador ni se duplica.
            prop_assert_eq!(de_golpe.matches("%command%").count(), 1);
            prop_assert_eq!(en_serie.matches("%command%").count(), 1);
        }

        /// Un `env` **nuevo** queda delante de todos los wrappers vivos.
        ///
        /// Es la parte de la propiedad que protege la semántica: `PROTON_X=1` detrás de
        /// `mangohud` se aplicaría al wrapper y no al juego. No se mira la posición global
        /// (eso incluiría lo que ya venía mal colocado en el original, que el motor no
        /// reordena), sino el bloque nuevo respecto al primer wrapper vivo.
        #[test]
        fn prop_un_env_nuevo_nunca_queda_detras_de_un_wrapper(
            texto in comando(),
            i_env in prop::sample::select(vec![2usize, 3, 4]),
        ) {
            let c = cat();
            let a = analizar(&texto, &c);
            let env = c.iter().find(|d| d.indice == i_env).unwrap();
            // Solo tiene sentido si el env es nuevo.
            prop_assume!(!a.ajustes_activos().contains(&i_env));

            let salida = serializar(&a, &c, &[cambio(&c, i_env, true)]);
            let Some(salida_env) = analizar(&salida, &c).segmentos.iter().find(|s| s.valor == env.valor).map(|s| s.span.inicio) else {
                return Ok(());
            };

            // Todo wrapper vivo en la salida tiene que estar DETRÁS del env nuevo.
            for w in analizar(&salida, &c).segmentos.iter().filter(|s| s.rol == Rol::Wrapper) {
                prop_assert!(
                    salida_env < w.span.inicio,
                    "el env nuevo {:?} quedó detrás del wrapper {:?} en {:?}",
                    env.valor,
                    w.texto,
                    salida
                );
            }
        }

        // ---- 6. spans válidos y límites UTF-8 / sin pánicos ----

        #[test]
        fn prop_los_spans_son_validos_y_no_hay_panicos(texto in comando()) {
            let c = cat();
            let a = analizar(&texto, &c);
            let mut anterior = 0;
            for seg in &a.segmentos {
                prop_assert!(seg.span.inicio <= seg.span.fin);
                prop_assert!(seg.span.fin <= texto.len());
                prop_assert!(texto.is_char_boundary(seg.span.inicio));
                prop_assert!(texto.is_char_boundary(seg.span.fin));
                prop_assert_eq!(seg.span.recortar(&texto), seg.texto.as_str());
                prop_assert!(seg.span.inicio >= anterior);
                anterior = seg.span.fin;
            }
            // Cualquier entrada, incluso basura sin forma de comando, no revienta.
            let _ = analizar(&texto, &c);
            let _ = serializar(&a, &c, &[cambio(&c, 0, true)]);
        }
    }

    // ---- entradas arbitrarias: nada de pánicos ----

    proptest! {
        #![proptest_config(ProptestConfig { cases: 512, ..Default::default() })]

        #[test]
        fn prop_ninguna_entrada_arbitraria_hace_panico(basura in any::<String>()) {
            let c = cat();
            let a = analizar(&basura, &c);
            // Analizar y serializar devuelven algo siempre; el contenido no importa.
            let _ = serializar(&a, &c, &[cambio(&c, 0, true)]);
            let _ = serializar(&a, &c, &[cambio(&c, 0, false)]);
            let _ = validar(&basura);
            // Si no es editable, la salida es el original exacto.
            if !a.editable {
                prop_assert_eq!(serializar(&a, &c, &[cambio(&c, 0, true)]), basura.clone());
            }
        }

        /// Textos hechos de los caracteres que más problemas dan en un lexer.
        #[test]
        fn prop_los_caracteres_dificiles_no_rompen_el_lexer(
            texto in "[ \"'\\\\ab=\\t]{0,40}"
        ) {
            let c = cat();
            let a = analizar(&texto, &c);
            for seg in &a.segmentos {
                prop_assert!(texto.is_char_boundary(seg.span.inicio));
                prop_assert!(texto.is_char_boundary(seg.span.fin));
            }
            let _ = serializar(&a, &c, &[cambio(&c, 2, true)]);
        }

        // ---- 9. comillas, escapes, espacios y wrappers ----

        #[test]
        fn prop_escapar_valor_es_reversible(valor in "[a-zA-Z0-9_=:/.,-]{1,40}") {
            // Sin caracteres que necesiten comillas, el valor sale tal cual y se reconoce.
            let escapado = escapar_valor(&valor).expect("un valor sencillo se escapa");
            prop_assert_eq!(&escapado, &valor);
            let texto = format!("{} %command%", escapado);
            let c = vec![Definicion { indice: 0, valor: valor.clone(), kind: "env".into(), proton_only: false }];
            let a = analizar(&texto, &c);
            prop_assert_eq!(a.segmentos.len(), 2, "debería ser un solo token y el marcador");
            prop_assert_eq!(&a.segmentos[0].valor, &valor);
        }

        /// Un valor cualquiera (con espacios, comillas, escapes) ocupa UN solo token.
        ///
        /// Es la garantía que antes no existía: el motor no puede depender de que el
        /// catálogo no lleve espacios.
        #[test]
        fn prop_un_valor_con_espacios_sigue_siendo_un_argumento(
            valor in r#"[a-zA-Z0-9_=:/., "'-]{1,40}"#
        ) {
            let escapado = match escapar_valor(&valor) {
                Ok(v) => v,
                // Solo se rechaza vacío o con caracteres prohibidos: en esta estrategia no
                // hay, así que no debería ocurrir.
                Err(_) => return Ok(()),
            };
            let texto = format!("{} %command%", escapado);
            let a = analizar(&texto, &cat());
            prop_assert_eq!(a.segmentos.len(), 2, "el valor partido en varios tokens: {:?}", texto);
            prop_assert_eq!(&a.segmentos[0].valor, &valor);
            prop_assert_eq!(a.segmentos[1].valor.as_str(), "%command%");
        }

        /// Lo que `escapar_valor` escribe, el lexer lo lee idéntico.
        ///
        /// Cubre el fallo que se colaba antes: dentro de comillas simples se aplicaba el
        /// escape de la barra, así que `'a\b'` daba `ab`. Con valores que llevan `\`, el
        /// tipo de comilla elegido tenía que ser el correcto para que el viaje de ida y
        /// vuelta no perdiera la barra.
        #[test]
        fn prop_escapar_valor_sobrevive_al_lexer(
            valor in prop::collection::vec(
                prop::sample::select(vec![
                    'a', 'b', '1', '=', ' ', '\\', '"', '\'', '*', '~', '-', '.', 'ñ',
                ]),
                1..20,
            )
        ) {
            let valor: String = valor.into_iter().collect();
            // Solo tiene sentido para valores con forma de asignación `CLAVE=valor`: un
            // token suelto sería un wrapper o un ejecutable, no un env.
            prop_assume!(valor.contains('=') && !valor.starts_with('='));
            let Ok(escapado) = escapar_valor(&valor) else { return Ok(()) };
            let c = vec![Definicion { indice: 0, valor: valor.clone(), kind: "env".into(), proton_only: false }];
            let a = analizar(&format!("{escapado} %command%", ), &c);
            prop_assert_eq!(
                a.segmentos.len(),
                2,
                "el valor {:?} partió el comando: {:?}",
                valor,
                escapado
            );
            prop_assert_eq!(&a.segmentos[0].valor, &valor, "escapado: {:?}", escapado);
            // Y se reconoce de vuelta como el mismo ajuste.
            prop_assert_eq!(a.ajustes_activos(), vec![0]);
        }

        // ---- 10. conflictos y duplicidades ----

        #[test]
        fn prop_un_duplicado_no_se_gestiona_dos_veces(
            // Mismo token repetido (env o wrapper) antes del marcador.
            valor in prop::sample::select(vec!["mangohud", "PROTON_X=1", "DXVK_HUD=fps"]),
            veces in 2usize..5,
        ) {
            let c = cat();
            let partes: Vec<&str> = std::iter::repeat(valor).take(veces).collect();
            let texto = format!("{} %command%", partes.join(" "));
            let a = analizar(&texto, &c);
            prop_assert_eq!(a.ajustes_activos().len(), 1, "solo una aparición se gestiona");
            let duplicados = a.avisos.iter().filter(|x| matches!(x, Aviso::Duplicado { .. })).count();
            prop_assert_eq!(duplicados, veces - 1);
        }
    }
}
