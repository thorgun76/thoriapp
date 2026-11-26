//! Fixtures de regresión hechas con **datos reales** de un equipo con Steam.
//!
//! Cada caso de aquí se copió de una ejecución de verdad. No son inventos: son la forma exacta
//! que tienen los procesos de Steam, y sirven para que la detección no vuelva a romperse del
//! modo en que se rompió (un juego bajo Proton no era detectable porque su proceso Linux no
//! se llama como el juego).
//!
//! **Anonimización**: el nombre de usuario real se sustituyó por `jugador` en todas las rutas
//! (`/home/thor` → `/home/jugador`, también bajo `/run/media`). Se conserva la estructura
//! completa de cada ruta —prefijos Wine, directorios de trabajo, `steamapps/common`, catálogos
//! de lanzadores— y las relaciones entre ejecutables, prefijos y procesos: solo cambia el
//! nombre de usuario. Los PID y los AppID son los originales, porque forman parte de lo que la
//! detección debe reproducir.
//!
//! Origen de los datos: CachyOS, Steam, `Palworld` con `proton-cachyos-slr` (AppID 1623730),
//! aportados por el usuario tras la prueba de aceptación fallida.
//!
//! Aviso que vale para todo este fichero: **pasar estas pruebas no es haber probado un juego
//! real**. Son datos reales procesados por código simulado; la prueba contra Steam de verdad
//! la hace el usuario en su equipo.

/// Un proceso de `/proc` tal como se lee, antes de interpretarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcesoDeFixture {
    pub pid: u32,
    pub ppid: u32,
    pub comm: &'static str,
    /// Ruta resuelta de `/proc/<pid>/exe`. `None` si no se pudo leer (permisos).
    pub exe: Option<&'static str>,
    /// Argumentos separados por NUL, sin el NUL final.
    pub cmdline: &'static str,
    /// Solo las variables de la lista blanca.
    pub entorno: &'static [(&'static str, &'static str)],
    /// Instante de arranque ya formateado.
    pub inicio: &'static str,
    /// Carpeta de instalación que el proceso declara (`STEAM_COMPAT_INSTALL_PATH`, ya
    /// reducida a `steamapps/common/<carpeta>`).
    ///
    /// Se escribe a mano en cada fixture y no se toma del `entorno`: así queda explícito si
    /// el proceso la declara o no, que es justo lo que distingue al juego del script `proton`.
    pub carpeta_declarada: Option<&'static str>,
    /// Directorio de trabajo (`/proc/<pid>/cwd`).
    ///
    /// Un juego nativo lanzado por un script intérprete (`starsector.sh` → `java`) no lleva el
    /// nombre del juego en ningún campo: solo su `cwd` dice a qué instalación pertenece.
    pub cwd: Option<&'static str>,
}

/// Palworld bajo `proton-cachyos-slr`, tal cual se observó.
///
/// Lo importante de este caso es lo que **no** cuadra con la intuición:
///
/// - el proceso principal se llama `GameThread`, no `Palworld`;
/// - su `exe` es el preloader de Wine, dentro de `compatibilitytools.d`;
/// - el juego solo aparece en los argumentos, con ruta de Windows (`S:\...`);
/// - `PROTON_VERSION` **no está** en el entorno;
/// - el padre (19328) no aparece en la lista, así que no se puede subir por el árbol;
/// - el `AppId=` de `SteamLaunch` lo lleva otro proceso del mismo padre (19180, `reaper`).
pub const PALWORLD_PROTON: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 19130,
        ppid: 1,
        comm: "steam",
        exe: Some("/usr/bin/steam"),
        cmdline: "/usr/bin/steam\0-steamdeck\0",
        entorno: &[],
        inicio: "2026-11-24 17:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 19180,
        ppid: 19130,
        comm: "reaper",
        exe: Some("/usr/lib/steam/steam/ubuntu12_64/steamwebhelper"),
        cmdline: "reaper\0SteamLaunch\0AppId=1623730\0--\0/usr/bin/python3\0/usr/lib/steam/steam/steam-launch-wrapper\0",
        entorno: &[],
        inicio: "2026-11-24 18:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 19328,
        ppid: 19180,
        comm: "python3",
        exe: Some("/usr/bin/python3.13"),
        cmdline: "/usr/bin/python3\0/usr/lib/steam/steam/steam-launch-wrapper\0--\0game-performance\0mangohud\0%command%\0",
        entorno: &[],
        inicio: "2026-11-24 18:00:01",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 19361,
        ppid: 19328,
        comm: "python3",
        exe: Some("/usr/bin/python3.13"),
        cmdline: "/usr/bin/python3\0/usr/lib/steam/steam/steam-runtime-launcher\0",
        entorno: &[],
        inicio: "2026-11-24 18:00:02",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 19570,
        ppid: 19328,
        comm: "Palworld.exe",
        exe: Some("/usr/bin/python3.13"),
        cmdline: "/usr/bin/python3\0/usr/lib/steam/steam/steam-runtime-launcher\0--verb=waitforexitandrun\0--\0/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton\0waitforexitandrun\0/run/games/steamapps/common/Palworld/Palworld.exe\0",
        entorno: &[],
        inicio: "2026-11-24 18:00:03",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 19588,
        ppid: 19328,
        comm: "GameThread",
        exe: Some(
            "/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/i386-unix/wine64-preloader",
        ),
        cmdline: "S:\\steamapps\\common\\Palworld\\Pal\\Binaries\\Win64\\Palworld-Win64-Shipping.exe\0Pal\0-dx12\0",
        entorno: &[
            ("SteamAppId", "1623730"),
            ("SteamGameId", "1623730"),
            ("SteamClientLaunch", "1"),
        ],
        inicio: "2026-11-24 18:00:05",
        carpeta_declarada: None,
        cwd: None,
    },
];

/// Lanzamiento real de Palworld envuelto en `systemd-inhibit`, tal como lo entrega Steam
/// con el runtime de CachyOS: el inhibidor del sueño es el **lanzador**, no el juego, pero
/// hereda `SteamAppId`/`SteamGameId` igual que todo el árbol.
pub const PALWORLD_CON_INHIBIDOR: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 22010,
        ppid: 1,
        comm: "steam",
        exe: Some("/usr/bin/steam"),
        cmdline: "/usr/bin/steam\0",
        entorno: &[],
        inicio: "2026-11-24 20:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 22100,
        ppid: 22010,
        comm: "reaper",
        exe: Some("/usr/lib/steam/steam/ubuntu12_64/steamwebhelper"),
        cmdline: "reaper\0SteamLaunch\0AppId=1623730\0--\0/usr/bin/python3\0",
        entorno: &[],
        inicio: "2026-11-24 20:00:01",
        carpeta_declarada: None,
        cwd: None,
    },
    // El wrapper del sistema: hereda el entorno de Steam y por eso «parece» del juego.
    ProcesoDeFixture {
        pid: 22150,
        ppid: 22100,
        comm: "systemd-inhibit",
        exe: Some("/usr/bin/systemd-inhibit"),
        cmdline: "/usr/bin/systemd-inhibit\0--who=Steam\0--what=sleep:idle\0--mode=block\0--\0mangohud\0%command%\0",
        entorno: &[
            ("SteamAppId", "1623730"),
            ("SteamGameId", "1623730"),
            ("SteamClientLaunch", "1"),
        ],
        inicio: "2026-11-24 20:00:02",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 22200,
        ppid: 22150,
        comm: "GameThread",
        exe: Some(
            "/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/i386-unix/wine64-preloader",
        ),
        cmdline: "S:\\steamapps\\common\\Palworld\\Pal\\Binaries\\Win64\\Palworld-Win64-Shipping.exe\0Pal\0-dx12\0",
        entorno: &[
            ("SteamAppId", "1623730"),
            ("SteamGameId", "1623730"),
            ("SteamClientLaunch", "1"),
        ],
        inicio: "2026-11-24 20:00:05",
        carpeta_declarada: None,
        cwd: None,
    },
];

/// Juego nativo: el proceso Linux se llama como el juego y vive en `steamapps/common`.
pub const JUEGO_NATIVO: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 30010,
        ppid: 1,
        comm: "steam",
        exe: Some("/usr/bin/steam"),
        cmdline: "/usr/bin/steam\0",
        entorno: &[],
        inicio: "2026-11-24 17:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 30200,
        ppid: 30010,
        comm: "7DaysToDie",
        exe: Some("/run/media/jugador/SSD/SteamLibrary/steamapps/common/7 Days to Die/7DaysToDie.x86_64"),
        cmdline: "/run/media/jugador/SSD/SteamLibrary/steamapps/common/7 Days to Die/7DaysToDie.x86_64\0-batchmode\0-nogs\0",
        entorno: &[
            ("SteamAppId", "251570"),
            ("SteamGameId", "251570"),
            ("SteamClientLaunch", "1"),
        ],
        inicio: "2026-11-24 18:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 30215,
        ppid: 1,
        comm: "wineserver",
        exe: Some("/usr/bin/wineserver"),
        cmdline: "wineserver\0-p\0",
        entorno: &[],
        inicio: "2026-11-24 18:00:01",
        carpeta_declarada: None,
        cwd: None,
    },
];

/// Dos juegos a la vez, cada uno con su árbol. El padre del primero es un lanzador común.
pub const DOS_JUEGOS: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 41000,
        ppid: 1,
        comm: "steam",
        exe: Some("/usr/bin/steam"),
        cmdline: "/usr/bin/steam\0",
        entorno: &[],
        inicio: "2026-11-24 17:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 41200,
        ppid: 41000,
        comm: "reaper",
        exe: Some("/usr/lib/steam/steam/ubuntu12_64/steamwebhelper"),
        cmdline: "reaper\0SteamLaunch\0AppId=1623730\0",
        entorno: &[],
        inicio: "2026-11-24 18:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 41201,
        ppid: 41000,
        comm: "reaper",
        exe: Some("/usr/lib/steam/steam/ubuntu12_64/steamwebhelper"),
        cmdline: "reaper\0SteamLaunch\0AppId=251570\0",
        entorno: &[],
        inicio: "2026-11-24 18:00:10",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 41300,
        ppid: 41200,
        comm: "GameThread",
        exe: Some(
            "/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/i386-unix/wine64-preloader",
        ),
        cmdline: "S:\\steamapps\\common\\Palworld\\Pal\\Binaries\\Win64\\Palworld-Win64-Shipping.exe\0Pal\0-dx12\0",
        entorno: &[("SteamAppId", "1623730"), ("SteamGameId", "1623730")],
        inicio: "2026-11-24 18:00:05",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 41400,
        ppid: 41201,
        comm: "7DaysToDie",
        exe: Some("/run/media/jugador/SSD/SteamLibrary/steamapps/common/7 Days to Die/7DaysToDie.x86_64"),
        cmdline: "/run/media/jugador/SSD/SteamLibrary/steamapps/common/7 Days to Die/7DaysToDie.x86_64\0",
        entorno: &[("SteamAppId", "251570"), ("SteamGameId", "251570")],
        inicio: "2026-11-24 18:00:12",
        carpeta_declarada: None,
        cwd: None,
    },
];

/// AppID contradictorio: el entorno dice uno y la línea de comandos de Steam, otro.
pub const APPID_CONTRADICTORIO: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 51000,
        ppid: 50999,
        comm: "reaper",
        exe: Some("/usr/lib/steam/steam/ubuntu12_64/steamwebhelper"),
        cmdline: "reaper\0SteamLaunch\0AppId=251570\0",
        entorno: &[],
        inicio: "2026-11-24 18:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 51100,
        ppid: 50999,
        comm: "GameThread",
        exe: Some(
            "/usr/share/steam/compatibilitytools.d/Proton-9.0/files/lib/wine/wine64-preloader",
        ),
        cmdline: "Z:\\juegos\\steamapps\\common\\Palworld\\Palworld-Win64-Shipping.exe\0",
        entorno: &[("SteamAppId", "1623730"), ("SteamGameId", "251570")],
        inicio: "2026-11-24 18:00:05",
        carpeta_declarada: None,
        cwd: None,
    },
];

/// Procesos de un contenedor: solo los propios, sin Steam ni juego. Es lo que se ve desde
/// Docker, y debe degradar a «sin visibilidad», no a «no corre».
pub const CONTENEDOR: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 1,
        ppid: 0,
        comm: "bash",
        exe: Some("/usr/bin/bash"),
        cmdline: "/bin/bash\0",
        entorno: &[],
        inicio: "2026-11-24 17:00:00",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 12,
        ppid: 1,
        comm: "thoriapp",
        exe: Some("/workspace/tauri-app/src-tauri/target/debug/thoriapp"),
        cmdline: "/workspace/tauri-app/src-tauri/target/debug/thoriapp\0",
        entorno: &[],
        inicio: "2026-11-24 17:05:00",
        carpeta_declarada: None,
        cwd: None,
    },
];

/// Deskrawl (Unity) bajo el Steam Linux Runtime `srt`, tal como se observó en la prueba 2.
///
/// Es el caso que faltaba: `srt-bwrap` envuelve el juego, y dentro del contenedor de Steam
/// conviven helpers del sistema invitado (`tabtip`, `xalia`) y el manejador de cuelgues de
/// Unity (`UnityCrashHandler64`). Ninguno es el juego; el juego aparece en la línea de
/// comandos del `reaper`, que es de donde se lee su `.exe`.
/// Deskrawl real desde Steam, **tal cual lo reportó el usuario** (PID anonimizados solo en
/// la cuenta, no en la estructura).
///
/// A diferencia de `DESKRAWL_SRT`, aquí los procesos **declaran** `STEAM_COMPAT_INSTALL_PATH`,
/// que es lo que hace Proton en el equipo real: lo pone sobre todo el contenedor, no solo
/// sobre el juego. Esa diferencia es justo la que destapó el defecto: con la carpeta
/// declarada, `tabtip.exe` pasaba por juego.
///
/// Tres procesos, una sola ejecución:
/// - `pv-adverb` (557367): el envoltorio del runtime; menciona `Deskrawl.exe` como argumento.
/// - `tabtip.exe` (557505): el teclado táctil de Windows, auxiliar del sistema invitado.
/// - `Deskrawl.exe` (557514): el juego.
pub const DESKRAWL_PV_ADVERB: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 557298,
        ppid: 1,
        comm: "reaper",
        exe: Some("/usr/lib/steam/ubuntu12_32/reaper"),
        cmdline: "reaper\0SteamLaunch\0AppId=4623570\0--\0/reaper\0",
        entorno: &[],
        inicio: "2026-11-25 22:37:55",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 557367,
        ppid: 557298,
        comm: "pv-adverb",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb"),
        cmdline: "/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb\0--\0/reaper\0SteamLaunch\0AppId=4623570\0--\0Z:\\home\\jugador\\.local\\share\\Steam\\steamapps\\common\\Deskrawl\\Deskrawl.exe\0",
        entorno: &[
            ("SteamAppId", "4623570"),
            ("SteamGameId", "4623570"),
            ("SteamClientLaunch", "1"),
            ("STEAM_COMPAT_INSTALL_PATH", "/home/jugador/.local/share/Steam/steamapps/common/Deskrawl"),
        ],
        inicio: "2026-11-25 22:37:59",
        carpeta_declarada: Some("steamapps/common/Deskrawl"),
        cwd: Some("steamapps/common/Deskrawl"),
    },
    ProcesoDeFixture {
        pid: 557505,
        ppid: 557367,
        comm: "tabtip.exe",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/wine64-preloader"),
        cmdline: "C:\\windows\\system32\\tabtip.exe\0",
        entorno: &[
            ("SteamAppId", "4623570"),
            ("SteamGameId", "4623570"),
            ("STEAM_COMPAT_INSTALL_PATH", "/home/jugador/.local/share/Steam/steamapps/common/Deskrawl"),
            ("WINEPREFIX", "/home/jugador/.local/share/Steam/steamapps/compatdata/4623570/pfx/"),
        ],
        inicio: "2026-11-25 22:38:09",
        carpeta_declarada: Some("steamapps/common/Deskrawl"),
        cwd: Some("steamapps/common/Deskrawl"),
    },
    ProcesoDeFixture {
        pid: 557514,
        ppid: 557367,
        comm: "Deskrawl.exe",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/wine64-preloader"),
        cmdline: "S:\\steamapps\\common\\Deskrawl\\Deskrawl.exe\0",
        entorno: &[
            ("SteamAppId", "4623570"),
            ("SteamGameId", "4623570"),
            ("STEAM_COMPAT_INSTALL_PATH", "/home/jugador/.local/share/Steam/steamapps/common/Deskrawl"),
            ("WINEPREFIX", "/home/jugador/.local/share/Steam/steamapps/compatdata/4623570/pfx/"),
        ],
        inicio: "2026-11-25 22:38:09",
        carpeta_declarada: Some("steamapps/common/Deskrawl"),
        cwd: Some("steamapps/common/Deskrawl"),
    },
];

pub const DESKRAWL_SRT: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 40010,
        ppid: 1,
        comm: "steam",
        exe: Some("/usr/bin/steam"),
        cmdline: "/usr/bin/steam\0",
        entorno: &[],
        inicio: "2026-11-25 14:15:58",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 40100,
        ppid: 40010,
        comm: "reaper",
        exe: Some("/usr/lib/steam/steam/ubuntu12_64/steamwebhelper"),
        cmdline: "reaper\0SteamLaunch\0AppId=4623570\0--\0/usr/share/steam/ubuntu12_32/reaper\0",
        entorno: &[],
        inicio: "2026-11-25 14:15:59",
        carpeta_declarada: None,
        cwd: None,
    },
    // El envoltorio del runtime: hereda el entorno de Steam entero y es el padre de todo.
    ProcesoDeFixture {
        pid: 40150,
        ppid: 40100,
        comm: "srt-bwrap",
        exe: Some("/usr/lib/steam/steam/ubuntu12_64/srt-bwrap"),
        cmdline: "/usr/lib/steam/steam/ubuntu12_64/srt-bwrap\0--verbosity=1\0--\0/reaper\0SteamLaunch\0AppId=4623570\0--\0Deskrawl.exe\0",
        entorno: &[
            ("SteamAppId", "4623570"),
            ("SteamGameId", "4623570"),
            ("SteamClientLaunch", "1"),
        ],
        inicio: "2026-11-25 14:16:00",
        carpeta_declarada: None,
        cwd: None,
    },
    // Preloader de Wine: el proceso que de verdad ejecuta el juego.
    ProcesoDeFixture {
        pid: 40200,
        ppid: 40150,
        comm: "GameThread",
        exe: Some(
            "/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/x86_64-unix/wine64-preloader",
        ),
        cmdline: "Z:\\home\\jugador\\.local\\share\\Steam\\steamapps\\common\\Deskrawl\\Deskrawl.exe\0",
        entorno: &[
            ("SteamAppId", "4623570"),
            ("SteamGameId", "4623570"),
            ("SteamClientLaunch", "1"),
        ],
        inicio: "2026-11-25 14:16:04",
        carpeta_declarada: None,
        cwd: None,
    },
    // Helpers del sistema invitado dentro del contenedor: no son el juego.
    ProcesoDeFixture {
        pid: 40250,
        ppid: 40200,
        comm: "tabtip.exe",
        exe: Some("/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/x86_64-unix/wine64-preloader"),
        cmdline: "C:\\windows\\system32\\tabtip.exe\0",
        entorno: &[
            ("SteamAppId", "4623570"),
            ("SteamGameId", "4623570"),
        ],
        inicio: "2026-11-25 14:16:10",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 40260,
        ppid: 40200,
        comm: "xalia.exe",
        exe: Some("/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/x86_64-unix/wine64-preloader"),
        cmdline: "C:\\windows\\system32\\xalia.exe\0",
        entorno: &[
            ("SteamAppId", "4623570"),
            ("SteamGameId", "4623570"),
        ],
        inicio: "2026-11-25 14:16:11",
        carpeta_declarada: None,
        cwd: None,
    },
    ProcesoDeFixture {
        pid: 40270,
        ppid: 40200,
        comm: "UnityCrashHandler64.exe",
        exe: Some("/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/x86_64-unix/wine64-preloader"),
        cmdline: "Z:\\home\\jugador\\.local\\share\\Steam\\steamapps\\common\\Deskrawl\\UnityCrashHandler64.exe\0",
        entorno: &[
            ("SteamAppId", "4623570"),
            ("SteamGameId", "4623570"),
        ],
        inicio: "2026-11-25 14:16:12",
        carpeta_declarada: None,
        cwd: None,
    },
];

/// Starsector en Lutris con **Linux nativo**, tal cual se observó en CachyOS.
///
/// Es el caso que demostró que el fallo no era de Wine/Proton: un juego puede ser un script
/// (`starsector.sh`) que arranca un intérprete (`java`), sin `.exe`, sin `SteamAppId` y fuera
/// de `steamapps/common`. Toda la evidencia del juego está en tres cosas: el ejecutable
/// configurado en el `.yml`, el `cwd` compartido y el árbol de procesos.
///
/// Árbol real: 641413 (`lutris-wrapper`) → 641415 (`starsector.sh`) → 641416 (`java`).
/// Los tres tienen `cwd` = `/home/jugador/Games/starsector`.
pub const STARSECTOR_NATIVO: &[ProcesoDeFixture] = &[
    // 1. El envoltorio de Lutris. Es un `python3` de `/usr/bin`: auxiliar, nunca el juego.
    ProcesoDeFixture {
        pid: 641413,
        ppid: 1,
        comm: "lutris-wrapper",
        exe: Some("/usr/bin/python3.14"),
        cmdline: "lutris-wrapper\0Star Sector\00\0starsector.sh\0",
        entorno: &[],
        inicio: "2026-11-26 21:10:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/starsector"),
    },
    // 2. El script de arranque configurado en el `.yml` (`game.exe`).
    ProcesoDeFixture {
        pid: 641415,
        ppid: 641413,
        comm: "starsector.sh",
        exe: Some("/usr/bin/bash"),
        cmdline: "/bin/sh\0./starsector.sh\0",
        entorno: &[],
        inicio: "2026-11-26 21:10:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/starsector"),
    },
    // 3. El intérprete que **ejecuta** el juego, con su JDK dentro de la instalación.
    ProcesoDeFixture {
        pid: 641416,
        ppid: 641415,
        comm: "java",
        exe: Some("/home/jugador/Games/starsector/jre_linux/bin/java"),
        cmdline: "/home/jugador/Games/starsector/jre_linux/bin/java\0-Xms2048m\0-jar\0starsector-core/starsector.jar\0com.fs.starfarer.StarfarerLauncher\0",
        entorno: &[],
        inicio: "2026-11-26 21:10:01",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/starsector"),
    },
];

/// Star Valor en Lutris con **Wine**, `directory` vacío y sin AppID de Steam.
///
/// El `.yml` real solo declara `game.exe` y la sección `wine` sin prefijo explícito: no hay
/// `working_dir` ni `prefix`. El prefijo se deriva de la carpeta de Lutris, y aquí se
/// representa lo que el adaptador produce de verdad.
pub const STAR_VALOR_WINE: &[ProcesoDeFixture] = &[
    // El envoltorio de Lutris, con el comando del juego en sus argumentos.
    ProcesoDeFixture {
        pid: 652001,
        ppid: 1,
        comm: "lutris-wrapper",
        exe: Some("/usr/bin/python3.14"),
        cmdline: "lutris-wrapper\0Star Valor\00\0wine\0/home/jugador/Games/Star Valor/Star Valor.exe\0",
        entorno: &[("WINEPREFIX", "/home/jugador/.local/share/lutris/runners/wine/star-valor")],
        inicio: "2026-11-26 22:05:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Star Valor"),
    },
    // El proceso Windows del juego bajo el preloader.
    ProcesoDeFixture {
        pid: 652010,
        ppid: 652001,
        comm: "Star Valor.exe",
        exe: Some("/usr/lib/x86_64-linux-gnu/wine/wine64-preloader"),
        cmdline: "C:\\windows\\system32\\wineboot.exe\0Z:\\home\\thor\\Games\\Star Valor\\Star Valor.exe\0",
        entorno: &[("WINEPREFIX", "/home/jugador/.local/share/lutris/runners/wine/star-valor/pfx")],
        inicio: "2026-11-26 22:05:02",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Star Valor"),
    },
];

/// Eslabong en Heronite con **UMU + Proton**, `SteamAppId=0` y prefijo propio.
///
/// Los datos vienen de la validación real anterior: UMU exporta `GAMEID=umu-0` y pone
/// `SteamAppId=SteamGameId=0`, que no identifica ningún juego de Steam.
pub const ESLABONG_UMU: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 663001,
        ppid: 1,
        comm: "umu-run",
        exe: Some("/usr/bin/python3"),
        cmdline: "/usr/bin/python3\0/usr/share/umu-run/umu_run.py\0/home/jugador/Games/Eslabong/eslabong.exe\0",
        entorno: &[
            ("SteamAppId", "0"),
            ("SteamGameId", "0"),
            ("WINEPREFIX", "/home/jugador/Games/Heronite/Prefixes/eslabong/pfx"),
        ],
        inicio: "2026-11-26 23:00:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Eslabong"),
    },
    ProcesoDeFixture {
        pid: 663010,
        ppid: 663001,
        comm: "eslabong.exe",
        exe: Some("/usr/lib/x86_64-linux-gnu/wine/wine64-preloader"),
        cmdline: "Z:\\home\\thor\\Games\\Eslabong\\eslabong.exe\0",
        entorno: &[
            ("SteamAppId", "0"),
            ("SteamGameId", "0"),
            ("PROTON_VERSION", "Proton 9.0-4"),
            ("WINEPREFIX", "/home/jugador/Games/Heronite/Prefixes/eslabong/pfx"),
        ],
        inicio: "2026-11-26 23:00:03",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Eslabong"),
    },
];

// ==================== ÁRBOLES REALES DE LA CAPTURA DE CACHYOS (26-11-2026) ====================
//
// Estos tres árboles son transcripción literal de la captura `diag_topologia.json` del equipo
// del usuario. Están aquí porque los fixtures anteriores **no reproducían** la duplicación: el
// motor real sí se ejecutaba contra ellos y daba un solo grupo, mientras el equipo daba tres
// sesiones. La diferencia era el eslabón descartado (`python3`, `reaper`), que cortaba la
// cadena y convertía al nieto en raíz.
//
// No son el árbol ideal: son el árbol observado, con sus procesos descartados incluidos.

/// Eslabong por Heronite/UMU. Un `python3` intermedio parte el árbol en dos ramas.
///
/// ```text
/// 251473 python3.14
/// └── 251519 srt-bwrap
///     └── 251605 pv-adverb
///         ├── 251724 wine64-preloader   ← rama directa
///         └── 251638 python3            ← DESCARTADO como auxiliar
///             └── 251647 wine64-preloader
/// ```
pub const ESLABONG_TOPOLOGIA_REAL: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 251473,
        ppid: 1,
        comm: "python3.14",
        exe: Some("/usr/bin/python3.14"),
        cmdline: "/usr/bin/python3.14\0/usr/share/umu-run/umu_run.py\0",
        entorno: &[],
        inicio: "2026-11-26 11:27:38",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    ProcesoDeFixture {
        pid: 251519,
        ppid: 251473,
        comm: "srt-bwrap",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/srt-bwrap"),
        cmdline: "srt-bwrap\0",
        entorno: &[],
        inicio: "2026-11-26 11:27:38",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Eslabong"),
    },
    ProcesoDeFixture {
        pid: 251605,
        ppid: 251519,
        comm: "pv-adverb",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb"),
        cmdline: "pv-adverb\0",
        entorno: &[
            ("SteamAppId", "0"),
            ("SteamGameId", "0"),
            ("WINEPREFIX", "/home/jugador/Games/Heronite/Prefixes/eslabong/pfx"),
        ],
        inicio: "2026-11-26 11:27:39",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Eslabong"),
    },
    ProcesoDeFixture {
        pid: 251724,
        ppid: 251605,
        comm: "wine64-preloader",
        exe: Some("/usr/lib/x86_64-linux-gnu/wine/wine64-preloader"),
        cmdline: "Z:\\home\\thor\\Games\\Eslabong\\eslabong.exe\0",
        entorno: &[
            ("SteamAppId", "0"),
            ("SteamGameId", "0"),
            ("WINEPREFIX", "/home/jugador/Games/Heronite/Prefixes/eslabong/pfx"),
            ("PROTON_VERSION", "proton-cachyos-10.0-20260102-slr-x86_64"),
        ],
        inicio: "2026-11-26 11:27:42",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Eslabong"),
    },
    ProcesoDeFixture {
        pid: 251638,
        ppid: 251605,
        comm: "python3",
        exe: Some("/usr/bin/python3"),
        cmdline: "/usr/bin/python3\0/usr/share/umu-run/umu_run.py\0",
        entorno: &[
            ("SteamAppId", "0"),
            ("SteamGameId", "0"),
            ("WINEPREFIX", "/home/jugador/Games/Heronite/Prefixes/eslabong/pfx"),
        ],
        inicio: "2026-11-26 11:27:43",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Eslabong"),
    },
    ProcesoDeFixture {
        pid: 251647,
        ppid: 251638,
        comm: "wine64-preloader",
        exe: Some("/usr/lib/x86_64-linux-gnu/wine/wine64-preloader"),
        cmdline: "Z:\\home\\thor\\Games\\Eslabong\\eslabong.exe\0",
        entorno: &[
            ("SteamAppId", "0"),
            ("SteamGameId", "0"),
            ("WINEPREFIX", "/home/jugador/Games/Heronite/Prefixes/eslabong/pfx"),
            ("PROTON_VERSION", "proton-cachyos-10.0-20260102-slr-x86_64"),
        ],
        inicio: "2026-11-26 11:27:43",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Eslabong"),
    },
];

/// Tattoo Tycoon por Heroic. Mismo patrón: un `python3` intermedio.
///
/// ```text
/// 252757 srt-bwrap
/// └── 252824 pv-adverb
///     ├── 253005 wine64-preloader   ← cwd drive_c/windows
///     └── 252857 python3            ← DESCARTADO como auxiliar
///         └── 252951 wine-preloader
/// ```
///
/// Antes de estos grupos se creó una sesión con 252630 (`pv-adverb`, sin nombre), que es el
/// falso positivo de la infraestructura de arranque correlacionada por prefijo.
pub const TATTOO_TOPOLOGIA_REAL: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 252630,
        ppid: 1,
        comm: "pv-adverb",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb"),
        cmdline: "pv-adverb\0",
        entorno: &[
            ("WINEPREFIX", "/home/jugador/Games/Heroic/Prefixes/default/Tattoo Tycoon/pfx"),
        ],
        inicio: "2026-11-26 11:29:08",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    ProcesoDeFixture {
        pid: 252757,
        ppid: 1,
        comm: "srt-bwrap",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/srt-bwrap"),
        cmdline: "srt-bwrap\0",
        entorno: &[],
        inicio: "2026-11-26 11:29:10",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Heroic/TattooTycoon"),
    },
    ProcesoDeFixture {
        pid: 252824,
        ppid: 252757,
        comm: "pv-adverb",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb"),
        cmdline: "pv-adverb\0",
        entorno: &[
            ("WINEPREFIX", "/home/jugador/Games/Heroic/Prefixes/default/Tattoo Tycoon/pfx"),
        ],
        inicio: "2026-11-26 11:29:11",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Heroic/TattooTycoon"),
    },
    ProcesoDeFixture {
        pid: 252857,
        ppid: 252824,
        comm: "python3",
        exe: Some("/usr/bin/python3"),
        cmdline: "/usr/bin/python3\0/usr/share/umu-run/umu_run.py\0",
        entorno: &[
            ("WINEPREFIX", "/home/jugador/Games/Heroic/Prefixes/default/Tattoo Tycoon/pfx"),
        ],
        inicio: "2026-11-26 11:29:12",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Heroic/TattooTycoon"),
    },
    ProcesoDeFixture {
        pid: 252951,
        ppid: 252857,
        comm: "wine-preloader",
        exe: Some("/usr/lib/x86_64-linux-gnu/wine/wine-preloader"),
        cmdline: "Z:\\home\\thor\\Games\\Heroic\\TattooTycoon\\Tattoo Tycoon.exe\0",
        entorno: &[
            ("WINEPREFIX", "/home/jugador/Games/Heroic/Prefixes/default/Tattoo Tycoon/pfx"),
            ("PROTON_VERSION", "proton-cachyos-slr"),
        ],
        inicio: "2026-11-26 11:29:13",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Heroic/TattooTycoon"),
    },
    ProcesoDeFixture {
        pid: 253005,
        ppid: 252824,
        comm: "wine64-preloader",
        exe: Some("/usr/lib/x86_64-linux-gnu/wine/wine64-preloader"),
        cmdline: "Z:\\home\\thor\\Games\\Heroic\\TattooTycoon\\Tattoo Tycoon.exe\0",
        entorno: &[
            ("WINEPREFIX", "/home/jugador/Games/Heroic/Prefixes/default/Tattoo Tycoon/pfx"),
            ("PROTON_VERSION", "proton-cachyos-slr"),
        ],
        inicio: "2026-11-26 11:29:13",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/Games/Heroic/Prefixes/default/Tattoo Tycoon/drive_c/windows"),
    },
];

/// Palworld por Steam, más el falso positivo de `iscriptevaluator`/`steam`.
///
/// ```text
/// 253993 systemd-inhibit
/// └── 254011 python3            ← DESCARTADO
///     └── 254020 reaper         ← DESCARTADO
///         └── 254027 srt-bwrap
///             └── 254095 pv-adverb
///                 └── 254204 wine64-preloader
/// ```
///
/// El representante del grupo fragmentado era el **ancestro** del representante real, que es
/// la firma exacta de una cadena rota.
pub const PALWORLD_TOPOLOGIA_REAL: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 253821,
        ppid: 1,
        comm: "pv-adverb",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb"),
        cmdline: "pv-adverb\0",
        entorno: &[],
        inicio: "2026-11-26 11:30:40",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 253857,
        ppid: 1,
        comm: "wine-preloader",
        exe: Some("/usr/lib/x86_64-linux-gnu/wine/wine-preloader"),
        cmdline: "wine-preloader\0iscriptevaluator.exe\0",
        entorno: &[],
        inicio: "2026-11-26 11:30:41",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 253993,
        ppid: 1,
        comm: "systemd-inhibit",
        exe: Some("/usr/bin/systemd-inhibit"),
        cmdline: "/usr/bin/systemd-inhibit\0--what=sleep\0",
        entorno: &[
            ("SteamAppId", "1623730"),
            ("SteamGameId", "1623730"),
        ],
        inicio: "2026-11-26 11:30:47",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    ProcesoDeFixture {
        pid: 254011,
        ppid: 253993,
        comm: "python3",
        exe: Some("/usr/bin/python3"),
        cmdline: "/usr/bin/python3\0-c\0",
        entorno: &[],
        inicio: "2026-11-26 11:30:47",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 254020,
        ppid: 254011,
        comm: "reaper",
        exe: Some("/home/jugador/.local/share/Steam/steamapps/common/SteamLinuxRuntime_sniper/reaper"),
        cmdline: "reaper\0",
        entorno: &[],
        inicio: "2026-11-26 11:30:48",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 254027,
        ppid: 254020,
        comm: "srt-bwrap",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/srt-bwrap"),
        cmdline: "srt-bwrap\0",
        entorno: &[],
        inicio: "2026-11-26 11:30:48",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 254095,
        ppid: 254027,
        comm: "pv-adverb",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb"),
        cmdline: "pv-adverb\0",
        entorno: &[
            ("SteamAppId", "1623730"),
            ("SteamGameId", "1623730"),
        ],
        inicio: "2026-11-26 11:30:49",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 254204,
        ppid: 254095,
        comm: "wine64-preloader",
        exe: Some("/run/host/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/i386-unix/wine64-preloader"),
        cmdline: "Z:\\home\\thor\\.local\\share\\Steam\\steamapps\\common\\Palworld\\Palworld.exe\0",
        entorno: &[
            ("SteamAppId", "1623730"),
            ("SteamGameId", "1623730"),
            ("PROTON_VERSION", "proton-cachyos-slr"),
        ],
        inicio: "2026-11-26 11:30:50",
        carpeta_declarada: Some(
            "/home/jugador/.local/share/Steam/steamapps/common/Palworld",
        ),
        cwd: Some(
            "/home/jugador/.local/share/Steam/steamapps/compatdata/1623730/pfx/drive_c/windows",
        ),
    },
];

// ============ Topologías reales de la validación de 5.3 (diag_ciclo_vida.json) ============
//
// Los fixtures anteriores tienen los envoltorios colgando de `ppid: 1`, que **no** es lo que
// trae el equipo. Estos reproducen la cadena exacta que se midió en CachyOS, con los PIDs y
// `ppid` del diagnóstico. La diferencia importa: la raíz de lanzamiento se calcula subiendo
// por los padres, así que un árbol «plano» no puede reproducir qué raíz sale.

/// Palworld, registro 1219 del diagnóstico real.
///
/// Dos grupos con el **mismo AppID** (1623730): el `systemd-inhibit` (426577) y el
/// `wine64-preloader` (426782), que desciende de él pasando por 426674/426606/426599/426590.
/// El `systemd-inhibit` cuelga de 426576, que sube por Steam hasta 3355.
pub const PALWORLD_REAL_5_3: &[ProcesoDeFixture] = &[
    ProcesoDeFixture {
        pid: 426576,
        ppid: 127410,
        comm: "systemd",
        exe: Some("/usr/lib/systemd/systemd"),
        cmdline: "/usr/lib/systemd/systemd\0--user\0",
        entorno: &[],
        inicio: "2026-11-26 09:12:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    ProcesoDeFixture {
        pid: 127410,
        ppid: 127399,
        comm: "systemd",
        exe: Some("/usr/lib/systemd/systemd"),
        cmdline: "/usr/lib/systemd/systemd\0--user\0",
        entorno: &[],
        inicio: "2026-11-26 09:12:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    ProcesoDeFixture {
        pid: 127399,
        ppid: 127367,
        comm: "systemd",
        exe: Some("/usr/lib/systemd/systemd"),
        cmdline: "/usr/lib/systemd/systemd\0--user\0",
        entorno: &[],
        inicio: "2026-11-26 09:12:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    ProcesoDeFixture {
        pid: 127367,
        ppid: 127312,
        comm: "systemd",
        exe: Some("/usr/lib/systemd/systemd"),
        cmdline: "/usr/lib/systemd/systemd\0--user\0",
        entorno: &[],
        inicio: "2026-11-26 09:12:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    ProcesoDeFixture {
        pid: 127312,
        ppid: 3355,
        comm: "systemd",
        exe: Some("/usr/lib/systemd/systemd"),
        cmdline: "/usr/lib/systemd/systemd\0--user\0",
        entorno: &[],
        inicio: "2026-11-26 09:12:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    ProcesoDeFixture {
        pid: 3355,
        ppid: 1,
        comm: "systemd",
        exe: Some("/usr/lib/systemd/systemd"),
        cmdline: "/usr/lib/systemd/systemd\0--user\0",
        entorno: &[],
        inicio: "2026-11-26 09:12:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    // El `systemd-inhibit` que arranca Steam para el juego. Lleva el AppID.
    ProcesoDeFixture {
        pid: 426577,
        ppid: 426576,
        comm: "systemd-inhibit",
        exe: Some("/usr/bin/systemd-inhibit"),
        cmdline: "/usr/bin/systemd-inhibit\0--what=sleep\0",
        entorno: &[("SteamAppId", "1623730"), ("SteamGameId", "1623730")],
        inicio: "2026-11-26 13:05:00",
        carpeta_declarada: None,
        cwd: Some("/home/jugador"),
    },
    // La cadena de lanzamiento que desciende de él.
    ProcesoDeFixture {
        pid: 426590,
        ppid: 426577,
        comm: "python3",
        exe: Some("/usr/bin/python3"),
        cmdline: "/usr/bin/python3\0-c\0",
        entorno: &[],
        inicio: "2026-11-26 13:05:01",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 426599,
        ppid: 426590,
        comm: "reaper",
        exe: Some(
            "/home/jugador/.local/share/Steam/steamapps/common/SteamLinuxRuntime_sniper/reaper",
        ),
        cmdline: "reaper\0",
        entorno: &[],
        inicio: "2026-11-26 13:05:01",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 426606,
        ppid: 426599,
        comm: "srt-bwrap",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/srt-bwrap"),
        cmdline: "srt-bwrap\0",
        entorno: &[],
        inicio: "2026-11-26 13:05:02",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    ProcesoDeFixture {
        pid: 426674,
        ppid: 426606,
        comm: "pv-adverb",
        exe: Some("/usr/lib/pressure-vessel/from-host/libexec/steam-runtime-tools-0/pv-adverb"),
        cmdline: "pv-adverb\0",
        entorno: &[("SteamAppId", "1623730"), ("SteamGameId", "1623730")],
        inicio: "2026-11-26 13:05:03",
        carpeta_declarada: None,
        cwd: Some("/home/jugador/.local/share/Steam"),
    },
    // El envoltorio del juego: el representante del grupo 1.
    ProcesoDeFixture {
        pid: 426782,
        ppid: 426674,
        comm: "wine64-preloader",
        exe: Some("/run/host/usr/share/steam/compatibilitytools.d/proton/files/lib/wine/i386-unix/wine64-preloader"),
        cmdline: "Z:\\home\\thor\\.local\\share\\Steam\\steamapps\\common\\Palworld\\Palworld.exe\0",
        entorno: &[("SteamAppId", "1623730"), ("SteamGameId", "1623730")],
        inicio: "2026-11-26 13:05:04",
        carpeta_declarada: Some("/home/jugador/.local/share/Steam/steamapps/common/Palworld"),
        cwd: Some(
            "/home/jugador/.local/share/Steam/steamapps/compatdata/1623730/pfx/drive_c/windows",
        ),
    },
];
