import { Copy, Trophy, Trash2 } from 'lucide-react';
import { formatTimestamp } from '../lib/format';
import { clasificar, CRITERIO_RANKING, type EntradaRanking } from '../lib/ranking';
import { comandoPegable, etiquetaPrueba, textoComando } from '../lib/pruebaLabel';
import type { Benchmark } from '../lib/types';

/**
 * «Cuál va mejor de tus N grabaciones» para **un solo juego**.
 *
 * Vive aparte de `BenchmarksView` porque se enseña en dos sitios: la ventana «Pruebas
 * de este juego» de la ficha del juego y la pestaña de benchmarks (que solo la pinta
 * cuando hay un juego abierto). El criterio de orden está en `lib/ranking.ts`.
 */

const fmt = (n: number, dec = 1) => n.toFixed(dec).replace('.', ',');

export function RankingCard({
  rows,
  onDelete,
  onDeleteAll,
  onCopy,
}: {
  /** Grabaciones del juego, sin ordenar (la tarjeta las clasifica). */
  rows: Benchmark[];
  /** Borra una grabación concreta. Sin esto no se pinta el botón de borrar. */
  onDelete?: (b: Benchmark) => void;
  /** Borra todo el historial del juego. Sin esto no se pinta el botón. */
  onDeleteAll?: () => void;
  /**
   * Copia el comando de una grabación al portapapeles.
   *
   * Va junto al de borrar porque son las dos acciones de la fila: recuperar el comando con
   * el que se hizo la prueba (para volver a ponerlo en Steam) y descartarla. Antes había que
   * buscarla en la pestaña Benchmarks para poder copiarla.
   */
  onCopy?: (comando: string) => void;
}) {
  const ordenadas: EntradaRanking[] = clasificar(rows);
  if (ordenadas.length === 0) return null;

  const hayBorrado = Boolean(onDelete ?? onDeleteAll);

  return (
    <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
      <div className="flex items-start justify-between gap-3 mb-1">
        <h3 className="text-base font-bold text-[var(--text)] flex items-center gap-2">
          <Trophy className="w-5 h-5 text-[var(--action)]" />
          {/* Con una sola grabación no hay nada que comparar: «de tus 1 grabaciones» era
              incorrecto gramaticalmente y además prometía un ranking que no existe. */}
          {ordenadas.length === 1
            ? 'Tu única grabación de este juego'
            : `Cuál va mejor de tus ${ordenadas.length} grabaciones`}
        </h3>
        {onDeleteAll && ordenadas.length > 1 && (
          <button
            type="button"
            onClick={onDeleteAll}
            className="shrink-0 flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-semibold text-[var(--error)] bg-error/10 border border-error/30 hover:bg-error/20 transition-colors"
            aria-label="Borrar todas las grabaciones de este juego"
          >
            <Trash2 className="w-3.5 h-3.5" />
            Borrar todas
          </button>
        )}
      </div>
      <p className="text-xs text-[var(--text-muted)] mb-4 leading-relaxed">
        {ordenadas.length === 1
          ? // Explicar cómo se ordena una lista de uno no aporta nada: lo que falta es
            // una segunda grabación, así que se dice eso.
            'Es tu punto de partida. Graba otra vez cambiando una sola cosa y aquí se verá cuál va mejor.'
          : CRITERIO_RANKING}
      </p>

      <div className="space-y-2">
        {ordenadas.map((r) => {
          // En la ganadora no se repite la diferencia: sería «0,0 %».
          const etiquetaDiferencia = r.puesto === 1 ? 'referencia' : `${fmt(r.pctVsGanadora)} %`;
          return (
            <div
              key={r.bench.id}
              className={`flex items-center gap-3 px-3 py-2.5 rounded-lg border ${
                r.ganadora
                  ? 'bg-compatible/10 border-compatible/40'
                  : 'bg-[var(--bg)] border-[var(--border)]'
              }`}
            >
              <span
                className={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold shrink-0 ${
                  r.ganadora
                    ? 'bg-[var(--compatible)] text-black'
                    : 'bg-[var(--surface)] text-[var(--text-muted)]'
                }`}
              >
                {r.puesto}
              </span>

              <div className="min-w-0 flex-1">
                <p className="text-sm font-semibold text-[var(--text)] truncate">
                  {r.ganadora && <span className="text-[var(--compatible)]">Mejor · </span>}
                  {etiquetaPrueba(r.bench)}
                </p>
                <p className="text-xs text-[var(--text-dim)] truncate">
                  {formatTimestamp(r.bench.timestamp)}
                  {' · '}
                  {/* Se dice de dónde sale el texto: mezclar nota y comando hacía que el
                      `%command%` pareciera una nota escrita por el usuario. */}
                  {r.bench.notes ?? textoComando(r.bench) ?? 'Comando no registrado'}
                </p>
              </div>

              <div className="text-right shrink-0">
                <p className="text-sm font-bold text-[var(--text)] font-mono">
                  {r.low1.toFixed(0)} FPS{' '}
                  <span className="text-[10px] font-normal text-[var(--text-dim)]">
                    1 % low
                  </span>
                </p>
                <p className="text-[11px] text-[var(--text-muted)] font-mono">
                  {etiquetaDiferencia} · {r.fps.toFixed(0)} medios
                  {r.estabilidad != null && ` · ${Math.round(r.estabilidad)}/100`}
                </p>
              </div>

              {onCopy &&
                (() => {
                  // Sin comando registrado no hay nada que copiar: se omite el botón en vez
                  // de copiar un `%command%` inventado.
                  const comando = comandoPegable(r.bench);
                  if (!comando) return null;
                  return (
                    <button
                      type="button"
                      onClick={() => onCopy(comando)}
                      className="shrink-0 p-1.5 rounded-lg text-[var(--text-dim)] hover:text-[var(--action)] hover:bg-action/10 transition-colors"
                      aria-label={`Copiar el comando de ${etiquetaPrueba(r.bench)}`}
                      title={`Copiar el comando de ${etiquetaPrueba(r.bench)}`}
                    >
                      <Copy className="w-4 h-4" />
                    </button>
                  );
                })()}

              {onDelete && (
                <button
                  type="button"
                  onClick={() => onDelete(r.bench)}
                  className="shrink-0 p-1.5 rounded-lg text-[var(--text-dim)] hover:text-[var(--error)] hover:bg-error/10 transition-colors"
                  aria-label={`Borrar ${etiquetaPrueba(r.bench)}`}
                  title={`Borrar ${etiquetaPrueba(r.bench)}`}
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              )}
            </div>
          );
        })}
      </div>

      {hayBorrado && ordenadas.length > 1 && (
        <p className="text-[11px] text-[var(--text-dim)] mt-3 leading-relaxed">
          Se guardan como máximo 10 pruebas por juego: al guardar la número 11 se borra
          sola la más antigua.
        </p>
      )}
    </article>
  );
}
