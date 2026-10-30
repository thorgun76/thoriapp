import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import {
  TrendingUp, BarChart3, Award, Calendar,
  Gamepad2, Target
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

interface DashboardStats {
  total_tests: number;
  total_games: number;
  last_test_date: string | null;
  best_improvements: Array<{
    game_name: string;
    improvement_percent: number;
    parameter: string;
    date: string;
  }>;
  recent_tests: Array<{
    game_name: string;
    baseline_fps: number;
    optimized_fps: number;
    improvement_percent: number;
    date: string;
  }>;
}

export function DashboardView() {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadDashboardStats();
  }, []);

  const loadDashboardStats = async () => {
    try {
      const allBenchmarks = await invoke<Benchmark[]>('load_benchmarks');

      // Calcular estadísticas
      const totalTests = allBenchmarks.length;
      const uniqueGames = new Set(allBenchmarks.map(b => b.appid));
      const totalGames = uniqueGames.size;

      // Última prueba
      const sortedByDate = [...allBenchmarks].sort((a, b) =>
        new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
      );
      const lastTestDate = sortedByDate.length > 0 ? sortedByDate[0].timestamp : null;

      // Agrupar benchmarks por juego
      const benchmarksByGame: Record<number, Benchmark[]> = {};
      allBenchmarks.forEach(b => {
        if (!benchmarksByGame[b.appid]) {
          benchmarksByGame[b.appid] = [];
        }
        benchmarksByGame[b.appid].push(b);
      });

      // Calcular mejores mejoras
      const improvements: Array<{
        game_name: string;
        improvement_percent: number;
        parameter: string;
        date: string;
      }> = [];

      Object.values(benchmarksByGame).forEach(gameBenchmarks => {
        if (gameBenchmarks.length >= 2) {
          // Ordenar por fecha
          const sorted = [...gameBenchmarks].sort((a, b) =>
            new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
          );

          // Comparar el primero (base) con el último (optimizado)
          const baseline = sorted[0];
          const optimized = sorted[sorted.length - 1];

          const improvementPercent = ((optimized.avg_fps - baseline.avg_fps) / baseline.avg_fps) * 100;

          if (improvementPercent > 0) {
            // Extraer parámetro del comando de lanzamiento
            const params = optimized.launch_command.split(' ').filter(p => p !== '%command%' && p.includes('='));
            const parameter = params.length > 0 ? params[params.length - 1] : 'Configuración optimizada';

            improvements.push({
              game_name: baseline.game_name,
              improvement_percent: improvementPercent,
              parameter: parameter,
              date: optimized.timestamp,
            });
          }
        }
      });

      // Ordenar mejoras y tomar las top 5
      const bestImprovements = improvements
        .sort((a, b) => b.improvement_percent - a.improvement_percent)
        .slice(0, 5);

      // Pruebas recientes (últimas 10)
      const recentTests = sortedByDate.slice(0, 10).map(benchmark => {
        const gameBenchmarks = benchmarksByGame[benchmark.appid];
        const baseline = gameBenchmarks.reduce((min, b) =>
          b.avg_fps < min.avg_fps ? b : min
        , gameBenchmarks[0]);

        const improvementPercent = ((benchmark.avg_fps - baseline.avg_fps) / baseline.avg_fps) * 100;

        return {
          game_name: benchmark.game_name,
          baseline_fps: baseline.avg_fps,
          optimized_fps: benchmark.avg_fps,
          improvement_percent: improvementPercent,
          date: benchmark.timestamp,
        };
      });

      setStats({
        total_tests: totalTests,
        total_games: totalGames,
        last_test_date: lastTestDate,
        best_improvements: bestImprovements,
        recent_tests: recentTests,
      });
    } catch (e) {
      console.error('Error al cargar estadísticas:', e);
    } finally {
      setLoading(false);
    }
  };

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffHours < 1) return 'Hace menos de 1 hora';
    if (diffHours < 24) return `Hace ${diffHours} hora${diffHours > 1 ? 's' : ''}`;
    if (diffDays < 7) return `Hace ${diffDays} día${diffDays > 1 ? 's' : ''}`;
    return date.toLocaleDateString('es-ES');
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <div className="w-12 h-12 border-4 border-[var(--action)]/30 border-t-[var(--action)] rounded-full animate-spin mx-auto mb-4"></div>
          <p className="text-[var(--text-muted)]">Cargando estadísticas...</p>
        </div>
      </div>
    );
  }

  if (!stats || stats.total_tests === 0) {
    return (
      <div className="max-w-4xl mx-auto p-8">
        <div className="text-center py-16">
          <BarChart3 className="w-16 h-16 text-[var(--text-dim)] mx-auto mb-4" />
          <h2 className="text-2xl font-bold text-[var(--text)] mb-2">
            No hay pruebas de rendimiento aún
          </h2>
          <p className="text-[var(--text-muted)] mb-6">
            Comienza a probar diferentes configuraciones de juegos para ver tus estadísticas aquí
          </p>
          <div className="text-sm text-[var(--text-dim)]">
            <p>💡 Consejo: Ve a la pestaña "Benchmarks" para comenzar a medir el rendimiento</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-6xl mx-auto p-8 space-y-6">
      {/* Header */}
      <div>
        <p className="text-xs text-[var(--text-muted)] mb-2">Dashboard</p>
        <h2 className="text-2xl font-bold">Mis pruebas de rendimiento</h2>
      </div>

      {/* Estadísticas generales */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
          className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
        >
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-lg bg-[var(--action)]/10 flex items-center justify-center">
              <Target className="w-5 h-5 text-[var(--action)]" />
            </div>
            <div>
              <p className="text-xs text-[var(--text-muted)]">Total de pruebas</p>
              <p className="text-2xl font-bold text-[var(--text)]">{stats.total_tests}</p>
            </div>
          </div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.2 }}
          className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
        >
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-lg bg-[var(--compatible)]/10 flex items-center justify-center">
              <Gamepad2 className="w-5 h-5 text-[var(--compatible)]" />
            </div>
            <div>
              <p className="text-xs text-[var(--text-muted)]">Juegos probados</p>
              <p className="text-2xl font-bold text-[var(--text)]">{stats.total_games}</p>
            </div>
          </div>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.3 }}
          className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
        >
          <div className="flex items-center gap-3 mb-3">
            <div className="w-10 h-10 rounded-lg bg-[var(--caution)]/10 flex items-center justify-center">
              <Calendar className="w-5 h-5 text-[var(--caution)]" />
            </div>
            <div>
              <p className="text-xs text-[var(--text-muted)]">Última prueba</p>
              <p className="text-lg font-bold text-[var(--text)]">
                {stats.last_test_date ? formatDate(stats.last_test_date) : 'N/A'}
              </p>
            </div>
          </div>
        </motion.div>
      </div>

      {/* Mejores mejoras */}
      {stats.best_improvements.length > 0 && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.4 }}
          className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
        >
          <h3 className="text-base font-bold mb-4 flex items-center gap-2 text-[var(--text)]">
            <Award className="w-5 h-5 text-[var(--caution)]" />
            Mejores mejoras descubiertas
          </h3>
          <div className="space-y-3">
            {stats.best_improvements.map((improvement, index) => (
              <div
                key={index}
                className="flex items-center justify-between p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]"
              >
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 rounded-full bg-[var(--caution)]/20 text-[var(--caution)] flex items-center justify-center text-sm font-bold">
                    {index + 1}
                  </div>
                  <div>
                    <p className="text-sm font-semibold text-[var(--text)]">{improvement.game_name}</p>
                    <p className="text-xs text-[var(--text-muted)]">{improvement.parameter}</p>
                  </div>
                </div>
                <div className="text-right">
                  <p className="text-lg font-bold text-[var(--compatible)]">
                    +{improvement.improvement_percent.toFixed(1)}%
                  </p>
                  <p className="text-xs text-[var(--text-dim)]">{formatDate(improvement.date)}</p>
                </div>
              </div>
            ))}
          </div>
        </motion.div>
      )}

      {/* Pruebas recientes */}
      {stats.recent_tests.length > 0 && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.5 }}
          className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]"
        >
          <h3 className="text-base font-bold mb-4 flex items-center gap-2 text-[var(--text)]">
            <TrendingUp className="w-5 h-5 text-[var(--action)]" />
            Pruebas recientes
          </h3>
          <div className="space-y-2">
            {stats.recent_tests.map((test, index) => (
              <div
                key={index}
                className="flex items-center justify-between p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors"
              >
                <div className="flex items-center gap-3">
                  <Gamepad2 className="w-5 h-5 text-[var(--text-muted)]" />
                  <div>
                    <p className="text-sm font-semibold text-[var(--text)]">{test.game_name}</p>
                    <p className="text-xs text-[var(--text-muted)]">
                      {test.baseline_fps.toFixed(1)} FPS → {test.optimized_fps.toFixed(1)} FPS
                    </p>
                  </div>
                </div>
                <div className="text-right">
                  <p className={`text-sm font-bold ${
                    test.improvement_percent > 0
                      ? 'text-[var(--compatible)]'
                      : test.improvement_percent < 0
                        ? 'text-[var(--error)]'
                        : 'text-[var(--text-muted)]'
                  }`}>
                    {test.improvement_percent > 0 ? '+' : ''}{test.improvement_percent.toFixed(1)}%
                  </p>
                  <p className="text-xs text-[var(--text-dim)]">{formatDate(test.date)}</p>
                </div>
              </div>
            ))}
          </div>
        </motion.div>
      )}
    </div>
  );
}
