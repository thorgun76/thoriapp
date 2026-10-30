#!/usr/bin/env python3
from PIL import Image, ImageDraw, ImageFont
import os

os.makedirs('src-tauri/icons', exist_ok=True)

size = 512
img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
draw = ImageDraw.Draw(img)

for i in range(size):
    r = int(124 + (59 - 124) * i / size)
    g = int(58 + (130 - 58) * i / size)
    b = int(237 + (246 - 237) * i / size)
    for x in range(size):
        center_x, center_y = size // 2, size // 2
        distance = ((x - center_x) ** 2 + (i - center_y) ** 2) ** 0.5
        if distance <= size // 2:
            img.putpixel((x, i), (r, g, b, 255))

try:
    font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 200)
except:
    font = ImageFont.load_default()

text = "T"
bbox = draw.textbbox((0, 0), text, font=font)
text_width = bbox[2] - bbox[0]
text_height = bbox[3] - bbox[1]
x = (size - text_width) // 2
y = (size - text_height) // 2
draw.text((x, y), text, fill=(255, 255, 255, 255), font=font)

img.save('src-tauri/icons/icon.png')

sizes = [(32, '32x32.png'), (128, '128x128.png'), (256, '128x128@2x.png')]
for s, name in sizes:
    img_resized = img.resize((s, s), Image.Resampling.LANCZOS)
    img_resized.save(f'src-tauri/icons/{name}')

img.save('src-tauri/icons/icon.ico', format='ICO', sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (256, 256)])
open('src-tauri/icons/icon.icns', 'w').close()

print("✅ Iconos generados")
