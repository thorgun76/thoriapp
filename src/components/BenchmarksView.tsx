import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import type { RecordedSetup, Benchmark, BenchmarkComparison, GroupComparison } from '../lib/types';
import { api } from '../lib/api';
import { useMangoHud } from '../hooks/useMangoHud';
import { useBenchmarks, MismoJuego } from '../hooks/useBenchmarks';
import { useShaderCache } from '../hooks/useShaderCache';
import { ShaderStutterCard } from './ShaderStutterCard';
import { useComparison } from '../hooks/useComparison';
import { useLogWatcher } from '../hooks/useLogWatcher';
import { useConfirm, useToast } from '../hooks/useConfirm';
import { ConfirmDialog } from './ConfirmDialog';
import { fmtDuration, formatTimestamp } from '../lib/format';
import { tieThreshold, marginLabel } from '../lib/noise';
import { duraMenosDe, textoDeDuracion } from '../lib/duracion';
import { MIN_TIE_PERCENT } from '../lib/umbrales';
import {
  interpretar, ANTES, DESPUES,
  type Tono, type CambioMetrica, type Ganador,
} from '../lib/interpretacion';
import { estabilidad } from '../lib/estabilidad';
import { etiquetasDe, titularDelCambio, mejorObservada } from '../lib/interpretacion';
import { GroupComparisonCard } from './GroupComparisonCard';
import { Toggle } from './Toggle';
import { openExternal } from '../lib/links';
import { sufijoOrigen, textoComando } from '../lib/pruebaLabel';
import { PruebaN } from './PruebaN';
import { InstantaneaCard } from './InstantaneaCard';
import {
  Upload, Trash2, BarChart3, AlertCircle, Zap, FileText, Search,
  Clock, Gamepad2, Gauge, Activity, ChevronDown, Timer, ThumbsUp, ThumbsDown,
  Scale, Info, Save, Check, ExternalLink, AlertTriangle,
} from 'lucide-react';

/* ── Documentación que se enlaza desde esta pantalla ── */
const MANGOHUD_URL = 'https://github.com/flightlessmango/MangoHud';
const MANGOHUD_WIKI_URL = 'https://wiki.archlinux.org/title/MangoHud';

/** Enlace externo pequeño: se abre con Rust, que valida el dominio contra su lista. */
function HelpLink({ url, children }: { url: string; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={() => openExternal(url)}
      className="text-xs font-semibold text-[var(--action)] hover:text-[var(--action-hover)] inline-flex items-center gap-1 underline underline-offset-2 hover:no-underline transition-colors rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
    >
      <ExternalLink className="w-3 h-3" />
      {children}
    </button>
  );
}

/* ────────────────────────────── Tipos ────────────────────────────── */

// Se re-exportan para quien ya los importaba desde aquí (DashboardView).
export type { Benchmark, BenchmarkComparison };

interface BenchmarksViewProps {
  currentAppid?: number;
  currentGameName?: string;
  currentLaunchCommand?: string;
  /** Nombre del perfil activo, para dejarlo apuntado con la grabación. */
  currentProfileName?: string;
  mangohudLogPath?: string;
  onLogPathChange?: (path: string) => void;
  /**
   * ¿Hay un juego abierto ahora mismo?
   *
   * Importa porque llegados aquí desde la barra lateral (`Pruebas`, `Benchmarks`) no
   * tiene por qué haber ninguno: la app permite ver el historial sin abrir un juego. Sin
   * juego abierto no se sabe qué comando había, y guardar `%command%` sería inventárselo.
   */
  hayJuegoAbierto?: boolean;
  /**
   * Revisión del motor para la instantánea (subfase 5.2).
   *
   * Sirve para saber si dos datos de la instantánea salieron de la misma versión del
   * comando. Opcional: sin él se guarda la instantánea sin ese dato.
   */
  revisionMotor?: number | null;
  /** ¿El comando actual tiene cambios sin guardar respecto al perfil cargado? */
  cambiosSinGuardar?: boolean;
  /**
   * Lo que la interfaz sabe de la configuración **del juego abierto**.
   *
   * Se pide por juego para no atribuir a la grabación el perfil o el comando de otro. Si
   * el juego de la grabación no es el abierto, el contexto resultante lleva
   * `correspondeAlJuego: false` y el backend lo registra como desconocido.
   */
  contextoDelJuegoAbierto?: {
    appid: number;
    gameName: string;
    perfilId: string | null;
    perfilNombre: string | null;
    comando: string;
    modoProton: boolean;
  } | null;
}

/* ─────────────────────── Constantes y utilidades ─────────────────────── */

// La más reciente va en azul (#38bdf8); el resto, neutro. Verde y rojo se reservan para "mejor" y "peor".

// Grabaciones más cortas que esto dan resultados poco fiables
const SHORT_SECONDS = 45;

const DURATION_OPTIONS = [
  { seconds: 30, label: '30 s', note: 'Rápido' },
  { seconds: 60, label: '1 min', note: 'Recomendado' },
  { seconds: 90, label: '1 min 30 s', note: 'Muy fiable' },
  { seconds: 120, label: '2 min', note: 'Máxima precisión' },
];

// Segundos iniciales que se descartan: incluyen la carga del nivel y la compilación
// inicial de shaders, que no representan el rendimiento en juego.
const WARMUP_OPTIONS = [
  { seconds: 0, label: 'Nada', note: 'Todo cuenta' },
  { seconds: 10, label: '10 s', note: 'Recomendado' },
  { seconds: 15, label: '15 s', note: 'Cargas lentas' },
  { seconds: 30, label: '30 s', note: 'Muy estricto' },
];

const DEFAULT_WARMUP = 10;
const WARMUP_STORAGE_KEY = 'mangohud_warmup_seconds';
/** Recuerda si la recogida automática de grabaciones está activada. */
const AUTO_RECOGER_KEY = 'mangohud_auto_recoger';

/**
 * Estabilidad 0-100: qué parte de tu velocidad media se mantiene en los peores momentos.
 *
 * Se delega en `lib/estabilidad.ts`, que sanea NaN/∞/negativos y satura a 100. Aquí se
 * devuelve 0 solo para que `buildRows` (que trabaja con `number`) siga funcionando; los
 * datos inválidos se detectan en `lib/interpretacion.ts`, que es quien decide el veredicto.
 */
function smoothScore(b: Benchmark): number {
  return estabilidad(b.avg_fps, b.fps_1_percent_low, b.fps_0_1_percent_low) ?? 0;
}

function smoothLabel(score: number): string {
  if (score >= 65) return 'Muy fluido';
  if (score >= 50) return 'Fluido';
  if (score >= 35) return 'Algunos tirones';
  return 'Muchos tirones';
}

const hasStutterData = (b: Benchmark) =>
  b.spikes_per_min != null &&
  b.pct_over_33ms != null &&
  b.jitter_ms != null &&
  b.worst_frame_ms != null;

type Winner = 'base' | 'test' | 'tie';
type Group = 'speed' | 'smooth';
type Level = 'better' | 'same' | 'worse';

interface MetricRow {
  key: string;
  group: Group;
  label: string;
  hint: string;
  base: number;
  test: number;
  decimals: number;
  unit: string;
  higherIsBetter: boolean;
  weight: number; // 0 = se muestra, pero no cuenta para el veredicto
  floor: number;  // evita porcentajes absurdos cuando el valor base es casi 0
  change: number; // cambio % real del número
  gain: number;   // positivo = mejor, negativo = peor
  winner: Winner;
}

type RowDef = Omit<MetricRow, 'change' | 'gain' | 'winner'>;

/** Ganador de una métrica: por debajo de `tie` % la diferencia se considera empate. */
const winnerOf = (gain: number, tie: number): Winner =>
  Math.abs(gain) < tie ? 'tie' : gain > 0 ? 'test' : 'base';

function makeRow(d: RowDef): MetricRow {
  const denom = Math.max(Math.abs(d.base), d.floor);
  const change = ((d.test - d.base) / denom) * 100;
  const gain = d.higherIsBetter ? change : -change;
  const winner: Winner = winnerOf(gain, MIN_TIE_PERCENT);
  return { ...d, change, gain, winner };
}

/** Recalcula los ganadores con el margen de ruido real de las dos grabaciones. */
function applyTie(rows: MetricRow[], tie: number): MetricRow[] {
  if (!Number.isFinite(tie) || tie <= MIN_TIE_PERCENT) return rows;
  return rows.map((r) => ({ ...r, winner: winnerOf(r.gain, tie) }));
}

export function buildRows(c: BenchmarkComparison): MetricRow[] {
  const b = c.baseline;
  const t = c.test;

  const rows: MetricRow[] = [
    makeRow({
      key: 'avg', group: 'speed', label: 'FPS promedio', hint: 'Más alto = mejor',
      base: b.avg_fps, test: t.avg_fps, decimals: 1, unit: '',
      higherIsBetter: true, weight: 1, floor: 0.01,
    }),
    makeRow({
      key: 'ft', group: 'speed', label: 'Tiempo por imagen', hint: 'Menos ms = mejor',
      base: b.frame_time_avg, test: t.frame_time_avg, decimals: 2, unit: ' ms',
      higherIsBetter: false, weight: 0, floor: 0.01,
    }),
    makeRow({
      key: 'smooth', group: 'smooth', label: 'Estabilidad de FPS', hint: 'Índice propio de Thoriapp, sobre 100. Más alto = más parejo',
      base: smoothScore(b), test: smoothScore(t), decimals: 0, unit: '',
      higherIsBetter: true, weight: 3, floor: 1,
    }),
    makeRow({
      key: 'low1', group: 'smooth', label: 'Peores momentos (1% Low)', hint: 'FPS en el 1% más lento. Más alto = mejor',
      base: b.fps_1_percent_low, test: t.fps_1_percent_low, decimals: 1, unit: '',
      higherIsBetter: true, weight: 2, floor: 0.01,
    }),
    makeRow({
      key: 'low01', group: 'smooth', label: 'Peores tirones (0.1% Low)', hint: 'FPS en el 0.1% más lento. Más alto = mejor',
      base: b.fps_0_1_percent_low, test: t.fps_0_1_percent_low, decimals: 1, unit: '',
      higherIsBetter: true, weight: 1, floor: 0.01,
    }),
  ];

  // Las métricas de tirones solo existen en benchmarks nuevos: se comparan solo si ambos las tienen
  if (hasStutterData(b) && hasStutterData(t)) {
    rows.push(
      makeRow({
        key: 'spikes', group: 'smooth', label: 'Tirones bruscos por minuto', hint: 'Menos = mejor',
        base: b.spikes_per_min!, test: t.spikes_per_min!, decimals: 1, unit: '',
        higherIsBetter: false, weight: 3, floor: 1,
      }),
      makeRow({
        key: 'over33', group: 'smooth', label: 'Tiempo bajo 30 FPS', hint: '% del tiempo. Menos = mejor',
        base: b.pct_over_33ms!, test: t.pct_over_33ms!, decimals: 1, unit: '%',
        higherIsBetter: false, weight: 2, floor: 1,
      }),
      makeRow({
        key: 'jitter', group: 'smooth', label: 'Saltos entre imágenes', hint: 'Menos ms = mejor',
        base: b.jitter_ms!, test: t.jitter_ms!, decimals: 2, unit: ' ms',
        higherIsBetter: false, weight: 1, floor: 0.5,
      }),
      makeRow({
        key: 'worst', group: 'smooth', label: 'Peor tirón', hint: 'Menos ms = mejor',
        base: b.worst_frame_ms!, test: t.worst_frame_ms!, decimals: 1, unit: ' ms',
        higherIsBetter: false, weight: 0, floor: 5,
      }),
    );
  }

  return applyTie(rows, tieThreshold(c.noise_percent));
}

/**
 * Nivel de una familia de métricas para el chip de cada tarjeta.
 *
 * Ya no hay puntuación ponderada (los pesos 3/2/1 se han retirado): el resultado lo
 * decide `interpretar()` en `lib/interpretacion.ts`, que es el único intérprete que
 * queda. Aquí solo se traduce el cambio de una métrica a su etiqueta visual.
 */
const levelOfCambio = (c: CambioMetrica): Level =>
  c === 'mejor' ? 'better' : c === 'peor' ? 'worse' : 'same';

/** Nombres visibles de cada grabación. Únicos para toda la aplicación. */
export { ANTES as NOMBRE_ANTES, DESPUES as NOMBRE_DESPUES };

/** Color de cada estado del resultado, compartido con la vista de pruebas guardadas. */
export const TONO_STYLE: Record<Tono, { box: string; iconBox: string; text: string; Icon: typeof ThumbsUp }> = {
  bien:     { box: 'border-compatible/60 bg-compatible/10', iconBox: 'bg-compatible/20', text: 'text-[var(--compatible)]', Icon: ThumbsUp },
  mal:      { box: 'border-error/60 bg-error/10',           iconBox: 'bg-error/20',      text: 'text-[var(--error)]',      Icon: ThumbsDown },
  atencion: { box: 'border-caution/60 bg-caution/10',       iconBox: 'bg-caution/20',    text: 'text-[var(--caution)]',    Icon: Scale },
  neutro:   { box: 'border-[var(--border)] bg-[var(--bg)]', iconBox: 'bg-border/40',     text: 'text-[var(--text-muted)]', Icon: Scale },
};

const LEVEL_STYLE: Record<Level, { chip: string; word: string }> = {
  better: { chip: 'bg-compatible/15 text-[var(--compatible)]', word: 'Mejor' },
  same:   { chip: 'bg-border/40 text-[var(--text-muted)]',     word: 'Igual' },
  worse:  { chip: 'bg-error/15 text-[var(--error)]',           word: 'Peor' },
};

/* ─────────────────────────── Componentes pequeños ─────────────────────────── */

/** Dos barras (la de antes y la nueva) con el mismo criterio, para ver quién gana de un vistazo. */
function CompareMeter({
  icon,
  title,
  help,
  base,
  test,
  decimals,
  unit,
  level,
  footer,
  etiquetaBase,
  etiquetaTest,
}: {
  icon: ReactNode;
  title: string;
  help: string;
  base: number;
  test: number;
  decimals: number;
  unit: string;
  level: Level;
  footer: string;
  /** Cómo se llama cada grabación: «Prueba 1» o «Antes» en las antiguas. */
  etiquetaBase: string;
  etiquetaTest: string;
}) {
  const max = Math.max(base, test, 0.0001);
  const pct = (v: number) => `${Math.max(4, (v / max) * 100)}%`;
  const style = LEVEL_STYLE[level];

  return (
    <div className="p-4 rounded-xl bg-[var(--bg)] border border-[var(--border)]">
      <div className="flex items-center justify-between gap-2 mb-1">
        <h4 className="text-sm font-bold text-[var(--text)] flex items-center gap-2">
          {icon}
          {title}
        </h4>
        <span className={`text-xs font-bold px-2.5 py-1 rounded-md whitespace-nowrap ${style.chip}`}>
          {style.word}
        </span>
      </div>
      <p className="text-xs text-[var(--text-muted)] mb-4">{help}</p>

      <div className="space-y-3">
        <div>
          <div className="flex justify-between text-xs mb-1">
            <span className="font-semibold text-[#38bdf8]" title={etiquetaBase}>{etiquetaBase}</span>
            <span className="font-bold text-[var(--text)]">{base.toFixed(decimals)}{unit}</span>
          </div>
          <div className="h-2.5 rounded-full bg-[var(--surface)] overflow-hidden">
            <div className="h-full rounded-full bg-[#38bdf8] transition-all duration-500" style={{ width: pct(base) }} />
          </div>
        </div>
        <div>
          <div className="flex justify-between text-xs mb-1">
            <span className="font-semibold text-[var(--caution)]" title={etiquetaTest}>{etiquetaTest}</span>
            <span className="font-bold text-[var(--text)]">{test.toFixed(decimals)}{unit}</span>
          </div>
          <div className="h-2.5 rounded-full bg-[var(--surface)] overflow-hidden">
            <div className="h-full rounded-full bg-[var(--caution)] transition-all duration-500" style={{ width: pct(test) }} />
          </div>
        </div>
      </div>

      <p className="text-xs text-[var(--text-muted)] mt-3">{footer}</p>
    </div>
  );
}

export function DetailsTable({
  rows,
  showEstabilidad = false,
  etiquetaBase = 'Antes',
  etiquetaTest = 'Después',
}: {
  rows: MetricRow[];
  showEstabilidad?: boolean;
  etiquetaBase?: string;
  etiquetaTest?: string;
}) {
  const groups: { id: Group; title: string }[] = [
    { id: 'speed', title: 'FPS medios' },
    { id: 'smooth', title: showEstabilidad ? 'Estabilidad y peores momentos' : 'Fluidez' },
  ];

  return (
    <div className="overflow-x-auto mt-3">
      <div className="min-w-[520px] space-y-4">
        <div className="grid grid-cols-[1.6fr_0.8fr_0.8fr_1.4fr] gap-3 px-4 text-xs font-semibold">
          <span className="text-[var(--text-dim)]">Medida</span>
          <span className="text-[#38bdf8] text-right">{etiquetaBase}</span>
          <span className="text-[var(--caution)] text-right">{etiquetaTest}</span>
          <span className="text-[var(--text-dim)] text-right">Resultado</span>
        </div>

        {groups.map((g) => (
          <div key={g.id} className="space-y-2">
            <p className="px-4 text-xs font-bold text-[var(--text-muted)]">{g.title}</p>
            {rows.filter((r) => r.group === g.id).map((r) => {
              const chip =
                r.winner === 'tie'
                  ? 'bg-border/40 text-[var(--text-muted)]'
                  : r.gain > 0
                  ? 'bg-compatible/15 text-[var(--compatible)]'
                  : 'bg-error/15 text-[var(--error)]';
              const word = r.winner === 'tie' ? 'Sin cambio concluyente' : r.gain > 0 ? 'Mejor' : 'Peor';

              return (
                <div
                  key={r.key}
                  className="grid grid-cols-[1.6fr_0.8fr_0.8fr_1.4fr] gap-3 items-center px-4 py-2.5 rounded-lg bg-[var(--bg)] border border-[var(--border)]"
                >
                  <div>
                    <p className="text-sm font-semibold text-[var(--text)]">{r.label}</p>
                    <p className="text-[11px] text-[var(--text-dim)]">{r.hint}</p>
                  </div>
                  <span className={`text-right text-sm ${r.winner === 'base' ? 'font-bold text-[var(--text)]' : 'text-[var(--text-muted)]'}`}>
                    {r.base.toFixed(r.decimals)}{r.unit}
                  </span>
                  <span className={`text-right text-sm ${r.winner === 'test' ? 'font-bold text-[var(--text)]' : 'text-[var(--text-muted)]'}`}>
                    {r.test.toFixed(r.decimals)}{r.unit}
                  </span>
                  <div className="flex justify-end">
                    <span className={`text-xs font-bold px-2 py-1 rounded-md whitespace-nowrap ${chip}`}>
                      {r.change > 0 ? '▲' : r.change < 0 ? '▼' : '='} {Math.abs(r.change).toFixed(1)}% · {word}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}

/**
 * Quién gana en una comparación, para poder mostrarlo fuera de esta pantalla.
 *
 * Delega en `interpretar()`, el único intérprete de la aplicación. Los nombres de la
 * unión de tipos se mantienen (`empate`/`depende`) para no tocar a sus consumidores.
 */
export type OverallWinner = 'antes' | 'despues' | 'empate' | 'depende';

const GANADOR_A_WINNER: Record<Ganador, OverallWinner> = {
  antes: 'antes',
  despues: 'despues',
  ninguno: 'empate',
  mixto: 'depende',
};

export function summarizeComparison(c: BenchmarkComparison): {
  winner: OverallWinner;
  title: string;
} {
  const r = interpretar(c, etiquetasDe(c));
  return {
    winner: GANADOR_A_WINNER[r.ganador],
    title: r.titulo,
  };
}

/* ───────────────────── Tarjeta de resultado (se reutiliza en Mis pruebas) ───────────────────── */

export function ComparisonResultCard({
  comparison,
  onClose,
  closeLabel = 'Cerrar comparación',
  title = 'Resultado de la comparación',
  groupRows,
  children,
}: {
  comparison: BenchmarkComparison;
  onClose?: () => void;
  closeLabel?: string;
  title?: string;
  /**
   * Comparación de todas las grabaciones del juego. Es la que puede decir si la
   * diferencia de estas dos es real; la de una sola contra otra no puede.
   */
  groupRows?: GroupComparison[] | null;
  children?: ReactNode;
}) {
  const [showDetails, setShowDetails] = useState(false);

  // Único intérprete: el mismo que usa la vista de pruebas guardadas. Los avisos que
  // antes calculaba `buildWarnings` vienen ya de aquí, con las mismas reglas.
  const etiquetas = etiquetasDe(comparison);
  const r = interpretar(comparison, etiquetas);
  const rows = buildRows(comparison);
  // Titular y ganadora con la MISMA redacción que la vista de prueba guardada: el
  // «Gana “Prueba 1”» en rojo se leía al revés (parecía que la ganadora era la mala).
  const mejor = mejorObservada(r, etiquetas);
  const style = TONO_STYLE[r.tono];
  const VerdictIcon = style.Icon;

  const smoothRow = rows.find((rr) => rr.key === 'smooth')!;
  const avg = r.metricas.find((m) => m.clave === 'avg_fps')!;
  const est = r.metricas.find((m) => m.clave === 'estabilidad')!;
  const noiseText = r.margenTexto;

  const smoothFooter =
    est.cambio === 'mejor'
      ? `Con ${etiquetas.despues} el juego se mantiene más parejo.`
      : est.cambio === 'peor'
      ? `Con ${etiquetas.despues} el juego tiene más tirones.`
      : est.cambio === 'sin_datos'
      ? 'No hay datos suficientes para comparar la estabilidad.'
      : 'El juego se mantiene igual de parejo.';

  return (
    <article
      className="p-5 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-xl shadow-action/10"
    >
      <h3 className="text-base font-bold text-[var(--text)] mb-4">{title}</h3>

      {r.avisos.length > 0 && (
        <div className="mb-4 p-3 rounded-lg bg-[var(--caution-bg)] border border-caution/50 space-y-1.5">
          {r.avisos.map((w) => (
            <p key={w} className="text-xs text-[var(--caution)] font-semibold flex items-start gap-2">
              <AlertCircle className="w-4 h-4 shrink-0" />
              {w}
            </p>
          ))}
        </div>
      )}

      {/* Veredicto */}
      <div className={`p-5 rounded-xl border-2 mb-4 flex items-start gap-4 ${style.box}`}>
        <div className={`w-12 h-12 rounded-xl flex items-center justify-center shrink-0 ${style.iconBox}`}>
          <VerdictIcon className={`w-6 h-6 ${style.text}`} />
        </div>
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2 mb-1">
            <p className={`text-xl font-bold ${style.text}`}>{titularDelCambio(r, etiquetas)}</p>
            <span className="text-[11px] font-bold px-2 py-0.5 rounded-md bg-[var(--surface)] text-[var(--text-muted)] border border-[var(--border)]">
              {r.chipConfianza}
            </span>
          </div>
          <p className="text-sm text-[var(--text)] mt-1">{r.explicacion}</p>
          {mejor ? (
            <p className="text-sm font-semibold text-[var(--text)] mt-2">
              Mejor resultado observado: {mejor.nombre}
            </p>
          ) : null}
          {noiseText && (
            <p className="text-xs text-[var(--text-muted)] mt-2 flex items-start gap-1.5">
              <Info className="w-3.5 h-3.5 shrink-0 mt-0.5" />
              <span>
                Margen de ruido de estas grabaciones: <strong>{noiseText}</strong>. Una diferencia menor
                puede ser simple variación entre partidas; repite la prueba antes de decidir.
              </span>
            </p>
          )}
          {/* Las dos grabaciones no bastan para concluir: se dice aquí, junto al
              titular, para que no se lea solo el «gana» y se ignore el resto. */}
          {sinEvidenciaDeGrupo(groupRows) && (
            <p className="text-xs font-semibold text-[var(--caution)] mt-2 flex items-start gap-1.5">
              <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
              <span>
                Todavía no hay pruebas de que este cambio sea real: con las grabaciones
                que tienes, la diferencia entra dentro del ruido. Graba la misma escena
                varias veces antes y después.
              </span>
            </p>
          )}
        </div>
      </div>

      {/* FPS medios y estabilidad. Los niveles salen del intérprete compartido, así que
          esta tarjeta y la vista de pruebas guardadas no pueden discrepar. */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
        <CompareMeter
          icon={<Gauge className="w-4 h-4 text-[var(--action)]" />}
          title="FPS medios"
          help="Cuántas imágenes por segundo (FPS) muestra el juego."
          base={avg.antes ?? 0}
          test={avg.despues ?? 0}
          decimals={0}
          unit=" FPS"
          level={levelOfCambio(avg.cambio)}
          etiquetaBase={etiquetas.antes}
          etiquetaTest={etiquetas.despues}
          footer={
            avg.diferencia == null
              ? 'No hay FPS medios guardados.'
              : `${avg.diferencia >= 0 ? '+' : ''}${avg.diferencia.toFixed(1)} FPS${
                  avg.diferenciaPct != null
                    ? ` (${avg.diferenciaPct >= 0 ? '+' : ''}${avg.diferenciaPct.toFixed(1)} %)`
                    : ''
                }${noiseText ? `, margen ${noiseText}` : ''}.`
          }
        />
        <CompareMeter
          icon={<Activity className="w-4 h-4 text-[var(--action)]" />}
          title="Estabilidad de FPS"
          help="Índice propio de Thoriapp (0-100): qué parte de la velocidad media se mantiene en los peores momentos. Alto no significa que la velocidad sea suficiente."
          base={smoothRow.base}
          test={smoothRow.test}
          decimals={0}
          unit=""
          level={levelOfCambio(est.cambio)}
          etiquetaBase={etiquetas.antes}
          etiquetaTest={etiquetas.despues}
          footer={smoothFooter}
        />
      </div>

      {/* Detalles técnicos */}
      <button
        type="button"
        onClick={() => setShowDetails((s) => !s)}
        aria-expanded={showDetails}
        className="w-full flex items-center justify-between px-4 py-2.5 rounded-lg text-sm font-semibold text-[var(--text-muted)] bg-[var(--bg)] border border-[var(--border)] hover:border-action/50 hover:text-[var(--text)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
      >
        Ver detalles técnicos
        <ChevronDown className={`w-4 h-4 transition-transform duration-200 ${showDetails ? 'rotate-180' : ''}`} />
      </button>
      {showDetails && (
        <DetailsTable
          rows={rows}
          etiquetaBase={etiquetas.antes}
          etiquetaTest={etiquetas.despues}
        />
      )}

      {children}

      {onClose && (
        <button
          type="button"
          onClick={onClose}
          className="w-full mt-4 px-4 py-2.5 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] hover:border-action/50 transition-all duration-200"
        >
          {closeLabel}
        </button>
      )}
    </article>
  );
}

/**
 * Compara el historial del juego en dos mitades: las grabaciones antiguas contra las
 * recientes.
 *
 * La pantalla ya compara automáticamente las dos últimas, pero dos grabaciones no
 * distinguen una mejora real del ruido. Esto usa todas las del historial: si hay 6, se
 * compara la mitad vieja contra la mitad nueva, que es el reparto más honesto sin
 * que el usuario tenga que marcar nada. Con menos de `MIN_RUNS` por lado, el backend
 * responde que no se puede concluir, y eso también se enseña.
 */
function useGroupComparison(
  benchmarks: Benchmark[]
): GroupComparison[] | null {
  const [rows, setRows] = useState<GroupComparison[] | null>(null);

  useEffect(() => {
    if (benchmarks.length < 2) {
      setRows(null);
      return;
    }
    // `benchmarks` llega de la más antigua a la más reciente.
    const half = Math.floor(benchmarks.length / 2);
    const antes = benchmarks.slice(0, half).map((b) => b.id);
    const despues = benchmarks.slice(half).map((b) => b.id);

    let cancelled = false;
    (async () => {
      try {
        const result = await api.compareBenchmarkGroups(antes, despues);
        if (!cancelled) setRows(result);
      } catch {
        // Comparar grupos es una ayuda: si falla (grabaciones borradas a mano, por
        // ejemplo) no se rompe la pantalla, simplemente no se muestra.
        if (!cancelled) setRows(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [benchmarks]);

  return rows;
}

/**
 * ¿El grupo dice que no hay diferencia demostrable?
 *
 * Sin datos de grupo (menos de dos grabaciones) no se avisa: no hay nada que
 * contradecir. Con datos, si ninguna métrica supera el ruido, el veredicto de una
 * sola comparación es una pista, no un resultado.
 */
function sinEvidenciaDeGrupo(rows?: GroupComparison[] | null): boolean {
  if (!rows || rows.length === 0) return false;
  return !rows.some((r) => r.verdict === 'mejora' || r.verdict === 'empeora');
}

// `sufijoOrigen` y `textoComando` viven en `lib/pruebaLabel` para compartirse con el
// tooltip de `PruebaN` y con «Mis pruebas» sin dependencias circulares.
export { sufijoOrigen, textoComando };

/* ─────────────────────────────── Vista principal ─────────────────────────────── */
export function BenchmarksView({
  currentAppid,
  currentGameName,
  currentLaunchCommand,
  currentProfileName,
  mangohudLogPath,
  onLogPathChange,
  hayJuegoAbierto = true,
  revisionMotor = null,
  cambiosSinGuardar = false,
  contextoDelJuegoAbierto = null,
}: BenchmarksViewProps) {
  const [logPath, setLogPath] = useState(mangohudLogPath || '/tmp/MangoHud');
  const [notes, setNotes] = useState('');
  // Segundos iniciales que se descartan al analizar. Se recuerda entre sesiones.
  const [warmup, setWarmup] = useState<number>(() => {
    const raw = localStorage.getItem(WARMUP_STORAGE_KEY);
    const saved = raw === null ? Number.NaN : Number(raw);
    return WARMUP_OPTIONS.some((o) => o.seconds === saved) ? saved : DEFAULT_WARMUP;
  });
  const { toast, setToast } = useToast();
  const { confirmState, ask, close } = useConfirm();

  // Los hooks notifican con un toast propio de esta vista.
  const notify = (message: string) => setToast(message);

  // Copiar el comando desde el tooltip de «Prueba N» del historial. Recibe el comando ya
  // limpio (sin el prefijo «Steam:»), listo para pegar en Steam.
  const copiarComando = async (texto: string) => {
    try {
      await navigator.clipboard.writeText(texto);
      setToast('✓ Comando copiado');
    } catch {
      setToast('❌ No se pudo copiar');
    }
  };

  const {
    installed: mangohudInstalled,
    duration: logDuration,
    changeDuration,
    detectLogPath,
    restoreConfig,
  } = useMangoHud(notify);

  const {
    benchmarks,
    analyzing,
    analyze,
    remove: deleteBenchmark,
    removeAll: deleteAllBenchmarks,
    removeAllForGame: deleteAllForGame,
    error: errorHistorial,
  } = useBenchmarks({
    currentAppid,
    currentGameName,
    currentLaunchCommand,
    // Grabación de un juego que no está abierto: se recupera el último comando que la app
    // registró para ese juego. Se pasa el nombre porque el appid del CSV llega a 0.
    comandoParaJuego: (juego) => api.ultimoComandoRegistrado(juego),
    // Y lo que Steam tiene puesto de verdad, que es lo que se ejecutó al entrar al juego.
    // Se manda el nombre porque el AppID del CSV es 0: el backend resuelve por nombre.
    steamParaJuego: (juego) =>
      api.steamLaunchOptionsParaJuego(juego).then((o) => o?.opciones ?? null),
    // Instantánea: solo se atribuye la configuración al juego de la grabación cuando es el
    // mismo que está abierto. Si el CSV es de otro juego (o no hay ninguno abierto), el
    // contexto va marcado como ajeno y el backend guarda la configuración como
    // desconocida, en lugar de apuntarle el perfil de otro juego.
    contextoParaJuego: async ({ appid, gameName }) => {
      const abierto = contextoDelJuegoAbierto;
      if (!abierto || !hayJuegoAbierto) return null;
      const mismaAppid =
        appid !== 0 && abierto.appid !== 0 && appid === abierto.appid;
      // El CSV puede venir sin AppID: el nombre es entonces la única identidad.
      const mismoNombre =
        !!gameName &&
        !!abierto.gameName &&
        MismoJuego(gameName, abierto.gameName);
      const corresponde = mismaAppid || (appid === 0 && mismoNombre);

      return {
        appidReal: corresponde ? abierto.appid || null : appid !== 0 ? appid : null,
        comandoConfirmado: abierto.comando,
        revisionMotor,
        opcionesSteam: null,
        perfilId: corresponde ? abierto.perfilId : null,
        perfilNombre: corresponde ? abierto.perfilNombre : null,
        cambiosSinGuardar,
        modoProton: corresponde ? abierto.modoProton : null,
        correspondeAlJuego: corresponde,
      };
    },
    onNotify: notify,
  });

  // Caché de shaders del juego para el diagnóstico del arranque (solo recorre discos).
  const shaderCache = useShaderCache(currentAppid);

  // La lista llega de Rust en orden de guardado (la más antigua primero). Aquí se
  // muestra al revés: lo más reciente arriba, que es lo que interesa al entrar.
  const shown: Benchmark[] = [...benchmarks].reverse();

  /**
   * Qué está enseñando esta vista, y por tanto qué alcance tienen sus textos y sus acciones.
   *
   * La pestaña general **no** es «el historial de un juego»: carga todos los juegos cuando no
   * hay ninguno abierto y se limita a uno cuando lo hay (`useBenchmarks` decide por
   * `currentAppid`). El encabezado y el botón de borrar
   * estaban escritos como si siempre fuera un solo juego, así que con ocho pruebas de dos
   * juegos la cabecera decía «Historial de este juego» y el borrado se anunciaba como «Las
   * pruebas de otros juegos no se tocan» mientras se ofrecía a borrar el archivo entero.
   *
   * Se deriva una sola vez y lo consumen el título, el borrado y su confirmación: si cada
   * sitio lo dedujera por su cuenta, volverían a discrepar.
   */
  const alcance =
    currentAppid || currentGameName
      ? { tipo: 'juego' as const, nombre: currentGameName ?? 'este juego' }
      : { tipo: 'general' as const, nombre: null };

  // Cuántos juegos distintos hay de verdad en lo que se enseña. Sirve para no afirmar
  // «todos los juegos» cuando en realidad solo hay uno guardado.
  const juegosEnPantalla = new Set(
    shown.map((b) => (b.appid ? `appid:${b.appid}` : `nombre:${b.game_name.trim().toLowerCase()}`))
  ).size;

  // Se compara siempre la más reciente contra la siguiente más antigua del mismo juego.
  const antesId = shown[1]?.id;
  const despuesId = shown[0]?.id;

  const {
    comparison,
    isSaved: isCurrentSaved,
    saving: savingComparison,
    save: saveCurrentComparison,
    clear: clearComparison,
  } = useComparison(antesId, despuesId, notify);

  // La comparación de dos grabaciones es una pista; el grupo dice si es real.
  const groupRows = useGroupComparison(benchmarks);

  // Recogida automática: el usuario graba con Shift+F2 dentro del juego y la app se
  // encarga de analizar y guardar lo que aparece. Se recuerda entre sesiones.
  const [autoRecoger, setAutoRecoger] = useState(() => {
    // Apagado por defecto: empezar a analizar ficheros sin que el usuario lo pida
    // sorprende, y esto guarda cosas en su historial.
    return localStorage.getItem(AUTO_RECOGER_KEY) === '1';
  });
  const [ultimaRecogida, setUltimaRecogida] = useState<string | null>(null);

  /**
   * Instantánea del comando configurado, tomada en el momento de la acción.
   *
   * El valor se toma **aquí**, no dentro del guardado: entre pulsar «analizar» y que el
   * backend escriba pasan segundos, y en ese hueco el usuario puede cambiar el perfil.
   *
   * `command` va tal cual (solo se recortan los extremos). No se tokeniza, no se
   * reordena y no se quitan `mangohud` ni los wrappers: es el texto que el usuario tiene
   * configurado, y reconstruirlo cambiaría la historia de la prueba.
   */
  const capturarSetup = (): RecordedSetup => ({
    command: (currentLaunchCommand || '').trim(),
    profileName: currentProfileName || undefined,
    expectedSeconds: logDuration ?? undefined,
    at: new Date().toISOString(),
  });

  const { checkNow, checking } = useLogWatcher({
    folder: logPath,
    enabled: autoRecoger,
    // El comando se lee cuando aparece la grabación, no al guardarla: si se leyera
    // después, una grabación hecha con `mangohud %command%` se guardaría con el comando
    // que el usuario tiene puesto ahora, y la comparación diría que no cambió nada.
    captureSetup: capturarSetup,
    process: async (log, setup) => {
      const saved = await analyze(log.path, '', warmup, setup);
      if (saved) {
        setUltimaRecogida(`${log.program} · ${log.timestamp}`);
      }
      return saved;
    },
    onError: (msg) => notify(msg),
  });

  const toggleAutoRecoger = (next: boolean) => {
    setAutoRecoger(next);
    localStorage.setItem(AUTO_RECOGER_KEY, next ? '1' : '0');
    if (next) {
      // Sin juego abierto no hay comando que registrar, y la grabación se guardaría como
      // «no registrado» aunque el usuario tenga ajustes puestos. Se avisa ANTES de grabar,
      // no después de que la prueba ya esté guardada sin el dato.
      notify(
        hayJuegoAbierto
          ? '✓ Guardado automático activado. Graba con Shift + F2 dentro del juego.'
          : '⚠ No hay ningún juego abierto: abre el juego en la Biblioteca primero o la prueba se guardará sin su comando.'
      );
      void checkNow();
    } else {
      notify('Guardado automático desactivado.');
    }
  };

  useEffect(() => {
    if (mangohudLogPath) setLogPath(mangohudLogPath);
  }, [mangohudLogPath]);

  const handleLogPathChange = (newPath: string) => {
    setLogPath(newPath);
    localStorage.setItem('mangohud_log_path', newPath);
    onLogPathChange?.(newPath);
  };

  const analyzeLog = async () => {
    // También el guardado manual necesita la instantánea: antes se pasaba `undefined` y
    // se caía al comando "actual" en el momento de guardar, que no es el de la grabación.
    const saved = await analyze(logPath, notes, warmup, capturarSetup());
    if (saved) setNotes('');
  };

  const changeWarmup = (seconds: number) => {
    setWarmup(seconds);
    localStorage.setItem(WARMUP_STORAGE_KEY, String(seconds));
  };

  // Confirmaciones propias de la app (en vez de confirm() nativo, que bloquea la ventana)
  const askDeleteAll = () =>
    ask({
      title: 'Borrar el historial',
      // El alcance de la confirmación tiene que ser el mismo que el de la vista. Antes decía
      // siempre «de este juego… los demás no se tocan», también cuando la vista enseñaba
      // varios juegos y el botón borraba **todo**.
      message:
        alcance.tipo === 'juego'
          ? `Se eliminarán las ${shown.length} grabaciones de ${alcance.nombre}. Los demás juegos no se tocan. Esta acción no se puede deshacer.`
          : `Se eliminarán las ${shown.length} grabaciones de ${juegosEnPantalla === 1 ? '1 juego' : `los ${juegosEnPantalla} juegos`} del historial. Se borrarán todas las pruebas guardadas. Esta acción no se puede deshacer.`,
      confirmLabel: 'Borrar historial',
      // **Alcance real del borrado.** Antes llamaba siempre a `deleteAllBenchmarks`, que vacía
      // el archivo entero. Desde la ficha de un juego eso se llevaba por delante las pruebas
      // de todos los demás, justo lo contrario de lo que la confirmación prometía.
      onConfirm: alcance.tipo === 'juego' ? deleteAllForGame : deleteAllBenchmarks,
    });

  const askDeleteBenchmark = (id: string) =>
    ask({
      title: 'Eliminar esta grabación',
      message: 'Se eliminará esta grabación del historial. Esta acción no se puede deshacer.',
      confirmLabel: 'Eliminar',
      onConfirm: () => deleteBenchmark(id),
    });

  const durationIsPreset = DURATION_OPTIONS.some((o) => o.seconds === logDuration);
  const durationHint =
    logDuration === null
      ? 'Leyendo tu configuración…'
      : logDuration === 0
      ? 'Ahora no hay límite: tendrás que pulsar Shift + F2 otra vez para parar la grabación.'
      : durationIsPreset
      ? `Dentro del juego pulsa Shift + F2 para empezar. La grabación se detiene sola a los ${fmtDuration(logDuration)}.`
      : `Ahora cada grabación dura ${fmtDuration(logDuration)}. Elige una opción para cambiarlo.`;

  return (
    <div className="space-y-6">
      {/* Toast */}
      {toast && (
        <div
          role="status"
          className="fixed bottom-6 right-6 z-50 px-5 py-3 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-xl shadow-action/20 text-sm font-semibold text-white"
        >
          {toast}
        </div>
      )}

      {/* Aviso: sin MangoHud no hay nada que grabar */}
      {mangohudInstalled === false && (
        <article className="p-5 rounded-xl bg-[var(--caution-bg)] border-2 border-[var(--caution)]">
          <div className="flex items-start gap-3">
            <AlertCircle className="w-6 h-6 text-[var(--caution)] shrink-0 mt-0.5" />
            <div className="flex-1">
              <h3 className="text-base font-bold text-[var(--caution)] mb-2">
                MangoHud no está instalado
              </h3>
              <p className="text-sm text-[var(--text)] mb-3">
                MangoHud es necesario para grabar logs de rendimiento y analizar el impacto de los parámetros de optimización.
              </p>
              <div className="space-y-2 text-sm text-[var(--text-muted)]">
                <p className="font-semibold">Cómo instalar MangoHud:</p>
                <div className="bg-[var(--bg)] p-3 rounded-lg font-mono text-xs">
                  <p className="text-[var(--text-dim)]"># Ubuntu/Debian/Pop!_OS</p>
                  <p className="text-[var(--compatible)]">sudo apt install mangohud</p>
                  <p className="text-[var(--text-dim)] mt-2"># Fedora</p>
                  <p className="text-[var(--compatible)]">sudo dnf install mangohud</p>
                  <p className="text-[var(--text-dim)] mt-2"># Arch/Manjaro</p>
                  <p className="text-[var(--compatible)]">sudo pacman -S mangohud</p>
                </div>
                <div className="flex flex-wrap items-center gap-x-4 gap-y-1 pt-1">
                  <HelpLink url={MANGOHUD_URL}>Documentación oficial</HelpLink>
                  <HelpLink url={MANGOHUD_WIKI_URL}>Guía paso a paso (Arch Wiki)</HelpLink>
                </div>
              </div>
            </div>
          </div>
        </article>
      )}

      {/* Grabar y analizar */}
      <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <div className="mb-4">
          <h3 className="text-base font-bold text-[var(--text)] flex items-center gap-2">
            <Upload className="w-5 h-5 text-[var(--action)]" />
            Grabar y analizar
          </h3>
          <p className="text-sm text-[var(--text-muted)] mt-1">
            Graba con <kbd className="px-1.5 py-0.5 rounded bg-[var(--bg)] border border-[var(--border)] text-[11px] font-semibold text-[var(--text)]">Shift + F2</kbd> dentro del juego y analiza el resultado aquí.
          </p>
          <div className="mt-2 p-2.5 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
            <p className="text-xs text-[var(--text-muted)] flex items-start gap-1.5">
              <Info className="w-3.5 h-3.5 shrink-0 mt-0.5 text-[var(--action)]" />
              <span>
                La grabación la hace <strong className="text-[var(--text)]">MangoHud</strong>, no esta app: tienes que
                tenerlo instalado y activo en el juego. Sin él, Shift + F2 no hace nada.
              </span>
            </p>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 mt-2 pl-5">
              <HelpLink url={MANGOHUD_URL}>Cómo instalar MangoHud</HelpLink>
              <HelpLink url={MANGOHUD_WIKI_URL}>Guía de MangoHud (Arch Wiki)</HelpLink>
            </div>
          </div>
        </div>

        <div className="space-y-4">
          {/* Duración de la grabación */}
          <div className="p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
            <div className="flex items-center gap-2 mb-3">
              <Timer className="w-4 h-4 text-[var(--action)]" />
              <p className="text-sm font-bold text-[var(--text)]">¿Cuánto debe durar cada grabación?</p>
            </div>

            <div className="grid grid-cols-2 sm:grid-cols-4 gap-2" role="radiogroup" aria-label="Duración de la grabación">
              {DURATION_OPTIONS.map((opt) => {
                const active = logDuration === opt.seconds;
                return (
                  <button
                    key={opt.seconds}
                    type="button"
                    role="radio"
                    aria-checked={active}
                    onClick={() => changeDuration(opt.seconds)}
                    className={`px-3 py-2.5 rounded-lg border-2 text-center transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)] ${
                      active
                        ? 'border-[var(--action)] bg-action/15'
                        : 'border-[var(--border)] hover:border-action/50'
                    }`}
                  >
                    <span className={`block text-sm font-bold ${active ? 'text-[var(--text)]' : 'text-[var(--text-muted)]'}`}>
                      {opt.label}
                    </span>
                    <span className={`block text-[11px] ${opt.seconds === 60 ? 'text-[var(--action)] font-semibold' : 'text-[var(--text-dim)]'}`}>
                      {opt.note}
                    </span>
                  </button>
                );
              })}
            </div>

            <p className="text-xs text-[var(--text-muted)] mt-3 flex items-start gap-1.5">
              <Info className="w-3.5 h-3.5 shrink-0 mt-0.5" />
              <span>
                {durationHint} Más tiempo da resultados más fiables. Si cambias esto con el juego abierto y no se aplica, reinícialo.
              </span>
            </p>

            {/* Deshacer: la copia .thoriapp.bak se crea antes de cada cambio, pero
                hasta ahora no había forma de restaurarla desde aquí. */}
            <button
              type="button"
              onClick={restoreConfig}
              title="Vuelve al MangoHud.conf que había antes de que Thoriapp lo modificara"
              className="mt-3 text-xs text-[var(--text-muted)] hover:text-[var(--action)] underline underline-offset-2 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)] rounded"
            >
              Deshacer mis cambios en MangoHud.conf
            </button>
          </div>

          {/* Calentamiento: se descarta el arranque de la grabación */}
          <div className="p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
            <div className="flex items-center gap-2 mb-3">
              <Clock className="w-4 h-4 text-[var(--action)]" />
              <p className="text-sm font-bold text-[var(--text)]">¿Cuánto arranque descartar?</p>
            </div>

            <div className="grid grid-cols-2 sm:grid-cols-4 gap-2" role="radiogroup" aria-label="Segundos iniciales que se descartan">
              {WARMUP_OPTIONS.map((opt) => {
                const active = warmup === opt.seconds;
                return (
                  <button
                    key={opt.seconds}
                    type="button"
                    role="radio"
                    aria-checked={active}
                    onClick={() => changeWarmup(opt.seconds)}
                    className={`px-3 py-2.5 rounded-lg border-2 text-center transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)] ${
                      active
                        ? 'border-[var(--action)] bg-action/15'
                        : 'border-[var(--border)] hover:border-action/50'
                    }`}
                  >
                    <span className={`block text-sm font-bold ${active ? 'text-[var(--text)]' : 'text-[var(--text-muted)]'}`}>
                      {opt.label}
                    </span>
                    <span className={`block text-[11px] ${opt.seconds === DEFAULT_WARMUP ? 'text-[var(--action)] font-semibold' : 'text-[var(--text-dim)]'}`}>
                      {opt.note}
                    </span>
                  </button>
                );
              })}
            </div>

            <p className="text-xs text-[var(--text-muted)] mt-3 flex items-start gap-1.5">
              <Info className="w-3.5 h-3.5 shrink-0 mt-0.5" />
              <span>
                Al empezar a grabar, el juego todavía carga el nivel y compila shaders: esos segundos hunden los
                «peores momentos» sin que el juego vaya peor de verdad. Se descartan los primeros segundos de
                cada grabación (si la grabación es muy corta, se analiza entera para no quedarte sin datos).
              </span>
            </p>
          </div>

          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="text-xs font-semibold text-[var(--text-muted)]">
                Ruta del log de MangoHud
              </label>
              <button
                onClick={async () => {
                  const detectedPath = await detectLogPath();
                  if (detectedPath) {
                    handleLogPathChange(detectedPath);
                    setToast('✓ Ruta detectada automáticamente');
                  }
                }}
                className="text-xs text-[var(--action)] hover:text-[var(--action-hover)] flex items-center gap-1 transition-colors"
              >
                <Search className="w-3 h-3" />
                Detectar automáticamente
              </button>
            </div>
            <input
              type="text"
              value={logPath}
              onChange={(e) => handleLogPathChange(e.target.value)}
              placeholder="/home/user/.local/share/goverlay/logs"
              className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm text-[var(--text)] focus:outline-none focus:border-[var(--action)] transition-colors"
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-[var(--text-muted)] mb-1 block">
              Notas (opcional)
            </label>
            <input
              type="text"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Ej: Con game-performance activado"
              className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm text-[var(--text)] focus:outline-none focus:border-[var(--action)] transition-colors"
            />
          </div>

          <button
            onClick={analyzeLog}
            disabled={analyzing}
            className="w-full px-4 py-2.5 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 shadow-lg shadow-action/20 hover:shadow-xl hover:shadow-action/30"
          >
            {analyzing ? (
              <>
                <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                Analizando...
              </>
            ) : (
              <>
                <FileText className="w-4 h-4" />
                Analizar y guardar benchmark
              </>
            )}
          </button>
        </div>

        {/* Guardado automático: grabar con Shift+F2 y no volver a tocar nada */}
        <div className="mt-4 pt-4 border-t border-[var(--border)]">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <p className="text-sm font-semibold text-[var(--text)]">
                Guardar las grabaciones automáticamente
              </p>
              <p className="text-xs text-[var(--text-muted)] mt-1 leading-relaxed">
                Tú grabas como siempre (<strong>Shift + F2</strong> dentro del juego); la app
                vigila <code className="text-[var(--text)]">{logPath}</code> y guarda cada
                grabación en cuanto MangoHud termina de escribirla. No arranca grabaciones
                por su cuenta ni toca tu configuración de MangoHud.
              </p>
            </div>
            <Toggle
              checked={autoRecoger}
              onChange={toggleAutoRecoger}
              label="Guardar las grabaciones automáticamente"
            />
          </div>

          {/* Sin juego abierto, el guardado automático no puede registrar el comando: se
              dice aquí, antes de grabar, en vez de descubrirlo en el historial. */}
          {autoRecoger && !hayJuegoAbierto && (
            <div className="mt-3 p-3 rounded-lg bg-[var(--caution-bg)] border border-caution/40">
              <p className="text-xs text-[var(--caution)] font-semibold">
                No hay ningún juego abierto.
              </p>
              <p className="text-[11px] text-[var(--text-muted)] mt-1 leading-relaxed">
                Puedes grabar y se guardará la prueba, pero quedará sin comando registrado:
                Thoriapp no sabe qué ajustes había puestos. Abre el juego en la Biblioteca
                antes de grabar si quieres que la comparación sepa qué cambió.
              </p>
            </div>
          )}

          {autoRecoger ? (
            <div className="mt-3 flex items-center gap-2 text-xs text-[var(--text-muted)]">
              <span
                className={`w-2 h-2 rounded-full shrink-0 ${
                  checking ? 'bg-[var(--action)] animate-pulse' : 'bg-[var(--compatible)]'
                }`}
                aria-hidden="true"
              />
              <span className="min-w-0">
                {checking ? 'Buscando grabaciones nuevas…' : 'Vigilando la carpeta'}
                {ultimaRecogida && <> · última guardada: <strong className="text-[var(--text)]">{ultimaRecogida}</strong></>}
              </span>
            </div>
          ) : (
            <p className="mt-3 text-[11px] text-[var(--text-dim)] leading-relaxed">
              Desactivado: las grabaciones se guardan solo cuando pulsas el botón de arriba.
            </p>
          )}
        </div>
      </article>

      {/* Subfase 5.1: si el historial no se pudo leer, se dice. Sin esto la lista
          simplemente no aparecía y parecía que no había grabaciones. */}
      {errorHistorial && (
        <article
          role="alert"
          className="p-4 rounded-xl bg-[var(--caution-bg)] border border-caution/50"
        >
          <p className="flex items-center gap-2 text-sm font-semibold text-[var(--caution)] mb-1">
            <AlertTriangle className="w-4 h-4" />
            No se pudo leer el historial de benchmarks
          </p>
          <p className="text-xs text-[var(--text-muted)] leading-relaxed break-words">
            {errorHistorial}
          </p>
        </article>
      )}

      {/* Diagnóstico del arranque: shaders compilándose al principio */}
      {shown.length > 0 && (
        <ShaderStutterCard benchmark={shown[0]} cache={shaderCache} loadingCache={shaderCache === null && !!currentAppid} />
      )}

      {/* Clasificación: con varias grabaciones, decir cuál gana de un vistazo */}

      {/* Configuración histórica de la prueba más reciente (subfase 5.2). Plegada por
          defecto; se despliega para auditar con qué se hizo. */}
      {shown.length > 0 && (
        <InstantaneaCard bench={shown[0]} onCopy={copiarComando} />
      )}

      {/* Lista de Benchmarks */}
      {shown.length > 0 && (
        <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-base font-bold text-[var(--text)] flex items-center gap-2">
              <BarChart3 className="w-5 h-5 text-[var(--action)]" />
              {alcance.tipo === 'juego' ? 'Historial de este juego' : 'Historial de todos los juegos'}
              <span className="text-xs font-normal text-[var(--text-muted)] bg-[var(--bg)] px-2 py-0.5 rounded-full">
                {shown.length}
              </span>
            </h3>
            <button
              onClick={askDeleteAll}
              className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-error/10 text-[var(--error)] border border-error/30 hover:bg-error/20 hover:border-error/50 hover:scale-105 transition-all duration-200 flex items-center gap-1.5 group"
              title={
                alcance.tipo === 'juego'
                  ? `Borrar todo el historial de ${alcance.nombre}`
                  : 'Borrar el historial de todos los juegos'
              }
            >
              <Trash2 className="w-3.5 h-3.5 group-hover:rotate-12 transition-transform" />
              {alcance.tipo === 'juego' ? 'Borrar historial' : 'Borrar todo el historial'}
            </button>
          </div>

          <div className="space-y-3 max-h-[500px] overflow-y-auto pr-1">
            {shown.map((bench, idx) => {
              // idx 0 = la grabación más reciente, y es contra la que se compara.
              const isLatest = idx === 0;
              const score = smoothScore(bench);
              // `duration_seconds` vale 0 cuando no hay reloj fiable: ese cero no es «dura 0 s».
              const isShort = duraMenosDe(bench, SHORT_SECONDS);

              return (
                <div
                  key={bench.id}
                  className={`p-4 rounded-xl bg-[var(--bg)] border-2 transition-all duration-200 ${
                    isLatest
                      ? 'border-[#38bdf8] shadow-lg shadow-[#38bdf8]/10'
                      : 'border-[var(--border)]'
                  }`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0">
                      {/* Identidad de la grabación: «Prueba N», con su comando consultable
                          al pasar el ratón, al llegar con el teclado o al pulsar. El
                          comando no se pinta siempre: ocupa dos líneas y ensucia la lista. */}
                      <div className="mb-3 flex items-center gap-2">
                        <PruebaN
                          bench={bench}
                          onCopy={copiarComando}
                          className="text-sm font-bold text-[var(--text)]"
                        />
                        {isLatest && (
                          <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-bold text-white bg-[#38bdf8]">
                            Más reciente
                          </span>
                        )}
                      </div>

                      {/* Nombre del juego */}
                      <div className="flex items-center gap-2 mb-2">
                        <Gamepad2 className="w-4 h-4 text-[var(--action)]" />
                        <h4 className="text-base font-bold text-[var(--text)] truncate">
                          {bench.game_name || 'Desconocido'}
                        </h4>
                      </div>

                      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-[var(--text-muted)] mb-4">
                        <Clock className="w-3.5 h-3.5" />
                        <span>{formatTimestamp(bench.timestamp)}</span>
                        <span>•</span>
                        <span>Duró {textoDeDuracion(bench, fmtDuration)}</span>
                        {bench.warmup_seconds != null && bench.warmup_seconds > 0 && (
                          <>
                            <span>•</span>
                            <span
                              className="flex items-center gap-1"
                              title="Se descartaron los primeros segundos (carga y compilación inicial de shaders)"
                            >
                              <Clock className="w-3.5 h-3.5" />
                              sin los primeros {Math.round(bench.warmup_seconds)} s
                            </span>
                          </>
                        )}
                        {bench.noise_percent != null && (
                          <span
                            className="font-semibold text-[var(--text-dim)]"
                            title="Cuánto puede moverse el FPS promedio solo por variación natural dentro de esta grabación"
                          >
                            • ruido {marginLabel(bench.noise_percent)}
                          </span>
                        )}
                        {isShort && (
                          <span className="flex items-center gap-1 text-[var(--caution)] font-semibold" title="Con grabaciones cortas el resultado puede variar">
                            <AlertCircle className="w-3.5 h-3.5" />
                            Grabación corta
                          </span>
                        )}
                        {bench.notes && (
                          <>
                            <span>•</span>
                            <span className="italic truncate max-w-[200px]">{bench.notes}</span>
                          </>
                        )}
                      </div>

                      {/* Resumen simple */}
                      <div className="grid grid-cols-3 gap-3">
                        <div className="bg-[var(--surface)] rounded-lg p-3 border border-[var(--border)]" title="FPS promedio">
                          <div className="text-xs text-[var(--text-muted)] mb-1 font-medium">Velocidad</div>
                          <div className="text-2xl font-bold text-[var(--text)]">
                            {bench.avg_fps.toFixed(0)}
                            <span className="text-xs font-medium text-[var(--text-muted)] ml-1">FPS</span>
                          </div>
                        </div>

                        <div
                          className="bg-[var(--surface)] rounded-lg p-3 border border-[var(--border)]"
                          title="Qué tan parejo va el juego. Cuanto más alto, menos tirones."
                        >
                          <div className="text-xs text-[var(--text-muted)] mb-1 font-medium">Fluidez</div>
                          <div className="text-2xl font-bold text-[var(--text)]">{score.toFixed(0)}%</div>
                          <div className="h-1.5 rounded-full bg-[var(--bg)] overflow-hidden mt-1.5">
                            <div className="h-full rounded-full bg-[var(--action)]" style={{ width: `${score}%` }} />
                          </div>
                          <div className="text-[11px] text-[var(--text-muted)] mt-1">{smoothLabel(score)}</div>
                        </div>

                        <div
                          className="bg-[var(--surface)] rounded-lg p-3 border border-[var(--border)]"
                          title="1% Low: tus FPS en el 1% de momentos más lentos"
                        >
                          <div className="text-xs text-[var(--text-muted)] mb-1 font-medium">Peores momentos</div>
                          <div className="text-2xl font-bold text-[var(--text)]">
                            {bench.fps_1_percent_low.toFixed(0)}
                            <span className="text-xs font-medium text-[var(--text-muted)] ml-1">FPS</span>
                          </div>
                        </div>
                      </div>
                    </div>

                    <button
                      onClick={() => askDeleteBenchmark(bench.id)}
                      className="p-2 rounded-lg text-[var(--error)] hover:bg-error/10 hover:scale-110 transition-all duration-200"
                      title="Eliminar benchmark"
                      aria-label="Eliminar benchmark"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>

          <p className="text-xs text-[var(--text-dim)] mt-3">
            Velocidad = FPS promedio. Fluidez = qué tan parejo va el juego (más alto, menos tirones). Peores momentos = tus FPS en el 1% más lento.
          </p>

          {shown.length === 1 && (
            <div className="mt-4 p-3 rounded-lg bg-[#38bdf8]/10 border border-[#38bdf8]/30">
              <p className="text-xs text-[var(--text)]">
                <strong className="text-[#38bdf8]">Punto de partida guardado.</strong> Cambia una sola cosa en el juego, graba otra vez
                y pulsa <strong className="text-[var(--action)]">Analizar y guardar</strong>: la nueva grabación se guardará
                en el historial y se comparará sola con esta.
              </p>
            </div>
          )}
        </article>
      )}

      {/* ¿La diferencia es real? Necesita varias grabaciones por lado. */}
      {groupRows && groupRows.some((r) => r.antes_runs + r.despues_runs > 0) && (
        <GroupComparisonCard rows={groupRows} />
      )}

      {/* Resultado de la comparación */}
      {comparison && (
        <ComparisonResultCard
          comparison={comparison}
          groupRows={groupRows}
          onClose={clearComparison}
        >
          <button
            type="button"
            onClick={saveCurrentComparison}
            disabled={isCurrentSaved || savingComparison}
            className={`w-full mt-4 px-4 py-3 rounded-lg text-sm font-semibold transition-all duration-200 flex items-center justify-center gap-2 ${
              isCurrentSaved
                ? 'bg-compatible/10 text-[var(--compatible)] border border-compatible/40 cursor-default'
                : 'bg-[var(--action)] text-white hover:bg-[var(--action-hover)] shadow-lg shadow-action/20 disabled:opacity-50 disabled:cursor-not-allowed'
            }`}
          >
            {isCurrentSaved ? (
              <>
                <Check className="w-4 h-4" />
                Guardada en Mis pruebas
              </>
            ) : (
              <>
                <Save className="w-4 h-4" />
                {savingComparison ? 'Guardando…' : 'Guardar en Mis pruebas'}
              </>
            )}
          </button>
        </ComparisonResultCard>
      )}

      {/* Guía rápida */}
      <article className="p-6 rounded-xl bg-gradient-to-br from-[var(--surface)] to-[var(--bg)] border-2 border-[var(--border)]">
        <div className="flex items-center gap-3 mb-5">
          <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-[var(--action)] to-[var(--caution)] flex items-center justify-center">
            <Zap className="w-5 h-5 text-white" />
          </div>
          <div>
            <h3 className="text-lg font-bold text-[var(--text)]">Guía rápida de benchmarks</h3>
            <p className="text-xs text-[var(--text-muted)]">Mide si un cambio mejora tu juego, en 3 pasos</p>
          </div>
        </div>

        {/* Explicación del historial */}
        <div className="mb-6 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
          <h4 className="text-sm font-bold text-[var(--text)] mb-1">¿Cómo funciona el historial?</h4>
          <p className="text-xs text-[var(--text-muted)] mb-3">
            No tienes que elegir nada: cada grabación se guarda sola y se compara con la anterior.
          </p>
          <div className="grid grid-cols-2 gap-3">
            <div className="p-3 rounded-lg bg-[var(--surface)] border-2 border-[#38bdf8]/30">
              <div className="flex items-center gap-2 mb-2">
                <span className="w-5 h-5 rounded-full bg-[#38bdf8] text-[10px] font-bold text-white flex items-center justify-center">1</span>
                <span className="text-sm font-bold text-[#38bdf8]">La más reciente</span>
              </div>
              <p className="text-xs text-[var(--text-muted)]">
                La última que grabaste. Se compara sola con la de antes, así ves al momento si el cambio sirvió.
              </p>
            </div>
            <div className="p-3 rounded-lg bg-[var(--surface)] border-2 border-[var(--border)]">
              <div className="flex items-center gap-2 mb-2">
                <span className="w-5 h-5 rounded-full bg-[var(--text-muted)] text-[10px] font-bold text-white flex items-center justify-center">10</span>
                <span className="text-sm font-bold text-[var(--text)]">Las 10 últimas</span>
              </div>
              <p className="text-xs text-[var(--text-muted)]">
                Guardamos las <strong className="text-[var(--text)]">10 últimas</strong> de cada juego. La número 11 borra la más antigua: no tienes que limpiar nada.
              </p>
            </div>
          </div>
        </div>

        {/* Pasos */}
        <div className="space-y-4">
          <div className="flex gap-4 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-action/50 transition-colors">
            <div className="w-10 h-10 rounded-full bg-gradient-to-br from-[var(--action)] to-[var(--action-hover)] flex items-center justify-center text-sm font-bold text-white shrink-0 shadow-lg shadow-action/20">
              1
            </div>
            <div className="flex-1">
              <p className="font-bold text-[var(--text)] mb-1">Graba cómo va tu juego, sin cambios</p>
              <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                Arriba, elige cuánto debe durar la grabación (lo ideal es 1 minuto). En el juego pulsa <kbd className="px-1.5 py-0.5 rounded bg-[var(--surface)] border border-[var(--border)] text-[var(--text)] font-mono text-[10px]">Shift + F2</kbd> y
                juega siempre por la misma zona. Cuando termine, pulsa <strong className="text-[var(--action)]">Analizar y guardar</strong>.
                Esa grabación queda guardada sola como <strong className="text-[#38bdf8]">punto de partida</strong>.
              </p>
            </div>
          </div>

          <div className="flex gap-4 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-action/50 transition-colors">
            <div className="w-10 h-10 rounded-full bg-gradient-to-br from-[var(--action)] to-[var(--action-hover)] flex items-center justify-center text-sm font-bold text-white shrink-0 shadow-lg shadow-action/20">
              2
            </div>
            <div className="flex-1">
              <p className="font-bold text-[var(--text)] mb-1">Cambia una sola cosa y graba otra vez</p>
              <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                Activa un parámetro (por ejemplo <code className="px-1.5 py-0.5 rounded bg-[var(--surface)] border border-[var(--border)] text-[var(--text)] font-mono text-[10px]">game-performance</code>)
                y repite la grabación en el mismo sitio y con la misma duración. Pulsa <strong className="text-[var(--action)]">Analizar y guardar</strong>:
                queda guardada en el historial de este juego.
              </p>
            </div>
          </div>

          <div className="flex gap-4 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-action/50 transition-colors">
            <div className="w-10 h-10 rounded-full bg-gradient-to-br from-[var(--action)] to-[var(--action-hover)] flex items-center justify-center text-sm font-bold text-white shrink-0 shadow-lg shadow-action/20">
              3
            </div>
            <div className="flex-1">
              <p className="font-bold text-[var(--text)] mb-1">Mira el resultado</p>
              <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                La comparación aparece sola, sin pulsar nada más. Te diremos quién gana y qué hacer.
              </p>
              <div className="mt-2 p-2 rounded bg-caution/10 border border-caution/30">
                <p className="text-[11px] text-[var(--caution)] font-semibold">
                  💡 Fíjate primero en la Fluidez: un juego con más FPS pero con tirones se siente peor.
                </p>
              </div>
            </div>
          </div>
        </div>

        {/* Qué pasa si grabo más veces */}
        <div className="mt-4 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
          <h4 className="text-sm font-bold text-[var(--text)] mb-2">¿Y si grabo muchas veces?</h4>
          <ul className="space-y-1.5 text-xs text-[var(--text-muted)] leading-relaxed list-disc pl-4">
            <li>
              Cada grabación se guarda sola. Se comparan siempre las <strong className="text-[var(--text)]">dos últimas</strong>, así que
              no necesitas anotar nada a mano.
            </li>
            <li>
              De cada juego se conservan las <strong className="text-[var(--text)]">10 grabaciones más recientes</strong>. Al grabar la
              número 11 se borra la más antigua de ese juego, sin tocar los demás.
            </li>
            <li>
              Si quieres empezar una serie limpia, pulsa <strong className="text-[var(--text)]">Borrar historial</strong>. Los otros juegos no se tocan.
            </li>
            <li>
              También puedes comparar dos grabaciones cualesquiera de <strong className="text-[var(--text)]">Mis pruebas</strong>, con cualquier configuración y fecha.
            </li>
          </ul>
        </div>

        {/* Ejemplo */}
        <div className="mt-6 p-4 rounded-lg bg-gradient-to-r from-[#38bdf8]/10 to-caution/10 border border-[var(--border)]">
          <h4 className="text-sm font-bold text-[var(--text)] mb-3">Ejemplo práctico</h4>
          <div className="space-y-2 text-xs text-[var(--text-muted)]">
            <div className="flex items-center gap-2">
              <span className="text-[var(--compatible)] font-bold">✓</span>
              <span>Grabación 1: Palworld sin optimizar → 60 FPS → queda como <strong className="text-[#38bdf8]">punto de partida</strong></span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[var(--compatible)] font-bold">✓</span>
              <span>Activa <code className="px-1 py-0.5 rounded bg-[var(--surface)] border border-[var(--border)] font-mono text-[10px]">game-performance</code></span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[var(--compatible)] font-bold">✓</span>
              <span>Grabación 2: Palworld con game-performance → 70 FPS → se guarda en el historial</span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[var(--compatible)] font-bold">✓</span>
              <span>La comparación aparece sola → <strong className="text-[var(--compatible)]">+16.7% de FPS</strong> y, si la Fluidez se mantiene, gana la nueva grabación 🎉</span>
            </div>
          </div>
        </div>
      </article>

      {/* Cuadro de confirmación (estilo de la app, no el nativo) */}
      {confirmState && <ConfirmDialog state={confirmState} onClose={close} />}
    </div>
  );
}
