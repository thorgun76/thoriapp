#!/bin/bash

echo "=== Diagnóstico Completo de Launchers ==="
echo ""

# Verificar Lutris
echo "📂 LUTRIS:"
if [ -d "$HOME/.local/share/lutris" ]; then
    echo "✅ Directorio existe: ~/.local/share/lutris"
    echo ""
    echo "   Buscando base de datos SQLite:"
    find "$HOME/.local/share/lutris" -name "*.db" -o -name "*.sqlite" 2>/dev/null
    echo ""
    echo "   Buscando archivos de juegos en subdirectorios:"
    find "$HOME/.local/share/lutris" -name "*.yml" -o -name "*.json" 2>/dev/null | head -20
    echo ""
    echo "   Estructura completa del directorio:"
    tree -L 2 "$HOME/.local/share/lutris" 2>/dev/null || find "$HOME/.local/share/lutris" -maxdepth 2 -type f | head -30
else
    echo "❌ Directorio NO existe: ~/.local/share/lutris"
fi
echo ""

# Verificar Heroic
echo "📂 HEROIC:"
if [ -d "$HOME/.config/heroic" ]; then
    echo "✅ Directorio existe: ~/.config/heroic"
    echo ""
    echo "   Todos los archivos JSON en legendaryConfig:"
    find "$HOME/.config/heroic/legendaryConfig" -name "*.json" -type f 2>/dev/null
    echo ""
    echo "   Contenido de un archivo JSON de ejemplo (Sugar.json):"
    if [ -f "$HOME/.config/heroic/legendaryConfig/legendary/metadata/Sugar.json" ]; then
        cat "$HOME/.config/heroic/legendaryConfig/legendary/metadata/Sugar.json" | head -30
    fi
    echo ""
    echo "   Archivos en gog_store:"
    find "$HOME/.config/heroic/gog_store" -name "*.json" -type f 2>/dev/null
    echo ""
    echo "   Archivos en nile_store:"
    find "$HOME/.config/heroic/nile_store" -name "*.json" -type f 2>/dev/null
    echo ""
    echo "   Archivos en sideload_apps:"
    find "$HOME/.config/heroic/sideload_apps" -name "*.json" -type f 2>/dev/null
else
    echo "❌ Directorio NO existe: ~/.config/heroic"
fi
echo ""

# Verificar Heronite
echo "📂 HERONITE:"
if [ -d "$HOME/.local/share/heronite" ]; then
    echo "✅ Directorio existe: ~/.local/share/heronite"
    echo ""
    echo "   Estructura completa:"
    tree -L 3 "$HOME/.local/share/heronite" 2>/dev/null || find "$HOME/.local/share/heronite" -maxdepth 3 -type f | head -50
    echo ""
    echo "   Archivos JSON:"
    find "$HOME/.local/share/heronite" -name "*.json" -type f 2>/dev/null
    echo ""
    echo "   Contenido de ui-state.json (si existe):"
    if [ -f "$HOME/.local/share/heronite/ui-state.json" ]; then
        cat "$HOME/.local/share/heronite/ui-state.json" | head -50
    fi
elif [ -d "$HOME/.config/heronite" ]; then
    echo "✅ Directorio existe: ~/.config/heronite"
    echo ""
    echo "   Buscando en subdirectorios:"
    find "$HOME/.config/heronite" -type d | head -20
    echo ""
    echo "   Buscando archivos de juegos:"
    find "$HOME/.config/heronite" -name "*game*" -o -name "*library*" -o -name "*installed*" 2>/dev/null | head -20
else
    echo "❌ Directorios NO existen"
fi
echo ""

echo "=== Fin del diagnóstico ==="
