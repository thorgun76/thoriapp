import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion, AnimatePresence } from 'framer-motion';
import {
  TrendingUp, Upload, Trash2, BarChart3, CheckCircle,
  AlertCircle, Zap, Target, FileText, Search, Clock, Gamepad2
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

// Componente Toggle moderno (consistente con el resto de la app)
function Toggle({
  enabled,
  onToggle,
  label,
  color = 'action'
}: {
  enabled: boolean;
  onToggle: () => void;
  label: string;
  color?: 'action' | 'compatible' | 'caution';
}) {
  const colorClasses = {
    action: 'bg-[var(--action)]',
    compatible: 'bg-[var(--compatible)]',
    caution: 'bg-[var(--caution)]',
  };

  return (
    <button
      type="button"
      onClick={onToggle}
      className="flex items-center gap-2 cursor-pointer group"
    >
      <div
        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors duration-200 ${
          enabled ? colorClasses[color] : 'bg-[var(--bg)] border border-[var(--border)]'
        }`}
      >
        <span
          className={`inline-block h-4 w-4 transform rounded-full bg-white shadow-sm transition-transform duration-200 ${
            enabled ? 'translate-x-6' : 'translate-x-1'
          }`}
        />
      </div>
      <span className={`text-xs font-semibold ${enabled ? 'text-[var(--text)]' : 'text-[var(--text-muted)]'}`}>
        {label}
      </span>
    </button>
  );
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

  useEffect(() => {
    if (mangohudLogPath) {
      setLogPath(mangohudLogPath);
    }
  }, [mangohudLogPath]);

  useEffect(() => {
    if (toast) {
      const timer = setTimeout(() => setToast(null), 3000);
      return () => clearTimeout(timer);
    }
  }, [toast]);

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

      const isDuplicate = benchmarks.some(b =>
        b.avg_fps === benchmark.avg_fps &&
        b.fps_1_percent_low === benchmark.fps_1_percent_low &&
        b.fps_0_1_percent_low === benchmark.fps_0_1_percent_low &&
        b.total_frames === benchmark.total_frames &&
        b.timestamp === benchmark.timestamp
      );

      if (isDuplicate) {
        setToast('⚠️ Ya existe un benchmark con estos valores (posible duplicado)');
        setAnalyzing(false);
        return;
      }

      let finalGameName = benchmark.game_name;

      if ((!finalGameName || finalGameName === 'Desconocido') && currentGameName && currentGameName !== 'Desconocido') {
        finalGameName = currentGameName;
      }

      await invoke('save_benchmark', {
        benchmark,
        profileId: '',
        gameName: finalGameName,
        appid: currentAppid || 0,
        launchCommand: currentLaunchCommand || '',
        notes: notes || null,
      });

      setToast(`✓ Benchmark guardado: ${finalGameName} (${benchmark.timestamp})`);
      setNotes('');
      loadBenchmarks();
    } catch (e) {
      setToast(`❌ Error: ${e}`);
    } finally {
      setAnalyzing(false);
    }
  };

  const toggleBaseline = (id: string) => {
    if (selectedBaseline === id) {
      setSelectedBaseline(null);
    } else {
      setSelectedBaseline(id);
      if (selectedTest === id) {
        setSelectedTest(null);
      }
    }
  };

  const toggleTest = (id: string) => {
    if (selectedTest === id) {
      setSelectedTest(null);
    } else {
      setSelectedTest(id);
      if (selectedBaseline === id) {
        setSelectedBaseline(null);
      }
    }
  };

  const deleteAllBenchmarks = async () => {
    if (!confirm(`¿Estás seguro de que quieres eliminar TODOS los ${benchmarks.length} benchmarks?\n\nEsta acción no se puede deshacer.`)) {
      return;
    }

    try {
      await invoke('delete_all_benchmarks');
      setToast(`✓ Se eliminaron ${benchmarks.length} benchmarks`);
      setSelectedBaseline(null);
      setSelectedTest(null);
      setComparison(null);
      loadBenchmarks();
    } catch (e) {
      setToast(`❌ Error al eliminar: ${e}`);
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

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString('es-ES', {
        day: '2-digit',
        month: '2-digit',
        year: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      });
    } catch {
      return timestamp;
    }
  };

  // Función para obtener color de FPS según rendimiento
  const getFpsColor = (fps: number) => {
    if (fps >= 60) return 'text-[var(--compatible)]';
    if (fps >= 30) return 'text-[var(--caution)]';
    return 'text-[var(--error)]';
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
            className="w-full px-4 py-2.5 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 shadow-lg shadow-[var(--action)]/20 hover:shadow-xl hover:shadow-[var(--action)]/30"
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
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-base font-bold text-[var(--text)] flex items-center gap-2">
              <BarChart3 className="w-5 h-5 text-[var(--action)]" />
              Benchmarks Guardados
              <span className="text-xs font-normal text-[var(--text-muted)] bg-[var(--bg)] px-2 py-0.5 rounded-full">
                {benchmarks.length}
              </span>
            </h3>
            <button
              onClick={deleteAllBenchmarks}
              className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-[var(--error)]/10 text-[var(--error)] border border-[var(--error)]/30 hover:bg-[var(--error)]/20 hover:border-[var(--error)]/50 hover:scale-105 transition-all duration-200 flex items-center gap-1.5 group"
              title="Eliminar todos los benchmarks"
            >
              <Trash2 className="w-3.5 h-3.5 group-hover:rotate-12 transition-transform" />
              Borrar todos
            </button>
          </div>

          <div className="space-y-3 max-h-[500px] overflow-y-auto pr-1">
            {benchmarks.map((bench) => {
              const isSelected = selectedBaseline === bench.id || selectedTest === bench.id;
              const isBaseline = selectedBaseline === bench.id;
              const isTest = selectedTest === bench.id;

              return (
                <motion.div
                  key={bench.id}
                  layout
                  className={`p-4 rounded-xl bg-[var(--bg)] border-2 transition-all duration-200 ${
                    isBaseline
                      ? 'border-[var(--compatible)] shadow-lg shadow-[var(--compatible)]/10'
                      : isTest
                      ? 'border-[var(--caution)] shadow-lg shadow-[var(--caution)]/10'
                      : isSelected
                      ? 'border-[var(--action)] shadow-md shadow-[var(--action)]/10'
                      : 'border-[var(--border)] hover:border-[var(--action)]/50 hover:shadow-md'
                  }`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0">
                      {/* Toggles modernos */}
                      <div className="flex items-center gap-4 mb-3">
                        <Toggle
                          enabled={isBaseline}
                          onToggle={() => toggleBaseline(bench.id)}
                          label="Base"
                          color="compatible"
                        />
                        <Toggle
                          enabled={isTest}
                          onToggle={() => toggleTest(bench.id)}
                          label="Prueba"
                          color="caution"
                        />
                      </div>

                      {/* Nombre del juego y fecha */}
                      <div className="flex items-center gap-2 mb-3">
                        <Gamepad2 className="w-4 h-4 text-[var(--action)]" />
                        <h4 className="text-base font-bold text-[var(--text)] truncate">
                          {bench.game_name || 'Desconocido'}
                        </h4>
                      </div>

                      <div className="flex items-center gap-1.5 text-xs text-[var(--text-muted)] mb-4">
                        <Clock className="w-3.5 h-3.5" />
                        <span>{formatTimestamp(bench.timestamp)}</span>
                        {bench.notes && (
                          <>
                            <span className="mx-1">•</span>
                            <span className="italic truncate max-w-[200px]">{bench.notes}</span>
                          </>
                        )}
                      </div>

                      {/* Estadísticas de FPS - Más grandes y visibles */}
                      <div className="grid grid-cols-3 gap-3">
                        <div className="bg-[var(--surface)] rounded-lg p-3 border border-[var(--border)]">
                          <div className="text-xs text-[var(--text-muted)] mb-1 font-medium">
                            FPS Promedio
                          </div>
                          <div className={`text-2xl font-bold ${getFpsColor(bench.avg_fps)}`}>
                            {bench.avg_fps.toFixed(1)}
                          </div>
                        </div>
                        <div className="bg-[var(--surface)] rounded-lg p-3 border border-[var(--border)]">
                          <div className="text-xs text-[var(--text-muted)] mb-1 font-medium">
                            1% Low
                          </div>
                          <div className={`text-2xl font-bold ${getFpsColor(bench.fps_1_percent_low)}`}>
                            {bench.fps_1_percent_low.toFixed(1)}
                          </div>
                        </div>
                        <div className="bg-[var(--surface)] rounded-lg p-3 border border-[var(--border)]">
                          <div className="text-xs text-[var(--text-muted)] mb-1 font-medium">
                            0.1% Low
                          </div>
                          <div className={`text-2xl font-bold ${getFpsColor(bench.fps_0_1_percent_low)}`}>
                            {bench.fps_0_1_percent_low.toFixed(1)}
                          </div>
                        </div>
                      </div>
                    </div>

                    <button
                      onClick={() => deleteBenchmark(bench.id)}
                      className="p-2 rounded-lg text-[var(--error)] hover:bg-[var(--error)]/10 hover:scale-110 transition-all duration-200"
                      title="Eliminar benchmark"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </motion.div>
              );
            })}
          </div>

          {selectedBaseline && selectedTest && selectedBaseline !== selectedTest && (
            <motion.button
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              onClick={compareBenchmarks}
              disabled={loading}
              className="w-full mt-4 px-4 py-3 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 shadow-lg shadow-[var(--action)]/20 hover:shadow-xl hover:shadow-[var(--action)]/30 hover:scale-[1.02]"
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
            </motion.button>
          )}
        </article>
      )}

      {/* Resultados de Comparación */}
      {comparison && (
        <motion.article
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="p-5 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-xl shadow-[var(--action)]/10"
        >
          <h3 className="text-base font-bold text-[var(--text)] mb-4 flex items-center gap-2">
            <CheckCircle className="w-5 h-5 text-[var(--compatible)]" />
            Resultados de la Comparación
          </h3>

          <div className="p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] mb-4">
            <p className="text-sm font-semibold text-[var(--text)]">
              {comparison.conclusion}
            </p>
          </div>

          <div className="grid grid-cols-2 gap-4 mb-4">
            <div className="p-4 rounded-lg bg-[var(--bg)] border-2 border-[var(--compatible)]/30">
              <p className="text-xs text-[var(--compatible)] font-bold mb-3 uppercase tracking-wide">Benchmark Base</p>
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <span className="text-xs text-[var(--text-muted)]">FPS Promedio:</span>
                  <span className="text-lg font-bold text-[var(--compatible)]">{comparison.baseline.avg_fps.toFixed(1)}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-xs text-[var(--text-muted)]">1% Low:</span>
                  <span className="text-lg font-bold text-[var(--compatible)]">{comparison.baseline.fps_1_percent_low.toFixed(1)}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-xs text-[var(--text-muted)]">0.1% Low:</span>
                  <span className="text-lg font-bold text-[var(--compatible)]">{comparison.baseline.fps_0_1_percent_low.toFixed(1)}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-xs text-[var(--text-muted)]">Frame Time:</span>
                  <span className="text-lg font-bold text-[var(--text)]">{comparison.baseline.frame_time_avg.toFixed(2)}ms</span>
                </div>
              </div>
            </div>

            <div className="p-4 rounded-lg bg-[var(--bg)] border-2 border-[var(--caution)]/30">
              <p className="text-xs text-[var(--caution)] font-bold mb-3 uppercase tracking-wide">Benchmark de Prueba</p>
              <div className="space-y-2">
                <div className="flex justify-between items-center">
                  <span className="text-xs text-[var(--text-muted)]">FPS Promedio:</span>
                  <span className="text-lg font-bold text-[var(--caution)]">{comparison.test.avg_fps.toFixed(1)}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-xs text-[var(--text-muted)]">1% Low:</span>
                  <span className="text-lg font-bold text-[var(--caution)]">{comparison.test.fps_1_percent_low.toFixed(1)}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-xs text-[var(--text-muted)]">0.1% Low:</span>
                  <span className="text-lg font-bold text-[var(--caution)]">{comparison.test.fps_0_1_percent_low.toFixed(1)}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-xs text-[var(--text-muted)]">Frame Time:</span>
                  <span className="text-lg font-bold text-[var(--text)]">{comparison.test.frame_time_avg.toFixed(2)}ms</span>
                </div>
              </div>
            </div>
          </div>

          <div className="space-y-2 mb-4">
            <div className="flex items-center justify-between p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <span className="text-xs text-[var(--text-muted)] font-medium">Cambio en FPS Promedio:</span>
              <div className="flex items-center gap-2">
                {getChangeIcon(comparison.fps_change_percent)}
                <span className={`text-base font-bold ${getChangeColor(comparison.fps_change_percent)}`}>
                  {comparison.fps_change_percent > 0 ? '+' : ''}{comparison.fps_change_percent.toFixed(1)}%
                </span>
              </div>
            </div>

            <div className="flex items-center justify-between p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <span className="text-xs text-[var(--text-muted)] font-medium">Cambio en 1% Low:</span>
              <div className="flex items-center gap-2">
                {getChangeIcon(comparison.fps_1_percent_change_percent)}
                <span className={`text-base font-bold ${getChangeColor(comparison.fps_1_percent_change_percent)}`}>
                  {comparison.fps_1_percent_change_percent > 0 ? '+' : ''}{comparison.fps_1_percent_change_percent.toFixed(1)}%
                </span>
              </div>
            </div>

            <div className="flex items-center justify-between p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <span className="text-xs text-[var(--text-muted)] font-medium">Cambio en 0.1% Low:</span>
              <div className="flex items-center gap-2">
                {getChangeIcon(comparison.fps_0_1_percent_change_percent)}
                <span className={`text-base font-bold ${getChangeColor(comparison.fps_0_1_percent_change_percent)}`}>
                  {comparison.fps_0_1_percent_change_percent > 0 ? '+' : ''}{comparison.fps_0_1_percent_change_percent.toFixed(1)}%
                </span>
              </div>
            </div>

            <div className="flex items-center justify-between p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
              <span className="text-xs text-[var(--text-muted)] font-medium">Cambio en Frame Time:</span>
              <div className="flex items-center gap-2">
                {comparison.frame_time_change_percent < 0 ? (
                  <TrendingUp className="w-4 h-4 text-[var(--compatible)]" />
                ) : (
                  <AlertCircle className="w-4 h-4 text-[var(--error)]" />
                )}
                <span className={`text-base font-bold ${comparison.frame_time_change_percent < 0 ? 'text-[var(--compatible)]' : 'text-[var(--error)]'}`}>
                  {comparison.frame_time_change_percent > 0 ? '+' : ''}{comparison.frame_time_change_percent.toFixed(1)}%
                </span>
              </div>
            </div>
          </div>

          <button
            onClick={() => setComparison(null)}
            className="w-full px-4 py-2.5 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] hover:border-[var(--action)]/50 transition-all duration-200"
          >
            Cerrar Comparación
          </button>
        </motion.article>
      )}

      {/* Tutorial mejorado */}
      <article className="p-6 rounded-xl bg-gradient-to-br from-[var(--surface)] to-[var(--bg)] border-2 border-[var(--border)]">
        <div className="flex items-center gap-3 mb-5">
          <div className="w-10 h-10 rounded-lg bg-gradient-to-br from-[var(--action)] to-[var(--caution)] flex items-center justify-center">
            <Zap className="w-5 h-5 text-white" />
          </div>
          <div>
            <h3 className="text-lg font-bold text-[var(--text)]">Guía Rápida de Benchmarks</h3>
            <p className="text-xs text-[var(--text-muted)]">Aprende a medir el rendimiento en 4 pasos simples</p>
          </div>
        </div>

        {/* Explicación de Base y Prueba */}
        <div className="mb-6 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
          <h4 className="text-sm font-bold text-[var(--text)] mb-3 flex items-center gap-2">
            <i className="fa-solid fa-circle-question text-[var(--action)]"></i>
            ¿Qué son Base y Prueba?
          </h4>
          <div className="grid grid-cols-2 gap-3">
            <div className="p-3 rounded-lg bg-[var(--surface)] border-2 border-[var(--compatible)]/30">
              <div className="flex items-center gap-2 mb-2">
                <div className="w-8 h-5 rounded-full bg-[var(--compatible)] flex items-center justify-end pr-1">
                  <div className="w-3 h-3 rounded-full bg-white"></div>
                </div>
                <span className="text-sm font-bold text-[var(--compatible)]">Base</span>
              </div>
              <p className="text-xs text-[var(--text-muted)]">
                Tu benchmark de <strong className="text-[var(--text)]">referencia</strong> (antes de hacer cambios). Es el punto de partida para comparar.
              </p>
            </div>
            <div className="p-3 rounded-lg bg-[var(--surface)] border-2 border-[var(--caution)]/30">
              <div className="flex items-center gap-2 mb-2">
                <div className="w-8 h-5 rounded-full bg-[var(--caution)] flex items-center justify-end pr-1">
                  <div className="w-3 h-3 rounded-full bg-white"></div>
                </div>
                <span className="text-sm font-bold text-[var(--caution)]">Prueba</span>
              </div>
              <p className="text-xs text-[var(--text-muted)]">
                Tu benchmark <strong className="text-[var(--text)]">nuevo</strong> (después de activar un parámetro). Se compara con Base para ver mejoras.
              </p>
            </div>
          </div>
        </div>

        {/* Pasos mejorados */}
        <div className="space-y-4">
          <div className="flex gap-4 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors">
            <div className="w-10 h-10 rounded-full bg-gradient-to-br from-[var(--action)] to-[var(--action-hover)] flex items-center justify-center text-sm font-bold text-white shrink-0 shadow-lg shadow-[var(--action)]/20">
              1
            </div>
            <div className="flex-1">
              <p className="font-bold text-[var(--text)] mb-1">Graba tu primer benchmark (sin cambios)</p>
              <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                Juega 30 segundos en tu juego favorito y graba con MangoHud (<kbd className="px-1.5 py-0.5 rounded bg-[var(--surface)] border border-[var(--border)] text-[var(--text)] font-mono text-[10px]">Shift+F2</kbd>).
                Luego analiza el log aquí.
              </p>
            </div>
          </div>

          <div className="flex gap-4 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors">
            <div className="w-10 h-10 rounded-full bg-gradient-to-br from-[var(--action)] to-[var(--action-hover)] flex items-center justify-center text-sm font-bold text-white shrink-0 shadow-lg shadow-[var(--action)]/20">
              2
            </div>
            <div className="flex-1">
              <p className="font-bold text-[var(--text)] mb-1">Activa el toggle <span className="text-[var(--compatible)]">Base</span></p>
              <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                En el benchmark que acabas de crear, activa el toggle verde <strong className="text-[var(--compatible)]">Base</strong>.
                Este será tu punto de referencia.
              </p>
            </div>
          </div>

          <div className="flex gap-4 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors">
            <div className="w-10 h-10 rounded-full bg-gradient-to-br from-[var(--action)] to-[var(--action-hover)] flex items-center justify-center text-sm font-bold text-white shrink-0 shadow-lg shadow-[var(--action)]/20">
              3
            </div>
            <div className="flex-1">
              <p className="font-bold text-[var(--text)] mb-1">Cambia un parámetro y graba otro benchmark</p>
              <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                Activa un parámetro de optimización (ej: <code className="px-1.5 py-0.5 rounded bg-[var(--surface)] border border-[var(--border)] text-[var(--text)] font-mono text-[10px]">game-performance</code>)
                y repite la misma prueba en las mismas condiciones. Analiza el nuevo log.
              </p>
            </div>
          </div>

          <div className="flex gap-4 p-4 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors">
            <div className="w-10 h-10 rounded-full bg-gradient-to-br from-[var(--action)] to-[var(--action-hover)] flex items-center justify-center text-sm font-bold text-white shrink-0 shadow-lg shadow-[var(--action)]/20">
              4
            </div>
            <div className="flex-1">
              <p className="font-bold text-[var(--text)] mb-1">Activa <span className="text-[var(--caution)]">Prueba</span> y compara</p>
              <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                En el nuevo benchmark, activa el toggle amarillo <strong className="text-[var(--caution)]">Prueba</strong>.
                Aparecerá el botón <strong className="text-[var(--action)]">Comparar Benchmarks</strong>. Haz clic para ver las diferencias.
              </p>
              <div className="mt-2 p-2 rounded bg-[var(--caution)]/10 border border-[var(--caution)]/30">
                <p className="text-[10px] text-[var(--caution)] font-semibold">
                  💡 Tip: El 1% low es más importante que los FPS medios para medir fluidez
                </p>
              </div>
            </div>
          </div>
        </div>

        {/* Ejemplo visual */}
        <div className="mt-6 p-4 rounded-lg bg-gradient-to-r from-[var(--compatible)]/10 to-[var(--caution)]/10 border border-[var(--border)]">
          <h4 className="text-sm font-bold text-[var(--text)] mb-3 flex items-center gap-2">
            <i className="fa-solid fa-lightbulb text-[var(--caution)]"></i>
            Ejemplo práctico
          </h4>
          <div className="space-y-2 text-xs text-[var(--text-muted)]">
            <div className="flex items-center gap-2">
              <span className="text-[var(--compatible)] font-bold">✓</span>
              <span>Benchmark 1: Palworld sin optimizar → FPS: 60 → Marcar como <strong className="text-[var(--compatible)]">Base</strong></span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[var(--compatible)] font-bold">✓</span>
              <span>Activar <code className="px-1 py-0.5 rounded bg-[var(--surface)] border border-[var(--border)] font-mono text-[10px]">game-performance</code></span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[var(--compatible)] font-bold">✓</span>
              <span>Benchmark 2: Palworld con game-performance → FPS: 70 → Marcar como <strong className="text-[var(--caution)]">Prueba</strong></span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-[var(--compatible)] font-bold">✓</span>
              <span>Clic en <strong className="text-[var(--action)]">Comparar</strong> → Resultado: <strong className="text-[var(--compatible)]">+16.7% FPS</strong> 🎉</span>
            </div>
          </div>
        </div>
      </article>
    </div>
  );
}
