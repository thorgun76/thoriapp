import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { claseDeAjuste } from '../lib/gameView';
import { CommandBox } from './CommandBox';
import type { RuntimeEnv } from '../lib/types';

afterEach(cleanup);

const info = { name: 'Juego', native: false, errors: [] };

function makeEnv(overrides: Partial<RuntimeEnv> = {}): RuntimeEnv {
  return {
    binaries: [],
    mangohud_binary: null,
    mangohud_vulkan_layer: null,
    steam_flatpak: false,
    ntsync: false,
    kernel: '6.11.0',
    ...overrides,
  };
}

function renderBox(props: {
  launchCommand?: string;
  env?: RuntimeEnv | null;
  launcher?: string;
  native?: boolean;
  manualEditing?: boolean;
}) {
  return render(
    <CommandBox
      gameName="Juego"
      launchCommand={props.launchCommand ?? '%command%'}
      activeCount={0}
      copied={false}
      manualEditing={props.manualEditing ?? false}
      manualCommand=""
      info={{ ...info, native: props.native ?? false }}
      env={props.env ?? null}
      launcher={props.launcher}
      onCopy={vi.fn()}
      onSaveProfile={vi.fn()}
      onReset={vi.fn()}

      onConfirmarEdicion={vi.fn()}
    />
  );
}

describe('CommandBox: revisión previa del comando', () => {
  it('avisa de un wrapper que no está instalado', () => {
    renderBox({ launchCommand: 'gamemoderun %command%', env: makeEnv() });

    expect(screen.getByText(/«gamemoderun» no está instalado/)).toBeTruthy();
    // Y dice cómo arreglarlo.
    expect(screen.getByText(/apt install gamemode/)).toBeTruthy();
  });

  it('no avisa cuando todo lo que usa el comando existe', () => {
    renderBox({
      launchCommand: 'gamemoderun %command%',
      env: makeEnv({ binaries: [{ name: 'gamemoderun', path: '/usr/bin/gamemoderun' }] }),
    });

    expect(screen.queryByText(/no está instalado/)).toBeNull();
    expect(screen.getByText(/está instalado en tu sistema/)).toBeTruthy();
  });

  it('sin datos del sistema no muestra ni avisos ni el visto bueno', () => {
    renderBox({ launchCommand: 'gamemoderun %command%', env: null });

    expect(screen.queryByText(/no está instalado/)).toBeNull();
    expect(screen.queryByText(/está instalado en tu sistema/)).toBeNull();
  });

  it('avisa de variables de Proton en un juego nativo', () => {
    renderBox({
      launchCommand: 'DXVK_HUD=fps %command%',
      env: makeEnv(),
      native: true,
    });

    expect(screen.getByText(/Variables de Proton en un juego nativo/)).toBeTruthy();
  });

  it('avisa cuando Steam es Flatpak y el comando usa MangoHud', () => {
    renderBox({
      launchCommand: 'mangohud %command%',
      env: makeEnv({
        steam_flatpak: true,
        binaries: [{ name: 'mangohud', path: '/usr/bin/mangohud' }],
      }),
      launcher: 'steam',
    });

    expect(screen.getByText(/Steam es Flatpak/)).toBeTruthy();
  });
});

describe('CommandBox: tier de ProtonDB', () => {
  it('muestra el tier en español, no el valor crudo en inglés', () => {
    render(
      <CommandBox
        gameName="Battlefield 6"
        launchCommand="%command%"
        activeCount={0}
        copied={false}
        manualEditing={false}
        manualCommand=""
        info={{ ...info, tier: 'borked' }}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}

      onConfirmarEdicion={vi.fn()}
      />
    );

    // «ProtonDB · No funciona»: el nivel se traduce, no se deja el valor crudo.
    expect(screen.getByText(/ProtonDB · No funciona/)).toBeTruthy();
    expect(screen.queryByText(/borked/)).toBeNull();
  });
});

// ==================== PUERTA DURA DEL ANTI-CHEAT ====================
// El bug: la app decía «denegado por el anti-cheat» y seguía mostrando un comando
// listo para copiar. Aquí ya no hay nada que copiar.

describe('CommandBox: anti-cheat denegado', () => {
  const blockedBox = (blocked: boolean) =>
    render(
      <CommandBox
        gameName="Battlefield 6"
        launchCommand={blocked ? 'Anti-cheat denegado: …' : '%command%'}
        activeCount={0}
        copied={false}
        manualEditing={false}
        manualCommand=""
        info={info}
        env={makeEnv()}
        blocked={blocked}
        blockedReason="Anti-cheat denegado (EA anticheat): no hay comando."
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}

      onConfirmarEdicion={vi.fn()}
      />
    );

  it('no ofrece copiar ni guardar el comando', () => {
    blockedBox(true);
    expect(screen.getByText(/Sin comando de lanzamiento/)).toBeTruthy();
    expect(screen.getByText(/Anti-cheat denegado/)).toBeTruthy();
    expect(screen.queryByText(/Copiar comando/)).toBeNull();
    expect(screen.queryByText(/Guardar perfil/)).toBeNull();
    // Tampoco el editor manual: sería otra vía para montar el comando a mano.
    expect(screen.queryByText(/Editar comando manualmente/)).toBeNull();
  });

  it('sin bloqueo la caja sigue igual', () => {
    blockedBox(false);
    expect(screen.queryByText(/Sin comando de lanzamiento/)).toBeNull();
    expect(screen.getByText(/Copiar comando/)).toBeTruthy();
  });
});

describe('CommandBox: resumen de lo que incluye el comando', () => {
  const withParts = (activeParts: { value: string; kind: 'env' | 'wrapper' | 'arg' }[]) =>
    render(
      <CommandBox
        gameName="Juego"
        launchCommand="gamemoderun DXVK_HUD=1 %command% -novid"
        activeCount={activeParts.length}
        activeParts={activeParts}
        copied={false}
        manualEditing={false}
        manualCommand=""
        info={info}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}

      onConfirmarEdicion={vi.fn()}
      />
    );

  it('lista cada pieza del comando con su valor literal', () => {
    withParts([
      { value: 'gamemoderun', kind: 'wrapper' },
      { value: 'DXVK_HUD=1', kind: 'env' },
      { value: '-novid', kind: 'arg' },
    ]);

    expect(screen.getByText('Incluye')).toBeTruthy();
    for (const v of ['gamemoderun', 'DXVK_HUD=1', '-novid']) {
      expect(screen.getByText(v)).toBeTruthy();
    }
  });

  it('enseña el valor del ajuste, nunca un índice interno', () => {
    // Regresión del chip «9»: indexar `rules.params[indice]` fallaba para las
    // recomendaciones (no están en `params`), y el chip acababa mostrando el índice.
    withParts([
      { value: 'mangohud', kind: 'arg' },
      { value: 'RADV_PERFTEST=sam', kind: 'env' },
    ]);
    expect(screen.getByText('mangohud')).toBeTruthy();
    expect(screen.getByText('RADV_PERFTEST=sam')).toBeTruthy();
    expect(screen.queryByText('9')).toBeNull();
  });

  it('sin piezas no aparece el resumen', () => {
    // Un comando base no tiene nada que resumir, y un «Incluye» vacío sería ruido.
    withParts([]);
    expect(screen.queryByText('Incluye')).toBeNull();
  });

  it('explica qué es cada tipo de pieza al pasar el ratón', () => {
    withParts([
      { value: 'gamemoderun', kind: 'wrapper' },
      { value: 'DXVK_HUD=1', kind: 'env' },
    ]);
    expect(screen.getByTitle(/Programa que envuelve el lanzamiento/)).toBeTruthy();
    expect(screen.getByTitle(/Variable de entorno/)).toBeTruthy();
  });
});

describe('CommandBox: modo de ejecución', () => {
  const renderModo = (preparandoProton: boolean, native = true) =>
    render(
      <CommandBox
        gameName="Juego"
        launchCommand="%command%"
        activeCount={0}
        activeParts={[]}
        copied={false}
        manualEditing={false}
        manualCommand=""
        info={{ ...info, native }}
        env={null}
        preparandoProton={preparandoProton}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}

      onConfirmarEdicion={vi.fn()}
      />
    );

  it('dice el modo detectado cuando no se prepara Proton', () => {
    renderModo(false, true);
    // `GameBadges` pinta su propia etiqueta con el mismo texto: vale cualquiera.
    expect(screen.getAllByText('Linux nativo').length).toBeGreaterThan(0);
  });

  it('con «Probar con Proton» activo no se contradice: avisa de que se prepara para Proton', () => {
    // Antes la caja seguía diciendo «Linux nativo» con el interruptor de Proton puesto.
    renderModo(true, true);
    expect(screen.getByText(/Preparando para Proton/i)).toBeTruthy();
    // Ya no se afirma a la vez que el comando es «Linux nativo»: las insignias del juego
    // pueden seguir describiendo lo detectado, pero la etiqueta del comando no se contradice.
    expect(screen.queryByText(/^Linux nativo$/, { selector: 'span.px-1\\.5' })).toBeNull();
  });
});

describe('CommandBox: estabilidad de altura', () => {
  /**
   * Regresión del salto de ~92 px medido en la app real: la fila de chips se montaba solo
   * cuando había ajustes, así que activar el primero empujaba hacia abajo todo lo de
   * debajo y desactivar el último lo devolvía. Ahora las dos filas existen siempre.
   */
  const montar = (activeParts: { value: string; kind: 'env' | 'wrapper' | 'arg' }[]) =>
    render(
      <CommandBox
        gameName="Juego"
        launchCommand="mangohud %command%"
        activeCount={activeParts.length}
        activeParts={activeParts}
        copied={false}
        manualEditing={false}
        manualCommand=""
        info={info}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}

      onConfirmarEdicion={vi.fn()}
      />
    );

  it('la fila de ajustes existe aunque no haya ninguno', () => {
    const { container } = montar([]);
    expect(container.querySelector('[aria-label="Ajustes reconocidos activos"]')).not.toBeNull();
  });

  it('las filas ocupan sitio desde el principio (min-h fijo)', () => {
    const { container } = montar([]);
    const fila = container.querySelector('[aria-label="Ajustes reconocidos activos"]') as HTMLElement;
    expect(fila.className).toContain('min-h-[30px]');
  });

  it('añadir una pieza no monta ni desmonta filas', () => {
    const { container, rerender } = montar([]);
    const filasAntes = container.querySelectorAll('.min-h-\\[30px\\]').length;

    rerender(
      <CommandBox
        gameName="Juego"
        launchCommand="mangohud %command%"
        activeCount={1}
        activeParts={[{ value: 'mangohud', kind: 'wrapper' }]}
        copied={false}
        manualEditing={false}
        manualCommand=""
        info={info}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}

      onConfirmarEdicion={vi.fn()}
      />
    );

    expect(container.querySelectorAll('.min-h-\\[30px\\]').length).toBe(filasAntes);
  });
});

describe('CommandBox: editor manual del comando', () => {
  const abrir = (manualCommand = 'mangohud %command%') =>
    render(
      <CommandBox
        gameName="Juego"
        launchCommand={manualCommand}
        activeCount={0}
        copied={false}
        manualEditing={false}
        manualCommand={manualCommand}
        info={{ name: 'Juego', native: false, errors: [] }}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}
        onConfirmarEdicion={vi.fn()}
      />
    );

  it('el botón «Editar comando» está siempre disponible', () => {
    abrir();
    expect(screen.getByRole('button', { name: /editar comando/i })).toBeTruthy();
  });

  it('el editor no está desplegado hasta que se pulsa el botón', () => {
    abrir();
    expect(screen.queryByLabelText('Comando de lanzamiento')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: /editar comando/i }));

    expect(screen.getByLabelText('Comando de lanzamiento')).toBeTruthy();
  });

  it('al abrirse, parte del comando confirmado', () => {
    abrir('gamemoderun %command%');
    fireEvent.click(screen.getByRole('button', { name: /editar comando/i }));

    expect((screen.getByLabelText('Comando de lanzamiento') as HTMLTextAreaElement).value).toBe(
      'gamemoderun %command%'
    );
  });

  it('escribir no manda nada al motor: solo se envía al confirmar', () => {
    const onConfirmarEdicion = vi.fn();
    abrir();
    fireEvent.click(screen.getByRole('button', { name: /editar comando/i }));

    fireEvent.change(screen.getByLabelText('Comando de lanzamiento'), {
      target: { value: 'nuevo %command%' },
    });

    expect(onConfirmarEdicion).not.toHaveBeenCalled();
  });

  it('confirmar envía el texto escrito al motor', () => {
    const onConfirmarEdicion = vi.fn();
    render(
      <CommandBox
        gameName="Juego"
        launchCommand="%command%"
        activeCount={0}
        copied={false}
        manualEditing={false}
        manualCommand="%command%"
        info={{ name: 'Juego', native: false, errors: [] }}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}
        onConfirmarEdicion={onConfirmarEdicion}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: /editar comando/i }));
    fireEvent.change(screen.getByLabelText('Comando de lanzamiento'), {
      target: { value: 'PROTON_USE_NTSYNC=1 %command%' },
    });

    fireEvent.click(screen.getByRole('button', { name: /confirmar/i }));

    expect(onConfirmarEdicion).toHaveBeenCalledWith('PROTON_USE_NTSYNC=1 %command%');
  });

  it('cancelar no manda nada y cierra el editor', () => {
    const onConfirmarEdicion = vi.fn();
    render(
      <CommandBox
        gameName="Juego"
        launchCommand="%command%"
        activeCount={0}
        copied={false}
        manualEditing={false}
        manualCommand="%command%"
        info={{ name: 'Juego', native: false, errors: [] }}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}
        onConfirmarEdicion={onConfirmarEdicion}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: /editar comando/i }));
    fireEvent.change(screen.getByLabelText('Comando de lanzamiento'), {
      target: { value: 'borrado %command%' },
    });

    fireEvent.click(screen.getByRole('button', { name: /cancelar/i }));

    expect(onConfirmarEdicion).not.toHaveBeenCalled();
    expect(screen.queryByLabelText('Comando de lanzamiento')).toBeNull();
  });

  it('cancelar y reabrir descarta el borrador: vuelve el comando confirmado', () => {
    abrir('mangohud %command%');
    fireEvent.click(screen.getByRole('button', { name: /editar comando/i }));
    fireEvent.change(screen.getByLabelText('Comando de lanzamiento'), {
      target: { value: 'texto tirado %command%' },
    });
    fireEvent.click(screen.getByRole('button', { name: /cancelar/i }));

    fireEvent.click(screen.getByRole('button', { name: /editar comando/i }));

    expect((screen.getByLabelText('Comando de lanzamiento') as HTMLTextAreaElement).value).toBe(
      'mangohud %command%'
    );
  });

  it('con un comando vacío no se puede confirmar', () => {
    abrir('%command%');
    fireEvent.click(screen.getByRole('button', { name: /editar comando/i }));
    fireEvent.change(screen.getByLabelText('Comando de lanzamiento'), {
      target: { value: '   ' },
    });

    expect(
      (screen.getByRole('button', { name: /confirmar/i }) as HTMLButtonElement).disabled
    ).toBe(true);
  });
});

describe('gameView: clasificación de ajustes', () => {
  it('marca como diagnóstico lo que sirve para medir', () => {
    expect(claseDeAjuste({ impact: 'medicion' })).toBe('diagnostico');
  });

  it('marca como riesgo lo que puede empeorar el rendimiento', () => {
    // RADV_DEBUG=nofastclears: baja el rendimiento a propósito.
    expect(claseDeAjuste({ impact: 'evitar' })).toBe('riesgo');
  });

  it('el riesgo gana al diagnóstico si coinciden', () => {
    expect(claseDeAjuste({ impact: 'evitar', experimental: true })).toBe('riesgo');
  });

  it('usa el campo experimental existente', () => {
    expect(claseDeAjuste({ impact: 'medio', experimental: true })).toBe('experimental');
  });

  it('reconoce «Experimental» al principio del aviso ya existente', () => {
    expect(
      claseDeAjuste({ impact: 'medio', warn: 'Experimental: puede dar fallos con el cursor.' })
    ).toBe('experimental');
  });

  it('un ajuste normal no lleva etiqueta', () => {
    expect(claseDeAjuste({ impact: 'alto', warn: 'Solo en GPUs con RT.' })).toBeNull();
  });
});
