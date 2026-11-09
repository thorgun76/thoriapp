import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { ShaderCache } from '../lib/types';

/**
 * Caché de shaders del juego actual en el disco.
 *
 * Es una consulta que recorre directorios de Steam, así que se hace una vez por juego y
 * se guarda por sesión: no cambia mientras la app está abierta (solo cambia al jugar).
 * Si falla, devuelve `null` y la tarjeta simplemente no habla de la caché.
 */
let cache: Map<number, ShaderCache | null> | null = null;

/** Solo para tests: limpia la caché en memoria entre casos. */
export function __resetShaderCache() {
  cache = null;
}

export function useShaderCache(appid: number | undefined): ShaderCache | null {
  const [data, setData] = useState<ShaderCache | null>(() =>
    appid && cache?.has(appid) ? cache.get(appid)! : null
  );

  useEffect(() => {
    if (!appid) {
      setData(null);
      return;
    }
    cache ??= new Map();
    if (cache.has(appid)) {
      setData(cache.get(appid)!);
      return;
    }

    let cancelled = false;
    api
      .getShaderCache(appid)
      .then((result) => {
        cache!.set(appid, result);
        if (!cancelled) setData(result);
      })
      .catch((e) => {
        console.error('No se pudo leer la caché de shaders:', e);
        cache!.set(appid, null);
        if (!cancelled) setData(null);
      });

    return () => {
      cancelled = true;
    };
  }, [appid]);

  return data;
}
