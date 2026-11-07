import { describe, expect, it } from 'vitest';
import { dedupeAgainstSteam, normalizeTitle } from './dedupe';
import type { Game } from './types';

const steam = (name: string, appid = 0): Game => ({ appid, name, launcher: 'steam' });
const ext = (name: string, launcher: string, appid = 0): Game => ({ appid, name, launcher });

describe('normalizeTitle', () => {
  it('ignora mayúsculas y espacios', () => {
    expect(normalizeTitle('Soulmask')).toBe(normalizeTitle('SOULMASK'));
    expect(normalizeTitle('Soul mask')).toBe('soulmask');
  });

  it('quita acentos, marcas y puntuación', () => {
    expect(normalizeTitle('Pokémon™')).toBe('pokemon');
    expect(normalizeTitle('Assassin\'s Creed: Origins')).toBe('assassinscreedorigins');
  });

  it('quita el sufijo de tienda que añaden algunos launchers', () => {
    expect(normalizeTitle('Soulmask (Steam)')).toBe('soulmask');
    expect(normalizeTitle('Soulmask - GOG')).toBe('soulmask');
  });

  it('trata "Edition" y "Edición" como lo mismo', () => {
    expect(normalizeTitle('Game Edition')).toBe(normalizeTitle('Game Edición'));
    expect(normalizeTitle('Juego Edición Digital')).toBe('juegoediciondigital');
  });
});

describe('dedupeAgainstSteam', () => {
  it('oculta el mismo juego si también está en Lutris', () => {
    // Caso reportado: Soulmask en Steam y repetido en Lutris y Heronite.
    const result = dedupeAgainstSteam(
      [steam('Soulmask')],
      [ext('Soulmask', 'lutris'), ext('Soulmask', 'heronite')]
    );
    expect(result).toEqual([]);
  });

  it('oculta también si el título difiere en mayúsculas o formato', () => {
    const result = dedupeAgainstSteam(
      [steam('Soulmask')],
      [ext('SOULMASK', 'lutris'), ext('Soulmask™', 'heroic')]
    );
    expect(result).toEqual([]);
  });

  it('conserva los juegos que no están en Steam', () => {
    const result = dedupeAgainstSteam(
      [steam('Soulmask')],
      [ext('Soulmask', 'lutris'), ext('Valheim', 'lutris'), ext('Control', 'heroic')]
    );
    expect(result.map((g) => g.name)).toEqual(['Valheim', 'Control']);
  });

  it('oculta por AppID de Steam aunque el título cambie', () => {
    // Un juego de Steam abierto desde Lutris conserva su appid: es la
    // coincidencia más fiable, por encima del nombre.
    const result = dedupeAgainstSteam([steam('Soulmask', 2646460)], [ext('Soulmask Mod', 'lutris', 2646460)]);
    expect(result).toEqual([]);
  });

  it('no oculta un juego externo sin appid solo por compartir appid 0', () => {
    const result = dedupeAgainstSteam([steam('A', 0)], [ext('B', 'lutris', 0)]);
    expect(result.map((g) => g.name)).toEqual(['B']);
  });

  it('sin biblioteca de Steam no oculta nada', () => {
    const externals = [ext('Soulmask', 'lutris')];
    expect(dedupeAgainstSteam([], externals)).toEqual(externals);
  });

  it('no altera el orden ni el contenido del resto', () => {
    const externals = [ext('Zeta', 'lutris'), ext('Soulmask', 'heroic'), ext('Alpha', 'lutris')];
    const result = dedupeAgainstSteam([steam('Soulmask')], externals);
    expect(result.map((g) => g.name)).toEqual(['Zeta', 'Alpha']);
  });

  it('dos juegos de Steam con el mismo nombre ocultan una sola copia externa', () => {
    const result = dedupeAgainstSteam(
      [steam('Soulmask'), steam('Soulmask')],
      [ext('Soulmask', 'lutris')]
    );
    expect(result).toEqual([]);
  });

  it('conserva un juego externo con nombre solo de símbolos', () => {
    // No se puede comparar un título vacío: mejor mostrarlo que perderlo.
    const result = dedupeAgainstSteam([steam('Soulmask')], [ext('***', 'lutris')]);
    expect(result.map((g) => g.name)).toEqual(['***']);
  });
});
