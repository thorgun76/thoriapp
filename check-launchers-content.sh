#!/bin/bash

echo "=== Contenido de Archivos Clave ==="
echo ""

# Ver contenido de un archivo .yml de Lutris
echo "📄 LUTRIS - Ejemplo de archivo .yml:"
if [ -f "$HOME/.local/share/lutris/games/star-valor-1766789644.yml" ]; then
    echo "Archivo: star-valor-1766789644.yml"
    cat "$HOME/.local/share/lutris/games/star-valor-1766789644.yml"
else
    echo "❌ Archivo no encontrado"
fi
echo ""
echo "---"
echo ""

# Ver contenido de library.json de Heronite
echo "📄 HERONITE - library.json:"
if [ -f "$HOME/.local/share/heronite/local_library/library.json" ]; then
    echo "Archivo: ~/.local/share/heronite/local_library/library.json"
    cat "$HOME/.local/share/heronite/local_library/library.json" | head -100
else
    echo "❌ Archivo no encontrado"
fi
echo ""
echo "---"
echo ""

# Ver contenido de legendary_library.json de Heronite
echo "📄 HERONITE - legendary_library.json:"
if [ -f "$HOME/.local/share/heronite/store_cache/legendary_library.json" ]; then
    echo "Archivo: ~/.local/share/heronite/store_cache/legendary_library.json"
    cat "$HOME/.local/share/heronite/store_cache/legendary_library.json" | head -100
else
    echo "❌ Archivo no encontrado"
fi
echo ""
echo "---"
echo ""

# Ver contenido de un archivo JSON de Heroic (uno que parezca ser de instalación)
echo "📄 HEROIC - Ejemplo de archivo de metadata:"
if [ -f "$HOME/.config/heroic/legendaryConfig/legendary/metadata/Sugar.json" ]; then
    echo "Archivo: Sugar.json (primeras 50 líneas)"
    cat "$HOME/.config/heroic/legendaryConfig/legendary/metadata/Sugar.json" | head -50
else
    echo "❌ Archivo no encontrado"
fi
echo ""
echo "---"
echo ""

# Ver si hay archivos de configuración de juegos instalados en Heroic
echo "📄 HEROIC - GamesConfig:"
if [ -d "$HOME/.config/heroic/GamesConfig" ]; then
    echo "Archivos en GamesConfig:"
    ls -la "$HOME/.config/heroic/GamesConfig"
    echo ""
    echo "Contenido de un archivo de ejemplo:"
    first_config=$(find "$HOME/.config/heroic/GamesConfig" -name "*.json" -type f | head -1)
    if [ -n "$first_config" ]; then
        echo "Archivo: $first_config"
        cat "$first_config"
    fi
else
    echo "❌ Directorio GamesConfig no encontrado"
fi
echo ""

echo "=== Fin del diagnóstico ==="
