import { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { BenchmarkComparison } from '../lib/types';
import { COMPARISONS_CHANGED_EVENT, notifyComparisonsChanged } from './useSavedComparisons';

/** Clave única de un par inicial/final, para saber si ya está guardado. */
const keyOf = (c: BenchmarkComparison) => `${c.baseline.id}|${c.test.id}`;

/**
 * Compara automáticamente la prueba inicial con la final en cuanto existen los dos, y
 * lleva la cuenta de qué comparaciones están guardadas en "Mis pruebas".
 */
export function useComparison(antesId?: string, despuesId?: string, onNotify?: (m: string) => void) {
  const [comparison, setComparison] = useState<BenchmarkComparison | null>(null);
  const [savedKeys, setSavedKeys] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  // Comparación automática en cuanto hay una prueba inicial y una final.
  // El flag `cancelled` evita que una respuesta lenta pise a otra más reciente.
  useEffect(() => {
    if (!antesId || !despuesId) {
      setComparison(null);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const result = await api.compareBenchmarks(antesId, despuesId);
        if (!cancelled) setComparison(result);
      } catch (e) {
        if (!cancelled) onNotify?.(`❌ Error al comparar: ${e}`);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [antesId, despuesId, onNotify]);

  const refreshSaved = useCallback(async () => {
    try {
      const list = await api.loadComparisons();
      setSavedKeys(list.map((item) => keyOf(item.comparison)));
    } catch (e) {
      console.error('Error al leer las pruebas guardadas:', e);
    }
  }, []);

  // Al borrar algo desde "Mis pruebas" hay que refrescar para que el botón de
  // guardar vuelva a estar disponible.
  useEffect(() => {
    refreshSaved();
    const onChanged = () => {
      refreshSaved();
    };
    window.addEventListener(COMPARISONS_CHANGED_EVENT, onChanged);
    return () => window.removeEventListener(COMPARISONS_CHANGED_EVENT, onChanged);
  }, [refreshSaved]);

  const isSaved = comparison ? savedKeys.includes(keyOf(comparison)) : false;

  const save = useCallback(async () => {
    if (!comparison || isSaved) return;
    setSaving(true);
    try {
      await api.saveComparison(comparison.baseline.id, comparison.test.id);
      onNotify?.('✓ Guardada en Mis pruebas');
      await refreshSaved();
      notifyComparisonsChanged();
    } catch (e) {
      onNotify?.(`❌ ${e}`);
    } finally {
      setSaving(false);
    }
  }, [comparison, isSaved, onNotify, refreshSaved]);

  /** Oculta el resultado (el usuario cierra la tarjeta). */
  const clear = useCallback(() => setComparison(null), []);

  return { comparison, isSaved, saving, save, clear };
}
