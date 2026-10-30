use std::{fs, path::Path};

#[derive(Clone, serde::Serialize)]
pub struct Stats {
    pub file: String,
    pub frames: usize,
    pub avg_fps: f64,
    pub duration_s: f64,
    pub low1: f64,
    pub low01: f64,
}

fn newest_csv(dir: &Path) -> Result<String, String> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("No puedo leer {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            n.ends_with(".csv") && !n.contains("summary")
        })
        .collect();
    files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    files
        .last()
        .map(|e| e.path().to_string_lossy().to_string())
        .ok_or_else(|| "No hay ningún .csv de MangoHud en esa carpeta".into())
}

fn low(sorted_desc: &[f64], frac: f64) -> f64 {
    let n = ((sorted_desc.len() as f64 * frac).ceil() as usize).max(1);
    let mean = sorted_desc[..n].iter().sum::<f64>() / n as f64;
    1000.0 / mean
}

pub fn analyze(path: &str) -> Result<Stats, String> {
    let p = Path::new(path.trim());
    let file = if p.is_dir() { newest_csv(p)? } else { p.to_string_lossy().to_string() };
    let text = fs::read_to_string(&file).map_err(|e| format!("No puedo leer {file}: {e}"))?;
    let mut lines = text.lines();
    let header = lines
        .by_ref()
        .find(|l| l.starts_with("fps,"))
        .ok_or("No parece un log de MangoHud (no encuentro la cabecera fps,frametime)")?;
    let col = header
        .split(',')
        .position(|c| c.trim() == "frametime")
        .ok_or("El log no tiene columna frametime")?;
    let mut ft: Vec<f64> = lines
        .filter_map(|l| l.split(',').nth(col)?.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .collect();
    if ft.len() < 100 {
        return Err("El log tiene muy pocos frames; graba al menos unos 30 segundos".into());
    }
    let duration_s = ft.iter().sum::<f64>() / 1000.0;
    let avg = 1000.0 / (ft.iter().sum::<f64>() / ft.len() as f64);
    ft.sort_by(|a, b| b.partial_cmp(a).unwrap());
    Ok(Stats {
        file: file.rsplit('/').next().unwrap_or(&file).to_string(),
        frames: ft.len(),
        avg_fps: avg,
        duration_s,
        low1: low(&ft, 0.01),
        low01: low(&ft, 0.001),
    })
}
