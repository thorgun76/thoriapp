import { useMemo, useState } from 'react';
import {
  ArrowLeft, Trash2, Copy, Terminal, ChevronDown, Info,
  AlertCircle, TrendingUp, TrendingDown, Minus, HelpCircle,
  ThumbsUp, ThumbsDown, Scale, Check, Crown, Swords, ArrowDownRight,
} from 'lucide-react';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import { nombreLegible, etiquetasDe, type Etiquetas } from '../lib/etiquetas';
import type { EvaluacionPersistida, SavedComparison } from '../lib/api';
import {
  esLegible,
  evaluacionInterpretable,
  formatearDiferencia,
  presentarGuardada,
  textoDeDireccion,
  textoDePreferencia,
  textoDeCompatibilidad,
  textoDeEtiqueta,
  type EstadoDeLectura,
  type MetricaPresentada,
  type ModeloDePresentacion,
  type Tono,
} from '../lib/presentacionEvaluacion';
import { estabilidadDe, estabilidadLabel } from '../lib/estabilidad';
import { textoDeDuracion, duraMenosDe } from '../lib/duracion';
import { GraficoTemporal } from './GraficoTemporal';
import { PruebaN } from './PruebaN';
import { InstantaneaCard } from './InstantaneaCard';

/* ─────────────────────────────── Estilos ─────────────────────────────── */

/**
 * Color de cada tono. Los colores de las series (antes/después) NO están aquí: van
 * fijos en {@link SERIE} para que no cambien según quién gane, que era una de las
 * formas de confundir al usuario.
 */
const TONO: Record<Tono, { caja: string; icono: string; texto: string; Icon: typeof ThumbsUp }> = {
  positivo: { caja: 'border-compatible/60 bg-compatible/10', icono: 'bg-compatible/20', texto: 'text-[var(--compatible)]', Icon: ThumbsUp },
  negativo: { caja: 'border-error/60 bg-error/10',           icono: 'bg-error/20',      texto: 'text-[var(--error)]',      Icon: ThumbsDown },
  aviso:    { caja: 'border-caution/60 bg-caution/10',       icono: 'bg-caution/20',    texto: 'text-[var(--caution)]',    Icon: Scale },
  neutro:   { caja: 'border-[var(--border)] bg-[var(--bg)]', icono: 'bg-border/40',     texto: 'text-[var(--text-muted)]', Icon: Minus },
};

/** Colores fijos de las dos series. Nunca dependen del resultado. */
const SERIE = {
  antes: { texto: 'text-[#38bdf8]', barra: 'bg-[#38bdf8]', hex: '#38bdf8' },
  despues: { texto: 'text-[var(--caution)]', barra: 'bg-[var(--caution)]', hex: 'var(--caution)' },
} as const;

/**
 * **Sin `CAMBIO_STYLE`.** Había un chip verde «Mejor» / rojo «Peor» que se pintaba a
 * partir de si la cifra había subido (H2): un rojo en «Tiempo por imagen» sugería que era
 * peor cuando en realidad bajaba el tiempo. Ahora el color de una métrica sale **solo** de
 * la preferencia que estableció el motor, y si no la estableció no hay color.
 */
const PREFERENCIA_STYLE = {
  favor_de_candidata: {
    chip: 'bg-compatible/15 text-[var(--compatible)]',
    Icon: TrendingUp,
    word: 'A favor de la nueva',
  },
  favor_de_referencia: {
    chip: 'bg-error/15 text-[var(--error)]',
    Icon: TrendingDown,
    word: 'A favor de la anterior',
  },
  indistinto: {
    chip: 'bg-border/40 text-[var(--text-muted)]',
    Icon: Minus,
    word: 'Indistinto',
  },
  no_establecida: {
    chip: 'bg-border/40 text-[var(--text-dim)]',
    Icon: HelpCircle,
    word: 'Sin preferencia',
  },
} as const;

type PreferenciaConocida = keyof typeof PREFERENCIA_STYLE;

const esPreferenciaConocida = (c: string | undefined): c is PreferenciaConocida =>
  c != null && c in PREFERENCIA_STYLE;

const f = (n: number | null, d = 1) => (n == null ? '—' : n.toFixed(d));

/* ─────────────────────────── Piezas pequeñas ─────────────────────────── */

/** Barras de dos series con la MISMA escala (0..max de las dos). */
function Barras({
  antes,
  despues,
  max,
  unidad,
  decimals,
  etiquetas,
  escalaAbsoluta,
}: {
  /** I: cuando se fija, las barras usan esa escala (0-100) en vez de la relativa al máximo. */
  escalaAbsoluta?: number;
  antes: number | null;
  despues: number | null;
  max: number;
  unidad: string;
  decimals: number;
  /** «Prueba N» cuando la grabación tiene número; «Antes»/«Después» si es antigua. */
  etiquetas: Etiquetas;
}) {
  // I: con precisión 0 (escalas absolutas como la estabilidad) el número visible es el
  // redondeado, y la barra tiene que medir **esa** cifra. Si mide el valor sin redondear,
  // la etiqueta dice «77» y la barra queda al 76,67 %: dos lecturas distintas del mismo
  // dato en la misma línea.
  const mostrado = (v: number) => (decimals === 0 ? Math.round(v) : v);
  const pct = (v: number | null) => {
    if (v == null) return '0%';
    // La estabilidad es un índice 0-100 y así se dibuja en todas las superficies. Antes se
    // dividía por el máximo de las dos grabaciones, así que 73 contra 62 salía como 100 %
    // y 85 %, y la misma cifra se veía más llena aquí que en la vista previa.
    const base = escalaAbsoluta ?? max;
    if (base <= 0) return '0%';
    return `${Math.max(2, Math.min(100, (mostrado(v) / base) * 100))}%`;
  };
  const fila = (v: number | null, serie: { texto: string; barra: string }, nombre: string) => (
    <div>
      <div className="flex justify-between items-baseline text-xs mb-1 gap-2">
        <span className={`font-semibold ${serie.texto}`}>{nombre}</span>
        <span className="font-bold text-[var(--text)] font-mono tabular-nums">
          {f(v, decimals)}{unidad}
        </span>
      </div>
      <div
        className="h-2.5 rounded-full bg-[var(--surface)] overflow-hidden"
        role="img"
        aria-label={`${nombre}: ${f(v, decimals)}${unidad}`}
      >
        <div className={`h-full rounded-full motion-safe:transition-all motion-safe:duration-500 ${serie.barra}`} style={{ width: pct(v) }} />
      </div>
    </div>
  );
  return (
    <div className="space-y-3">
      {fila(antes, SERIE.antes, etiquetas.antes)}
      {fila(despues, SERIE.despues, etiquetas.despues)}
    </div>
  );
}

/**
 * Tarjeta de una métrica destacada (FPS medios o estabilidad).
 *
 * Se conservan solo estas dos porque son las que el usuario compara de un vistazo; el
 * resto de medidas vive en una tabla compacta más abajo.
 *
 * Acepta **dos formas** de lo mismo, y por eso se distinguen:
 *
 *  - `evaluada`: métrica con `preferencia` del motor. Solo entonces puede llevar chip, y
 *    el chip habla de la preferencia de **esa métrica**, nunca de la comparación.
 *  - `descriptiva`: índice de estabilidad. El motor no lo evalúa (no hay clave de
 *    estabilidad en Rust), así que **no** lleva chip de ninguna clase (H3 arreglado: antes
 *    la tarjeta «Estabilidad» enseñaba el 1 % low rotulado como estabilidad y además
 *    decidía «Mejor»/«Peor» por el signo).
 */
function TarjetaMetrica({
  metrica,
  modo,
  etiquetas,
}: {
  metrica:
    | { tipo: 'evaluada'; datos: MetricaPresentada }
    | { tipo: 'descriptiva'; etiqueta: string; antes: number | null; despues: number | null };
  modo: 'fps' | 'estabilidad';
  etiquetas: Etiquetas;
}) {
  const esEstabilidad = modo === 'estabilidad';
  const antes = metrica.tipo === 'evaluada' ? metrica.datos.antes : metrica.antes;
  const despues = metrica.tipo === 'evaluada' ? metrica.datos.despues : metrica.despues;
  const etiqueta = metrica.tipo === 'evaluada'
    ? textoDeEtiqueta(metrica.datos.nombre)
    : metrica.etiqueta;

  // La preferencia, si existe, es la del motor. Un código desconocido no se pinta con
  // colores: no se sabe qué significa.
  const codigoPreferencia = metrica.tipo === 'evaluada'
    ? metrica.datos.preferencia?.codigo
    : undefined;
  const st = esPreferenciaConocida(codigoPreferencia) ? PREFERENCIA_STYLE[codigoPreferencia] : null;

  const maxFps = Math.max(antes ?? 0, despues ?? 0, 1);
  const maxEst = 100;

  return (
    <div className="p-4 rounded-xl bg-[var(--bg)] border border-[var(--border)] min-w-0">
      <div className="flex items-start justify-between gap-2 mb-1">
        <h4 className="text-sm font-bold text-[var(--text)] flex items-center gap-2 min-w-0">
          {esEstabilidad ? <Scale className="w-4 h-4 text-[var(--action)] shrink-0" aria-hidden="true" /> : <TrendingUp className="w-4 h-4 text-[var(--action)] shrink-0" aria-hidden="true" />}
          <span className="truncate">{etiqueta}</span>
        </h4>
        {/* Sin preferencia no hay chip: no se inventa un «Mejor» ni un «Igual». */}
        {st && (
          <span className={`text-[11px] font-bold px-2 py-0.5 rounded-md whitespace-nowrap flex items-center gap-1 ${st.chip}`}>
            <st.Icon className="w-3 h-3" aria-hidden="true" />
            {st.word}
          </span>
        )}
      </div>

      {esEstabilidad && antes != null && despues != null && (
        <p className="text-[11px] text-[var(--text-dim)] mb-3">
          {estabilidadLabel(antes)} → {estabilidadLabel(despues)}
        </p>
      )}
      {esEstabilidad && (
        <p className="text-[11px] text-[var(--text-dim)] mb-3">
          Índice descriptivo sobre 100. No se evalúa: no dice si la diferencia es real ni
          cuál de las dos pruebas es mejor.
        </p>
      )}

      <Barras
        antes={antes}
        despues={despues}
        max={esEstabilidad ? maxEst : maxFps}
        /* I: estabilidad siempre sobre 0-100, igual que en la vista previa. */
        escalaAbsoluta={esEstabilidad ? 100 : undefined}
        unidad={esEstabilidad ? '' : ' FPS'}
        decimals={esEstabilidad ? 0 : 1}
        etiquetas={etiquetas}
      />
    </div>
  );
}

/**
 * Unidad de la diferencia entre las dos grabaciones.
 *
 * No todo se mide en «%»:
 *  - `pct`     → variación relativa (FPS, 1 % low…), compara contra el valor inicial.
 *  - `pp`      → puntos porcentuales (un 4,2 % → 5,1 % del tiempo es +0,9 pp, no +21 %).
 *  - `puntos`  → puntos del índice de estabilidad, que ya va sobre 100.
 *  - `abs`     → diferencia absoluta; solo cuando el valor inicial es casi cero y un
 *                porcentaje mentiría (0,0 % → 1,0 % de tiempo bajo 30 FPS).
 */
type Unidad = 'pct' | 'pp' | 'puntos' | 'abs';

/**
 * C3-22: el texto sale de `formatearDiferencia`, que normaliza el cero **después** del
 * redondeo. Antes se concatenaba el signo a mano sobre `toFixed`, y una diferencia de
 * `-0.04` puntos producía «-0.0 pts»: un cero con signo que afirma una dirección que el
 * número no tiene. `signo` se sigue devolviendo porque la vista lo usa para ordenar, pero
 * **ya no decide el color**.
 */
export const diferencia = (
  antes: number,
  despues: number,
  unidad: Unidad,
  decimals: number
): { texto: string; signo: number } => {
  // I: la diferencia sale **de los valores originales**, nunca de cifras ya redondeadas:
  // restar redondeos inventa diferencias que los datos no tienen (−10,7 se convertiría en
  // −11 y 0,8 en 0). El problema estaba en la **presentación**: con precisión 0 el texto
  // «62,49 → 62,51» se pintaba «62 → 63» junto a «0 pts», negando el punto que el lector
  // ve subir. Se conserva el valor original y se muestra con un decimal, que es la
  // precisión de las demás diferencias; así el número es el real y no parece un redondeo.
  // `toPrecision` da el orden de magnitud real; `Number` limpia el ruido de coma flotante
  // (62,51 − 62,49 = 0,01999999999999602).
  const delta = Number((despues - antes).toPrecision(12));
  // Precisión de la diferencia: la de la columna, salvo que redondearla la convierta en
  // cero —«62,49 → 62,51» daría «0.0 pts»— o que el valor sea tan pequeño que valga dos
  // decimales. En ese caso se usan los decimales justos para escribir el valor real.
  const decimalesDiferencia = (() => {
    if (delta === 0) return decimals; // un cero es un cero, sin decimales de adorno
    if (decimals > 0) return decimals;
    const m = Math.abs(delta);
    // La precisión sigue a la magnitud: −10,7 con un decimal; 0,8 con uno; 0,02 con dos.
    // Así el número que se lee es el valor real y no un cero por redondeo.
    return m >= 0.1 ? 1 : 2;
  })();
  if (unidad === 'pct' || unidad === 'abs') {
    if (unidad === 'abs' || Math.abs(antes) < 0.5) {
      return { texto: formatearDiferencia(delta, decimalesDiferencia) ?? '—', signo: delta };
    }
    const rel = (delta / Math.abs(antes)) * 100;
    return { texto: formatearDiferencia(rel, 1, ' %') ?? '—', signo: rel };
  }
  const sufijo = unidad === 'pp' ? ' pp' : ' pts';
  return { texto: formatearDiferencia(delta, decimalesDiferencia, sufijo) ?? '—', signo: delta };
};

/**
 * Tabla única con todas las medidas de la comparación.
 *
 * Una fila por métrica, sin cajas ni explicaciones repetidas: solo la medida, los dos
 * valores, la diferencia con su unidad real y quién gana según el intérprete. El color
 * del ganador es el del INTÉRPRETE (no el de la variación), así una métrica que empeora
 * porque sube el tiempo por imagen no se pinta de verde.
 */
function TablaMetricas({
  comparacion,
  etiquetas,
  modelo,
  interpretable,
  lectura,
}: {
  comparacion: BenchmarkComparison;
  etiquetas: Etiquetas;
  /** Modelo compartido: aporta la preferencia de Rust y el estado de cada métrica. */
  modelo: ModeloDePresentacion;
  /** C: `false` si la evaluación no se puede interpretar (no soportada / malformada). */
  interpretable: boolean;
  /** A: **por qué** no se puede leer. Ausente no es lo mismo que ilegible. */
  lectura: EstadoDeLectura;
}) {
  const b = comparacion.baseline;
  const t = comparacion.test;

  // Las métricas evaluadas vienen del modelo; la estabilidad no está allí porque Rust no la
  // evalúa, así que se añade como fila **descriptiva** y nunca recibe preferencia.
  const porClave = new Map(modelo.metricas.map((m) => [m.clave, m]));

  const filas: {
    clave: string;
    label: string;
    descripcion: string;
    antes: number | null;
    despues: number | null;
    decimals: number;
    sufijo: string;
    unidad: Unidad;
    /** Solo para la fila descriptiva: el motor no tiene clave de estabilidad. */
    descriptiva: boolean;
  }[] = [
    {
      clave: 'avg_fps', label: 'FPS medios', descripcion: 'Imágenes por segundo de media',
      antes: b.avg_fps, despues: t.avg_fps, decimals: 1, sufijo: '', unidad: 'pct', descriptiva: false,
    },
    {
      clave: 'frame_time_avg', label: 'Tiempo por imagen', descripcion: 'Milisegundos por imagen',
      antes: b.frame_time_avg, despues: t.frame_time_avg, decimals: 2, sufijo: ' ms', unidad: 'pct', descriptiva: false,
    },
    {
      clave: 'estabilidad', label: 'Estabilidad', descripcion: 'Índice descriptivo sobre 100',
      antes: estabilidadDe(b), despues: estabilidadDe(t),
      decimals: 0, sufijo: '', unidad: 'puntos', descriptiva: true,
    },
    {
      clave: 'fps_1_percent_low', label: '1 % low', descripcion: 'Peor 1 % de imágenes',
      antes: b.fps_1_percent_low ?? null, despues: t.fps_1_percent_low ?? null, decimals: 1, sufijo: '', unidad: 'pct', descriptiva: false,
    },
    {
      clave: 'fps_0_1_percent_low', label: '0,1 % low', descripcion: 'Peor 0,1 % de imágenes',
      antes: b.fps_0_1_percent_low ?? null, despues: t.fps_0_1_percent_low ?? null, decimals: 1, sufijo: '', unidad: 'pct', descriptiva: false,
    },
  ];

  return (
    <div className="rounded-xl border border-[var(--border)] overflow-hidden">
      <table className="w-full text-sm">
        <caption className="sr-only">
          Todas las métricas comparadas entre {etiquetas.antes} y {etiquetas.despues}
        </caption>
        <thead>
          <tr className="bg-[var(--bg)] text-[11px] uppercase tracking-wide">
            <th scope="col" className="text-left font-semibold text-[var(--text-dim)] px-3 py-2">Métrica</th>
            <th scope="col" className="text-right font-semibold text-[#38bdf8] px-3 py-2 whitespace-nowrap">{etiquetas.antes}</th>
            <th scope="col" className="text-right font-semibold text-[var(--caution)] px-3 py-2 whitespace-nowrap">{etiquetas.despues}</th>
            <th scope="col" className="text-right font-semibold text-[var(--text-dim)] px-3 py-2">Cambio</th>
            <th scope="col" className="text-left font-semibold text-[var(--text-dim)] px-3 py-2">Evaluación</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-[var(--border)]">
          {filas.map((m) => {
            const ev = m.descriptiva ? null : porClave.get(m.clave) ?? null;
            const d =
              m.antes != null && m.despues != null
                ? diferencia(m.antes, m.despues, m.unidad, m.decimals)
                : null;

            // El color NO sale del signo: sale de la preferencia que estableció Rust para
            // **esta** métrica. Sin preferencia —descriptiva, excluida o indistinta— no hay
            // verde ni rojo, aunque el número suba o baje. Es lo que el motor no afirmó.
            const color =
              ev == null || ev.estado === 'descriptiva' || ev.estado === 'no_evaluada' || ev.estado === 'indistinta'
                ? 'text-[var(--text-muted)]'
                : ev.estado === 'con_preferencia'
                ? 'text-[var(--text)] font-semibold'
                : 'text-[var(--text-muted)]';
            const marca =
              ev == null || ev.estado === 'descriptiva'
                ? null
                : ev.estado === 'con_preferencia'
                ? 'text-[var(--compatible)]'
                : ev.estado === 'no_evaluada'
                ? 'text-[var(--caution)]'
                : 'text-[var(--text-dim)]';

            return (
              <tr key={m.clave} className="hover:bg-[var(--bg)]/60 transition-colors">
                <th scope="row" className="text-left px-3 py-2 font-normal">
                  <span className="text-[var(--text)] font-medium">{m.label}</span>
                  <span className="block text-[11px] text-[var(--text-dim)]">{m.descripcion}</span>
                </th>
                <td className="px-3 py-2 text-right font-mono tabular-nums text-[var(--text-muted)]">
                  {f(m.antes, m.decimals)}{m.sufijo}
                </td>
                <td className="px-3 py-2 text-right font-mono tabular-nums text-[var(--text-muted)]">
                  {f(m.despues, m.decimals)}{m.sufijo}
                </td>
                <td className={`px-3 py-2 text-right font-mono tabular-nums whitespace-nowrap ${color}`}>
                  {d ? d.texto : '—'}
                </td>
                <td className={`px-3 py-2 text-[11px] ${marca ?? 'text-[var(--text-dim)]'}`}>
                  {m.descriptiva
                    ? 'Descriptiva: no se evalúa'
                    : /* C3-06 / A: con una evaluación que no se puede leer no se afirma
                         qué consta en ella. Pero cada motivo se nombra por lo que es, no
                         todos como «no interpretable»: una comparación histórica **no
                         tiene** evaluación, y decir que no se puede interpretar atribuye
                         incompatibilidad a algo que nunca se registró. */
                    textoDeEstadoDeLaMetrica(lectura, () =>
                      ev == null
                        ? 'No consta en la evaluación'
                        : etiquetaDeEstadoDeMetrica(ev, etiquetas, interpretable)
                    )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/**
 * Cómo se nombra cada situación de una métrica. Cuatro estados, no dos.
 *
 * El nombre de la grabación lo pone la vista (`etiquetas`), no el código de preferencia:
 * «favor_de_candidata» es un identificador interno y no se enseña como si fuera una frase.
 */
function etiquetaDeEstadoDeMetrica(
  m: MetricaPresentada,
  etiquetas: Etiquetas,
  /**
   * C: si la evaluación del registro se puede interpretar.
   *
   * Cuando no lo es (no soportada o malformada), la tabla solo puede describir cifras: no
   * consta que una métrica careciera de apoyo, ni que se evaluara, ni qué prefirió el motor,
   * porque la evaluación no se puede leer. Afirmarlo sería inventar el motivo del silencio.
   */
  interpretable: boolean
): string {
  if (!interpretable) return 'Descriptiva: la evaluación no se puede interpretar';
  switch (m.estado) {
    case 'con_preferencia':
      return m.preferencia?.codigo === 'favor_de_referencia'
        ? `Favorece a «${etiquetas.antes}»`
        : `Favorece a «${etiquetas.despues}»`;
    case 'indistinta':
      return 'Evaluada: sin diferencia apreciable';
    case 'no_evaluada':
      // La ausencia de apoyo es ausencia: no se atribuye a un lado que el motor no señaló.
      return m.apoyo == null
        ? 'No evaluada: sin apoyo declarado'
        : 'No evaluada: apoyo insuficiente';
    case 'descriptiva':
      return 'Descriptiva: no se evalúa';
  }
}

/**
 * A: qué decir de una métrica cuando la evaluación no se puede leer.
 *
 * Los tres motivos son distintos y el usuario los distingue:
 * - `ausente`      — no hay evaluación registrada.
 * - `malformada`   — hay registro, pero no es legible.
 * - `no_soportada` — hay evaluación, pero de una metodología que esta versión no lee.
 *
 * Con evaluación legible se delega en la presentación normal del contrato.
 */
function textoDeEstadoDeLaMetrica(lectura: EstadoDeLectura, legible: () => string): string {
  switch (lectura) {
    case 'ausente':
      return 'Sin evaluación registrada: solo se muestran las cifras medidas';
    case 'malformada':
      return 'El registro de la evaluación no se puede leer';
    case 'no_soportada':
      return 'Evaluación de una metodología que esta versión no interpreta';
    case 'soportada':
      return legible();
  }
}

/**
 * A: la cabecera de la tabla tampoco puede decir lo mismo para los tres motivos.
 * Decía «la evaluación no se puede interpretar» también a una comparación histórica, que
 * no tiene evaluación ninguna.
 */
function textoDeCabeceraNoLegible(lectura: EstadoDeLectura): string {
  if (esLegible(lectura)) return '';
  switch (lectura) {
    case 'ausente':
      return 'Esta prueba no tiene evaluación registrada. Se muestran las cifras medidas, sin preferencia.';
    case 'malformada':
      return 'El registro de la evaluación guardada no se puede leer. Se muestran las cifras medidas, sin preferencia.';
    case 'no_soportada':
      return 'La evaluación guardada usa una metodología que esta versión no interpreta. Se muestran las cifras medidas, sin preferencia.';
    case 'soportada':
      return '';
  }
}

/** Icono del participante: escudo con espada arriba, flecha abajo si empeoró. */
function IconoParticipante({ lado, peor }: { lado: 'antes' | 'despues'; peor: boolean }) {
  if (peor) return <ArrowDownRight className="w-5 h-5" aria-hidden="true" />;
  return <Swords className="w-5 h-5" aria-hidden="true" data-lado={lado} />;
}

/**
 * Un participante del enfrentamiento.
 *
 * La corona NO se decide aquí: se pinta solo si `ganadora` llega ya resuelta por el
 * intérprete. Este componente no compara números.
 */
function Participante({
  fps,
  bench,
  lado,
  ganadora,
  peor,
  onCopy,
}: {
  fps: number | null;
  bench: Benchmark;
  lado: 'antes' | 'despues';
  ganadora: boolean;
  peor: boolean;
  onCopy: (t: string) => void;
}) {
  // Se reutilizan los colores de serie ya establecidos en toda la aplicación
  // (cian #38bdf8 / ámbar de `--caution`) en vez de introducir un cian y un naranja
  // nuevos: dos tonos casi iguales en la misma pantalla se leen como un error.
  const cian = lado === 'antes';
  const colorTexto = cian ? SERIE.antes.texto : 'text-[#f5b964]';
  // Resplandor sutil detrás del participante, con el color de su serie.
  const halo = cian
    ? 'shadow-[inset_0_0_40px_-22px_#38bdf8]'
    : 'shadow-[inset_0_0_40px_-22px_#f5b964]';

  return (
    <div
      className={`relative flex-1 min-w-0 rounded-xl border px-4 py-3 ${halo} ${
        cian ? 'border-[#38bdf8]/30 bg-[#38bdf8]/5' : 'border-[#f5b964]/30 bg-[#f5b964]/5'
      } ${ganadora ? 'ring-1 ring-amber-300/40' : ''}`}
    >
      {/* Fila del nombre con altura fija: la insignia «Mejor resultado» solo existe en un lado,
          así que si la fila creciera con ella los dos FPS quedarían a distinta altura y
          el duelo se vería descuadrado. Con `min-h` las dos filas miden lo mismo. */}
      <div className="flex items-center gap-2 min-w-0 min-h-[24px]">
        <span className={`shrink-0 ${colorTexto}`}>
          <IconoParticipante lado={lado} peor={peor} />
        </span>
        {/* El nombre conserva su tooltip con el comando completo. */}
        <PruebaN bench={bench} onCopy={onCopy} className={`text-sm font-bold ${colorTexto}`} />

        {/* Corona: solo para el mejor resultado observado, nunca para una victoria
            confirmada. Va en la misma línea que el nombre, no encima: así no empuja
            el marcador de FPS hacia abajo. */}
        {ganadora && (
          <span className="shrink-0 inline-flex items-center gap-1 rounded-full bg-amber-300/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-amber-300">
            <Crown className="w-3 h-3" aria-hidden="true" />
            Mejor resultado
          </span>
        )}
      </div>

      <p className={`mt-2 text-3xl font-bold font-mono tabular-nums leading-none ${colorTexto}`}>
        {fps == null ? '—' : fps.toFixed(1)}
        <span className="ml-1 text-xs font-semibold opacity-70">FPS</span>
      </p>
    </div>
  );
}

/**
 * Traduce el **modelo de presentación** a lo que se pinta. **No calcula nada**: elige
 * textos y tono a partir de lo que el modelo ya decidió.
 *
 * Que el veredicto no se registre no es un error ni un vacío rellenable: es un hecho sobre
 * el archivo. Se dice tal cual, sin inventar un «mejora» que entonces no constaba. Y un
 * registro **malformado** se presenta como no legible, sin lanzar: perder una fila de la
 * lista es aceptable, perder la pantalla no.
 */
function presentarModelo(m: ModeloDePresentacion): {
  tono: Tono;
  titulo: string;
  explicacion: string;
  chip: string | null;
  chipTitulo: string;
  sinGanadora: string;
  avisos: string[];
} {
  const confianza = m.confianza ? textoDeEtiqueta(m.confianza) : null;

  // A: el veredicto **del modelo** manda sobre el estado crudo del registro.
  //
  // El registro de PARCIAL_PERDIDA dice `estado: 'mejora'`, y eso es cierto de las métricas
  // que el motor juzgó; pero el veredicto de la comparación no es «mejora», es
  // `cambio_parcial`: decidió con parte de las métricas y una pérdida del 50 % quedó fuera.
  // Despachando por `m.titulo` se pintaba «rinde mejor que la anterior», que es justo la
  // afirmación global que el motor no hizo. Se despacha por `veredicto.tipo`.
  if (m.veredicto.tipo === 'cambio_parcial') {
    const nueva = m.veredicto.favor === 'candidata';
    return {
      tono: 'neutro',
      titulo: 'Mejora en parte de las métricas',
      explicacion: nueva
        ? 'La grabación nueva mejora en las métricas que la evaluación sí juzgó, pero no en ' +
          'todas: hay alguna métrica que quedó fuera de la decisión. No se concluye que una ' +
          'grabación sea mejor que la otra en conjunto.'
        : 'La grabación anterior queda mejor en las métricas que la evaluación sí juzgó, ' +
          'pero no en todas: hay alguna métrica que quedó fuera de la decisión. No se ' +
          'concluye que una grabación sea mejor que la otra en conjunto.',
      chip: confianza,
      chipTitulo:
        'La mejora se refiere solo a las métricas evaluadas. Las externas a la decisión se ' +
        'muestran aparte, con su valor medido y sin preferencia.',
      sinGanadora: '',
      avisos: [],
    };
  }

  switch (m.titulo) {
    case 'historica':
      return {
        tono: 'neutro',
        titulo: 'Evaluación original no registrada',
        explicacion:
          'El registro guardado no incluye una evaluación estructurada. Los datos medidos y ' +
          'sus diferencias están completos. No se reconstruye un veredicto moderno para esta ' +
          'prueba: atribuirle uno sería inventar una decisión que no consta.',
        chip: null,
        chipTitulo: '',
        sinGanadora: 'Sin evaluación registrada no consta qué grabación rindió mejor.',
        avisos: [],
      };
    case 'no_soportada': {
      return {
        tono: 'aviso',
        titulo: 'Evaluación no compatible con esta versión',
        // C: aquí **no** se cita el estado del registro. `detalle` es el código crudo tal
        // como quedó guardado (`mejora`), y enseñarlo entre paréntesis lo presenta como una
        // conclusión válida cuando esta versión no puede interpretarlo. Que el código no se
        // entienda no autoriza a leerlo: si lo hiciera, se mostraría como veredicto.
        explicacion:
          'La evaluación guardada usa una versión de metodología o unos códigos que esta ' +
          'versión de Thoriapp no sabe leer. Se conserva intacta; los datos medidos se ' +
          'muestran abajo, sin reinterpretarlos.',
        chip: 'No compatible',
        chipTitulo: 'Guardada con una metodología o códigos que esta versión no conoce.',
        sinGanadora: 'No consta qué grabación rindió mejor: la evaluación no es legible aquí.',
        avisos: [],
      };
    }
    case 'malformada':
      return {
        tono: 'aviso',
        titulo: 'Evaluación no legible',
        explicacion:
          'Esta prueba guarda una evaluación, pero su formato no es el que esta versión ' +
          'espera. El contenido original se conserva intacto; los datos medidos se muestran ' +
          'abajo sin reinterpretarlos.',
        chip: 'No legible',
        chipTitulo: 'Hay una evaluación guardada, pero no se puede leer aquí.',
        sinGanadora: 'No consta qué grabación rindió mejor: la evaluación no es legible.',
        avisos: [],
      };
    case 'datos_insuficientes':
      return {
        tono: 'aviso',
        titulo: 'Evaluación registrada: datos insuficientes',
        explicacion:
          'La evaluación sí se registró, y su conclusión fue que las grabaciones no bastaban ' +
          'para decidir. Eso no es lo mismo que no tener evaluación: aquí hubo una y dijo esto.',
        chip: 'Datos insuficientes',
        chipTitulo: 'La evaluación se registró y concluyó que no había datos suficientes.',
        sinGanadora: 'No hubo datos suficientes para decir cuál rindió mejor.',
        avisos: [],
      };
    case 'solo_descriptiva':
      return {
        tono: 'neutro',
        titulo: 'Evaluación descriptiva',
        explicacion:
          'Las metodologías de cálculo de las dos grabaciones no son comparables, así que la ' +
          'evaluación registrada describe las diferencias sin concluir cuál es mejor.',
        chip: 'Descriptiva',
        chipTitulo: 'Metodologías no comparables: la evaluación no emite veredicto.',
        sinGanadora: 'Sin comparabilidad metodológica no se puede decir cuál rindió mejor.',
        avisos: [],
      };
    case 'mixto':
      return {
        tono: 'neutro',
        titulo: 'Resultado mixto',
        explicacion:
          'La evaluación guardada concluyó que gana una en un aspecto y la otra en otro.',
        chip: confianza,
        chipTitulo:
          'Esta prueba compara dos grabaciones. Para confirmar una mejora hacen falta varias ' +
          'grabaciones por configuración.',
        sinGanadora:
          'Gana una prueba en un aspecto y la otra en otro: no hay un único mejor resultado.',
        avisos: [],
      };
    case 'sin_cambio':
      return {
        tono: 'neutro',
        titulo: 'Sin cambio',
        explicacion:
          'La evaluación guardada concluyó que la diferencia entra dentro del margen. Eso no ' +
          'significa que las dos configuraciones sean equivalentes: significa que estas dos ' +
          'grabaciones no permiten sostener que haya cambio.',
        chip: confianza,
        chipTitulo: 'No identifica las dos configuraciones: solo no sostiene un cambio.',
        sinGanadora: 'Ninguna de las dos destaca: la diferencia entra dentro del margen.',
        avisos: [],
      };
    case 'mejora':
      return {
        tono: 'positivo',
        titulo: 'Mejora',
        explicacion:
          'La evaluación guardada concluyó que la grabación nueva rinde mejor que la anterior.',
        chip: confianza,
        chipTitulo:
          'Esta prueba compara dos grabaciones. Para confirmar una mejora hacen falta varias ' +
          'grabaciones por configuración.',
        sinGanadora: '',
        avisos: [],
      };
    case 'empeora':
      return {
        tono: 'negativo',
        titulo: 'Empeora',
        explicacion:
          'La evaluación guardada concluyó que la grabación nueva rinde peor que la anterior.',
        chip: confianza,
        chipTitulo:
          'Esta prueba compara dos grabaciones. Para confirmar un empeoramiento hacen falta ' +
          'varias grabaciones por configuración.',
        sinGanadora: '',
        avisos: [],
      };
  }
}

/**
 * Por debajo de esta duración el 0,1 % low se apoya en muy pocas muestras.
 *
 * Es el mismo `MIN_STEADY_SECONDS` del backend; aquí solo decide si se enseña el aviso de
 * cautela, no si una comparación es válida.
 */
const MIN_GRABACION_CORTA_S = 20;

/**
 * Margen con el que se comparó.
 *
 * Se muestra el margen combinado **guardado**. Si no consta (pruebas antiguas), se dice
 * que no consta: no se rellena con el margen vigente, que sería atribuirle a la prueba un
 * criterio que no se aplicó entonces.
 */
/**
 * E: ruido y margen son **dos datos distintos** y aquí estaban fundidos.
 *
 * `comparison.noise_percent` es la dispersión combinada de las dos grabaciones: describe
 * cuánto varía la medición, no el criterio con el que se juzgó. El margen que el motor
 * **aplicó** viene en la evaluación (`condiciones.margen_porcentaje`). Rotular el ruido como
 * «Margen estimado» presentaba una dispersión como si fuera un umbral, y el ruido no es un
 * intervalo de confianza: solo dice cuánto se movieron las cifras.
 *
 * Ninguno se reconstruye: si el registro no lo trae, se dice que no consta.
 */
function textoDeRuido(c: BenchmarkComparison, lectura: EstadoDeLectura): string {
  const n = c.noise_percent;
  if (n == null || !Number.isFinite(n)) return 'no consta en el registro';
  // B: el dato **está** en el registro, pero las condiciones las escribió una metodología
  // que esta versión no interpreta. Decir «no consta» atribuye a una ausencia lo que es
  // una imposibilidad de lectura. Y al revés: sin evaluación registrada no hay nada que
  // interpretar, así que ahí «no se interpreta» sería igual de falso.
  switch (lectura) {
    case 'soportada':
    case 'ausente':
      return `±${n.toFixed(1)} %`;
    case 'malformada':
    case 'no_soportada':
      return 'no se interpreta con esta versión';
  }
}

/**
 * B: el margen que el motor aplicó, o por qué no se puede afirmar.
 *
 * Tres situaciones distintas, tres textos distintos:
 * - Hay valor y la evaluación es legible → se enseña el margen.
 * - La evaluación no es legible → el valor puede existir, pero **no se interpreta**; no se
 *   presenta como criterio vigente ni se dice que falte.
 * - No hay valor → no consta.
 */
function textoDeMargenAplicado(modelo: ModeloDePresentacion): string {
  const m = modelo.margenAplicado;
  if (m != null && Number.isFinite(m)) return `±${m.toFixed(1)} %`;
  // Quinta ronda: «no se interpreta» solo vale si **hay algo** que interpretar. Una prueba
  // histórica no tiene evaluación registrada, así que su margen no consta: decir que no se
  // interpreta le atribuye un registro que nunca existió. `esLegible` responde «¿se puede
  // leer?», que es `false` también para la ausencia; aquí hace falta distinguir el motivo.
  switch (modelo.estadoDeLectura) {
    case 'soportada':
    case 'ausente':
      return 'no consta en el registro';
    case 'malformada':
    case 'no_soportada':
      return 'no se interpreta con esta versión';
  }
}

/**
 * Panel principal: las dos pruebas enfrentadas y, debajo, la conclusión.
 *
 * Es un solo componente a propósito: separar el VS y el resultado en dos tarjetas hacía
 * que se leyeran como cosas distintas cuando son la misma frase («la segunda rinde peor,
 * la primera va mejor»).
 */
function PanelVS({
  comparison,
  modelo,
  etiquetas,
  onCopy,
  interpretable,
}: {
  comparison: BenchmarkComparison;
  /** Modelo de presentación compartido: veredicto, preferencia y límites ya decididos. */
  modelo: ModeloDePresentacion;
  etiquetas: Etiquetas;
  onCopy: (t: string) => void;
  /** J: si hay evaluación interpretable, la narrativa es relato, no veredicto vigente. */
  interpretable: boolean;
}) {
  // Presentación de lo guardado. No se recalcula ningún veredicto: `estado` es el que se
  // registró al guardar, y si no se registró se dice eso en vez de deducirlo ahora.
  const vista = presentarModelo(modelo);
  const style = TONO[vista.tono];

  // La ganadora sale del MODELO (`quienFavorece`), que solo la rellena para `mejora` o
  // `empeora`. Antes se deducía del estado por una tabla paralela; ahora hay una sola.
  const mejor = modelo.quienFavorece == null
    ? null
    : {
        lado: modelo.quienFavorece === 'referencia' ? ('antes' as const) : ('despues' as const),
        nombre: modelo.quienFavorece === 'referencia' ? etiquetas.antes : etiquetas.despues,
      };

  const baseFps = comparison.baseline.avg_fps;
  const testFps = comparison.test.avg_fps;

  // «Peor» solo cuando el veredicto **guardado** dice que empeora: sin ganadora no se
  // insulta a nadie.
  const peor = modelo.quienFavorece === 'referencia';

  return (
    <section
      className="rounded-2xl border border-[var(--border-hover)] bg-[var(--surface)] overflow-visible"
      aria-labelledby="resultado-principal"
    >
      <div className="p-4 sm:p-5">
        {/* Enfrentamiento. En pantallas estrechas se apila y el VS queda en medio. */}
        <div className="flex flex-col sm:flex-row items-stretch gap-2 sm:gap-3">
          <Participante
            fps={baseFps}
            bench={comparison.baseline}
            lado="antes"
            ganadora={mejor?.lado === 'antes'}
            peor={false}
            onCopy={onCopy}
          />

          <div className="flex items-center justify-center shrink-0 sm:px-1" aria-hidden="true">
            <span className="text-2xl font-black italic tracking-tighter text-[#fbbf24] drop-shadow-[0_0_10px_rgba(251,191,36,0.35)]">
              VS
            </span>
          </div>

          <Participante
            fps={testFps}
            bench={comparison.test}
            lado="despues"
            ganadora={mejor?.lado === 'despues'}
            peor={peor}
            onCopy={onCopy}
          />
        </div>

        {/* Conclusión, unida al VS por una línea sutil: forma parte del mismo panel. */}
        <div className={`mt-4 pt-4 border-t border-[var(--border)] rounded-xl`}>
          <div className="flex flex-wrap items-center gap-2 mb-1">
            <h3 id="resultado-principal" className={`text-lg font-bold ${style.texto}`}>
              {vista.titulo}
            </h3>
            {vista.chip && (
              <span
                className="text-[11px] font-bold px-2 py-0.5 rounded-md bg-[var(--surface)] text-[var(--text-muted)] border border-[var(--border)]"
                title={vista.chipTitulo}
              >
                {vista.chip}
              </span>
            )}
          </div>

          <p className="text-sm text-[var(--text)] leading-relaxed">{vista.explicacion}</p>

          {/* C3-04: la narrativa original que el motor escribió al medir, guardada con la
              comparación. Se enseña **tal cual**, sin reescribirla ni resumirla: es lo
              único que queda de lo que se dijo entonces cuando no hay evaluación
              registrada, y sustituirla por un texto nuevo sería reconstruir su historia.
              Con evaluación registrada se sigue mostrando: es el relato de la medición, y
              no compite con el veredicto porque no es uno. */}
          {modelo.narrativaOriginal && (
            <div className="mt-3 rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2">
              {/* J: con evaluación interpretable, el veredicto es el estructurado y la
                  narrativa queda como relato de la medición —no como una segunda
                  conclusión—. Sin evaluación, la narrativa es lo único que consta y se
                  identifica como tal. */}
              <p className="text-[11px] font-bold uppercase tracking-wide text-[var(--text-dim)] mb-1">
                {interpretable
                  ? 'Texto original de la medición'
                  : 'Texto original que sí consta en esta prueba'}
              </p>
              {interpretable && (
                <p className="text-[11px] text-[var(--text-dim)] mb-1">
                  Se conserva tal cual se escribió. El resultado que cuenta es el de la
                  evaluación registrada.
                </p>
              )}
              <p
                className="text-xs text-[var(--text-muted)] leading-relaxed whitespace-pre-line"
                data-testid="narrativa-original"
              >
                {modelo.narrativaOriginal}
              </p>
            </div>
          )}

          {/* C3-06 / C3-25: por qué la evaluación dijo lo que dijo, y qué no pudo decir.
              Antes el detalle guardado no mostraba ni motivos ni limitaciones: el veredicto
              aparecía sin justificación y una exclusión de métrica era invisible. Los
              códigos crudos quedan como detalle secundario, nunca como única explicación. */}
          {(modelo.motivos.length > 0 || modelo.limitaciones.length > 0) && (
            <div className="mt-3 space-y-2">
              {modelo.limitaciones.length > 0 && (
                <div className="rounded-lg border border-caution/50 bg-[var(--caution-bg)] px-3 py-2">
                  <p className="text-[11px] font-bold uppercase tracking-wide text-[var(--caution)] mb-1">
                    Límites de esta evaluación
                  </p>
                  <ul className="space-y-1">
                    {modelo.limitaciones.map((l) => (
                      <li key={l.codigo} className="text-xs text-[var(--text)] flex items-start gap-1.5">
                        <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5 text-[var(--caution)]" aria-hidden="true" />
                        <span>
                          {textoDeEtiqueta(l)}
                          {/* D: el código técnico es información secundaria, y solo se
                              muestra si no hay traducción. Repetirlo entre paréntesis
                              cuando ya está explicado no informa de nada. */}
                          {!l.conocido && (
                            <span className="ml-1 text-[var(--text-dim)]">({l.codigo})</span>
                          )}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {/* C3-25: qué métrica motivó cada limitación, con el apoyo de cada lado.
                  La limitación global dice qué pasó pero no a quién; el contrato sí lo
                  trae por métrica. Solo se enseña lo que el motor registró: el número de
                  muestras por lado, nunca el total de la grabación como si fuera apoyo del
                  percentil. */}
              {modelo.metricasAfectadas.length > 0 && (
                <div className="rounded-lg border border-caution/40 bg-[var(--bg)] px-3 py-2">
                  <p className="text-[11px] font-bold uppercase tracking-wide text-[var(--caution)] mb-1">
                    A qué métrica afecta
                  </p>
                  <ul className="space-y-1.5">
                    {modelo.metricasAfectadas.map((m) => (
                      <li key={`afectada-${m.clave}`} className="text-xs text-[var(--text)]">
                        <span className="font-semibold">{m.nombre}</span>
                        <span className="text-[var(--text-muted)]">
                          {' — '}
                          {m.motivos.map(textoDeEtiqueta).join('; ')}
                        </span>
                        {(m.apoyoReferencia || m.apoyoCandidata) && (
                          <span className="block text-[11px] text-[var(--text-dim)]">
                            Apoyo por lado:{' '}
                            {m.apoyoReferencia
                              ? `${m.apoyoReferencia.muestras} muestras (${etiquetas.antes})`
                              : 'no consta (la anterior)'}
                            {' · '}
                            {m.apoyoCandidata
                              ? `${m.apoyoCandidata.muestras} muestras (${etiquetas.despues})`
                              : 'no consta (la nueva)'}
                          </span>
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {modelo.motivos.length > 0 && (
                <div className="rounded-lg border border-[var(--border)] bg-[var(--bg)] px-3 py-2">
                  <p className="text-[11px] font-bold uppercase tracking-wide text-[var(--text-dim)] mb-1">
                    En qué se basó
                  </p>
                  <ul className="space-y-1">
                    {modelo.motivos.map((m) => (
                      <li key={m.codigo} className="text-xs text-[var(--text-muted)]">
                        {textoDeEtiqueta(m)}
                        {!m.conocido && (
                          <span className="ml-1 text-[var(--text-dim)]">({m.codigo})</span>
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
              {/* D: los códigos conocidos se conservan accesibles como información
                  técnica secundaria, en un plegado: la frase principal ya los explica, y
                  saturar con identificadores internos no ayuda a decidir. */}
              {[...modelo.limitaciones, ...modelo.motivos].length > 0 && (
                <details
                  className="mt-1 text-[11px] text-[var(--text-dim)]"
                  data-testid="codigos-tecnicos"
                >
                  <summary className="cursor-pointer select-none hover:text-[var(--text-muted)]">
                    Códigos técnicos de la evaluación
                  </summary>
                  <ul className="mt-1 space-y-0.5 font-mono">
                    {[...modelo.limitaciones, ...modelo.motivos].map((c) => (
                      <li key={`tec-${c.codigo}`}>
                        {c.codigo}
                        {c.conocido ? '' : ' (desconocido)'}
                      </li>
                    ))}
                  </ul>
                </details>
              )}
            </div>
          )}

          {/* C3-25: el motor observó una pérdida en una métrica que no pudo evaluar. Se
              dice cuál y con qué valores, y se aclara que no se evaluó: ocultarla dejaría
              al usuario con un «mejora» y una métrica empeorada sin explicación. */}
          {modelo.cobertura.perdidaEnMetricaExcluida && (
            <div
              className="mt-3 rounded-lg border border-caution/50 bg-[var(--caution-bg)] px-3 py-2"
              data-testid="perdida-metrica-excluida"
            >
              <p className="text-xs font-semibold text-[var(--caution)] flex items-start gap-1.5">
                <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" aria-hidden="true" />
                <span>
                  Hay una pérdida observada en una métrica que la evaluación no pudo juzgar,
                  así que no cuenta en el resultado. Está en la tabla, sin preferencia.
                </span>
              </p>
            </div>
          )}

          {/* Mejor resultado observado: es lo único que se puede afirmar con una sola
              comparación. Sin ganadora clara no se pinta. */}
          {mejor ? (
            <p className="mt-2 text-sm font-semibold text-[var(--text)] flex items-start gap-1.5">
              <Crown className="w-3.5 h-3.5 mt-0.5 shrink-0 text-amber-300" aria-hidden="true" />
              <span>
                Mejor resultado observado: <span className="text-amber-300">{mejor.nombre}</span>
              </span>
            </p>
          ) : (
            /* C3-05: sin texto que decir no se pinta nada. Antes se dibujaba la balanza
               con `sinGanadora` vacío y quedaba un icono suelto sin mensaje debajo del
               aviso. `mejor` es nulo en los resultados parciales (no hay ganadora global)
               pero ahí ya hay un titular que lo explica. */
            vista.sinGanadora !== '' && (
              <p className="mt-2 text-sm font-semibold text-[var(--text-muted)] flex items-start gap-1.5">
                <Scale className="w-3.5 h-3.5 mt-0.5 shrink-0" aria-hidden="true" />
                <span>{vista.sinGanadora}</span>
              </p>
            )
          )}

          {/* El aviso de fiabilidad del 0,1 % low se excluye aquí porque ya se explica
              en la caja técnica de abajo: el mismo aviso dos veces en una pantalla es ruido. */}
          {vista.avisos.length > 0 && (
            <ul className="mt-3 pt-3 border-t border-[var(--border)] space-y-1.5">
              {vista.avisos.map((a) => (
                <li key={a} className="text-xs text-[var(--caution)] font-medium flex items-start gap-2">
                  <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" aria-hidden="true" />
                  <span>{a}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </section>
  );
}

/* ────────────────────── Configuración comparada ────────────────────── */

/**
 * Separa un comando en tokens respetando comillas simples y dobles y el escapado.
 *
 * La versión anterior partía por espacios sin más: `PROTON_LOG=1 "mi ruta/juego"` daba
 * tres tokens distintos de los reales y el diff mostraba cambios que no existen.
 * Esto no interpreta el shell (no expande variables, no ejecuta nada): solo agrupa
 * correctamente lo que está entrecomillado.
 */
export function tokenizarComando(cmd: string): string[] {
  const out: string[] = [];
  let actual = '';
  let comilla: '"' | "'" | null = null;

  for (let i = 0; i < cmd.length; i++) {
    const ch = cmd[i];
    if (comilla) {
      if (ch === comilla) comilla = null;
      else actual += ch;
    } else if (ch === '"' || ch === "'") {
      comilla = ch;
    } else if (ch === '\\' && i + 1 < cmd.length) {
      actual += cmd[++i];
    } else if (/\s/.test(ch)) {
      if (actual) { out.push(actual); actual = ''; }
    } else {
      actual += ch;
    }
  }
  if (actual) out.push(actual);

  // El marcador de Steam no es un parámetro: se quita para el diff.
  return [...new Set(out.filter((t) => t && t !== '%command%'))];
}

interface DiffComandos {
  añadidos: string[];
  eliminados: string[];
  /** Los dos comandos registrados son exactamente el mismo texto. */
  iguales: boolean;
  /** Los dos comandos están registrados. `false` = no se puede afirmar nada. */
  comparable: boolean;
}

export function diffComandos(antes: string, despues: string): DiffComandos {
  const aTexto = antes.trim();
  const bTexto = despues.trim();

  // Sin los dos comandos no se puede comparar nada. Antes `diffComandos('', '')` daba
  // «iguales» y la pantalla afirmaba «sin cambios» sobre pruebas que no registraron el
  // comando: dos vacíos no son dos comandos iguales.
  if (!aTexto || !bTexto) {
    return { añadidos: [], eliminados: [], iguales: false, comparable: false };
  }

  // Igualdad por TEXTO COMPLETO, no por tokens. `--foo=1 --bar=2` y `--bar=2 --foo=1`
  // son comandos distintos aunque tengan las mismas palabras, y tokenizar convertía en
  // «sin cambios» cualquier reordenación.
  if (aTexto === bTexto) {
    return { añadidos: [], eliminados: [], iguales: true, comparable: true };
  }

  const a = tokenizarComando(aTexto);
  const b = tokenizarComando(bTexto);
  const añadidos = b.filter((t) => !a.includes(t));
  const eliminados = a.filter((t) => !b.includes(t));
  // Los tokens sirven para RESALTAR la diferencia, no para decidir si la hay: si el texto
  // difiere pero los tokens no (reordenación, comillas), `iguales` sigue siendo `false`.
  return { añadidos, eliminados, iguales: false, comparable: true };
}

/**
 * Bloque de configuración comparada.
 *
 * Cuando los dos comandos son iguales NO se muestran dos tarjetas: sería ruido visual
 * y, peor, invitaría a atribuir la diferencia de rendimiento a un cambio que no hubo.
 */
function ConfiguracionComparada({
  comparison,
  onCopy,
}: {
  comparison: BenchmarkComparison;
  onCopy: (t: string) => void;
}) {
  const antes = (comparison.baseline.launch_command || '').trim();
  const despues = (comparison.test.launch_command || '').trim();
  const diff = diffComandos(antes, despues);
  const etiquetas = etiquetasDe(comparison);

  const BotonCopiar = ({ texto }: { texto: string }) => (
    <button
      type="button"
      onClick={() => onCopy(texto)}
      className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-semibold text-[var(--text-muted)] bg-[var(--surface)] border border-[var(--border)] hover:text-[var(--text)] hover:border-action/50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
    >
      <Copy className="w-3 h-3" aria-hidden="true" />
      Copiar
    </button>
  );

  /** «No registrado» no es `%command%`: son cosas distintas y se dicen distinto. */
  const TextoComando = ({ comando }: { comando: string }) =>
    comando ? (
      <code className="block text-[11px] leading-relaxed font-mono text-[var(--text)] break-all select-text">
        {comando}
      </code>
    ) : (
      <p className="text-[11px] italic text-[var(--text-dim)]">Comando no registrado</p>
    );

  const Caja = ({ titulo, color, comando }: { titulo: string; color: string; comando: string }) => (
    <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)] min-w-0">
      <div className="flex items-center justify-between gap-2 mb-2">
        <span className="text-sm font-bold" style={{ color }}>
          {titulo}
        </span>
        {comando ? (
          <BotonCopiar texto={comando} />
        ) : (
          <span className="text-[10px] font-semibold px-2 py-0.5 rounded-md bg-border/40 text-[var(--text-dim)]">
            sin datos
          </span>
        )}
      </div>
      <TextoComando comando={comando} />
    </div>
  );

  const Cabecera = () => (
    <>
      <h3 className="text-base font-bold mb-1 flex items-center gap-2 text-[var(--text)]">
        <Terminal className="w-5 h-5 text-[var(--action)]" aria-hidden="true" />
        Configuración comparada
      </h3>
      <p className="text-xs text-[var(--text-muted)] mb-1">
        Comando registrado al iniciar cada prueba
      </p>
      <p className="text-[11px] text-[var(--text-dim)] mb-4 leading-relaxed">
        Es lo que Thoriapp tenía configurado cuando se capturó cada grabación, no lo que se
        ejecutó de verdad. Si cambiaste las opciones de lanzamiento en Steam a mano, esta
        pantalla no puede saberlo.
      </p>
    </>
  );

  // 1. Falta uno de los dos: no se afirma nada, ni igualdad ni diferencia.
  if (!diff.comparable) {
    const faltaAntes = !antes;
    const faltaDespues = !despues;
    return (
      <section className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <Cabecera />
        <div className="p-3 rounded-lg bg-[var(--caution-bg)] border border-caution/40 mb-3">
          <p className="text-xs font-semibold text-[var(--caution)] flex items-start gap-1.5">
            <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" aria-hidden="true" />
            <span>
              {faltaAntes && faltaDespues
                ? 'Ninguna de las dos grabaciones guardó el comando que tenías configurado. No se puede decir si cambió algo.'
                : `La grabación «${faltaAntes ? etiquetas.antes : etiquetas.despues}» no guardó el comando que tenías configurado. No se puede decir si cambió algo.`}
            </span>
          </p>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <Caja titulo={etiquetas.antes} color={SERIE.antes.hex} comando={antes} />
          <Caja titulo={etiquetas.despues} color={SERIE.despues.hex} comando={despues} />
        </div>
        <p className="mt-3 text-xs text-[var(--text-muted)] leading-relaxed">
          Las grabaciones anteriores a la versión que empezó a registrar el comando no lo
          conservan, y no se inventa: no hay forma de saber con qué se hicieron.
        </p>
      </section>
    );
  }

  // 2. Texto idéntico → una sola tarjeta, y solo ahora «sin cambios» es seguro.
  if (diff.iguales) {
    const esMarcador = antes === '%command%';
    return (
      <section className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <Cabecera />
        <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
          <div className="flex items-center justify-between gap-2 mb-2">
            <span className="text-xs font-bold px-2 py-0.5 rounded-md bg-border/40 text-[var(--text-muted)]">
              {esMarcador ? 'Sin parámetros en las dos' : 'Sin cambios'}
            </span>
            <BotonCopiar texto={antes} />
          </div>
          <TextoComando comando={antes} />
        </div>
        <p className="mt-3 text-xs text-[var(--text-muted)] leading-relaxed">
          {esMarcador ? (
            <>
              <strong className="text-[var(--text)]">%command%</strong> es el marcador de
              Steam para «el ejecutable del juego»: está registrado así en las dos pruebas,
              es decir, sin ningún ajuste activo en Thoriapp. No hay ningún cambio de
              configuración al que atribuir la diferencia de rendimiento.
            </>
          ) : (
            <>
              Las dos grabaciones tienen registrado exactamente el mismo comando, así que
              no hay ningún cambio de configuración al que atribuir la diferencia de
              rendimiento.
            </>
          )}
        </p>
      </section>
    );
  }

  // 3. Comandos distintos → los dos, completos, sin atribuir causalidad.
  return (
    <section className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
      <Cabecera />

      {(diff.añadidos.length > 0 || diff.eliminados.length > 0) && (
        <div className="mb-4 flex flex-wrap gap-2">
          {diff.añadidos.map((t) => (
            <span
              key={`a-${t}`}
              className="px-2.5 py-1 rounded-md text-xs font-mono font-semibold bg-compatible/10 text-[var(--compatible)] border border-compatible/30 select-text"
            >
              + {t}
            </span>
          ))}
          {diff.eliminados.map((t) => (
            <span
              key={`e-${t}`}
              className="px-2.5 py-1 rounded-md text-xs font-mono font-semibold bg-error/10 text-[var(--error)] border border-error/30 select-text"
            >
              − {t}
            </span>
          ))}
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <Caja titulo={etiquetas.antes} color={SERIE.antes.hex} comando={antes} />
        <Caja titulo={etiquetas.despues} color={SERIE.despues.hex} comando={despues} />
      </div>

      <p className="mt-3 text-xs text-[var(--text-muted)] leading-relaxed">
        El comando registrado cambió, pero eso no demuestra que sea la causa de la
        diferencia: el rendimiento también varía entre partidas. Repite la prueba antes de
        sacar conclusiones.
      </p>
    </section>
  );
}

/* ─────────────────────────── Vista de detalle ─────────────────────────── */

function formatFull(dateString: string): string {
  const date = new Date(dateString.replace(' ', 'T'));
  if (Number.isNaN(date.getTime())) return dateString;
  return date.toLocaleString('es-ES', {
    day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit',
  });
}

/**
 * Las métricas **tal como se guardaron** en la evaluación.
 *
 * Ni una cifra se recalcula: `antes`, `despues` y `diferencia` vienen del registro. La
 * dirección (sube/baja) y la preferencia (qué lado es mejor) se muestran por separado
 * porque no son lo mismo: subir FPS es mejor y subir frametime no.
 */
function TablaMetricasGuardadas({
  modelo,
  interpretable,
  lectura,
}: {
  modelo: ModeloDePresentacion;
  interpretable: boolean;
  /** A: por qué no se puede leer la evaluación, si ese es el caso. */
  lectura: EstadoDeLectura;
}) {
  const filas = modelo.metricas;
  if (filas.length === 0) return null;

  return (
    <div className="rounded-xl border border-[var(--border)] bg-[var(--bg)] overflow-hidden">
      <div className="px-3 py-2 border-b border-[var(--border)]">
        <h4 className="text-xs font-bold text-[var(--text)]">
          {modelo.veredicto.tipo === 'no_legible'
            ? 'Métricas registradas'
            : 'Métricas tal como se evaluaron'}
        </h4>
        <p className="text-[11px] text-[var(--text-muted)]">
          {/* C: sin evaluación interpretable no se puede afirmar que la compatibilidad no
              constara; lo único comprobable es que esta versión no lee el registro. */}
          {modelo.veredicto.tipo === 'no_legible'
            ? textoDeCabeceraNoLegible(lectura)
            : modelo.compatibilidad
            ? textoDeCompatibilidad(modelo.compatibilidad.codigo)
            : 'La compatibilidad metodológica no consta en el registro.'}
        </p>
      </div>
      <table className="w-full text-xs">
        <thead>
          <tr className="text-[var(--text-muted)]">
            <th className="text-left px-3 py-1.5 font-semibold">Métrica</th>
            <th className="text-right px-2 py-1.5 font-semibold">Antes</th>
            <th className="text-right px-2 py-1.5 font-semibold">Después</th>
            <th className="text-right px-2 py-1.5 font-semibold">Diferencia</th>
            {/* C3-24: dirección y preferencia son conceptos distintos y de fuentes
                distintas. En una sola columna se leían como una sola cosa. */}
            <th className="text-left px-3 py-1.5 font-semibold">Dirección</th>
            <th className="text-left px-3 py-1.5 font-semibold">Preferencia</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-[var(--border)]">
          {filas.map((m) => (
            <tr key={m.clave}>
              <td className="px-3 py-1.5 text-[var(--text)]">{textoDeEtiqueta(m.nombre)}</td>
              <td className="px-2 py-1.5 text-right font-mono tabular-nums">{f(m.antes)}</td>
              <td className="px-2 py-1.5 text-right font-mono tabular-nums">{f(m.despues)}</td>
              <td className="px-2 py-1.5 text-right font-mono tabular-nums">
                {m.diferenciaPct == null ? '—' : `${m.diferenciaPct >= 0 ? '+' : ''}${f(m.diferenciaPct)} %`}
              </td>
              <td className="px-3 py-1.5 text-[var(--text-muted)]">
                {/* Hecho medido: la dirección la marcan los datos, con independencia de
                    que la métrica entrara o no en la decisión. */}
                {m.direccion ? textoDeDireccion(m.direccion.codigo) : '—'}
              </td>
              <td className="px-3 py-1.5 text-[var(--text-muted)]">
                {/* H / C3-24: la preferencia la establece el motor. Si la evaluación no es
                    interpretable, no hay preferencia que afirmar. Sin ella, se distingue
                    «sin preferencia establecida» (evaluada, sin diferencia) de «no consta»
                    (la métrica no se evaluó). */}
                {!interpretable
                  ? 'no consta'
                  : m.preferencia
                  ? textoDePreferencia(m.preferencia.codigo)
                  : 'sin preferencia establecida'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function SavedComparisonDetail({
  comparison,
  evaluacion,
  estadoDeLaEvaluacion,
  onBack,
  onDelete,
  onCopy,
}: {
  comparison: BenchmarkComparison;
  evaluacion?: EvaluacionPersistida | null;
  /**
   * La clasificación **de Rust** (`load_comparisons`). Se transporta tal cual hasta aquí.
   *
   * Sin ella, una evaluación ilegible (`evaluacion` nula + registro corrupto) se
   * presentaba como «no registrada», que es afirmar que nunca existió. El detalle no puede
   * deducirla: solo Rust sabe si un registro ilegible estaba ahí.
   */
  estadoDeLaEvaluacion?: SavedComparison['estado_de_la_evaluacion'] | null;
  onBack: () => void;
  onDelete: () => void;
  onCopy: (t: string) => void;
}) {
  const [showDetails, setShowDetails] = useState(false);
  // Plegado por defecto, igual que los detalles técnicos.
  const [showComandos, setShowComandos] = useState(false);
  // Las etiquetas dependen solo de las dos grabaciones, así que se pueden memoizar aparte.
  const etiquetas = useMemo(() => etiquetasDe(comparison), [comparison]);
  // El modelo es el **guardado**, no uno recalculado ahora: rehacerlo con las reglas de hoy
  // y presentarlo como el original sería atribuirle a una prueba antigua una conclusión
  // que no tuvo. Una sola función compartida con el resto de vistas; si el registro es
  // imposible de leer, `presentarGuardada` devuelve `malformada` en vez de lanzar.
  const modelo = useMemo(
    () =>
      presentarGuardada({
        evaluacion,
        comparison,
        estado_de_la_evaluacion: estadoDeLaEvaluacion ?? undefined,
      }),
    [evaluacion, comparison, estadoDeLaEvaluacion]
  );

  // Si la grabación más corta no llega a la duración mínima, el 0,1 % low es menos fiable.
  // Se calcula con la duración **medida** guardada, no con una reinterpretación.
  const cortaLaGrabacion =
    duraMenosDe(comparison.baseline, MIN_GRABACION_CORTA_S) ||
    duraMenosDe(comparison.test, MIN_GRABACION_CORTA_S);

  // Cifras medidas y almacenadas: no dependen de ninguna evaluación y no se recalculan.
  //
  // La tarjeta de FPS medios usa la métrica del **modelo** cuando existe (así hereda la
  // preferencia que estableció el motor, y ninguna si no la estableció). Si la evaluación
  // no trae la métrica, la tarjeta se pinta descriptiva: dos cifras medidas y nada más.
  const metricaFps = modelo.metricas.find((m) => m.clave === 'avg_fps') ?? null;
  // C: una sola respuesta, la del modelo, para las dos tablas.
  //
  // A: el modelo distingue **por qué** no se puede leer (ausente / malformada /
  // no soportada). Antes un solo booleano colapsaba los tres y la comparación histórica
  // acababa diciendo que su evaluación «no se puede interpretar» cuando no existía.
  const interpretable = evaluacionInterpretable(modelo);
  const lectura = modelo.estadoDeLectura;
  const tarjetaFps: Parameters<typeof TarjetaMetrica>[0]['metrica'] = metricaFps
    ? { tipo: 'evaluada', datos: metricaFps }
    : {
        tipo: 'descriptiva',
        etiqueta: 'FPS medios',
        antes: comparison.baseline?.avg_fps ?? null,
        despues: comparison.test?.avg_fps ?? null,
      };
  // Estabilidad: descriptiva **siempre**. El motor no la evalúa (no hay clave en Rust), así
  // que aquí no hay preferencia que enseñar, y antes se enseñaba el 1 % low en su lugar.
  const estabilidad = modelo.descriptivas.estabilidad;
  const tarjetaEstabilidad: Parameters<typeof TarjetaMetrica>[0]['metrica'] = {
    tipo: 'descriptiva',
    etiqueta: 'Estabilidad',
    antes: estabilidad?.referencia?.indice ?? null,
    despues: estabilidad?.candidata?.indice ?? null,
  };

  return (
    <div className="max-w-5xl mx-auto p-4 sm:p-8 space-y-4 sm:space-y-6">
      <button
        type="button"
        onClick={onBack}
        className="flex items-center gap-2 text-sm font-semibold text-[var(--text-muted)] hover:text-[var(--text)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)] rounded"
      >
        <ArrowLeft className="w-4 h-4" aria-hidden="true" />
        Volver a Mis pruebas
      </button>

      {/* 1. Cabecera */}
      <header className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <p className="text-xs text-[var(--text-muted)] mb-1">Prueba guardada</p>
          <h2 className="text-xl sm:text-2xl font-bold truncate">
            {nombreLegible(comparison.test.game_name)}
          </h2>
          <p className="text-sm text-[var(--text-muted)] mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
            <span>{formatFull(comparison.test.timestamp)}</span>
            <span className="text-[var(--text-dim)]" aria-hidden="true">•</span>
            {/* Con las etiquetas visibles el «→» conserva su dirección: se lee
                «Prueba 1 → Prueba 2», no «Prueba 2 vs …». */}
            <span className="font-mono tabular-nums">
              <span className={SERIE.antes.texto} title={etiquetas.antes}>
                {etiquetas.antes} {f(comparison.baseline?.avg_fps ?? null)}
              </span>
              <span className="text-[var(--text-dim)]"> → </span>
              <span className={SERIE.despues.texto} title={etiquetas.despues}>
                {etiquetas.despues} {f(comparison.test?.avg_fps ?? null)}
              </span>{' '}
              FPS
            </span>
            {comparison.fps_change_percent != null && (
              <span className="font-bold font-mono tabular-nums">
                ({comparison.fps_change_percent >= 0 ? '+' : ''}
                {f(comparison.fps_change_percent)} %)
              </span>
            )}
          </p>
        </div>
        <button
          type="button"
          onClick={onDelete}
          className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-error/10 text-[var(--error)] border border-error/30 hover:bg-error/20 hover:border-error/50 transition-all duration-200 flex items-center gap-1.5 shrink-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--error)]"
        >
          <Trash2 className="w-3.5 h-3.5" aria-hidden="true" />
          Eliminar prueba
        </button>
      </header>

      {/* 2. Enfrentamiento y resultado (un solo panel) */}
      <PanelVS
        comparison={comparison}
        modelo={modelo}
        etiquetas={etiquetas}
        onCopy={onCopy}
        interpretable={interpretable}
      />

      {/* 3. Comparación de rendimiento — dos tarjetas destacadas y UNA tabla con el resto.
          Antes cada medida era una caja con su borde: aquí solo hay dos cajas y una tabla. */}
      <section className="space-y-3">
        <h3 className="text-base font-bold text-[var(--text)]">Comparación de rendimiento</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <TarjetaMetrica metrica={tarjetaFps} modo="fps" etiquetas={etiquetas} />
          <TarjetaMetrica metrica={tarjetaEstabilidad} modo="estabilidad" etiquetas={etiquetas} />
        </div>

        <TablaMetricas
          lectura={lectura}
          comparacion={comparison}
          etiquetas={etiquetas}
          modelo={modelo}
          interpretable={interpretable}
        />

        {/* 3b. Lo que la evaluación guardada dijo de cada métrica, si se registró. No se
            recalcula: se copian sus cifras, su dirección y su preferencia. */}
        {modelo.metricas.length > 0 && (
          <TablaMetricasGuardadas modelo={modelo} interpretable={interpretable} lectura={lectura} />
        )}

        {/* Un solo bloque de notas. El aviso del 0,1 % low aparece una única vez y
            solo si la grabación es corta. */}
        <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)] space-y-1.5">
          <p className="text-[11px] text-[var(--text-muted)] leading-relaxed flex items-start gap-2">
            <Info className="w-3.5 h-3.5 shrink-0 mt-0.5 text-[var(--action)]" aria-hidden="true" />
            <span>
              <strong className="text-[var(--text)]">La estabilidad de FPS es un índice propio de
              Thoriapp</strong>, no una métrica estándar de MangoHud: qué parte de la velocidad
              media se mantiene en los peores momentos (0-100). Un juego a 30 FPS constantes saca
              100 — significa «va parejo», no «va fluido».
            </span>
          </p>
          {cortaLaGrabacion && (
            <p className="text-[11px] text-[var(--caution)] leading-relaxed flex items-start gap-2">
              <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" aria-hidden="true" />
              <span>
                El <strong>0,1 % low</strong> puede variar mucho con grabaciones cortas.
                Interprétalo con cautela.
              </span>
            </p>
          )}
        </div>
      </section>

      {/* 4. Configuración comparada — plegada por defecto: con los dos comandos
          completos ocupaba más alto que el propio resultado. Cerrada solo enseña la
          cabecera; al abrirla conserva todo (comandos, diferencias, avisos, copiar). */}
      <section>
        <button
          type="button"
          onClick={() => setShowComandos((v) => !v)}
          aria-expanded={showComandos}
          aria-controls="configuracion-comparada"
          className="w-full flex items-center justify-between gap-2 px-4 py-2.5 rounded-lg text-sm font-semibold text-[var(--text-muted)] bg-[var(--bg)] border border-[var(--border)] hover:border-action/50 hover:text-[var(--text)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
        >
          <span className="flex items-center gap-2 min-w-0">
            <Terminal className="w-4 h-4 text-[var(--action)] shrink-0" aria-hidden="true" />
            <span className="truncate">Configuración comparada</span>
            <span className="text-[11px] font-normal text-[var(--text-dim)] hidden sm:inline truncate">
              · Comando registrado al iniciar cada prueba
            </span>
          </span>
          <ChevronDown
            className={`w-4 h-4 shrink-0 motion-safe:transition-transform motion-safe:duration-200 ${
              showComandos ? 'rotate-180' : ''
            }`}
            aria-hidden="true"
          />
        </button>
        {showComandos && (
          <div id="configuracion-comparada" className="mt-3">
            <ConfiguracionComparada comparison={comparison} onCopy={onCopy} />
          </div>
        )}
      </section>

      {/* 4b. Instantáneas históricas de cada prueba (subfase 5.2). Van aquí, después de la
          configuración comparada, porque responden a la misma pregunta con más detalle: con
          qué se hizo cada una. Cada una se pliega por separado. */}
      <InstantaneaCard bench={comparison.baseline} onCopy={onCopy} />
      <InstantaneaCard bench={comparison.test} onCopy={onCopy} />

      {/* 5. Detalles técnicos */}
      <section>
        <button
          type="button"
          onClick={() => setShowDetails((s) => !s)}
          aria-expanded={showDetails}
          aria-controls="detalles-tecnicos"
          className="w-full flex items-center justify-between px-4 py-2.5 rounded-lg text-sm font-semibold text-[var(--text-muted)] bg-[var(--bg)] border border-[var(--border)] hover:border-action/50 hover:text-[var(--text)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
        >
          Ver métricas avanzadas
          <ChevronDown
            className={`w-4 h-4 motion-safe:transition-transform motion-safe:duration-200 ${showDetails ? 'rotate-180' : ''}`}
            aria-hidden="true"
          />
        </button>
        {showDetails && (
          <div id="detalles-tecnicos" className="mt-3 space-y-3">
            <GraficoTemporal comparacion={comparison} etiquetas={etiquetas} />
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 text-[11px] text-[var(--text-muted)]">
              <p>
                <strong className="text-[var(--text)]">Muestras:</strong><br />
                {comparison.baseline.sample_count ?? comparison.baseline.total_frames}{' '}
                <span className={SERIE.antes.texto}>({etiquetas.antes})</span> ·{' '}
                {comparison.test.sample_count ?? comparison.test.total_frames}{' '}
                <span className={SERIE.despues.texto}>({etiquetas.despues})</span>
              </p>
              <p>
                <strong className="text-[var(--text)]">Duración:</strong><br />
                {textoDeDuracion(comparison.baseline, (v) => `${f(v)} s`)}{' '}
                <span className={SERIE.antes.texto}>({etiquetas.antes})</span> ·{' '}
                {textoDeDuracion(comparison.test, (v) => `${f(v)} s`)}{' '}
                <span className={SERIE.despues.texto}>({etiquetas.despues})</span>
              </p>
              <p>
                <strong className="text-[var(--text)]">Calentamiento:</strong><br />
                {f(comparison.baseline.warmup_seconds ?? 0, 0)} s ·{' '}
                {f(comparison.test.warmup_seconds ?? 0, 0)} s descartados
              </p>
              {/* E: los dos datos, separados y con su nombre. */}
              <p>
                <strong className="text-[var(--text)]">Ruido combinado:</strong><br />
                {textoDeRuido(comparison, lectura)}
              </p>
              <p>
                <strong className="text-[var(--text)]">Margen aplicado:</strong><br />
                {textoDeMargenAplicado(modelo)}
              </p>
            </div>
            <p className="text-[11px] text-[var(--text-dim)] leading-relaxed flex items-start gap-1.5">
              <Info className="w-3 h-3 shrink-0 mt-0.5" aria-hidden="true" />
              <span>
                El ruido combinado es cuánto varió la medición entre las dos grabaciones y no
                es un intervalo de confianza. El margen aplicado es el criterio con el que se
                juzgó la diferencia. Ninguno de los dos mide la variación entre partidas
                distintas (otra zona, otro enemigo): superar el margen no confirma por sí solo
                una mejora.
              </span>
            </p>
          </div>
        )}
      </section>
    </div>
  );
}

/** Marca de "guardado" reutilizable. Exportada para pruebas. */
export const IconoGuardado = Check;
