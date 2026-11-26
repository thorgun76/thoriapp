import { AlertTriangle, Code2, Info, Trash2 } from 'lucide-react';
import type { SegmentoComando } from '../lib/motorComando';

/**
 * Los argumentos que Thoriapp no gestiona, pero conserva.
 *
 * ## Por qué esta sección existe
 *
 * Un comando de Steam puede llevar cosas que Thoriapp no conoce: `-novid`, `-dx11`,
 * `PROTON_NO_ESYNC=1`, un wrapper recién salido. Al importarlo hay dos opciones malas:
 * borrarlos (se rompe el juego del usuario) o no importar nada (se pierde todo lo demás).
 * La tercera es esta: enseñarlos, decir que se conservan y no tocarlos.
 *
 * ## Qué no hace
 *
 * - **No los reescribe.** Se muestra el texto exacto, con sus comillas y su orden.
 * - **No los borra por su cuenta.** Solo se ofrece quitar los que el motor marca como
 *   borrables de forma segura; para el resto, el aviso de que hay que editarlos a mano.
 */
export function ArgumentosConservados({
  segmentos,
  onQuitar,
  deshabilitado = false,
}: {
  /** Segmentos conservados del análisis, en el orden del comando. */
  segmentos: SegmentoComando[];
  /**
   * Quita un segmento. Solo llega para los que el motor permite quitar.
   *
   * Se pasa el **índice** del segmento en el análisis, no su span: el motor localiza el
   * segmento por posición, y un span copiado por el frontend podría no corresponder ya a
   * nada si el comando cambió entre el análisis y la pulsación.
   */
  onQuitar?: (indice: number) => void;
  /** Hay una operación en curso: no se aceptan más pulsaciones. */
  deshabilitado?: boolean;
}) {
  // Solo se enseñan los que el motor **conserva**. El planteamiento anterior pintaba el
  // análisis completo y dejaba colarse dos clases de segmento que no son «personalizados»:
  //
  //  - el marcador `%command%`, que es estructura del lanzador, no un argumento;
  //  - los ajustes que Thoriapp sí gestiona (`game-performance`, `mangohud`…), que ya
  //    aparecían como activos en el comando de arriba. Verlos otra vez aquí abajo, como si
  //    no se entendieran, es exactamente la contradicción que reportó el usuario.
  //
  // El filtro vive aquí y no en quien pasa los segmentos porque así no depende de que cada
  // llamada se acuerde de filtrar: un componente que recibe el análisis entero sigue
  // enseñando solo lo suyo.
  const conservados = segmentos.flatMap((seg, indice) =>
    seg.gestion.tipo === 'conservado' ? [{ seg, indice, razon: seg.gestion.razon }] : [],
  );

  // Nada que enseñar: la tarjeta no aparece. Un bloque vacío «0 argumentos» solo ocupa
  // sitio y hace pensar que falta algo.
  if (conservados.length === 0) return null;

  return (
    <section
      aria-labelledby="titulo-argumentos-conservados"
      className="p-5 rounded-xl bg-[var(--card)] border border-[var(--border)] animate-[aparecer_180ms_ease-out]"
    >
      <div className="flex items-start justify-between gap-3 mb-1">
        <h3
          id="titulo-argumentos-conservados"
          className="flex items-center gap-2 text-sm font-semibold text-[var(--text)]"
        >
          <Code2 className="w-4 h-4 text-[var(--text-muted)] shrink-0" />
          Argumentos personalizados
        </h3>
        {/* El contador no cambia de ancho con el número: sin esto, quitar uno movía el
            encabezado y el resto de la tarjeta. */}
        <span className="shrink-0 text-[11px] font-mono px-2 py-0.5 rounded-md bg-[var(--bg)] border border-[var(--border)] text-[var(--text-muted)] tabular-nums">
          {conservados.length}
        </span>
      </div>

      <p className="text-xs text-[var(--text-muted)] leading-relaxed mb-3">
        Thoriapp no conoce estos argumentos, así que no los toca. Se mantienen tal cual en su
        posición y se conservarán al cambiar otros ajustes.
      </p>

      <ul className="flex flex-wrap gap-1.5">
        {conservados.map(({ seg, indice, razon }) => {
          const clave = `${seg.span.inicio}-${seg.span.fin}`;
          // El motivo de conservación vive en `gestion.razon`: es el motor el que decide
          // por qué no se gestiona un segmento, y aquí solo se lee. `razon` (no `motivo`)
          // es el nombre que usa Rust al serializar `Gestion::Conservado`.
          const motivo = razon.motivo;
          // Solo se ofrece quitar cuando el motor lo permite. `desconocido` sí se puede
          // quitar (es un token completo y bien delimitado); `ambiguo`, `no_seguro` y los
          // argumentos del juego, no.
          const sePuedeQuitar = motivo === 'desconocido' && onQuitar !== undefined;

          return (
            <li key={clave} className="group flex items-stretch max-w-full">
              <span
                className={`flex items-center gap-1.5 min-w-0 pl-2.5 py-1.5 text-[11px] font-mono text-[var(--text)] bg-[var(--bg)] border border-[var(--border)] ${
                  sePuedeQuitar ? 'rounded-l-md border-r-0' : 'rounded-md'
                }`}
              >
                <code className="truncate" title={seg.texto}>
                  {seg.texto}
                </code>
                {motivo !== null && motivo !== 'desconocido' && (
                  <span
                    className="text-[var(--text-dim)] shrink-0"
                    title={EXPLICACION_MOTIVO[motivo] ?? ''}
                  >
                    <Info className="w-3 h-3" aria-hidden="true" />
                    <span className="sr-only">{EXPLICACION_MOTIVO[motivo] ?? ''}</span>
                  </span>
                )}
              </span>
              {sePuedeQuitar && (
                <button
                  type="button"
                  onClick={() => onQuitar(indice)}
                  disabled={deshabilitado}
                  // El nombre accesible incluye el argumento: en una lista de cinco chips
                  // es el único modo de saber cuál se va a borrar.
                  aria-label={`Quitar «${seg.texto}»`}
                  title={`Quitar «${seg.texto}» del comando`}
                  className="px-2 rounded-r-md bg-[var(--bg)] border border-[var(--border)] text-[var(--text-dim)] hover:text-[var(--error)] hover:border-error/40 hover:bg-[var(--error-bg)] transition-colors duration-150 disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  <Trash2 className="w-3.5 h-3.5" aria-hidden="true" />
                </button>
              )}
            </li>
          );
        })}
      </ul>

      <p className="mt-3 flex items-start gap-1.5 text-[11px] text-[var(--text-dim)] leading-relaxed">
        <AlertTriangle className="w-3 h-3 shrink-0 mt-0.5" aria-hidden="true" />
        <span>
          «Desconocido» no significa «incorrecto»: es lo que Thoriapp no reconoce, y por eso
          no puede quitarlo sin riesgo. Si necesitas cambiarlos, usa la edición manual.
        </span>
      </p>
    </section>
  );
}

/** Cómo explicar cada motivo de conservación en una frase. */
const EXPLICACION_MOTIVO: Record<string, string> = {
  desconocido: 'Thoriapp no reconoce este argumento, pero lo conserva.',
  ambiguo: 'Encaja con más de un ajuste del catálogo: para no tocar el equivocado, se deja.',
  duplicado: 'Ya aparece antes en el comando, así que este segundo no se gestiona.',
  no_seguro: 'Hay comillas sin cerrar u otra cosa que impide editar esta parte con seguridad.',
  arg_juego: 'Es un argumento del propio juego: Thoriapp nunca lo toca.',
};
