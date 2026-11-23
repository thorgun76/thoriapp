/// <reference types="vite/client" />

// `?raw` lo aporta `vite/client`. Se usa en `command.test.ts` para importar
// `preflight.rs` como texto y comprobar que la lista de envoltorios del frontend
// coincide con la del backend. jsdom stubea `fs`, así que leerlo con Node no vale.
