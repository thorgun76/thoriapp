/**
 * Lógica de la vista de juego que no toca React: resumen de impactos, filtro de
 * ajustes, memoria de la pestaña por juego y detección de cambios sin guardar.
 *
 * Vive aparte para poder probarla sin montar App entera (que depende de Tauri).
 */
import type { Param } from './types';

// ==================== RESUMEN DE IMPACTOS ACTIVOS ====================

/** Orden de gravedad: el resumen enseña siempre el peor de los activos. */
const SEVERITY: Record<string, number> = {
  evitar: 5,
  dudoso: 4,
  medicion: 3,
  medio: 2,
  bajo: 1,
};

export interface ImpactSummary {
  /** Cuántos ajustes hay activos. */
  count: number;
  /** Peor impacto entre los activos (`null` si no hay ninguno). */
  worst: string | null;
  /** Texto corto para el chip del encabezado. */
  label: string;
  /** Clases Tailwind del chip, según la gravedad. */
  tone: 'none' | 'ok' | 'caution' | 'danger';
}

/**
 * Resume los impactos de los ajustes activos.
 *
 * Sirve para saber si se ha tocado algo delicado sin abrir la pestaña de
 * Optimización: con 20 parámetros, «3 ajustes activos» no dice si son inocuos.
 */
export function summarizeImpacts(params: Param[], enabled: boolean[]): ImpactSummary {
  const active = params.filter((_, i) => enabled[i] === true);

  if (active.length === 0) {
    return { count: 0, worst: null, label: 'Sin ajustes activos', tone: 'none' };
  }

  let worst: string | null = null;
  let worstScore = -1;
  for (const p of active) {
    const score = SEVERITY[p.impact] ?? 0;
    if (score > worstScore) {
      worstScore = score;
      worst = p.impact;
    }
  }

  // «evitar»/«dudoso» se pintan en rojo; «medicion» y «medio» en ámbar; el resto verde.
  const tone: ImpactSummary['tone'] =
    worst === 'evitar' || worst === 'dudoso'
      ? 'danger'
      : worst === 'medicion' || worst === 'medio'
        ? 'caution'
        : 'ok';

  const noun = active.length === 1 ? 'ajuste activo' : 'ajustes activos';
  return {
    count: active.length,
    worst,
    label: worst ? `${active.length} ${noun} · riesgo ${worst}` : `${active.length} ${noun}`,
    tone,
  };
}

// ==================== FILTRO DENTRO DE OPTIMIZACIÓN ====================

/**
 * Filtra parámetros por texto (título, valor o por qué) y por nivel de impacto.
 *
 * En modo experto la lista es larga; buscar «FSR» a mano es tedioso. Se compara
 * sin acentos ni mayúsculas, igual que el buscador de la biblioteca.
 */
export function filterParams(
  params: Param[],
  query: string,
  impact: string | null,
  normalize: (text: string) => string = (t) => t.toLowerCase()
): Param[] {
  const q = normalize(query.trim());
  return params.filter((p) => {
    if (impact && p.impact !== impact) return false;
    if (!q) return true;
    return (
      normalize(p.title).includes(q) ||
      normalize(p.value).includes(q) ||
      normalize(p.simple_why || p.why).includes(q)
    );
  });
}

/**
 * Niveles de impacto presentes, para no ofrecer filtros vacíos.
 *
 * Además de `params` acepta valores sueltos (recomendaciones de motor o de
 * hardware cuyo impacto se conoce por su parámetro equivalente). Sin esto, un
 * nivel que solo existiera en una recomendación no tendría etiqueta y esa fila
 * quedaría inalcanzable con el filtro puesto.
 */
export function impactsPresent(params: Param[], extra: (string | null | undefined)[] = []): string[] {
  const seen = new Set<string>();
  for (const p of params) if (p.impact) seen.add(p.impact);
  for (const impact of extra) if (impact) seen.add(impact);
  return [...seen].sort((a, b) => (SEVERITY[b] ?? 0) - (SEVERITY[a] ?? 0));
}

// ==================== PESTAÑA RECORDADA POR JUEGO ====================

export type GameTab = 'compatibility' | 'recommendations';

const TAB_KEY = 'thoriapp:view:';

/**
 * Pestaña con la que se abre un juego: la última que usó el usuario en él.
 *
 * Reabrir un juego siempre en «Optimización» obliga a volver a «Compatibilidad»
 * cada vez que se quiere comprobar el anti-cheat.
 */
export function recallTab(
  appid: number,
  storage: Pick<Storage, 'getItem'> | null = null
): GameTab {
  const store = storage ?? (typeof localStorage === 'undefined' ? null : localStorage);
  if (!store) return 'recommendations';
  try {
    const saved = store.getItem(TAB_KEY + appid);
    return saved === 'compatibility' || saved === 'recommendations' ? saved : 'recommendations';
  } catch {
    // localStorage puede estar bloqueado (modo privado): no es motivo de error.
    return 'recommendations';
  }
}

/** Guarda la pestaña en la que se quedó el usuario para ese juego. */
export function rememberTab(
  appid: number,
  tab: GameTab,
  storage: Pick<Storage, 'setItem'> | null = null
): void {
  const store = storage ?? (typeof localStorage === 'undefined' ? null : localStorage);
  if (!store) return;
  try {
    store.setItem(TAB_KEY + appid, tab);
  } catch {
    // Sin persistencia se sigue funcionando: solo se pierde la comodidad.
  }
}

// ==================== ESTADO DE GUARDADO ====================

/** Lo que define un perfil guardado, para compararlo con el estado actual. */
export interface ProfileSnapshot {
  enabledParams: boolean[];
  selectedCommunityRecs: number[];
  selectedEngineRecs: number[];
  forceProton: boolean;
  manualCommand: string;
  useManualCommand: boolean;
}

/**
 * Instantánea del estado actual de la vista de juego.
 *
 * Las selecciones se ordenan para que dos conjuntos iguales den la misma firma
 * aunque se hayan activado en distinto orden.
 */
export function snapshotOf(input: {
  enabled: boolean[];
  selectedCommunityRecs: Set<number>;
  selectedEngineRecs: Set<number>;
  forceProton: boolean;
  manualCommand: string;
  useManualCommand: boolean;
}): ProfileSnapshot {
  return {
    enabledParams: input.enabled.map((v) => v === true),
    selectedCommunityRecs: [...input.selectedCommunityRecs].sort((a, b) => a - b),
    selectedEngineRecs: [...input.selectedEngineRecs].sort((a, b) => a - b),
    forceProton: input.forceProton,
    manualCommand: input.manualCommand.trim(),
    useManualCommand: input.useManualCommand,
  };
}

/** ¿Hay cambios sin guardar respecto al perfil guardado? */
export function isDirty(current: ProfileSnapshot, saved: ProfileSnapshot | null): boolean {
  // Sin perfil guardado no hay nada que perder todavía: no se avisa.
  if (!saved) return false;
  return signature(current) !== signature(saved);
}

const signature = (s: ProfileSnapshot) =>
  JSON.stringify([
    s.enabledParams,
    s.selectedCommunityRecs,
    s.selectedEngineRecs,
    s.forceProton,
    s.manualCommand,
    s.useManualCommand,
  ]);

// ==================== COMANDO RÁPIDO DESDE LA BIBLIOTECA ====================

const BASE_COMMAND = '%command%';

/**
 * Comando que se puede copiar de un juego sin abrir su vista.
 *
 * En la biblioteca no hay selección de ajustes, así que lo útil no es repetir el
 * `%command%` de partida (no aporta nada) sino el comando del perfil que el
 * usuario ya guardó para ese juego. Si no hay perfil guardado se devuelve `null`
 * y la tarjeta no ofrece copiar: un botón que copia `%command%` solo engaña.
 */
export function quickCommandFor(
  appid: number,
  profiles: { id: string; appid: number }[],
  profileCommands: Record<string, string>
): string | null {
  const own = profiles.filter((p) => p.appid === appid);
  if (own.length === 0) return null;

  // El primer perfil con comando real (los perfiles pueden viajar sin él).
  for (const profile of own) {
    const command = profileCommands[profile.id];
    if (command && command.trim() && command.trim() !== BASE_COMMAND) return command.trim();
  }
  return null;
}
