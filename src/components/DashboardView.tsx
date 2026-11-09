import { useState } from 'react';
import { motion } from 'framer-motion';
import {
  TrendingUp, BarChart3, Calendar,
  Gamepad2, Target, Trash2, ArrowLeft, ChevronRight,
  Terminal, Trophy, Copy,
} from 'lucide-react';
import { ComparisonResultCard, summarizeComparison } from './BenchmarksView';
import type { SavedComparison } from '../hooks/useSavedComparisons';
import type { BenchmarkComparison } from '../lib/types';
import { useSavedComparisons } from '../hooks/useSavedComparisons';
import { useConfirm, useToast } from '../hooks/useConfirm';
import { ConfirmDialog, Toast } from './ConfirmDialog';

// "2026-11-03 10:47:42" -> formato ISO, que todos los motores web entienden
const parseDate = (s: string) => new Date(s.replace(' ', 'T'));

// La fecha de la prueba es la de la grabación nueva (el "después"), no la de cuando se guardó.
const testDateOf = (c: SavedComparison) => c.comparison.test.timestamp;

const normGame = (n: string) => n.trim().toLowerCase();

/* ───────────── Argumento probado y ganador (detalle de una prueba) ───────────── */

// Separa el comando en piezas (variables, banderas, envoltorios) sin el marcador %command%
const tokenize = (cmd: string) => Array.from(new Set(cmd.split(/\s+/).filter((t) => t && t !== '%command%')));

function diffCommands(before: string, after: string) {
  const a = tokenize(before);
  const b = tokenize(after);
  return {
    added: b.filter((t) => !a.includes(t)),
    removed: a.filter((t) => !b.includes(t)),
  };
}

function CommandBox({
  label,
  color,
  command,
  isWinner,
  onCopy,
}: {
  label: string;
  color: string;
  command: string;
  isWinner: boolean;
  onCopy: (text: string) => void;
}) {
  return (
    <div
      className={`p-3 rounded-lg bg-[var(--bg)] border-2 ${
        isWinner ? 'border-[var(--compatible)]/60' : 'border-[var(--border)]'
      }`}
    >
      <div className="flex items-center justify-between gap-2 mb-2">
        <span className="text-sm font-bold" style={{ color }}>
          {label}
        </span>
        {isWinner && (
          <span className="flex items-center gap-1 text-[11px] font-bold text-[var(--compatible)]">
            <Trophy className="w-3.5 h-3.5" />
            Ganador
          </span>
        )}
      </div>
      {command ? (
        <>
          <code className="block text-[11px] leading-relaxed font-mono text-[var(--text)] break-all select-text">
            {command}
          </code>
          <button
            type="button"
            onClick={() => onCopy(command)}
            className="mt-2 flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-semibold text-[var(--text-muted)] bg-[var(--surface)] border border-[var(--border)] hover:text-[var(--text)] hover:border-[var(--action)]/50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
          >
            <Copy className="w-3 h-3" />
            Copiar
          </button>
        </>
      ) : (
        <p className="text-xs text-[var(--text-muted)] italic">Sin parámetros (configuración original)</p>
      )}
    </div>
  );
}

function ArgumentCard({
  comparison,
  onCopy,
}: {
  comparison: BenchmarkComparison;
  onCopy: (text: string) => void;
}) {
  const summary = summarizeComparison(comparison);
  const beforeCmd = (comparison.baseline.launch_command || '').trim();
  const afterCmd = (comparison.test.launch_command || '').trim();
  const { added, removed } = diffCommands(beforeCmd, afterCmd);
  const hasChange = added.length > 0 || removed.length > 0;

  const badge =
    summary.winner === 'despues'
      ? { text: 'Ganó la grabación nueva', cls: 'bg-[var(--compatible)]/10 text-[var(--compatible)] border-[var(--compatible)]/40' }
      : summary.winner === 'antes'
      ? { text: 'Ganó la grabación anterior', cls: 'bg-[var(--compatible)]/10 text-[var(--compatible)] border-[var(--compatible)]/40' }
      : summary.winner === 'empate'
      ? { text: 'Sin ganador claro', cls: 'bg-[var(--bg)] text-[var(--text-muted)] border-[var(--border)]' }
      : { text: 'Depende de lo que prefieras', cls: 'bg-[var(--caution)]/10 text-[var(--caution)] border-[var(--caution)]/40' };

  return (
    <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
      <h3 className="text-base font-bold mb-4 flex items-center gap-2 text-[var(--text)]">
        <Terminal className="w-5 h-5 text-[var(--action)]" />
        Argumento probado
      </h3>

      {/* Qué cambió */}
      <div className="mb-4">
        <p className="text-xs text-[var(--text-muted)] mb-2">Lo que cambió entre las dos grabaciones</p>
        {hasChange ? (
          <div className="flex flex-wrap gap-2">
            {added.map((t) => (
              <span
                key={`add-${t}`}
                className="px-2.5 py-1 rounded-md text-xs font-mono font-semibold bg-[var(--compatible)]/10 text-[var(--compatible)] border border-[var(--compatible)]/30 select-text"
              >
                + {t}
              </span>
            ))}
            {removed.map((t) => (
              <span
                key={`rem-${t}`}
                className="px-2.5 py-1 rounded-md text-xs font-mono font-semibold bg-[var(--error)]/10 text-[var(--error)] border border-[var(--error)]/30 select-text"
              >
                − {t}
              </span>
            ))}
          </div>
        ) : (
          <p className="text-xs text-[var(--text-muted)] leading-relaxed">
            No se registró ninguna diferencia entre los dos comandos. El comando que se guarda es el que tenía la app
            al pulsar <strong className="text-[var(--text)]">Analizar y guardar</strong>, así que activa el parámetro
            antes de analizar la grabación.
          </p>
        )}
      </div>

      {/* Ganador */}
      <div className="mb-4 flex flex-wrap items-center gap-x-3 gap-y-1">
        <span className={`px-3 py-1 rounded-full text-xs font-bold border ${badge.cls}`}>{badge.text}</span>
        <span className="text-xs text-[var(--text-muted)]">{summary.advice}</span>
      </div>

      {/* Comandos completos */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <CommandBox
          label="Inicial"
          color="#38bdf8"
          command={beforeCmd}
          isWinner={summary.winner === 'antes'}
          onCopy={onCopy}
        />
        <CommandBox
          label="Final"
          color="var(--caution)"
          command={afterCmd}
          isWinner={summary.winner === 'despues'}
          onCopy={onCopy}
        />
      </div>
    </article>
  );
}

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

  // Fecha relativa para la tarjeta "Última prueba"
  const formatRelative = (dateString: string) => {
    const date = parseDate(dateString);
    const diffMs = Date.now() - date.getTime();
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (Number.isNaN(diffMs)) return dateString;
    if (diffHours < 1) return 'Hace menos de 1 hora';
    if (diffHours < 24) return `Hace ${diffHours} hora${diffHours > 1 ? 's' : ''}`;
    if (diffDays < 7) return `Hace ${diffDays} día${diffDays > 1 ? 's' : ''}`;
    return date.toLocaleDateString('es-ES');
  };

  // Fecha y hora exactas en que se hizo la prueba
  const formatFull = (dateString: string) => {
    const date = parseDate(dateString);
    if (Number.isNaN(date.getTime())) return dateString;
    return date.toLocaleString('es-ES', {
      day: '2-digit',
      month: '2-digit',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const percentClass = (p: number) =>
    p > 0 ? 'text-[var(--compatible)]' : p < 0 ? 'text-[var(--error)]' : 'text-[var(--text-muted)]';

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
          <div className="w-12 h-12 border-4 border-[var(--action)]/30 border-t-[var(--action)] rounded-full animate-spin mx-auto mb-4"></div>
          <p className="text-[var(--text-muted)]">Cargando estadísticas...</p>
        </div>
      </div>
    );
  }

  /* ───────────────────────── Detalle de una prueba ───────────────────────── */
  if (selected) {
    return (
      <div className="max-w-6xl mx-auto p-8 space-y-6">
        <button
          type="button"
          onClick={() => setSelectedId(null)}
          className="flex items-center gap-2 text-sm font-semibold text-[var(--text-muted)] hover:text-[var(--text)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)] rounded"
        >
          <ArrowLeft className="w-4 h-4" />
          Volver a Mis pruebas
        </button>

        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="text-xs text-[var(--text-muted)] mb-2">Prueba guardada</p>
            <h2 className="text-2xl font-bold truncate">{selected.game_name || 'Desconocido'}</h2>
            <p className="text-sm text-[var(--text-muted)] mt-1">
              {formatFull(testDateOf(selected))} • {selected.before_fps.toFixed(1)} FPS → {selected.after_fps.toFixed(1)} FPS{' '}
              <span className={`font-bold ${percentClass(selected.fps_change_percent)}`}>
                ({selected.fps_change_percent > 0 ? '+' : ''}{selected.fps_change_percent.toFixed(1)}%)
              </span>
            </p>
          </div>
          <button
            type="button"
            onClick={() => askDeleteOne(selected)}
            className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-[var(--error)]/10 text-[var(--error)] border border-[var(--error)]/30 hover:bg-[var(--error)]/20 hover:border-[var(--error)]/50 transition-all duration-200 flex items-center gap-1.5 shrink-0"
          >
            <Trash2 className="w-3.5 h-3.5" />
            Eliminar prueba
          </button>
        </div>

        <ArgumentCard comparison={selected.comparison} onCopy={copyText} />

        <ComparisonResultCard
          comparison={selected.comparison}
          onClose={() => setSelectedId(null)}
          closeLabel="Volver a Mis pruebas"
        />

        {overlays}
      </div>
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
  const totalGames = new Set(items.map((c) => normGame(c.game_name))).size;
  const lastTestDate = sorted.length > 0 ? testDateOf(sorted[0]) : null;

  return (
    <div className="max-w-6xl mx-auto p-8 space-y-6">
      {/* Header */}
      <div className="flex items-end justify-between gap-4">
        <div>
          <p className="text-xs text-[var(--text-muted)] mb-2">Dashboard</p>
          <h2 className="text-2xl font-bold">Mis pruebas de rendimiento</h2>
        </div>
        <button
          type="button"
          onClick={askDeleteAll}
          className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-[var(--error)]/10 text-[var(--error)] border border-[var(--error)]/30 hover:bg-[var(--error)]/20 hover:border-[var(--error)]/50 transition-all duration-200 flex items-center gap-1.5 group"
          title="Eliminar todas las pruebas guardadas"
        >
          <Trash2 className="w-3.5 h-3.5 group-hover:rotate-12 transition-transform" />
          Borrar todas las pruebas
        </button>
      </div>

      {/* Estadísticas generales */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
          className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
        >
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-lg bg-[var(--action)]/10 flex items-center justify-center">
              <Target className="w-5 h-5 text-[var(--action)]" />
            </div>
            <div>
              <p className="text-xs text-[var(--text-muted)]">Total de pruebas</p>
              <p className="text-2xl font-bold text-[var(--text)]">{items.length}</p>
            </div>
          </div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.2 }}
          className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
        >
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-lg bg-[var(--compatible)]/10 flex items-center justify-center">
              <Gamepad2 className="w-5 h-5 text-[var(--compatible)]" />
            </div>
            <div>
              <p className="text-xs text-[var(--text-muted)]">Juegos probados</p>
              <p className="text-2xl font-bold text-[var(--text)]">{totalGames}</p>
            </div>
          </div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.3 }}
          className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
        >
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-lg bg-[var(--caution)]/10 flex items-center justify-center">
              <Calendar className="w-5 h-5 text-[var(--caution)]" />
            </div>
            <div>
              <p className="text-xs text-[var(--text-muted)]">Última prueba</p>
              <p className="text-lg font-bold text-[var(--text)]">
                {lastTestDate ? formatRelative(lastTestDate) : 'N/A'}
              </p>
            </div>
          </div>
        </motion.div>
      </div>

      {/* Pruebas guardadas */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.5 }}
        className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
      >
        <h3 className="text-base font-bold mb-1 flex items-center gap-2 text-[var(--text)]">
          <TrendingUp className="w-5 h-5 text-[var(--action)]" />
          Pruebas recientes
        </h3>
        <p className="text-xs text-[var(--text-muted)] mb-4">
          Haz clic en una prueba para ver la comparación completa.
        </p>
        <div className="space-y-2">
          {sorted.map((test) => (
            <div
              key={test.id}
              role="button"
              tabIndex={0}
              onClick={() => setSelectedId(test.id)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  setSelectedId(test.id);
                }
              }}
              className="flex items-center justify-between gap-3 p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
            >
              <div className="flex items-center gap-3 min-w-0">
                <Gamepad2 className="w-5 h-5 text-[var(--text-muted)] shrink-0" />
                <div className="min-w-0">
                  <p className="text-sm font-semibold text-[var(--text)] truncate">{test.game_name || 'Desconocido'}</p>
                  <p className="text-xs text-[var(--text-muted)]">
                    {test.before_fps.toFixed(1)} FPS → {test.after_fps.toFixed(1)} FPS
                  </p>
                </div>
              </div>

              <div className="flex items-center gap-3 shrink-0">
                <div className="text-right">
                  <p className={`text-sm font-bold ${percentClass(test.fps_change_percent)}`}>
                    {test.fps_change_percent > 0 ? '+' : ''}{test.fps_change_percent.toFixed(1)}%
                  </p>
                  <p className="text-xs text-[var(--text-dim)]">{formatFull(testDateOf(test))}</p>
                </div>

                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    askDeleteOne(test);
                  }}
                  onKeyDown={(e) => e.stopPropagation()}
                  className="p-2 rounded-lg text-[var(--error)] hover:bg-[var(--error)]/10 hover:scale-110 transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--error)]"
                  title="Eliminar esta prueba"
                  aria-label={`Eliminar la prueba de ${test.game_name || 'este juego'}`}
                >
                  <Trash2 className="w-4 h-4" />
                </button>

                <ChevronRight className="w-4 h-4 text-[var(--text-dim)]" />
              </div>
            </div>
          ))}
        </div>
      </motion.div>

      {overlays}
    </div>
  );
}
