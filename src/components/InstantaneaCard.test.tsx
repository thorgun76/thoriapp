import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';

afterEach(cleanup);
import { InstantaneaCard } from './InstantaneaCard';
import type { Benchmark, ConfiguracionCapturada, SesionDeEjecucion, SnapshotConfig } from '../lib/types';

// La tarjeta pide la sesión de la prueba al desplegarse. Se simula la API para no depender
// de Tauri y para poder comprobar con qué se la llama.
vi.mock('../lib/api', () => ({
  api: { sesionDeBenchmark: vi.fn().mockResolvedValue(null) },
}));
import { api } from '../lib/api';

/**
 * Subfase 5.2: la tarjeta de configuración histórica.
 *
 * Lo que se comprueba aquí no es el dibujo, sino las dos reglas que sostienen la
 * funcionalidad: un hueco histórico nunca se rellena con datos actuales, y lo configurado
 * nunca se presenta como si se hubiera verificado durante la grabación.
 */

function snapshot(over: Partial<SnapshotConfig> = {}): SnapshotConfig {
  return {
    version: 1,
    identidad: {
      appid_real: 1623730,
      benchmark_id: 'b1',
      numero_prueba: 3,
      fecha: '2026-11-24 19:20:05',
    },
    configuracion: {
      comando_confirmado: {
        valor: 'mangohud %command%',
        procedencia: 'motor_rust',
      },
      revision_motor: 7,
      opciones_steam: {
        valor: 'mangohud %command%',
        procedencia: 'leido_de_steam',
      },
      perfil_id: 'perfil-1',
      perfil_nombre: {
        valor: 'Rendimiento',
        procedencia: 'configurado_en_thoriapp',
      },
      cambios_sin_guardar: false,
      modo_proton: { valor: false, procedencia: 'configurado_en_thoriapp' },
      ejecucion_verificada: null,
    },
    entorno: {
      gpu: { valor: 'AMD Radeon RX 6800', procedencia: 'detectado_en_sistema' },
      cpu: { valor: 'AMD Ryzen 7 5800X', procedencia: 'detectado_en_sistema' },
      ram_gb: { valor: 32, procedencia: 'detectado_en_sistema' },
      kernel: { valor: '6.8.0', procedencia: 'detectado_en_sistema' },
      driver: null,
      mesa: { valor: '24.0.9', procedencia: 'detectado_en_sistema' },
      proton_seleccionado: null,
      build_id: null,
      log_interval_ms: { valor: 100, procedencia: 'detectado_en_sistema' },
    },
    hardware_grabacion: null,
    resumen_mangohud: null,
    capturado_en: '2026-11-24 19:25:00',
    posterior_a_la_grabacion: true,
    ...over,
  };
}

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b1',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: '2026-11-24 19:20:05',
    avg_fps: 60,
    fps_1_percent_low: 50,
    fps_0_1_percent_low: 40,
    frame_time_avg: 16.6,
    number: 3,
    snapshot: snapshot(),
    ...over,
  } as Benchmark;
}

const abrir = () => {
  fireEvent.click(screen.getByRole('button', { name: /Configuración registrada/i }));
};

/**
 * Prueba 1. Un benchmark sin instantánea lo dice y no se rellena.
 */
describe('instantánea ausente (Prueba 1)', () => {
  it('s52 un benchmark antiguo dice «Instantánea no disponible»', () => {
    render(<InstantaneaCard bench={bench({ snapshot: null })} />);

    expect(screen.getByText('Instantánea no disponible')).toBeTruthy();
    // Y explica por qué no se completa con los datos de hoy.
    expect(screen.getByText(/No se completa con los datos actuales/i)).toBeTruthy();
    // No se inventa ninguna sección de configuración.
    expect(screen.queryByText(/Configuración registrada/i)).toBeNull();
  });
});

/**
 * Prueba 2. La sección arranca plegada y se despliega al pedirlo.
 */
describe('sección plegable (Prueba 2)', () => {
  it('s52 arranca cerrada y muestra el comando al abrirla', () => {
    render(<InstantaneaCard bench={bench()} />);

    const boton = screen.getByRole('button', { name: /Configuración registrada/i });
    expect(boton.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryAllByText('mangohud %command%')).toHaveLength(0);

    abrir();

    expect(boton.getAttribute('aria-expanded')).toBe('true');
    // Dos veces a propósito: el comando confirmado y las opciones de Steam del mismo texto.
    expect(screen.getAllByText('mangohud %command%')).toHaveLength(2);
  });
});

/**
 * Prueba 3. El comando se enseña literal y con su procedencia: lo que Thoriapp proponía
 * no es lo que Steam ejecutó.
 */
describe('comando confirmado (Prueba 3)', () => {
  it('s52 muestra el comando del motor sin presentarlo como ejecutado', () => {
    render(<InstantaneaCard bench={bench()} />);
    abrir();

    const comando = screen.getAllByText('mangohud %command%')[0];
    expect(comando.tagName.toLowerCase()).toBe('code');
    expect(comando.className).toContain('font-mono');
    // La procedencia del comando confirmado no se distingue por texto: el bloque del
    // comando ya dice «confirmado por Thoriapp» y un distintivo más sería ruido. Lo que sí
    // debe verse es la advertencia de que no es prueba de ejecución.
    expect(screen.getByText(/No prueba que Steam lo ejecutara/i)).toBeTruthy();
  });

  it('s52 sin comando registrado lo dice, sin inventarlo', () => {
    const snap = snapshot();
    snap.configuracion.comando_confirmado = null;
    render(<InstantaneaCard bench={bench({ snapshot: snap })} />);
    abrir();

    expect(screen.getByText(/No se registró el comando de este juego/i)).toBeTruthy();
  });
});

/**
 * Prueba 4. Proton y ejecución real: la distinción del punto 10.
 */
describe('proton configurado ≠ verificado (Prueba 4)', () => {
  it('s52 dice «no verificado» y no presume de ejecución', () => {
    const snap = snapshot();
    snap.configuracion.modo_proton = {
      valor: true,
      procedencia: 'configurado_en_thoriapp',
    };
    render(<InstantaneaCard bench={bench({ snapshot: snap })} />);
    abrir();

    expect(screen.getByText('Sí')).toBeTruthy();
    expect(screen.getAllByText('Configurado en Thoriapp').length).toBeGreaterThan(0);
    // Nunca se afirma que se ejecutara: no existe el distintivo de verificado.
    expect(screen.queryByText('Verificado en ejecución')).toBeNull();
    expect(screen.getByText(/no verificado/i)).toBeTruthy();
  });
});

/**
 * Prueba 5. Los huecos del entorno se dicen, no se rellenan.
 */
describe('datos ausentes del entorno (Prueba 5)', () => {
  it('s52 muestra y no rellena el driver, Proton ni el BuildID', () => {
    render(<InstantaneaCard bench={bench()} />);
    abrir();

    expect(screen.getByText('GPU')).toBeTruthy();
    expect(screen.getByText('AMD Radeon RX 6800')).toBeTruthy();
    expect(screen.getAllByText('No registrado').length).toBe(2);
    expect(screen.getByText('No disponible')).toBeTruthy();
  });

  it('s52 el intervalo de MangoHud se enseña en milisegundos', () => {
    render(<InstantaneaCard bench={bench()} />);
    abrir();
    expect(screen.getByText('100 ms')).toBeTruthy();
  });
});

/**
 * Prueba 6. La captura es posterior a la grabación y se dice.
 */
describe('momento de la captura (Prueba 6)', () => {
  it('s52 avisa de que se capturó después de grabar', () => {
    render(<InstantaneaCard bench={bench()} />);
    abrir();

    expect(screen.getByText(/después/i)).toBeTruthy();
    expect(screen.getByText(/No atestigua qué estuvo activo durante ella/i)).toBeTruthy();
  });
});

/**
 * Prueba 7. La cabecera resume sin abrir: perfil y AppID real.
 */
describe('cabecera de la instantánea (Prueba 7)', () => {
  it('s52 resume perfil y AppID sin desplegar', () => {
    render(<InstantaneaCard bench={bench()} />);

    const boton = screen.getByRole('button', { name: /Configuración registrada/i });
    expect(within(boton).getByText(/Perfil «Rendimiento»/)).toBeTruthy();
    expect(within(boton).getByText(/AppID 1623730/)).toBeTruthy();
  });

  it('s52 sin perfil lo dice en la cabecera', () => {
    const snap = snapshot();
    snap.configuracion.perfil_nombre = null;
    snap.configuracion.perfil_id = null;
    render(<InstantaneaCard bench={bench({ snapshot: snap })} />);

    expect(screen.getByText(/Sin perfil cargado/)).toBeTruthy();
  });
});

/**
 * Prueba 8. Copiar el comando es opcional y no rompe si no hay manejador.
 */
describe('copiar el comando (Prueba 8)', () => {
  it('s52 copia el comando literal', () => {
    const onCopy = vi.fn();
    render(<InstantaneaCard bench={bench()} onCopy={onCopy} />);
    abrir();

    fireEvent.click(screen.getByRole('button', { name: /Copiar/i }));
    expect(onCopy).toHaveBeenCalledWith('mangohud %command%');
  });

  it('s52 sin manejador no ofrece el botón', () => {
    render(<InstantaneaCard bench={bench()} />);
    abrir();
    expect(screen.queryByRole('button', { name: /Copiar/i })).toBeNull();
  });
});

/**
 * Incremento de cierre de 5.2: el equipo que registró MangoHud va separado del actual.
 */
describe('equipo de la grabación frente al actual (cierre 5.2)', () => {
  const grabacion = {
    os: { valor: 'Steam Runtime 4', procedencia: 'registrado_por_mangohud' },
    cpu: { valor: 'AMD Ryzen 5 5600X', procedencia: 'registrado_por_mangohud' },
    gpu: { valor: 'AMD Radeon RX 6600 (RADV NAVI23)', procedencia: 'registrado_por_mangohud' },
    ram_kib: { valor: 32777068, procedencia: 'registrado_por_mangohud' },
    ram_gib: { valor: 31.26, procedencia: 'registrado_por_mangohud' },
    kernel: { valor: '7.3.5-2-cachyos', procedencia: 'registrado_por_mangohud' },
    driver: null,
    cpuscheduler: { valor: 'performance', procedencia: 'registrado_por_mangohud' },
    intervalo_muestreo_ms: { valor: 100, procedencia: 'registrado_por_mangohud' },
  } as const;

  it('s52 muestra el bloque de la grabación con su procedencia', () => {
    render(<InstantaneaCard bench={bench({ snapshot: snapshot({ hardware_grabacion: grabacion as never }) })} />);
    abrir();

    expect(screen.getByText('Equipo durante la grabación')).toBeTruthy();
    expect(screen.getByText('AMD Ryzen 5 5600X')).toBeTruthy();
    expect(screen.getAllByText('Registrado por MangoHud').length).toBeGreaterThan(0);
  });

  it('s52 el runtime de Steam se etiqueta como entorno, no como distribución', () => {
    render(<InstantaneaCard bench={bench({ snapshot: snapshot({ hardware_grabacion: grabacion as never }) })} />);
    abrir();

    expect(screen.getByText('Steam Runtime 4')).toBeTruthy();
    expect(screen.getByText(/runtime de Steam, no tu distribución/i)).toBeTruthy();
  });

  it('s52 sin hardware de la grabación se dice desconocido y no se rellena con el actual', () => {
    render(<InstantaneaCard bench={bench({ snapshot: snapshot({ hardware_grabacion: null }) })} />);
    abrir();

    expect(screen.getByText(/no traía datos del equipo/i)).toBeTruthy();
    // El equipo actual sigue visible, pero en su propio bloque.
    expect(screen.getByText('Equipo al analizar')).toBeTruthy();
    expect(screen.getByText('AMD Ryzen 7 5800X')).toBeTruthy();
  });

  it('s52 avisa si el equipo de la grabación difiere del actual, sin tocar los datos', () => {
    render(<InstantaneaCard bench={bench({ snapshot: snapshot({ hardware_grabacion: grabacion as never }) })} />);
    abrir();

    // La grabación dice RX 6600 / Ryzen 5 5600X; el entorno de prueba, RX 6800 / 5800X.
    expect(screen.getByText(/no coincide del todo con el de ahora/i)).toBeTruthy();
    // Los dos valores siguen ahí, intactos.
    expect(screen.getByText('AMD Radeon RX 6600 (RADV NAVI23)')).toBeTruthy();
    expect(screen.getByText('AMD Radeon RX 6800')).toBeTruthy();
  });

  it('s52 sin diferencias no aparece el aviso', () => {
    const misma = {
      ...grabacion,
      cpu: { valor: 'AMD Ryzen 7 5800X', procedencia: 'registrado_por_mangohud' },
      gpu: { valor: 'AMD Radeon RX 6800', procedencia: 'registrado_por_mangohud' },
      kernel: { valor: '6.8.0', procedencia: 'registrado_por_mangohud' },
    };
    render(<InstantaneaCard bench={bench({ snapshot: snapshot({ hardware_grabacion: misma as never }) })} />);
    abrir();

    expect(screen.queryByText(/no coincide del todo con el de ahora/i)).toBeNull();
  });
});

/**
 * El resumen de MangoHud se muestra como fuente aparte, nunca mezclado con Thoriapp.
 */
describe('resumen de MangoHud (cierre 5.2)', () => {
  const resumen = {
    valores: [
      ['Average FPS', 60.4],
      ['1% Min FPS', 26.4349],
      ['0.1% Min FPS', 17.8632],
      ['97% Percentile FPS', 89.1928],
    ] as Array<[string, number]>,
    capturado_en: '2026-11-24 19:25:00',
  };

  it('s52 muestra el resumen con su propia metodología declarada', () => {
    render(<InstantaneaCard bench={bench({ snapshot: snapshot({ resumen_mangohud: resumen }) })} />);
    abrir();

    expect(screen.getByText('Resumen de MangoHud')).toBeTruthy();
    expect(screen.getByText(/No son las cifras de Thoriapp/i)).toBeTruthy();
    expect(screen.getByText('60.4')).toBeTruthy();
    expect(screen.getByText('26.4349')).toBeTruthy();
  });

  it('s52 sin resumen no aparece la sección', () => {
    render(<InstantaneaCard bench={bench({ snapshot: snapshot({ resumen_mangohud: null }) })} />);
    abrir();
    expect(screen.queryByText('Resumen de MangoHud')).toBeNull();
  });
});

/**
 * Subfase 5.3: la tarjeta dice qué se pudo observar de la ejecución.
 *
 * La tentación aquí es escribir «el juego no estaba corriendo» cuando el observador no ve
 * nada (contenedor, permisos). Las pruebas fijan lo contrario: sin evidencia, la tarjeta
 * habla de lo que falta, no de lo que pasó.
 */
describe('InstantaneaCard · sesión de ejecución (5.3)', () => {
  it('no pide la sesión hasta que se despliega', async () => {
    const { sesionDeBenchmark } = vi.mocked(api);
    sesionDeBenchmark.mockClear();
    render(<InstantaneaCard bench={bench()} />);
    expect(sesionDeBenchmark).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { expanded: false }));
    // Se pide exactamente una vez, para la prueba que se está mirando.
    await vi.waitFor(() => expect(sesionDeBenchmark).toHaveBeenCalledWith('b1'));
  });

  it('con sesión observada, dice cuándo se vio el juego', async () => {
    vi.mocked(api).sesionDeBenchmark.mockResolvedValue({
      id: 'ses_1',
      tipo: 'observada',
      identidad: {
        appid: 1623730,
        origen: 'entorno_del_proceso',
        candidatos: [1623730],
        nombre_observado: 'Palworld-Win64-Shipping',
        evidencia: ['SteamAppId y SteamGameId coinciden en 1623730'],
      },
      proceso: {
        pid: 4242,
        inicio_del_proceso: '2026-11-24 18:00:00',
        primera_observacion: '2026-11-24 18:00:05',
        ejecutable: 'Palworld-Win64-Shipping.exe',
        appid_del_entorno: { steam_app_id: '1623730', steam_game_id: '1623730' },
        bajo_proton: true,
        hijo_de_steam: null,
      },
      evidencia_proceso: 'observada',
      configuracion_inicial: null,
      grabaciones: ['b1'],
      abierta_en: '2026-11-24 18:00:05',
      ultima_observacion: '2026-11-24 18:40:05',
      cerrada_en: '2026-11-24 18:45:00',
    });
    render(<InstantaneaCard bench={bench()} />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    expect(await screen.findByText(/Se vio el juego en ejecución/)).toBeTruthy();
    expect(screen.getByText(/Palworld-Win64-Shipping\.exe/)).toBeTruthy();
    expect(screen.getByText(/hasta 2026-11-24 18:45:00/)).toBeTruthy();
    // Proton observado no demuestra que cada ajuste estuviera activo. El texto está partido
    // por el `<span>` del «no», así que se busca sobre el nodo padre.
    expect(screen.getByText(/El proceso iba bajo Proton/).textContent).toMatch(/no confirma/);
  });

  it('sin sesión, no afirma que el juego no corriera', async () => {
    vi.mocked(api).sesionDeBenchmark.mockResolvedValue(null);
    render(<InstantaneaCard bench={bench()} />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    expect(await screen.findByText(/No consta una ejecución observada/)).toBeTruthy();
    expect(screen.queryByText(/no estaba corriendo/i)).toBeNull();
    expect(screen.queryByText(/no se está ejecutando/i)).toBeNull();
  });

  it('una agrupación sin proceso no se presenta como observada', async () => {
    vi.mocked(api).sesionDeBenchmark.mockResolvedValue({
      id: 'g1',
      tipo: 'agrupada_sin_proceso',
      identidad: {
        appid: 251570,
        origen: 'inferido_por_nombre',
        candidatos: [251570],
        nombre_observado: '7DaysToDie',
        evidencia: [],
      },
      proceso: null,
      evidencia_proceso: 'sin_visibilidad',
      configuracion_inicial: null,
      grabaciones: [],
      abierta_en: '2026-11-24 18:00:00',
      ultima_observacion: null,
      cerrada_en: null,
    });
    render(<InstantaneaCard bench={bench()} />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    expect(await screen.findByText(/sin proceso identificado/i)).toBeTruthy();
    expect(screen.queryByText(/Se vio el juego en ejecución/)).toBeNull();
  });
});

/**
 * Prueba extra (5.3): el AppID llega resuelto desde Rust, con su procedencia.
 *
 * Regresión del fallo real de la prueba 15: la cabecera decía «AppID desconocido» aunque
 * la sesión asociada tenía el AppID 1623730. Rust ya lo resuelve desde la sesión; aquí se
 * comprueba que la tarjeta lo enseña y no lo da por supuesto.
 */
describe('AppID resuelto (5.3)', () => {
  it('muestra el AppID observado en el proceso con su procedencia', () => {
    const snap = snapshot({
      identidad: {
        appid_real: 1623730,
        appid_procedencia: 'observado_en_proceso',
        benchmark_id: 'b1',
        numero_prueba: 3,
        fecha: '2026-11-24 19:20:05',
      },
    });
    render(<InstantaneaCard bench={bench({ snapshot: snap })} />);
    expect(screen.getByText(/AppID 1623730 \(observado en el proceso\)/)).toBeTruthy();
    expect(screen.queryByText(/AppID desconocido/)).toBeNull();
  });

  it('un AppID inferido por nombre se presenta como inferencia', () => {
    const snap = snapshot({
      identidad: {
        appid_real: 1623730,
        appid_procedencia: 'inferido',
        benchmark_id: 'b1',
        numero_prueba: 3,
        fecha: '2026-11-24 19:20:05',
      },
    });
    render(<InstantaneaCard bench={bench({ snapshot: snap })} />);
    expect(screen.getByText(/inferido por nombre/)).toBeTruthy();
  });

  it('sin AppID conocido sigue diciendo que se desconoce', () => {
    const snap = snapshot({
      identidad: {
        appid_real: null,
        appid_procedencia: 'desconocido',
        benchmark_id: 'b1',
        numero_prueba: 3,
        fecha: '2026-11-24 19:20:05',
      },
    });
    render(<InstantaneaCard bench={bench({ snapshot: snap })} />);
    expect(screen.getByText(/AppID desconocido/)).toBeTruthy();
  });
});

/**
 * Quinta corrección de 5.3: la configuración de la sesión no es la del arranque.
 *
 * La sesión de Deskrawl empezó a las 14:16:01 y su configuración figuraba capturada a las
 * 14:20:30. La marca `posterior_al_arranque` ya venía del backend, pero **nadie la mostraba**:
 * se leía como si fuera lo puesto al empezar a jugar.
 */
describe('InstantaneaCard · configuración de la sesión posterior al arranque', () => {
  function sesionCon(configuracion_inicial: ConfiguracionCapturada | null): SesionDeEjecucion {
    return {
      id: 'ses_deskrawl',
      tipo: 'observada',
      identidad: {
        appid: 4623570,
        origen: 'entorno_del_proceso',
        candidatos: [4623570],
        nombre_observado: 'Deskrawl',
        evidencia: ['SteamAppId y SteamGameId coinciden en 4623570'],
      },
      proceso: {
        pid: 40200,
        inicio_del_proceso: '2026-11-24 14:16:00',
        primera_observacion: '2026-11-24 14:16:01',
        ejecutable: 'wine64-preloader',
        appid_del_entorno: { steam_app_id: '4623570', steam_game_id: '4623570' },
        bajo_proton: true,
        hijo_de_steam: null,
      },
      evidencia_proceso: 'observada',
      configuracion_inicial,
      grabaciones: ['b1'],
      abierta_en: '2026-11-24 14:16:01',
      ultima_observacion: '2026-11-24 14:40:01',
      cerrada_en: null,
    };
  }

  it('avisa de que la configuración se capturó después de arrancar el juego', async () => {
    vi.mocked(api).sesionDeBenchmark.mockResolvedValue(
      sesionCon({
        comando_confirmado: null,
        perfil: null,
        opciones_steam: null,
        revision_motor: null,
        capturado_en: '2026-11-24 14:20:30',
        posterior_al_arranque: true,
      })
    );
    render(<InstantaneaCard bench={bench()} />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    // Se busca el aviso propio de la sesión, no cualquier «después» de la tarjeta.
    const aviso0 = await screen.findByText(/La configuración de esta sesión se capturó/);
    // Y se dice cuándo empezó, para que la diferencia se vea sin calcularla.
    expect(aviso0.textContent).toContain('14:16:01');
    // El texto está partido por el `<strong>` del «después», así que se busca sobre el
    // nodo contenedor, igual que hace el test de Proton.
    const aviso = screen
      .getByText(/La configuración de esta sesión se capturó/)
      .closest('p') as HTMLElement;
    expect(aviso.textContent).toContain('no atestigua qué');
    expect(aviso.textContent).toContain('al empezar a jugar');
  });

  it('sin la marca no se añade el aviso', async () => {
    vi.mocked(api).sesionDeBenchmark.mockResolvedValue(
      sesionCon({
        comando_confirmado: null,
        perfil: null,
        opciones_steam: null,
        revision_motor: null,
        capturado_en: '2026-11-24 14:16:00',
        posterior_al_arranque: false,
      })
    );
    render(<InstantaneaCard bench={bench()} />);
    fireEvent.click(screen.getByRole('button', { expanded: false }));

    expect(await screen.findByText(/Se vio el juego en ejecución/)).toBeTruthy();
    expect(
      screen.queryByText(/La configuración de esta sesión se capturó/)
    ).toBeNull();
  });
});
