/**
 * Interruptor de encendido/apagado, con el mismo aspecto que el del resto de la app
 * (el de "Editar comando manualmente").
 *
 * Va aparte de la fila que lo usa porque lo importante es que **sea idéntico** en todas
 * partes: un interruptor con otra forma o con otro tamaño lee como si fuera otra cosa.
 */
export function Toggle({
  checked,
  onChange,
  label,
  disabled,
  indeterminate = false,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  /**
   * Nombre accesible. Obligatorio: el interruptor no lleva texto dentro, así que sin
   * esto sería un botón anónimo en el orden de tabulación y en un lector de pantalla.
   */
  label: string;
  disabled?: boolean;
  /**
   * El ajuste está puesto **en parte** del comando: hay otra aparición que Thoriapp no
   * controla. Se pinta encendido (porque de verdad está en la línea) con una marca
   * distinta, y `aria-checked="mixed"` lo anuncia como parcial.
   *
   * No se usa para «cargando»: mientras el motor responde el toggle se queda pulsable
   * (deshabilitarlo lo teñía de `opacity-50` y hacía parpadear toda la lista) y la
   * tarjeta solo se marca con `aria-busy`, sin cambio visual.
   */
  indeterminate?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      disabled={disabled}
      className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--surface)] disabled:opacity-50 disabled:cursor-not-allowed ${
        indeterminate
          ? // Parcial: el mismo morado apagado, para que se lea «puesto» sin parecer
            // completo.
            'bg-action/60'
          : checked
            ? 'bg-[var(--action)]'
            : 'bg-[var(--border)]'
      }`}
      role="switch"
      aria-checked={indeterminate ? 'mixed' : checked}
      aria-label={label}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 ease-in-out ${
          checked ? 'translate-x-6' : 'translate-x-1'
        }`}
      />
    </button>
  );
}
