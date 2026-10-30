#!/bin/bash

# Script de instalación de Thoriapp
# Ejecuta este script DESPUÉS de descargar todos los archivos

set -e

echo "🎮 Instalando Thoriapp..."
echo ""

# Verificar que estamos en el directorio correcto
if [ ! -f "package.json" ]; then
    echo "❌ Error: No se encontró package.json"
    echo "Por favor, ejecuta este script desde el directorio tauri-app"
    exit 1
fi

# Instalar dependencias de Node
echo "📦 Instalando dependencias de Node..."
npm install

# Generar iconos
echo "🎨 Generando iconos..."
if command -v python3 &> /dev/null; then
    python3 generate-icons.py 2>/dev/null || echo "⚠️  No se pudo generar iconos automáticamente (opcional)"
else
    echo "⚠️  Python3 no encontrado, generando iconos manualmente..."
    mkdir -p src-tauri/icons
    # Crear iconos vacíos para que compile
    touch src-tauri/icons/32x32.png
    touch src-tauri/icons/128x128.png
    touch src-tauri/icons/128x128@2x.png
    touch src-tauri/icons/icon.ico
    touch src-tauri/icons/icon.icns
fi

# Limpiar caché
echo "🧹 Limpiando caché..."
rm -rf node_modules/.vite

echo ""
echo "✅ ¡Instalación completada!"
echo ""
echo "🚀 Para ejecutar la aplicación:"
echo "   npm run tauri dev"
echo ""
echo "📦 Para compilar para producción:"
echo "   npm run tauri build"
echo ""
