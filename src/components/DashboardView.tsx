import { useState } from 'react';
import { motion } from 'framer-motion';
import { BarChart3, Trash2, ChevronRight } from 'lucide-react';
import { interpretar, nombreLegible, etiquetasDe, titularDelCambio } from '../lib/interpretacion';
import { formatTimestamp } from '../lib/format';
import { PruebaN } from './PruebaN';
import { SavedComparisonDetail } from './SavedComparisonDetail';
import type { SavedComparison } from '../hooks/useSavedComparisons';
import { useSavedComparisons } from '../hooks/useSavedComparisons';
import { useConfirm, useToast } from '../hooks/useConfirm';
import { ConfirmDialog, Toast } from './ConfirmDialog';

// "2026-11-03 10:47:42" -> formato ISO, que todos los motores web entienden
const parseDate = (s: string) => new Date(s.replace(' ', 'T'));

// La fecha de la prueba es la de la grabación nueva (el "después"), no la de cuando se guardó.
const testDateOf = (c: SavedComparison) => c.comparison.test.timestamp;

const normGame = (n: string) => n.trim().toLowerCase();

/** Color del título del resultado. Mismo vocabulario que el resto de la app. */
const ESTADO_TEXTO: Record<ReturnType<typeof interpretar>['estado'], string> = {
  mejora: 'text-[var(--compatible)]',
  empeora: 'text-[var(--error)]',
  mixto: 'text-[var(--caution)]',
  sin_cambio: 'text-[var(--text-muted)]',
  datos_insuficientes: 'text-[var(--text-dim)]',
};

/** «Mis pruebas»: las comparaciones guardadas, de la más reciente a la más antigua. */
export function DashboardView() {
  const { toast, setToast } = useToast();
  const { confirmState, ask, close } = useConfirm();
  const { items, loading, remove, removeAll } = useSavedComparisons(setToast);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const copyText = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setToast('✓ Copiado al portapapeles');
    } catch {
      setToast('❌ No se pudo copiar');
    }
  };

  const deleteOne = async (id: string) => {
    await remove(id);
    if (selectedId === id) setSelectedId(null);
  };

  const deleteAll = async () => {
    await removeAll();
    setSelectedId(null);
  };

  const askDeleteOne = (c: SavedComparison) =>
    ask({
      title: 'Eliminar prueba',
      message: `Se eliminará la prueba de ${c.game_name || 'este juego'}. Esta acción no se puede deshacer.`,
      confirmLabel: 'Eliminar',
      onConfirm: () => deleteOne(c.id),
    });

  const askDeleteAll = () =>
    ask({
      title: 'Borrar todas las pruebas',
      message: `Se eliminarán las ${items.length} pruebas guardadas. Esta acción no se puede deshacer.`,
      confirmLabel: 'Borrar todas',
      onConfirm: deleteAll,
    });

  // Fecha relativa: la de la prueba más reciente, para el subtítulo.
  const formatRelative = (dateString: string) => {
    const date = parseDate(dateString);
    const diffMs = Date.now() - date.getTime();
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (Number.isNaN(diffMs)) return dateString;
    if (diffHours < 1) return 'hace menos de 1 hora';
    if (diffHours < 24) return `hace ${diffHours} hora${diffHours > 1 ? 's' : ''}`;
    if (diffDays < 7) return `hace ${diffDays} día${diffDays > 1 ? 's' : ''}`;
    return date.toLocaleDateString('es-ES');
  };

  const sorted = [...items].sort(
    (a, b) => parseDate(testDateOf(b)).getTime() - parseDate(testDateOf(a)).getTime()
  );
  const selected = items.find((c) => c.id === selectedId) ?? null;

  // ── Cuadro de confirmación y aviso (se muestran en todas las pantallas) ──
  const overlays = (
    <>
      {toast && <Toast message={toast} />}
      {confirmState && <ConfirmDialog state={confirmState} onClose={close} />}
    </>
  );

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <div className="w-12 h-12 border-4 border-action/30 border-t-[var(--action)] rounded-full animate-spin mx-auto mb-4"></div>
          <p className="text-[var(--text-muted)]">Cargando estadísticas...</p>
        </div>
      </div>
    );
  }

  /* ───────────────────────── Detalle de una prueba ───────────────────────── */
  if (selected) {
    return (
      <>
        <SavedComparisonDetail
          comparison={selected.comparison}
          onBack={() => setSelectedId(null)}
          onDelete={() => askDeleteOne(selected)}
          onCopy={copyText}
        />
        {overlays}
      </>
    );
  }

  /* ───────────────────────── Sin pruebas guardadas ───────────────────────── */
  if (items.length === 0) {
    return (
      <div className="max-w-4xl mx-auto p-8">
        <div className="text-center py-16">
          <BarChart3 className="w-16 h-16 text-[var(--text-dim)] mx-auto mb-4" />
          <h2 className="text-2xl font-bold text-[var(--text)] mb-2">
            No hay pruebas guardadas aún
          </h2>
          <p className="text-[var(--text-muted)] mb-6">
            Aquí aparecerán las comparaciones que guardes para consultarlas cuando quieras
          </p>
          <div className="text-sm text-[var(--text-dim)]">
            <p>💡 Consejo: en la pestaña "Benchmarks" compara dos grabaciones y pulsa "Guardar en Mis pruebas"</p>
          </div>
        </div>
        {overlays}
      </div>
    );
  }

  /* ───────────────────────────── Estadísticas ───────────────────────────── */
  const idiomas = new Set(items.map((c) => normGame(c.game_name)));
  const lastTestDate = sorted.length > 0 ? testDateOf(sorted[0]) : null;

  // El subtítulo resume lo mismo que antes ocupaba tres tarjetas (pruebas, juegos y
  // última fecha) en una línea: sin cajas, sin iconos grandes y sin contadores sueltos.
  const subtitulo = [
    `${items.length} ${items.length === 1 ? 'comparación guardada' : 'comparaciones guardadas'}`,
    `${idiomas.size} ${idiomas.size === 1 ? 'juego' : 'juegos'}`,
    lastTestDate ? `última ${formatRelative(lastTestDate)}` : null,
  ]
    .filter(Boolean)
    .join(' · ');

  return (
    <div className="max-w-4xl mx-auto p-4 sm:p-8 space-y-5">
      {/* Header: título, resumen en una línea y la única acción destructiva. */}
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div className="min-w-0">
          <p className="text-xs text-[var(--text-muted)] mb-1">Dashboard</p>
          <h2 className="text-2xl font-bold text-[var(--text)]">Mis pruebas de rendimiento</h2>
          <p className="text-sm text-[var(--text-muted)] mt-1">{subtitulo}</p>
        </div>
        <button
          type="button"
          onClick={askDeleteAll}
          className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-error/10 text-[var(--error)] border border-error/30 hover:bg-error/20 hover:border-error/50 transition-all duration-200 flex items-center gap-1.5 group focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--error)]"
          title="Eliminar todas las pruebas guardadas"
        >
          <Trash2 className="w-3.5 h-3.5 group-hover:rotate-12 transition-transform" aria-hidden="true" />
          Borrar todas las pruebas
        </button>
      </div>

      {/* Lista. Una fila por comparación; el resultado lo decide el intérprete, no la
          resta de FPS. El nombre del juego solo se repite cuando cambia respecto a la
          fila de arriba: con varias pruebas seguidas del mismo juego, repetirlo en cada
          línea era la mitad del texto de la pantalla. */}
      <motion.ul
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.25 }}
        className="rounded-xl bg-[var(--surface)] border border-[var(--border)] divide-y divide-[var(--border)] overflow-hidden"
      >
        {sorted.map((test, i) => {
          const r = interpretar(test.comparison);
          const et = etiquetasDe(test.comparison);
          const titulo = titularDelCambio(r, et);
          const mismoJuego =
            i > 0 && normGame(sorted[i - 1].game_name) === normGame(test.game_name);
          const pctCambio = test.fps_change_percent;

          return (
            <li key={test.id}>
              <div
                role="button"
                tabIndex={0}
                onClick={() => setSelectedId(test.id)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    setSelectedId(test.id);
                  }
                }}
                aria-label={`${nombreLegible(test.game_name)}: ${et.antes} ${test.before_fps.toFixed(1)} FPS, ${et.despues} ${test.after_fps.toFixed(1)} FPS. ${titulo}`}
                className="group flex items-center gap-3 px-3 sm:px-4 py-2.5 cursor-pointer hover:bg-[var(--bg)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[var(--action)]"
              >
                {/* Juego (solo cuando cambia) y la línea de números. */}
                <div className="min-w-0 flex-1">
                  <p
                    className={`text-sm font-semibold text-[var(--text)] truncate ${
                      mismoJuego ? 'sr-only' : ''
                    }`}
                  >
                    {nombreLegible(test.game_name)}
                  </p>
                  <p className="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-[var(--text-muted)]">
                    <span className="font-mono tabular-nums">
                      <PruebaN bench={test.comparison.baseline} compacto />
                      {' '}
                      <span className="text-[var(--text)]">{test.before_fps.toFixed(1)}</span>
                      <span className="text-[var(--text-dim)]"> → </span>
                      <PruebaN bench={test.comparison.test} compacto />
                      {' '}
                      <span className="text-[var(--text)]">{test.after_fps.toFixed(1)}</span>
                      <span className="text-[var(--text-dim)]"> FPS</span>
                    </span>
                    <span className="text-[var(--text-dim)]" aria-hidden="true">·</span>
                    <span className="font-mono tabular-nums">{formatTimestamp(testDateOf(test))}</span>
                  </p>
                </div>

                {/* Resultado: variación de FPS y titular del intérprete. */}
                <div className="flex items-center gap-3 shrink-0">
                  <div className="text-right max-w-[13rem]">
                    <p className={`text-sm font-bold font-mono tabular-nums ${ESTADO_TEXTO[r.estado]}`}>
                      {pctCambio > 0 ? '+' : ''}
                      {pctCambio.toFixed(1)}%
                    </p>
                    <p className={`text-xs font-semibold truncate ${ESTADO_TEXTO[r.estado]}`}>{titulo}</p>
                  </div>

                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      askDeleteOne(test);
                    }}
                    onKeyDown={(e) => e.stopPropagation()}
                    className="p-1.5 rounded-lg text-[var(--text-dim)] hover:text-[var(--error)] hover:bg-error/10 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--error)]"
                    title="Eliminar esta prueba"
                    aria-label={`Eliminar la prueba de ${test.game_name || 'este juego'}`}
                  >
                    <Trash2 className="w-4 h-4" aria-hidden="true" />
                  </button>

                  <ChevronRight
                    className="w-4 h-4 text-[var(--text-dim)] group-hover:text-[var(--action)] transition-colors"
                    aria-hidden="true"
                  />
                </div>
              </div>
            </li>
          );
        })}
      </motion.ul>

      {overlays}
    </div>
  );
}
