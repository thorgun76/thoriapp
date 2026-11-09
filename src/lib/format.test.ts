import { describe, expect, it } from 'vitest';
import { fmtDuration, formatTimestamp, matchesSearch, tierDescription, tierLabel } from './format';

describe('fmtDuration', () => {
  it('menos de un minuto en segundos', () => {
    expect(fmtDuration(0)).toBe('0 s');
    expect(fmtDuration(30)).toBe('30 s');
    expect(fmtDuration(59)).toBe('59 s');
  });

  it('minutos exactos sin resto de segundos', () => {
    expect(fmtDuration(60)).toBe('1 min');
    expect(fmtDuration(120)).toBe('2 min');
  });

  it('minutos con resto', () => {
    expect(fmtDuration(90)).toBe('1 min 30 s');
    expect(fmtDuration(125)).toBe('2 min 5 s');
  });

  it('redondea los segundos fraccionarios', () => {
    expect(fmtDuration(44.6)).toBe('45 s');
    expect(fmtDuration(59.4)).toBe('59 s');
  });
});

describe('formatTimestamp', () => {
  it('formatea una fecha ISO', () => {
    const out = formatTimestamp('2026-11-07T11:36:12');
    expect(out).toMatch(/07\/11\/2026/);
    expect(out).toMatch(/11:36/);
  });

  it('devuelve el texto original si no es una fecha válida', () => {
    expect(formatTimestamp('no es una fecha')).toBe('no es una fecha');
    expect(formatTimestamp('')).toBe('');
  });
});

describe('tierLabel', () => {
  it('traduce los tiers de ProtonDB al español', () => {
    expect(tierLabel('platinum')).toBe('Platino');
    expect(tierLabel('gold')).toBe('Oro');
    expect(tierLabel('silver')).toBe('Plata');
    expect(tierLabel('bronze')).toBe('Bronce');
    expect(tierLabel('borked')).toBe('No funciona');
  });

  it('no distingue mayúsculas', () => {
    expect(tierLabel('Borked')).toBe('No funciona');
  });

  it('deja pasar un tier desconocido y los vacíos', () => {
    expect(tierLabel('pending')).toBe('pending');
    expect(tierLabel(null)).toBe('');
    expect(tierLabel(undefined)).toBe('');
  });
});

describe('tierDescription', () => {
  it('explica el tier en lenguaje llano', () => {
    expect(tierDescription('borked')).toContain('No arranca');
    expect(tierDescription('platinum')).toContain('sin tocar nada');
  });

  it('sin tier conocido no inventa texto', () => {
    expect(tierDescription('pending')).toBeNull();
    expect(tierDescription(undefined)).toBeNull();
  });
});

describe('matchesSearch', () => {
  it('encuentra sin acentos en el nombre', () => {
    expect(matchesSearch('Pokémon GO', 'pokemon')).toBe(true);
    expect(matchesSearch('Pokémon GO', 'POKÉMON')).toBe(true);
  });

  it('encuentra sin acentos en la búsqueda', () => {
    expect(matchesSearch('Pokemon GO', 'pokémon')).toBe(true);
    expect(matchesSearch('Assassin’s Creed', 'creed')).toBe(true);
  });

  it('una búsqueda vacía no filtra nada', () => {
    expect(matchesSearch('Celeste', '')).toBe(true);
    expect(matchesSearch('Celeste', '   ')).toBe(true);
  });

  it('no inventa coincidencias', () => {
    expect(matchesSearch('Celeste', 'hollow')).toBe(false);
  });

  it('no confunde números de la saga', () => {
    // «Fallout 4» no debe traer «Fallout 76».
    expect(matchesSearch('Fallout 76', 'fallout 4')).toBe(false);
  });
});
