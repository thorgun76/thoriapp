import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Home, TrendingUp, Shield, Search, Copy, Check, Star,
  ExternalLink, AlertTriangle, Terminal, Users, Settings,
  ChevronDown, Zap, Cpu, GripVertical, RefreshCw, Gamepad2
} from 'lucide-react';
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
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { BenchmarksView } from './components/BenchmarksView';
import { DashboardView } from './components/DashboardView';

// ==================== TIPOS ====================
interface Hardware {
  gpu: string;
  vendor: string;
  cpu: string;
  ram_gb: number;
  kernel: string;
  distro: string;
  session: string;
  mesa: string;
  ntsync: boolean;
  game_performance: boolean;
  tags: string[];
}

interface Game {
  appid: number;
  name: string;
  installed?: boolean;
  launcher?: string; // "steam", "lutris", "heroic", "heronite"
  install_path?: string;
  runner?: string;
}

interface Param {
  kind: string;
  value: string;
  title: string;
  why: string;
  impact: string;
  requires: string[];
  default_on: boolean;
  warn?: string;
  evidence: string;
  source?: string;
  proton_only: boolean;
  recommended_for?: string[];
  tested_on?: string;
  validated_date?: string;
  measured_improvement?: string;
  experimental?: boolean;
  not_recommended_for?: string[];
  essential?: boolean;
  simple_why?: string;
}

interface GameRules {
  name: string;
  appid: number;
  bottleneck: string;
  summary: string;
  params: Param[];
  tips: string[];
  has_rules: boolean;
}

interface GameInfo {
  name?: string;
  native?: boolean;
  tier?: string;
  score?: number;
  total?: number;
  confidence?: string;
  errors: string[];
}

interface GameEngine {
  name: string;
  version: string | null;
  api: string;
}

interface CommunityRecommendation {
  parameter: string;
  description: string;
  optimized_for: string[];
  notes?: string;
  recommended?: boolean;
}

interface Profile {
  id: string;
  name: string;
  appid: number;
  game_name: string;
  enabled_params: boolean[];
  selected_community_recs: number[];
  selected_engine_recs: number[];
  force_proton: boolean;
  expert_mode: boolean;
  manual_command?: string;
  use_manual_command: boolean;
  notes?: string;
  created_at: string;
  updated_at: string;
  order?: number;
}

type View = 'recommendations' | 'tests' | 'compatibility' | 'benchmarks';
type SidebarView = 'library' | 'tests' | 'compatibility' | 'profiles' | 'benchmarks';

// ==================== COMPONENTE PERFIL ORDENABLE ====================
interface SortableProfileProps {
  profile: Profile;
  isExpanded: boolean;
  onToggle: () => void;
  onCopy: () => void;
  onExport: () => void;
  onDelete: () => void;
  command: string | undefined;
}

function SortableProfile({ profile, isExpanded, onToggle, onCopy, onExport, onDelete, command }: SortableProfileProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: profile.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
    zIndex: isDragging ? 1000 : 'auto',
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={`rounded-xl bg-[var(--surface)] border overflow-hidden transition-colors duration-200 ${
        isExpanded ? 'border-[var(--action)]' : 'border-[var(--border)]'
      }`}
    >
      {/* Header del perfil (siempre visible) */}
      <div className="flex items-center">
        {/* Handle de arrastre */}
        <div
          {...attributes}
          {...listeners}
          className="p-2 cursor-grab active:cursor-grabbing hover:bg-[var(--card-hover)] transition-colors touch-none"
        >
          <GripVertical className="w-4 h-4 text-[var(--text-muted)]" />
        </div>

        {/* Contenido del perfil */}
        <button
          onClick={onToggle}
          className="flex-1 p-4 flex items-center gap-4 hover:bg-[var(--card-hover)] transition-colors text-left"
        >
          <div className="flex-1 min-w-0 grid grid-cols-2 gap-4">
            <div className="flex items-center gap-2">
              <h3 className="text-base font-bold text-[var(--text)] truncate">
                {profile.name}
              </h3>
              {profile.expert_mode && (
                <span className="text-[10px] px-2 py-0.5 rounded bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30 shrink-0">
                  Experto
                </span>
              )}
            </div>
            <p className="text-sm text-[var(--text-muted)] truncate text-right">
              {profile.game_name}
            </p>
          </div>
          <ChevronDown
            className={`w-5 h-5 text-[var(--text-muted)] transition-transform duration-200 shrink-0 ${
              isExpanded ? 'rotate-180' : ''
            }`}
          />
        </button>
      </div>

      {/* Contenido expandido (animado) */}
      <AnimatePresence initial={false}>
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            className="overflow-hidden"
          >
            <div className="px-4 pb-4 space-y-3 border-t border-[var(--border)]">
              {/* Fecha y notas */}
              <div className="pt-3 flex items-center gap-3 text-xs text-[var(--text-dim)]">
                <span>Creado: {new Date(profile.created_at).toLocaleDateString('es-ES')}</span>
                {profile.notes && (
                  <>
                    <span>·</span>
                    <span className="truncate">{profile.notes}</span>
                  </>
                )}
              </div>

              {/* Comando con botón copiar */}
              {command && (
                <div className="flex items-center gap-2">
                  <div className="flex-1 p-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] min-w-0">
                    <code className="text-xs text-[var(--compatible)] font-mono break-all">
                      {command}
                    </code>
                  </div>
                  <button
                    onClick={onCopy}
                    className="px-3 py-2 rounded-lg text-xs font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors flex items-center gap-1 shrink-0"
                  >
                    <Copy className="w-3 h-3" />
                    Copiar
                  </button>
                </div>
              )}

              {/* Botones de acción */}
              <div className="flex gap-2 justify-end pt-1">
                <button
                  onClick={onExport}
                  className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
                >
                  Exportar
                </button>
                <button
                  onClick={onDelete}
                  className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-[var(--error)]/10 text-[var(--error)] border border-[var(--error)]/30 hover:bg-[var(--error)]/20 transition-colors"
                >
                  Eliminar
                </button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

// ==================== FUNCIONES AUXILIARES ====================
const openExternalUrl = async (url: string) => {
  try {
    await invoke('open_url', { url });
  } catch (e) {
    window.open(url, '_blank');
  }
};

// ==================== COMPONENTE PRINCIPAL ====================
function App() {
  // Estados principales
  const [sidebarView, setSidebarView] = useState<SidebarView>('library');
  const [view, setView] = useState<View>('recommendations');
  const [hw, setHw] = useState<Hardware | null>(null);
  const [library, setLibrary] = useState<Game[]>([]);
  const [externalGames, setExternalGames] = useState<Game[]>([]);
  const [showExternalGames, setShowExternalGames] = useState(true);
  const [scanningLaunchers, setScanningLaunchers] = useState(false);
  const [current, setCurrent] = useState<Game | null>(null);
  const [rules, setRules] = useState<GameRules | null>(null);
  const [info, setInfo] = useState<GameInfo | null>(null);
  const [engine, setEngine] = useState<GameEngine | null>(null);
  const [engineRecommendations, setEngineRecommendations] = useState<string[]>([]);
  const [communityRecommendations, setCommunityRecommendations] = useState<CommunityRecommendation[]>([]);

  // Estados de selección
  const [enabled, setEnabled] = useState<boolean[]>([]);
  const [selectedCommunityRecs, setSelectedCommunityRecs] = useState<Set<number>>(new Set());
  const [selectedEngineRecs, setSelectedEngineRecs] = useState<Set<number>>(new Set());

  // Estados de UI
  const [query, setQuery] = useState('');
  const [suggestions, setSuggestions] = useState<Game[]>([]);
  const [showSug, setShowSug] = useState(false);
  const [copied, setCopied] = useState(false);
  const [expertMode, setExpertMode] = useState(false);
  // showUnavailable siempre es false (no hay control en la UI para cambiarlo)
  const showUnavailable = false;
  const [toast, setToast] = useState<string | null>(null);

  // Fase 3: Toggle para forzar Proton en juegos nativos
  const [forceProton, setForceProton] = useState(false);

  // Fase 4: Editor manual del comando
  const [manualCommand, setManualCommand] = useState<string>('');
  const [useManualCommand, setUseManualCommand] = useState(false);

  // Fase 5: Perfiles
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [profileCommands, setProfileCommands] = useState<Record<string, string>>({});
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

  const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

    if (over && active.id !== over.id) {
      const oldIndex = profiles.findIndex((p) => p.id === active.id);
      const newIndex = profiles.findIndex((p) => p.id === over.id);

      const newProfiles = arrayMove(profiles, oldIndex, newIndex);
      setProfiles(newProfiles);

      // Guardar el nuevo orden en el backend
      try {
        await invoke('reorder_profiles', {
          profileIds: newProfiles.map(p => p.id)
        });
        setToast('Orden de perfiles actualizado');
      } catch (e) {
        console.error('Error al reordenar perfiles:', e);
        setToast('Error al guardar el orden');
      }
    }
  };

  // Función para cancelar el arrastre
  const handleDragCancel = () => {
    setActiveId(null);
  };

  // Perfil activo (el que se está arrastrando)
  const activeProfile = activeId ? profiles.find(p => p.id === activeId) : null;

  // ==================== EFECTOS ====================
  useEffect(() => { loadData(); }, []);

  // Detectar automáticamente la ruta de logs de MangoHud
  useEffect(() => {
    const detectLogPath = async () => {
      try {
        const detectedPath = await invoke<string>('detect_mangohud_log_path');
        setMangohudLogPath(detectedPath);
        localStorage.setItem('mangohud_log_path', detectedPath);
      } catch (e) {
        console.error('Error detectando ruta de MangoHud:', e);
      }
    };

    detectLogPath();
  }, []);

  useEffect(() => {
    if (toast) {
      const timer = setTimeout(() => setToast(null), 3500);
      return () => clearTimeout(timer);
    }
  }, [toast]);

  // Fase 5: Cargar perfiles al inicio
  useEffect(() => {
    loadAllProfiles();
  }, []);

  const loadData = async () => {
    try {
      const hardware = await invoke<Hardware>('get_hardware');
      setHw(hardware);
      const lib = await invoke<Game[]>('scan_library');
      setLibrary(lib);

      // Cargar juegos de launchers externos (automático)
      setScanningLaunchers(true);
      try {
        const externalGamesList = await invoke<any[]>('scan_external_launchers');
        const convertedGames: Game[] = externalGamesList.map(g => ({
          appid: 0, // Los juegos externos no tienen Steam AppID
          name: g.name,
          installed: true,
          launcher: g.launcher,
          install_path: g.install_path,
          runner: g.runner,
        }));
        setExternalGames(convertedGames);

        // Mostrar notificación si se encontraron juegos externos
        if (convertedGames.length > 0) {
          const launcherCounts = convertedGames.reduce((acc, game) => {
            const launcher = game.launcher || 'unknown';
            acc[launcher] = (acc[launcher] || 0) + 1;
            return acc;
          }, {} as Record<string, number>);

          const summary = Object.entries(launcherCounts)
            .map(([launcher, count]) => `${count} de ${launcher}`)
            .join(', ');

          setToast(`✓ Detectados ${convertedGames.length} juegos externos (${summary})`);
        }
      } catch (e) {
        console.warn('Error cargando juegos de launchers externos:', e);
      } finally {
        setScanningLaunchers(false);
      }
    } catch (e) {
      console.error(e);
      setScanningLaunchers(false);
    }
  };

  const loadAllProfiles = async () => {
    try {
      const allProfiles = await invoke<Profile[]>('load_profiles');
      setProfiles(allProfiles);

      // Reconstruir comandos para cada perfil
      const commands: Record<string, string> = {};
      for (const profile of allProfiles) {
        try {
          let command = '';

          if (profile.use_manual_command && profile.manual_command) {
            command = profile.manual_command;
          } else {
            const rules = await invoke<GameRules>('get_rules', { appid: profile.appid });

            const envVars: string[] = [];
            const wrappers: string[] = [];
            const args: string[] = [];

            // Parámetros del juego
            rules.params.forEach((p, i) => {
              if (profile.enabled_params[i]) {
                if (p.kind === 'env') envVars.push(p.value);
                else if (p.kind === 'wrapper') wrappers.push(p.value);
                else args.push(p.value);
              }
            });

            // Recomendaciones de hardware
            const communityRecs = await invoke<CommunityRecommendation[]>('get_community_recommendations', { hwTags: hw?.tags || [] });
            communityRecs.forEach((rec, i) => {
              if (profile.selected_community_recs.includes(i)) {
                if (rec.parameter.includes('=') || rec.parameter.startsWith('PROTON_') || rec.parameter.startsWith('DXVK_') || rec.parameter.startsWith('VKD3D_') || rec.parameter.startsWith('WINE_') || rec.parameter.startsWith('AMD_') || rec.parameter.startsWith('RADV_') || rec.parameter.startsWith('__')) {
                  envVars.push(rec.parameter);
                } else if (rec.parameter === 'gamemoderun' || rec.parameter === 'mangohud' || rec.parameter === 'gamescope') {
                  wrappers.push(rec.parameter);
                } else {
                  args.push(rec.parameter);
                }
              }
            });

            // Recomendaciones del motor
            const engine = await invoke<GameEngine | null>('detect_engine', { appid: profile.appid, gameName: profile.game_name });
            if (engine) {
              const engineRecs = await invoke<string[]>('get_engine_recommendations', { engine });
              engineRecs.forEach((rec, i) => {
                if (profile.selected_engine_recs.includes(i)) {
                  envVars.push(rec);
                }
              });
            }

            command = [...envVars, ...wrappers, '%command%', ...args].join(' ');
          }

          commands[profile.id] = command;
        } catch (e) {
          console.error(`Error al reconstruir comando para perfil ${profile.id}:`, e);
          commands[profile.id] = '%command%';
        }
      }

      setProfileCommands(commands);
    } catch (e) {
      console.error('Error al cargar perfiles:', e);
    }
  };

  const openGame = async (g: Game) => {
    setCurrent(g);
    setEnabled([]);
    setSelectedCommunityRecs(new Set());
    setSelectedEngineRecs(new Set());
    setEngine(null);
    setEngineRecommendations([]);
    setCommunityRecommendations([]);
    setForceProton(false);

    try {
      // Para juegos de launchers externos (appid = 0), usar reglas genéricas
      const appidToUse = g.appid === 0 ? 0 : g.appid;

      const [r, i, eng] = await Promise.all([
        invoke<GameRules>('get_rules', { appid: appidToUse }),
        g.appid === 0
          ? Promise.resolve({
              name: g.name,
              native: g.launcher === 'lutris' || g.launcher === 'heronite',
              errors: [],
            } as GameInfo)
          : invoke<GameInfo>('game_info', { appid: g.appid }),
        invoke<GameEngine | null>('detect_engine', { appid: appidToUse, gameName: g.name })
      ]);

      setRules(r);
      setInfo(i);
      setEngine(eng);
      setEnabled(new Array(r.params.length).fill(false));

      if (eng) {
        const engineRecs = await invoke<string[]>('get_engine_recommendations', { engine: eng });
        setEngineRecommendations(engineRecs);
      }

      try {
        const communityRecs = await invoke<CommunityRecommendation[]>('get_community_recommendations', { hwTags: hw?.tags || [] });
        setCommunityRecommendations(communityRecs);
      } catch (e) {
        console.warn('No se pudieron cargar las recomendaciones de la comunidad:', e);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const missing = (p: Param): string | null => {
    if (info?.native === true && p.proton_only && !forceProton) {
      return 'Solo aplica con Proton; este juego es nativo de Linux';
    }
    const r = p.requires.find(t => !hw?.tags.includes(t));
    if (r) return `Necesita ${r}`;
    return null;
  };

  const getRecommendationStatus = (p: Param): string => {
    if (p.proton_only && !forceProton) {
      return 'not_recommended';
    }

    if (p.not_recommended_for && p.not_recommended_for.length > 0) {
      for (const notRec of p.not_recommended_for) {
        if (hw?.tags.includes(notRec)) {
          return 'not_applicable';
        }
      }
    }

    if (p.recommended_for && p.recommended_for.length > 0) {
      if (p.recommended_for.includes('all')) {
        return 'recommended';
      }
      for (const rec of p.recommended_for) {
        if (hw?.tags.includes(rec)) {
          return 'recommended';
        }
      }
      return 'neutral';
    }

    if (p.requires && p.requires.length > 0) {
      for (const req of p.requires) {
        if (!hw?.tags.includes(req)) {
          return 'not_available';
        }
      }
    }

    return 'neutral';
  };

  const toggleRule = (index: number) => {
    if (!rules) return;
    const param = rules.params[index];
    if (missing(param)) return;

    const newEnabled = [...enabled];
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

  const launchLine = (): string => {
    if (useManualCommand && manualCommand.trim()) {
      return manualCommand.trim();
    }

    if (!rules) return '%command%';
    const e: string[] = [], w: string[] = [], a: string[] = [];

    rules.params.forEach((p, i) => {
      if (enabled[i] !== true) return;
      if (p.kind === 'env') e.push(p.value);
      else if (p.kind === 'wrapper') w.push(p.value);
      else a.push(p.value);
    });

    communityRecommendations.forEach((rec, i) => {
      if (!selectedCommunityRecs.has(i)) return;
      if (rec.parameter.includes('=') || rec.parameter.startsWith('PROTON_') || rec.parameter.startsWith('DXVK_') || rec.parameter.startsWith('VKD3D_') || rec.parameter.startsWith('WINE_') || rec.parameter.startsWith('AMD_') || rec.parameter.startsWith('RADV_') || rec.parameter.startsWith('__')) {
        e.push(rec.parameter);
      } else if (rec.parameter === 'gamemoderun' || rec.parameter === 'mangohud' || rec.parameter === 'gamescope') {
        w.push(rec.parameter);
      } else {
        a.push(rec.parameter);
      }
    });

    engineRecommendations.forEach((rec, i) => {
      if (!selectedEngineRecs.has(i)) return;
      e.push(rec);
    });

    if (e.length === 0 && w.length === 0 && a.length === 0) {
      return '%command%';
    }

    return [...e, ...w, '%command%', ...a].join(' ');
  };

  const copyCmd = async () => {
    try {
      await navigator.clipboard.writeText(launchLine());
      setCopied(true);
      setToast('Comando copiado: ' + launchLine());
      setTimeout(() => setCopied(false), 2000);
    } catch (e) { console.error(e); }
  };

  const searchSteam = async (searchTerm: string) => {
    if (!searchTerm.trim() || searchTerm.length < 2) return;
    try {
      const results = await invoke<Game[]>('search_store', { term: searchTerm });
      setSuggestions(results);
      setShowSug(true);
    } catch (e) { console.error(e); }
  };

  const handleQueryChange = (value: string) => {
    setQuery(value);
    if (searchTimeoutRef.current) clearTimeout(searchTimeoutRef.current);
    if (value.length < 2) {
      setSuggestions([]);
      setShowSug(false);
      return;
    }
    searchTimeoutRef.current = setTimeout(() => searchSteam(value), 500);
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

  const filteredLib = library.filter(g => g.name.toLowerCase().includes(query.toLowerCase()));
  const filteredExternalGames = externalGames.filter(g => g.name.toLowerCase().includes(query.toLowerCase()));
  const combinedLibrary = showExternalGames
    ? [...filteredLib, ...filteredExternalGames]
    : filteredLib;

  const saveProfile = async () => {
    if (!current || !newProfileName.trim()) return;

    try {
      await invoke('save_profile', {
        name: newProfileName,
        appid: current.appid,
        gameName: current.name,
        enabledParams: enabled,
        selectedCommunityRecs: Array.from(selectedCommunityRecs),
        selectedEngineRecs: Array.from(selectedEngineRecs),
        forceProton: forceProton,
        expertMode: expertMode,
        manualCommand: manualCommand || null,
        useManualCommand: useManualCommand,
        notes: newProfileNotes || null,
      });

      setToast(`Perfil "${newProfileName}" guardado`);
      setShowSaveProfileModal(false);
      setNewProfileName('');
      setNewProfileNotes('');
      loadAllProfiles();
    } catch (e) {
      console.error('Error al guardar perfil:', e);
      setToast('Error al guardar perfil');
    }
  };

  const loadProfile = async (profile: Profile) => {
    try {
      let command = '';

      if (profile.use_manual_command && profile.manual_command) {
        command = profile.manual_command;
      } else {
        try {
          const rules = await invoke<GameRules>('get_rules', { appid: profile.appid });

          const envVars: string[] = [];
          const wrappers: string[] = [];
          const args: string[] = [];

          rules.params.forEach((p, i) => {
            if (profile.enabled_params[i]) {
              if (p.kind === 'env') envVars.push(p.value);
              else if (p.kind === 'wrapper') wrappers.push(p.value);
              else args.push(p.value);
            }
          });

          const communityRecs = await invoke<CommunityRecommendation[]>('get_community_recommendations', { hwTags: hw?.tags || [] });
          communityRecs.forEach((rec, i) => {
            if (profile.selected_community_recs.includes(i)) {
              if (rec.parameter.includes('=') || rec.parameter.startsWith('PROTON_') || rec.parameter.startsWith('DXVK_') || rec.parameter.startsWith('VKD3D_') || rec.parameter.startsWith('WINE_') || rec.parameter.startsWith('AMD_') || rec.parameter.startsWith('RADV_') || rec.parameter.startsWith('__')) {
                envVars.push(rec.parameter);
              } else if (rec.parameter === 'gamemoderun' || rec.parameter === 'mangohud' || rec.parameter === 'gamescope') {
                wrappers.push(rec.parameter);
              } else {
                args.push(rec.parameter);
              }
            }
          });

          const engine = await invoke<GameEngine | null>('detect_engine', { appid: profile.appid, gameName: profile.game_name });
          if (engine) {
            const engineRecs = await invoke<string[]>('get_engine_recommendations', { engine });
            engineRecs.forEach((rec, i) => {
              if (profile.selected_engine_recs.includes(i)) {
                envVars.push(rec);
              }
            });
          }

          command = [...envVars, ...wrappers, '%command%', ...args].join(' ');
        } catch (e) {
          console.error('Error al reconstruir comando:', e);
          command = '%command%';
        }
      }

      await navigator.clipboard.writeText(command);
      setToast(`✓ Comando copiado: ${command}`);
    } catch (e) {
      console.error('Error al cargar perfil:', e);
      setToast('Error al cargar perfil');
    }
  };

  const deleteProfile = async (profileId: string) => {
    try {
      await invoke('delete_profile', { profileId });
      setToast('Perfil eliminado');
      loadAllProfiles();
    } catch (e) {
      console.error('Error al eliminar perfil:', e);
      setToast('Error al eliminar perfil');
    }
  };

  // ==================== RENDER ====================
  return (
    <div className="h-screen flex bg-[var(--bg)] text-[var(--text)] overflow-hidden">
      {/* SIDEBAR */}
      <aside className="w-64 bg-[var(--surface)] border-r border-[var(--border)] flex flex-col">
        {/* Brand */}
        <div className="p-5 border-b border-[var(--border)]">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-[var(--action)] to-purple-600 flex items-center justify-center text-white font-bold text-lg">
              ✦
            </div>
            <div>
              <h1 className="text-base font-bold text-white">Thoriapp</h1>
              <p className="text-xs text-[var(--text-muted)]">Optimización Linux</p>
            </div>
          </div>
        </div>

        {/* Navigation */}
        <div className="flex-1 p-3 space-y-6">
          <div>
            <p className="text-[10px] font-semibold text-[var(--text-dim)] uppercase tracking-wider px-3 mb-2">Espacio de trabajo</p>
            <nav className="space-y-1">
              <button
                onClick={() => { setSidebarView('library'); setCurrent(null); }}
                className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
                  sidebarView === 'library' && !current
                    ? 'bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/20'
                    : 'text-[var(--text-muted)] hover:bg-[var(--card)] hover:text-white'
                }`}
              >
                <Home className="w-4 h-4" />
                <span>Mi biblioteca</span>
              </button>
              <button
                onClick={() => { setSidebarView('tests'); setCurrent(null); }}
                className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
                  sidebarView === 'tests'
                    ? 'bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/20'
                    : 'text-[var(--text-muted)] hover:bg-[var(--card)] hover:text-white'
                }`}
              >
                <TrendingUp className="w-4 h-4" />
                <span>Mis pruebas</span>
              </button>
              <button
                onClick={() => { setSidebarView('compatibility'); setCurrent(null); }}
                className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
                  sidebarView === 'compatibility'
                    ? 'bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/20'
                    : 'text-[var(--text-muted)] hover:bg-[var(--card)] hover:text-white'
                }`}
              >
                <Shield className="w-4 h-4" />
                <span>Compatibilidad</span>
              </button>
              <button
                onClick={() => { setSidebarView('profiles'); setCurrent(null); }}
                className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
                  sidebarView === 'profiles'
                    ? 'bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/20'
                    : 'text-[var(--text-muted)] hover:bg-[var(--card)] hover:text-white'
                }`}
              >
                <Star className="w-4 h-4" />
                <span>Perfiles</span>
                {profiles.length > 0 && (
                  <span className="ml-auto text-xs bg-[var(--action)]/20 text-[var(--action)] px-2 py-0.5 rounded-full">
                    {profiles.length}
                  </span>
                )}
              </button>
              <button
                onClick={() => { setSidebarView('benchmarks'); setCurrent(null); }}
                className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
                  sidebarView === 'benchmarks'
                    ? 'bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/20'
                    : 'text-[var(--text-muted)] hover:bg-[var(--card)] hover:text-white'
                }`}
              >
                <TrendingUp className="w-4 h-4" />
                <span>Benchmarks</span>
              </button>
            </nav>
          </div>

          <div>
            <p className="text-[10px] font-semibold text-[var(--text-dim)] uppercase tracking-wider px-3 mb-2">Experiencia</p>
            <nav className="space-y-1">
              <div className="w-full flex items-center justify-between px-3 py-2.5 rounded-lg text-sm">
                <div className={`flex items-center gap-3 ${expertMode ? 'text-[var(--action)]' : 'text-[var(--text-muted)]'}`}>
                  <Settings className="w-4 h-4" />
                  <span>{expertMode ? 'Modo experto' : 'Modo guiado'}</span>
                </div>
                <button
                  onClick={() => setExpertMode(!expertMode)}
                  className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
                    expertMode ? 'bg-[var(--action)]' : 'bg-[var(--border)]'
                  }`}
                  role="switch"
                  aria-checked={expertMode}
                >
                  <span
                    className={`inline-block h-4 w-4 transform rounded-full bg-white shadow-lg transition-transform duration-200 ease-in-out ${
                      expertMode ? 'translate-x-6' : 'translate-x-1'
                    }`}
                  />
                </button>
              </div>
              <p className="text-[10px] text-[var(--text-dim)] px-3 mt-1">
                {expertMode ? 'Todos los parámetros y detalles técnicos' : 'Solo ajustes esenciales'}
              </p>
            </nav>
          </div>
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-[var(--border)]">
          <div className="flex items-center gap-3 mb-2">
            <div className="w-8 h-8 rounded-full bg-gradient-to-br from-[var(--action)] to-purple-600 flex items-center justify-center text-white text-sm font-bold">
              L
            </div>
            <div>
              <p className="text-sm font-semibold text-white">Tu entorno Linux</p>
              <p className="text-xs text-[var(--text-muted)]">{hw?.distro || 'Cargando...'}</p>
            </div>
          </div>
          <p className="text-[10px] text-[var(--text-dim)]">Optimizador honesto para Linux</p>
        </div>
      </aside>

      {/* MAIN CONTENT */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {/* Topbar */}
        <header className="h-14 border-b border-[var(--border)] flex items-center px-6 gap-4 bg-[var(--surface)]">
          <div className="flex-1 relative max-w-md">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--text-muted)]" />
            <input
              type="text"
              value={query}
              onChange={(e) => handleQueryChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && suggestions.length > 0) {
                  openGame(suggestions[0]);
                  setShowSug(false);
                }
              }}
              onFocus={() => suggestions.length > 0 && setShowSug(true)}
              onBlur={() => setTimeout(() => setShowSug(false), 200)}
              placeholder="Buscar juegos para optimizar..."
              className="w-full pl-10 pr-4 py-2 rounded-lg bg-[var(--card)] border border-[var(--border)] focus:border-[var(--action)]/50 focus:outline-none text-sm"
            />
            {showSug && suggestions.length > 0 && (
              <div className="absolute top-full left-0 right-0 mt-1 bg-[var(--card)] border border-[var(--border)] rounded-lg shadow-xl overflow-hidden z-50 max-h-72 overflow-y-auto">
                {suggestions.map((g, i) => (
                  <button
                    key={i}
                    onClick={() => { openGame(g); setShowSug(false); setQuery(''); }}
                    className="w-full px-4 py-2.5 text-left hover:bg-[var(--card-hover)] transition-colors flex justify-between items-center"
                  >
                    <span className="text-sm">{g.name}</span>
                    <span className="text-xs text-[var(--text-muted)]">AppID {g.appid}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="flex items-center gap-3">
            <span className="text-xs px-2 py-1 rounded bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30">
              {combinedLibrary.length} juegos
            </span>
            {scanningLaunchers && (
              <span className="text-xs px-2 py-1 rounded bg-[var(--caution-bg)] text-[var(--caution)] border border-[var(--caution)]/30 flex items-center gap-1">
                <div className="w-3 h-3 border-2 border-[var(--caution)]/30 border-t-[var(--caution)] rounded-full animate-spin"></div>
                Escaneando launchers...
              </span>
            )}
            {hw && (
              <span className="text-xs px-2 py-1 rounded bg-[var(--compatible-bg)] text-[var(--compatible)] border border-[var(--compatible)]/30 flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-[var(--compatible)]"></span>
                {hw.vendor === 'Amd' ? 'AMD' : hw.vendor === 'Nvidia' ? 'NVIDIA' : hw.vendor === 'Intel' ? 'Intel' : 'Linux'}
              </span>
            )}
          </div>
        </header>

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          <AnimatePresence mode="wait">
            {/* LIBRARY VIEW */}
            {sidebarView === 'library' && !current && (
              <motion.div key="library" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <div className="max-w-6xl mx-auto">
                  <div className="flex items-center justify-between mb-6">
                    <div>
                      <p className="text-xs text-[var(--text-muted)] mb-2">Biblioteca</p>
                      <h2 className="text-2xl font-bold">Tu biblioteca de juegos</h2>
                    </div>
                    <div className="flex items-center gap-2">
                      {externalGames.length > 0 && (
                        <button
                          onClick={() => setShowExternalGames(!showExternalGames)}
                          className={`px-4 py-2 rounded-lg text-sm font-semibold transition-colors flex items-center gap-2 ${
                            showExternalGames
                              ? 'bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30'
                              : 'bg-[var(--surface)] text-[var(--text-muted)] border border-[var(--border)]'
                          }`}
                        >
                          <Gamepad2 className="w-4 h-4" />
                          {showExternalGames ? 'Ocultar' : 'Mostrar'} launchers externos ({externalGames.length})
                        </button>
                      )}
                      <button
                        onClick={async () => {
                          setScanningLaunchers(true);
                          try {
                            const externalGamesList = await invoke<any[]>('scan_external_launchers');
                            const convertedGames: Game[] = externalGamesList.map(g => ({
                              appid: 0,
                              name: g.name,
                              installed: true,
                              launcher: g.launcher,
                              install_path: g.install_path,
                              runner: g.runner,
                            }));
                            setExternalGames(convertedGames);

                            if (convertedGames.length > 0) {
                              const launcherCounts = convertedGames.reduce((acc, game) => {
                                const launcher = game.launcher || 'unknown';
                                acc[launcher] = (acc[launcher] || 0) + 1;
                                return acc;
                              }, {} as Record<string, number>);

                              const summary = Object.entries(launcherCounts)
                                .map(([launcher, count]) => `${count} de ${launcher}`)
                                .join(', ');

                              setToast(`✓ Re-escaneo completado: ${convertedGames.length} juegos (${summary})`);
                            } else {
                              setToast('ℹ️ No se encontraron juegos en launchers externos');
                            }
                          } catch (e) {
                            console.error('Error re-escaneando launchers:', e);
                            setToast('❌ Error al escanear launchers');
                          } finally {
                            setScanningLaunchers(false);
                          }
                        }}
                        disabled={scanningLaunchers}
                        className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        {scanningLaunchers ? (
                          <>
                            <div className="w-4 h-4 border-2 border-[var(--text-muted)]/30 border-t-[var(--text-muted)] rounded-full animate-spin"></div>
                            Escaneando...
                          </>
                        ) : (
                          <>
                            <RefreshCw className="w-4 h-4" />
                            Re-escanear
                          </>
                        )}
                      </button>
                    </div>
                  </div>

                  {combinedLibrary.length === 0 ? (
                    <div className="text-center py-16">
                      <Home className="w-12 h-12 text-[var(--text-dim)] mx-auto mb-4" />
                      <p className="text-[var(--text-muted)]">No se encontraron juegos</p>
                      <p className="text-xs text-[var(--text-dim)] mt-2">Usa el buscador para encontrar juegos en Steam</p>
                    </div>
                  ) : (
                    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
                      {combinedLibrary.map(g => (
                        <button
                          key={`${g.launcher || 'steam'}-${g.appid || g.name}`}
                          onClick={() => openGame(g)}
                          className="group relative aspect-[3/4] rounded-xl bg-[var(--card)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-all overflow-hidden"
                        >
                          {/* Indicador de launcher */}
                          {g.launcher && g.launcher !== 'steam' && (
                            <div className="absolute top-2 right-2 z-10">
                              <span className={`text-[10px] px-2 py-1 rounded font-semibold ${
                                g.launcher === 'lutris' ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30' :
                                g.launcher === 'heroic' ? 'bg-blue-500/20 text-blue-400 border border-blue-500/30' :
                                g.launcher === 'heronite' ? 'bg-purple-500/20 text-purple-400 border border-purple-500/30' :
                                'bg-gray-500/20 text-gray-400 border border-gray-500/30'
                              }`}>
                                {g.launcher.charAt(0).toUpperCase() + g.launcher.slice(1)}
                              </span>
                            </div>
                          )}
                          <div className="absolute inset-0 flex flex-col items-center justify-center p-3 text-center">
                            <Home className="w-8 h-8 text-[var(--text-dim)] group-hover:text-[var(--action)] transition-colors mb-2" />
                            <p className="text-xs font-medium text-[var(--text-muted)] group-hover:text-white line-clamp-3">{g.name}</p>
                          </div>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </motion.div>
            )}

            {/* GAME VIEW */}
            {sidebarView === 'library' && current && rules && info && (
              <motion.div key="game" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <div className="max-w-6xl mx-auto">
                  {/* Breadcrumb */}
                  <p className="text-xs text-[var(--text-muted)] mb-4">
                    Biblioteca <span className="mx-2">/</span> <strong className="text-white">{current.name}</strong>
                  </p>

                  {/* COMANDO DE LANZAMIENTO - ELEMENTO PROTAGONISTA */}
                  <section className="mb-8 p-6 rounded-2xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-lg shadow-[var(--action)]/10">
                    <div className="flex items-start justify-between mb-4">
                      <div>
                        <p className="text-xs text-[var(--action)] font-bold uppercase tracking-wider mb-2">Comando de lanzamiento</p>
                        <h2 className="text-2xl font-bold mb-1">{current.name}</h2>
                        <p className="text-sm text-[var(--text-muted)]">
                          {launchLine() === '%command%'
                            ? 'Punto de partida sin modificaciones. Activa los ajustes opcionales para realizar pruebas.'
                            : `${enabled.filter(e => e).length + selectedCommunityRecs.size + selectedEngineRecs.size} ajuste(s) activo(s). Prueba cada cambio por separado.`}
                        </p>
                      </div>
                      <div className="flex flex-col gap-2">
                        {info.native === true && (
                          <span className="text-xs px-3 py-1 rounded-full bg-[var(--compatible-bg)] text-[var(--compatible)] border border-[var(--compatible)]/30 font-semibold">
                            ● Linux nativo
                          </span>
                        )}
                        {info.tier && (
                          <span className={`text-xs px-3 py-1 rounded-full border font-semibold ${
                            info.tier.toLowerCase() === 'platinum' ? 'bg-[var(--compatible-bg)] text-[var(--compatible)] border-[var(--compatible)]/30' :
                            info.tier.toLowerCase() === 'gold' ? 'bg-[var(--caution-bg)] text-[var(--caution)] border-[var(--caution)]/30' :
                            'bg-[var(--surface)] text-[var(--text-muted)] border-[var(--border)]'
                          }`}>
                            ProtonDB: {info.tier}
                          </span>
                        )}
                      </div>
                    </div>

                    {/* Codebox grande y prominente */}
                    <div className="p-5 rounded-xl bg-[var(--bg)] border-2 border-[var(--border)] mb-4">
                      <code className="text-base text-[var(--compatible)] font-mono break-all leading-relaxed">{launchLine()}</code>
                    </div>

                    <div className="flex gap-3">
                      <button
                        onClick={copyCmd}
                        className={`flex-1 flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-bold text-base transition-all ${
                          copied
                            ? 'bg-[var(--compatible)] text-[var(--bg)]'
                            : 'bg-[var(--action)] text-white hover:bg-[var(--action-hover)]'
                        }`}
                      >
                        {copied ? <><Check className="w-5 h-5" /> ¡Copiado!</> : <><Copy className="w-5 h-5" /> Copiar comando</>}
                      </button>
                      <button
                        onClick={() => setShowSaveProfileModal(true)}
                        className="px-4 py-3 rounded-xl text-sm font-semibold bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30 hover:bg-[var(--action)]/20 transition-colors flex items-center gap-2"
                      >
                        <Star className="w-4 h-4" />
                        Guardar perfil
                      </button>
                      <button
                        onClick={() => {
                          setEnabled(new Array(rules.params.length).fill(false));
                          setSelectedCommunityRecs(new Set());
                          setSelectedEngineRecs(new Set());
                          setToast('Opciones restablecidas al comando base');
                        }}
                        className="px-4 py-3 rounded-xl text-sm font-semibold bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
                      >
                        Restablecer
                      </button>
                    </div>

                    {/* Fase 4: Editor manual del comando (solo en modo experto) */}
                    {expertMode && (
                      <div className="mt-4 p-4 rounded-xl bg-[var(--card)] border border-[var(--border)]">
                        <div className="flex items-center justify-between mb-3">
                          <div>
                            <p className="text-sm font-semibold text-[var(--text)]">Editar comando manualmente</p>
                            <p className="text-xs text-[var(--text-muted)] mt-1">
                              Modifica el comando directamente. Los cambios manuales sobrescriben las selecciones automáticas.
                            </p>
                          </div>
                          <button
                            onClick={() => {
                              setUseManualCommand(!useManualCommand);
                              if (!useManualCommand) {
                                setManualCommand(launchLine());
                              }
                            }}
                            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
                              useManualCommand ? 'bg-[var(--action)]' : 'bg-[var(--border)]'
                            }`}
                            role="switch"
                            aria-checked={useManualCommand}
                          >
                            <span
                              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 ease-in-out ${
                                useManualCommand ? 'translate-x-6' : 'translate-x-1'
                              }`}
                            />
                          </button>
                        </div>
                        {useManualCommand && (
                          <textarea
                            value={manualCommand}
                            onChange={(e) => setManualCommand(e.target.value)}
                            className="w-full p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm text-[var(--compatible)] font-mono focus:outline-none focus:border-[var(--action)] resize-none"
                            rows={3}
                            placeholder="%command%"
                          />
                        )}
                      </div>
                    )}
                  </section>

                  {/* TABS: Compatibilidad, Optimización, Diagnóstico */}
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
                      <button
                        onClick={() => setView('tests')}
                        className={`px-5 py-3 text-sm font-semibold transition-colors border-b-2 ${
                          view === 'tests'
                            ? 'text-[var(--action)] border-[var(--action)]'
                            : 'text-[var(--text-muted)] border-transparent hover:text-[var(--text)]'
                        }`}
                      >
                        Diagnóstico
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
                                      onClick={() => setForceProton(!forceProton)}
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
                                onClick={() => openExternalUrl(`https://www.protondb.com/app/${current.appid}`)}
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
                      )}

                      {/* TAB: OPTIMIZACIÓN */}
                      {view === 'recommendations' && (
                        <div className="space-y-4">
                          {/* Fase 4: Indicador de modo */}
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
                                  onClick={() => setForceProton(!forceProton)}
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
                                  if (idx !== -1) toggleRule(idx);
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
                                          onClick={() => toggleRule(idx)}
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
                                      onClick={() => toggleEngineRec(i)}
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
                                        onClick={() => toggleCommunityRec(i)}
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
                                      return (
                                        <button
                                          key={i}
                                          disabled={!!isMissing}
                                          onClick={() => toggleRule(idx)}
                                          className={`w-full text-left p-4 rounded-lg border transition-all ${
                                            isMissing
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
                                              {enabled[idx] && p.warn && (
                                                <p className="text-sm text-[var(--caution)] mt-2 flex items-start gap-2">
                                                  <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
                                                  {p.warn}
                                                </p>
                                              )}
                                              {enabled[idx] && p.source && (
                                                <button
                                                  onClick={(e) => { e.stopPropagation(); openExternalUrl(p.source!); }}
                                                  className="text-sm text-[var(--action)] hover:text-[var(--action-hover)] mt-2 inline-flex items-center gap-1"
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
                                        </button>
                                      );
                                    })}
                                </div>
                              </div>
                            )}
                          </article>
                        </div>
                      )}

                      {/* TAB: DIAGNÓSTICO */}
                      {view === 'tests' && (
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
                                  onClick={() => openExternalUrl('https://github.com/flightlessmango/MangoHud')}
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
                                  onClick={() => openExternalUrl('https://github.com/FeralInteractive/gamemode')}
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
                      )}
                    </div>

                    {/* Sidebar Column */}
                    <div className="space-y-4">
                      {/* Hardware Profile */}
                      {hw && (
                        <aside className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <h3 className="text-sm font-bold mb-4 flex items-center gap-2 text-[var(--text)]">
                            <Cpu className="w-4 h-4 text-[var(--action)]" />
                            Tu perfil de hardware
                          </h3>
                          <div className="space-y-2">
                            <div className="flex items-start gap-3 text-sm">
                              <span className="text-[var(--text-muted)] min-w-[70px]">Sistema</span>
                              <strong className="text-[var(--text)] text-right flex-1">{hw.distro}</strong>
                            </div>
                            <div className="flex items-start gap-3 text-sm">
                              <span className="text-[var(--text-muted)] min-w-[70px]">Gráficos</span>
                              <strong className="text-[var(--text)] text-right flex-1 break-words">{hw.gpu}</strong>
                            </div>
                            <div className="flex items-start gap-3 text-sm">
                              <span className="text-[var(--text-muted)] min-w-[70px]">CPU</span>
                              <strong className="text-[var(--text)] text-right flex-1 break-words">{hw.cpu}</strong>
                            </div>
                            <div className="flex items-start gap-3 text-sm">
                              <span className="text-[var(--text-muted)] min-w-[70px]">RAM</span>
                              <strong className="text-[var(--text)] text-right flex-1">{hw.ram_gb.toFixed(1)} GB</strong>
                            </div>
                            <div className="flex items-start gap-3 text-sm">
                              <span className="text-[var(--text-muted)] min-w-[70px]">Kernel</span>
                              <strong className="text-[var(--text)] text-right flex-1">{hw.kernel}</strong>
                            </div>
                            {hw.ntsync && (
                              <div className="flex items-start gap-3 text-sm">
                                <span className="text-[var(--text-muted)] min-w-[70px]">NTSYNC</span>
                                <strong className="text-[var(--compatible)] text-right flex-1">✓ Disponible</strong>
                              </div>
                            )}
                          </div>
                        </aside>
                      )}

                      {/* Quick Actions */}
                      <aside className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                        <h3 className="text-sm font-bold mb-3 text-[var(--text)]">Acciones rápidas</h3>
                        <div className="space-y-2">
                          <button
                            onClick={() => {
                              setView('recommendations');
                              setToast('Selecciona ajustes en la pestaña Optimización');
                            }}
                            className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors"
                          >
                            <Zap className="w-4 h-4" /> Ajustar optimización
                          </button>
                          <button
                            onClick={() => {
                              setView('tests');
                              setToast('Sigue el tutorial de medición');
                            }}
                            className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
                          >
                            <TrendingUp className="w-4 h-4" /> Medir rendimiento
                          </button>
                        </div>
                      </aside>

                      {/* Sources */}
                      <aside className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                        <p className="text-xs text-[var(--action)] font-semibold uppercase tracking-wider mb-3">Fuentes verificables</p>
                        <div className="space-y-2 mb-3">
                          <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
                            <span className="w-1.5 h-1.5 rounded-full bg-[var(--compatible)]"></span>
                            <span>Recomendaciones conservadoras</span>
                          </div>
                          <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
                            <span className="w-1.5 h-1.5 rounded-full bg-[var(--action)]"></span>
                            <span>Documentación oficial</span>
                          </div>
                        </div>
                        <p className="text-xs text-[var(--text-dim)]">
                          Las etiquetas de compatibilidad no demuestran una ganancia de rendimiento.
                        </p>
                      </aside>
                    </div>
                  </div>
                </div>
              </motion.div>
            )}

            {/* PROFILES VIEW */}
            {sidebarView === 'profiles' && (
              <motion.div key="profiles" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <div className="max-w-4xl mx-auto">
                  <div className="flex items-center justify-between mb-6">
                    <div>
                      <p className="text-xs text-[var(--text-muted)] mb-2">Perfiles</p>
                      <h2 className="text-2xl font-bold">Tus configuraciones guardadas</h2>
                    </div>
                    <div className="flex gap-2">
                      <button
                        onClick={async () => {
                          const input = document.createElement('input');
                          input.type = 'file';
                          input.accept = '.json';
                          input.onchange = async (e) => {
                            const file = (e.target as HTMLInputElement).files?.[0];
                            if (file) {
                              try {
                                // En navegadores, no podemos acceder a la ruta completa del archivo
                                // Por ahora, usamos el nombre del archivo como identificador
                                const path = file.name;
                                const imported = await invoke<Profile[]>('import_profile', { path });
                                setToast(`${imported.length} perfil(es) importado(s) correctamente`);
                                loadAllProfiles();
                              } catch (err) {
                                setToast('Error al importar perfil');
                              }
                            }
                          };
                          input.click();
                        }}
                        className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors flex items-center gap-2"
                      >
                        <Star className="w-4 h-4" />
                        Importar perfil
                      </button>
                      <button
                        onClick={async () => {
                          const path = prompt('Ruta para exportar todos los perfiles:', `/home/thoriapp_perfiles_backup.json`);
                          if (path) {
                            try {
                              await invoke('export_all_profiles', { path });
                              setToast('Todos los perfiles exportados correctamente');
                            } catch (err) {
                              setToast('Error al exportar perfiles');
                            }
                          }
                        }}
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
                              onExport={async () => {
                                const path = prompt('Ruta para exportar el perfil:', `/home/${profile.game_name}_${profile.name}.json`);
                                if (path) {
                                  try {
                                    await invoke('export_profile', { profileId: profile.id, path });
                                    setToast('Perfil exportado correctamente');
                                  } catch (err) {
                                    setToast('Error al exportar perfil');
                                  }
                                }
                              }}
                              onDelete={() => {
                                if (confirm(`¿Eliminar perfil "${profile.name}"?`)) {
                                  deleteProfile(profile.id);
                                }
                              }}
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
              <motion.div key="benchmarks" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
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
              <motion.div key="dashboard" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
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

      {/* Modal para guardar perfil */}
      <AnimatePresence>
        {showSaveProfileModal && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
            onClick={() => setShowSaveProfileModal(false)}
          >
            <motion.div
              initial={{ scale: 0.9, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.9, opacity: 0 }}
              className="bg-[var(--surface)] border border-[var(--border)] rounded-2xl max-w-md w-full p-6"
              onClick={(e) => e.stopPropagation()}
            >
              <h3 className="text-lg font-bold mb-4">Guardar perfil</h3>
              <div className="space-y-4">
                <div>
                  <label className="text-sm font-semibold text-[var(--text)] mb-2 block">
                    Nombre del perfil
                  </label>
                  <input
                    type="text"
                    value={newProfileName}
                    onChange={(e) => setNewProfileName(e.target.value)}
                    placeholder="Ej: Competitivo, Calidad visual, etc."
                    className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm focus:outline-none focus:border-[var(--action)]"
                    autoFocus
                  />
                </div>
                <div>
                  <label className="text-sm font-semibold text-[var(--text)] mb-2 block">
                    Notas (opcional)
                  </label>
                  <textarea
                    value={newProfileNotes}
                    onChange={(e) => setNewProfileNotes(e.target.value)}
                    placeholder="Describe esta configuración..."
                    className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm focus:outline-none focus:border-[var(--action)] resize-none"
                    rows={3}
                  />
                </div>
                <div className="flex gap-3">
                  <button
                    onClick={() => setShowSaveProfileModal(false)}
                    className="flex-1 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
                  >
                    Cancelar
                  </button>
                  <button
                    onClick={saveProfile}
                    disabled={!newProfileName.trim()}
                    className="flex-1 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    Guardar
                  </button>
                </div>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export default App;
