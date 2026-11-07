import { ExternalLink } from 'lucide-react';
import { openExternal } from '../lib/links';

/** Pestaña "Diagnóstico": cómo medir y herramientas de medición (contenido estático). */
export function DiagnosticTab() {
  return (
                        <div className="space-y-4">
                          <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                            <h3 className="text-base font-bold mb-3 text-[var(--text)]">Cómo medir el rendimiento</h3>
                            <div className="space-y-4">
                              <div className="flex gap-3">
                                <div className="w-7 h-7 rounded-full bg-[var(--action)]/20 text-[var(--action)] flex items-center justify-center text-sm font-bold shrink-0">1</div>
                                <div>
                                  <p className="text-sm font-semibold text-[var(--text)]">Establece una referencia</p>
                                  <p className="text-sm text-[var(--text-muted)] mt-1">Juega la misma escena sin ajustes y registra FPS y frametime con MangoHud.</p>
                                </div>
                              </div>
                              <div className="flex gap-3">
                                <div className="w-7 h-7 rounded-full bg-[var(--action)]/20 text-[var(--action)] flex items-center justify-center text-sm font-bold shrink-0">2</div>
                                <div>
                                  <p className="text-sm font-semibold text-[var(--text)]">Cambia una sola cosa</p>
                                  <p className="text-sm text-[var(--text-muted)] mt-1">Activa GameMode o modifica un ajuste. No cambies múltiples cosas a la vez.</p>
                                </div>
                              </div>
                              <div className="flex gap-3">
                                <div className="w-7 h-7 rounded-full bg-[var(--action)]/20 text-[var(--action)] flex items-center justify-center text-sm font-bold shrink-0">3</div>
                                <div>
                                  <p className="text-sm font-semibold text-[var(--text)]">Compara resultados</p>
                                  <p className="text-sm text-[var(--text-muted)] mt-1">Revisa FPS medios, 1% low y estabilidad. Repite 2-3 veces para confirmar.</p>
                                </div>
                              </div>
                            </div>
                          </article>

                          <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                            <h3 className="text-base font-bold mb-3 text-[var(--text)]">Herramientas de medición</h3>
                            <div className="space-y-3">
                              <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
                                <div className="flex items-center gap-2 mb-2">
                                  <h4 className="text-sm font-semibold text-[var(--text)]">MangoHud</h4>
                                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--compatible-bg)] text-[var(--compatible)] border border-[var(--compatible)]/30">
                                    ESENCIAL
                                  </span>
                                </div>
                                <p className="text-sm text-[var(--text-muted)] mb-2">
                                  Muestra FPS, frametime, uso de CPU/GPU y temperaturas. Presiona Shift+F2 para grabar logs.
                                </p>
                                <button
                                  onClick={() => openExternal('https://github.com/flightlessmango/MangoHud')}
                                  className="text-xs text-[var(--action)] hover:text-[var(--action-hover)] inline-flex items-center gap-1"
                                >
                                  <ExternalLink className="w-3 h-3" /> Documentación
                                </button>
                              </div>

                              <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
                                <div className="flex items-center gap-2 mb-2">
                                  <h4 className="text-sm font-semibold text-[var(--text)]">GameMode</h4>
                                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--caution-bg)] text-[var(--caution)] border border-[var(--caution)]/30">
                                    OPCIONAL
                                  </span>
                                </div>
                                <p className="text-sm text-[var(--text-muted)] mb-2">
                                  Optimiza el governor de CPU, prioridades de I/O y desactiva screensavers durante el juego.
                                </p>
                                <button
                                  onClick={() => openExternal('https://github.com/FeralInteractive/gamemode')}
                                  className="text-xs text-[var(--action)] hover:text-[var(--action-hover)] inline-flex items-center gap-1"
                                >
                                  <ExternalLink className="w-3 h-3" /> Documentación
                                </button>
                              </div>
                            </div>
                          </article>

                          <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                            <h3 className="text-base font-bold mb-3 text-[var(--text)]">Qué debes mirar</h3>
                            <p className="text-sm text-[var(--text-muted)] mb-3">
                              El FPS medio no cuenta toda la historia. Un 1% low bajo o un frametime irregular puede revelar tirones incluso con muchos FPS.
                            </p>
                            <div className="space-y-2 text-sm">
                              <div className="flex items-start gap-2">
                                <span className="text-[var(--compatible)] font-bold">•</span>
                                <span className="text-[var(--text)]"><strong>FPS medios:</strong> Rendimiento general</span>
                              </div>
                              <div className="flex items-start gap-2">
                                <span className="text-[var(--caution)] font-bold">•</span>
                                <span className="text-[var(--text)]"><strong>1% low:</strong> Los frames más lentos (más importante)</span>
                              </div>
                              <div className="flex items-start gap-2">
                                <span className="text-[var(--caution)] font-bold">•</span>
                                <span className="text-[var(--text)]"><strong>Frametime:</strong> Consistencia entre frames</span>
                              </div>
                            </div>
                          </article>
                        </div>
  );
}
