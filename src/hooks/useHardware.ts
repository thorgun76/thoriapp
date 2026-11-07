import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { Hardware } from '../lib/types';

interface UseHardwareResult {
  hw: Hardware | null;
  /** true mientras la primera detección está en curso. */
  loading: boolean;
  error: string | null;
  /** Vuelve a lanzar la detección (botón "Actualizar"). */
  refresh: () => void;
}

/**
 * Detecta el hardware una sola vez al arrancar y lo cachea.
 *
 * Antes esto vivía dentro de `loadData()` en App.tsx, encadenado a la
 * biblioteca y a los launchers. Sacarlo aquí hace explícito que el hardware
 * se detecta UNA vez y que ningún otro efecto lo puede volver a disparar.
 */
export function useHardware(): UseHardwareResult {
  const [hw, setHw] = useState<Hardware | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    api
      .getHardware()
      .then((detected) => {
        if (cancelled) return;
        setHw(detected);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        console.error('Error detectando hardware:', e);
        setError(String(e));
      })
      .finally(() => {
        if (cancelled) return;
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [reloadKey]);

  return {
    hw,
    loading,
    error,
    refresh: () => setReloadKey((k) => k + 1),
  };
}
