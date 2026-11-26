import { describe, it, expect } from 'vitest';
// (?raw) como en `command.test.ts`: el frontend no tiene tipos de Node, así que la fuente
// se lee con la API de Vite en vez de con `fs` y `__dirname`.
import dashboardView from '../components/DashboardView.tsx?raw';
import savedComparisonDetail from '../components/SavedComparisonDetail.tsx?raw';
import benchmarksView from '../components/BenchmarksView.tsx?raw';

/**
 * Guardia estructural contra la reinterpretación automática [5.4.1.B].
 *
 * El defecto que se corrigió: «Mis pruebas» y el detalle de una prueba guardada llamaban a
 * `interpretar()` sobre el dato guardado. Como `interpretar()` cambia con las reglas de la
 * app, una prueba de hace meses aparecía con un veredicto **nuevo**, indistinguible de uno
 * registrado. Estas pruebas fallan si eso vuelve.
 *
 * No son pruebas de comportamiento: son el cinturón que impide que una refactorización
 * reintroduzca la llamada sin que nadie lo note. La cubierta de comportamiento está en
 * `evaluacionGuardada.test.ts`.
 */

/** Componentes que pintan comparaciones **guardadas**. */
const CONSUMIDORES_GUARDADOS: Array<[string, string]> = [
  ['DashboardView.tsx', dashboardView],
  ['SavedComparisonDetail.tsx', savedComparisonDetail],
];

const IMPORTA_INTERPRETAR = /import\s*\{[^}]*\binterpretar\b[^}]*\}\s*from/;

/** Quita comentarios para no confundir la explicación con el uso real. */
const sinComentarios = (fuente: string) =>
  fuente.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');

describe('las comparaciones guardadas no se reinterpretan', () => {
  it.each(CONSUMIDORES_GUARDADOS)('%s no importa interpretar()', (_nombre, fuente) => {
    expect(IMPORTA_INTERPRETAR.test(fuente)).toBe(false);
  });

  it.each(CONSUMIDORES_GUARDADOS)('%s no llama a interpretar()', (_nombre, fuente) => {
    expect(/\binterpretar\s*\(/.test(sinComentarios(fuente))).toBe(false);
  });

  it('el detalle guardado usa el veredicto almacenado', () => {
    expect(savedComparisonDetail).toContain('veredictoGuardado(');
  });

  it('el listado guardado usa la clasificación del backend', () => {
    expect(dashboardView).toContain('estadoDeEvaluacionGuardada(');
  });
});

describe('interpretar() sigue siendo el único intérprete de lo no guardado', () => {
  // Mientras viva en TypeScript (bloque C lo llevará a Rust), las comparaciones **no
  // guardadas** lo siguen usando. Si alguien lo quitara de aquí sin trasladarlo, se
  // quedaría sin veredicto la comparación recién hecha.
  it('BenchmarksView sigue interpretando comparaciones no guardadas', () => {
    expect(/\binterpretar\s*\(/.test(sinComentarios(benchmarksView))).toBe(true);
  });
});
