import { describe, expect, it, vi } from 'vitest';
import { matchAppId, normalizeName, resolveAppId, searchTerms } from './appid';

describe('normalizeName', () => {
  it('ignora mayúsculas, acentos y signos', () => {
    expect(normalizeName('  Pokémon™  Rojo: Fuego ')).toBe('pokemon rojo fuego');
    expect(normalizeName('Cyberpunk 2077 – Ultimate')).toBe('cyberpunk 2077 ultimate');
    expect(normalizeName('Cyberpunk 2077: Ultimate')).toBe('cyberpunk 2077 ultimate');
  });

  it('trata el apóstrofe recto, el tipográfico y ninguno como el mismo nombre', () => {
    // Lutris, Steam y el usuario escriben el apóstrofe de tres formas distintas.
    expect(normalizeName("No Man's Sky")).toBe('no mans sky');
    expect(normalizeName('No Man’s Sky')).toBe('no mans sky');
    expect(normalizeName('No Mans Sky')).toBe('no mans sky');
  });
});

describe('matchAppId', () => {
  // Lo que devuelve la tienda buscando el nombre completo.
  const exact = [{ appid: 4348910, name: 'Probably Stolen - Cyberpunk Shopkeeper Sim' }];

  it('resuelve el AppID cuando el nombre coincide exactamente', () => {
    expect(matchAppId(exact, 'Probably Stolen - Cyberpunk Shopkeeper Sim')).toBe(4348910);
    // Da igual cómo venga escrito: se compara normalizado.
    expect(matchAppId(exact, 'probably stolen   cyberpunk shopkeeper sim')).toBe(4348910);
  });

  it('acepta un prefijo por palabras cuando es único', () => {
    // Lutris guarda a veces solo el nombre corto.
    expect(matchAppId(exact, 'Probably Stolen')).toBe(4348910);
    // O al revés: la biblioteca tiene el nombre largo y la tienda el corto.
    expect(matchAppId([{ appid: 7, name: 'Portal 2' }], 'Portal 2 - Cooperative')).toBe(7);
  });

  it('no empareja por letras sueltas: «Stolen» no es «Stolen Realm»', () => {
    // Regresión del bug real: la búsqueda difusa de Steam devuelve varios y
    // coger el primero enseñaba el nivel de ProtonDB de otro juego.
    expect(matchAppId([{ appid: 1, name: 'Probably Stolen Realm' }], 'Probably Stolen')).toBeNull();
  });

  it('ante dos candidatas que encajan por separador no elige', () => {
    const ambiguous = [
      { appid: 1, name: 'Probably Stolen - Cyberpunk Shopkeeper Sim' },
      { appid: 2, name: 'Probably Stolen - Soundtrack' },
    ];
    expect(matchAppId(ambiguous, 'Probably Stolen')).toBeNull();
  });

  it('descarta la banda sonora y se queda con el juego', () => {
    // "Probably Stolen Soundtrack" no cae en un separador, así que no compite.
    const hits = [
      { appid: 4348910, name: 'Probably Stolen - Cyberpunk Shopkeeper Sim' },
      { appid: 5113870, name: 'Probably Stolen Soundtrack' },
    ];
    expect(matchAppId(hits, 'Probably Stolen')).toBe(4348910);
  });

  it('sin resultados o sin nombre no inventa nada', () => {
    expect(matchAppId([], 'Cualquier cosa')).toBeNull();
    expect(matchAppId(exact, '   ')).toBeNull();
  });
});

describe('searchTerms', () => {
  it('prueba el nombre completo y luego las primeras palabras', () => {
    // La puntuación se descarta: la tienda indexa palabras, no " - ".
    expect(searchTerms('Probably Stolen - Cyberpunk Shopkeeper Sim')).toEqual([
      'Probably Stolen Cyberpunk Shopkeeper Sim',
      'Probably Stolen Cyberpunk Shopkeeper',
      'Probably Stolen Cyberpunk',
    ]);
  });

  it('suelta los adornos que la tienda no conoce', () => {
    expect(searchTerms('Portal 2 (GOG)')).toEqual(['Portal 2']);
    expect(searchTerms('Celeste [FitGirl Repack]')).toEqual(['Celeste']);
  });

  it('no repite términos con nombres cortos', () => {
    expect(searchTerms('Portal 2')).toEqual(['Portal 2']);
  });
});

describe('resolveAppId', () => {
  const game = { appid: 4348910, name: 'Probably Stolen - Cyberpunk Shopkeeper Sim' };

  it('usa el primer término que da una coincidencia', async () => {
    const search = vi.fn().mockResolvedValue([game]);
    const r = await resolveAppId(game.name, search);

    expect(r.appid).toBe(4348910);
    expect(search).toHaveBeenCalledTimes(1);
  });

  it('reintenta con menos palabras si el nombre completo no da resultados', async () => {
    const search = vi
      .fn()
      .mockResolvedValueOnce([]) // "… Sim (GOG)": la tienda no conoce el adorno
      .mockResolvedValue([game]);

    const r = await resolveAppId('Probably Stolen - Cyberpunk Shopkeeper Sim (GOG)', search);

    expect(r.appid).toBe(4348910);
    expect(search).toHaveBeenCalledTimes(2);
    expect(r.tried).toHaveLength(2);
  });

  it('no insiste si la red falla', async () => {
    const search = vi.fn().mockRejectedValue('sin red');
    const r = await resolveAppId(game.name, search);

    expect(r.appid).toBeNull();
    // Un solo intento: reintentar con otro término no arregla una caída de red.
    expect(search).toHaveBeenCalledTimes(1);
  });
});

// ==================== NOMBRE TRUNCADO (bug real) ====================
// Lutris guardó el nombre cortado por la mitad: «Probably Stolen Cyberpunk
// Shopkeeper S». La tienda tiene «Probably Stolen - Cyberpunk Shopkeeper Sim».
describe('matchAppId: nombres truncados', () => {
  const hits = [{ appid: 4348910, name: 'Probably Stolen - Cyberpunk Shopkeeper Sim' }];

  it('empareja un nombre cortado a mitad de palabra', () => {
    expect(matchAppId(hits, 'Probably Stolen Cyberpunk Shopkeeper S')).toBe(4348910);
  });

  it('no confunde un título más corto con otro distinto', () => {
    // «Probably Stolen» es el 71 % de «Probably Stolen Realm»: no llega al umbral.
    expect(matchAppId([{ appid: 1, name: 'Probably Stolen Realm' }], 'Probably Stolen')).toBeNull();
  });

  it('los resultados repetidos del mismo AppID no cuentan como ambigüedad', () => {
    // La tienda devolvió tres veces el mismo juego y antes eso anulaba el emparejamiento.
    const repeated = [hits[0], { ...hits[0] }, { ...hits[0] }];
    expect(matchAppId(repeated, 'Probably Stolen - Cyberpunk Shopkeeper Sim')).toBe(4348910);
  });
});
