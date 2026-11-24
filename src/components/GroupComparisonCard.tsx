import { AlertCircle, Check, Info, Scale } from 'lucide-react';
import type { GroupComparison } from '../lib/types';

/**
 * Resultado de comparar **grupos** de grabaciones (varias por configuración).
 *
 * Existe porque comparar una grabación contra otra no distingue una mejora real del
 * ruido del juego: dos partidas idénticas se separan fácilmente un 6 %. Con 3 o más
 * por lado el backend da un intervalo de confianza, y aquí se traduce a un veredicto
 * de tres estados en vez de dos: «no concluyente» es una respuesta válida.
 */

/** Icono y colores por veredicto, con el mismo lenguaje visual que el resto de la app. */
const STYLES: Record<string, { box: string; text: string; Icon: typeof Check; word: string }> = {
  mejora: {
    box: 'bg-compatible/10 border-compatible/40',
    text: 'text-[var(--compatible)]',
    Icon: Check,
    word: 'Mejora',
  },
  empeora: {
    box: 'bg-error/10 border-error/40',
    text: 'text-[var(--error)]',
    Icon: AlertCircle,
    word: 'Empeora',
  },
  no_concluyente: {
    box: 'bg-[var(--bg)] border-[var(--border)]',
    text: 'text-[var(--text-muted)]',
    Icon: Scale,
    word: 'No concluyente',
  },
};

const NOMBRES: Record<string, string> = {
  avg_fps: 'FPS promedio',
  fps1_percent_low: '1 % de los peores frames',
  fps01_percent_low: '0,1 % de los peores frames',
};

const fmt = (n: number, dec = 1) => n.toFixed(dec).replace('.', ',');

/** Envuelve el icono del veredicto para no repetir el tamaño y el `shrink-0`. */
function Icono({ Icon }: { Icon: typeof Check }) {
  return <Icon className="w-5 h-5 shrink-0" />;
}

export function GroupComparisonCard({
  rows,
  etiquetas,
}: {
  rows: GroupComparison[];
  /**
   * Nombres de los dos grupos. Aquí se comparan **varias** pruebas por lado, así que no
   * se puede hablar de una «Prueba N» concreta: las etiquetas dicen qué lado es cuál
   * («antes del cambio» / «después del cambio»), que es lo que de verdad representa.
   */
  etiquetas?: { antes: string; despues: string };
}) {
  const et = etiquetas ?? { antes: 'Antes', despues: 'Después' };
  if (!rows.length) return null;

  // El veredicto global es el del FPS promedio (es lo que la gente mide), pero los
  // percentiles se enseñan siempre: una mejora de promedio con peores 1 % lows es
  // precisamente el caso que un promedio esconde.
  const principal = rows.find((r) => r.metric === 'avg_fps') ?? rows[0];
  const style = STYLES[principal.verdict] ?? STYLES.no_concluyente;
  const runs = Math.min(principal.antes_runs, principal.despues_runs);
  const hayIntervalo = principal.verdict !== 'no_concluyente' || principal.ci_low_percent !== 0;

  return (
    <section className="mb-6 p-5 rounded-2xl bg-[var(--surface)] border border-[var(--border)]">
      <div className="flex items-start justify-between gap-4 mb-4">
        <div className="min-w-0">
          <p className="text-[11px] font-bold uppercase tracking-[0.14em] text-[var(--text-dim)] mb-1">
            Comparación de varias pruebas
          </p>
          <h3 className="text-lg font-bold text-[var(--text)]">
            ¿El cambio es real o es ruido?
          </h3>
          <p className="text-xs text-[var(--text-muted)] mt-1 leading-relaxed">
            {runs} grabación(es) por lado. Se compara la <strong>mediana</strong> de cada
            grupo, no la mejor ni la peor sesión.
          </p>
        </div>
      </div>

      <div className={`p-4 rounded-xl border ${style.box} mb-4`}>
        <p className={`flex items-center gap-2 font-bold ${style.text}`}>
          <Icono Icon={style.Icon} />
          {style.word}
          {hayIntervalo && (
            <span className="font-mono text-sm">
              {principal.change_percent >= 0 ? '+' : ''}
              {fmt(principal.change_percent)} %
            </span>
          )}
        </p>
        <p className="mt-2 text-xs text-[var(--text)] leading-relaxed">{principal.conclusion}</p>

        {hayIntervalo && (
          <p className="mt-2 text-[11px] text-[var(--text-muted)] leading-relaxed">
            Intervalo del 95 %: {fmt(principal.ci_low_percent)} % a{' '}
            {fmt(principal.ci_high_percent)} %. Solo se afirma una mejora si todo el
            intervalo queda por encima de ±{fmt(principal.required_margin_percent, 0)} %.
          </p>
        )}
      </div>

      {/* Los percentiles, uno por línea: son la otra mitad del resultado y una mejora
          de promedio con peores 1 % lows es justo lo que hay que ver. */}
      {rows.length > 1 && (
        <table className="w-full text-xs">
          <thead>
            <tr className="text-left text-[var(--text-dim)] uppercase tracking-wider text-[10px]">
              <th className="pb-2 font-semibold">Métrica</th>
              <th className="pb-2 font-semibold text-right">{et.antes}</th>
              <th className="pb-2 font-semibold text-right">{et.despues}</th>
              <th className="pb-2 font-semibold text-right">Cambio</th>
            </tr>
          </thead>
          <tbody className="text-[var(--text)]">
            {rows.map((r) => {
              const s = STYLES[r.verdict] ?? STYLES.no_concluyente;
              return (
                <tr key={r.metric} className="border-t border-[var(--border)]">
                  <td className="py-1.5">{NOMBRES[r.metric] ?? r.metric}</td>
                  <td className="py-1.5 text-right font-mono">{fmt(r.antes_median)}</td>
                  <td className="py-1.5 text-right font-mono">{fmt(r.despues_median)}</td>
                  <td className={`py-1.5 text-right font-mono font-semibold ${s.text}`}>
                    {r.change_percent >= 0 ? '+' : ''}
                    {fmt(r.change_percent)} %
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}

      {runs < 3 && (
        <p className="mt-3 text-[11px] text-[var(--text-muted)] flex items-start gap-1.5 leading-relaxed">
          <Info className="w-3.5 h-3.5 shrink-0 mt-0.5" />
          Con menos de 3 grabaciones por lado no se puede separar el efecto del cambio
          del ruido del juego. Graba la misma escena varias veces antes y después.
        </p>
      )}
    </section>
  );
}

/**
 * ¿Hay evidencia suficiente para que el veredicto de una sola comparación valga?
 *
 * La tarjeta de arriba (una grabación contra otra) sigue siendo útil como pista, pero
 * cuando el grupo dice «no concluyente» no puede quedarse un «Gana la grabación
 * nueva» al lado: el usuario leería solo el titular optimista.
 */
export function hayEvidenciaDeGrupo(rows: GroupComparison[] | null | undefined): boolean {
  if (!rows || !rows.length) return true; // sin datos de grupo no se toca nada
  return rows.some((r) => r.verdict === 'mejora' || r.verdict === 'empeora');
}
