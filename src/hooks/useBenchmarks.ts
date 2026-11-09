import { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { Benchmark } from '../lib/types';

/** Grabaciones que se conservan de cada juego. Debe coincidir con el backend. */
export const MAX_BENCHMARKS_PER_GAME = 10;

interface UseBenchmarksOptions {
  currentAppid?: number;
  currentGameName?: string;
  currentLaunchCommand?: string;
  onNotify?: (message: string) => void;
}

/**
 * Las grabaciones de MangoHud: cargarlas, analizar un log nuevo y borrarlas.
 *
 * El backend conserva las últimas MAX_BENCHMARKS_PER_GAME grabaciones de cada juego, así
 * que aquí solo se mantiene la lista tal cual llega de Rust (de la más antigua a la más
 * reciente). El recorte no se replica aquí: si algún día cambia en Rust, cambia solo allí.
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
   * `warmupSeconds` descarta los primeros segundos de la grabación (arranque y carga).
   * Devuelve `false` si había un duplicado o hubo error, para que la vista sepa
   * si limpiar el campo de notas.
   */
  const analyze = useCallback(
    async (logPath: string, notes: string, warmupSeconds: number): Promise<boolean> => {
      if (!logPath.trim()) {
        onNotify?.('Ingresa la ruta del log de MangoHud');
        return false;
      }

      setAnalyzing(true);
      try {
        const benchmark = await api.analyzeMangohudLog(logPath, warmupSeconds);

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

        // El backend conserva las últimas MAX_BENCHMARKS_PER_GAME grabaciones de cada
        // juego; el mensaje solo explica en qué punto de la serie queda esta.
        const count = benchmarks.length;
        if (count === 0) {
          onNotify?.(`✓ Guardada como punto de partida (${gameName}). Cambia algo y graba otra vez para comparar`);
        } else if (count >= MAX_BENCHMARKS_PER_GAME) {
          onNotify?.(`✓ Guardada. Ya tenías ${MAX_BENCHMARKS_PER_GAME}: se borró la más antigua de ${gameName}`);
        } else {
          onNotify?.(`✓ Guardada (${count + 1} de ${MAX_BENCHMARKS_PER_GAME}). Se compara sola con la anterior`);
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
