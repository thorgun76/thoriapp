use std::{env, fs, path::Path, process::Command};

#[derive(Default, Clone, Copy, PartialEq, Debug, serde::Serialize)]
pub enum Vendor {
    Amd,
    Nvidia,
    Intel,
    #[default]
    Unknown,
}

#[derive(Default, Clone, Debug, serde::Serialize)]
pub struct Hardware {
    pub gpu: String,
    pub vendor: Vendor,
    pub cpu: String,
    pub ram_gb: f32,
    pub kernel: String,
    pub distro: String,
    pub session: String,
    pub mesa: String,
    pub ntsync: bool,
    pub game_performance: bool,
    /// VRAM dedicada en MiB. `None` = no se pudo leer (iGPU sin memoria propia,
    /// `vulkaninfo` ausente o salida con formato distinto).
    pub vram_mb: Option<u64>,
    /// Hilos lógicos (`nproc`).
    pub cpu_threads: Option<u32>,
    /// Núcleos físicos, contando un solo hilo por núcleo en SMT.
    pub cpu_cores: Option<u32>,
    /// La CPU soporta AVX2. `None` = no se pudo determinar.
    pub cpu_avx2: Option<bool>,
}

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

/// ¿Está el ejecutable en el PATH? Se apoya en `preflight`, que además mira las rutas
/// habituales (una app abierta desde el menú puede heredar un PATH recortado).
fn in_path(bin: &str) -> bool {
    crate::preflight::find_binary(bin).is_some()
}

fn clean_gpu_name(raw: &str) -> String {
    let mut name = raw.to_string();
    
    if let Some(start) = name.rfind('[') {
        if let Some(end) = name[start..].find(']') {
            name = name[start + 1..start + end].to_string();
        }
    }
    
    if let Some(rev_pos) = name.find("(rev") {
        name = name[..rev_pos].trim().to_string();
    }
    
    name = name
        .replace("Advanced Micro Devices, Inc.", "")
        .replace("[AMD/ATI]", "")
        .replace("NVIDIA Corporation", "")
        .trim()
        .to_string();
    
    name = name.replace("/", " / ");
    
    while name.contains("  ") {
        name = name.replace("  ", " ");
    }
    
    name.trim().to_string()
}

pub fn detect() -> Hardware {
    let mut hw = Hardware::default();

    let lspci = run("lspci", &[]);
    let gpus: Vec<&str> = lspci
        .lines()
        .filter(|l| l.contains("VGA") || l.contains("3D controller") || l.contains("Display controller"))
        .collect();
    let pick = gpus
        .iter()
        .find(|l| l.contains("NVIDIA") || l.contains("AMD") || l.contains("ATI"))
        .or(gpus.first())
        .copied()
        .unwrap_or("");
    let raw_gpu = pick.split_once(": ").map(|x| x.1).unwrap_or("No detectada");
    hw.gpu = clean_gpu_name(raw_gpu);
    hw.vendor = if pick.contains("NVIDIA") {
        Vendor::Nvidia
    } else if pick.contains("AMD") || pick.contains("ATI") {
        Vendor::Amd
    } else if pick.contains("Intel") {
        Vendor::Intel
    } else {
        Vendor::Unknown
    };

    let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    hw.cpu = cpuinfo
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .unwrap_or("Desconocida")
        .trim()
        .to_string();
    let mem = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let kb: f32 = mem
        .lines()
        .find(|l| l.starts_with("MemTotal"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    hw.ram_gb = kb / 1024.0 / 1024.0;

    hw.kernel = run("uname", &["-r"]).trim().to_string();
    hw.distro = fs::read_to_string("/etc/os-release")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
        .unwrap_or_else(|| "Linux".into());
    hw.session = env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "desconocida".into());

    hw.mesa = run("vulkaninfo", &["--summary"])
        .lines()
        .find(|l| l.contains("driverInfo"))
        .and_then(|l| l.split('=').nth(1))
        .unwrap_or("")
        .trim()
        .to_string();

    hw.ntsync = Path::new("/dev/ntsync").exists();
    hw.game_performance = in_path("game-performance");

    let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    hw.cpu_threads = count_cpu_threads(&cpuinfo);
    hw.cpu_cores = count_cpu_cores(&cpuinfo);
    hw.cpu_avx2 = cpu_supports_avx2(&cpuinfo);
    hw.vram_mb = parse_vram_bytes(&run("vulkaninfo", &["--summary"]))
        .map(|b| b / (1024 * 1024));
    hw
}

// ==================== LECTURA PURA (testeable sin tocar el sistema) ====================

/// `nproc`: número de hilos lógicos que ve el sistema.
pub fn count_cpu_threads(cpuinfo: &str) -> Option<u32> {
    let n = cpuinfo.lines().filter(|l| l.starts_with("processor")).count();
    (n > 0).then_some(n as u32)
}

/// Núcleos físicos: `cpu cores` de `/proc/cpuinfo` ya lo dice en x86 (y repite el
/// mismo valor en cada hilo). Si no está, se cuenta una sola vez por
/// `physical id`+`core id`; si tampoco, se asume igual que los hilos.
///
/// Importa separarlo de los hilos porque los requisitos de los juegos hablan de
/// núcleos, y decir "tienes 4 cuando tienes 8 hilos" o al revés asusta o engaña.
pub fn count_cpu_cores(cpuinfo: &str) -> Option<u32> {
    if let Some(cores) = cpuinfo.lines().find_map(|l| {
        l.strip_prefix("cpu cores")?
            .split(':')
            .nth(1)?
            .trim()
            .parse::<u32>()
            .ok()
    }) {
        return (cores > 0).then_some(cores);
    }

    let ids: Vec<(String, String)> = cpuinfo
        .split("\n\n")
        .filter_map(|block| {
            let field = |key: &str| {
                block
                    .lines()
                    .find(|l| l.starts_with(key))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|v| v.trim().to_string())
            };
            Some((field("physical id")?, field("core id")?))
        })
        .collect();
    if !ids.is_empty() {
        let mut uniq = ids;
        uniq.sort();
        uniq.dedup();
        return Some(uniq.len() as u32);
    }

    count_cpu_threads(cpuinfo)
}

/// AVX2 aparece en `flags` (Intel) o `flags` (AMD en kernels recientes).
/// `None` cuando no hay bloque `flags` que leer (otras arquitecturas).
pub fn cpu_supports_avx2(cpuinfo: &str) -> Option<bool> {
    let mut hay_flags = false;
    for flags in cpuinfo.lines().filter(|l| l.starts_with("flags")) {
        hay_flags = true;
        let tiene = flags
            .split(':')
            .nth(1)
            .map(|list| list.split_whitespace().any(|f| f == "avx2"))
            .unwrap_or(false);
        if tiene {
            return Some(true);
        }
    }
    hay_flags.then_some(false)
}

/// VRAM dedicada (bytes) según `vulkaninfo --summary`.
///
/// El bloque que interesa es el heap marcado `DEVICE_LOCAL`, que es la memoria de
/// la tarjeta; los demás heaps son host-visible (RAM mapeada) y contarlos como
/// VRAM daría cifras absurdas en una iGPU.
///
/// ```text
/// memoryHeaps: count = 3
///         memoryHeaps[0]:
///                 size   = 8573157376 (0x1ff000000) (7.98 GiB)
///                 budget = 0x1fcf44000 (8179.59 MiB)
///                 usage  = 0x00000000 (0.00 B)
///                 flags: count = 1
///                         MEMORY_HEAP_DEVICE_LOCAL_BIT
/// ```
///
/// Devuelve `None` si no hay ningún heap `DEVICE_LOCAL con tamaño legible`
/// (típico en iGPU, donde la VRAM es RAM compartida).
pub fn parse_vram_bytes(vulkaninfo_summary: &str) -> Option<u64> {
    let mut size: Option<u64> = None;
    let mut device_local = false;

    for raw in vulkaninfo_summary.lines() {
        let line = raw.trim();
        if line.starts_with("memoryHeaps[") {
            if let Some(bytes) = heap_bytes(size, device_local) {
                return Some(bytes);
            }
            size = None;
            device_local = false;
            continue;
        }
        if line.starts_with("size") && line.contains('=') && size.is_none() {
            size = line
                .split('=')
                .nth(1)
                .and_then(|v| v.split_whitespace().next())
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0);
            continue;
        }
        if line.contains("MEMORY_HEAP_DEVICE_LOCAL_BIT") {
            device_local = true;
        }
    }

    heap_bytes(size, device_local)
}

fn heap_bytes(size: Option<u64>, device_local: bool) -> Option<u64> {
    if device_local {
        size
    } else {
        None
    }
}

impl Hardware {
    /// Tags que consumen las reglas (`requires`/`recommended_for`) y el filtro de
    /// recomendaciones de la comunidad. Un tag ausente significa "no se cumple",
    /// así que solo se emiten los que se han podido comprobar de verdad.
    pub fn tags(&self) -> Vec<String> {
        let mut t: Vec<String> = Vec::new();
        match self.vendor {
            Vendor::Amd => t.push("amd".to_string()),
            Vendor::Nvidia => t.push("nvidia".to_string()),
            Vendor::Intel => t.push("intel".to_string()),
            Vendor::Unknown => {}
        }
        if self.ntsync {
            t.push("ntsync".into());
        }
        if self.session == "wayland" {
            t.push("wayland".into());
        }
        if self.game_performance {
            t.push("game-performance".into());
        }
        // Buckets de VRAM: los requisitos de los juegos hablan de GB de VRAM y en
        // Proton la VRAM es el limitante más habitual en GPUs de gama baja.
        match self.vram_mb {
            Some(mb) if mb < 4096 => t.push("vram-bajo".into()),
            Some(mb) if mb < 8192 => t.push("vram-medio".into()),
            Some(_) => t.push("vram-alto".into()),
            // iGPU sin VRAM dedicada: no es "poca VRAM", es compartida con la RAM.
            None => {}
        }
        if self.cpu_avx2 == Some(false) {
            t.push("cpu-sin-avx2".into());
        }
        // `rt` = la GPU tiene ray tracing **utilizable** (RTX 30/40/50, RX 7000+).
        // No basta con tener las unidades: en RTX 20 y RX 6000 activarlo cuesta FPS,
        // así que ahí no se emite el tag y las reglas que lo piden quedan como «no
        // aplica» en vez de recomendarse. La decisión la toma `gpu_rank`, que es
        // donde vive la tabla de modelos.
        if crate::gpu_rank::rt_for_gpu(&self.gpu).is_some_and(|rt| rt.es_suficiente()) {
            t.push("rt".into());
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPUINFO_6C12T: &str = "processor\t: 0\nphysical id\t: 0\ncore id\t\t: 0\ncpu cores\t: 6\nflags\t\t: fpu vme avx avx2 sse4_2\n\nprocessor\t: 1\nphysical id\t: 0\ncore id\t\t: 1\ncpu cores\t: 6\nflags\t\t: fpu avx avx2\n";

    const VULKANINFO_AMD: &str = "deviceName = AMD Radeon RX 6800 XT\ndriverInfo = Mesa 24.1.0\nmemoryHeaps: count = 3\n\tmemoryHeaps[0]:\n\t\tsize   = 8573157376 (0x1ff000000) (7.98 GiB)\n\t\tbudget = 0x1fcf44000 (8179.59 MiB)\n\t\tusage  = 0x00000000 (0.00 B)\n\t\tflags: count = 1\n\t\t\tMEMORY_HEAP_DEVICE_LOCAL_BIT\n\tmemoryHeaps[1]:\n\t\tsize   = 17145646400 (0x3fe000000) (15.97 GiB)\n\t\tflags: count = 0\n";

    #[test]
    fn cuenta_hilos_y_nucleos() {
        assert_eq!(count_cpu_threads(CPUINFO_6C12T), Some(2));
        assert_eq!(count_cpu_cores(CPUINFO_6C12T), Some(6));
    }

    #[test]
    fn nucleos_sin_campo_cpu_cores_usa_physical_y_core_id() {
        let cpuinfo = "processor\t: 0\nphysical id\t: 0\ncore id\t\t: 0\n\nprocessor\t: 1\nphysical id\t: 0\ncore id\t\t: 0\n\nprocessor\t: 2\nphysical id\t: 0\ncore id\t\t: 1\n";
        // Tres hilos, dos nucleos fisicos reales.
        assert_eq!(count_cpu_threads(cpuinfo), Some(3));
        assert_eq!(count_cpu_cores(cpuinfo), Some(2));
    }

    #[test]
    fn nucleos_sin_datos_cae_a_los_hilos() {
        let cpuinfo = "processor\t: 0\n\nprocessor\t: 1\n";
        assert_eq!(count_cpu_cores(cpuinfo), Some(2));
    }

    #[test]
    fn cpuinfo_vacio_no_inventa_numeros() {
        assert_eq!(count_cpu_threads(""), None);
        assert_eq!(count_cpu_cores(""), None);
        assert_eq!(cpu_supports_avx2(""), None);
    }

    #[test]
    fn detecta_avx2() {
        assert_eq!(cpu_supports_avx2(CPUINFO_6C12T), Some(true));
        assert_eq!(
            cpu_supports_avx2("processor\t: 0\nflags\t\t: fpu avx sse4_2\n"),
            Some(false)
        );
        // Una CPU sin `flags` (ni Intel ni AMD de escritorio) no es "sin AVX2".
        assert_eq!(cpu_supports_avx2("processor\t: 0\n"), None);
    }

    #[test]
    fn vram_sale_del_heap_device_local() {
        assert_eq!(parse_vram_bytes(VULKANINFO_AMD), Some(8573157376));
    }

    #[test]
    fn vram_ignora_heaps_host_visible() {
        let solo_host = "memoryHeaps: count = 1\n\tmemoryHeaps[0]:\n\t\tsize   = 17145646400 (0x3fe000000) (15.97 GiB)\n\t\tflags: count = 0\n";
        assert_eq!(parse_vram_bytes(solo_host), None);
    }

    #[test]
    fn vram_auc_ausente_no_rompe() {
        assert_eq!(parse_vram_bytes(""), None);
        assert_eq!(parse_vram_bytes("deviceName = algo\n"), None);
    }

    #[test]
    fn tags_de_vram_por_tramo() {
        let mut hw = Hardware::default();
        hw.vram_mb = Some(4095);
        assert!(hw.tags().contains(&"vram-bajo".to_string()));
        hw.vram_mb = Some(6144);
        assert!(hw.tags().contains(&"vram-medio".to_string()));
        hw.vram_mb = Some(16384);
        assert!(hw.tags().contains(&"vram-alto".to_string()));
        // iGPU: sin VRAM dedicada no se etiqueta como poca VRAM.
        hw.vram_mb = None;
        let tags = hw.tags();
        assert!(!tags.iter().any(|t| t.starts_with("vram-")));
    }

    #[test]
    fn tag_de_cpu_sin_avx2_solo_si_se_sabe() {
        let mut hw = Hardware::default();
        hw.cpu_avx2 = Some(false);
        assert!(hw.tags().contains(&"cpu-sin-avx2".to_string()));
        hw.cpu_avx2 = Some(true);
        assert!(!hw.tags().contains(&"cpu-sin-avx2".to_string()));
        hw.cpu_avx2 = None;
        assert!(!hw.tags().contains(&"cpu-sin-avx2".to_string()));
    }

    #[test]
    fn tag_rt_solo_con_ray_tracing_utilizable() {
        // El tag `rt` lo usan las reglas que activan ray tracing (VKD3D_CONFIG=dxr).
        // Si se emitiera por tener "RTX" en el nombre, una RTX 2060 recibiria el
        // consejo de activar RT, que es el bug que arregla `gpu_rank::rt_tier`.
        let mut hw = Hardware::default();

        hw.gpu = "NVIDIA GeForce RTX 4070".into();
        assert!(hw.tags().contains(&"rt".to_string()), "Ada si");

        hw.gpu = "NVIDIA GeForce RTX 2060".into();
        assert!(!hw.tags().contains(&"rt".to_string()), "Turing: RT justo, no utilizable");

        hw.gpu = "AMD Radeon RX 6800 XT (RADV NAVI21)".into();
        assert!(!hw.tags().contains(&"rt".to_string()), "RDNA 2: RT justo");

        hw.gpu = "AMD Radeon RX 7800 XT".into();
        assert!(hw.tags().contains(&"rt".to_string()), "RDNA 3 si");

        hw.gpu = "NVIDIA GeForce GTX 1660 Ti".into();
        assert!(!hw.tags().contains(&"rt".to_string()), "sin unidades de RT");

        // GPU desconocida o sin nombre: nunca se inventa el tag.
        hw.gpu = "GPU rara".into();
        assert!(!hw.tags().contains(&"rt".to_string()));
        hw.gpu = String::new();
        assert!(!hw.tags().contains(&"rt".to_string()));
    }
}
