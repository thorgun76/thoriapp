import { useMemo, useState } from 'react';
import {
  ArrowLeft, Trash2, Copy, Terminal, ChevronDown, Info,
  AlertCircle, TrendingUp, TrendingDown, Minus, HelpCircle,
  ThumbsUp, ThumbsDown, Scale, Check, Crown, Swords, ArrowDownRight,
} from 'lucide-react';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import {
  interpretar, nombreLegible, etiquetasDe, titularDelCambio, mejorObservada,
  type Etiquetas,
  type Interpretacion, type MetricaInterpretada, type Tono,
} from '../lib/interpretacion';
import { estabilidadDe, estabilidadLabel } from '../lib/estabilidad';
import { GraficoTemporal } from './GraficoTemporal';
import { PruebaN } from './PruebaN';

/* ─────────────────────────────── Estilos ─────────────────────────────── */

/**
 * Color de cada tono. Los colores de las series (antes/después) NO están aquí: van
 * fijos en {@link SERIE} para que no cambien según quién gane, que era una de las
 * formas de confundir al usuario.
 */
const TONO: Record<Tono, { caja: string; icono: string; texto: string; Icon: typeof ThumbsUp }> = {
  bien:     { caja: 'border-compatible/60 bg-compatible/10', icono: 'bg-compatible/20', texto: 'text-[var(--compatible)]', Icon: ThumbsUp },
  mal:      { caja: 'border-error/60 bg-error/10',           icono: 'bg-error/20',      texto: 'text-[var(--error)]',      Icon: ThumbsDown },
  atencion: { caja: 'border-caution/60 bg-caution/10',       icono: 'bg-caution/20',    texto: 'text-[var(--caution)]',    Icon: Scale },
  neutro:   { caja: 'border-[var(--border)] bg-[var(--bg)]', icono: 'bg-border/40',     texto: 'text-[var(--text-muted)]', Icon: Minus },
};

/** Colores fijos de las dos series. Nunca dependen del resultado. */
const SERIE = {
  antes: { texto: 'text-[#38bdf8]', barra: 'bg-[#38bdf8]', hex: '#38bdf8' },
  despues: { texto: 'text-[var(--caution)]', barra: 'bg-[var(--caution)]', hex: 'var(--caution)' },
} as const;

const CAMBIO_STYLE = {
  mejor: { chip: 'bg-compatible/15 text-[var(--compatible)]', Icon: TrendingUp, word: 'Mejor' },
  peor: { chip: 'bg-error/15 text-[var(--error)]', Icon: TrendingDown, word: 'Peor' },
  sin_cambio: { chip: 'bg-border/40 text-[var(--text-muted)]', Icon: Minus, word: 'Sin cambio' },
  sin_datos: { chip: 'bg-border/40 text-[var(--text-dim)]', Icon: HelpCircle, word: 'Sin datos' },
} as const;

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
}: {
  antes: number | null;
  despues: number | null;
  max: number;
  unidad: string;
  decimals: number;
  /** «Prueba N» cuando la grabación tiene número; «Antes»/«Después» si es antigua. */
  etiquetas: Etiquetas;
}) {
  const pct = (v: number | null) => {
    if (v == null) return '0%';
    // Escala desde 0 para no exagerar diferencias pequeñas (barras truncadas).
    return `${Math.max(2, Math.min(100, (v / max) * 100))}%`;
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
 * resto de medidas vive en una tabla compacta más abajo. Cada tarjeta muestra sus dos
 * barras con la MISMA escala que la otra cuando miden lo mismo (0..100 en estabilidad).
 */
function TarjetaMetrica({
  metrica,
  modo,
  etiquetas,
}: {
  metrica: MetricaInterpretada;
  modo: 'fps' | 'estabilidad';
  etiquetas: Etiquetas;
}) {
  const st = CAMBIO_STYLE[metrica.cambio];
  const Icon = st.Icon;
  const esEstabilidad = modo === 'estabilidad';

  const maxFps = Math.max(metrica.antes ?? 0, metrica.despues ?? 0, 1);
  const maxEst = 100;

  return (
    <div className="p-4 rounded-xl bg-[var(--bg)] border border-[var(--border)] min-w-0">
      <div className="flex items-start justify-between gap-2 mb-1">
        <h4 className="text-sm font-bold text-[var(--text)] flex items-center gap-2 min-w-0">
          {esEstabilidad ? <Scale className="w-4 h-4 text-[var(--action)] shrink-0" aria-hidden="true" /> : <TrendingUp className="w-4 h-4 text-[var(--action)] shrink-0" aria-hidden="true" />}
          <span className="truncate">{metrica.etiqueta}</span>
        </h4>
        <span className={`text-[11px] font-bold px-2 py-0.5 rounded-md whitespace-nowrap flex items-center gap-1 ${st.chip}`}>
          <Icon className="w-3 h-3" aria-hidden="true" />
          {st.word}
        </span>
      </div>

      {esEstabilidad && metrica.antes != null && (
        <p className="text-[11px] text-[var(--text-dim)] mb-3">
          {estabilidadLabel(metrica.antes)} → {estabilidadLabel(metrica.despues)}
        </p>
      )}

      <Barras
        antes={metrica.antes}
        despues={metrica.despues}
        max={esEstabilidad ? maxEst : maxFps}
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

const diferencia = (
  antes: number,
  despues: number,
  unidad: Unidad,
  decimals: number
): { texto: string; signo: number } => {
  if (unidad === 'pct' || unidad === 'abs') {
    const delta = despues - antes;
    if (unidad === 'abs' || Math.abs(antes) < 0.5) {
      return { texto: `${delta > 0 ? '+' : ''}${delta.toFixed(decimals)}`, signo: delta };
    }
    const rel = (delta / Math.abs(antes)) * 100;
    return { texto: `${rel > 0 ? '+' : ''}${rel.toFixed(1)} %`, signo: rel };
  }
  const delta = despues - antes;
  const sufijo = unidad === 'pp' ? ' pp' : ' pts';
  return { texto: `${delta > 0 ? '+' : ''}${delta.toFixed(decimals)}${sufijo}`, signo: delta };
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
}: {
  comparacion: BenchmarkComparison;
  etiquetas: Etiquetas;
}) {
  const b = comparacion.baseline;
  const t = comparacion.test;

  const filas: {
    clave: string;
    label: string;
    hint: string;
    antes: number | null;
    despues: number | null;
    decimals: number;
    sufijo: string;
    unidad: Unidad;
    mejorArriba: boolean;
  }[] = [
    {
      clave: 'avg', label: 'FPS medios', hint: 'Más alto = mejor',
      antes: b.avg_fps, despues: t.avg_fps, decimals: 1, sufijo: '', unidad: 'pct', mejorArriba: true,
    },
    {
      clave: 'ft', label: 'Tiempo por imagen', hint: 'Menos ms = mejor',
      antes: b.frame_time_avg, despues: t.frame_time_avg, decimals: 2, sufijo: ' ms', unidad: 'pct', mejorArriba: false,
    },
    {
      clave: 'est', label: 'Estabilidad', hint: 'Índice sobre 100',
      antes: estabilidadDe(b), despues: estabilidadDe(t),
      decimals: 0, sufijo: '', unidad: 'puntos', mejorArriba: true,
    },
    {
      clave: 'low1', label: '1 % low', hint: 'Peor 1 % de imágenes',
      antes: b.fps_1_percent_low ?? null, despues: t.fps_1_percent_low ?? null, decimals: 1, sufijo: '', unidad: 'pct', mejorArriba: true,
    },
    {
      clave: 'low01', label: '0,1 % low', hint: 'Peor 0,1 % de imágenes',
      antes: b.fps_0_1_percent_low ?? null, despues: t.fps_0_1_percent_low ?? null, decimals: 1, sufijo: '', unidad: 'pct', mejorArriba: true,
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
          </tr>
        </thead>
        <tbody className="divide-y divide-[var(--border)]">
          {filas.map((m) => {
            const d = m.antes != null && m.despues != null ? diferencia(m.antes, m.despues, m.unidad, m.decimals) : null;
            const ganaDespues = d != null && (m.mejorArriba ? d.signo > 0 : d.signo < 0);
            const ganaAntes = d != null && (m.mejorArriba ? d.signo < 0 : d.signo > 0);
            const color =
              d == null || d.signo === 0
                ? 'text-[var(--text-muted)]'
                : ganaDespues
                ? 'text-[var(--compatible)]'
                : 'text-[var(--error)]';
            return (
              <tr key={m.clave} className="hover:bg-[var(--bg)]/60 transition-colors">
                <th scope="row" className="text-left px-3 py-2 font-normal">
                  <span className="text-[var(--text)] font-medium">{m.label}</span>
                  <span className="block text-[11px] text-[var(--text-dim)]">{m.hint}</span>
                </th>
                <td className={`px-3 py-2 text-right font-mono tabular-nums ${ganaAntes ? 'text-[var(--text)] font-semibold' : 'text-[var(--text-muted)]'}`}>
                  {f(m.antes, m.decimals)}{m.sufijo}
                </td>
                <td className={`px-3 py-2 text-right font-mono tabular-nums ${ganaDespues ? 'text-[var(--text)] font-semibold' : 'text-[var(--text-muted)]'}`}>
                  {f(m.despues, m.decimals)}{m.sufijo}
                </td>
                <td className={`px-3 py-2 text-right font-mono tabular-nums font-bold whitespace-nowrap ${color}`}>
                  {d ? d.texto : '—'}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
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
 * Panel principal: las dos pruebas enfrentadas y, debajo, la conclusión.
 *
 * Es un solo componente a propósito: separar el VS y el resultado en dos tarjetas hacía
 * que se leyeran como cosas distintas cuando son la misma frase («la segunda rinde peor,
 * la primera va mejor»).
 */
function PanelVS({
  comparison,
  r,
  etiquetas,
  onCopy,
}: {
  comparison: BenchmarkComparison;
  r: Interpretacion;
  etiquetas: Etiquetas;
  onCopy: (t: string) => void;
}) {
  const style = TONO[r.tono];

  const mejor = mejorObservada(r, etiquetas);

  // La fiabilidad del 0,1 % low se explica una sola vez, en la caja técnica de abajo
  // (`low01.aviso` es exactamente ese texto). Filtrarlo por identidad es fiable porque
  // el intérprete guarda el mismo string que mete en `avisos`.
  const avisosDelVeredicto = r.avisos.filter((a) => a !== r.low01.aviso);

  const baseFps = comparison.baseline.avg_fps;
  const testFps = comparison.test.avg_fps;

  // «Peor» solo cuando el cambio empeora de verdad: sin ganadora no se insulta a nadie.
  const peor = r.ganador === 'antes';

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
              {titularDelCambio(r, etiquetas)}
            </h3>
            <span
              className="text-[11px] font-bold px-2 py-0.5 rounded-md bg-[var(--surface)] text-[var(--text-muted)] border border-[var(--border)]"
              title="Esta prueba compara dos grabaciones. Para confirmar una mejora hacen falta varias grabaciones por configuración."
            >
              {r.chipConfianza}
            </span>
          </div>

          <p className="text-sm text-[var(--text)] leading-relaxed">{r.explicacion}</p>

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
            <p className="mt-2 text-sm font-semibold text-[var(--text-muted)] flex items-start gap-1.5">
              <Scale className="w-3.5 h-3.5 mt-0.5 shrink-0" aria-hidden="true" />
              <span>
                {r.estado === 'mixto'
                  ? 'Gana una prueba en un aspecto y la otra en otro: no hay un único mejor resultado.'
                  : r.estado === 'sin_cambio'
                  ? 'Ninguna de las dos destaca: la diferencia entra dentro del margen.'
                  : 'No hay datos suficientes para decir cuál rindió mejor.'}
              </span>
            </p>
          )}

          {/* El aviso de fiabilidad del 0,1 % low se excluye aquí porque ya se explica
              en la caja técnica de abajo: el mismo aviso dos veces en una pantalla es ruido. */}
          {avisosDelVeredicto.length > 0 && (
            <ul className="mt-3 pt-3 border-t border-[var(--border)] space-y-1.5">
              {avisosDelVeredicto.map((a) => (
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

export function SavedComparisonDetail({
  comparison,
  onBack,
  onDelete,
  onCopy,
}: {
  comparison: BenchmarkComparison;
  onBack: () => void;
  onDelete: () => void;
  onCopy: (t: string) => void;
}) {
  const [showDetails, setShowDetails] = useState(false);
  // Plegado por defecto, igual que los detalles técnicos.
  const [showComandos, setShowComandos] = useState(false);
  // Las etiquetas dependen solo de las dos grabaciones, así que se pueden memoizar aparte.
  const etiquetas = useMemo(() => etiquetasDe(comparison), [comparison]);
  const r = useMemo(() => interpretar(comparison, etiquetas), [comparison, etiquetas]);

  const avg = r.metricas.find((m) => m.clave === 'avg_fps')!;
  const est = r.metricas.find((m) => m.clave === 'estabilidad')!;

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
                {etiquetas.antes} {f(avg.antes)}
              </span>
              <span className="text-[var(--text-dim)]"> → </span>
              <span className={SERIE.despues.texto} title={etiquetas.despues}>
                {etiquetas.despues} {f(avg.despues)}
              </span>{' '}
              FPS
            </span>
            {avg.diferenciaPct != null && (
              <span className="font-bold font-mono tabular-nums">
                ({avg.diferenciaPct >= 0 ? '+' : ''}
                {f(avg.diferenciaPct)} %)
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
      <PanelVS comparison={comparison} r={r} etiquetas={etiquetas} onCopy={onCopy} />

      {/* 3. Comparación de rendimiento — dos tarjetas destacadas y UNA tabla con el resto.
          Antes cada medida era una caja con su borde: aquí solo hay dos cajas y una tabla. */}
      <section className="space-y-3">
        <h3 className="text-base font-bold text-[var(--text)]">Comparación de rendimiento</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <TarjetaMetrica metrica={avg} modo="fps" etiquetas={etiquetas} />
          <TarjetaMetrica metrica={est} modo="estabilidad" etiquetas={etiquetas} />
        </div>

        <TablaMetricas comparacion={comparison} etiquetas={etiquetas} />

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
          {!r.low01.fiable && (
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
                {f(comparison.baseline.duration_seconds)} s{' '}
                <span className={SERIE.antes.texto}>({etiquetas.antes})</span> ·{' '}
                {f(comparison.test.duration_seconds)} s{' '}
                <span className={SERIE.despues.texto}>({etiquetas.despues})</span>
              </p>
              <p>
                <strong className="text-[var(--text)]">Calentamiento:</strong><br />
                {f(comparison.baseline.warmup_seconds ?? 0, 0)} s ·{' '}
                {f(comparison.test.warmup_seconds ?? 0, 0)} s descartados
              </p>
              <p>
                <strong className="text-[var(--text)]">Margen estimado:</strong><br />
                {r.margenTexto ?? 'no guardado (±5 %)'}
              </p>
            </div>
            <p className="text-[11px] text-[var(--text-dim)] leading-relaxed flex items-start gap-1.5">
              <Info className="w-3 h-3 shrink-0 mt-0.5" aria-hidden="true" />
              <span>
                El margen mide el error de medición <em>dentro</em> de cada grabación, no la
                variación entre partidas distintas (otra zona, otro enemigo). Superarlo no
                confirma una mejora: solo indica que las dos grabaciones midieron distinto más
                allá del ruido de muestreo.
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
