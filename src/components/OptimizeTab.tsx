import { AlertTriangle, Check, Cpu, ExternalLink, Settings, Terminal, Users, Zap } from 'lucide-react';
import { openExternal } from '../lib/links';
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
  return (
    <div className="space-y-4">
                        <div className={`p-3 rounded-lg border ${
                          expertMode
                            ? 'bg-[var(--action)]/5 border-[var(--action)]/20'
                            : 'bg-[var(--compatible-bg)] border-[var(--compatible)]/20'
                        }`}>
                          <div className="flex items-center gap-2">
                            {expertMode ? (
                              <>
                                <Settings className="w-4 h-4 text-[var(--action)]" />
                                <p className="text-xs text-[var(--action)] font-semibold">Modo experto activado</p>
                              </>
                            ) : (
                              <>
                                <Check className="w-4 h-4 text-[var(--compatible)]" />
                                <p className="text-xs text-[var(--compatible)] font-semibold">Modo guiado: solo ajustes esenciales</p>
                              </>
                            )}
                          </div>
                          <p className="text-[10px] text-[var(--text-muted)] mt-1">
                            {expertMode
                              ? 'Mostrando todos los parámetros con detalles técnicos completos'
                              : 'Mostrando solo los parámetros más importantes. Activa el modo experto en el sidebar para ver todo.'}
                          </p>
                        </div>

                        {/* Fase 3: Toggle Proton estilo Discord */}
                        {info.native === true && (
                          <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
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
                        <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
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
                                  ? 'bg-[var(--compatible-bg)] border-[var(--compatible)]/50'
                                  : 'bg-[var(--bg)] border-[var(--border)] hover:border-[var(--compatible)]/40'
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

                        {/* Ajustes para probar */}
                        <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
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
                            const systemAdjustments = rules.params
                              .filter(p => p.value === 'game-performance' || p.value === 'gamemoderun')
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
                                              ? 'bg-[var(--caution-bg)] border-[var(--caution)]/50'
                                              : 'bg-[var(--bg)] border-[var(--border)] hover:border-[var(--caution)]/40'
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
                                        {p.requires && p.requires.length > 0 && (
                                          <p className="text-[10px] text-[var(--text-dim)] mt-1">
                                            Requisito: {p.requires.join(', ')}
                                          </p>
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
                                {engineRecommendations.map((rec, i) => (
                                  <button
                                    key={i}
                                    onClick={() => onToggleEngineRec(i)}
                                    className={`text-left p-3 rounded-lg border transition-all ${
                                      selectedEngineRecs.has(i)
                                        ? 'bg-[var(--action)]/20 border-[var(--action)]/50'
                                        : 'bg-[var(--bg)] border-[var(--border)] hover:border-[var(--action)]/40'
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
                            const filteredRecs = communityRecommendations
                              .filter(rec => rec.parameter !== 'mangohud' && rec.parameter !== 'gamemoderun')
                              .filter(rec => expertMode || rec.recommended === true);

                            if (filteredRecs.length === 0) return null;

                            return (
                              <div className="mb-6">
                                <h4 className="text-sm font-semibold text-[var(--compatible)] mb-3 flex items-center gap-2">
                                  <Users className="w-4 h-4" />
                                  Recomendaciones por hardware
                                </h4>
                                <div className={`grid gap-2 ${filteredRecs.length === 1 ? 'grid-cols-1' : 'grid-cols-1 md:grid-cols-2'}`}>
                                  {filteredRecs.map((rec, i) => (
                                    <button
                                      key={i}
                                      onClick={() => onToggleCommunityRec(i)}
                                      className={`text-left p-3 rounded-lg border transition-all ${
                                        selectedCommunityRecs.has(i)
                                          ? 'bg-[var(--compatible-bg)] border-[var(--compatible)]/50'
                                          : 'bg-[var(--bg)] border-[var(--border)] hover:border-[var(--compatible)]/40'
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
                                  <span className="text-[10px] px-2 py-0.5 rounded bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30">
                                    Modo experto
                                  </span>
                                )}
                              </div>
                              <div className="space-y-2">
                                {rules.params
                                  .filter(p => p.impact !== 'evitar')
                                  .filter(p => p.value !== 'mangohud' && p.value !== 'gamemoderun' && p.value !== 'game-performance')
                                  .filter(p => !p.requires?.includes('game-performance'))
                                  .filter(p => showUnavailable || !missing(p))
                                  .filter(p => expertMode || p.essential === true)
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
                                                ? 'bg-[var(--caution-bg)] border-[var(--caution)]/50'
                                                : 'bg-[var(--bg)] border-[var(--border)] hover:border-[var(--caution)]/40'
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
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--compatible-bg)] text-[var(--compatible)] border border-[var(--compatible)]/30 font-semibold">
                                                      ⭐ Recomendado
                                                    </span>
                                                  );
                                                }
                                                if (status === 'not_applicable') {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--error-bg)] text-[var(--error)] border border-[var(--error)]/30">
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
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--caution-bg)] text-[var(--caution)] border border-[var(--caution)]/30">
                                                      ⚠️ Experimental
                                                    </span>
                                                  );
                                                }
                                                return null;
                                              })()}

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
