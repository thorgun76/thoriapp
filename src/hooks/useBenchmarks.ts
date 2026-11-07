import { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { Benchmark } from '../lib/types';

/** Nombre normalizado para comparar juegos sin distinguir extensión ni mayúsculas. */
const normName = (n: string) => n.toLowerCase().replace(/\.(x86_64|x86|exe|bin)$/, '').trim();

interface UseBenchmarksOptions {
  currentAppid?: number;
  currentGameName?: string;
  currentLaunchCommand?: string;
  onNotify?: (message: string) => void;
}

/**
 * Las grabaciones de MangoHud: cargarlas, analizar un log nuevo y borrarlas.
 *
 * El backend guarda como máximo una prueba inicial y una final, así que aquí solo se
 * mantiene la lista tal cual llega de Rust.
 */
export function useBenchmarks({
  currentAppid,
  currentGameName,
  currentLaunchCommand,
  onNotify,
}: UseBenchmarksOptions) {
  const [benchmarks, setBenchmarks] = useState<Benchmark[]>([]);
  const [analyzing, setAnalyzing] = useState(false);

  const load = useCallback(async () => {
    try {
      const list = currentAppid
        ? await api.loadBenchmarksForGame(currentAppid)
        : await api.loadBenchmarks();
      setBenchmarks(list);
    } catch (e) {
      console.error('Error al cargar benchmarks:', e);
    }
  }, [currentAppid]);

  useEffect(() => {
    load();
  }, [load]);

  /**
   * Analiza el log de MangoHud y lo guarda.
   * Devuelve `false` si había un duplicado o hubo error, para que la vista sepa
   * si limpiar el campo de notas.
   */
  const analyze = useCallback(
    async (logPath: string, notes: string): Promise<boolean> => {
      if (!logPath.trim()) {
        onNotify?.('Ingresa la ruta del log de MangoHud');
        return false;
      }

      setAnalyzing(true);
      try {
        const benchmark = await api.analyzeMangohudLog(logPath);

        // Dos grabaciones con las mismas métricas y el mismo timestamp son el
        // mismo log analizado dos veces.
        const duplicate = benchmarks.some(
          (b) =>
            b.avg_fps === benchmark.avg_fps &&
            b.fps_1_percent_low === benchmark.fps_1_percent_low &&
            b.fps_0_1_percent_low === benchmark.fps_0_1_percent_low &&
            b.total_frames === benchmark.total_frames &&
            b.timestamp === benchmark.timestamp
        );
        if (duplicate) {
          onNotify?.('⚠️ Ya existe un benchmark con estos valores (posible duplicado)');
          return false;
        }

        // El log a veces no trae el nombre del juego: se usa el que tiene abierto la app.
        let gameName = benchmark.game_name;
        const knownName = currentGameName && currentGameName !== 'Desconocido';
        if ((!gameName || gameName === 'Desconocido') && knownName) {
          gameName = currentGameName;
        }

        await api.saveBenchmark({
          benchmark,
          profileId: '',
          gameName,
          appid: currentAppid || 0,
          launchCommand: currentLaunchCommand || '',
          notes: notes || null,
        });

        // Mismo criterio que el backend: la primera es la inicial y la siguiente la
        // final (las nuevas reemplazan a la final). Otro juego = prueba nueva.
        const first = benchmarks[0];
        const newAppid = currentAppid || 0;
        const sameGame = first
          ? first.appid && newAppid
            ? first.appid === newAppid
            : normName(first.game_name) === normName(gameName)
          : true;

        if (!first || !sameGame) {
          onNotify?.(
            first
              ? `✓ Juego distinto: empezamos una prueba nueva. Guardada como prueba inicial (${gameName})`
              : `✓ Guardada como prueba inicial (${gameName}). Cambia algo y graba otra vez para comparar`
          );
        } else if (benchmarks.length >= 2) {
          onNotify?.('✓ Guardada como nueva prueba final (reemplazó a la anterior)');
        } else {
          onNotify?.(`✓ Guardada como prueba final (${gameName})`);
        }

        await load();
        return true;
      } catch (e) {
        onNotify?.(`❌ Error: ${e}`);
        return false;
      } finally {
        setAnalyzing(false);
      }
    },
    [benchmarks, currentAppid, currentGameName, currentLaunchCommand, load, onNotify]
  );

  const remove = useCallback(
    async (id: string) => {
      try {
        await api.deleteBenchmark(id);
        onNotify?.('✓ Benchmark eliminado');
        await load();
      } catch (e) {
        onNotify?.(`❌ Error al eliminar: ${e}`);
      }
    },
    [load, onNotify]
  );

  const removeAll = useCallback(async () => {
    try {
      await api.deleteAllBenchmarks();
      onNotify?.(`✓ Se eliminaron ${benchmarks.length} benchmarks`);
      await load();
    } catch (e) {
      onNotify?.(`❌ Error al eliminar: ${e}`);
    }
  }, [benchmarks.length, load, onNotify]);

  return { benchmarks, analyzing, load, analyze, remove, removeAll };
}
