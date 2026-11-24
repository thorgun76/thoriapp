//! Candado para los tests que tocan la variable de entorno `HOME`.
//!
//! `HOME` es global al proceso y `cargo test` ejecuta los tests en paralelo, así que dos
//! tests que la reescriban se pisan: uno apunta `HOME` a su carpeta temporal, el otro
//! también, y el primero acaba leyendo (o no leyendo) los ficheros del segundo.
//!
//! Eso hacía fallar `steam_launch::tests::resuelve_por_nombre_cuando_el_appid_es_cero`
//! aproximadamente uno de cada cinco arranques, con un `left: None` que parecía un fallo
//! de lógica y era una carrera.
//!
//! La solución es `serial()`, que devuelve un guardián: mientras viva, ningún otro test
//! puede entrar en un bloque que también lo pida.
//!
//! ```ignore
//! let _guardia = crate::test_entorno::serial();
//! let anterior = std::env::var_os("HOME");
//! std::env::set_var("HOME", &temporal);
//! // ... test ...
//! // La restauración ocurre sola al salir del ámbito, aunque el test haga `panic!`.
//! ```
//!
//! Se resuelve a mano y no con la caja `serial_test` para no añadir una dependencia por
//! 30 líneas, igual que el resto del proyecto.

use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Candado de módulo, no por test: lo que se protege es la variable global.
static CANDADO: OnceLock<Mutex<()>> = OnceLock::new();

/// Reproduce y restaura `HOME` alrededor del test.
///
/// Existe para que la restauración ocurra **también si el test falla**: con el
/// `set_var`/restaurar a mano del final, un `panic!` dejaba `HOME` apuntando a una carpeta
/// temporal ya borrada y contaminaba a todos los tests siguientes del mismo proceso.
pub struct HomeTemporal {
    anterior: Option<OsString>,
    /// Solo para mantener sujeto el candado mientras vive esta estructura.
    _guardia: MutexGuard<'static, ()>,
}

impl HomeTemporal {
    /// Pone `HOME` en `ruta` y devuelve un guardián que lo restaura al salir del ámbito.
    pub fn nuevo(ruta: impl AsRef<std::path::Path>) -> Self {
        let guardia = serial();
        let anterior = std::env::var_os("HOME");
        std::env::set_var("HOME", ruta.as_ref());
        Self {
            anterior,
            _guardia: guardia,
        }
    }
}

impl Drop for HomeTemporal {
    fn drop(&mut self) {
        match self.anterior.take() {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }
}

/// Espera a que no haya ningún otro test tocando `HOME`.
///
/// Un test que se caiga mientras sujeta el candado lo envenenaría y todos los demás
/// fallarían con un `PoisonError` críptico. Se ignora el envenenamiento a propósito: los
/// tests comprueban el resultado de sus funciones, no la consistencia de este mutex.
pub fn serial() -> MutexGuard<'static, ()> {
    CANDADO
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    // Ojo: aquí **no** se lee `HOME` para comprobarlo.
    //
    // `DROP` sí restaura `HOME`, pero el resto de tests del proceso también la reescriben, así
    // que en cuanto este suelta el candado el valor global pasa a ser cosa de otro. Cualquier
    // aserción del tipo «`HOME` ahora vale X» es una carrera por diseño y daba falsos fallos.
    // Lo que se prueba es la mecánica del candado, que sí es determinista.
    use super::*;

    /// El candado es exclusivo: dos hilos no pueden estar a la vez dentro.
    ///
    /// Es lo que arregla el fallo intermitente: antes, dos tests reescribían `HOME` en
    /// paralelo y uno leía la Steam del otro.
    #[test]
    fn el_candado_es_exclusivo() {
        let candado = serial();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // Bloqneado hasta que el test suelte el candado de arriba.
            let _guardia = serial();
            let _ = tx.send(());
        });

        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(150)).is_err(),
            "otro hilo ha entrado en la sección crítica mientras estaba ocupada"
        );

        drop(candado);
        assert!(
            rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok(),
            "el candado no se libera al soltarlo"
        );
    }

    /// `HomeTemporal` escribe su ruta y la devuelve al salir, sin dejar la inventada.
    ///
    /// Se comprueba **mientras el guardián vive** (determinista) y, tras soltarlo, solo que
    /// la ruta del test no se ha quedado puesta.
    #[test]
    fn escribe_y_restaura() {
        let original = std::env::var_os("HOME");
        {
            let _entorno = HomeTemporal::nuevo("/ruta/del_test_entorno");
            assert_eq!(
                std::env::var_os("HOME"),
                Some(std::ffi::OsString::from("/ruta/del_test_entorno"))
            );
        }
        assert_ne!(
            std::env::var_os("HOME"),
            Some(std::ffi::OsString::from("/ruta/del_test_entorno")),
            "la ruta del test se ha quedado puesta tras soltar el guardián"
        );
        // `original` se lee solo para documentar el contrato; no se compara porque otro
        // test puede haber cambiado `HOME` en el hueco.
        let _ = original;
    }

    /// Si el test hace `panic!`, el `Drop` devuelve `HOME` igual.
    ///
    /// Ese era el fallo de fondo de los tests originales: restauraban a mano al final, así
    /// que un `panic!` dejaba `HOME` apuntando a una carpeta temporal ya borrada.
    #[test]
    fn restaura_aunque_el_test_falle() {
        let resultado = std::panic::catch_unwind(|| {
            let _entorno = HomeTemporal::nuevo("/ruta/que_el_panic_no_debe_dejar");
            panic!("fallo simulado");
        });
        assert!(resultado.is_err(), "el panic tiene que propagarse");
        assert_ne!(
            std::env::var_os("HOME"),
            Some(std::ffi::OsString::from("/ruta/que_el_panic_no_debe_dejar")),
            "el panic se llevó por delante la restauración de HOME"
        );
    }
}
