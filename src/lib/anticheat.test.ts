import { describe, expect, it } from 'vitest';
import {
  antiCheatsLabel,
  antiCheatVerdict,
  deniedCommand,
  deniedReason,
  isBlocked,
  isDenied,
  isDeniedCommand,
} from './anticheat';
import type { AntiCheatInfo } from './types';

const info = (status: string, anticheats: string[] = []): AntiCheatInfo => ({ status, anticheats });

describe('antiCheatVerdict', () => {
  it('Supported es el único estado que se da por bueno', () => {
    const v = antiCheatVerdict(info('Supported'));
    expect(v.severity).toBe('ok');
    expect(v.label).toBe('Funciona');
    expect(v.advice).toBeNull();
  });

  it('Running avisa de que no hay soporte oficial', () => {
    const v = antiCheatVerdict(info('Running'));
    expect(v.severity).toBe('caution');
    expect(v.detail).toContain('no está habilitado oficialmente');
    expect(v.advice).toContain('actualización');
  });

  it('Broken y Denied bloquean', () => {
    expect(antiCheatVerdict(info('Broken')).severity).toBe('blocked');
    expect(antiCheatVerdict(info('Denied')).severity).toBe('blocked');
    expect(isBlocked(info('Denied'))).toBe(true);
  });

  it('Planned no bloquea pero avisa', () => {
    expect(antiCheatVerdict(info('Planned')).severity).toBe('caution');
    expect(isBlocked(info('Planned'))).toBe(false);
  });

  it('no distingue mayúsculas', () => {
    expect(antiCheatVerdict(info('DENIED')).label).toBe('Bloqueado por el anti-cheat');
  });

  it('un estado nuevo nunca se trata como bueno', () => {
    const v = antiCheatVerdict(info('SomethingNew'));
    expect(v.severity).toBe('unknown');
    expect(v.label).toBe('SomethingNew');
  });

  it('sin datos devuelve un veredicto explícito', () => {
    for (const empty of [null, undefined, info('')]) {
      const v = antiCheatVerdict(empty);
      expect(v.severity).toBe('unknown');
      expect(v.label).toBe('Sin datos');
    }
  });
});

describe('antiCheatsLabel', () => {
  it('junta varios anti-cheats', () => {
    expect(antiCheatsLabel(info('Denied', ['Easy Anti-Cheat', 'BattlEye']))).toBe(
      'Easy Anti-Cheat + BattlEye'
    );
  });

  it('sin anticheats devuelve null', () => {
    expect(antiCheatsLabel(info('Supported'))).toBeNull();
    expect(antiCheatsLabel(null)).toBeNull();
  });
});

// ==================== PUERTA DURA ====================
// El hueco que cierra esto: la app avisaba «denegado por el anti-cheat» y aun así
// seguía generando comandos de lanzamiento. Seguir ese consejo puede costar la cuenta.

describe('isDenied', () => {
  it('Denied bloquea de verdad', () => {
    expect(isDenied(info('Denied'))).toBe(true);
    // La fuente no siempre escribe el estado en el mismo formato.
    expect(isDenied(info('DENIED'))).toBe(true);
    expect(isDenied(info('  denied  '))).toBe(true);
  });

  it('Broken avisa pero NO bloquea', () => {
    // No arranca, pero no hay riesgo de cuenta; el aviso ya está en la tarjeta.
    expect(isDenied(info('Broken'))).toBe(false);
  });

  it('sin datos o con estado raro no se bloquea por defecto', () => {
    // Negativo por defecto dejaría sin comandos juegos que van perfectamente,
    // porque el catálogo de AreWeAntiCheatYet no cubre todo Steam.
    const casos = [
      null,
      undefined,
      info(''),
      info('SomethingNew'),
      info('Planned'),
      info('Running'),
      info('Supported'),
    ];
    for (const caso of casos) expect(isDenied(caso)).toBe(false);
  });

  it('el motivo dice el anti-cheat y por qué no hay comando', () => {
    const msg = deniedReason(info('Denied', ['EA anticheat']));
    expect(msg).toContain('EA anticheat');
    expect(msg).toContain('no genera comandos');
    // Sin anticheats listados sigue siendo un motivo legible.
    expect(deniedReason(info('Denied'))).toContain('Anti-cheat denegado');
  });
});

describe('deniedCommand', () => {
  it('el marcador y el motivo no se confunden con un comando', () => {
    const marca = deniedCommand(2807960);
    expect(isDeniedCommand(marca)).toBe(true);
    expect(marca).toContain('2807960');
    // Un comando de verdad no se toma por un bloqueo.
    expect(isDeniedCommand('%command%')).toBe(false);
    expect(isDeniedCommand('gamemoderun %command%')).toBe(false);
  });
});
