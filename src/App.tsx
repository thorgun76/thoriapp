import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Copy, Star } from 'lucide-react';
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
  DragStartEvent,
  DragOverlay,
} from '@dnd-kit/core';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { BenchmarksView } from './components/BenchmarksView';
import { DashboardView } from './components/DashboardView';
import { Sidebar } from './components/Sidebar';
import { LibraryView } from './components/LibraryView';
import { SaveProfileModal } from './components/SaveProfileModal';
import { SortableProfile } from './components/SortableProfile';
import { CommandBox } from './components/CommandBox';
import { GameSidebar } from './components/GameSidebar';
import { Topbar } from './components/Topbar';
import { CompatTab } from './components/CompatTab';
import { OptimizeTab } from './components/OptimizeTab';
import { api } from './lib/api';
import type {
  Game,
  LauncherFilter,
  Param,
  Profile,
  SidebarView,
  View,
} from './lib/types';
import { useHardware } from './hooks/useHardware';
import { useLibrary } from './hooks/useLibrary';
import { useProfiles } from './hooks/useProfiles';
import { useGameDetails } from './hooks/useGameDetails';
import { useRuntimeEnv } from './hooks/useRuntimeEnv';
import { useConfirm, useToast } from './hooks/useConfirm';
import { ConfirmDialog } from './components/ConfirmDialog';
import { missingReason, recommendationStatus } from './lib/recommendations';
import { classifyParameter, joinCommand, type CommandParts } from './lib/command';
import { matchesSearch } from './lib/format';
import { deniedReason, isDenied } from './lib/anticheat';

// Abre el diálogo de archivo, lee el JSON y se lo pasa al backend.
// (El navegador no expone la ruta real del archivo: hay que leer el contenido.)
const pickProfileFile = (onContent: (content: string) => void) => {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.json';
  input.onchange = async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (file) onContent(await file.text());
  };
  input.click();
};

// ==================== COMPONENTE PRINCIPAL ====================
function App() {
  // Estados principales
  const [sidebarView, setSidebarView] = useState<SidebarView>('library');
  const [view, setView] = useState<View>('recommendations');
  // Hardware: detectado una sola vez por su propio hook (ver src/hooks/useHardware.ts)
  const { hw } = useHardware();
  // Estado real de la maquina (wrappers instalados, capa Vulkan, Flatpak...) para
  // avisar antes de que el comando falle al lanzar el juego.
  const runtimeEnv = useRuntimeEnv();
  const [showExternalGames, setShowExternalGames] = useState(true);
  // Juego abierto: reglas, ProtonDB, motor y recomendaciones (useGameDetails)
  const {
    current,
    rules,
    info,
    antiCheat,
    requirements,
    requirementsLoading,
    requirementsError,
    protonPulse,
    protonPulseLoading,
    engine,
    engineRecommendations,
    communityRecommendations,
    loading: gameLoading,
    error: gameError,
    openGame: loadGameDetails,
    closeGame,
  } = useGameDetails({ getHwTags: () => hw?.tags || [], getGpuName: () => hw?.gpu || null });

  // Estados de selección
  const [enabled, setEnabled] = useState<boolean[]>([]);
  const [selectedCommunityRecs, setSelectedCommunityRecs] = useState<Set<number>>(new Set());
  const [selectedEngineRecs, setSelectedEngineRecs] = useState<Set<number>>(new Set());

  // Estados de UI
  const [copied, setCopied] = useState(false);
  const [expertMode, setExpertMode] = useState(false);
  // showUnavailable siempre es false (no hay control en la UI para cambiarlo)
  const showUnavailable = false;
  const { toast, setToast } = useToast(3500);
  const { confirmState, ask, close } = useConfirm();
  const [launcherFilter, setLauncherFilter] = useState<LauncherFilter>('all');

  // Biblioteca + busqueda: escaneo de Steam/launchers y debounce del buscador
  const {
    library,
    externalGames,
    hiddenDuplicates,
    heroniteUninstalled,
    scanningLaunchers,
    query,
    suggestions,
    showSug,
    setShowSug,
    clearSearch,
    handleQueryChange,
    loadInitial,
    rescanExternal,
  } = useLibrary({ getHwTags: () => hw?.tags || [], onNotify: setToast });

  // Perfiles: carga, comandos reconstruidos, guardar/borrar/reordenar/import-export
  const {
    profiles,
    profileCommands,
    loadAll: loadAllProfiles,
    copyCommand: copyProfileCommand,
    save: saveProfileInStore,
    remove: removeProfile,
    reorder: reorderProfiles,
    importFromText: importProfilesFromText,
    exportAll: exportAllProfiles,
    exportOne: exportProfile,
  } = useProfiles({
    getHwTags: () => hw?.tags || [],
    getGpuName: () => hw?.gpu || null,
    onNotify: setToast,
  });

  // Fase 3: Toggle para forzar Proton en juegos nativos
  const [forceProton, setForceProton] = useState(false);

  // Fase 4: Editor manual del comando
  const [manualCommand, setManualCommand] = useState<string>('');
  const [useManualCommand, setUseManualCommand] = useState(false);

  // Fase 5: Perfiles (estado y comandos los gestiona useProfiles)
  // Número de pruebas guardadas (se muestra junto a "Mis pruebas" en el menú)
  const [savedTestsCount, setSavedTestsCount] = useState(0);
  useEffect(() => {
    const refresh = async () => {
      try {
        const list = await api.loadComparisons();
        setSavedTestsCount(list.length);
      } catch (e) {
        console.error('Error al contar las pruebas guardadas:', e);
      }
    };
    refresh();
    window.addEventListener('thoriapp:comparisons-changed', refresh);
    return () => window.removeEventListener('thoriapp:comparisons-changed', refresh);
  }, []);
  const [expandedProfile, setExpandedProfile] = useState<string | null>(null);
  const [showSaveProfileModal, setShowSaveProfileModal] = useState(false);
  const [newProfileName, setNewProfileName] = useState('');
  const [newProfileNotes, setNewProfileNotes] = useState('');

  // Fase 6: Benchmarks
  const [activeId, setActiveId] = useState<string | null>(null);
  const [mangohudLogPath, setMangohudLogPath] = useState<string>(() => {
    // Cargar desde localStorage o detectar automáticamente
    const saved = localStorage.getItem('mangohud_log_path');
    if (saved) return saved;

    // Detectar automáticamente desde la configuración de MangoHud
    try {
      const homeDir = localStorage.getItem('home_dir') || '';
      const possiblePaths = [
        `${homeDir}/.local/share/goverlay/logs`,
        `${homeDir}/.local/share/MangoHud/logs`,
        '/tmp/MangoHud',
      ];

      for (const path of possiblePaths) {
        if (path) return path;
      }
    } catch (e) {
      console.error('Error detectando ruta de MangoHud:', e);
    }

    return '/tmp/MangoHud'; // Fallback
  });


  // Sensores para drag & drop
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 5,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  // Función para manejar el inicio del arrastre
  const handleDragStart = (event: DragStartEvent) => {
    setActiveId(event.active.id as string);
  };

  // Función para manejar el final del arrastre
  const handleDragEnd = async (event: DragEndEvent) => {
    setActiveId(null);
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const oldIndex = profiles.findIndex((p) => p.id === active.id);
    const newIndex = profiles.findIndex((p) => p.id === over.id);
    await reorderProfiles(arrayMove(profiles, oldIndex, newIndex));
  };

  // Función para cancelar el arrastre
  const handleDragCancel = () => {
    setActiveId(null);
  };

  // Perfil activo (el que se está arrastrando)
  const activeProfile = activeId ? profiles.find(p => p.id === activeId) : null;

  // ==================== EFECTOS ====================
  useEffect(() => { loadInitial(); }, [loadInitial]);

  // Los comandos de los perfiles dependen de los tags de hardware: si el
  // hardware llega DESPUÉS de la primera carga, hay que reconstruirlos.
  useEffect(() => {
    if (hw?.tags?.length) loadAllProfiles();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hw?.tags?.join(',') ?? '']);

  // Detectar automáticamente la ruta de logs de MangoHud
  useEffect(() => {
    const detectLogPath = async () => {
      try {
        const detectedPath = await api.detectMangohudLogPath();
        setMangohudLogPath(detectedPath);
        localStorage.setItem('mangohud_log_path', detectedPath);
      } catch (e) {
        console.error('Error detectando ruta de MangoHud:', e);
      }
    };

    detectLogPath();
  }, []);

  // Al cargar las reglas de un juego hay que dimensionar `enabled` a sus
  // parámetros. Si no, `toggleRule` escribe en un array corto y deja huecos,
  // que al serializar se convierten en `null` y Rust rechaza el perfil con
  // «invalid type: null, expected a boolean».
  useEffect(() => {
    if (!rules) return;
    setEnabled((prev) =>
      prev.length === rules.params.length
        ? prev
        : new Array(rules.params.length).fill(false)
    );
  }, [rules]);

  // openGame delega en useGameDetails y reinicia la selección de esta vista
  const openGame = async (g: Game) => {
    setEnabled([]);
    setSelectedCommunityRecs(new Set());
    setSelectedEngineRecs(new Set());
    setForceProton(false);
    await loadGameDetails(g);
  };

  const getStatusOptions = () => ({ hwTags: hw?.tags || [], forceProton });

  // Anti-cheat denegado = sin comando. Se calcula aquí y no dentro de `launchLine`
  // para que la caja del comando, el portapapeles, el guardado de perfil y la
  // pestaña de benchmarks usen el MISMO criterio.
  const blockedByAnticheat = isDenied(antiCheat);
  const blockedMessage = blockedByAnticheat ? deniedReason(antiCheat) : '';

  const missing = (p: Param): string | null =>
    missingReason(p, { isNative: info?.native === true, ...getStatusOptions() });

  const getRecommendationStatus = (p: Param): string =>
    recommendationStatus(p, getStatusOptions());

  const toggleRule = (index: number) => {
    if (!rules) return;
    const param = rules.params[index];
    if (missing(param)) return;

    // Se parte siempre de un array del tamaño de `params` para que no queden
    // huecos: los huecos se serializan como `null` y Rust espera booleanos.
    const newEnabled = Array.from(
      { length: rules.params.length },
      (_, i) => enabled[i] === true
    );
    newEnabled[index] = !newEnabled[index];
    setEnabled(newEnabled);
  };

  const toggleCommunityRec = (index: number) => {
    const newSet = new Set(selectedCommunityRecs);
    if (newSet.has(index)) {
      newSet.delete(index);
    } else {
      newSet.add(index);
    }
    setSelectedCommunityRecs(newSet);
  };

  const toggleEngineRec = (index: number) => {
    const newSet = new Set(selectedEngineRecs);
    if (newSet.has(index)) {
      newSet.delete(index);
    } else {
      newSet.add(index);
    }
    setSelectedEngineRecs(newSet);
  };

  /**
   * Piezas del comando, en el orden en que se montan. Devuelve también las del
   * modo manual no: ahí el texto lo pone el usuario y no se puede trocear sin
   * inventarse qué es cada cosa.
   */
  const commandParts = (): CommandParts => {
    const parts: CommandParts = { envVars: [], wrappers: [], args: [] };
    if (blockedByAnticheat || !rules) return parts;

    rules.params.forEach((p, i) => {
      if (enabled[i] !== true) return;
      if (p.kind === 'env') parts.envVars.push(p.value);
      else if (p.kind === 'wrapper') parts.wrappers.push(p.value);
      // `kind: "config"` no es un parámetro de lanzador: es un ajuste dentro del
      // juego ("usa FSR en Calidad"). Algunos textos llevan espacios, así que sin
      // este filtro acabaría dentro del comando y lo rompería.
      else if (p.kind === 'arg') parts.args.push(p.value);
    });

    communityRecommendations.forEach((rec, i) => {
      if (!selectedCommunityRecs.has(i)) return;
      const kind = classifyParameter(rec.parameter);
      if (kind === 'env') parts.envVars.push(rec.parameter);
      else if (kind === 'wrapper') parts.wrappers.push(rec.parameter);
      else parts.args.push(rec.parameter);
    });

    engineRecommendations.forEach((rec, i) => {
      if (!selectedEngineRecs.has(i)) return;
      parts.envVars.push(rec);
    });

    return parts;
  };

  /** Resumen visual del comando: se deriva de las mismas piezas, no de un cálculo aparte. */
  const activeParts = (): { value: string; kind: 'env' | 'wrapper' | 'arg' }[] => {
    if (useManualCommand) return [];
    const { envVars, wrappers, args } = commandParts();
    return [
      ...wrappers.map((value) => ({ value, kind: 'wrapper' as const })),
      ...envVars.map((value) => ({ value, kind: 'env' as const })),
      ...args.map((value) => ({ value, kind: 'arg' as const })),
    ];
  };

  const launchLine = (): string => {
    // Puerta dura del anti-cheat: si el juego está `Denied`, no hay comando. Antes
    // se avisaba en la pestaña Compatibilidad y aun así se generaba uno, que es
    // cómo un usuario acaba baneado siguiendo un consejo de la app.
    if (blockedByAnticheat) return blockedMessage;

    if (useManualCommand && manualCommand.trim()) {
      return manualCommand.trim();
    }
    if (!rules) return '%command%';

    const { envVars, wrappers, args } = commandParts();
    if (envVars.length === 0 && wrappers.length === 0 && args.length === 0) {
      return '%command%';
    }
    return joinCommand({ envVars, wrappers, args });
  };

  const copyCmd = async () => {
    if (blockedByAnticheat) {
      setToast('Comando no generado: anti-cheat denegado');
      return;
    }
    try {
      await navigator.clipboard.writeText(launchLine());
      setCopied(true);
      setToast('Comando copiado: ' + launchLine());
      setTimeout(() => setCopied(false), 2000);
    } catch (e) { console.error(e); }
  };

  /**
   * Copia las opciones de lanzamiento de un reporte de Proton Pulse. Es el texto de
   * otra persona: se copia tal cual y se dice de dónde sale, sin fusionarlo con el
   * comando actual (mezclar dos `%command%` produce un comando que no arranca).
   */
  const copyLaunchOptions = async (options: string) => {
    try {
      await navigator.clipboard.writeText(options);
      setToast('Opciones de otro usuario copiadas: ' + options);
    } catch (e) { console.error(e); }
  };

  const impactColor = (impact: string) => {
    const colors: Record<string, string> = {
      alto: 'text-green-400 bg-green-500/10 border-green-500/30',
      medio: 'text-blue-400 bg-blue-500/10 border-blue-500/30',
      bajo: 'text-gray-400 bg-gray-500/10 border-gray-500/30',
      medicion: 'text-cyan-400 bg-cyan-500/10 border-cyan-500/30',
      dudoso: 'text-orange-400 bg-orange-500/10 border-orange-500/30',
      evitar: 'text-red-400 bg-red-500/10 border-red-500/30'
    };
    return colors[impact] || colors.bajo;
  };

  const evidenceLabel = (evidence: string) => {
    const labels: Record<string, string> = {
      medido: '✓ Medido',
      documentado: '📄 Documentado',
      heuristica: '⚠ Heurística',
      reportado: '📊 Reportado'
    };
    return labels[evidence] || labels.heuristica;
  };

  // NUEVO: Contadores por launcher (ANTES de filtrar para que no cambien)
  const launcherCounts = {
    steam: library.length,
    lutris: externalGames.filter(g => g.launcher === 'lutris').length,
    heroic: externalGames.filter(g => g.launcher === 'heroic').length,
    heronite: externalGames.filter(g => g.launcher === 'heronite').length,
  };

  // NUEVO: Filtrar por launcher
  const filteredLib = library.filter(g => {
    const matchesQuery = matchesSearch(g.name, query);
    const matchesLauncher = launcherFilter === 'all' || launcherFilter === 'steam';
    return matchesQuery && matchesLauncher;
  });

  const filteredExternalGames = externalGames.filter(g => {
    const matchesQuery = matchesSearch(g.name, query);
    const matchesLauncher = launcherFilter === 'all' || g.launcher === launcherFilter;
    return matchesQuery && matchesLauncher;
  });

  const combinedLibrary = showExternalGames
    ? [...filteredLib, ...filteredExternalGames]
    : filteredLib;

  // Total real de la biblioteca (sin filtrar), para el contador de arriba.
  const libraryTotal = showExternalGames ? library.length + externalGames.length : library.length;

  const saveProfile = async () => {
    if (!current || !newProfileName.trim()) return;
    // Guardar un perfil de un juego denegado equivale a guardar el comando: no se
    // permite, para que la puerta no se pueda saltar por la puerta de atrás.
    if (blockedByAnticheat) {
      setToast('No se guardan perfiles de juegos bloqueados por el anti-cheat');
      return;
    }
    const ok = await saveProfileInStore(
      {
        name: newProfileName,
        appid: current.appid,
        gameName: current.name,
        // Array plano y completo del tamaño de params: cualquier hueco se
        // serializaría como `null` y Rust rechazaría el perfil.
        enabledParams: rules
          ? rules.params.map((_, i) => enabled[i] === true)
          : [],
        selectedCommunityRecs: Array.from(selectedCommunityRecs),
        selectedEngineRecs: Array.from(selectedEngineRecs),
        forceProton,
        expertMode,
        manualCommand: manualCommand || null,
        useManualCommand,
        notes: newProfileNotes || null,
      },
      newProfileName
    );
    if (ok) {
      setShowSaveProfileModal(false);
      setNewProfileName('');
      setNewProfileNotes('');
    }
  };

  const loadProfile = (profile: Profile) => copyProfileCommand(profile);

  const deleteProfile = (profileId: string) => removeProfile(profileId);

  // ==================== RENDER ====================
  return (
    <div className="h-screen flex bg-[var(--bg)] text-[var(--text)] overflow-hidden">
      <Sidebar
        sidebarView={sidebarView}
        hasCurrentGame={!!current}
        savedTestsCount={savedTestsCount}
        profilesCount={profiles.length}
        expertMode={expertMode}
        distro={hw?.distro}
        onNavigate={(v) => { setSidebarView(v); closeGame(); }}
        onToggleExpertMode={() => setExpertMode(!expertMode)}
      />

      {/* MAIN CONTENT */}
      <main className="flex-1 flex flex-col overflow-hidden">
        <Topbar
          query={query}
          suggestions={suggestions}
          showSuggestions={showSug}
          gameCount={libraryTotal}
          matchedCount={combinedLibrary.length}
          scanningLaunchers={scanningLaunchers}
          hardware={hw}
          onQueryChange={handleQueryChange}
          onFocus={() => suggestions.length > 0 && setShowSug(true)}
          onBlur={() => setTimeout(() => setShowSug(false), 200)}
          onPickSuggestion={(g) => { openGame(g); clearSearch(); }}
        />

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          <AnimatePresence mode="wait">
            {/* LIBRARY VIEW */}
            {sidebarView === 'library' && !current && (
              <motion.div key="library" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <LibraryView
                  games={combinedLibrary}
                  counts={launcherCounts}
                  launcherFilter={launcherFilter}
                  externalCount={externalGames.length}
                  hiddenDuplicates={hiddenDuplicates}
                  heroniteUninstalled={heroniteUninstalled}
                  showExternalGames={showExternalGames}
                  scanningLaunchers={scanningLaunchers}
                  onFilterChange={setLauncherFilter}
                  onToggleExternal={() => setShowExternalGames(!showExternalGames)}
                  onRescan={rescanExternal}
                  onOpenGame={openGame}
                />
              </motion.div>
            )}

            {/* Cargando el juego (sobre todo los que vienen del buscador) */}
            {sidebarView === 'library' && current && gameLoading && (
              <motion.div
                key="game-loading"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="p-8 flex items-center justify-center h-64"
              >
                <div className="text-center">
                  <div className="w-12 h-12 border-4 border-[var(--action)]/30 border-t-[var(--action)] rounded-full animate-spin mx-auto mb-4"></div>
                  <p className="text-[var(--text-muted)]">Cargando {current.name}…</p>
                </div>
              </motion.div>
            )}

            {/* No se pudieron cargar las reglas: se avisa y se ofrece volver */}
            {sidebarView === 'library' && current && !gameLoading && gameError && (
              <motion.div
                key="game-error"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="p-8"
              >
                <div className="max-w-lg mx-auto p-6 rounded-xl bg-[var(--error)]/10 border-2 border-[var(--error)]/40">
                  <h3 className="text-base font-bold text-[var(--error)] mb-2">
                    No se pudo abrir {current.name}
                  </h3>
                  <p className="text-sm text-[var(--text-muted)] mb-4">{gameError}</p>
                  <button
                    onClick={closeGame}
                    className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
                  >
                    Volver a la biblioteca
                  </button>
                </div>
              </motion.div>
            )}

            {/* GAME VIEW */}
            {sidebarView === 'library' && current && !gameLoading && !gameError && rules && info && (
              <motion.div key="game" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <div className="max-w-6xl mx-auto">
                  {/* Breadcrumb */}
                  <p className="text-xs text-[var(--text-muted)] mb-4">
                    Biblioteca <span className="mx-2">/</span> <strong className="text-white">{current.name}</strong>
                  </p>

                  <CommandBox
                    gameName={current.name}
                    launchCommand={launchLine()}
                    activeParts={activeParts()}
                    // Bloqueado por anti-cheat: se oculta cualquier opción activa
                    // para no sugerir que el comando existe.
                    activeCount={
                      blockedByAnticheat
                        ? 0
                        : enabled.filter(Boolean).length +
                          selectedCommunityRecs.size +
                          selectedEngineRecs.size
                    }
                    blocked={blockedByAnticheat}
                    blockedReason={blockedMessage}
                    copied={copied}
                    expertMode={expertMode}
                    manualEditing={useManualCommand}
                    manualCommand={manualCommand}
                    info={info}
                    env={runtimeEnv}
                    launcher={current.launcher}
                    onCopy={copyCmd}
                    onSaveProfile={() => setShowSaveProfileModal(true)}
                    onReset={() => {
                      setEnabled(new Array(rules.params.length).fill(false));
                      setSelectedCommunityRecs(new Set());
                      setSelectedEngineRecs(new Set());
                      setToast('Opciones restablecidas al comando base');
                    }}
                    onToggleManualEditing={() => {
                      setUseManualCommand(!useManualCommand);
                      if (!useManualCommand) setManualCommand(launchLine());
                    }}
                    onManualCommandChange={setManualCommand}
                  />

                  {/* TABS: Compatibilidad, Optimización */}
                  <div className="mb-6 border-b border-[var(--border)]">
                    <div className="flex gap-1">
                      <button
                        onClick={() => setView('compatibility')}
                        className={`px-5 py-3 text-sm font-semibold transition-colors border-b-2 ${
                          view === 'compatibility'
                            ? 'text-[var(--action)] border-[var(--action)]'
                            : 'text-[var(--text-muted)] border-transparent hover:text-[var(--text)]'
                        }`}
                      >
                        Compatibilidad
                      </button>
                      <button
                        onClick={() => setView('recommendations')}
                        className={`px-5 py-3 text-sm font-semibold transition-colors border-b-2 ${
                          view === 'recommendations'
                            ? 'text-[var(--action)] border-[var(--action)]'
                            : 'text-[var(--text-muted)] border-transparent hover:text-[var(--text)]'
                        }`}
                      >
                        Optimización
                      </button>
                    </div>
                  </div>

                  {/* Hero info (compacta) */}
                  <section className="mb-6 p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                    <div className="flex items-start justify-between">
                      <div>
                        <p className="text-xs text-[var(--text-muted)] uppercase tracking-wider mb-1">Perfil de optimización · Linux</p>
                        <p className="text-sm text-[var(--text)] mb-2">{rules.summary}</p>
                        <div className="flex flex-wrap gap-2">
                          {engine && <span className="text-xs px-2 py-1 rounded bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">{engine.name}</span>}
                          <span className="text-xs px-2 py-1 rounded bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">Steam AppID {current.appid}</span>
                          {info.total && <span className="text-xs px-2 py-1 rounded bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">{info.total} reportes</span>}
                        </div>
                      </div>
                    </div>
                  </section>

                  {/* Content según tab seleccionado */}
                  <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                    {/* Main Column */}
                    <div className="lg:col-span-2">
                      {/* TAB: COMPATIBILIDAD */}
                      {view === 'compatibility' && (
                        <CompatTab
                          current={current}
                          rules={rules}
                          info={info}
                          antiCheat={antiCheat}
                          protonPulse={protonPulse}
                          protonPulseLoading={protonPulseLoading}
                          onCopyLaunchOptions={copyLaunchOptions}
                          engine={engine}
                          requirements={requirements}
                          requirementsLoading={requirementsLoading}
                          requirementsError={requirementsError}
                        />
                      )}

                      {/* TAB: OPTIMIZACIÓN */}
                      {view === 'recommendations' && (
                        <OptimizeTab
                          rules={rules}
                          info={info}
                          engine={engine}
                          engineRecommendations={engineRecommendations}
                          communityRecommendations={communityRecommendations}
                          enabled={enabled}
                          selectedCommunityRecs={selectedCommunityRecs}
                          selectedEngineRecs={selectedEngineRecs}
                          expertMode={expertMode}
                          forceProton={forceProton}
                          showUnavailable={showUnavailable}
                          onToggleRule={toggleRule}
                          onToggleCommunityRec={toggleCommunityRec}
                          onToggleEngineRec={toggleEngineRec}
                          onToggleProton={() => setForceProton(!forceProton)}
                          missing={missing}
                          getRecommendationStatus={getRecommendationStatus}
                          impactColor={impactColor}
                          evidenceLabel={evidenceLabel}
                        />
                      )}

                    </div>

                    {/* Sidebar Column */}
                    <GameSidebar
                      hardware={hw}
                      onAdjustOptimization={() => {
                        setView('recommendations');
                        setToast('Selecciona ajustes en la pestaña Optimización');
                      }}
                      onMeasurePerformance={() => {
                        setSidebarView('benchmarks');
                        setToast('Graba con MangoHud y analiza aquí el resultado');
                      }}
                    />
                  </div>
                </div>
              </motion.div>
            )}

            {/* PROFILES VIEW */}
            {sidebarView === 'profiles' && (
              <motion.div key="profiles" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <div className="max-w-4xl mx-auto">
                  <div className="flex items-center justify-between mb-6">
                    <div>
                      <p className="text-xs text-[var(--text-muted)] mb-2">Perfiles</p>
                      <h2 className="text-2xl font-bold">Tus configuraciones guardadas</h2>
                    </div>
                    <div className="flex gap-2">
                      <button
                        onClick={() => pickProfileFile(importProfilesFromText)}
                        className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors flex items-center gap-2"
                      >
                        <Star className="w-4 h-4" />
                        Importar perfil
                      </button>
                      <button
                        onClick={() =>
                          ask({
                            title: 'Exportar todos los perfiles',
                            message: 'Escribe la ruta del archivo .json donde se guardarán.',
                            confirmLabel: 'Exportar',
                            tone: 'primary',
                            input: {
                              label: 'Ruta del archivo',
                              defaultValue: '~/thoriapp_perfiles_backup.json',
                              onSubmit: exportAllProfiles,
                            },
                          })
                        }
                        className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors flex items-center gap-2"
                      >
                        <Copy className="w-4 h-4" />
                        Exportar todos
                      </button>
                    </div>
                  </div>

                  {profiles.length === 0 ? (
                    <div className="text-center py-16">
                      <Star className="w-12 h-12 text-[var(--text-dim)] mx-auto mb-4" />
                      <p className="text-[var(--text-muted)] mb-2">No tienes perfiles guardados</p>
                      <p className="text-xs text-[var(--text-dim)]">Configura un juego y guarda la configuración como perfil</p>
                    </div>
                  ) : (
                    <DndContext
                      sensors={sensors}
                      collisionDetection={closestCenter}
                      onDragStart={handleDragStart}
                      onDragEnd={handleDragEnd}
                      onDragCancel={handleDragCancel}
                    >
                      <SortableContext
                        items={profiles.map(p => p.id)}
                        strategy={verticalListSortingStrategy}
                      >
                        <div className="space-y-2">
                          {profiles.map((profile) => (
                            <SortableProfile
                              key={profile.id}
                              profile={profile}
                              isExpanded={expandedProfile === profile.id}
                              onToggle={() => setExpandedProfile(expandedProfile === profile.id ? null : profile.id)}
                              onCopy={() => loadProfile(profile)}
                              onExport={() =>
                                ask({
                                  title: 'Exportar perfil',
                                  message: `Se guardará «${profile.name}» en un archivo .json.`,
                                  confirmLabel: 'Exportar',
                                  tone: 'primary',
                                  input: {
                                    label: 'Ruta del archivo',
                                    defaultValue: `~/${profile.game_name}_${profile.name}.json`,
                                    onSubmit: (path) => exportProfile(profile.id, path),
                                  },
                                })
                              }
                              onDelete={() =>
                                ask({
                                  title: 'Eliminar perfil',
                                  message: `Se eliminará «${profile.name}». Esta acción no se puede deshacer.`,
                                  confirmLabel: 'Eliminar',
                                  onConfirm: () => deleteProfile(profile.id),
                                })
                              }
                              command={profileCommands[profile.id]}
                            />
                          ))}
                        </div>
                      </SortableContext>
                      <DragOverlay>
                        {activeProfile ? (
                          <div className="rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-2xl shadow-[var(--action)]/20 p-4 cursor-grabbing">
                            <div className="grid grid-cols-2 gap-4">
                              <div className="flex items-center gap-2">
                                <h3 className="text-base font-bold text-[var(--text)] truncate">
                                  {activeProfile.name}
                                </h3>
                                {activeProfile.expert_mode && (
                                  <span className="text-[10px] px-2 py-0.5 rounded bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30 shrink-0">
                                    Experto
                                  </span>
                                )}
                              </div>
                              <p className="text-sm text-[var(--text-muted)] truncate text-right">
                                {activeProfile.game_name}
                              </p>
                            </div>
                          </div>
                        ) : null}
                      </DragOverlay>
                    </DndContext>
                  )}
                </div>
              </motion.div>
            )}

            {/* BENCHMARKS VIEW */}
            {sidebarView === 'benchmarks' && (
              <motion.div key="benchmarks" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <div className="max-w-4xl mx-auto">
                  <BenchmarksView
                    currentAppid={current?.appid}
                    currentGameName={current?.name}
                    currentLaunchCommand={launchLine()}
                    mangohudLogPath={mangohudLogPath}
                    onLogPathChange={setMangohudLogPath}
                  />
                </div>
              </motion.div>
            )}

            {/* DASHBOARD VIEW - MIS PRUEBAS */}
            {sidebarView === 'tests' && (
              <motion.div key="dashboard" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <DashboardView />
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </main>

      {/* Toast */}
      <AnimatePresence>
        {toast && (
          <motion.div
            initial={{ opacity: 0, y: 50 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 50 }}
            className="fixed bottom-6 right-6 px-5 py-3 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-xl shadow-[var(--action)]/20 text-sm font-semibold text-white"
          >
            {toast}
          </motion.div>
        )}
      </AnimatePresence>

      {/* Diálogos propios: el nativo del sistema rompería el estilo de la app */}
      {confirmState && <ConfirmDialog state={confirmState} onClose={close} />}

      <SaveProfileModal
        open={showSaveProfileModal}
        name={newProfileName}
        notes={newProfileNotes}
        onNameChange={setNewProfileName}
        onNotesChange={setNewProfileNotes}
        onCancel={() => setShowSaveProfileModal(false)}
        onSave={saveProfile}
      />
    </div>
  );
}

export default App;
