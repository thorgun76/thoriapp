# 🎮 Thoriapp - Optimizador de Juegos para Linux

## 📦 Instalación Rápida

### 1. Descargar todos los archivos

Descarga TODOS los archivos desde el enlace arriba a la derecha.

### 2. Instalar dependencias

```bash
cd tauri-app
chmod +x install.sh
./install.sh
```

### 3. Ejecutar la aplicación

```bash
npm run tauri dev
```

## 🎯 Características

### ✅ 6 Fases Completadas

1. **Interfaz moderna** 
2. **Separación de conceptos** - Compatibilidad y Optimización (dos pestañas, sin duplicar)
3. **Recomendaciones inteligentes** - Filtrado por hardware, toggle Proton
4. **Dos modos** - Guiado (simple) y Experto (detallado)
5. **Perfiles guardados** - Drag & drop, exportar/importar
6. **Benchmarks** - Análisis de MangoHud, historial de las 10 últimas grabaciones por juego

### 🎨 Funcionalidades Principales

- Detección automática de hardware (CPU, GPU, RAM, kernel)
- Búsqueda en Steam Store (todos los juegos)
- Recomendaciones honestas con niveles de evidencia
- Sistema de perfiles para guardar configuraciones
- Drag & drop para reordenar perfiles
- Exportar/importar perfiles como JSON
- Análisis de benchmarks con logs de MangoHud
- Historial por juego: las 10 grabaciones más recientes, las viejas se borran solas
- Diagnóstico de stutter de shaders: primeros segundos frente al resto + estado de la caché en disco
- Comparación de rendimiento antes/después con margen de ruido
- Modo experto con detalles técnicos completos
- Toggle "Probar con Proton" para juegos nativos
- Revisión previa del comando: avisa si falta un wrapper, si MangoHud no es visible desde Steam Flatpak o si una variable de Proton no aplica al juego
- Benchmarks con control de ruido: descarta el arranque y muestra el margen de error de cada grabación
- Aviso de anti-cheat por juego (datos de AreWeAntiCheatYet): dice si el juego funciona, funciona sin soporte oficial o está bloqueado

## 🚀 Uso Básico

### 1. Abrir la app
```bash
npm run tauri dev
```

### 2. Buscar un juego
- Usa el buscador en la parte superior
- Escribe el nombre del juego
- Selecciona de los resultados

### 3. Ver recomendaciones
- Ve a la pestaña "Optimización"
- Activa/desactiva parámetros
- Copia el comando de lanzamiento

### 4. Guardar perfil
- Configura un juego
- Haz clic en "Guardar perfil"
- Dale un nombre y notas opcionales

### 5. Medir rendimiento
- Graba un log con MangoHud (Shift+F2)
- Ve a la pestaña "Benchmarks"
- Analiza el log
- Compara antes/después

## 📋 Estructura del Proyecto

```
tauri-app/
├── src/                          # Frontend React
│   ├── App.tsx                   # Componente principal
│   ├── main.tsx                  # Entry point
│   ├── index.css                 # Estilos
│   └── components/
│       └── BenchmarksView.tsx    # Vista de benchmarks
│
├── src-tauri/                    # Backend Rust
│   ├── src/
│   │   ├── main.rs              # Comandos Tauri
│   │   ├── hw.rs                # Detección de hardware
│   │   ├── preflight.rs         # Estado real del sistema (wrappers, capa Vulkan, sandbox)
│   │   ├── anticheat.rs         # Avisos de anti-cheat (AreWeAntiCheatYet, caché 24 h)
│   │   ├── steam.rs             # API de Steam
│   │   ├── rules.rs             # Sistema de reglas
│   │   ├── engine.rs            # Detección de motor
│   │   ├── profiles.rs          # Sistema de perfiles
│   │   ├── benchmarks.rs        # Análisis de benchmarks
│   │   └── log.rs               # Análisis de logs
│   ├── games.json               # Reglas por juego
│   ├── Cargo.toml               # Dependencias Rust
│   ├── tauri.conf.json          # Configuración Tauri
│   └── capabilities/
│       └── default.json         # Permisos
│
├── package.json                  # Dependencias Node
├── vite.config.ts               # Configuración Vite
├── tsconfig.json                # Configuración TypeScript
├── tailwind.config.js           # Configuración Tailwind
├── install.sh                   # Script de instalación
└── generate-icons.py            # Generador de iconos
```

## 🐛 Solución de Problemas

### Error: "Cannot find module '@dnd-kit'"
```bash
npm install @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities
```

### Error: "tauri.conf.json not found"
Verifica que el archivo está en `src-tauri/tauri.conf.json`

### Error de compilación de Rust
```bash
cd src-tauri
cargo clean
cargo build
```

### La app no abre
Instala las dependencias del sistema:

**Ubuntu/Debian:**
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

**Fedora:**
```bash
sudo dnf install webkit2gtk4.1-devel gcc gcc-c++ make pkgconf openssl-devel curl wget file
```

**Arch:**
```bash
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl
```

## 📊 Requisitos del Sistema

- **Node.js** 18+
- **Rust** 1.70+
- **Python 3** (para generar iconos, opcional)
- **Dependencias del sistema** (webkit2gtk, etc.)

## 📚 Documentación Adicional

- `INSTALACION_SIMPLE.md` - Guía de instalación simplificada
- `FASE_1_2_COMPLETADAS.md` - Reestructuración visual
- `FASE_3_IMPLEMENTADA.md` - Motor de recomendaciones
- `FASE_4_IMPLEMENTADA.md` - Dos modos de experiencia
- `FASE_5_IMPLEMENTADA.md` - Sistema de perfiles
- `FASE_6_IMPLEMENTACION_COMPLETA.md` - Comparador de benchmarks

## 🤝 Contribuir

Las contribuciones son bienvenidas:
1. Fork el repositorio
2. Crea una rama para tu feature
3. Commit tus cambios
4. Push a la rama
5. Abre un Pull Request

## 📄 Licencia

MIT License - Usa, modifica y distribuye libremente.

## 🙏 Créditos

- **Tauri** - Framework para apps de escritorio
- **React** - Frontend library
- **Steam API** - Datos de juegos
- **ProtonDB** - Ratings de compatibilidad
- **MangoHud** - Análisis de rendimiento
- **Comunidad Linux Gaming** - Feedback y testing

---

**Hecho con ❤️ para la comunidad gaming de Linux** 🐧🎮
