import { describe, expect, it } from 'vitest';
// `?raw` (ver src/vite-env.d.ts): jsdom stubea `fs` y no hay `@types/node`, así
// que los archivos se leen como texto con Vite. Es el mismo patrón que usa
// command.test.ts para comparar con preflight.rs.
import app from './App.tsx?raw';
import config from '../vite.config.ts?raw';

/**
 * Comprobaciones sobre el empaquetado que no se pueden ver montando componentes.
 *
 * Montar App entera exigiría simular todos los hooks de Tauri, y lo que se quiere
 * fijar aquí es la forma del bundle: que las vistas pesadas no entren en el trozo
 * inicial y que las dependencias vayan por su lado.
 */
describe('empaquetado', () => {
  it('las vistas pesadas se cargan bajo demanda', () => {
    // Regresión: con import estático, BenchmarksView (54 kB) entraba en el trozo
    // inicial y el arranque pagaba por una pantalla que quizá no se abre.
    expect(app).toMatch(/const BenchmarksView = lazy\(/);
    expect(app).toMatch(/const DashboardView = lazy\(/);
    expect(app).not.toMatch(/import \{ BenchmarksView \} from/);
    expect(app).not.toMatch(/import \{ DashboardView \} from/);
  });

  it('las vistas bajo demanda tienen un hueco de carga', () => {
    // Sin fallback, el primer cambio de sección enseña un salto en blanco.
    expect(app.match(/<Suspense fallback=/g) ?? []).toHaveLength(2);
  });

  it('las dependencias van en trozos separados del código de la app', () => {
    expect(config).toContain('manualChunks');
    // La forma de función permite comprobar React primero; con objeto, Rollup lo
    // metía dentro del primer paquete que lo importase.
    expect(config).toMatch(/manualChunks\(id\)/);
    expect(config).toContain('"react"');
    expect(config).toContain('"motion"');
    expect(config).toContain('"dnd"');
  });
});
