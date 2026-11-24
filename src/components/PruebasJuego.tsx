import { useCallback, useEffect, useState } from 'react';
import { ArrowRight, Info, X } from 'lucide-react';
import { api } from '../lib/api';
import type { Benchmark, GroupComparison } from '../lib/types';
import { useBenchmarks } from '../hooks/useBenchmarks';
import { useConfirm, useToast } from '../hooks/useConfirm';
import { ConfirmDialog } from './ConfirmDialog';
import { GroupComparisonCard } from './GroupComparisonCard';
import { RankingCard } from './RankingCard';
import { etiquetaPrueba } from '../lib/pruebaLabel';

/**
 * «Pruebas de este juego»: el historial de un juego concreto, en una ventana.
 *
 * Es la versión ligera de la pestaña Benchmarks: **solo lee y borra**. Aquí no se analiza
 * ni se captura nada — para eso está la pestaña Benchmarks, y el botón de abajo lleva
 * allí. Meter el formulario de análisis en la ficha del juego la llenaría de campos que
 * no tienen que ver con optimizar.
 *
 * Todo lo que se enseña sale del backend filtrado por `appid` (`load_benchmarks_for_game`),
 * así que nunca se mezclan juegos. Comparar 1 % lows de juegos distintos no significaría
 * nada: son valores absolutos.
 */

interface Props {
  appid: number;
  gameName: string;
  onClose: () => void;
  /**
   * Lleva a la pestaña Benchmarks con este juego abierto, que es donde se analiza y se
   * guarda una prueba nueva. No se duplica ese formulario aquí.
   */
  onMedir: () => void;
}

/**
 * Compara la mitad antigua del historial contra la mitad reciente.
 *
 * Es la misma lógica que `BenchmarksView`: una sola comparación de dos grabaciones no
 * distingue una mejora del ruido del juego, y con varias por lado el backend da un
 * intervalo de confianza. Duplicar aquí la fórmula daría dos veredictos distintos para
 * los mismos datos, así que se reutiliza el mismo reparto.
 */
function useGrupos(benchmarks: Benchmark[]): GroupComparison[] | null {
  const [rows, setRows] = useState<GroupComparison[] | null>(null);

  useEffect(() => {
    if (benchmarks.length < 2) {
      setRows(null);
      return;
    }
    const half = Math.floor(benchmarks.length / 2);
    const antes = benchmarks.slice(0, half).map((b) => b.id);
    const despues = benchmarks.slice(half).map((b) => b.id);

    let cancelled = false;
    (async () => {
      try {
        const result = await api.compareBenchmarkGroups(antes, despues);
        if (!cancelled) setRows(result);
      } catch {
        // Comparar grupos es una ayuda: si falla no se rompe la ventana.
        if (!cancelled) setRows(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [benchmarks]);

  return rows;
}

export function PruebasJuego({
  appid,
  gameName,
  onClose,
  onMedir,
}: Props) {
  const { toast, setToast } = useToast();
  const { confirmState, ask, close } = useConfirm();
  const notify = useCallback((m: string) => setToast(m), [setToast]);

  const { benchmarks, remove, removeAllForGame } = useBenchmarks({
    currentAppid: appid,
    currentGameName: gameName,
    onNotify: notify,
  });

  const grupos = useGrupos(benchmarks);

  /**
   * Copia el comando de una grabación para poder volver a ponerlo en Steam.
   *
   * Se confirma con un toast porque el portapapeles no da señal visible: sin aviso el
   * usuario no sabe si se copió.
   */
  const copiarComando = useCallback(async (texto: string) => {
    try {
      await navigator.clipboard.writeText(texto);
      setToast('✓ Comando copiado');
    } catch {
      setToast('❌ No se pudo copiar');
    }
  }, [setToast]);

  // Escape cierra: la ventana tapa la ficha del juego.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const confirmarBorrado = (b: Benchmark) =>
    ask({
      title: `¿Borrar ${etiquetaPrueba(b)}?`,
      message:
        'Se elimina esta grabación del historial de este juego. Las demás no cambian y los números no se renumeran.',
      confirmLabel: 'Borrar',
      onConfirm: () => remove(b.id),
    });

  const confirmarBorradoTotal = () =>
    ask({
      title: '¿Borrar todas las pruebas de este juego?',
      message: `Se eliminarán las ${benchmarks.length} grabaciones de ${gameName}. Las pruebas de otros juegos no se tocan.`,
      confirmLabel: 'Borrar todas',
      onConfirm: () => removeAllForGame(),
    });

  const vacio = benchmarks.length === 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="titulo-pruebas-juego"
    >
      {/* Cierra al pulsar fuera: el overlay no es accionable, solo un atajo. */}
      <div className="absolute inset-0" onClick={onClose} aria-hidden="true" />

      <div className="relative w-full max-w-3xl max-h-[85vh] flex flex-col rounded-2xl bg-[var(--bg)] border border-[var(--border)] shadow-2xl overflow-hidden">
        <header className="flex items-start justify-between gap-3 px-6 py-4 border-b border-[var(--border)] bg-[var(--surface)] shrink-0">
          <div className="min-w-0">
            <p className="text-[11px] font-bold uppercase tracking-[0.14em] text-[var(--text-dim)]">
              Historial de este juego
            </p>
            <h2 id="titulo-pruebas-juego" className="text-lg font-bold text-[var(--text)] truncate">
              {gameName}
            </h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="shrink-0 p-2 rounded-lg text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--bg)] transition-colors"
            aria-label="Cerrar"
          >
            <X className="w-5 h-5" />
          </button>
        </header>

        <div className="flex-1 overflow-y-auto p-6 space-y-5">
          {vacio ? (
            <div className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
              <p className="flex items-center gap-2 text-sm font-semibold text-[var(--text)] mb-1">
                <Info className="w-4 h-4 text-[#38bdf8]" />
                Todavía no hay pruebas de {gameName}
              </p>
              <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                Graba con <strong>Shift+F2</strong> dentro del juego, cierra la grabación y
                la app la analizará y la guardará sola. Con la segunda ya se puede comparar
                cuál va mejor.
              </p>
            </div>
          ) : (
            <>
              <RankingCard
                rows={benchmarks}
                onDelete={confirmarBorrado}
                onDeleteAll={confirmarBorradoTotal}
                onCopy={copiarComando}
              />
              {grupos && grupos.some((r) => r.antes_runs + r.despues_runs > 0) && (
                <GroupComparisonCard rows={grupos} />
              )}
            </>
          )}
        </div>

        <footer className="px-6 py-4 border-t border-[var(--border)] bg-[var(--surface)] shrink-0 flex items-center justify-between gap-3">
          <p className="text-[11px] text-[var(--text-dim)] leading-relaxed">
            El detalle de cada prueba está en «Mis pruebas».
          </p>
          <button
            type="button"
            onClick={onMedir}
            className="shrink-0 flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors"
          >
            {vacio ? 'Medir la primera' : 'Medir una prueba nueva'}
            <ArrowRight className="w-4 h-4" />
          </button>
        </footer>
      </div>

      {confirmState && <ConfirmDialog state={confirmState} onClose={close} />}
      {toast && (
        <div className="fixed bottom-6 right-6 px-4 py-2.5 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-xl text-sm font-semibold text-white z-[60]">
          {toast}
        </div>
      )}
    </div>
  );
}
