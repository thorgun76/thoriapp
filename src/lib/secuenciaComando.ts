/**
 * Secuenciación de las respuestas IPC del motor de comandos.
 *
 * ## El problema
 *
 * Las llamadas al backend son asíncronas y **no se contestan en el orden en que se
 * piden**. Con dos `analyze_launch_command` seguidos (pulsar dos veces un toggle, o
 * cambiar de juego mientras la primera petición sigue en vuelo) la respuesta lenta puede
 * llegar **después** de la rápida. Si se aplica tal cual, la UI se queda con el estado
 * viejo y el siguiente cambio se calcula sobre él: el toggle se mueve y el comando no
 * cambia, o cambia a algo que el usuario ya había descartado.
 *
 * ## La solución
 *
 * Cada petición lleva un número. Se aplica la respuesta solo si:
 *
 * 1. es de la petición vigente (`esLaActual`), y
 * 2. pertenece al contexto actual —el mismo juego y el mismo comando de partida—, para que
 *    cambiar de juego o importar otro comando invalide lo que venía en camino.
 *
 * No hay estado compartido ni caché: el objeto se crea por componente y las peticiones
 * llevan su número.
 */

/** Identifica el contexto al que pertenece una petición. */
export interface ContextoComando {
  /** Appid del juego. Si cambia, la respuesta de otro juego no sirve. */
  appid: number;
  /**
   * Comando de partida. Si cambia (importación de Steam, edición manual), las respuestas
   * de la versión anterior ya no describen nada de lo que hay en pantalla.
   */
  original: string;
}

/** Una petición en vuelo, con su número y su contexto. */
export interface Peticion {
  seq: number;
  contexto: ContextoComando;
}

/**
 * Contador de peticiones. Una instancia por componente.
 *
 * Es deliberadamente pequeño y sin estado derivado: guarda el último número emitido y el
 * último aplicado, nada más.
 */
export class SecuenciaComando {
  private emitidos = 0;
  private aplicado = 0;

  /** Abre una petición nueva y devuelve lo que hay que pasarle al backend. */
  abrir(contexto: ContextoComando): Peticion {
    this.emitidos += 1;
    return { seq: this.emitidos, contexto };
  }

  /**
   * ¿Se puede aplicar esta respuesta?
   *
   * Dos condiciones, y las dos importan:
   *
   * - **Es la última pedida** (`seq === emitidos`): descarta las que llegaron tarde.
   * - **Es más nueva que la última aplicada** (`seq > aplicado`): evita aplicar dos veces la
   *   misma, o volver atrás.
   *
   * Si la respuesta es válida, queda registrada como aplicada.
   */
  aceptar(peticion: Peticion, actual: ContextoComando): boolean {
    if (!this.mismoContexto(peticion.contexto, actual)) return false;
    if (peticion.seq !== this.emitidos) return false;
    if (peticion.seq <= this.aplicado) return false;
    this.aplicado = peticion.seq;
    return true;
  }

  /**
   * ¿Sigue vigente esta petición? Sirve para no pintar un «cargando» de algo ya superado.
   */
  esLaActual(peticion: Peticion, actual: ContextoComando): boolean {
    return (
      this.mismoContexto(peticion.contexto, actual) && peticion.seq === this.emitidos
    );
  }

  /** Número de la última petición emitida. Para diagnóstico y tests. */
  get ultimaEmitida(): number {
    return this.emitidos;
  }

  /** Número de la última respuesta aplicada. */
  get ultimaAplicada(): number {
    return this.aplicado;
  }

  private mismoContexto(a: ContextoComando, b: ContextoComando): boolean {
    return a.appid === b.appid && a.original === b.original;
  }
}
