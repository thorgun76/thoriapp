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
    rollupOptions: {
      output: {
        // Las dependencias cambian mucho menos que el código de la app: en trozos
        // aparte, una actualización de Thoriapp no invalida todo el caché. Además
        // separa los paquetes grandes (framer-motion, dnd-kit) del trozo principal.
        // Con objeto, Rollup mete React dentro del primer paquete que lo importe
        // (quedaba en `dnd`, de 184 kB). La función decide por módulo, con React
        // comprobado primero.
        manualChunks(id) {
          if (!id.includes("node_modules")) return undefined;
          if (/node_modules[\\/](react|react-dom|scheduler)[\\/]/.test(id)) return "react";
          if (id.includes("framer-motion")) return "motion";
          if (id.includes("@dnd-kit")) return "dnd";
          return "vendor";
        },
      },
    },
  },
});
