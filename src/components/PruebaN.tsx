import { useEffect, useId, useRef, useState } from 'react';
import { Copy } from 'lucide-react';
import type { Benchmark } from '../lib/types';
import { etiquetaPrueba, textoComando } from '../lib/pruebaLabel';

/**
 * Etiqueta «Prueba N» con su comando consultable.
 *
 * ## Por qué existe
 *
 * «Antes» y «Después» no dicen nada por sí solos: sin saber qué se cambió, una comparación
 * es un antes/después de qué. El número identifica la grabación de forma estable (lo asigna
 * el backend al guardar y no cambia nunca) y el comando queda a un gesto de distancia.
 *
 * ## Por qué el comando no está siempre a la vista
 *
 * Un comando como `mesa_glthread=true game-performance mangohud %command%` ocupa dos líneas
 * y ensucia la lista. Detrás de la etiqueta se consulta cuando hace falta, que es lo que el
 * usuario pidió; `Configuración comparada` sigue enseñando los dos completos al comparar.
 *
 * ## Accesibilidad
 *
 * El globo se pinta siempre en el DOM (oculto con `opacity-0`) y se enlaza con
 * `aria-describedby`, como el tooltip de `GameBadges`: así llega a lectores de pantalla.
 * Se abre al pasar el ratón y al llegar con el teclado (`group-focus-within`), y también
 * con un clic, que es la única forma en pantalla táctil.
 */

export interface PruebaNProps {
  /** Grabación a la que se refiere la etiqueta. `null` = no hay dato. */
  bench: Benchmark | null | undefined;
  /** Copia el comando al portapapeles. Si falta, no se ofrece el botón de copiar. */
  onCopy?: (texto: string) => void;
  /** Clase extra para la etiqueta (color, tamaño). */
  className?: string;
  /**
   * Variante compacta para listas: sin el globo de comando.
   *
   * En la lista de «Mis pruebas» el número identifica la fila; el comando se consulta al
   * abrir la prueba, donde ya está el tooltip completo y la configuración comparada.
   * Dibujar un globo por fila multiplica nodos sin que nadie los abra desde ahí.
   */
  compacto?: boolean;
}

export function PruebaN({ bench, onCopy, className = '', compacto = false }: PruebaNProps) {
  const tipId = useId();
  const [abierto, setAbierto] = useState(false);
  const raiz = useRef<HTMLSpanElement>(null);

  // Un clic fuera cierra el globo: sin esto, en táctil se queda abierto para siempre.
  useEffect(() => {
    if (!abierto) return;
    const fuera = (e: MouseEvent | TouchEvent) => {
      if (raiz.current && !raiz.current.contains(e.target as Node)) setAbierto(false);
    };
    document.addEventListener('mousedown', fuera);
    document.addEventListener('touchstart', fuera);
    return () => {
      document.removeEventListener('mousedown', fuera);
      document.removeEventListener('touchstart', fuera);
    };
  }, [abierto]);

  const comando = textoComando(bench ?? null);
  const textoCompleto = comando ?? 'Comando no registrado';
  // Al portapapeles va el comando **sin** el prefijo «Steam:»/«Comando:», que es solo
  // presentación: pegado en Steam, ese prefijo sería un argumento inválido.
  const paraCopiar = comando ? comando.replace(/^(Steam|Comando): /, '') : null;

  if (compacto) {
    return <span className={className}>{etiquetaPrueba(bench)}</span>;
  }

  return (
    <span
      ref={raiz}
      className="relative inline-flex group"
      onKeyDown={(e) => {
        if (e.key === 'Escape') setAbierto(false);
      }}
    >
      <button
        type="button"
        aria-describedby={tipId}
        aria-expanded={abierto}
        onClick={() => setAbierto((v) => !v)}
        className={`inline-flex items-center gap-1 rounded px-1 -mx-1 text-left underline decoration-dotted decoration-[var(--border-hover)] underline-offset-2 hover:decoration-current focus:outline-none focus-visible:ring-1 focus-visible:ring-[var(--action)] ${className}`}
      >
        {etiquetaPrueba(bench)}
      </button>

      <span
        id={tipId}
        role="tooltip"
        className={`absolute left-0 top-full z-30 mt-1.5 w-72 max-w-[min(20rem,80vw)] rounded-lg border border-[var(--border-hover)] bg-[var(--card)] px-3 py-2 text-left text-[11px] font-normal leading-snug shadow-xl transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100 ${
          abierto ? 'opacity-100' : 'pointer-events-none opacity-0'
        }`}
      >
        <span className="block font-semibold text-[var(--text-muted)]">
          {comando ? 'Comando registrado' : 'Sin dato'}
        </span>
        <span
          className={`mt-1 block break-all font-mono text-[11px] ${
            comando ? 'text-[var(--text)]' : 'italic text-[var(--text-dim)]'
          }`}
        >
          {textoCompleto}
        </span>
        {paraCopiar && paraCopiar !== 'sin opciones de lanzamiento' && onCopy ? (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onCopy(paraCopiar);
            }}
            className="mt-2 inline-flex items-center gap-1.5 rounded px-1.5 py-0.5 text-[11px] font-semibold text-[var(--action)] hover:bg-[var(--action)]/10"
          >
            <Copy className="h-3 w-3" />
            Copiar
          </button>
        ) : null}
      </span>
    </span>
  );
}
