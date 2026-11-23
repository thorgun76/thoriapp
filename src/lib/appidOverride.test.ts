import { beforeEach, describe, expect, it } from 'vitest';
import { clearOverride, getOverride, loadOverrides, overrideKey, setOverride } from './appidOverride';

describe('appidOverride', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('guarda y recupera el AppID de un juego', () => {
    const game = { name: 'No Man’s Sky', launcher: 'lutris' };

    expect(getOverride(game)).toBeNull();
    setOverride(game, 275850);
    expect(getOverride(game)).toBe(275850);
  });

  it('la clave no depende de mayúsculas, acentos ni del launcher por defecto', () => {
    // La misma entrada debe encontrarse aunque Lutris reescriba el nombre con
    // otra puntuación; si no, el vínculo se perdería al reabrir la biblioteca.
    setOverride({ name: 'No Man’s Sky', launcher: 'lutris' }, 275850);

    expect(getOverride({ name: 'no man’s sky', launcher: 'lutris' })).toBe(275850);
    expect(getOverride({ name: 'No Mans Sky', launcher: 'lutris' })).toBe(275850);
    // Otro launcher es otro juego: no se mezclan.
    expect(getOverride({ name: 'No Man’s Sky', launcher: 'heroic' })).toBeNull();
  });

  it('sobrevive a un almacén corrupto sin romper la app', () => {
    localStorage.setItem('thoriapp:appidOverrides', '{no es json');
    expect(loadOverrides()).toEqual({});
    expect(getOverride({ name: 'X' })).toBeNull();

    // Y los valores que no son AppIDs válidos se descartan al leer.
    localStorage.setItem('thoriapp:appidOverrides', JSON.stringify({ 'steam:x': -3, 'steam:y': '12' }));
    expect(loadOverrides()).toEqual({});
  });

  it('clearOverride borra solo ese juego', () => {
    const a = { name: 'Juego A', launcher: 'lutris' };
    const b = { name: 'Juego B', launcher: 'lutris' };
    setOverride(a, 10);
    setOverride(b, 20);

    clearOverride(a);

    expect(getOverride(a)).toBeNull();
    expect(getOverride(b)).toBe(20);
  });

  it('la clave incluye launcher y nombre normalizado', () => {
    expect(overrideKey({ name: 'Half-Life 2', launcher: 'lutris' })).toBe('lutris:half life 2');
    expect(overrideKey({ name: 'Half-Life 2' })).toBe('steam:half life 2');
  });
});
