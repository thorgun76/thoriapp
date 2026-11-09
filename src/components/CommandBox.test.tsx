import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
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
      expertMode={false}
      manualEditing={props.manualEditing ?? false}
      manualCommand=""
      info={{ ...info, native: props.native ?? false }}
      env={props.env ?? null}
      launcher={props.launcher}
      onCopy={vi.fn()}
      onSaveProfile={vi.fn()}
      onReset={vi.fn()}
      onToggleManualEditing={vi.fn()}
      onManualCommandChange={vi.fn()}
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
        expertMode={false}
        manualEditing={false}
        manualCommand=""
        info={{ ...info, tier: 'borked' }}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}
        onToggleManualEditing={vi.fn()}
        onManualCommandChange={vi.fn()}
      />
    );

    expect(screen.getByText(/ProtonDB: No funciona/)).toBeTruthy();
    expect(screen.queryByText(/ProtonDB: borked/)).toBeNull();
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
        expertMode
        manualEditing={false}
        manualCommand=""
        info={info}
        env={makeEnv()}
        blocked={blocked}
        blockedReason="Anti-cheat denegado (EA anticheat): no hay comando."
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}
        onToggleManualEditing={vi.fn()}
        onManualCommandChange={vi.fn()}
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
        expertMode={false}
        manualEditing={false}
        manualCommand=""
        info={info}
        env={null}
        onCopy={vi.fn()}
        onSaveProfile={vi.fn()}
        onReset={vi.fn()}
        onToggleManualEditing={vi.fn()}
        onManualCommandChange={vi.fn()}
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
