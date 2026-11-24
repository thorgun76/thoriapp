//! Estado del vigilante: qué grabaciones ya se analizaron.
//!
//! Esto es lo único que necesita recordar la app entre sondeos. Se guarda en disco para
//! que reiniciar Thoriapp no vuelva a analizar (ni a duplicar en el historial) todas las
//! grabaciones de la carpeta.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Cuántas rutas se recuerdan como máximo.
///
/// La carpeta de MangoHud crece sin límite, y este fichero no puede hacerlo también. Al
/// superar el tope se descartan las entradas que llegaron primero (las más viejas por
/// orden de inserción), que son las grabaciones que el usuario ya no va a reanalizar.
pub const MAX_REMEMBERED: usize = 500;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WatchState {
    /// Rutas ya procesadas, en orden de llegada.
    seen: Vec<String>,
}

impl WatchState {
    /// Carga el estado. Un fichero corrupto o inexistente se trata como estado vacío:
    /// en el peor caso se reanaliza una grabación, que es mucho menos grave que
    /// arrancar con error.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear la carpeta de estado: {}", e))?;
        }
        let content = serde_json::to_string(self)
            .map_err(|e| format!("Error al serializar el estado: {}", e))?;
        fs::write(path, content).map_err(|e| format!("Error al guardar el estado: {}", e))
    }

    /// ¿Ya se procesó esta ruta?
    pub fn contains(&self, path: &str) -> bool {
        self.seen.iter().any(|p| p == path)
    }

    /// Marca una ruta como procesada, respetando el tope.
    pub fn mark(&mut self, path: &str) {
        if self.contains(path) {
            return;
        }
        self.seen.push(path.to_string());
        if self.seen.len() > MAX_REMEMBERED {
            let excess = self.seen.len() - MAX_REMEMBERED;
            self.seen.drain(..excess);
        }
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

/// Ruta del fichero de estado dentro del directorio de configuración de la app.
pub fn state_file(config_dir: &Path) -> PathBuf {
    config_dir.join("watch_state.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("thoriapp-ws-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("watch_state.json")
    }

    #[test]
    fn recordar_y_preguntar() {
        let mut st = WatchState::default();
        assert!(!st.contains("/a.csv"));
        assert!(st.is_empty());
        st.mark("/a.csv");
        assert!(st.contains("/a.csv"));
        assert_eq!(st.len(), 1);
    }

    #[test]
    fn marcar_dos_veces_no_duplica() {
        let mut st = WatchState::default();
        st.mark("/a.csv");
        st.mark("/a.csv");
        assert_eq!(st.len(), 1);
    }

    #[test]
    fn se_descartan_las_mas_viejas_al_pasarse_del_tope() {
        let mut st = WatchState::default();
        for i in 0..MAX_REMEMBERED + 5 {
            st.mark(&format!("/log-{}.csv", i));
        }
        assert_eq!(st.len(), MAX_REMEMBERED);
        // Las cinco primeras ya no están: son las que se olvidan.
        assert!(!st.contains("/log-0.csv"));
        assert!(!st.contains("/log-4.csv"));
        // La primera que se conserva y la última sí.
        assert!(st.contains("/log-5.csv"));
        assert!(st.contains(&format!("/log-{}.csv", MAX_REMEMBERED + 4)));
    }

    #[test]
    fn sobrevive_a_guardar_y_cargar() {
        let file = temp_file("persist");
        let mut st = WatchState::default();
        st.mark("/uno.csv");
        st.mark("/dos.csv");
        st.save(&file).unwrap();

        let leido = WatchState::load(&file);
        assert!(leido.contains("/uno.csv"));
        assert!(leido.contains("/dos.csv"));
        assert_eq!(leido.len(), 2);
    }

    #[test]
    fn un_fichero_corrupto_empieza_vacio_sin_error() {
        let file = temp_file("corrupto");
        fs::write(&file, "{ esto no es json").unwrap();
        let st = WatchState::load(&file);
        assert!(st.is_empty());
    }

    #[test]
    fn un_fichero_que_no_existe_empieza_vacio() {
        let st = WatchState::load(Path::new("/no/existe/watch_state.json"));
        assert!(st.is_empty());
    }
}
