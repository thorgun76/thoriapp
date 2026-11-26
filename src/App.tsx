import { useState, useEffect, useMemo, lazy, Suspense } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
// Vistas pesadas que solo se montan al cambiar de sección: se cargan bajo
// demanda para que el arranque no pague por ellas (BenchmarksView sola pesa
// ~58 KB de código fuente).
const BenchmarksView = lazy(() =>
  import('./components/BenchmarksView').then((m) => ({ default: m.BenchmarksView }))
);
const DashboardView = lazy(() =>
  import('./components/DashboardView').then((m) => ({ default: m.DashboardView }))
);
// Se abre desde la ficha del juego: se carga al pulsar «Pruebas del juego», no antes.
const PruebasJuego = lazy(() =>
  import('./components/PruebasJuego').then((m) => ({ default: m.PruebasJuego }))
);
import { Sidebar } from './components/Sidebar';
import { LibraryView } from './components/LibraryView';
import { SaveProfileModal } from './components/SaveProfileModal';
import { ImportPreviewDialog } from './components/ImportPreviewDialog';
import { CommandBox } from './components/CommandBox';
import { GameTabs } from './components/GameTabs';
import { OptimizationSummary } from './components/OptimizationSummary';
import { GameSidebar } from './components/GameSidebar';
import { PerfilesTab } from './components/PerfilesTab';
import { Topbar } from './components/Topbar';
import { CompatTab } from './components/CompatTab';
import { OptimizeTab } from './components/OptimizeTab';
import { api } from './lib/api';
import type {
  EstadoObservador,
  Game,
  LauncherFilter,
  Param,
  Profile,
  SidebarView,
  View,
} from './lib/types';
import { useCatalogoDeAjustes } from './hooks/useCatalogoDeAjustes';
import { useHardware } from './hooks/useHardware';
import { useLibrary } from './hooks/useLibrary';
import { useProfiles } from './hooks/useProfiles';
import { useGameDetails } from './hooks/useGameDetails';
import { useRuntimeEnv } from './hooks/useRuntimeEnv';
import { usarOpcionesSteam } from './hooks/useSteamLaunch';
import {
  claveJuego,
  comandoManualDe,
  editandoAMano,
} from './lib/comandoManual';
import { useConfirm, useToast } from './hooks/useConfirm';
import { ArgumentosConservados } from './components/ArgumentosConservados';
import { BarreraDeErrores } from './components/BarreraDeErrores';
import { ConfirmDialog } from './components/ConfirmDialog';
import type { AccionRestablecer } from './components/MenuRestablecer';
import { missingReason, recommendationStatus } from './lib/recommendations';
import { checkManualCommand } from './lib/command';
import { usarComando } from './hooks/useComando';
import { conPedidos, estaEncendido as ajusteEncendido } from './lib/motorComando';
import { matchesSearch } from './lib/format';
import { viewForOpenedGame } from './lib/navigation';
import {
  isDirty,
  quickCommandFor,
  recallTab,
  rememberTab,
  snapshotOf,
  summarizeImpacts,
} from './lib/gameView';
import { deniedReason, isDenied } from './lib/anticheat';

/**
 * Hueco mientras se descarga una vista cargada bajo demanda (`lazy`).
 *
 * Sin esto, el primer cambio a «Mis pruebas» o «Benchmarks» enseñaría un salto en
 * blanco mientras llega el trozo de código.
 */
function ViewLoading({ label }: { label: string }) {
  return (
    <div className="flex items-center justify-center h-64">
      <div className="text-center">
        <div className="w-10 h-10 border-4 border-action/30 border-t-[var(--action)] rounded-full animate-spin mx-auto mb-3"></div>
        <p className="text-sm text-[var(--text-muted)]">{label}</p>
      </div>
    </div>
  );
}

// Abre el diálogo de archivo, lee el JSON y se lo pasa al backend.
// (El navegador no expone la ruta real del archivo: hay que leer el contenido.)
const pickProfileFile = (onContent: (content: string, fileName: string) => void) => {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.json';
  input.onchange = async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    // El nombre se pasa para poder enseñarlo en el cuadro de vista previa.
    if (file) onContent(await file.text(), file.name);
  };
  input.click();
};

// Nombre apto para un archivo: el nombre del juego puede traer `/`, `:` o `.`
// ("Elden Ring: Nightreign"), y una barra convertiría el nombre en una carpeta.
const fileSafeName = (s: string) => s.replace(/[/\\:*?"<>|]/g, '-').trim();

// ==================== COMPONENTE PRINCIPAL ====================
function App() {
  // Estados principales
  const [sidebarView, setSidebarView] = useState<SidebarView>('library');
  /**
   * Estado del observador de procesos (subfase 5.3). Se consulta cada pocos segundos: es
   * barato (un comando síncrono sobre un estado en memoria) y no depende de la pantalla.
   */
  const [estadoObservador, setEstadoObservador] = useState<EstadoObservador | null>(null);
  useEffect(() => {
    let vivo = true;
    const leer = () => {
      api
        .estadoObservador()
        .then((e) => {
          if (vivo) setEstadoObservador(e);
        })
        .catch(() => {
          // Sin backend (tests, web) el interruptor se queda sin datos en vez de mentir.
        });
    };
    leer();
    const t = setInterval(leer, 5000);
    return () => {
      vivo = false;
      clearInterval(t);
    };
  }, []);
  const cambiarObservador = async (activo: boolean) => {
    try {
      await api.configurarObservador(activo);
      setEstadoObservador(await api.estadoObservador());
    } catch {
      // Si falla, se deja el estado anterior: el interruptor no debe mentir.
    }
  };
  /**
   * Ventana «Pruebas de este juego», abierta desde las acciones rápidas de la ficha.
   *
   * Es el historial del juego abierto, no el global: comparar 1 % lows entre juegos
   * distintos no significa nada. Aquí solo se consulta y se borra; para medir se lleva a
   * la pestaña Benchmarks, que sí sabe capturar y analizar.
   */
  const [pruebasDeJuego, setPruebasDeJuego] = useState<{
    appid: number;
    gameName: string;
  } | null>(null);
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
    linkAppid,
  } = useGameDetails({ getHwTags: () => hw?.tags || [], getGpuName: () => hw?.gpu || null });

  // Estados de selección
  const [selectedCommunityRecs, setSelectedCommunityRecs] = useState<Set<number>>(new Set());
  const [selectedEngineRecs, setSelectedEngineRecs] = useState<Set<number>>(new Set());

  // Estados de UI
  const [copied, setCopied] = useState(false);
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
    confirmImport,
    cancelImport,
    pendingImport,
    exportAll: exportAllProfiles,
    exportOne: exportProfile,
    update: updateProfileInStore,
    profilesForGame,
  } = useProfiles({
    getHwTags: () => hw?.tags || [],
    getGpuName: () => hw?.gpu || null,
    onNotify: setToast,
  });

  // Fase 3: Toggle para forzar Proton en juegos nativos
  const [forceProton, setForceProton] = useState(false);

  // Fase 4: Editor manual del comando.
  //
  // El comando manual es **por juego**, no global. Guardarlo en un único estado hacía que
  // traer el comando de Steam para Palworld lo mostrara también en todos los demás juegos,
  // que es un fallo de datos: cada juego tiene su propia línea de lanzamiento en Steam.
  // Se indexa por la clave del juego, que ya distingue un AppID de 0 (juego sin vincular)
  // y de un id no numérico (Lutris/Heroic).
  const [manualCommands, setManualCommands] = useState<Record<string, string>>({});
  // Qué juegos están con el editor manual activo. También por juego: activarlo en uno no
  // debe activarlo en el resto.
  const [manualEditingKeys, setManualEditingKeys] = useState<Record<string, boolean>>({});

  const clave = claveJuego(current);
  const manualCommand = comandoManualDe(manualCommands, clave);
  const useManualCommand = editandoAMano(manualEditingKeys, clave);

  const setManualCommand = (valor: string) =>
    setManualCommands((prev) => ({ ...prev, [clave]: valor }));
  const setUseManualCommand = (valor: boolean) =>
    setManualEditingKeys((prev) => ({ ...prev, [clave]: valor }));

  // Nombre del archivo que se está importando, solo para enseñarlo en la vista previa.
  const [importFileName, setImportFileName] = useState<string>('perfiles.json');

  // Instantánea del último estado guardado como perfil: sirve para avisar de que
  // hay cambios sin guardar cuando el usuario cierra la vista del juego.
  const [savedSnapshot, setSavedSnapshot] = useState<ReturnType<typeof snapshotOf> | null>(null);

  /**
   * Perfil cargado en el editor, si lo hay.
   *
   * Es **identidad**, no estado de configuración: qué perfil está cargado se guarda aquí;
   * qué comando hay, lo dice el motor. `savedSnapshot` sigue siendo la instantánea con la
   * que se comparan los cambios sin guardar.
   *
   * Nunca significa «Steam está usando esto»: cargar un perfil solo cambia Thoriapp.
   */
  const [perfilCargado, setPerfilCargado] = useState<{ id: string; nombre: string } | null>(
    null
  );
  /**
   * Perfil que hay que cargar en cuanto el juego termine de abrirse.
   *
   * Cargar depende del juego abierto (el motor analiza con el appid abierto), así que no se
   * puede adelantar. Se guarda aquí y un efecto lo aplica cuando `current` ya es el juego
   * del perfil; si la apertura falla, el pendiente se descarta y no se carga nada.
   */
  const [perfilPendiente, setPerfilPendiente] = useState<Profile | null>(null);
  /**
   * La instantánea la fija quien carga o guarda, no este efecto: aquí solo se limpia.
   * Se declara antes para poder usarlo al cambiar de juego.
   */

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
  const [showSaveProfileModal, setShowSaveProfileModal] = useState(false);
  const [newProfileName, setNewProfileName] = useState('');
  const [newProfileNotes, setNewProfileNotes] = useState('');

  // Fase 6: Benchmarks
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

  // openGame delega en useGameDetails y reinicia la selección de esta vista
  /**
   * Abre un juego, avisando antes si se van a perder cambios sin guardar.
   *
   * Es el único camino de apertura: si hubiera dos (uno que pregunta y otro que no), el
   * que no pregunta acabaría usándose. Cambiar de juego descarta la configuración actual
   * igual que cargar un perfil, así que se trata igual.
   */
  const pedirAbrirJuego = (g: Game) => {
    const mismoJuego = current && (current.appid ?? 0) === (g.appid ?? 0);
    if (!mismoJuego && current && hasUnsavedChanges) {
      ask({
        title: 'Cambiar de juego',
        message:
          'Tienes cambios sin guardar en el juego actual. Cambia de juego para descartarlos ' +
          'o vuelve a guardar el perfil antes.',
        confirmLabel: 'Descartar y cambiar',
        accionesExtra: [
          {
            label: 'Guardar y continuar',
            tone: 'primary',
            onSelect: () => {
              if (!perfilCargado) {
                setToast('No hay un perfil cargado donde guardar; usa «Guardar perfil»');
                return;
              }
              void (async () => {
                const ok = await guardarEnPerfilCargado();
                if (ok) await abrir(g);
              })();
            },
          },
        ],
        onConfirm: () => void abrir(g),
      });
      return;
    }
    void abrir(g);
  };

  const abrir = async (g: Game) => {
    setSelectedCommunityRecs(new Set());
    setSelectedEngineRecs(new Set());
    setForceProton(false);
    // Sin perfil guardado todavía: no hay cambios que perder.
    setSavedSnapshot(null);
    // El perfil cargado era del juego anterior: la identidad no viaja entre juegos.
    setPerfilCargado(null);
    // Reabrir un juego en la pestaña donde se quedó, en vez de siempre en
    // «Optimización» (quien revisa el anti-cheat lo agradece).
    setView(recallTab(g.appid ?? 0));
    await loadGameDetails(g);
  };

  /** Cambia de pestaña y la recuerda para este juego. */
  const selectView = (next: View) => {
    setView(next);
    if (next === 'compatibility' || next === 'recommendations') {
      rememberTab(current?.appid ?? 0, next);
    }
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

  /**
   * Pide encender o apagar un ajuste.
   *
   * No compone el comando: le pide al motor que lo haga y adopta lo que confirme. Si el
   * motor no puede (hay un conflicto, la sintaxis no cuadra), el comando no cambia y se
   * ve el motivo.
   */
  const toggleRule = (index: number) => {
    if (!rules) return;
    const param = rules.params[index];
    if (missing(param)) return;

    const estado = motor.revision.estados.find((e) => e.indice === index) ?? null;
    // `parcial` está puesto aunque el toggle no se lea como «activo»: pulsarlo apaga lo
    // que Thoriapp sí controla, que es lo que el usuario espera.
    const estaPuesto = estado?.estado === 'activo' || estado?.estado === 'parcial';
    motor.fijarAjuste(index, !estaPuesto);
  };

  /**
   * Índice del catálogo efectivo de una recomendación, **según el motor**.
   *
   * Aquí se calculaba `params.length + indexOf`, y estaba mal: `catalogo_efectivo` en Rust
   * salta las recomendaciones vacías, `%command%` y las que ya están en el perfil del juego.
   * El perfil genérico ya trae `PROTON_LOCAL_SHADER_CACHE=1` y `mangohud`, así que desde la
   * primera descartada todos los índices se desplazaban y el toggle escribía en otro ajuste.
   * Era el `WINE_FULLSCREEN_FSR=1` que parecía no responder.
   *
   * Ahora el mapa viene de Rust (`catalogo_de_ajustes`), que es quien decide esos saltos.
   */
  const indiceDeRecomendacion = (texto: string): number =>
    indicesDelMotor.get(texto.trim()) ?? -1;

  /**
   * ¿La recomendación está ya puesta en el comando?
   *
   * Se pregunta al motor, no a un `Set` local: el estado que vale es el del comando
   * confirmado. Con una copia en React, desactivar una recomendación que se importó de Steam
   * no la quitaba del comando y la tarjeta seguía diciendo que estaba activa.
   */
  const recomendacionPuesta = (texto: string): boolean => {
    const indice = indiceDeRecomendacion(texto);
    if (indice < 0) return false;
    const estado = motor.revision.estados.find((e) => e.indice === indice);
    return estado?.estado === 'activo' || estado?.estado === 'parcial';
  };

  const toggleCommunityRec = (index: number) => {
    const texto = communityRecommendations[index]?.parameter;
    if (texto) {
      // La recomendación se aplica en el motor, como cualquier otro ajuste: es lo que hace
      // que el comando de arriba la recoja.
      const indice = indiceDeRecomendacion(texto);
      // Sin índice no hay ajuste que tocar. Inventar uno (el `-1` de antes) hacía que el
      // toggle no respondiera sin decir por qué.
      if (indice < 0) return;
      motor.fijarAjuste(indice, !recomendacionPuesta(texto));
      return;
    }
    const newSet = new Set(selectedCommunityRecs);
    if (newSet.has(index)) {
      newSet.delete(index);
    } else {
      newSet.add(index);
    }
    setSelectedCommunityRecs(newSet);
  };

  const toggleEngineRec = (index: number) => {
    const texto = engineRecommendations[index];
    if (texto) {
      const indice = indiceDeRecomendacion(texto);
      // Sin índice no hay ajuste que tocar. Inventar uno (el `-1` de antes) hacía que el
      // toggle no respondiera sin decir por qué.
      if (indice < 0) return;
      motor.fijarAjuste(indice, !recomendacionPuesta(texto));
      return;
    }
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
  /**
   * Problema detectado en el comando manual, o `''` si está bien o no aplica.
   *
   * Se calcula aparte para no repetir la comprobación en `launchLine`, en la caja
   * del comando y al guardar el perfil: las tres tienen que decir lo mismo.
   */
  const manualCommandProblem = useMemo(() => {
    if (!useManualCommand) return '';
    const text = manualCommand.trim();
    if (!text) return '';
    const check = checkManualCommand(text);
    return check.safe ? '' : check.reason;
  }, [useManualCommand, manualCommand]);


  /**
   * El motor de comandos es la fuente de verdad.
   *
   * Sustituye al montaje a mano que había aquí (`commandParts` + `joinCommand`), que era
   * justo lo que impedía seguir editando después de importar: al pasar a «modo manual» el
   * texto dejaba de venir de los toggles.
   *
   * `rules` es la señal de «el juego ya está cargado»: antes de eso analizar contra un
   * catálogo vacío diría que no reconoce nada y se perderían los ajustes reales.
   */
  /**
   * Texto del que parte el motor al abrir un juego.
   *
   * `%command%` es el punto de partida limpio: los ajustes se encienden desde los toggles,
   * y lo que añada el usuario se conserva a partir de ahí. Un perfil guardado se carga
   * por su acción explícita, no por el mero hecho de abrir el juego: así abrir la ficha no
   * sobrescribe lo que haya en Steam sin decirlo.
   */
  const comandoInicial = rules === null ? null : '%command%';

  /**
   * Argumentos que se ofrecen como recomendación (motor y hardware) y que no están en
   * `rules.params`. Van al motor como catálogo efectivo: así activarlas es una operación
   * del motor y el comando de arriba las recoge. Antes solo se marcaban en un `Set` local y
   * el comando no cambiaba nunca.
   *
   * Se deduplica por texto porque dos fuentes pueden recomendar lo mismo (`engine` y
   * `hardware`); dos definiciones con el mismo valor hacen que el motor lo vea ambiguo y
   * activar una apagaría la otra.
   */
  const recomendacionesVisibles = useMemo(() => {
    const vistas = new Set<string>();
    const salida: string[] = [];
    for (const rec of engineRecommendations) {
      const texto = rec.trim();
      if (!texto || vistas.has(texto)) continue;
      vistas.add(texto);
      salida.push(texto);
    }
    for (const rec of communityRecommendations) {
      const texto = rec.parameter.trim();
      if (!texto || vistas.has(texto)) continue;
      vistas.add(texto);
      salida.push(texto);
    }
    return salida;
  }, [engineRecommendations, communityRecommendations]);

  // El mapa de índices tiene que salir del motor (ver el hook): calcularlo aquí con
  // `params.length + indexOf` desplazaba todo lo que iba detrás de una recomendación
  // descartada, y los toggles escribían en otro ajuste.
  const indicesDelMotor = useCatalogoDeAjustes(current?.appid, recomendacionesVisibles);

  const motor = usarComando(
    current?.appid,
    rules?.params ?? [],
    comandoInicial,
    rules !== null,
    recomendacionesVisibles,
  );

  /**
   * Ajustes encendidos, **derivados del motor**.
   *
   * Antes era un `useState` que la UI mantenía por su cuenta; eso es lo que permitía que
   * los toggles y el comando se desincronizaran (el fallo original: importabas de Steam,
   * `enabled` no cambiaba y los toggles movían un array que ya no se usaba).
   *
   * Ahora el motor dice qué está puesto; esto solo lo aplana para el resto de la UI.
   */
  const enabled = useMemo<boolean[]>(() => {
    const total = rules?.params.length ?? 0;
    const lista = new Array(total).fill(false);
    for (const estado of motor.revision.estados) {
      if (estado.indice < total) lista[estado.indice] = ajusteEncendido(estado);
    }
    return lista;
    // `motor.revision.estados` es la única entrada: si cambia, cambian los toggles.
  }, [rules?.params.length, motor.revision.estados]);

  /**
   * El comando que se pinta, se copia y se guarda.
   *
   * Sale **solo** del motor: es lo último que confirmó. Nunca se compone en React a partir
   * de los toggles, que es lo que hacía que el texto y los controles pudieran no
   * corresponderse.
   */
  const launchLine = (): string => {
    if (blockedByAnticheat) return blockedMessage;
    return motor.comando;
  };

  /**
   * Lo que Steam tiene configurado para este juego.
   *
   * Solo lectura: la app nunca escribe en `localconfig.vdf` (Steam reescribe
   * `LaunchOptions` al cerrarse). Si Steam responde con otro comando, se ofrece traerlo al
   * editor; no se aplica solo. La lectura se hace una vez por juego: togglear no la
   * repite.
   */
  const steam = usarOpcionesSteam(current?.appid, {
    importado: motor.originalImportado,
    actual: motor.comando,
  });

  /**
   * La configuración vigente se le manda al observador para que, si hay un juego vivo, la
   * guarde con su instante. No es evidencia de nada: la sesión la marca como **posterior al
   * arranque**, porque cuando el observador ve el proceso el juego ya lleva rato corriendo.
   *
   * Se manda a propósito al cambiar de perfil o al editar el comando, que es justo el
   * momento en que la memoria podría quedarse con lo anterior. Un fallo no puede romper el
   * cambio de perfil: el observador se queda con lo último que supo.
   */
  useEffect(() => {
    const comando = motor.comando.trim();
    api
      .fijarConfiguracionDeSesion({
        comando: comando === '' ? null : comando,
        procedencia: comando === '' ? null : 'Thoriapp',
        perfil: perfilCargado?.nombre ?? null,
        opcionesSteam: steam.opciones?.opciones || null,
      })
      .catch(() => {
        // Sin backend no hay observador al que contarle nada.
      });
  }, [motor.comando, perfilCargado?.nombre, steam.opciones?.opciones]);


  /**
   * Relación del comando actual con Steam.
   *
   * Se distinguen cinco casos porque «no coincide» no siempre es un problema:
   *
   * - `steam_cambiado`: Steam tiene algo distinto de lo que se importó. Eso sí es un aviso
   *   naranja, porque alguien lo cambió por fuera.
   * - `local`: el usuario tocó ajustes. Es lo normal y no debe mirar como un error: por eso
   *   va en morado, no en naranja.
   * - `sincronizado`: coincide con Steam tal cual.
   * - `pendiente`: se está leyendo. `desconocido`: no se pudo leer, no se afirma nada.
   */
  const sincronizacion = useMemo<
    { tipo: 'sincronizado' | 'local' | 'steam_cambiado' | 'desconocido' | 'pendiente' }
  >(() => {
    if (steam.estado === 'cargando') return { tipo: 'pendiente' };
    if (steam.estado !== 'ok' || !steam.opciones) return { tipo: 'desconocido' };

    const enSteam = steam.opciones.opciones.trim();
    const actual = motor.comando.trim();
    const importado = motor.originalImportado?.trim() ?? null;

    // Steam sigue teniendo lo mismo que se importó, y el usuario lo ha cambiado aquí.
    if (importado !== null && enSteam === importado && actual !== enSteam) {
      return { tipo: 'local' };
    }
    if (enSteam === actual) return { tipo: 'sincronizado' };
    // Steam cambió respecto a lo que se importó: esto no lo ha hecho el usuario aquí.
    if (importado !== null && enSteam !== importado) return { tipo: 'steam_cambiado' };
    // Sin importación previa no se puede saber quién cambió qué.
    return { tipo: 'steam_cambiado' };
  }, [steam.estado, steam.opciones, motor.comando, motor.originalImportado]);

  /**
   * Acciones del menú de restablecimiento.
   *
   * Se construyen aquí, con la disponibilidad real de cada una: lo que todavía no se puede
   * hacer sale deshabilitado con el motivo, en vez de desaparecer o de fingir que funciona.
   */
  /**
   * Instantánea de la configuración actual, para comparar con la del perfil cargado.
   *
   * Sale del motor (`enabled` se deriva de `motor.revision.estados`), no de los toggles:
   * así volver manualmente al estado guardado hace que el aviso desaparezca solo.
   */
  const instantaneaActual = snapshotOf({
    enabled,
    selectedCommunityRecs,
    selectedEngineRecs,
    forceProton,
    manualCommand,
    useManualCommand,
    // El comando literal entra en la comparación: dos configuraciones con los mismos
    // toggles y textos distintos (argumentos desconocidos) sí son cambios.
    comando: motor.revision.comando,
  });

  /**
   * Por qué no se puede guardar ahora mismo, o `null` si sí se puede.
   *
   * Guardar mientras el motor tiene una operación en vuelo escribiría una versión que el
   * usuario ya no está viendo (o una que nunca se confirmó). Es preferible esperar a una
   * fracción de segundo que a guardar un comando equivocado.
   */
  const motivoNoGuardable = (): string | null => {
    if (!current) return 'No hay ningún juego abierto.';
    if (blockedByAnticheat) return 'No se guardan perfiles de juegos bloqueados por el anti-cheat';
    if (motor.cargando || motor.ajustesPendientes !== null) {
      return 'El motor todavía está aplicando un cambio. Espera a que termine.';
    }
    return null;
  };

  /**
   * Hay cambios sin guardar respecto a la instantánea del perfil cargado.
   *
   * Se compara **estado contra estado**, no «si se ha pulsado un toggle»: volver a mano al
   * valor guardado hace que el aviso desaparezca, y eso es lo que espera cualquiera.
   */
  const hasUnsavedChanges = isDirty(instantaneaActual, savedSnapshot);

  /**
   * Perfiles del juego abierto, derivados de la colección única de `useProfiles`.
   *
   * No es una segunda fuente: si se mantuviera una lista aparte, borrar en la biblioteca
   * global dejaría el perfil en el sidebar (o al revés). Se filtra por appid exacto.
   */
  const perfilesDelJuego = profilesForGame(current?.appid);

  const accionesRestablecer = useMemo<AccionRestablecer[]>(() => {
    const hayImportado = motor.originalImportado !== null;
    return [
      {
        id: 'original',
        titulo: 'Restaurar comando importado',
        detalle: 'Vuelve al texto exacto que se trajo de Steam, con sus ajustes originales.',
        disponible: hayImportado,
        motivoNoDisponible: 'Todavía no se ha importado ningún comando de Steam.',
      },
      {
        id: 'perfil',
        titulo: 'Restaurar perfil cargado',
        detalle: perfilCargado
          ? `Vuelve a «${perfilCargado.nombre}» desde su última versión guardada.`
          : 'Carga los ajustes de un perfil que hayas guardado.',
        // Restaurar es la **misma** operación que cargar, no una ruta aparte: si hubiera
        // dos formas de generar el comando podrían dar resultados distintos.
        disponible:
          perfilCargado !== null &&
          perfilesDelJuego.some((p) => p.id === perfilCargado.id) &&
          hasUnsavedChanges,
        motivoNoDisponible:
          perfilCargado === null
            ? 'No hay ningún perfil cargado. Elígelo en «Perfiles del juego».'
            : !perfilesDelJuego.some((p) => p.id === perfilCargado.id)
              ? 'Ese perfil ya no existe.'
              : 'No hay cambios sin guardar: ya estás en el perfil guardado.',
      },
      {
        id: 'limpiar',
        titulo: 'Limpiar todos los argumentos',
        detalle: 'Quita los ajustes de Thoriapp y deja el comando base.',
        disponible: motor.comando !== '',
        motivoNoDisponible: 'Todavía no hay comando cargado.',
      },
    ];
  }, [
    motor.originalImportado,
    motor.comando,
    perfilCargado,
    perfilesDelJuego,
    hasUnsavedChanges,
  ]);

  /**
   * Ejecuta una acción del menú.
   *
   * Las que descartan trabajo piden confirmación: restaurar el importado pierde lo que el
   * usuario haya cambiado, y limpiar se lleva todos los ajustes. Sin el aviso, un clic
   * accidental borra el comando que estaba probando.
   */
  const ejecutarRestablecer = (id: string) => {
    if (id === 'original') {
      const yaEsOriginal = motor.comando === motor.originalImportado;
      if (yaEsOriginal) return;
      ask({
        title: 'Restaurar el comando importado',
        message:
          'Se perderán los cambios que hayas hecho con los controles desde que trajiste el comando de Steam. Steam no se modifica.',
        confirmLabel: 'Restaurar original',
        tone: 'primary',
        onConfirm: () => {
          void motor.restaurarImportado();
          setToast('Comando original importado restaurado');
        },
      });
      return;
    }
    if (id === 'perfil' && perfilCargado) {
      const actual = perfilesDelJuego.find((p) => p.id === perfilCargado.id);
      if (!actual) return;
      // Se reutiliza la carga normal: así la confirmación, la validación y el respaldo del
      // motor son exactamente los mismos que al pulsar «Cargar perfil».
      pedirCargarPerfil(actual);
      return;
    }
    if (id === 'limpiar') {
      const hayAjustes = motor.revision.estados.some(ajusteEncendido);
      if (!hayAjustes) return;
      ask({
        title: 'Limpiar todos los argumentos',
        message:
          'Se quitarán todos los ajustes de Thoriapp. Los argumentos propios del juego que no gestiona la app se conservan.',
        confirmLabel: 'Limpiar',
        tone: 'danger',
        onConfirm: () => {
          void motor.limpiarAjustes();
          setToast('Ajustes limpiados');
        },
      });
    }
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

  /**
   * Payload del perfil tal como está ahora.
   *
   * El comando es **el que confirmó el motor**, literal: es lo único que garantiza que el
   * perfil diga lo mismo que la caja del comando. Reconstruirlo desde `enabled` perdería
   * los argumentos que Thoriapp no conoce y podría reordenar el resto.
   */
  const payloadPerfil = (nombre: string, notas: string | null) => ({
    name: nombre,
    appid: current!.appid,
    gameName: current!.name,
    // Array plano y completo del tamaño de params: cualquier hueco se serializaría como
    // `null` y Rust rechazaría el perfil.
    enabledParams: rules ? rules.params.map((_, i) => enabled[i] === true) : [],
    selectedCommunityRecs: Array.from(selectedCommunityRecs),
    selectedEngineRecs: Array.from(selectedEngineRecs),
    forceProton,
    manualCommand: manualCommand || null,
    useManualCommand,
    notes: notas,
    command: motor.revision.comando,
  });

  /** Marca el estado actual como «lo guardado». */
  const fijarComoGuardado = () => setSavedSnapshot(instantaneaActual);

  /**
   * Actualiza el perfil cargado sin cambiar su identidad.
   *
   * Conserva `id` y `created_at` (los pone Rust); solo cambia lo que se ha editado. Si no
   * hay perfil cargado no hace nada y lo dice: crear un perfil es otra acción, y hacerlo
   * aquí sobrescribiría en silencio, que es justo lo que no debe pasar.
   */
  const guardarEnPerfilCargado = async (): Promise<boolean> => {
    const motivo = motivoNoGuardable();
    if (motivo) {
      setToast(motivo);
      return false;
    }
    if (!perfilCargado) {
      setToast('No hay un perfil cargado: usa «Guardar perfil» para crear uno nuevo');
      return false;
    }
    const original = profiles.find((p) => p.id === perfilCargado.id);
    if (!original) {
      setToast('El perfil cargado ya no existe; guárdalo como uno nuevo');
      return false;
    }
    const ok = await updateProfileInStore(
      original.id,
      {
        name: original.name,
        enabledParams: rules ? rules.params.map((_, i) => enabled[i] === true) : [],
        selectedCommunityRecs: Array.from(selectedCommunityRecs),
        selectedEngineRecs: Array.from(selectedEngineRecs),
        forceProton,
        manualCommand: manualCommand || null,
        useManualCommand,
        notes: original.notes ?? null,
        command: motor.revision.comando,
      },
      original.name
    );
    if (ok) {
      fijarComoGuardado();
      return true;
    }
    return false;
  };

  /**
   * Guarda desde el modal de «Guardar perfil».
   *
   * Con un perfil cargado y su mismo nombre se **actualiza**; en cualquier otro caso se crea
   * uno nuevo. Nunca sobrescribe un perfil distinto sin que el usuario lo haya pedido: el
   * único que se actualiza es el que está cargado.
   */
  const saveProfile = async () => {
    if (!current || !newProfileName.trim()) return;
    const motivo = motivoNoGuardable();
    if (motivo) {
      setToast(motivo);
      return;
    }
    const payload = payloadPerfil(newProfileName, newProfileNotes || null);
    const ok = await saveProfileInStore(payload, newProfileName);
    if (!ok) return;

    // Guardar como nuevo con otro nombre deja de ser el perfil cargado: el que está en el
    // editor ahora es el nuevo. Así «Restaurar perfil cargado» apunta a lo que se ve.
    const recienGuardados = await api.loadProfiles().catch(() => null);
    const guardado = recienGuardados?.find(
      (p) => p.appid === current.appid && p.name === newProfileName
    );
    if (guardado) setPerfilCargado({ id: guardado.id, nombre: guardado.name });

    fijarComoGuardado();
    setShowSaveProfileModal(false);
    setNewProfileName('');
    setNewProfileNotes('');
  };

  /**
   * Comando copiable de cada juego de la biblioteca, tomado de sus perfiles
   * guardados: permite copiar desde la tarjeta sin abrir la vista.
   */
  const quickCommands = useMemo(() => {
    const map: Record<number, string> = {};
    for (const profile of profiles) {
      if (map[profile.appid]) continue;
      const command = quickCommandFor(profile.appid, profiles, profileCommands);
      if (command) map[profile.appid] = command;
    }
    return map;
  }, [profiles, profileCommands]);

  const copyQuickCommand = async (command: string) => {
    try {
      await navigator.clipboard.writeText(command);
      setToast('Comando copiado: ' + command);
    } catch (e) {
      console.error(e);
      setToast('No se pudo copiar el comando');
    }
  };

  /**
   * Instantánea de un perfil guardado, en el mismo formato que la actual.
   *
   * El comando literal del perfil se compara **también**: dos configuraciones pueden tener
   * los mismos toggles y comandos distintos (argumentos que Thoriapp no conoce), y en ese
   * caso hay cambios. Comparar solo los toggles daba «guardado» a un comando distinto.
   */
  const instantaneaDePerfil = (p: Profile) =>
    snapshotOf({
      enabled: p.enabled_params ?? [],
      selectedCommunityRecs: new Set(p.selected_community_recs ?? []),
      selectedEngineRecs: new Set(p.selected_engine_recs ?? []),
      forceProton: p.force_proton,
      manualCommand: p.manual_command ?? '',
      useManualCommand: p.use_manual_command,
      comando: p.command ?? undefined,
    });



  /**
   * Carga un perfil en el juego abierto.
   *
   * Es la operación de restauración: la misma que usa «Restaurar perfil cargado» y la que
   * usará el menú de restablecimiento, para no tener tres rutas que puedan divergir.
   *
   * Orden que importa:
   * 1. el perfil tiene que ser de **este** juego;
   * 2. se pide al motor que analice su comando (`motor.cargarPerfil`) y se espera;
   * 3. **solo si el motor confirmó** se actualizan identidad, `savedSnapshot` y Proton.
   *
   * Si el motor falla no se toca nada: ni comando, ni toggles, ni el perfil cargado
   * anterior, ni la instantánea. Es lo que hace la carga atómica desde fuera.
   */
  const cargarPerfilEnJuego = async (perfil: Profile) => {
    if (!current) return false;
    if (perfil.appid !== current.appid) {
      setToast('Ese perfil es de otro juego y no se puede cargar aquí');
      return false;
    }
    // El comando del perfil: el literal si lo guardó, o el reconstruido por
    // `useProfiles` para los perfiles antiguos. No se reconstruye aquí: habría una segunda
    // forma de generar el mismo comando y ambas podrían discrepar.
    const comando = (perfil.command ?? profileCommands[perfil.id] ?? '').trim();
    if (!comando) {
      setToast('Este perfil no tiene un comando que se pueda aplicar');
      return false;
    }

    // Un perfil sin comando guardado se reconstruye desde sus ajustes: se dice, porque el
    // texto resultante no es necesariamente el que se guardó (el orden puede cambiar).
    if (!perfil.command) {
      setToast(`Perfil antiguo: el comando de «${perfil.name}» se reconstruye desde sus ajustes`);
    }

    const confirmado = await motor.cargarPerfil(comando);
    if (confirmado === null) {
      // `motor.fallo` ya está puesto: el aviso de error lo pinta la barrera del motor.
      setToast('No se pudo cargar el perfil; la configuración anterior sigue igual');
      return false;
    }

    // El modo Proton se restaura como cualquier otro campo del perfil.
    setForceProton(perfil.force_proton);
    setPerfilCargado({ id: perfil.id, nombre: perfil.name });
    // La instantánea de referencia pasa a ser la del perfil: cargarlo no es un cambio sin
    // guardar, es volver a lo guardado.
    setSavedSnapshot(instantaneaDePerfil(perfil));
    setToast(`Perfil «${perfil.name}» cargado. Steam no se ha modificado.`);
    return true;
  };

  /**
   * Aplica el perfil pendiente en cuanto su juego está abierto y el motor está listo.
   *
   * Se espera a que el motor tenga análisis del comando (`revision.analisis`): cargar antes
   * pediría al motor analizar con un juego a medio abrir.
   */
  useEffect(() => {
    if (!perfilPendiente || !current) return;
    if (perfilPendiente.appid !== current.appid) return;
    if (!motor.revision.analisis) return;
    const pendiente = perfilPendiente;
    setPerfilPendiente(null);
    void cargarPerfilEnJuego(pendiente);
    // `cargarPerfilEnJuego` se redefine en cada render; lo que importa es que el pendiente,
    // el juego abierto y el análisis estén listos una sola vez.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [perfilPendiente, current, motor.revision.analisis]);

  /**
   * Abre el juego del perfil **y** carga el perfil en él.
   *
   * El perfil se carga una vez abierto el juego, no antes: `cargarPerfilEnJuego` comprueba
   * que el appid coincide con el juego abierto, y mientras no esté abierto esa comparación
   * no dice nada. Si el juego no se puede abrir, no se carga nada.
   */
  const abrirJuegoYCargarPerfil = async (perfil: Profile) => {
    // Se busca en la biblioteca completa (Steam + launchers externos), no solo en Steam:
    // un perfil puede ser de un juego añadido a mano.
    const juego = combinedLibrary.find((g) => (g.appid ?? 0) === perfil.appid);
    if (!juego) {
      setToast('Ese juego ya no está en la biblioteca de Steam');
      return;
    }
    // Si ya hay otro juego abierto con cambios, se pregunta antes de perderlos: cambiar de
    // juego descarta la configuración actual igual que cargar un perfil.
    if (current && current.appid !== juego.appid && hasUnsavedChanges) {
      ask({
        title: 'Cambiar de juego',
        message:
          'Tienes cambios sin guardar en el juego actual. Cambiar de juego los descarta. ' +
          'Guárdalos antes desde «Guardar perfil».',
        confirmLabel: 'Cambiar y cargar',
        onConfirm: () => {
          setPerfilPendiente(perfil);
          void abrir(juego);
        },
      });
      return;
    }
    setPerfilPendiente(perfil);
    await abrir(juego);
  };

  /**
   * Abre el juego de un perfil sin cargarlo.
   *
   * Navegar no cambia ninguna configuración; aun así se pregunta si se van a perder
   * cambios, porque abrir otro juego también descarta el estado actual.
   */
  const abrirJuegoDesdePerfil = (appid: number) => {
    const juego = combinedLibrary.find((g) => (g.appid ?? 0) === appid);
    if (!juego) {
      setToast('Ese juego ya no está en la biblioteca de Steam');
      return;
    }
    if (current && current.appid !== appid && hasUnsavedChanges) {
      ask({
        title: 'Cambiar de juego',
        message: 'Tienes cambios sin guardar en el juego actual y se descartarán.',
        confirmLabel: 'Cambiar de juego',
        onConfirm: () => void abrir(juego),
      });
      return;
    }
    void abrir(juego);
  };

  /**
   * Punto único para «cargar este perfil», con la protección de cambios sin guardar.
   *
   * Seleccionar en el desplegable no pasa por aquí: solo pulsar «Cargar» (o una acción
   * equivalente) puede descartar algo, y por eso pregunta.
   */
  const pedirCargarPerfil = (perfil: Profile) => {
    const yaEsElMismo = perfilCargado?.id === perfil.id;
    const hayCambios = hasUnsavedChanges && !yaEsElMismo;
    if (!hayCambios) {
      void cargarPerfilEnJuego(perfil);
      return;
    }
    ask({
      title: `Cargar «${perfil.name}»`,
      message:
        'Tienes cambios sin guardar en la configuración actual. Puedes guardarlos en su ' +
        'perfil antes de cargar este, descartarlos, o cancelar y seguir como estabas. ' +
        'Steam no se modifica en ningún caso.',
      confirmLabel: 'Cancelar',
      tone: 'primary',
      // «Cancelar» cierra sin hacer nada: el botón principal del diálogo es justamente ese
      // para que el clic por inercia no descarte trabajo.
      onConfirm: () => {},
      accionesExtra: [
        {
          label: 'Guardar y continuar',
          tone: 'primary',
          onSelect: () => {
            // Solo tiene sentido si hay un perfil cargado donde guardar; sin él, se
            // descarta la opción en vez de crear un perfil a espaldas del usuario.
            if (!perfilCargado) {
              setToast('No hay un perfil cargado donde guardar; usa «Guardar perfil» en el comando');
              return;
            }
            void (async () => {
              const ok = await guardarEnPerfilCargado();
              if (ok) void cargarPerfilEnJuego(perfil);
            })();
          },
        },
        {
          label: 'Descartar y continuar',
          tone: 'danger',
          onSelect: () => void cargarPerfilEnJuego(perfil),
        },
      ],
    });
  };

  /** Resumen de impactos para el encabezado de la vista de juego. */
  const impacts = rules
    ? summarizeImpacts(rules.params, enabled)
    : { count: 0, worst: null, label: 'Sin ajustes activos', tone: 'none' as const };

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
        distro={hw?.distro}
        onNavigate={(v) => { setSidebarView(v); closeGame(); }}
        observador={estadoObservador}
        onCambiarObservador={cambiarObservador}
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
          // La vista de juego solo se pinta dentro de la biblioteca: si se busca desde
          // pruebas, perfiles o benchmarks hay que volver a ella o el clic no hace nada.
          onPickSuggestion={(g) => { setSidebarView(viewForOpenedGame(sidebarView)); pedirAbrirJuego(g); clearSearch(); }}
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
                  onOpenGame={pedirAbrirJuego}
                  quickCommands={quickCommands}
                  onCopyCommand={copyQuickCommand}
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
                  <div className="w-12 h-12 border-4 border-action/30 border-t-[var(--action)] rounded-full animate-spin mx-auto mb-4"></div>
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
                <div className="max-w-lg mx-auto p-6 rounded-xl bg-error/10 border-2 border-error/40">
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
              // Un fallo al pintar la ficha no puede dejar la ventana en negro: la
              // barrera enseña el error y el resto de la app sigue usable.
              <BarreraDeErrores zona={`la ficha de ${current.name}`} onReintentar={() => current && void abrir(current)}>
              <motion.div key="game" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <div className="max-w-6xl mx-auto">
                  {/* Breadcrumb navegable: antes era texto plano que parecía un
                      enlace. Volver a la biblioteca es la salida natural de la vista. */}
                  <div className="flex items-center justify-between gap-3 mb-4">
                    <p className="text-xs text-[var(--text-muted)]">
                      <button
                        onClick={closeGame}
                        className="hover:text-[var(--action)] hover:underline transition-colors"
                      >
                        ← Biblioteca
                      </button>
                      <span className="mx-2">/</span>
                      <strong className="text-white">{current.name}</strong>
                    </p>
                    {hasUnsavedChanges && (
                      <span
                        className="flex items-center gap-1.5 text-xs font-semibold text-[var(--caution)]"
                        title="Has cambiado ajustes desde el último perfil guardado"
                      >
                        <span className="w-2 h-2 rounded-full bg-[var(--caution)]" />
                        Cambios sin guardar
                      </span>
                    )}
                  </div>

                  <CommandBox
                    gameName={current.name}
                    launchCommand={launchLine()}
                    // Bloqueado por anti-cheat: se oculta cualquier opción activa
                    // para no sugerir que el comando existe.
                    activeCount={
                      blockedByAnticheat ? 0 : motor.revision.estados.filter(ajusteEncendido).length
                    }
                    blocked={blockedByAnticheat}
                    blockedReason={blockedMessage}
                    copied={copied}
                    manualEditing={useManualCommand}
                    manualCommand={manualCommand}
                    manualCommandError={manualCommandProblem}
                    info={info}
                    // Un juego de Lutris puede tener el AppID resuelto por
                    // nombre: la etiqueta de ProtonDB enlaza con esa ficha.
                    appid={info.appid ?? current.appid}
                    // Elegir el juego a mano solo tiene sentido sin AppID: con él
                    // lo que falta es el nivel, no la identidad.
                    onLinkAppid={
                      current.appid === 0
                        ? (id) => {
                            void linkAppid(current, id);
                            setToast('Juego vinculado con su ficha de Steam');
                          }
                        : undefined
                    }
                    env={runtimeEnv}
                    launcher={current.launcher}
                    preparandoProton={forceProton}
                    // Todo lo visual sale del motor: procedencia, chips de ajustes
                    // reconocidos y el original guardado. Nada se deduce en React.
                    procedencia={motor.revision.procedencia}
                    perfilCargado={perfilCargado?.nombre ?? null}
                    cambiosSinGuardar={hasUnsavedChanges}
                    // El valor sale del motor (`e.valor`), no de `rules.params[e.indice]`:
                    // las recomendaciones no están en `params`, así que aquel acceso daba
                    // `undefined` y el chip acababa enseñando el índice interno («9»).
                    ajustesActivos={motor.revision.estados
                      .filter(ajusteEncendido)
                      .map((e) => ({ valor: e.valor, estado: e.estado }))}
                    comandoImportado={motor.originalImportado}
                    sincronizacion={sincronizacion}
                    actualizando={motor.cargando}
                    fallo={motor.fallo}
                    accionesRestablecer={accionesRestablecer}
                    onRestablecer={ejecutarRestablecer}
                    onCopy={copyCmd}
                    onSaveProfile={() => setShowSaveProfileModal(true)}
                    // Confirmar es el único camino: el texto pasa por el motor, que valida
                    // y actualiza la revisión confirmada; los ajustes se re-sincronizan
                    // solos porque el motor es la única fuente. Cancelar no llega aquí.
                    onConfirmarEdicion={(valor) => {
                      setManualCommand(valor);
                      setUseManualCommand(true);
                      void motor.importar(valor);
                    }}
                    // Steam ya puede tener otro comando: se ofrece traerlo, sin tocar
                    // Steam ni sobrescribir el editor por nuestra cuenta.
                    steamOpciones={
                      sincronizacion.tipo === 'steam_cambiado' ? steam.opciones : null
                    }
                    onImportarDeSteam={() => {
                      if (!steam.opciones) return;
                      // Se importa al motor, **sin** pasar a modo manual: es justo lo
                      // que rompía la edición visual. Los ajustes reconocidos quedan
                      // activos y lo desconocido se conserva.
                      void motor.importar(steam.opciones.opciones);
                      setToast('Comando de Steam traído a Thoriapp (Steam no se ha modificado)');
                    }}
                  />

                  {/* Argumentos que Thoriapp no entiende: se enseñan y se pueden quitar
                      uno a uno. Van justo debajo del comando porque son parte de él, no un
                      ajuste. */}
                  <ArgumentosConservados
                    segmentos={motor.revision.analisis?.segmentos ?? []}
                    deshabilitado={motor.cargando}
                    onQuitar={(indice) => void motor.quitarSegmento(indice)}
                  />

                  {/* TABS: Compatibilidad, Optimización */}
                  <GameTabs view={view} onSelect={selectView} />

                  {/* Resumen del perfil: qué se está tocando y con qué respaldo.
                      Va a todo el ancho y después de las pestañas, para que la
                      columna estrecha de la derecha no lo aplaste. */}
                  <OptimizationSummary
                    summary={rules.summary}
                    impacts={impacts}
                    engine={engine?.name}
                    appid={current.appid}
                    reports={info.total}
                  />

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
                          // Lo pedido se adelanta mientras el motor responde: sin esto el
                          // toggle no se movía hasta volver el IPC (se veía como pestañeo).
                          estadosMotor={conPedidos(motor.revision.estados, motor.ajustesPendientes)}
                          // Solo los ajustes en vuelo pintan su punto de progreso: antes se
                          // marcaban todos y parpadeaba la lista entera en cada pulsación.
                          enVuelo={motor.ajustesPendientes}
                          // El estado de las recomendaciones lo dice el motor: el `Set`
                          // local solo servía para guardar el perfil, no para el comando.
                          recomendacionesPuestas={(texto) => recomendacionPuesta(texto)}
                          selectedCommunityRecs={selectedCommunityRecs}
                          selectedEngineRecs={selectedEngineRecs}
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
                      perfiles={{
                        // Derivados de la colección única por appid exacto: no hay una
                        // segunda lista que pueda desincronizarse de la biblioteca global.
                        perfiles: perfilesDelJuego,
                        cargadoId: perfilCargado?.id ?? null,
                        cargadoNombre: perfilCargado?.nombre ?? null,
                        sinGuardar: hasUnsavedChanges,
                        ocupado: motor.cargando,
                        onCargar: pedirCargarPerfil,
                        onRestaurarCargado: () => {
                          // La misma operación que cargarlo: no hay una segunda ruta.
                          const actual = perfilesDelJuego.find((p) => p.id === perfilCargado?.id);
                          if (actual) void cargarPerfilEnJuego(actual);
                        },
                        onGestionar: () => {
                          setSidebarView('profiles');
                          closeGame();
                        },
                      }}
                      onMeasurePerformance={() => {
                        setSidebarView('benchmarks');
                        setToast('Graba con MangoHud y analiza aquí el resultado');
                      }}
                      onShowGameTests={() =>
                        setPruebasDeJuego({
                          appid: current.appid,
                          gameName: current.name,
                        })
                      }
                    />
                  </div>
                </div>
              </motion.div>
              </BarreraDeErrores>
            )}

            {/* PROFILES VIEW */}
            {sidebarView === 'profiles' && (
              <motion.div key="profiles" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <PerfilesTab
                  // La colección única: la biblioteca global no tiene lista propia.
                  profiles={profiles}
                  profileCommands={profileCommands}
                  onAbrirJuego={(appid) => abrirJuegoDesdePerfil(appid)}
                  onCargarEnJuego={(profile) => abrirJuegoYCargarPerfil(profile)}
                  onCopy={loadProfile}
                  onExport={(profile) =>
                    ask({
                      title: 'Exportar perfil',
                      message: `Se guardará «${profile.name}» en un archivo .json dentro de tu carpeta personal.`,
                      confirmLabel: 'Exportar',
                      tone: 'primary',
                      input: {
                        label: 'Ruta del archivo',
                        defaultValue: `~/Descargas/${fileSafeName(profile.game_name)}_${fileSafeName(profile.name)}.json`,
                        onSubmit: (path) => exportProfile(profile.id, path),
                      },
                    })
                  }
                  onDelete={(profile) =>
                    ask({
                      title: 'Eliminar perfil',
                      message: `Se eliminará «${profile.name}». Esta acción no se puede deshacer.`,
                      confirmLabel: 'Eliminar',
                      onConfirm: () => deleteProfile(profile.id),
                    })
                  }
                  onReorder={(ordered) => void reorderProfiles(ordered)}
                  onImportar={() =>
                    pickProfileFile((content, fileName) => {
                      setImportFileName(fileName);
                      importProfilesFromText(content);
                    })
                  }
                  onExportarTodos={() =>
                    ask({
                      title: 'Exportar todos los perfiles',
                      message:
                        'Escribe la ruta del archivo .json donde se guardarán. ' +
                        'Se permite cualquier carpeta dentro de tu carpeta personal (menos las de configuración, como ~/.config).',
                      confirmLabel: 'Exportar',
                      tone: 'primary',
                      input: {
                        label: 'Ruta del archivo',
                        defaultValue: '~/Descargas/thoriapp_perfiles_backup.json',
                        onSubmit: exportAllProfiles,
                      },
                    })
                  }
                />
              </motion.div>
            )}

            {/* BENCHMARKS VIEW */}
            {sidebarView === 'benchmarks' && (
              <motion.div key="benchmarks" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <div className="max-w-4xl mx-auto">
                  <Suspense fallback={<ViewLoading label="Cargando benchmarks…" />}>
                    <BenchmarksView
                      currentAppid={current?.appid}
                      currentGameName={current?.name}
                      currentLaunchCommand={launchLine()}
                      // El perfil que el usuario tiene cargado, no el que se arrastraba en
                      // la lista (ese ya no existe fuera de la pestaña de perfiles).
                      currentProfileName={perfilCargado?.nombre}
                      // Sin juego abierto no hay comando que registrar: la vista lo avisa
                      // en vez de guardar una prueba con un comando inventado.
                      hayJuegoAbierto={!!current}
                      // Instantánea (subfase 5.2): la configuración que la vista puede
                      // atribuir a una grabación. Solo la del juego abierto, y solo si el
                      // motor tiene un comando **confirmado** (no uno pendiente).
                      // El motor no publica un número de revisión estable: se pasa `null`
                      // antes que inventar un contador que no garantizaría nada.
                      revisionMotor={null}
                      cambiosSinGuardar={hasUnsavedChanges}
                      contextoDelJuegoAbierto={
                        current && !motor.cargando && motor.ajustesPendientes === null
                          ? {
                              appid: current.appid,
                              gameName: current.name,
                              perfilId: perfilCargado?.id ?? null,
                              perfilNombre: perfilCargado?.nombre ?? null,
                              comando: motor.revision.comando,
                              modoProton: forceProton,
                            }
                          : null
                      }
                      mangohudLogPath={mangohudLogPath}
                      onLogPathChange={setMangohudLogPath}
                    />
                  </Suspense>
                </div>
              </motion.div>
            )}

            {/* DASHBOARD VIEW - MIS PRUEBAS */}
            {sidebarView === 'tests' && (
              <motion.div key="dashboard" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="p-8">
                <Suspense fallback={<ViewLoading label="Cargando tus pruebas…" />}>
                  <DashboardView />
                </Suspense>
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
            className="fixed bottom-6 right-6 px-5 py-3 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-xl shadow-action/20 text-sm font-semibold text-white"
          >
            {toast}
          </motion.div>
        )}
      </AnimatePresence>

      {/* Pruebas del juego abierto: solo lectura y borrado, sin capturar nada. «Medir
          una prueba nueva» lleva a Benchmarks con el juego ya abierto, que es donde el
          formulario de análisis tiene sentido. */}
      {pruebasDeJuego && (
        <Suspense fallback={null}>
          <PruebasJuego
            appid={pruebasDeJuego.appid}
            gameName={pruebasDeJuego.gameName}
            onClose={() => setPruebasDeJuego(null)}
            onMedir={() => {
              setPruebasDeJuego(null);
              setSidebarView('benchmarks');
            }}
          />
        </Suspense>
      )}

      {/* Diálogos propios: el nativo del sistema rompería el estilo de la app */}
      {confirmState && <ConfirmDialog state={confirmState} onClose={close} />}

      {/* Vista previa de importación: el archivo no se escribe hasta que se confirma */}
      {pendingImport && (
        <ImportPreviewDialog
          preview={pendingImport.preview}
          fileName={importFileName}
          onConfirm={confirmImport}
          onCancel={cancelImport}
        />
      )}

      <SaveProfileModal
        open={showSaveProfileModal}
        name={newProfileName}
        notes={newProfileNotes}
        onNameChange={setNewProfileName}
        onNotesChange={setNewProfileNotes}
        onCancel={() => setShowSaveProfileModal(false)}
        onSave={saveProfile}
        perfilCargado={perfilCargado?.nombre ?? null}
        onUpdate={() => {
          // Actualiza el perfil cargado sin salir del diálogo hasta saber si fue bien.
          void (async () => {
            const ok = await guardarEnPerfilCargado();
            if (ok) {
              setShowSaveProfileModal(false);
              setNewProfileName('');
              setNewProfileNotes('');
            }
          })();
        }}
      />
    </div>
  );
}

export default App;
