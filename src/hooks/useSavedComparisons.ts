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

  const load = useCallback(async () => {
    try {
      const list = await api.loadComparisons();
      setItems(list);
    } catch (e) {
      console.error('Error al cargar las pruebas guardadas:', e);
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

  return { items, loading, load, remove, removeAll };
}
