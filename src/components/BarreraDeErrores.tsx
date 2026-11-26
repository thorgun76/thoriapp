import { Component, Fragment, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle, RotateCcw } from 'lucide-react';

/**
 * Barrera de errores de renderizado.
 *
 * ## Por qué existe
 *
 * Sin esto, cualquier excepción lanzada mientras React pinta un componente desmonta el
 * árbol entero y deja la ventana **en negro**, sin mensaje y sin forma de recuperarse
 * salvo cerrar la app. Un fallo de un solo ajuste no puede costar la aplicación completa.
 *
 * ## Qué NO hace
 *
 * - **No oculta el error.** Lo muestra con su mensaje y su traza, y lo registra en consola.
 *   Una barrera que se traga el error solo cambia un pantallazo negro por un silencio.
 * - **No sustituye al arreglo.** Si aparece, es que hay un bug y hay que corregirlo; esto
 *   es la red, no el suelo.
 * - **No captura errores asíncronos.** Para eso está `capturarErroresGlobales`, porque un
 *   `reject` de una promesa no pasa por aquí.
 */
interface EstadoBarrera {
  error: Error | null;
  componente: string | null;
  /**
   * Cambia en cada reintento. Se usa como `key` del contenido: sin ella React reutiliza el
   * árbol que acaba de fallar y el error se repite, así que «Reintentar» no reintentaría
   * nada de verdad.
   */
  intento: number;
}

export class BarreraDeErrores extends Component<
  {
    children: ReactNode;
    /**
     * Nombre de la zona protegida («Ficha del juego»). Sirve para saber *dónde* falló sin
     * leer la traza.
     */
    zona: string;
    /** Se llama al recuperar. Si no se pasa, se reintenta repintando. */
    onReintentar?: () => void;
  },
  EstadoBarrera
> {
  state: EstadoBarrera = { error: null, componente: null, intento: 0 };

  static getDerivedStateFromError(error: Error): Partial<EstadoBarrera> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Se registra completo a propósito: es el único sitio donde queda constancia del bug
    // cuando ocurre en la app empaquetada, donde no hay devtools abiertas.
    console.error(`[Thoriapp] Fallo al pintar «${this.props.zona}»:`, error);
    if (info.componentStack) {
      console.error('[Thoriapp] Componentes implicados:', info.componentStack);
    }
    this.setState({ componente: primerComponente(info.componentStack ?? undefined) });
    esteUltimoError = { error, zona: this.props.zona };
  }

  reintentar = () => {
    this.setState((previo) => ({ error: null, componente: null, intento: previo.intento + 1 }));
    this.props.onReintentar?.();
  };

  render() {
    const { error, componente } = this.state;
    const { children, zona } = this.props;
    if (!error) return <Fragment key={this.state.intento}>{children}</Fragment>;

    return (
      <section
        role="alert"
        aria-labelledby="titulo-fallo-render"
        className="m-4 p-5 rounded-xl bg-[var(--card)] border border-error/40"
      >
        <h2
          id="titulo-fallo-render"
          className="flex items-center gap-2 text-sm font-semibold text-[var(--error)] mb-2"
        >
          <AlertTriangle className="w-4 h-4 shrink-0" aria-hidden="true" />
          No se pudo mostrar {zona}: {error.message}
        </h2>

        <p className="text-xs text-[var(--text-muted)] leading-relaxed mb-3">
          El resto de la aplicación sigue funcionando: puedes volver a la biblioteca o
          reintentar. Los comandos guardados no se han tocado.
        </p>

        {/* El mensaje y la traza se enseñan tal cual. Son feos, pero son lo único que
            permite arreglar el fallo sin adivinar. */}
        <pre className="mb-3 p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-[11px] font-mono text-[var(--text)] whitespace-pre-wrap break-words max-h-48 overflow-auto">
          {error.message}
          {componente ? `\n\nen <${componente}>` : ''}
        </pre>

        <button
          type="button"
          onClick={this.reintentar}
          className="inline-flex items-center gap-2 px-4 py-2 rounded-lg text-xs font-semibold bg-[var(--action)] text-[var(--bg)] hover:opacity-90 transition-opacity duration-150"
        >
          <RotateCcw className="w-3.5 h-3.5" aria-hidden="true" />
          Reintentar
        </button>
      </section>
    );
  }
}

/** Último error capturado. Solo para los tests y para el aviso de diagnóstico. */
let esteUltimoError: { error: Error; zona: string } | null = null;

export function ultimoErrorDeRender() {
  return esteUltimoError;
}

/** Primer componente nombrado de la pila de React (`\n    at Foo (…)`). */
function primerComponente(componentStack: string | undefined): string | null {
  if (!componentStack) return null;
  const m = /at\s+([A-Za-z0-9_$.]+)/.exec(componentStack);
  return m ? m[1] : null;
}
