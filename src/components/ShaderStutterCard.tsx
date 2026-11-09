import { AlertCircle, Check, HardDrive, Loader2 } from 'lucide-react';
import type { Benchmark, ShaderCache } from '../lib/types';
import { cacheSizeLabel, shaderVerdict, SIGNIFICANT_PENALTY, slowFramesRatio } from '../lib/shaders';
import type { ShaderVerdictKind } from '../lib/shaders';

const STYLES: Record<ShaderVerdictKind, { box: string; text: string; Icon: typeof Check }> = {
  clean: {
    box: 'bg-[var(--compatible-bg)] border-[var(--compatible)]/30',
    text: 'text-[var(--compatible)]',
    Icon: Check,
  },
  unclear: {
    box: 'bg-[var(--card)] border-[var(--border)]',
    text: 'text-[var(--text-muted)]',
    Icon: AlertCircle,
  },
  shaders: {
    box: 'bg-[var(--caution-bg)] border-[var(--caution)]/40',
    text: 'text-[var(--caution)]',
    Icon: AlertCircle,
  },
  'no-data': {
    box: 'bg-[var(--card)] border-[var(--border)]',
    text: 'text-[var(--text-dim)]',
    Icon: AlertCircle,
  },
};

interface ShaderStutterCardProps {
  /** La grabación más reciente: es la que se diagnostica. */
  benchmark: Benchmark | null;
  cache: ShaderCache | null;
  loadingCache?: boolean;
}

/**
 * "¿Por qué tartamudea al empezar?" — arranque vs resto de la grabación, más el estado
 * de la caché de shaders en disco.
 *
 * Va en Benchmarks y no en una pestaña aparte a propósito: el diagnóstico sale de los
 * mismos datos que ya estás mirando, y separarlo obligaba a saltar de pantalla para
 * entender una cifra.
 */
export function ShaderStutterCard({ benchmark, cache, loadingCache }: ShaderStutterCardProps) {
  if (!benchmark) return null;

  const stutter = benchmark.shader_stutter;
  const verdict = shaderVerdict(stutter, cache);
  const style = STYLES[verdict.kind];
  const { Icon } = style;
  const ratio = stutter ? slowFramesRatio(stutter) : null;

  return (
    <article className={`p-5 rounded-xl border ${style.box}`}>
      <div className="flex items-start gap-3">
        <Icon className={`w-5 h-5 shrink-0 mt-0.5 ${style.text}`} />
        <div className="flex-1 min-w-0">
          <h3 className="text-base font-bold text-[var(--text)]">¿Tartamudea al empezar?</h3>
          <p className={`text-sm font-semibold mt-1 ${style.text}`}>{verdict.title}</p>
          <p className="text-xs text-[var(--text-muted)] mt-1">{verdict.detail}</p>

          {/* Arranque vs resto, con los números a la vista */}
          {stutter && verdict.kind !== 'no-data' && (
            <div className="grid grid-cols-2 gap-3 mt-3">
              <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
                <p className="text-[11px] text-[var(--text-muted)] mb-1">
                  Primeros {Math.round(stutter.window_seconds)} s
                </p>
                <p className="text-lg font-bold text-[var(--text)]">
                  {stutter.early_fps_1_percent_low.toFixed(0)}
                  <span className="text-xs font-medium text-[var(--text-muted)] ml-1">FPS peor 1 %</span>
                </p>
                <p className="text-[11px] text-[var(--text-muted)] mt-1">
                  {stutter.early_pct_over_33ms.toFixed(1)} % de frames lentos
                </p>
              </div>
              <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
                <p className="text-[11px] text-[var(--text-muted)] mb-1">Resto de la grabación</p>
                <p className="text-lg font-bold text-[var(--text)]">
                  {stutter.steady_fps_1_percent_low.toFixed(0)}
                  <span className="text-xs font-medium text-[var(--text-muted)] ml-1">FPS peor 1 %</span>
                </p>
                <p className="text-[11px] text-[var(--text-muted)] mt-1">
                  {stutter.steady_pct_over_33ms.toFixed(1)} % de frames lentos
                  {ratio !== null && ratio >= 2 && ` · ${ratio.toFixed(1)}× más al principio`}
                </p>
              </div>
            </div>
          )}

          <p className="text-xs text-[var(--text)] mt-3">{verdict.advice}</p>

          {/* Estado de la caché en disco */}
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-[var(--text-muted)] mt-3 pt-3 border-t border-[var(--border)]">
            <HardDrive className="w-3.5 h-3.5" />
            {loadingCache ? (
              <span className="flex items-center gap-1">
                <Loader2 className="w-3 h-3 animate-spin" /> Buscando la caché de shaders…
              </span>
            ) : cache ? (
              <>
                <span>
                  Caché en disco: <strong className="text-[var(--text)]">{cacheSizeLabel(cache.bytes)}</strong>
                </span>
                <span>·</span>
                <span>
                  {cache.steam_precached
                    ? 'Steam ya la tiene preparada'
                    : 'Steam no la tiene preparada'}
                </span>
              </>
            ) : (
              <span>No se pudo leer la caché de shaders (el diagnóstico de arriba sigue valiendo).</span>
            )}
          </div>

          <p className="text-[10px] text-[var(--text-dim)] mt-2">
            Se compara el peor 1 % de los primeros segundos con el del resto, porque compilar shaders no baja el
            FPS medio: provoca tirones sueltos. El aviso salta a partir del {SIGNIFICANT_PENALTY} %.
          </p>
        </div>
      </div>
    </article>
  );
}
