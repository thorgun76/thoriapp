#!/bin/bash

echo "=== Diagnóstico de Launchers ==="
echo ""

# Verificar Lutris
echo "📂 LUTRIS:"
if [ -d "$HOME/.local/share/lutris" ]; then
    echo "✅ Directorio existe: ~/.local/share/lutris"
    echo "   Archivos .yml:"
    find "$HOME/.local/share/lutris" -maxdepth 1 -name "*.yml" -type f | head -10
    echo ""
    echo "   Contenido de un archivo .yml (ejemplo):"
    first_yml=$(find "$HOME/.local/share/lutris" -maxdepth 1 -name "*.yml" -type f | head -1)
    if [ -n "$first_yml" ]; then
        cat "$first_yml" | head -20
    fi
else
    echo "❌ Directorio NO existe: ~/.local/share/lutris"
fi
echo ""

# Verificar Heroic
echo "📂 HEROIC:"
if [ -d "$HOME/.config/heroic" ]; then
    echo "✅ Directorio existe: ~/.config/heroic"
    echo "   Subdirectorios:"
    ls -la "$HOME/.config/heroic" | grep "^d"
    echo ""
    echo "   Archivos JSON en legendaryConfig:"
    if [ -d "$HOME/.config/heroic/legendaryConfig" ]; then
        find "$HOME/.config/heroic/legendaryConfig" -name "*.json" -type f | head -5
    fi
    echo ""
    echo "   Archivos JSON en gog_store:"
    if [ -d "$HOME/.config/heroic/gog_store" ]; then
        find "$HOME/.config/heroic/gog_store" -name "*.json" -type f | head -5
    fi
else
    echo "❌ Directorio NO existe: ~/.config/heroic"
fi
echo ""

# Verificar Heronite
echo "📂 HERONITE:"
if [ -d "$HOME/.config/heronite" ]; then
    echo "✅ Directorio existe: ~/.config/heronite"
    echo "   Contenido:"
    ls -la "$HOME/.config/heronite"
    echo ""
    echo "   Archivos JSON:"
    find "$HOME/.config/heronite" -name "*.json" -type f | head -10
elif [ -d "$HOME/.local/share/heronite" ]; then
    echo "✅ Directorio existe: ~/.local/share/heronite"
    echo "   Contenido:"
    ls -la "$HOME/.local/share/heronite"
    echo ""
    echo "   Archivos JSON:"
    find "$HOME/.local/share/heronite" -name "*.json" -type f | head -10
else
    echo "❌ Directorios NO existen: ~/.config/heronite ni ~/.local/share/heronite"
fi
echo ""

echo "=== Fin del diagnóstico ==="
