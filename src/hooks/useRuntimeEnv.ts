import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { RuntimeEnv } from '../lib/types';

/**
 * Estado real de la máquina (wrappers instalados, capa Vulkan de MangoHud, si Steam es
 * Flatpak…). Se pide UNA vez por sesión y se comparte: no cambia mientras la app está
 * abierta y no tiene sentido repetir el escaneo en cada vista.
 */
let cachedRequest: Promise<RuntimeEnv | null> | null = null;

function requestEnv(): Promise<RuntimeEnv | null> {
  cachedRequest ??= api.getRuntimeEnv().catch((e) => {
    // Sin estos datos la app funciona igual: simplemente no muestra avisos previos.
    console.error('No se pudo leer el entorno del sistema:', e);
    return null;
  });
  return cachedRequest;
}

/** Devuelve `null` mientras carga o si el backend no responde. */
export function useRuntimeEnv(): RuntimeEnv | null {
  const [env, setEnv] = useState<RuntimeEnv | null>(null);

  useEffect(() => {
    let cancelled = false;
    requestEnv().then((value) => {
      if (!cancelled) setEnv(value);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return env;
}

/** Reinicia la caché. Solo para tests. */
export function __resetRuntimeEnvCache() {
  cachedRequest = null;
}
