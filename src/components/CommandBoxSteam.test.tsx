import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { GameInfo, OpcionesSteam } from '../lib/types';
import { CommandBox } from './CommandBox';

afterEach(cleanup);

const PALWORLD = 'mesa_glthread=true game-performance mangohud %command%';

const info: GameInfo = {
  appid: 1623730,
  name: 'Palworld',
  native: false,
  tier: undefined,
  score: undefined,
  total: undefined,
  confidence: undefined,
  errors: [],
};

function renderBox(props: {
  launchCommand?: string;
  appid?: number;
  steamOpciones?: OpcionesSteam | null;
  onImportarDeSteam?: () => void;
  blocked?: boolean;
  blockedReason?: string;
}) {
  return render(
    <CommandBox
      gameName="Palworld"
      launchCommand={props.launchCommand ?? '%command%'}
      activeCount={0}
      copied={false}
      expertMode={false}
      manualEditing={false}
      manualCommand=""
      info={info}
      appid={props.appid ?? 1623730}
      env={null}
      blocked={props.blocked ?? false}
      blockedReason={props.blockedReason ?? ''}
      onCopy={vi.fn()}
      onSaveProfile={vi.fn()}
      onReset={vi.fn()}
      onToggleManualEditing={vi.fn()}
      onManualCommandChange={vi.fn()}
      steamOpciones={props.steamOpciones ?? null}
      onImportarDeSteam={props.onImportarDeSteam}
    />
  );
}

describe('CommandBox: aviso de Steam', () => {
  it('avisa solo si Steam tiene un comando distinto', () => {
    renderBox({ steamOpciones: { opciones: PALWORLD, cuenta: '1' } });
    expect(screen.getByTestId('aviso-steam')).toBeTruthy();
  });

  it('no avisa si coincide', () => {
    renderBox({ launchCommand: PALWORLD, steamOpciones: { opciones: PALWORLD, cuenta: '1' } });
    expect(screen.queryByTestId('aviso-steam')).toBeNull();
  });

  it('no avisa si no se pudo leer Steam', () => {
    // `null` = no se sabe nada. Callarse es lo correcto; inventar `%command%` era el bug.
    renderBox({ steamOpciones: null });
    expect(screen.queryByTestId('aviso-steam')).toBeNull();
  });

  it('no avisa en un juego sin AppID', () => {
    renderBox({ appid: 0, steamOpciones: { opciones: PALWORLD, cuenta: '1' } });
    expect(screen.queryByTestId('aviso-steam')).toBeNull();
  });

  it('no avisa si el juego está bloqueado por anti-cheat', () => {
    // Se usa el motivo real que manda `App`: con `blocked: true` pero sin motivo,
    // `BlockedCard` (que solo comprueba el motivo) sí pinta la caja normal.
    renderBox({
      launchCommand: 'Sin comando de lanzamiento: anti-cheat denegado',
      steamOpciones: { opciones: PALWORLD, cuenta: '1' },
      blocked: true,
      blockedReason: 'Anti-cheat denegado',
    });
    expect(screen.queryByTestId('aviso-steam')).toBeNull();
  });

  it('importar lleva el texto de Steam al editor sin tocar Steam', () => {
    const onImportarDeSteam = vi.fn();
    renderBox({ steamOpciones: { opciones: PALWORLD, cuenta: '1' }, onImportarDeSteam });
    fireEvent.click(screen.getByRole('button', { name: /Traer a Thoriapp/i }));
    expect(onImportarDeSteam).toHaveBeenCalledTimes(1);
  });

  it('el comando de Thoriapp se sigue viendo junto al de Steam', () => {
    const { container } = renderBox({ steamOpciones: { opciones: PALWORLD, cuenta: '1' } });
    const t = container.textContent ?? '';
    expect(t).toContain('%command%');
    expect(t).toContain(PALWORLD);
  });

  it('no añade ningún botón permanente: solo el importador', () => {
    renderBox({ steamOpciones: { opciones: PALWORLD, cuenta: '1' } });
    // El editor manual sigue teniendo su interruptor (decisión de diseño del proyecto:
    // se mantiene para preparar pruebas sin tocar Steam). Lo que no se añaden son
    // botones nuevos permanentes, salvo el importador, que solo sale con discrepancia.
    const botones = screen.getAllByRole('button').map((b) => b.textContent ?? '');
    expect(botones.filter((t) => /Traer a Thoriapp/.test(t)).length).toBe(1);
    expect(botones.filter((t) => /Copiar el de Thoriapp/.test(t)).length).toBe(1);
  });

  it('sin discrepancia no aparece ningún texto sobre Steam', () => {
    const { container } = renderBox({ steamOpciones: null });
    expect(container.textContent).not.toMatch(/Steam tiene otras opciones/i);
  });
});
