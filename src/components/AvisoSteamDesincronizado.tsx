import { AlertCircle, ArrowDownToLine, RefreshCw } from 'lucide-react';
import type { OpcionesSteam } from '../lib/types';

/**
 * Aviso discreto: «Steam tiene otro comando».
 *
 * ## Diseño
 *
 * No es un modal ni un botón permanente en la barra: es una línea dentro de la caja del
 * comando, donde el usuario ya está mirando. Aparece **solo** cuando Steam tiene algo
 * configurado y distinto, que es justo el momento en que importa.
 *
 * ## Por qué avisa en vez de corregir
 *
 * Thoriapp no puede saber si el comando de Steam es el que el usuario quiere probar. El
 * editor existe para preparar ajustes sin tocar Steam (decisión de diseño del proyecto), así
 * que igualar los dos automáticamente rompería justo ese uso. Se ofrece importar, y nada más.
 *
 * ## Por qué no escribe en Steam
 *
 * Steam reescribe `LaunchOptions` al cerrarse: escribir en `localconfig.vdf` se pierde en la
 * siguiente partida. La vía fiable es que el usuario copie desde Thoriapp a Steam.
 */
export function AvisoSteamDesincronizado({
  steam,
  comandoActual,
  onImportar,
  onRevisar,
}: {
  steam: OpcionesSteam;
  comandoActual: string;
  /** Copia el texto de Steam al editor de Thoriapp. No toca Steam. */
  onImportar: () => void;
  /** Copia el comando de Thoriapp al portapapeles para pegarlo en Steam. */
  onRevisar?: () => void;
}) {
  return (
    <div
      className="mt-3 p-3 rounded-lg bg-[var(--caution-bg)] border border-caution/40"
      data-testid="aviso-steam"
    >
      <p className="text-xs font-semibold text-[var(--caution)] flex items-start gap-1.5">
        <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" aria-hidden="true" />
        <span>
          Steam tiene otras opciones de lanzamiento para este juego. Si grabas sin pasar por
          Thoriapp, la prueba registrará el comando de arriba, no el de Steam.
        </span>
      </p>

      <div className="mt-2 p-2 rounded bg-[var(--bg)] border border-[var(--border)]">
        <p className="text-[10px] uppercase tracking-wider text-[var(--text-dim)] mb-1">
          En Steam
        </p>
        <code className="block text-[11px] font-mono text-[var(--text)] break-all select-text">
          {steam.opciones || '(vacío: Steam no añade nada al ejecutable)'}
        </code>
      </div>

      <div className="mt-2 flex flex-wrap items-center gap-2">
        {steam.opciones && (
          <button
            type="button"
            onClick={onImportar}
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
          >
            <ArrowDownToLine className="w-3 h-3" aria-hidden="true" />
            Traer a Thoriapp
          </button>
        )}
        {onRevisar && (
          <button
            type="button"
            onClick={onRevisar}
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-semibold text-[var(--text-muted)] bg-[var(--surface)] border border-[var(--border)] hover:text-[var(--text)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
          >
            <RefreshCw className="w-3 h-3" aria-hidden="true" />
            Copiar el de Thoriapp para pegar en Steam
          </button>
        )}
        <span className="text-[10px] text-[var(--text-dim)]">
          {comandoActual === '%command%'
            ? 'Thoriapp no añade ningún ajuste ahora mismo.'
            : 'Thoriapp no modifica Steam por su cuenta.'}
        </span>
      </div>
    </div>
  );
}
