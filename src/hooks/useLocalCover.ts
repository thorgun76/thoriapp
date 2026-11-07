import { useEffect, useState } from 'react';
import type { Game } from '../lib/types';
import { api } from '../lib/api';

/**
 * Caché en memoria de las carátulas locales.
 *
 * Cada carátula viaja como data URI base64 (~40 KB por imagen), así que se pide
 * una sola vez por juego. La clave incluye launcher y clave de carátula para que
 * dos juegos distintos nunca compartan entrada. Se guardan también los `null`
 * (el launcher no tiene carátula) para no repetir la consulta al hacer scroll.
 */
const cache = new Map<string, string | null>();

/** Peticiones en curso, para que dos tarjetas del mismo juego no pidan dos veces. */
const inFlight = new Map<string, Promise<string | null>>();

/**
 * Se incrementa al vaciar la caché. Va en la dependencia del efecto para que
 * tras un re-escaneo las tarjetas ya montadas vuelvan a consultar el disco
 * (la clave del juego no cambia, así que sin esto no se refrescarían).
 */
let cacheVersion = 0;

const cacheKey = (game: Game) =>
  `${game.launcher ?? 'steam'}:${game.cover_key ?? game.appid ?? game.name}`;

/** Vaciado para el botón "Re-escanear": el usuario pudo cambiar una portada. */
export function clearCoverCache() {
  cache.clear();
  inFlight.clear();
  cacheVersion += 1;
}

/**
 * Carátula que guarda en disco el launcher del juego (Lutris, Heroic, Heronite).
 * Devuelve `undefined` mientras carga y `null` si el launcher no tiene ninguna.
 */
export function useLocalCover(game: Game | null | undefined): string | null | undefined {
  const key = game ? cacheKey(game) : '';

  const [state, setState] = useState<{ key: string; uri: string | null } | null>(() =>
    key && cache.has(key) ? { key, uri: cache.get(key) ?? null } : null
  );
  const [version, setVersion] = useState(cacheVersion);

  // Un `clearCoverCache` deja las versiones desincronizadas. Se sincroniza
  // durante el render (no en el efecto) para que el efecto vuelva a ejecutarse
  // en el acto: si no, la dependencia ya tendría el valor nuevo.
  if (version !== cacheVersion) setVersion(cacheVersion);

  useEffect(() => {
    if (!game || !key) return;

    // Ya resuelto (y no es de otra tarjeta): no se vuelve a consultar.
    if (cache.has(key)) {
      setState({ key, uri: cache.get(key) ?? null });
      return;
    }

    let cancelled = false;
    const request =
      inFlight.get(key) ??
      api
        // El runner importa en Heronite: indexa su arte por `<runner>:<appName>`.
        .getLocalCover(game.launcher ?? 'steam', game.appid, game.cover_key, game.runner)
        .finally(() => inFlight.delete(key));
    inFlight.set(key, request);

    request
      .then((uri) => {
        if (cancelled) return;
        cache.set(key, uri);
        setState({ key, uri });
      })
      .catch(() => {
        // Un fallo leyendo disco no debe romper la tarjeta: se cae al siguiente
        // origen de imagen (CDN de Steam o icono de mando).
        if (cancelled) return;
        cache.set(key, null);
        setState({ key, uri: null });
      });

    return () => {
      cancelled = true;
    };
    // `game` se recrea en cada render; la identidad real es `key`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version]);

  if (!key) return null;
  // Si el juego cambió y aún no llegó su respuesta, no se muestra la anterior.
  if (state?.key !== key) return cache.has(key) ? cache.get(key) : undefined;
  return state.uri;
}
