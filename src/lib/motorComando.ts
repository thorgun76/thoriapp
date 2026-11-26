/**
 * Espejo en TS de los tipos de `src-tauri/src/launch_command.rs`.
 *
 * Aquí **no hay reglas**: solo la forma de lo que el motor devuelve. Todo lo que decide
 * algo (qué está activo, dónde entra un ajuste, si un comando vale, si dos comandos
 * significan lo mismo) se pregunta por IPC. Duplicar esas reglas en TS es justo lo que
 * provocó los fallos anteriores: el frontend y el backend acababan diciendo cosas
 * distintas sobre el mismo comando.
 *
 * La única excepción es el estado de los ajustes (`EstadoAjuste`), que se recibe ya
 * calculado. React lo pinta, no lo deduce.
 */

/** Quién manda sobre un segmento del comando. */
export type Gestion =
  /** Thoriapp lo reconoce y puede activarlo o desactivarlo. */
  | { tipo: 'thoriapp'; clave: string; indice: number }
  /** Es el `%command%`. */
  | { tipo: 'marcador' }
  /**
   * Se conserva tal cual. `razon` dice por qué.
   *
   * El nombre **no es decorativo**: Rust lo serializa como
   * `{"tipo":"conservado","razon":{"motivo":"desconocido"}}` (campo `razon` de la variante
   * `Gestion::Conservado`). Leer `gestion.motivo` lanzaba
   * «undefined is not an object (evaluating 's.gestion.motivo.motivo')» al pintar la ficha,
   * que es la pantalla negra que reportó el usuario.
   */
  | { tipo: 'conservado'; razon: Conservado };

/**
 * Por qué un segmento se conserva en vez de gestionarse.
 *
 * Se guarda el motivo y no un simple booleano porque «no lo conocemos» y «es ambiguo»
 * piden cosas distintas del usuario: la primera se puede enseñar y callar, la segunda hay
 * que preguntarla.
 */
export type Conservado =
  | { motivo: 'desconocido' }
  | { motivo: 'ambiguo'; candidatos: number[] }
  | { motivo: 'duplicado'; de_span: SpanComando }
  | { motivo: 'arg_juego' }
  | { motivo: 'no_seguro'; detalle: string };

/** Papel de un segmento dentro de la línea. */
export type Rol =
  /** Variable de entorno: `FOO=bar`. */
  | 'env'
  /** Envoltorio conocido: `mangohud`, `gamescope`… */
  | 'wrapper'
  /** Argumento suelto del lanzador que Thoriapp gestiona (`-novid`). */
  | 'arg'
  /** Argumento propio de un envoltorio: `-W 1920` en `gamescope -W 1920 -- %command%`. */
  | 'arg_wrapper'
  /** El marcador `%command%`. */
  | 'marcador'
  /** Programa propio entre los envoltorios y el marcador. */
  | 'ejecutable'
  /** Argumento del juego, detrás de `%command%`. Thoriapp no lo toca nunca. */
  | 'arg_juego';

/** Tramo de bytes del texto original. */
export interface SpanComando {
  inicio: number;
  fin: number;
}

/** Un trozo del comando, con su papel y (si se reconoce) su ajuste. */
export interface SegmentoComando {
  span: SpanComando;
  /** Texto exacto, tal cual está en el original. */
  texto: string;
  /** Texto sin comillas, que es con lo que se compara. */
  valor: string;
  rol: Rol;
  gestion: Gestion;
  /** `false` si tiene una comilla sin cerrar. */
  cerrado: boolean;
}

/** Aviso sobre algo que Thoriapp no puede decidir solo. */
export type AvisoComando =
  | { tipo: 'sin_marcador' }
  | { tipo: 'marcador_repetido'; posiciones: SpanComando[] }
  | { tipo: 'comilla_sin_cerrar'; span: SpanComando; comilla: string }
  /** El mismo valor aparece dos veces; el segundo no se gestiona. */
  | { tipo: 'duplicado'; span: SpanComando; valor: string }
  /** Dos valores distintos para la misma variable. */
  | { tipo: 'contradiccion'; key: string; spans: SpanComando[] }
  /** El valor encaja con más de un ajuste del catálogo. */
  | { tipo: 'ambiguo'; span: SpanComando; valor: string; candidatos: number[] }
  /** Comparte nombre de variable con un ajuste del catálogo, pero no es el mismo. */
  | { tipo: 'mismo_nombre_distinto_valor'; key: string; valor: string };

/**
 * Lo que devuelve `analyze_launch_command`. Es el estado completo del comando y se
 * reenvía tal cual a `serialize_launch_command`.
 *
 * `original` es inmutable: es el texto de partida y la única fuente de verdad para
 * reconstruir.
 */
export interface AnalisisComando {
  original: string;
  segmentos: SegmentoComando[];
  avisos: AvisoComando[];
  /**
   * `false` si algún aviso impide tocar el comando. Entonces la UI enseña el original y
   * explica por qué; nunca lo reconstruye a medias.
   */
  editable: boolean;
  /** Cuántos segmentos reconoce Thoriapp. */
  reconocidos: number;
  /** Cuántos conserva sin tocarlos. */
  conservados: number;
  /**
   * `true` si la línea está escrita en la gramática que el motor entiende del todo.
   *
   * No es lo mismo que `editable`: un comando sin `%command%` se escribe bien (sintaxis
   * válida) pero no se puede editar. Una comilla sin cerrar rompe las dos.
   */
  sintaxis_valida: boolean;
  /** Por qué no es válida la sintaxis, para enseñarlo tal cual. */
  motivo_sintaxis: string | null;
}

/** Petición de activar o desactivar un ajuste. */
export interface CambioAjuste {
  /** Índice en el catálogo con el que se analizó. Informativo; no identifica el ajuste. */
  indice: number;
  /**
   * Texto exacto del ajuste. **Es la identidad**: el índice cambia entre el catálogo del
   * juego y el efectivo (el que añade las recomendaciones), el valor no.
   */
  valor: string;
  activo: boolean;
}

/**
 * Lo que devuelve `serialize_launch_command`.
 *
 * No es un `Result` de Tauri: un fallo del motor no es una excepción, es un estado que la
 * UI tiene que enseñar. Si `aplicado` es `false`, `comando` es el original intacto y
 * `error` dice por qué.
 */
export interface ResultadoSerializacion {
  comando: string;
  aplicado: boolean;
  error: ErrorMotor | null;
}

/** El análisis recibido no cuadraba consigo mismo. */
export type ErrorMotor =
  | {
      tipo: 'span_fuera_de_rango';
      segmento: number;
      inicio: number;
      fin: number;
      longitud: number;
    }
  | { tipo: 'span_en_mitad_de_caracter'; segmento: number; inicio: number; fin: number }
  | { tipo: 'spans_desordenados'; segmento: number; inicio: number; fin_anterior: number }
  | {
      tipo: 'texto_no_coincide';
      segmento: number;
      esperado: string;
      recibido: string;
    }
  | { tipo: 'indice_desconocido'; indice: number };

/** Motivo por el que un comando no se puede lanzar. */
export interface MotivoRechazo {
  /**
   * Código estable. No comparar por `mensaje`: cambia con la redacción.
   *
   * `vacio`, `demasiado_largo`, `encadenamiento`, `sintaxis`, `sin_marcador`,
   * `lanzador_desconocido`, `ruta_absoluta`, `marcador_repetido`.
   */
  codigo: string;
  mensaje: string;
}

/**
 * Situación de un ajuste respecto al comando actual.
 *
 * Las cinco variantes las decide el motor. React no las deduce: si un ajuste aparece dos
 * veces y solo una es gestionable, el motor lo dice (`parcial`) y la UI no puede pintarlo
 * como desactivado mientras queda una aparición activa.
 */
export interface EstadoAjuste {
  /** Índice del ajuste en el catálogo (`rules.params`). Viene dado, no se deduce. */
  indice: number;
  /**
   * Texto exacto del ajuste (`mangohud`, `RADV_PERFTEST=sam`). Es lo que hay que enseñar:
   * un índice interno no significa nada para el usuario.
   */
  valor: string;
  /** Clase de pieza (`env`, `wrapper`, `arg`), según el motor. */
  kind: string;
  /**
   * Solo tiene efecto bajo Proton/Wine. Lo decide Rust (`rules::is_proton_only`) y aquí
   * solo se lee. **No** dupliques esta regla en TypeScript: la copia que había divergió y
   * dejó durante una versión ajustes pintados como aplicables que el motor rechazaba.
   */
  proton_only?: boolean;
  /**
   * Las cinco situaciones. Se modelan como campos opcionales en lugar de una unión
   * discriminada porque el motor serializa el enum plano (`#[serde(flatten)]`): los campos
   * de cada variante llegan al mismo nivel, no anidados. `estado` dice cuál es.
   */
  estado: 'inactivo' | 'activo' | 'parcial' | 'ambiguo' | 'conflicto';
  /** Tramo donde está, cuando está activo. */
  span?: SpanComando;
  /** En `parcial`: dónde sí se puede actuar. */
  gestionadas?: SpanComando[];
  /** En `parcial`: lo que queda en el comando fuera de control de Thoriapp. */
  conservadas?: SpanComando[];
  /** En `ambiguo` y `conflicto`: todas las apariciones. */
  spans?: SpanComando[];
  /** En `ambiguo`: ajustes del catálogo con los que encaja. */
  candidatos?: number[];
  /** En `conflicto`: los valores del comando que ocupan la misma variable. */
  en_comando?: string[];
}

/** Extrae el estado de un ajuste por su índice, si lo hay en la lista. */
export function estadoDe(
  estados: readonly EstadoAjuste[],
  indice: number,
): EstadoAjuste | null {
  return estados.find((e) => e.indice === indice) ?? null;
}

/**
 * ¿El toggle tiene que aparecer encendido?
 *
 * Solo `activo` lo está del todo. `parcial` y `conflicto` **no** son «encendido»: tienen su
 * propio aspecto porque el usuario tiene que enterarse de que queda algo en el comando que
 * Thoriapp no controla.
 */
export function estaEncendido(estado: EstadoAjuste | null): boolean {
  return estado?.estado === 'activo';
}

/**
 * ¿El toggle es accionable?
 *
 * `ambiguo` no lo es: Thoriapp no sabe qué ajuste del catálogo corresponde a lo que hay en
 * el comando, así que activarlo o desactivarlo sería adivinar.
 */
export function esAccionable(estado: EstadoAjuste | null): boolean {
  if (estado === null) return true;
  return estado.estado !== 'ambiguo';
}

/**
 * Estados confirmados con los ajustes **pedidos** ya reflejados.
 *
 * El motor tarda un viaje IPC en contestar. Si la interfaz pinta solo lo confirmado, el
 * toggle no se mueve al pulsarlo y «salta» al volver: eso es el pestañeo. Aquí se adelanta
 * el resultado esperado de cada ajuste en vuelo, sin inventar nada más: el resto del estado
 * (spans, parcial, conflicto) sigue viniendo del motor.
 *
 * Lo pedido es la única fuente de este adelanto; cuando el motor confirma, `pendientes`
 * vuelve a `null` y manda otra vez lo confirmado.
 *
 * @param estados Lo confirmado por Rust.
 * @param pendientes Índices pedidos sin confirmar (`null` = nada en vuelo).
 */
export function conPedidos(
  estados: EstadoAjuste[],
  pendientes: Set<number> | null
): EstadoAjuste[] {
  if (!pendientes) return estados;

  const yaPuesto = (e: EstadoAjuste) => e.estado === 'activo' || e.estado === 'parcial';
  const porIndice = new Map(estados.map((e) => [e.indice, e]));

  // Los que ya existen se quedan en su sitio; solo cambia `estado`.
  const actualizados = estados.map((e) => {
    const deberia = pendientes.has(e.indice);
    if (deberia === yaPuesto(e)) return e;
    // Si se pide apagar, se apaga; si se pide encender, se enciende. Los campos de detalle
    // del estado anterior no valen para el nuevo, así que se descartan.
    return deberia
      ? { indice: e.indice, valor: e.valor, kind: e.kind, proton_only: e.proton_only, estado: 'activo' as const }
      : { indice: e.indice, valor: e.valor, kind: e.kind, proton_only: e.proton_only, estado: 'inactivo' as const };
  });

  // Un ajuste pedido que el motor aún no conocía (recién aparecido) necesita entrada propia,
  // o su tarjeta seguiría leyéndose como inactiva. Si no hay datos, se deja al motor.
  const faltan = [...pendientes].filter((i) => !porIndice.has(i));
  if (faltan.length === 0) return actualizados;
  return actualizados;
}
