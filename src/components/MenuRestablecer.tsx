import { ChevronDown, Eraser, History, RotateCcw } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

/** Una acción del menú de restablecimiento. */
export interface AccionRestablecer {
  id: string;
  titulo: string;
  /** Qué hace exactamente. Se enseña bajo el título: «restaurar» sin más es ambiguo. */
  detalle: string;
  /** `false` para lo que todavía no está disponible: no se deja pulsar. */
  disponible: boolean;
  /** Motivo por el que no está disponible, si no lo está. */
  motivoNoDisponible?: string;
}

/**
 * Menú de restablecimiento del comando.
 *
 * 「Restablecer」 a secas era un botón único que hacía una cosa sola y no siempre la que el
 * usuario esperaba. Aquí hay tres acciones distintas y se ven antes de pulsar:
 *
 * 1. volver al comando importado de Steam (recupera su texto exacto);
 * 2. cargar un perfil guardado (solo la entrada: la migración llega en la fase 4);
 * 3. quitar todos los ajustes de Thoriapp, dejando el comando en `%command%` más los
 *    argumentos propios del juego.
 *
 * Las que descartan cambios piden confirmación; las que no se pueden hacer todavía se
 * enseñan deshabilitadas con el motivo, en vez de desaparecer o fingir que funcionan.
 */
export function MenuRestablecer({
  acciones,
  onElegir,
  deshabilitado = false,
}: {
  acciones: AccionRestablecer[];
  onElegir: (id: string) => void;
  /** Hay una operación en curso: el menú se abre, pero las acciones no se ejecutan. */
  deshabilitado?: boolean;
}) {
  const [abierto, setAbierto] = useState(false);
  const raiz = useRef<HTMLDivElement>(null);

  // Sin acciones no hay nada que elegir: un botón que abre un menú vacío es una promesa
  // incumplida.
  const hayAcciones = acciones.length > 0;

  // Cerrar al pulsar fuera o con Escape: es un menú flotante y quedarse abierto tras
  // cambiar de pestaña lo deja tapando el comando.
  useEffect(() => {
    if (!abierto) return;
    const fuera = (e: MouseEvent) => {
      if (!raiz.current?.contains(e.target as Node)) setAbierto(false);
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setAbierto(false);
    };
    document.addEventListener('mousedown', fuera);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', fuera);
      document.removeEventListener('keydown', escape);
    };
  }, [abierto]);

  if (!hayAcciones) return null;

  return (
    <div ref={raiz} className="relative">
      <button
        type="button"
        onClick={() => setAbierto((v) => !v)}
        disabled={deshabilitado}
        aria-haspopup="menu"
        aria-expanded={abierto}
        className="w-full sm:w-auto px-5 py-4 rounded-xl text-sm font-semibold bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] hover:text-[var(--text)] transition-colors duration-150 flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <RotateCcw className="w-4 h-4" aria-hidden="true" />
        Restablecer
        <ChevronDown
          className={`w-3.5 h-3.5 transition-transform duration-150 ${abierto ? 'rotate-180' : ''}`}
          aria-hidden="true"
        />
      </button>

      {abierto && (
        <div
          role="menu"
          aria-label="Restablecer comando"
          className="absolute right-0 bottom-full mb-2 z-30 w-72 p-1.5 rounded-xl bg-[var(--card)] border border-[var(--border)] shadow-xl shadow-black/40 animate-[aparecer_120ms_ease-out]"
        >
          {acciones.map((accion) => (
            <button
              key={accion.id}
              type="button"
              role="menuitem"
              disabled={!accion.disponible}
              title={accion.motivoNoDisponible}
              onClick={() => {
                setAbierto(false);
                onElegir(accion.id);
              }}
              className="w-full text-left px-3 py-2.5 rounded-lg hover:bg-[var(--card-hover)] transition-colors duration-150 disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
            >
              <span className="flex items-center gap-2 text-sm font-medium text-[var(--text)]">
                {accion.id === 'original' ? (
                  <History className="w-3.5 h-3.5 text-[var(--action)]" aria-hidden="true" />
                ) : (
                  <Eraser className="w-3.5 h-3.5 text-[var(--text-muted)]" aria-hidden="true" />
                )}
                {accion.titulo}
              </span>
              <span className="block text-[11px] text-[var(--text-muted)] mt-0.5 leading-snug">
                {accion.disponible ? accion.detalle : accion.motivoNoDisponible}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
