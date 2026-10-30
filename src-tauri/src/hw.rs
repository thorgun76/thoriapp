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
}

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

fn in_path(bin: &str) -> bool {
    env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .any(|d| Path::new(d).join(bin).exists())
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
    let raw_gpu = pick.splitn(2, ": ").nth(1).unwrap_or("No detectada");
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
    hw
}

impl Hardware {
    pub fn tags(&self) -> Vec<String> {
        let mut t = vec![];
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
        t
    }
}
