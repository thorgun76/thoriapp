import { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { SavedComparison } from '../lib/api';

export type { SavedComparison } from '../lib/api';

// Aviso entre pantallas: Pruebas y Mis pruebas se actualizan cuando cambia la lista.
export const COMPARISONS_CHANGED_EVENT = 'thoriapp:comparisons-changed';

/** Avisa a las demás pantallas de que la lista de pruebas guardadas cambió. */
export const notifyComparisonsChanged = () =>
  window.dispatchEvent(new Event(COMPARISONS_CHANGED_EVENT));

/**
 * "Mis pruebas": la lista de comparaciones que el usuario decidió guardar.
 *
 * Se recarga sola cuando otra pantalla guarda o borra algo, para que las dos
 * vistas nunca muestren estados distintos.
 */
export function useSavedComparisons(onNotify?: (message: string) => void) {
  const [items, setItems] = useState<SavedComparison[]>([]);
  const [loading, setLoading] = useState(true);
  /**
   * Por qué no se pudieron leer las pruebas guardadas.
   *
   * Distingue «no tienes ninguna» de «no se pudo leer el archivo». Sin esto, un
   * `saved_comparisons.json` dañado se pintaba como una pantalla vacía y el usuario
   * creía haber perdido sus pruebas sin motivo.
   */
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const list = await api.loadComparisons();
      setItems(list);
      setError(null);
    } catch (e) {
      console.error('Error al cargar las pruebas guardadas:', e);
      setError(String(e));
      // Se dejan los items anteriores: mostrar una lista vacía aquí afirmaría que no
      // hay ninguna, que es justo lo contrario de lo que sabemos.
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
    const onChanged = () => {
      load();
    };
    window.addEventListener(COMPARISONS_CHANGED_EVENT, onChanged);
    return () => window.removeEventListener(COMPARISONS_CHANGED_EVENT, onChanged);
  }, [load]);

  const remove = useCallback(
    async (id: string) => {
      try {
        await api.deleteComparison(id);
        onNotify?.('✓ Prueba eliminada');
        notifyComparisonsChanged();
      } catch (e) {
        onNotify?.(`❌ Error al eliminar: ${e}`);
      }
    },
    [onNotify]
  );

  const removeAll = useCallback(async () => {
    try {
      await api.deleteAllComparisons();
      onNotify?.('✓ Se eliminaron todas las pruebas');
      notifyComparisonsChanged();
    } catch (e) {
      onNotify?.(`❌ Error al eliminar: ${e}`);
    }
  }, [onNotify]);

  return { items, loading, load, remove, removeAll, error };
}
