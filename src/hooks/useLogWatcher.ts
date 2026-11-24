import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import type { FinishedLog, RecordedSetup } from '../lib/types';

/** Cada cuánto se mira la carpeta de logs mientras hay un juego abierto. */
export const POLL_MS = 5000;

interface UseLogWatcherOptions {
  /** Carpeta donde MangoHud escribe los CSV. */
  folder: string | null;
  /** Se llama con cada grabación nueva ya analizada y guardada. */
  onFound?: (log: FinishedLog) => void;
  /** Se llama si el análisis o el guardado fallan, con el motivo. */
  onError?: (message: string) => void;
  /**
   * Habilita el sondeo. Se apaga cuando el usuario está mirando otra parte de la app o
   * no hay juego abierto: no tiene sentido recorrer la carpeta cada 5 s sin motivo.
   */
  enabled?: boolean;
  /**
   * Analiza y guarda una grabación. Devuelve `true` si salió bien.
   *
   * Se inyecta (en vez de llamar a la API aquí dentro) para poder probar el hook sin
   * tocar Tauri, y para que el guardado siga viviendo en `useBenchmarks`.
   */
  process: (log: FinishedLog, setup: RecordedSetup | null) => Promise<boolean>;
  /**
   * Lee la configuración actual (comando, perfil, duración). Se llama **en el momento de
   * detectar el cambio**, no antes: el comando que se guarda con la grabación debe ser el
   * que estaba puesto mientras se jugaba.
   */
  captureSetup?: () => RecordedSetup | null;
}

/**
 * Vigila la carpeta de MangoHud y analiza sola las grabaciones que aparecen.
 *
 * El usuario graba como siempre: entra en la escena y pulsa `Shift+F2`. Aquí no se
 * arranca ninguna grabación ni se toca la configuración de MangoHud; solo se mira la
 * carpeta para no tener que ir a buscar el fichero a mano.
 *
 * Una grabación solo se da por terminada cuando MangoHud ha escrito su `_summary.csv`,
 * que ocurre 1 ms después de cerrar el CSV de datos. Los CSV sin summary (grabaciones que
 * se cortaron al cerrar el juego) se ignoran a propósito.
 *
 * Solo se procesa lo que el backend presenta como **nuevo**: el registro de "ya visto" se
 * lleva en disco, así que reiniciar la app no vuelve a analizar ni a duplicar el
 * historial. Y solo se marca como procesado lo que se guardó bien, para no perder una
 * grabación si el análisis falla.
 */
export function useLogWatcher({
  folder,
  onFound,
  onError,
  enabled = true,
  process,
  captureSetup,
}: UseLogWatcherOptions) {
  const [lastFound, setLastFound] = useState<FinishedLog | null>(null);
  const [checking, setChecking] = useState(false);

  // En una ref para que el temporizador no se reinicie en cada render ni capture una
  // versión vieja del callback.
  const processRef = useRef(process);
  processRef.current = process;
  const onFoundRef = useRef(onFound);
  onFoundRef.current = onFound;
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;
  const captureSetupRef = useRef(captureSetup);
  captureSetupRef.current = captureSetup;

  // Evita dos pasadas solapadas si una tarda más que el intervalo.
  const busy = useRef(false);
  // Grabaciones que ya se intentaron y no se pudieron guardar en esta sesión.
  const failedRef = useRef<Set<string>>(new Set());
  // Las de `failedRef` que ya se avisaron, para no repetir el aviso.
  const yaAvisadoRef = useRef<Set<string>>(new Set());

  const checkNow = useCallback(async (): Promise<FinishedLog[]> => {
    if (!folder || busy.current) return [];
    busy.current = true;
    setChecking(true);
    const procesadas: FinishedLog[] = [];
    try {
      const nuevas = await api.scanNewMangohudLogs(folder);
      // Se descartan las que ya se intentaron sin éxito en esta misma sesión. Sin esto,
      // una grabación que no se puede guardar se reintentaría cada 5 s para siempre: el
      // aviso aparecería en bucle y la pantalla se movería sola.
      const pendientes = nuevas.filter((l) => !failedRef.current.has(l.path));
      // Una sola foto por pasada: todas las grabaciones recogidas ahora comparten el
      // estado actual de la app.
      const setup = captureSetupRef.current?.() ?? null;
      for (const log of pendientes) {
        const ok = await processRef.current(log, setup);
        if (ok) {
          // Solo se marca lo que se guardó: si falló, se olvida hasta la próxima sesión.
          failedRef.current.delete(log.path);
          await api.markLogsProcessed([log.path]);
          procesadas.push(log);
          setLastFound(log);
          onFoundRef.current?.(log);
        } else {
          failedRef.current.add(log.path);
        }
      }
      // Un único aviso por pasada, no uno por fichero fallido.
      if (pendientes.length > 0 && procesadas.length === 0) {
        const nuevos = pendientes.filter((l) => !yaAvisadoRef.current.has(l.path));
        if (nuevos.length > 0) {
          for (const l of nuevos) yaAvisadoRef.current.add(l.path);
          onErrorRef.current?.(
            nuevos.length === 1
              ? `No se pudo guardar "${nuevos[0].program} · ${nuevos[0].timestamp}". No se volverá a intentar hasta que reinicies la app.`
              : `No se pudieron guardar ${nuevos.length} grabaciones. No se volverá a intentarlo hasta que reinicies la app.`
          );
        }
      }
    } catch (e) {
      onErrorRef.current?.(`No se pudo revisar la carpeta de grabaciones: ${e}`);
    } finally {
      busy.current = false;
      setChecking(false);
    }
    return procesadas;
  }, [folder]);

  useEffect(() => {
    if (!folder || !enabled) return;
    // Una pasada al empezar: si grabas y luego abres la app, la recoge sin esperar.
    void checkNow();
    const id = setInterval(() => void checkNow(), POLL_MS);
    return () => clearInterval(id);
  }, [folder, enabled, checkNow]);

  return { checkNow, lastFound, checking };
}
