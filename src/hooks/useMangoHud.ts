import { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';
import { fmtDuration } from '../lib/format';

/**
 * Detecta si MangoHud está instalado y gestiona cuánto dura cada grabación.
 *
 * La duración vive en la configuración de MangoHud, no en la app, así que se
 * escribe en el backend y se relee al arrancar (el usuario puede haberla cambiado
 * fuera).
 */
export function useMangoHud(onNotify?: (message: string) => void) {
  const [installed, setInstalled] = useState<boolean | null>(null);
  const [duration, setDuration] = useState<number | null>(null);

  // ¿Está instalado? `null` = todavía comprobando.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const ok = await api.checkMangohudInstalled();
        if (!cancelled) setInstalled(ok);
      } catch (e) {
        console.error('Error detectando MangoHud:', e);
        if (!cancelled) setInstalled(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Duración actual, leída de la configuración de MangoHud
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const seconds = await api.getMangohudLogDuration();
        if (!cancelled) setDuration(seconds);
      } catch (e) {
        console.error('Error leyendo la duración de MangoHud:', e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  /**
   * Cambia la duración. Si el backend falla se revierte al valor anterior para
   * que la interfaz no muestre algo que no se guardó.
   */
  const changeDuration = useCallback(
    async (seconds: number) => {
      const previous = duration;
      setDuration(seconds);
      try {
        await api.setMangohudLogDuration(seconds);
        onNotify?.(`✓ Ahora cada grabación dura ${fmtDuration(seconds)}`);
      } catch (e) {
        setDuration(previous);
        onNotify?.(`❌ ${e}`);
      }
    },
    [duration, onNotify]
  );

  /** Detecta la ruta del log desde la configuración de MangoHud. */
  const detectLogPath = useCallback(async (): Promise<string | null> => {
    try {
      return await api.detectMangohudLogPath();
    } catch (e) {
      onNotify?.(`❌ Error detectando ruta: ${e}`);
      return null;
    }
  }, [onNotify]);

  return { installed, duration, changeDuration, detectLogPath };
}
