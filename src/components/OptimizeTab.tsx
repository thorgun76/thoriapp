import { useState } from 'react';
import { AlertTriangle, Check, Cpu, ExternalLink, Search, Terminal, Users, X, Zap } from 'lucide-react';
import { openExternal } from '../lib/links';
import { normalizeForSearch } from '../lib/format';
import { filterParams, impactsPresent } from '../lib/gameView';
import { SectionNav } from './SectionNav';
import type { CommunityRecommendation, GameEngine, GameInfo, GameRules, Param } from '../lib/types';

interface OptimizeTabProps {
  rules: GameRules;
  info: GameInfo;
  engine: GameEngine | null;
  engineRecommendations: string[];
  communityRecommendations: CommunityRecommendation[];
  enabled: boolean[];
  selectedCommunityRecs: Set<number>;
  selectedEngineRecs: Set<number>;
  expertMode: boolean;
  /** Cambia entre modo guiado y experto. Vive en App: el sidebar usa el mismo estado. */
  onToggleExpertMode: () => void;
  forceProton: boolean;
  showUnavailable: boolean;
  onToggleRule: (index: number) => void;
  onToggleCommunityRec: (index: number) => void;
  onToggleEngineRec: (index: number) => void;
  onToggleProton: () => void;
  missing: (p: Param) => string | null;
  getRecommendationStatus: (p: Param) => string;
  impactColor: (impact: string) => string;
  evidenceLabel: (evidence: string) => string;
}

/** Pestaña "Optimización": parámetros, motor, hardware y recomendaciones por juego. */
export function OptimizeTab({
  rules,
  info,
  engine,
  engineRecommendations,
  communityRecommendations,
  enabled,
  selectedCommunityRecs,
  selectedEngineRecs,
  expertMode,
  onToggleExpertMode,
  forceProton,
  showUnavailable,
  onToggleRule,
  onToggleCommunityRec,
  onToggleEngineRec,
  onToggleProton,
  missing,
  getRecommendationStatus,
  impactColor,
  evidenceLabel,
}: OptimizeTabProps) {
  // Filtro local de la pestaña: en modo experto la lista crece hasta decenas de
  // ajustes y buscar «FSR» a mano es tedioso.
  const [query, setQuery] = useState('');
  const [impact, setImpact] = useState<string | null>(null);

  const filtering = query.trim().length > 0 || impact !== null;

  /** Impacto de un valor, si ese valor es un parámetro del juego. */
  const impactOf = (value: string): string | null =>
    rules.params.find((p) => p.value === value)?.impact ?? null;

  /**
   * Ajustes que la pestaña puede llegar a enseñar con el modo actual.
   *
   * Es el MISMO criterio que aplican las listas (`showUnavailable`, `essential`
   * en modo guiado, `evitar` fuera, `config` incluido). Sin esto, la etiqueta
   * «bajo» existía siempre (sus 7 parámetros son `essential: false`) pero al
   * pulsarla no quedaba nada: un chip que promete algo que no puede dar.
   */
  const reachableParams = rules.params
    .filter((p) => showUnavailable || !missing(p))
    .filter((p) => p.impact !== 'evitar')
    .filter((p) => expertMode || p.essential === true);

  // Los niveles salen de lo que de verdad se puede ver, más los deducibles de las
  // recomendaciones de motor/hardware (si no, esas filas quedarían inalcanzables).
  const impactOptions = impactsPresent(reachableParams, [
    ...engineRecommendations.map(impactOf),
    ...communityRecommendations.filter((r) => expertMode || r.recommended === true).map((r) => impactOf(r.parameter)),
  ]);

  const matches = (p: Param) => filterParams([p], query, impact, normalizeForSearch).length > 0;

  /** Parámetros que pasan el filtro de la barra (sin los filtros propios de cada lista). */
  const visibleParams = filterParams(rules.params, query, impact, normalizeForSearch);

  /**
   * Texto que se busca contra filas que no son parámetros (proton, diagnóstico,
   * motor, hardware). El filtro de impacto no aplica: esas filas no tienen uno.
   */
  const textQuery = normalizeForSearch(query.trim());
  const textMatches = (...fields: (string | null | undefined)[]) =>
    textQuery.length === 0 ||
    fields.some((field) => field && normalizeForSearch(field).includes(textQuery));

  /**
   * Qué se ve con el filtro puesto.
   *
   * Con una búsqueda activa desaparece todo lo que no coincide, no solo las
   * listas largas: si solo se filtraran los parámetros, el usuario escribía y la
   * pantalla seguía igual (había que bajar a mirar si algo había cambiado).
   */
  /**
   * Filas que no son parámetros de `rules.params` y que, por tanto, no tienen
   * `impact` propio: Probar con Proton y las recomendaciones de motor/hardware.
   * Con una etiqueta de impacto puesta no pueden coincidir, así que se ocultan
   * (antes solo miraban el texto y se quedaban siempre).
   *
   * MangoHud NO está en este grupo: es un parámetro más de `rules.params` con su
   * propio impacto (`medicion` en games.json), así que se compara como los demás.
   */
  const impactFilterOn = impact !== null;
  const nonParamRowsVisible = !impactFilterOn;

  const showProton =
    info.native === true && nonParamRowsVisible && textMatches('probar con proton', 'proton');

  const impactAllows = (value: string) => (impactFilterOn ? impactOf(value) === impact : true);

  // MangoHud se busca por texto («mangohud», «fps»…) y por su impacto real. Si el
  // juego no lo trae entre sus parámetros, `impactOf` es `null` y con una etiqueta
  // puesta queda oculto: no se puede afirmar que coincida.
  const mangoHudParam = rules.params.find((p) => p.value === 'mangohud');
  const showMangoHud =
    !!mangoHudParam &&
    !missing(mangoHudParam) &&
    textMatches('mangohud', 'diagnóstico', 'herramientas de diagnóstico', 'fps') &&
    impactAllows('mangohud');

  const showEngineRecs = engineRecommendations
    .map((rec, i) => ({ rec, i }))
    .filter(({ rec }) => textMatches(rec, 'motor', engine?.name))
    .filter(({ rec }) => impactAllows(rec));
  const showCommunityRecs = communityRecommendations
    .map((rec, i) => ({ rec, i }))
    // Mismo criterio que la lista: fuera mangohud/gamemoderun (ya salen arriba) y,
    // en modo guiado, solo las marcadas como recomendadas.
    .filter(({ rec }) => rec.parameter !== 'mangohud' && rec.parameter !== 'gamemoderun')
    .filter(({ rec }) => expertMode || rec.recommended === true)
    .filter(({ rec }) => textMatches(rec.parameter, rec.description, 'hardware'))
    .filter(({ rec }) => impactAllows(rec.parameter));
  const showSystemAdjustments = rules.params.filter(
    (p) => (p.value === 'game-performance' || p.value === 'gamemoderun') && matches(p)
  );
  const showGameParams = visibleParams
    .filter((p) => p.impact !== 'evitar')
    .filter((p) => p.value !== 'mangohud' && p.value !== 'gamemoderun' && p.value !== 'game-performance')
    .filter((p) => !p.requires?.includes('game-performance'))
    .filter((p) => expertMode || p.essential === true);

  /** Nada que enseñar con el filtro puesto: se dice en vez de dejar la pestaña vacía. */
  const nothingMatches =
    filtering &&
    showSystemAdjustments.length === 0 &&
    showGameParams.length === 0 &&
    showEngineRecs.length === 0 &&
    showCommunityRecs.length === 0 &&
    !showMangoHud &&
    !showProton;

  const sections = [
    ...(info.native === true ? [{ id: 'sec-proton', label: 'Proton' }] : []),
    { id: 'sec-diagnostico', label: 'Diagnóstico' },
    { id: 'sec-ajustes', label: 'Ajustes' },
  ];

  return (
    <div className="space-y-4">
      {/* Con un filtro activo las secciones se ocultan, así que el índice sobra. */}
      {!filtering && <SectionNav sections={sections} />}

      {/* FILTRO DE AJUSTES */}
      <div className="p-3 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--text-dim)]" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar un ajuste (FSR, mangohud, sincronización…)"
            aria-label="Buscar ajuste"
            className="w-full pl-9 pr-9 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] focus:border-action/50 focus:outline-none text-sm"
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              aria-label="Limpiar la búsqueda"
              className="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded text-[var(--text-dim)] hover:text-[var(--text)]"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* Filtro por impacto: solo los niveles que existen en este juego. */}
        <div className="flex flex-wrap items-center gap-1.5 mt-2">
          <span className="text-[10px] uppercase tracking-wider text-[var(--text-dim)] mr-1">
            Impacto
          </span>
          {impactOptions.map((level) => (
            <button
              key={level}
              onClick={() => setImpact(impact === level ? null : level)}
              aria-pressed={impact === level}
              className={`text-[10px] px-2 py-0.5 rounded border font-semibold uppercase transition-colors ${
                impact === level
                  ? impactColor(level)
                  : 'bg-[var(--bg)] text-[var(--text-dim)] border-[var(--border)] hover:text-[var(--text)]'
              }`}
            >
              {level}
            </button>
          ))}
          {filtering && (
            <button
              onClick={() => { setQuery(''); setImpact(null); }}
              className="ml-auto text-[10px] text-[var(--action)] hover:underline font-semibold"
            >
              Quitar filtros
            </button>
          )}
        </div>
        {/* Con el filtro puesto solo queda lo que coincide; si no queda nada, se
            dice aquí en vez de enseñar una pestaña vacía. */}
        {nothingMatches && (
          <p className="text-xs text-[var(--text-muted)] mt-2">
            Ningún ajuste coincide con «{query.trim()}». Quita los filtros para ver todos.
          </p>
        )}
      </div>

                        {!filtering && (
                        <div className={`p-4 rounded-xl border ${
                          expertMode
                            ? 'bg-action/5 border-action/20'
                            : 'bg-[var(--compatible-bg)] border-compatible/20'
                        }`}>
                          <div className="flex items-center justify-between gap-4">
                            <div className="min-w-0">
                              <p className={`text-sm font-semibold ${
                                expertMode ? 'text-[var(--action)]' : 'text-[var(--compatible)]'
                              }`}>
                                {expertMode ? 'Modo experto' : 'Modo guiado'}
                              </p>
                              <p className="text-xs text-[var(--text-muted)] mt-0.5">
                                {expertMode
                                  ? 'Ves todos los parámetros con su detalle técnico.'
                                  : 'Ves solo los ajustes que de verdad importan.'}
                              </p>
                            </div>
                            <button
                              onClick={onToggleExpertMode}
                              className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
                                expertMode ? 'bg-[var(--action)]' : 'bg-[var(--border)]'
                              }`}
                              role="switch"
                              aria-checked={expertMode}
                              aria-label="Activar modo experto"
                            >
                              <span
                                className={`inline-block h-4 w-4 transform rounded-full bg-white shadow-lg transition-transform duration-200 ease-in-out ${
                                  expertMode ? 'translate-x-6' : 'translate-x-1'
                                }`}
                              />
                            </button>
                          </div>
                          <p className="text-[10px] text-[var(--text-dim)] mt-2">
                            También puedes cambiarlo desde el menú lateral.
                          </p>
                        </div>
                        )}

                        {/* Fase 3: Toggle Proton estilo Discord */}
                        {showProton && (
                          <article id="sec-proton" className="scroll-mt-4 p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                            <div className="flex items-center justify-between">
                              <div className="flex-1">
                                <h3 className="text-base font-bold text-[var(--text)]">Probar con Proton</h3>
                                <p className="text-sm text-[var(--text-muted)] mt-1">
                                  Algunos juegos nativos funcionan mejor con Proton. Actívalo para ver parámetros de compatibilidad.
                                </p>
                              </div>
                              <button
                                onClick={() => onToggleProton()}
                                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
                                  forceProton ? 'bg-[var(--action)]' : 'bg-[var(--border)]'
                                }`}
                                role="switch"
                                aria-checked={forceProton}
                              >
                                <span
                                  className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 ease-in-out ${
                                    forceProton ? 'translate-x-6' : 'translate-x-1'
                                  }`}
                                />
                              </button>
                            </div>
                          </article>
                        )}

                        {/* Herramientas de diagnóstico */}
                        {showMangoHud && (
                        <article id="sec-diagnostico" className="scroll-mt-4 p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <div className="flex items-start justify-between mb-4">
                            <div>
                              <h3 className="text-base font-bold text-[var(--text)]">Herramientas de diagnóstico</h3>
                              <p className="text-sm text-[var(--text-muted)] mt-1">
                                Estas herramientas miden el rendimiento, no lo optimizan. Úsalas para verificar si los ajustes funcionan.
                              </p>
                            </div>
                          </div>

                          <div className="grid grid-cols-1 gap-2">
                            {/* MangoHud */}
                            <button
                              onClick={() => {
                                const idx = rules.params.findIndex(p => p.value === 'mangohud');
                                if (idx !== -1) onToggleRule(idx);
                              }}
                              className={`text-left p-3 rounded-lg border transition-all ${
                                enabled[rules.params.findIndex(p => p.value === 'mangohud')]
                                  ? 'bg-[var(--compatible-bg)] border-compatible/50'
                                  : 'bg-[var(--bg)] border-[var(--border)] hover:border-compatible/40'
                              }`}
                            >
                              <div className="flex items-center gap-2 mb-1">
                                <code className="text-xs font-mono font-bold text-[var(--compatible)]">mangohud</code>
                                {enabled[rules.params.findIndex(p => p.value === 'mangohud')] && <Check className="w-3 h-3 text-[var(--compatible)]" />}
                              </div>
                              <p className="text-xs text-[var(--text-muted)]">Overlay de FPS, frametime, CPU/GPU. Shift+F2 para grabar logs.</p>
                            </button>
                          </div>
                        </article>
                        )}

                        {/* Ajustes para probar */}
                        <article id="sec-ajustes" className="scroll-mt-4 p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <div className="flex items-start justify-between mb-4">
                            <div>
                              <h3 className="text-base font-bold text-[var(--text)]">Ajustes para probar</h3>
                              <p className="text-sm text-[var(--text-muted)] mt-1">
                                Parámetros de optimización reales. Actívalos individualmente y mide el resultado.
                              </p>
                            </div>
                          </div>

                          {/* Ajustes del sistema */}
                          {(() => {
                            const systemAdjustments = showSystemAdjustments
                              .filter(p => showUnavailable || !missing(p));

                            if (systemAdjustments.length === 0) return null;

                            return (
                              <div className="mb-6">
                                <h4 className="text-sm font-semibold text-[var(--caution)] mb-3 flex items-center gap-2">
                                  <Cpu className="w-4 h-4" />
                                  Ajustes del sistema
                                </h4>
                                <div className={`grid gap-2 ${systemAdjustments.length === 1 ? 'grid-cols-1' : 'grid-cols-1 md:grid-cols-2'}`}>
                                  {systemAdjustments.map((p) => {
                                    const idx = rules.params.indexOf(p);
                                    const isMissing = missing(p);
                                    return (
                                      <button
                                        key={idx}
                                        disabled={!!isMissing}
                                        onClick={() => onToggleRule(idx)}
                                        className={`text-left p-3 rounded-lg border transition-all ${
                                          isMissing
                                            ? 'opacity-50 bg-[var(--bg)] border-[var(--border)] cursor-not-allowed'
                                            : enabled[idx]
                                              ? 'bg-[var(--caution-bg)] border-caution/50'
                                              : 'bg-[var(--bg)] border-[var(--border)] hover:border-caution/40'
                                        }`}
                                      >
                                        <div className="flex items-center gap-2 mb-1">
                                          <code className="text-xs font-mono font-bold text-[var(--caution)]">{p.value}</code>
                                          {enabled[idx] && <Check className="w-3 h-3 text-[var(--caution)]" />}
                                          <span className={`text-[10px] px-1.5 py-0.5 rounded border ${impactColor(p.impact)}`}>
                                            {p.impact.toUpperCase()}
                                          </span>
                                        </div>
                                        <p className="text-xs text-[var(--text-muted)]">{p.why}</p>
                                        {isMissing ? (
                                          <p className="text-[10px] text-[var(--error)] mt-1">🚫 {isMissing}</p>
                                        ) : (
                                          p.requires &&
                                          p.requires.length > 0 && (
                                            <p className="text-[10px] text-[var(--text-dim)] mt-1">
                                              Requisito: {p.requires.join(', ')}
                                            </p>
                                          )
                                        )}
                                      </button>
                                    );
                                  })}
                                </div>
                              </div>
                            );
                          })()}

                          {/* Engine Recommendations */}
                          {engineRecommendations.length > 0 && (
                            <div className="mb-6">
                              <h4 className="text-sm font-semibold text-[var(--action)] mb-3 flex items-center gap-2">
                                <Terminal className="w-4 h-4" />
                                Parámetros del motor ({engine?.name})
                              </h4>
                              <div className={`grid gap-2 ${engineRecommendations.length === 1 ? 'grid-cols-1' : 'grid-cols-1 md:grid-cols-2'}`}>
                                {showEngineRecs.map(({ rec, i }) => (
                                  <button
                                    key={i}
                                    onClick={() => onToggleEngineRec(i)}
                                    className={`text-left p-3 rounded-lg border transition-all ${
                                      selectedEngineRecs.has(i)
                                        ? 'bg-action/20 border-action/50'
                                        : 'bg-[var(--bg)] border-[var(--border)] hover:border-action/40'
                                    }`}
                                  >
                                    <div className="flex items-center gap-2 mb-1">
                                      <code className="text-xs font-mono font-bold text-[var(--action)]">{rec}</code>
                                      {selectedEngineRecs.has(i) && <Check className="w-3 h-3 text-[var(--action)]" />}
                                    </div>
                                    <p className="text-xs text-[var(--text-muted)]">
                                      {rec.includes('NTSYNC') ? 'Sincronización mejorada de hilos' :
                                       rec.includes('LARGE_ADDRESS') ? 'Permite usar más de 2GB de RAM' :
                                       'Parámetro específico del motor'}
                                    </p>
                                  </button>
                                ))}
                              </div>
                            </div>
                          )}

                          {/* Hardware Recommendations */}
                          {(() => {
                            if (showCommunityRecs.length === 0) return null;

                            return (
                              <div className="mb-6">
                                <h4 className="text-sm font-semibold text-[var(--compatible)] mb-3 flex items-center gap-2">
                                  <Users className="w-4 h-4" />
                                  Recomendaciones por hardware
                                </h4>
                                <div className={`grid gap-2 ${showCommunityRecs.length === 1 ? 'grid-cols-1' : 'grid-cols-1 md:grid-cols-2'}`}>
                                  {showCommunityRecs.map(({ rec, i }) => (
                                    <button
                                      key={i}
                                      onClick={() => onToggleCommunityRec(i)}
                                      className={`text-left p-3 rounded-lg border transition-all ${
                                        selectedCommunityRecs.has(i)
                                          ? 'bg-[var(--compatible-bg)] border-compatible/50'
                                          : 'bg-[var(--bg)] border-[var(--border)] hover:border-compatible/40'
                                      }`}
                                    >
                                      <div className="flex items-center gap-2 mb-1">
                                        <code className="text-xs font-mono font-bold text-[var(--compatible)]">{rec.parameter}</code>
                                        {selectedCommunityRecs.has(i) && <Check className="w-3 h-3 text-[var(--compatible)]" />}
                                      </div>
                                      <p className="text-xs text-[var(--text-muted)]">{rec.description}</p>
                                    </button>
                                  ))}
                                </div>
                              </div>
                            );
                          })()}

                          {/* Game-specific Recommendations */}
                          {rules.has_rules && (
                            <div>
                              <div className="flex items-center justify-between mb-3">
                                <h4 className="text-sm font-semibold text-[var(--caution)] flex items-center gap-2">
                                  <Zap className="w-4 h-4" />
                                  Parámetros específicos del juego
                                </h4>
                                {expertMode && (
                                  <span className="text-[10px] px-2 py-0.5 rounded bg-action/10 text-[var(--action)] border border-action/30">
                                    Modo experto
                                  </span>
                                )}
                              </div>
                              <div className="space-y-2">
                                {/* Con filtro activo y ningún resultado se dice, en vez
                                    de dejar un hueco que parece un fallo. */}
                                {showGameParams
                                  .filter(p => showUnavailable || !missing(p))
                                  .map((p, i) => {
                                    const idx = rules.params.indexOf(p);
                                    const isMissing = missing(p);
                                    // Un `config` es un ajuste DENTRO del juego ("usa FSR en
                                    // Calidad"), no un parámetro del lanzador: si se pudiera
                                    // activar, acabaría pegado al comando y lo rompería. Se
                                    // muestra como consejo, sin interruptor.
                                    const esAjusteInterno = p.kind === 'config';
                                    return (
                                      <div
                                        key={i}
                                        role={esAjusteInterno || isMissing ? undefined : 'checkbox'}
                                        aria-checked={esAjusteInterno || isMissing ? undefined : !!enabled[idx]}
                                        aria-disabled={esAjusteInterno || isMissing ? undefined : false}
                                        tabIndex={esAjusteInterno || isMissing ? undefined : 0}
                                        onClick={() => {
                                          if (!isMissing && !esAjusteInterno) onToggleRule(idx);
                                        }}
                                        onKeyDown={(e) => {
                                          if (isMissing || esAjusteInterno) return;
                                          if (e.key === ' ' || e.key === 'Enter') {
                                            e.preventDefault();
                                            onToggleRule(idx);
                                          }
                                        }}
                                        className={`w-full text-left p-4 rounded-lg border transition-all ${
                                          esAjusteInterno
                                            ? 'bg-[var(--bg)] border-dashed border-[var(--border)] cursor-default'
                                            : isMissing
                                              ? 'opacity-50 bg-[var(--bg)] border-[var(--border)] cursor-not-allowed'
                                              : enabled[idx]
                                                ? 'bg-[var(--caution-bg)] border-caution/50'
                                                : 'bg-[var(--bg)] border-[var(--border)] hover:border-caution/40'
                                        }`}
                                      >
                                        <div className="flex items-start justify-between gap-3">
                                          <div className="flex-1">
                                            <div className="flex items-center gap-2 mb-2 flex-wrap">
                                              <h5 className="text-sm font-semibold text-[var(--text)]">{p.title}</h5>
                                              <span className={`text-[10px] px-1.5 py-0.5 rounded border ${impactColor(p.impact)}`}>
                                                {p.impact.toUpperCase()}
                                              </span>
                                              {/* Qué acaba en el comando y qué no. Sin esta distinción
                                                  el usuario no sabe qué está tocando al activar la tarjeta. */}
                                              {esAjusteInterno ? (
                                                <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">
                                                  🎮 En el juego
                                                </span>
                                              ) : (
                                                <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--bg)] text-[var(--text-dim)] border border-[var(--border)] font-mono">
                                                  {p.kind === 'wrapper' ? 'envoltorio' : p.kind === 'arg' ? 'argumento' : 'variable'}
                                                </span>
                                              )}

                                              {/* Fase 3: Badges de recomendación */}
                                              {(() => {
                                                const status = getRecommendationStatus(p);
                                                if (status === 'recommended') {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--compatible-bg)] text-[var(--compatible)] border border-compatible/30 font-semibold">
                                                      ⭐ Recomendado
                                                    </span>
                                                  );
                                                }
                                                if (status === 'not_applicable') {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--error-bg)] text-[var(--error)] border border-error/30">
                                                      ℹ️ No aplica
                                                    </span>
                                                  );
                                                }
                                                if (status === 'not_recommended') {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--card)] text-[var(--text-dim)] border border-[var(--border)]">
                                                      🔒 Proton
                                                    </span>
                                                  );
                                                }
                                                if (p.experimental) {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--caution-bg)] text-[var(--caution)] border border-caution/30">
                                                      ⚠️ Experimental
                                                    </span>
                                                  );
                                                }
                                                return null;
                                              })()}

                                              {/* `missing()` deshabilitaba la tarjeta sin decir por qué: el
                                                  usuario veía un gris inerte. El motivo ya estaba calculado. */}
                                              {isMissing && (
                                                <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">
                                                  🚫 {isMissing}
                                                </span>
                                              )}

                                              {enabled[idx] && <Check className="w-4 h-4 text-[var(--caution)]" />}
                                            </div>
                                            <p className="text-sm text-[var(--text-muted)]">
                                              {expertMode ? p.why : (p.simple_why || p.why)}
                                            </p>
                                            {/* Un ajuste interno se lee siempre, aunque esté
                                                «desactivado»: es un consejo, no un toggle. */}
                                            {(esAjusteInterno || enabled[idx]) && p.warn && (
                                              <p className="text-sm text-[var(--caution)] mt-2 flex items-start gap-2">
                                                <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
                                                {p.warn}
                                              </p>
                                            )}
                                            {(esAjusteInterno || enabled[idx]) && p.source && (
                                              <button
                                                onClick={(e) => { e.stopPropagation(); openExternal(p.source!); }}
                                                aria-label={`Abrir la fuente de «${p.title}» en el navegador`}
                                                className="text-sm text-[var(--action)] hover:text-[var(--action-hover)] mt-2 inline-flex items-center gap-1 rounded"
                                              >
                                                <ExternalLink className="w-4 h-4" /> Fuente
                                              </button>
                                            )}

                                            {/* Fase 3: Metadata expandible para modo experto */}
                                            {expertMode && enabled[idx] && (p.tested_on || p.validated_date || p.measured_improvement) && (
                                              <div className="mt-3 p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
                                                <p className="text-xs font-semibold text-[var(--text-muted)] mb-2 uppercase tracking-wider">Detalles técnicos</p>
                                                <div className="space-y-1.5 text-xs">
                                                  {p.tested_on && (
                                                    <div className="flex items-start gap-2">
                                                      <span className="text-[var(--text-dim)] min-w-[80px]">Probado en:</span>
                                                      <span className="text-[var(--text)]">{p.tested_on}</span>
                                                    </div>
                                                  )}
                                                  {p.validated_date && (
                                                    <div className="flex items-start gap-2">
                                                      <span className="text-[var(--text-dim)] min-w-[80px]">Validado:</span>
                                                      <span className="text-[var(--text)]">{p.validated_date}</span>
                                                    </div>
                                                  )}
                                                  {p.measured_improvement && (
                                                    <div className="flex items-start gap-2">
                                                      <span className="text-[var(--text-dim)] min-w-[80px]">Mejora:</span>
                                                      <span className="text-[var(--compatible)] font-semibold">{p.measured_improvement}</span>
                                                    </div>
                                                  )}
                                                  {p.evidence && (
                                                    <div className="flex items-start gap-2">
                                                      <span className="text-[var(--text-dim)] min-w-[80px]">Evidencia:</span>
                                                      <span className="text-[var(--text)]">{evidenceLabel(p.evidence)}</span>
                                                    </div>
                                                  )}
                                                </div>
                                              </div>
                                            )}
                                          </div>
                                        </div>
                                      </div>
                                    );
                                  })}
                              </div>
                            </div>
                          )}
                        </article>
    </div>
  );
}
