import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion, AnimatePresence } from 'framer-motion';
import {
  TrendingUp, Upload, Trash2, BarChart3, CheckCircle,
  AlertCircle, Zap, Target, FileText, Search
} from 'lucide-react';

interface Benchmark {
  id: string;
  profile_id: string;
  game_name: string;
  appid: number;
  timestamp: string;
  duration_seconds: number;
  avg_fps: number;
  fps_1_percent_low: number;
  fps_0_1_percent_low: number;
  max_fps: number;
  min_fps: number;
  frame_time_avg: number;
  frame_time_1_percent_high: number;
  total_frames: number;
  notes: string | null;
  launch_command: string;
}

interface BenchmarkComparison {
  baseline: Benchmark;
  test: Benchmark;
  fps_change_percent: number;
  fps_1_percent_change_percent: number;
  fps_0_1_percent_change_percent: number;
  frame_time_change_percent: number;
  conclusion: string;
}

interface BenchmarksViewProps {
  currentAppid?: number;
  currentGameName?: string;
  currentLaunchCommand?: string;
  mangohudLogPath?: string;
  onLogPathChange?: (path: string) => void;
}

export function BenchmarksView({ currentAppid, currentGameName, currentLaunchCommand, mangohudLogPath, onLogPathChange }: BenchmarksViewProps) {
  const [benchmarks, setBenchmarks] = useState<Benchmark[]>([]);
  const [selectedBaseline, setSelectedBaseline] = useState<string | null>(null);
  const [selectedTest, setSelectedTest] = useState<string | null>(null);
  const [comparison, setComparison] = useState<BenchmarkComparison | null>(null);
  const [loading, setLoading] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [logPath, setLogPath] = useState(mangohudLogPath || '/tmp/MangoHud');
  const [notes, setNotes] = useState('');
  const [toast, setToast] = useState<string | null>(null);
  const [mangohudInstalled, setMangohudInstalled] = useState<boolean | null>(null);

  // Sincronizar con el estado global cuando cambie
  useEffect(() => {
    if (mangohudLogPath) {
      setLogPath(mangohudLogPath);
    }
  }, [mangohudLogPath]);

  // Controlar duración del toast (3 segundos)
  useEffect(() => {
    if (toast) {
      const timer = setTimeout(() => setToast(null), 3000);
      return () => clearTimeout(timer);
    }
  }, [toast]);

  // Detectar si MangoHud está instalado
  useEffect(() => {
    const checkMangoHud = async () => {
      try {
        const installed = await invoke<boolean>('check_mangohud_installed');
        setMangohudInstalled(installed);
      } catch (e) {
        console.error('Error detectando MangoHud:', e);
        setMangohudInstalled(false);
      }
    };

    checkMangoHud();
  }, []);

  // Guardar en localStorage cuando cambie
  const handleLogPathChange = (newPath: string) => {
    setLogPath(newPath);
    localStorage.setItem('mangohud_log_path', newPath);
    if (onLogPathChange) {
      onLogPathChange(newPath);
    }
  };

  useEffect(() => {
    loadBenchmarks();
  }, [currentAppid]);

  const loadBenchmarks = async () => {
    try {
      if (currentAppid) {
        const gameBenchmarks = await invoke<Benchmark[]>('load_benchmarks_for_game', { appid: currentAppid });
        setBenchmarks(gameBenchmarks);
      } else {
        const allBenchmarks = await invoke<Benchmark[]>('load_benchmarks');
        setBenchmarks(allBenchmarks);
      }
    } catch (e) {
      console.error('Error al cargar benchmarks:', e);
    }
  };

  const analyzeLog = async () => {
    if (!logPath.trim()) {
      setToast('Ingresa la ruta del log de MangoHud');
      return;
    }

    setAnalyzing(true);
    try {
      const benchmark = await invoke<Benchmark>('analyze_mangohud_log_command', { logPath });

      // Guardar el benchmark
      await invoke('save_benchmark', {
        benchmark,
        profileId: '',
        gameName: currentGameName || 'Desconocido',
        appid: currentAppid || 0,
        launchCommand: currentLaunchCommand || '',
        notes: notes || null,
      });

      setToast('✓ Benchmark analizado y guardado');
      setNotes('');
      loadBenchmarks();
    } catch (e) {
      setToast(`❌ Error: ${e}`);
    } finally {
      setAnalyzing(false);
    }
  };

  const compareBenchmarks = async () => {
    if (!selectedBaseline || !selectedTest) {
      setToast('Selecciona un benchmark base y uno de prueba');
      return;
    }

    setLoading(true);
    try {
      const result = await invoke<BenchmarkComparison>('compare_benchmarks', {
        baselineId: selectedBaseline,
        testId: selectedTest,
      });
      setComparison(result);
    } catch (e) {
      setToast(`❌ Error al comparar: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const deleteBenchmark = async (id: string) => {
    if (!confirm('¿Eliminar este benchmark?')) return;

    try {
      await invoke('delete_benchmark', { benchmarkId: id });
      setToast('✓ Benchmark eliminado');
      loadBenchmarks();

      if (selectedBaseline === id) setSelectedBaseline(null);
      if (selectedTest === id) setSelectedTest(null);
      if (comparison?.baseline.id === id || comparison?.test.id === id) {
        setComparison(null);
      }
    } catch (e) {
      setToast(`❌ Error al eliminar: ${e}`);
    }
  };

  const getChangeColor = (percent: number) => {
    if (percent > 5) return 'text-[var(--compatible)]';
    if (percent > 0) return 'text-[var(--caution)]';
    if (percent > -5) return 'text-[var(--text-muted)]';
    return 'text-[var(--error)]';
  };

  const getChangeIcon = (percent: number) => {
    if (percent > 5) return <TrendingUp className="w-4 h-4 text-[var(--compatible)]" />;
    if (percent > 0) return <TrendingUp className="w-4 h-4 text-[var(--caution)]" />;
    if (percent > -5) return <AlertCircle className="w-4 h-4 text-[var(--text-muted)]" />;
    return <AlertCircle className="w-4 h-4 text-[var(--error)]" />;
  };

  return (
    <div className="space-y-6">
      {/* Toast */}
      <AnimatePresence>
        {toast && (
          <motion.div
            initial={{ opacity: 0, y: 50 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 50 }}
            className="fixed bottom-6 right-6 z-50 px-5 py-3 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-xl shadow-[var(--action)]/20 text-sm font-semibold text-white"
          >
            {toast}
          </motion.div>
        )}
      </AnimatePresence>

      {/* Mensaje si MangoHud no está instalado */}
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
                <p className="text-xs text-[var(--text-dim)] mt-2">
                  Después de instalar, configura MangoHud para grabar logs automáticamente.
                </p>
              </div>
            </div>
          </div>
        </article>
      )}

      {/* Analizar Log de MangoHud */}
      <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <div className="flex items-start justify-between mb-4">
          <div>
            <h3 className="text-base font-bold text-[var(--text)] flex items-center gap-2">
              <Upload className="w-5 h-5 text-[var(--action)]" />
              Analizar Log de MangoHud
            </h3>
            <p className="text-sm text-[var(--text-muted)] mt-1">
              Graba un log con MangoHud (Shift+F2) durante al menos 30 segundos y analiza el rendimiento.
            </p>
          </div>
        </div>

        <div className="space-y-3">
          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="text-xs font-semibold text-[var(--text-muted)]">
                Ruta del log de MangoHud
              </label>
              <button
                onClick={async () => {
                  try {
                    const detectedPath = await invoke<string>('detect_mangohud_log_path');
                    handleLogPathChange(detectedPath);
                    setToast('✓ Ruta detectada automáticamente');
                  } catch (e) {
                    setToast(`❌ Error detectando ruta: ${e}`);
                  }
                }}
                className="text-xs text-[var(--action)] hover:text-[var(--action-hover)] flex items-center gap-1"
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
              className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm text-[var(--text)] focus:outline-none focus:border-[var(--action)]"
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
              className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm text-[var(--text)] focus:outline-none focus:border-[var(--action)]"
            />
          </div>

          <button
            onClick={analyzeLog}
            disabled={analyzing}
            className="w-full px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
          >
            {analyzing ? (
              <>
                <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                Analizando...
              </>
            ) : (
              <>
                <FileText className="w-4 h-4" />
                Analizar y Guardar Benchmark
              </>
            )}
          </button>
        </div>
      </article>

      {/* Lista de Benchmarks */}
      {benchmarks.length > 0 && (
        <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
          <h3 className="text-base font-bold text-[var(--text)] mb-4 flex items-center gap-2">
            <BarChart3 className="w-5 h-5 text-[var(--action)]" />
            Benchmarks Guardados ({benchmarks.length})
          </h3>

          <div className="space-y-2 max-h-96 overflow-y-auto">
            {benchmarks.map((bench) => (
              <div
                key={bench.id}
                className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <input
                        type="radio"
                        name="baseline"
                        checked={selectedBaseline === bench.id}
                        onChange={() => setSelectedBaseline(bench.id)}
                        className="w-4 h-4 accent-[var(--compatible)]"
                      />
                      <span className="text-xs text-[var(--compatible)] font-semibold">Base</span>

                      <input
                        type="radio"
                        name="test"
                        checked={selectedTest === bench.id}
                        onChange={() => setSelectedTest(bench.id)}
                        className="w-4 h-4 accent-[var(--caution)] ml-2"
                      />
                      <span className="text-xs text-[var(--caution)] font-semibold">Prueba</span>
                    </div>

                    <p className="text-sm font-semibold text-[var(--text)] truncate">
                      {bench.game_name}
                    </p>
                    <p className="text-xs text-[var(--text-muted)] mt-1">
                      {new Date(bench.timestamp).toLocaleString('es-ES')}
                    </p>
                    {bench.notes && (
                      <p className="text-xs text-[var(--text-dim)] mt-1 italic">
                        {bench.notes}
                      </p>
                    )}

                    <div className="grid grid-cols-3 gap-2 mt-2 text-xs">
                      <div>
                        <span className="text-[var(--text-muted)]">FPS Promedio:</span>
                        <span className="ml-1 font-semibold text-[var(--text)]">{bench.avg_fps.toFixed(1)}</span>
                      </div>
                      <div>
                        <span className="text-[var(--text-muted)]">1% Low:</span>
                        <span className="ml-1 font-semibold text-[var(--text)]">{bench.fps_1_percent_low.toFixed(1)}</span>
                      </div>
                      <div>
                        <span className="text-[var(--text-muted)]">0.1% Low:</span>
                        <span className="ml-1 font-semibold text-[var(--text)]">{bench.fps_0_1_percent_low.toFixed(1)}</span>
                      </div>
                    </div>
                  </div>

                  <button
                    onClick={() => deleteBenchmark(bench.id)}
                    className="p-2 rounded-lg text-[var(--error)] hover:bg-[var(--error)]/10 transition-colors"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
            ))}
          </div>

          {selectedBaseline && selectedTest && selectedBaseline !== selectedTest && (
            <button
              onClick={compareBenchmarks}
              disabled={loading}
              className="w-full mt-4 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
            >
              {loading ? (
                <>
                  <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  Comparando...
                </>
              ) : (
                <>
                  <Target className="w-4 h-4" />
                  Comparar Benchmarks Seleccionados
                </>
              )}
            </button>
          )}
        </article>
      )}

      {/* Resultados de Comparación */}
      {comparison && (
        <article className="p-5 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)]">
          <h3 className="text-base font-bold text-[var(--text)] mb-4 flex items-center gap-2">
            <CheckCircle className="w-5 h-5 text-[var(--compatible)]" />
            Resultados de la Comparación
          </h3>

          <div className="p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] mb-4">
            <p className="text-sm font-semibold text-[var(--text)] mb-2">
              {comparison.conclusion}
            </p>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <p className="text-xs text-[var(--compatible)] font-semibold mb-2">Benchmark Base</p>
              <div className="space-y-1 text-xs">
                <div className="flex justify-between">
                  <span className="text-[var(--text-muted)]">FPS Promedio:</span>
                  <span className="font-semibold text-[var(--text)]">{comparison.baseline.avg_fps.toFixed(1)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-muted)]">1% Low:</span>
                  <span className="font-semibold text-[var(--text)]">{comparison.baseline.fps_1_percent_low.toFixed(1)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-muted)]">0.1% Low:</span>
                  <span className="font-semibold text-[var(--text)]">{comparison.baseline.fps_0_1_percent_low.toFixed(1)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-muted)]">Frame Time:</span>
                  <span className="font-semibold text-[var(--text)]">{comparison.baseline.frame_time_avg.toFixed(2)}ms</span>
                </div>
              </div>
            </div>

            <div className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <p className="text-xs text-[var(--caution)] font-semibold mb-2">Benchmark de Prueba</p>
              <div className="space-y-1 text-xs">
                <div className="flex justify-between">
                  <span className="text-[var(--text-muted)]">FPS Promedio:</span>
                  <span className="font-semibold text-[var(--text)]">{comparison.test.avg_fps.toFixed(1)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-muted)]">1% Low:</span>
                  <span className="font-semibold text-[var(--text)]">{comparison.test.fps_1_percent_low.toFixed(1)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-muted)]">0.1% Low:</span>
                  <span className="font-semibold text-[var(--text)]">{comparison.test.fps_0_1_percent_low.toFixed(1)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-[var(--text-muted)]">Frame Time:</span>
                  <span className="font-semibold text-[var(--text)]">{comparison.test.frame_time_avg.toFixed(2)}ms</span>
                </div>
              </div>
            </div>
          </div>

          <div className="mt-4 space-y-2">
            <div className="flex items-center justify-between p-2 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <span className="text-xs text-[var(--text-muted)]">Cambio en FPS Promedio:</span>
              <div className="flex items-center gap-2">
                {getChangeIcon(comparison.fps_change_percent)}
                <span className={`text-sm font-bold ${getChangeColor(comparison.fps_change_percent)}`}>
                  {comparison.fps_change_percent > 0 ? '+' : ''}{comparison.fps_change_percent.toFixed(1)}%
                </span>
              </div>
            </div>

            <div className="flex items-center justify-between p-2 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <span className="text-xs text-[var(--text-muted)]">Cambio en 1% Low:</span>
              <div className="flex items-center gap-2">
                {getChangeIcon(comparison.fps_1_percent_change_percent)}
                <span className={`text-sm font-bold ${getChangeColor(comparison.fps_1_percent_change_percent)}`}>
                  {comparison.fps_1_percent_change_percent > 0 ? '+' : ''}{comparison.fps_1_percent_change_percent.toFixed(1)}%
                </span>
              </div>
            </div>

            <div className="flex items-center justify-between p-2 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <span className="text-xs text-[var(--text-muted)]">Cambio en 0.1% Low:</span>
              <div className="flex items-center gap-2">
                {getChangeIcon(comparison.fps_0_1_percent_change_percent)}
                <span className={`text-sm font-bold ${getChangeColor(comparison.fps_0_1_percent_change_percent)}`}>
                  {comparison.fps_0_1_percent_change_percent > 0 ? '+' : ''}{comparison.fps_0_1_percent_change_percent.toFixed(1)}%
                </span>
              </div>
            </div>

            <div className="flex items-center justify-between p-2 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <span className="text-xs text-[var(--text-muted)]">Cambio en Frame Time:</span>
              <div className="flex items-center gap-2">
                {comparison.frame_time_change_percent < 0 ? (
                  <TrendingUp className="w-4 h-4 text-[var(--compatible)]" />
                ) : (
                  <AlertCircle className="w-4 h-4 text-[var(--error)]" />
                )}
                <span className={`text-sm font-bold ${comparison.frame_time_change_percent < 0 ? 'text-[var(--compatible)]' : 'text-[var(--error)]'}`}>
                  {comparison.frame_time_change_percent > 0 ? '+' : ''}{comparison.frame_time_change_percent.toFixed(1)}%
                </span>
              </div>
            </div>
          </div>

          <button
            onClick={() => setComparison(null)}
            className="w-full mt-4 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
          >
            Cerrar Comparación
          </button>
        </article>
      )}

      {/* Tutorial */}
      <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <h3 className="text-base font-bold text-[var(--text)] mb-3 flex items-center gap-2">
          <Zap className="w-5 h-5 text-[var(--caution)]" />
          Cómo Medir el Rendimiento
        </h3>
        <div className="space-y-3 text-sm text-[var(--text-muted)]">
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[var(--action)]/20 text-[var(--action)] flex items-center justify-center text-xs font-bold shrink-0">1</div>
            <div>
              <p className="font-semibold text-[var(--text)]">Establece una referencia</p>
              <p className="text-xs mt-1">Juega la misma escena sin ajustes y graba un log con MangoHud (Shift+F2) durante al menos 30 segundos.</p>
            </div>
          </div>
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[var(--action)]/20 text-[var(--action)] flex items-center justify-center text-xs font-bold shrink-0">2</div>
            <div>
              <p className="font-semibold text-[var(--text)]">Cambia una sola cosa</p>
              <p className="text-xs mt-1">Activa un parámetro (ej: game-performance) y repite la misma prueba en las mismas condiciones.</p>
            </div>
          </div>
          <div className="flex gap-3">
            <div className="w-6 h-6 rounded-full bg-[var(--action)]/20 text-[var(--action)] flex items-center justify-center text-xs font-bold shrink-0">3</div>
            <div>
              <p className="font-semibold text-[var(--text)]">Compara resultados</p>
              <p className="text-xs mt-1">Selecciona el benchmark base y el de prueba, luego haz clic en "Comparar". El 1% low es más importante que los FPS medios.</p>
            </div>
          </div>
        </div>
      </article>
    </div>
  );
}
