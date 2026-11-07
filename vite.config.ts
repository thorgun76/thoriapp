import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // El caché de dependencias se escribe a un directorio escribible por el usuario.
  // Por defecto se usa `node_modules/.vite`, que da EACCES cuando node_modules
  // pertenece a otro usuario (p. ej. si se instaló con root desde un contenedor).
  cacheDir: '.vite-cache',
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  // Los tests de hooks necesitan un DOM. El resto de tests son de funciones puras.
  test: {
    environment: "jsdom",
    // El entorno puede traer NODE_ENV=production, y React entonces carga su
    // build de producción, donde act() no existe.
    env: { NODE_ENV: "test" },
  },
  build: {
    target: process.env.TAURI_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
