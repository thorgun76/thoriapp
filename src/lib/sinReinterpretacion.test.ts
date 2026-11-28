import { describe, it, expect } from 'vitest';
// (?raw) como en `command.test.ts`: el frontend no tiene tipos de Node, así que la fuente
// se lee con la API de Vite en vez de con `fs` y `__dirname`.
import dashboardView from '../components/DashboardView.tsx?raw';
import savedComparisonDetail from '../components/SavedComparisonDetail.tsx?raw';
import benchmarksView from '../components/BenchmarksView.tsx?raw';

/**
 * Guardia estructural contra la reinterpretación automática.
 *
 * **Actualizado en C.3.** El defecto original [5.4.1.B]: «Mis pruebas» y el detalle de una
 * prueba guardada llamaban a `interpretar()` sobre el dato guardado, así que una prueba de
 * hace meses aparecía con un veredicto nuevo, indistinguible de uno registrado.
 *
 * C.3 fue más lejos que B: `interpretar()` **ya no existe**. La interpretación vive en
 * Rust y el frontend solo presenta lo que el motor decidió. Estas pruebas son el cinturón
 * que impide reintroducir un intérprete de frontend aunque el módulo no esté.
 *
 * No son pruebas de comportamiento: eso está en `presentacionEvaluacion.test.ts` y en
 * `consistencia.test.tsx`.
 */

/** Componentes que pintan comparaciones, guardadas o no. */
const CONSUMIDORES: Array<[string, string]> = [
  ['DashboardView.tsx', dashboardView],
  ['SavedComparisonDetail.tsx', savedComparisonDetail],
  ['BenchmarksView.tsx', benchmarksView],
];

/** Quita comentarios para no confundir la explicación con el uso real. */
const sinComentarios = (fuente: string) =>
  fuente.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');

describe('no queda intérprete de frontend', () => {
  it('el módulo `interpretacion.ts` ya no existe en `src/`', async () => {
    // Se consulta con el glob de Vite y no con `fs`: el frontend no tiene tipos de Node,
    // igual que el `?raw` de arriba. Importarlo directamente rompería la suite entera al
    // resolver, que es exactamente lo que se quiere detectar, pero de forma ilegible.
    const modulos = import.meta.glob('./interpretacion.ts');
    expect(Object.keys(modulos)).toEqual([]);
  });

  it.each(CONSUMIDORES)('%s no llama a interpretar()', (_nombre, fuente) => {
    expect(/\binterpretar\s*\(/.test(sinComentarios(fuente))).toBe(false);
  });

  it.each(CONSUMIDORES)('%s no decide un veredicto por el signo de una cifra', (_nombre, fuente) => {
    // Un veredicto nunca se deduce de una comparación numérica en la vista: la única
    // fuente es `estado`. Se busca el patrón, no la palabra «mejor», que sí aparece en
    // los textos de presentación.
    const cuerpo = sinComentarios(fuente);
    expect(cuerpo).not.toMatch(/estado\s*=\s*[^=]*\b(mejor|empeora)\b/);
  });
});

describe('las comparaciones guardadas no se reinterpretan', () => {
  it('el detalle guardado presenta el modelo compartido', () => {
    expect(savedComparisonDetail).toContain('presentarGuardada(');
  });

  it('el listado guardado presenta el modelo compartido', () => {
    expect(dashboardView).toContain('presentarGuardada(');
  });

  it('ninguna de las dos lee el estado con una tabla propia', () => {
    // Antes cada vista traducía el estado por su cuenta (`veredictoGuardado`,
    // `ESTADO_TEXTO`). Ahora hay una sola función y las dos la consumen.
    expect(savedComparisonDetail).not.toContain('veredictoGuardado(');
    expect(dashboardView).not.toContain('estadoDeEvaluacionGuardada(');
  });
});

describe('la nueva comparación también pasa por el motor', () => {
  it('BenchmarksView pide la evaluación a Rust, no la calcula', () => {
    // La tarjeta de resultado exige `evaluacion`: no hay ruta que pinte un veredicto sin
    // una evaluación del motor, y `summarizeComparison` solo resume lo que no se evaluó.
    expect(benchmarksView).toContain('presentarEvaluacion(');
    expect(benchmarksView).toContain('presentarSinEvaluacion(');
  });

  it('no se guarda una comparación sin evaluación', () => {
    // `api.ts` expone `saveComparisonEvaluada` y solo esa.
    expect(benchmarksView).not.toContain('saveComparison(');
  });
});
