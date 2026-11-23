import { useId, type ReactNode } from 'react';
import { ExternalLink, Medal } from 'lucide-react';
import { tierDescription, tierLabel } from '../lib/format';
import { openExternal } from '../lib/links';
import { TuxIcon } from './icons/TuxIcon';

/**
 * Píldora base de las etiquetas informativas.
 *
 * Las dos etiquetas comparten exactamente estas clases (altura, padding, borde,
 * radio, tipografía y ancho) para que se lean como una pareja: mismo peso, misma
 * altura y mismo ancho, sin que una parezca más importante que la otra.
 */
const PILL =
  'flex w-full items-center justify-center gap-1.5 px-3 py-1.5 rounded-full border text-xs font-semibold leading-none whitespace-nowrap';

/**
 * Color del badge de ProtonDB según el nivel, de mejor a peor.
 *
 * Fondos oscuros semitransparentes (`--*-bg`) con borde del mismo tono: el nivel
 * se distingue por color, pero sin fondo saturado para que no compita con el
 * botón «Copiar comando». Un tier desconocido (ProtonDB añade valores nuevos) cae
 * en el rojo, que es el caso conservador.
 */
export function tierColor(tier: string): string {
  switch (tier.toLowerCase()) {
    case 'platinum':
      // Platino no es verde: es el metal más claro. Casi blanco con un halo
      // frío, para que se lea como el nivel más alto sin parecer «éxito verde».
      return 'bg-platinum/10 text-platinum border-platinum/40';
    case 'gold':
      return 'bg-[var(--caution-bg)] text-[var(--caution)] border-caution/40';
    case 'bronze':
      // Cobre apagado: se distingue de oro sin salir de la gama cálida.
      return 'bg-bronze/10 text-bronze border-bronze/50';
    case 'silver':
      // Un escalón por debajo del platino: gris, pero apagado.
      return 'bg-silver/10 text-silver border-silver/50';
    case 'pending':
      // Sin reportes suficientes no es «malo»: es «no se sabe». Por eso gris
      // neutro y no el rojo de `borked`.
      return 'bg-[var(--surface)] text-[var(--text-muted)] border-[var(--border)]';
    default:
      return 'bg-[var(--error-bg)] text-[var(--error)] border-error/40';
  }
}

/**
 * Tooltip de una etiqueta.
 *
 * Se pinta siempre en el DOM (oculto con `opacity-0`) y se enlaza con
 * `aria-describedby`: así el texto llega también a lectores de pantalla, cosa que
 * un tooltip montado solo bajo demanda no hace. Aparece al pasar el ratón y al
 * llegar con el teclado (`group-focus-within`).
 */
function Tip({ id, children }: { id: string; children: ReactNode }) {
  return (
    <span
      id={id}
      role="tooltip"
      className="pointer-events-none absolute right-0 top-full z-20 mt-2 w-60 rounded-lg border border-[var(--border-hover)] bg-[var(--card)] px-2.5 py-2 text-[11px] font-normal leading-snug text-[var(--text)] opacity-0 shadow-xl transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100"
    >
      {children}
    </span>
  );
}

/**
 * Etiquetas «Linux nativo» y «ProtonDB» de la cabecera del juego.
 *
 * Son dos cosas distintas y se pintan como dos cosas distintas:
 *
 * - **Linux nativo**: hay versión oficial para Linux. No dice nada sobre cómo
 *   funciona, por eso el icono no es un check (un check se lee como «verificado»
 *   y garantizaría un funcionamiento que no depende de la app).
 * - **ProtonDB**: valoración *de la comunidad* bajo Proton, con su nivel real
 *   (Platino/Oro/Plata/Bronce/No funciona). No es equivalente a lo anterior.
 *
 * Si hay `appid`, la etiqueta de ProtonDB es un botón que abre el análisis del
 * juego en protondb.com por el mismo camino que el resto de enlaces externos
 * (`openExternal` → Rust valida el dominio contra una allowlist).
 *
 * Cuando no hay nivel se pinta igualmente «ProtonDB · sin datos» en gris: antes la
 * etiqueta simplemente no aparecía, y eso se lee como «este juego no necesita
 * Proton» en vez de «la app no tiene el dato».
 */
export function GameBadges({
  native,
  tier,
  appid,
  name,
  onLink,
}: {
  native?: boolean;
  tier?: string | null;
  appid?: number;
  /** Nombre con el que la app ha buscado el juego en Steam (para explicar el «sin datos»). */
  name?: string;
  /**
   * Permite vincular a mano el juego con su ficha de Steam.
   *
   * Solo se ofrece cuando no hay AppID (si lo hay, el dato que falta es el nivel,
   * no la identidad) y la detección automática ha fallado: es la salida cuando la
   * biblioteca guarda un nombre que la tienda no reconoce.
   */
  onLink?: () => void;
}) {
  const nativeTipId = useId();
  const protonTipId = useId();

  const protonTooltip = (
    <>
      <strong className="block text-[var(--text)]">Valoración de la comunidad</strong>
      {tier ? tierDescription(tier) : 'Sin datos suficientes todavía.'}
      {appid ? (
        <span className="mt-1 block text-[var(--text-muted)]">
          Pulsa para abrir el análisis en ProtonDB.
        </span>
      ) : null}
    </>
  );

  return (
    // Una sola columna de rejilla: las dos píldoras se estiran al ancho de la más
    // larga, así quedan igualadas sin fijar un ancho a ojo que se rompería con
    // «ProtonDB · No funciona».
    <div className="grid gap-2 shrink-0">
      {native === true && (
        <span className="group relative block">
          <span
            // `tabIndex` para que el tooltip se pueda descubrir con el teclado:
            // la etiqueta no hace nada al pulsarla, pero sí explica algo.
            tabIndex={0}
            aria-describedby={nativeTipId}
            className={`${PILL} bg-[var(--compatible-bg)] text-[var(--compatible)] border-compatible/40 cursor-help focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-compatible/60`}
          >
            <TuxIcon className="w-3.5 h-3.5" />
            Linux nativo
          </span>
          <Tip id={nativeTipId}>
            <strong className="block text-[var(--text)]">Versión para Linux</strong>
            El juego tiene una versión oficial para Linux: se lanza sin Proton ni
            capas de traducción.
          </Tip>
        </span>
      )}

      {tier ? (
        <span className="group relative block">
          {appid ? (
            <button
              type="button"
              onClick={() => openExternal(`https://www.protondb.com/app/${appid}`)}
              aria-describedby={protonTipId}
              className={`${PILL} ${tierColor(tier)} cursor-pointer transition-colors hover:brightness-125 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-action/60`}
            >
              <Medal className="w-3.5 h-3.5" aria-hidden="true" />
              ProtonDB · {tierLabel(tier)}
              <ExternalLink className="w-3 h-3 opacity-70" aria-hidden="true" />
            </button>
          ) : (
            <span
              tabIndex={0}
              aria-describedby={protonTipId}
              className={`${PILL} ${tierColor(tier)} cursor-help focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-action/60`}
            >
              <Medal className="w-3.5 h-3.5" aria-hidden="true" />
              ProtonDB · {tierLabel(tier)}
            </span>
          )}
          <Tip id={protonTipId}>{protonTooltip}</Tip>
        </span>
      ) : (
        // Sin nivel conocido: se dice explícitamente en vez de esconder la
        // etiqueta. Gris neutro, para no robarla protagonismo al botón de copiar.
        <span className="group relative block">
          {onLink && !appid ? (
            <button
              type="button"
              onClick={onLink}
              aria-describedby={protonTipId}
              className={`${PILL} bg-[var(--surface)] text-[var(--text-muted)] border-[var(--border)] cursor-pointer transition-colors hover:border-action/50 hover:text-[var(--text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-action`}
            >
              <Medal className="w-3.5 h-3.5" aria-hidden="true" />
              ProtonDB · sin datos
            </button>
          ) : (
            <span
              tabIndex={0}
              aria-describedby={protonTipId}
              className={`${PILL} bg-[var(--surface)] text-[var(--text-muted)] border-[var(--border)] cursor-help focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-action/60`}
            >
              <Medal className="w-3.5 h-3.5" aria-hidden="true" />
              ProtonDB · sin datos
            </span>
          )}
          <Tip id={protonTipId}>
            <strong className="block text-[var(--text)]">ProtonDB sin datos</strong>
            {appid
              ? 'La app no ha podido leer el nivel de este juego en ProtonDB.'
              : `Steam no reconoce «${name || 'este juego'}», así que no hay AppID que consultar.`}
            {onLink && !appid ? (
              <span className="mt-1 block text-[var(--text-muted)]">
                Pulsa para buscar el juego en Steam y vincularlo.
              </span>
            ) : (
              <>
                {' '}No significa que no funcione: ProtonDB se llena con reportes de
                usuarios{appid ? ', y puede que todavía no haya ninguno' : ''}.
              </>
            )}
          </Tip>
        </span>
      )}
    </div>
  );
}
