import { useState } from 'react';
import { ChevronDown, FolderOpen, Save } from 'lucide-react';
import type { Profile } from '../lib/types';

export interface PerfilesDelJuegoProps {
  /** Perfiles de **este** appid, ya filtrados. Es una derivación de la colección única. */
  perfiles: Profile[];
  /** Id del perfil cargado en el editor, o `null` si no se ha cargado ninguno. */
  cargadoId: string | null;
  /** Nombre del perfil cargado, ya resuelto. `null` si no hay. */
  cargadoNombre: string | null;
  /** Hay cambios sin guardar respecto a la instantánea del perfil cargado. */
  sinGuardar: boolean;
  /** El motor está ocupado: no se puede cargar nada todavía. */
  ocupado: boolean;
  /**
   * Carga el perfil indicado. El padre decide si pide confirmación: este componente no
   * descarta cambios por su cuenta.
   */
  onCargar: (perfil: Profile) => void;
  /** Vuelve a la configuración del perfil cargado, descartando lo que haya encima. */
  onRestaurarCargado: () => void;
  /** Lleva a la biblioteca global de perfiles. */
  onGestionar: () => void;
}

/**
 * Perfiles guardados de este juego, en la columna derecha de su vista.
 *
 * Es un atajo para lo que más se hace con un perfil: cargarlo. La gestión (importar,
 * exportar, renombrar, borrar, ordenar) se queda en la pestaña global; aquí solo se elige
 * y se carga.
 *
 * Decisiones que no son de estilo:
 *
 * - **Seleccionar no carga.** El desplegable solo cambia qué perfil está elegido; el
 *   comando no se toca hasta pulsar «Cargar perfil». Cargar es una operación que puede
 *   descartar trabajo, así que no puede pasar por mover un `<select>`.
 * - **El alto no crece con el número de perfiles**: el selector es un `<select>` nativo,
 *   que además resuelve teclado y accesibilidad sin inventar nada.
 * - **No hay aquí un botón de guardar principal.** El de verdad está en `CommandBox`; este
 *   solo aparece cuando hay un perfil cargado y lo que hace es *actualizarlo*. Duplicar el
 *   botón de guardar crearía dos caminos para la misma acción.
 */
export function PerfilesDelJuego({
  perfiles,
  cargadoId,
  cargadoNombre,
  sinGuardar,
  ocupado,
  onCargar,
  onRestaurarCargado,
  onGestionar,
}: PerfilesDelJuegoProps) {
  // Selección aparte de la carga: elegir un perfil en la lista no debe cambiar nada.
  const [seleccionado, setSeleccionado] = useState<string>('');
  const elegido = perfiles.find((p) => p.id === seleccionado) ?? null;

  const cargado = perfiles.find((p) => p.id === cargadoId) ?? null;
  // Un perfil cargado puede no estar en esta lista (se borró, o vino de otro sitio): se
  // enseña su nombre igual, porque es lo que el usuario tiene delante.
  const nombreCargado = cargado?.name ?? cargadoNombre;

  return (
    <aside className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
      <div className="flex items-baseline justify-between mb-3">
        <h3 className="text-sm font-bold text-[var(--text)]">Perfiles del juego</h3>
        <span className="text-[11px] text-[var(--text-dim)]">
          {perfiles.length === 0
            ? 'ninguno'
            : `${perfiles.length} ${perfiles.length === 1 ? 'perfil' : 'perfiles'}`}
        </span>
      </div>

      {perfiles.length === 0 ? (
        <>
          <p className="text-xs text-[var(--text-muted)] leading-snug">
            Este juego no tiene perfiles guardados. Configúralo y usa «Guardar perfil» para
            crear el primero.
          </p>
          <button
            onClick={onGestionar}
            className="mt-3 w-full flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-semibold text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] hover:text-[var(--text)] transition-colors"
          >
            <FolderOpen className="w-3.5 h-3.5" /> Ver todos los perfiles
          </button>
        </>
      ) : (
        <>
          {/* `<select>` nativo: con cien perfiles el alto no cambia. */}
          <label className="block text-[11px] text-[var(--text-muted)] mb-1" htmlFor="perfil-juego">
            Elegir perfil
          </label>
          <div className="relative">
            <select
              id="perfil-juego"
              value={seleccionado}
              onChange={(e) => setSeleccionado(e.target.value)}
              // Deshabilitado mientras el motor trabaja: cargar dos veces seguidas dejaría
              // dos peticiones compitiendo por la misma revisión.
              disabled={ocupado}
              className="w-full appearance-none px-3 py-2 pr-8 rounded-lg text-sm bg-[var(--bg)] text-[var(--text)] border border-[var(--border)] hover:border-[var(--border-hover)] focus:outline-none focus:border-[var(--border-hover)] disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              <option value="">Selecciona un perfil…</option>
              {perfiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                  {p.command ? '' : ' (antiguo)'}
                </option>
              ))}
            </select>
            <ChevronDown
              className="w-3.5 h-3.5 absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none text-[var(--text-dim)]"
              aria-hidden="true"
            />
          </div>

          <button
            onClick={() => elegido && onCargar(elegido)}
            disabled={!elegido || ocupado}
            className="mt-2 w-full flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Save className="w-3.5 h-3.5" /> Cargar perfil
          </button>

          {/* Qué está cargado. Se dice «cargado», nunca «activo»: Steam no está usando
              nada de esto todavía, y confundirlo llevaría a pegar el comando creyendo que
              ya está aplicado. */}
          {nombreCargado && (
            <div className="mt-3 pt-3 border-t border-[var(--border)] space-y-1.5">
              <div className="flex items-start justify-between gap-2">
                <span className="text-[11px] text-[var(--text-muted)]">Cargado</span>
                <strong
                  className="text-[11px] text-[var(--text)] text-right break-words"
                  title={nombreCargado}
                >
                  {nombreCargado}
                </strong>
              </div>

              {sinGuardar && (
                <p className="text-[11px] text-[var(--caution)] leading-snug">
                  Cambios sin guardar en este perfil.
                </p>
              )}

              <button
                onClick={onRestaurarCargado}
                disabled={ocupado}
                className="w-full px-2 py-1.5 rounded-lg text-[11px] font-semibold text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] hover:text-[var(--text)] transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Restaurar perfil cargado
              </button>
            </div>
          )}

          <button
            onClick={onGestionar}
            className="mt-2 w-full flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg text-[11px] font-semibold text-[var(--text-dim)] hover:text-[var(--text)] transition-colors"
          >
            <FolderOpen className="w-3 h-3" /> Gestionar todos los perfiles
          </button>
        </>
      )}
    </aside>
  );
}
