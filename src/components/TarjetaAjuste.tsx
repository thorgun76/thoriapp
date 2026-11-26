import { AlertTriangle, Check, GitMerge, HelpCircle, Info, MinusCircle } from 'lucide-react';
import type { EstadoAjuste } from '../lib/motorComando';
import type { Param } from '../lib/types';
import { claseDeAjuste, ETIQUETA_CLASE } from '../lib/gameView';
import { Toggle } from './Toggle';

/**
 * Tarjeta de un ajuste, con el estado real que dice el motor.
 *
 * ## Por qué no es un booleano
 *
 * «Activado / desactivado» es mentira en tres de los cinco casos:
 *
 * - **Parcial**: el ajuste aparece dos veces y solo una es gestionable. Activarlo deja una
 *   aparición viva que Thoriapp no controla. Pintarlo como apagado invita a pulsar y creer
 *   que se ha quitado.
 * - **Ambiguo**: el mismo texto encaja con dos ajustes del catálogo. El toggle no puede
 *   saber cuál.
 * - **Conflicto**: hay *otro* valor de la misma variable (`DXVK_HUD=fps` frente a
 *   `DXVK_HUD=full`). Activarlo crea una contradicción que el motor rechazaría.
 *
 * El estado llega de Rust en `EstadoAjuste`; aquí solo se pinta y se traduce a lenguaje
 * llano. Ninguna regla de las cinco se reimplementa en TS.
 */
export function TarjetaAjuste({
  param,
  indice,
  estado,
  encendido,
  accionable,
  pendiente,
  onCambiar,
  /** Se llama cuando el ajuste no se puede cambiar y el usuario quiere saber por qué. */
  onExplicar,
  requisito = null,
  motivoNoAccionable = null,
}: {
  param: Param;
  indice: number;
  /** Estado del motor. `null` si aún no hay análisis. */
  estado: EstadoAjuste | null;
  /** ¿Está puesto según el motor? Parcial cuenta como puesto. */
  encendido: boolean;
  /** ¿Se puede pulsar? `ambiguo` no. */
  accionable: boolean;
  /** Hay una operación en vuelo. La tarjeta se ve «pending» pero no se bloquea. */
  pendiente: boolean;
  onCambiar: (indice: number, activo: boolean) => void;
  onExplicar?: (indice: number) => void;
  /**
   * Por qué el ajuste no se puede usar en este equipo («Necesita una GPU AMD»), o `null`.
   *
   * Se enseña en la tarjeta: dejarla en gris sin decir el motivo es lo que hacía pensar
   * que el ajuste estaba roto en vez de que falta un requisito.
   */
  requisito?: string | null;
  /**
   * Por qué el ajuste está ahí pero no se puede activar ahora («Solo se aplica con
   * Proton»), o `null`.
   *
   * Se distingue de `requisito` porque no es un fallo del equipo: es que no aplica a este
   * modo de ejecución. Se pinta en tono de aviso, no de error. Si el ajuste **ya está** en el
   * comando se explica igual, para que el usuario sepa que puede no surtir efecto, pero no se
   * bloquea: quitar algo que él importó sería cambiar su comando sin permiso.
   */
  motivoNoAccionable?: string | null;
}) {
  const tipo = estado?.estado ?? 'inactivo';
  const aviso = EXPLICACION[tipo];
  // `simple_why` primero: es la descripción breve escrita para todo el mundo. El detalle
  // técnico (`why`) sigue disponible en el «por qué» y en el `warn`, así que no se pierde
  // nada al dejar de haber un modo experto que lo mostrara.
  const descripcion = param.simple_why || param.why;
  const clase = claseDeAjuste(param);

  // El botón que hace de tarjeta solo cuando no hay controles dentro. Con un botón
  // anidado (el «por qué») el HTML sería inválido y el lector de pantalla leería dos
  // botones superpuestos, así que en ese caso se usa un `div` con `role="switch"`.
  const tieneDetalle =
    tipo === 'ambiguo' || tipo === 'conflicto' || tipo === 'parcial';

  return (
    <div
      className={`group relative flex items-start gap-3 p-3.5 rounded-xl border transition-colors duration-150 ${
        encendido
          ? 'bg-[var(--card)] border-action/45'
          : 'bg-[var(--surface)] border-[var(--border)] hover:border-[var(--border-hover)]'
      } ${!accionable ? 'opacity-80' : ''}`}
      // `aria-busy` en la tarjeta y no en el toggle: el estado pendiente es de la
      // operación, no de un control concreto.
      aria-busy={pendiente || undefined}
      data-estado={tipo}
    >
      <div className="min-w-0 flex-1">
        {/* Primera línea: nombre + indicador de estado, SIEMPRE en la misma posición.
            Antes era `flex-wrap` + `items-baseline`: con el nombre largo, el indicador
            caía a la línea siguiente en unos estados y no en otros («Desactivado» es más
            ancho que «Activo»), así que al pulsar el toggle la etiqueta saltaba de sitio y
            la tarjeta cambiaba de alto. Ahora no hay `wrap` en esta línea y el indicador
            tiene ancho reservado, así que el cambio de estado no mueve nada. */}
        <div className="flex items-center gap-2">
          <h5 className="text-sm font-semibold text-[var(--text)] leading-tight truncate">
            {param.title || param.value}
          </h5>
          {/* El estado va con texto, no solo con color: quien no distingue el morado del
              gris tiene que poder leer qué pasa. */}
          <EstadoEtiqueta tipo={tipo} encendido={encendido} />
        </div>
        <div className="mt-1 flex items-center gap-1.5 min-w-0">
          <code className="text-[11px] font-mono text-[var(--text-dim)] truncate">
            {param.value}
          </code>
          {/* Qué clase de pieza es: no es lo mismo una variable que un envoltorio. */}
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg)] text-[var(--text-dim)] border border-[var(--border)]">
            {TIPO_PIEZA[param.kind] ?? 'ajuste'}
          </span>
          {/* Solo se etiqueta lo que necesita una advertencia: diagnóstico, riesgo o
              experimental. El resto de ajustes no lleva chip, para no llenar la lista de
              etiquetas que no aportan. Sale de los metadatos que ya había (`impact`,
              `experimental`, `warn`), no de una clasificación nueva. */}
          {clase && (
            <span
              data-clase={clase}
              className={`text-[10px] px-1.5 py-0.5 rounded border ${COLOR_CLASE[clase]}`}
              title={EXPLICACION_CLASE[clase]}
            >
              {ETIQUETA_CLASE[clase]}
            </span>
          )}
        </div>

        {requisito && (
          <p className="mt-1 flex items-start gap-1.5 text-[11px] text-[var(--error)] leading-snug">
            <AlertTriangle className="w-3 h-3 shrink-0 mt-0.5" aria-hidden="true" />
            {requisito}
          </p>
        )}

        {motivoNoAccionable && (
          <p className="mt-1 flex items-start gap-1.5 text-[11px] text-[var(--caution)] leading-snug">
            <Info className="w-3 h-3 shrink-0 mt-0.5" aria-hidden="true" />
            {motivoNoAccionable}
          </p>
        )}

        {descripcion && (
          <p className="mt-1 text-xs text-[var(--text-muted)] leading-relaxed line-clamp-2">
            {descripcion}
          </p>
        )}

        {/* El aviso del catálogo (`warn`) es información de seguridad que ya existía: no
            se pierde al cambiar el aspecto. */}
        {param.warn && (
          <p className="mt-1 flex items-start gap-1.5 text-[11px] text-[var(--caution)] leading-snug">
            <AlertTriangle className="w-3 h-3 shrink-0 mt-0.5" aria-hidden="true" />
            {param.warn}
          </p>
        )}

        {aviso && (
          <div className="mt-1.5 flex items-start gap-1.5">
            <aviso.Icono
              className={`w-3 h-3 shrink-0 mt-0.5 ${aviso.color}`}
              aria-hidden="true"
            />
            <p className={`text-[11px] leading-snug ${aviso.color}`}>{aviso.texto}</p>
            {tieneDetalle && onExplicar && (
              <button
                type="button"
                onClick={() => onExplicar(indice)}
                className="text-[11px] underline decoration-dotted underline-offset-2 text-[var(--text-muted)] hover:text-[var(--text)] shrink-0"
              >
                ver
              </button>
            )}
          </div>
        )}
      </div>

      <div className="pt-0.5 flex items-center shrink-0">
        {/* Aquí había un punto morado que latía mientras el motor respondía. Se quitó:
            aparecía a la izquierda del toggle en cada pulsación, y como el flex no reservaba
            su ancho (`shrink-0` faltaba y el punto se comprimía a 0) el toggle se corría un
            par de píxeles al activarlo. Además el aviso ya lo da el propio interruptor, que
            se mueve al instante con `conPedidos`; un parpadeo de 100 ms solo distraía. */}
        <Toggle
          checked={encendido}
          onChange={(next) => onCambiar(indice, next)}
          // El nombre accesible lleva el valor del ajuste: sin él, una lista de veinte
          // toggles se anuncia como veinte «interruptor» iguales.
          label={`${param.value}${ESTADO_ACCESIBLE[tipo]}`}
          // `pendiente` **no** deshabilita: deshabilitar pinta `opacity-50` y, como una
          // operación del motor dura lo que tarda el IPC, todas las tarjetas parpadeaban a
          // la vez en cada toggle. Un clic de más ya está cubierto por la cola del motor,
          // y el punto de progreso es lo que dice que hay algo en vuelo.
          disabled={!accionable}
          // `parcial` no es un simple «encendido»: se dice en el nombre accesible para que
          // quede claro que hay una aparición fuera de control.
          indeterminate={tipo === 'parcial'}
        />
      </div>
    </div>
  );
}

/**
 * Etiqueta corta del estado.
 *
 * Todas las variantes comparten la misma caja (`inline-flex`, icono de 3, `shrink-0` y el
 * mismo interletraje) para que cambiar de estado no mueva ni un píxel. `shrink-0` importa:
 * sin él, cuando el nombre del ajuste es largo el flex encoge la etiqueta y el texto se
 * parte en dos líneas, que es otra forma de que la altura de la tarjeta cambie al pulsar.
 */
function EstadoEtiqueta({ tipo, encendido }: { tipo: string; encendido: boolean }) {
  const base = 'flex items-center gap-1 shrink-0 text-[10px] uppercase tracking-wider whitespace-nowrap';

  if (tipo === 'activo') {
    return (
      <span className={`${base} text-[var(--compatible)]`}>
        <Check className="w-3 h-3" aria-hidden="true" />
        Activo
      </span>
    );
  }
  if (tipo === 'parcial') {
    return (
      <span className={`${base} text-[var(--caution)]`}>
        <GitMerge className="w-3 h-3" aria-hidden="true" />
        Parcial
      </span>
    );
  }
  if (tipo === 'ambiguo') {
    return (
      <span className={`${base} text-[var(--caution)]`}>
        <HelpCircle className="w-3 h-3" aria-hidden="true" />
        Ambiguo
      </span>
    );
  }
  if (tipo === 'conflicto') {
    return (
      <span className={`${base} text-[var(--error)]`}>
        <AlertTriangle className="w-3 h-3" aria-hidden="true" />
        Conflicto
      </span>
    );
  }
  // Inactivo: mismo sitio que los demás, en gris secundario.
  return (
    <span className={`${base} text-[var(--text-dim)]`}>
      <MinusCircle className="w-3 h-3" aria-hidden="true" />
      {encendido ? 'Activo' : 'Desactivado'}
    </span>
  );
}

/**
 * Explicación de cada situación en lenguaje llano.
 *
 * Un usuario que ve «Parcial» no sabe qué hacer; uno que lee «sigue puesto en otra parte
 * del comando y Thoriapp no lo controla ahí» sí. Ninguna de estas frases decide nada: solo
 * describen lo que el motor ya ha determinado.
 */
const EXPLICACION: Record<
  string,
  { texto: string; Icono: typeof AlertTriangle; color: string }
> = {
  parcial: {
    texto:
      'Sigue puesto en otra parte del comando, donde Thoriapp no lo controla. Desactivarlo aquí no lo quita del todo.',
    Icono: GitMerge,
    color: 'text-[var(--caution)]',
  },
  ambiguo: {
    texto:
      'Ese texto encaja con más de un ajuste del catálogo. Thoriapp no sabe cuál es y no lo cambia solo.',
    Icono: HelpCircle,
    color: 'text-[var(--caution)]',
  },
  conflicto: {
    texto:
      'El comando ya tiene otro valor de la misma variable. Thoriapp no lo sustituye sin avisar.',
    Icono: AlertTriangle,
    color: 'text-[var(--error)]',
  },
};

/** Cómo se llama cada clase de pieza en el comando. */
/** Tono de cada clase. `riesgo` va en rojo porque advierte; el resto en ámbar/dim. */
const COLOR_CLASE: Record<Exclude<ReturnType<typeof claseDeAjuste>, null>, string> = {
  diagnostico: 'bg-[var(--bg)] text-[var(--text-muted)] border-[var(--border)]',
  riesgo: 'bg-[var(--error)]/10 text-[var(--error)] border-[var(--error)]/40',
  experimental: 'bg-[var(--caution)]/10 text-[var(--caution)] border-[var(--caution)]/40',
};

const EXPLICACION_CLASE: Record<Exclude<ReturnType<typeof claseDeAjuste>, null>, string> = {
  diagnostico: 'Sirve para medir o investigar, no para mejorar el rendimiento.',
  riesgo: 'Puede reducir el rendimiento o impedir que el juego arranque.',
  experimental: 'Evidencia limitada o depende de tu entorno.',
};

const TIPO_PIEZA: Record<string, string> = {
  env: 'variable',
  wrapper: 'envoltorio',
  arg: 'argumento',
};

/** Sufijo del nombre accesible según el estado. */
const ESTADO_ACCESIBLE: Record<string, string> = {
  activo: ', activo',
  inactivo: ', desactivado',
  parcial: ', activo en parte del comando',
  ambiguo: ', ambiguo: no se puede cambiar desde aquí',
  conflicto: ', en conflicto con otro valor del comando',
};
