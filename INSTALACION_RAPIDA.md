# 🚀 INSTALACIÓN SUPER SIMPLE

## Paso 1: Descargar

Descarga TODOS los archivos desde el enlace arriba a la derecha.

## Paso 2: Extraer

Extrae el archivo descargado en tu directorio preferido.

## Paso 3: Instalar

```bash
cd tauri-app
chmod +x install.sh
./install.sh
```

## Paso 4: Ejecutar

```bash
npm run tauri dev
```

¡Eso es todo! La aplicación se abrirá automáticamente.

---

## 📋 Archivos Incluidos

### Frontend (React)
- ✅ src/App.tsx (2070 líneas - app completa)
- ✅ src/components/BenchmarksView.tsx
- ✅ src/index.css
- ✅ src/main.tsx

### Backend (Rust)
- ✅ src-tauri/src/main.rs
- ✅ src-tauri/src/hw.rs
- ✅ src-tauri/src/steam.rs
- ✅ src-tauri/src/rules.rs
- ✅ src-tauri/src/engine.rs
- ✅ src-tauri/src/profiles.rs
- ✅ src-tauri/src/benchmarks.rs
- ✅ src-tauri/src/log.rs
- ✅ src-tauri/games.json
- ✅ src-tauri/Cargo.toml
- ✅ src-tauri/tauri.conf.json

### Configuración
- ✅ package.json
- ✅ vite.config.ts
- ✅ tsconfig.json
- ✅ tailwind.config.js
- ✅ postcss.config.js
- ✅ index.html

### Scripts
- ✅ install.sh (instalación automática)
- ✅ generate-icons.py (generador de iconos)

---

## 🎯 Características

### 6 Fases Completadas:

1. **Interfaz moderna** - Diseño profesional
2. **Separación de conceptos** - Compatibilidad, Optimización, Diagnóstico
3. **Recomendaciones inteligentes** - Filtrado por hardware
4. **Dos modos** - Guiado y Experto
5. **Perfiles guardados** - Drag & drop
6. **Benchmarks** - Análisis de MangoHud

### Funcionalidades:

- ✅ Detección automática de hardware
- ✅ Búsqueda en Steam Store
- ✅ Recomendaciones honestas
- ✅ Sistema de perfiles
- ✅ Drag & drop para reordenar
- ✅ Exportar/importar perfiles
- ✅ Análisis de benchmarks
- ✅ Comparación de rendimiento
- ✅ Modo experto
- ✅ Toggle "Probar con Proton"

---

## 🐛 Si Algo Sale Mal

### Error: "Cannot find module"
```bash
npm install
```

### Error de compilación Rust
```bash
cd src-tauri
cargo clean
cargo build
```

### Faltan dependencias del sistema

**Ubuntu/Debian:**
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file
```

**Fedora:**
```bash
sudo dnf install webkit2gtk4.1-devel gcc gcc-c++ make pkgconf openssl-devel
```

**Arch:**
```bash
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl
```

---

## 📞 Soporte

Si tienes problemas:
1. Revisa el README.md completo
2. Consulta la documentación de cada fase
3. Verifica que tengas todas las dependencias instaladas

---

**¡Disfruta de Thoriapp!** 🎮🐧
