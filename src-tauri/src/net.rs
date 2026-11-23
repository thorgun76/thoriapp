//! Descargas por red con límite de tamaño.
//!
//! Todas las fuentes externas de la app (Steam, ProtonDB, Proton Pulse,
//! AreWeAntiCheatYet) son datos que el usuario no controla. Sin un tope, una
//! respuesta enorme (o un servidor que no termina de enviar) hace que la app
//! intente leer gigabytes en memoria hasta quedarse sin ella.
//!
//! Aquí se lee a mano en trozos y se corta en cuanto se pasa del límite. No se
//! usa `into_string()` de `ureq` porque ese método lee la respuesta entera sin
//! preguntar cuánto ocupa.

use std::io::Read;
use std::time::Duration;

use ureq::Response;

/// Tope por defecto: ninguna de las respuestas que se piden llega ni de lejos,
/// y deja sitio para un `games.json` de AreWeAntiCheatYet que crezca.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Tope para las listas de anticheat, que son el archivo más grande.
pub const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;

/// Error legible en español para que el mensaje llegue tal cual a la interfaz.
#[derive(Debug)]
pub struct DownloadError(pub String);

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for DownloadError {}

fn describe(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, _) => format!("el servidor respondió {code}"),
        ureq::Error::Transport(t) => format!("no se pudo conectar: {t}"),
    }
}

/// Hace la petición y devuelve la respuesta, sin leer el cuerpo todavía.
fn request(url: &str, timeout: Duration) -> Result<Response, DownloadError> {
    ureq::get(url)
        .set("User-Agent", "thoriapp/1.0")
        .timeout(timeout)
        .call()
        .map_err(|e| DownloadError(describe(e)))
}

/// Lee el cuerpo de una respuesta como texto, cortando en `max_bytes`.
///
/// Se comprueba primero `Content-Length` para no descargar de más cuando el
/// servidor lo declara. Si miente (o no lo declara) se corta igualmente en
/// mitad de la lectura, así que el tope se respeta de las dos formas.
pub fn read_body(response: Response, max_bytes: usize) -> Result<String, DownloadError> {
    // `Content-Length` es opcional y puede venir mal: se usa solo como aviso
    // temprano, nunca como garantía.
    if let Some(len) = response
        .header("Content-Length")
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        if len > max_bytes {
            return Err(DownloadError(format!(
                "la respuesta ocupa {} bytes y el límite es {}",
                len, max_bytes
            )));
        }
    }

    // Se reserva como mucho el tamaño del límite, nunca lo que diga el servidor.
    let mut buf: Vec<u8> = Vec::new();
    let leidos = response
        .into_reader()
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|e| DownloadError(format!("respuesta ilegible: {e}")))?;

    // El `+1` es la trampa: si se han leído más de `max_bytes`, el cuerpo era
    // mayor que el límite aunque el servidor no lo dijera.
    if leidos > max_bytes {
        return Err(DownloadError(format!(
            "la respuesta supera el límite de {} bytes",
            max_bytes
        )));
    }

    String::from_utf8(buf).map_err(|_| DownloadError("la respuesta no es texto UTF-8".to_string()))
}

/// Descarga texto con el límite por defecto.
pub fn fetch_text(url: &str, timeout: Duration) -> Result<String, String> {
    let response = request(url, timeout).map_err(|e| e.0)?;
    read_body(response, MAX_RESPONSE_BYTES).map_err(|e| e.0)
}

/// Descarga texto con un límite explícito (para archivos grandes conocidos).
pub fn fetch_text_with_limit(
    url: &str,
    timeout: Duration,
    max_bytes: usize,
) -> Result<String, String> {
    let response = request(url, timeout).map_err(|e| e.0)?;
    read_body(response, max_bytes).map_err(|e| e.0)
}

/// Descarga y parsea JSON, con el mismo límite.
pub fn fetch_json(url: &str, timeout: Duration) -> Result<serde_json::Value, String> {
    let text = fetch_text(url, timeout)?;
    serde_json::from_str(&text).map_err(|e| format!("la respuesta no es JSON válido: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lector falso que produce `n` bytes sin reservar memoria por adelantado.
    /// Simula un cuerpo enorme (o una respuesta que no termina) sin pedir 8 MB
    /// reales ni tocar la red.
    struct LectorInfinito {
        quedan: usize,
    }

    impl Read for LectorInfinito {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.quedan == 0 {
                return Ok(0);
            }
            let n = buf.len().min(self.quedan);
            buf[..n].fill(b'a');
            self.quedan -= n;
            Ok(n)
        }
    }

    /// Aplica el mismo tope que `read_body` pero sobre un lector cualquiera, para
    /// poder probarlo sin construir un `ureq::Response`.
    fn leer_con_tope(lector: impl Read, max_bytes: usize) -> Result<Vec<u8>, String> {
        let mut buf = Vec::new();
        let leidos = lector
            .take((max_bytes + 1) as u64)
            .read_to_end(&mut buf)
            .map_err(|e| format!("respuesta ilegible: {e}"))?;
        if leidos > max_bytes {
            return Err(format!("la respuesta supera el límite de {} bytes", max_bytes));
        }
        Ok(buf)
    }

    #[test]
    fn lee_un_cuerpo_normal() {
        let lector = std::io::Cursor::new(b"hola mundo".to_vec());
        assert_eq!(leer_con_tope(lector, 1000).unwrap(), b"hola mundo");
    }

    #[test]
    fn acepta_justo_el_limite() {
        let lector = std::io::Cursor::new(vec![b'a'; 1000]);
        assert_eq!(leer_con_tope(lector, 1000).unwrap().len(), 1000);
    }

    #[test]
    fn corta_cuando_el_cuerpo_supera_el_limite() {
        // El caso que importa: el tope se aplica DURANTE la lectura, sin esperar
        // a tener el cuerpo entero en memoria.
        let err = leer_con_tope(std::io::Cursor::new(vec![b'a'; 5000]), 1000).unwrap_err();
        assert!(err.contains("supera el límite"), "mensaje: {err}");
    }

    #[test]
    fn un_cuerpo_infinito_se_corta_sin_colgarse() {
        // Simula un servidor que nunca termina de enviar: sin el tope, esto
        // llenaría la memoria hasta que el proceso muriera.
        let lector = LectorInfinito { quedan: usize::MAX };
        let err = leer_con_tope(lector, 4096).unwrap_err();
        assert!(err.contains("supera el límite"), "mensaje: {err}");
    }

    #[test]
    fn el_limite_por_defecto_es_razonable() {
        // Ni tan bajo que corte respuestas legítimas, ni tan alto que no proteja.
        assert!(MAX_RESPONSE_BYTES >= 1024 * 1024);
        assert!(MAX_CATALOG_BYTES >= MAX_RESPONSE_BYTES);
    }
}
