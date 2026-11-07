import { ExternalLink } from 'lucide-react';
import { openExternal } from '../lib/links';
import type { Game, GameEngine, GameInfo, GameRules } from '../lib/types';

interface CompatTabProps {
  current: Game;
  rules: GameRules;
  info: GameInfo;
  engine: GameEngine | null;
  forceProton: boolean;
  onToggleProton: () => void;
}

/** Pestaña "Compatibilidad": datos de ProtonDB, nativo/Proton y cuello de botella. */
export function CompatTab({ current, rules, info, engine, forceProton, onToggleProton }: CompatTabProps) {
  return (
    <div className="space-y-4">
                        <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <h3 className="text-base font-bold mb-3 text-[var(--text)]">Información del juego</h3>
                          <div className="space-y-3">
                            <div className="flex justify-between text-sm">
                              <span className="text-[var(--text-muted)]">Nombre</span>
                              <strong className="text-[var(--text)]">{info.name || current.name}</strong>
                            </div>
                            <div className="flex justify-between text-sm">
                              <span className="text-[var(--text-muted)]">AppID</span>
                              <strong className="text-[var(--text)]">{current.appid}</strong>
                            </div>
                            <div className="flex justify-between text-sm">
                              <span className="text-[var(--text-muted)]">Ejecución</span>
                              <strong className={info.native ? 'text-[var(--compatible)]' : 'text-[var(--caution)]'}>
                                {info.native ? 'Linux nativo' : 'Requiere Proton'}
                              </strong>
                            </div>

                            {/* Fase 3: Toggle para forzar Proton en juegos nativos */}
                            {info.native === true && (
                              <div className="mt-3 p-3 rounded-lg bg-[var(--card)] border border-[var(--border)]">
                                <div className="flex items-center justify-between">
                                  <div className="flex-1">
                                    <p className="text-sm font-semibold text-[var(--text)]">Probar con Proton</p>
                                    <p className="text-xs text-[var(--text-muted)] mt-1">
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
                              </div>
                            )}

                            {engine && (
                              <div className="flex justify-between text-sm">
                                <span className="text-[var(--text-muted)]">Motor</span>
                                <strong className="text-[var(--text)]">{engine.name}</strong>
                              </div>
                            )}
                            {engine && (
                              <div className="flex justify-between text-sm">
                                <span className="text-[var(--text-muted)]">API gráfica</span>
                                <strong className="text-[var(--text)]">{engine.api.toUpperCase()}</strong>
                              </div>
                            )}
                          </div>
                        </article>

                        {info.tier && (
                          <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                            <h3 className="text-base font-bold mb-3 text-[var(--text)]">Compatibilidad ProtonDB</h3>
                            <div className="space-y-3">
                              <div className="flex justify-between text-sm">
                                <span className="text-[var(--text-muted)]">Tier</span>
                                <strong className={`font-bold ${
                                  info.tier.toLowerCase() === 'platinum' ? 'text-[var(--compatible)]' :
                                  info.tier.toLowerCase() === 'gold' ? 'text-[var(--caution)]' :
                                  info.tier.toLowerCase() === 'silver' ? 'text-[var(--text)]' :
                                  info.tier.toLowerCase() === 'bronze' ? 'text-[var(--caution)]' :
                                  'text-[var(--error)]'
                                }`}>
                                  {info.tier.toLowerCase() === 'platinum' ? 'Platino' :
                                   info.tier.toLowerCase() === 'gold' ? 'Oro' :
                                   info.tier.toLowerCase() === 'silver' ? 'Plata' :
                                   info.tier.toLowerCase() === 'bronze' ? 'Bronce' :
                                   info.tier.toLowerCase() === 'borked' ? 'No funciona' :
                                   info.tier}
                                </strong>
                              </div>
                              {info.total && (
                                <div className="flex justify-between text-sm">
                                  <span className="text-[var(--text-muted)]">Reportes</span>
                                  <strong className="text-[var(--text)]">{info.total}</strong>
                                </div>
                              )}
                              {info.confidence && (
                                <div className="flex justify-between text-sm">
                                  <span className="text-[var(--text-muted)]">Confianza</span>
                                  <strong className="text-[var(--text)]">
                                    {info.confidence.toLowerCase() === 'strong' ? 'Alta' :
                                     info.confidence.toLowerCase() === 'moderate' ? 'Moderada' :
                                     info.confidence.toLowerCase() === 'weak' ? 'Baja' :
                                     info.confidence}
                                  </strong>
                                </div>
                              )}
                            </div>
                            <button
                              onClick={() => openExternal(`https://www.protondb.com/app/${current.appid}`)}
                              className="mt-4 w-full flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors"
                            >
                              <ExternalLink className="w-4 h-4" /> Ver en ProtonDB
                            </button>
                          </article>
                        )}

                        <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <h3 className="text-base font-bold mb-3 text-[var(--text)]">Cuello de botella</h3>
                          <p className="text-sm text-[var(--text-muted)] mb-3">{rules.bottleneck}</p>
                          <p className="text-sm text-[var(--text)]">{rules.summary}</p>
                        </article>
    </div>
  );
}
