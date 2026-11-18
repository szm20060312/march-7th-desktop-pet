"""Deterministic extraction only: no generated or painted animation frames.
The sources are ImageGen outputs. Selected figure pixels are retained inside padded
alpha bounds; uniform row transforms preserve the generated vertical motion.
"""
from pathlib import Path
import hashlib
import json
import argparse
import re
from collections import deque
import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument("--output", default="qa-generated")
parser.add_argument("--left-source", default="running-left-v2-source.png")
args = parser.parse_args()
if not re.fullmatch(r"qa-[a-z0-9-]+", args.output) or Path(args.left_source).name != args.left_source:
    raise ValueError("Use a local versioned QA folder and a local source filename")
OUT = ROOT / args.output
OUT.mkdir(exist_ok=True)
CW, CH, PAD = 192, 208, 8
ROWS = [
    ("idle", 0, 6, 280),
    ("running-right", 1, 8, 90),
    ("running-left", 2, 8, 90),
    ("waving", 3, 4, 180),
    ("jumping", 4, 5, 140),
    ("look-a", 9, 8, 350),
    ("look-b", 10, 8, 350),
]

def digest(file):
    return hashlib.sha256(file.read_bytes()).hexdigest()

def separated_figures(alpha, expected):
    """Extract disconnected generated figures even when their X bounds overlap.
    Pixel ownership follows original nonzero alpha connectivity, not guessed
    cell cuts. No missing pixels or poses are synthesized.
    """
    height, width = alpha.shape
    active = (alpha > 0).ravel().copy()
    owners = np.zeros(active.shape, dtype=np.int32)
    objects, component = [], 0
    for seed in np.flatnonzero(active):
        if not active[seed]:
            continue
        component += 1
        active[seed] = False
        queue = deque([int(seed)])
        count, left, right, top, bottom = 0, width, 0, height, 0
        while queue:
            point = queue.popleft()
            owners[point] = component
            y, x = divmod(point, width)
            count += 1
            left, right, top, bottom = min(left, x), max(right, x), min(top, y), max(bottom, y)
            for yy in range(max(0, y - 1), min(height, y + 2)):
                for xx in range(max(0, x - 1), min(width, x + 2)):
                    index = yy * width + xx
                    if active[index]:
                        active[index] = False
                        queue.append(index)
        objects.append({"id": component, "pixels": count, "box": (left, top, right + 1, bottom + 1)})
    figures = sorted((obj for obj in objects if obj["pixels"] > 100), key=lambda obj: obj["box"][0])
    ignored = sum(obj["pixels"] for obj in objects if obj["pixels"] <= 100)
    if len(figures) != expected or ignored > np.count_nonzero(alpha) * .01:
        raise ValueError(f"Cannot safely isolate {expected} figures: {len(figures)} figures, {ignored} detached pixels")
    return owners.reshape(alpha.shape), figures, ignored

def extract(name, expected):
    source = ROOT / (args.left_source if name == "running-left" else f"{name}-source.png")
    image = Image.open(source).convert("RGBA")
    alpha = np.array(image.getchannel("A"))
    active = (alpha > 16).sum(axis=0) > 2
    ranges, start = [], None
    for x, value in enumerate(active.tolist() + [False]):
        if value and start is None:
            start = x
        elif not value and start is not None:
            if x - start >= 12:
                ranges.append((start, x))
            start = None
    owners, figures, ignored = None, None, 0
    if len(ranges) != expected:
        owners, figures, ignored = separated_figures(alpha, expected)
        ranges = [(obj["box"][0], obj["box"][2]) for obj in figures]
    if (alpha[:, 0] > 16).any() or (alpha[:, -1] > 16).any():
        raise ValueError(f"{name}: source touches outer edge")
    ys = np.where((alpha > 16).any(axis=1))[0]
    y0, y1 = max(0, int(ys[0]) - 2), min(image.height, int(ys[-1]) + 3)
    # Use one vertical box and one scale per row. In particular, never align
    # each jump frame's feet independently: that would erase the hop.
    row_height = y1 - y0
    first_alpha = alpha[:, ranges[0][0]:ranges[0][1]]
    first_ys = np.where((first_alpha > 16).any(axis=1))[0]
    neutral_height = int(first_ys[-1] - first_ys[0] + 1)
    widest = max(right - left + 4 for left, right in ranges)
    scale = min(180 / neutral_height, (CW - 2 * PAD) / widest, (CH - 2 * PAD) / row_height)
    frames, geometry = [], []
    for index, (left, right) in enumerate(ranges):
        bounds = (max(0, left - 2), y0, min(image.width, right + 2), y1)
        if owners is None:
            crop = image.crop(bounds)
        else:
            pixels = np.array(image)
            pixels[:, :, 3] = np.where(owners == figures[index]["id"], alpha, 0)
            crop = Image.fromarray(pixels).crop(bounds)
        size = (round(crop.width * scale), round(crop.height * scale))
        crop = crop.resize(size, Image.Resampling.LANCZOS)
        frame = Image.new("RGBA", (CW, CH))
        frame.alpha_composite(crop, ((CW - size[0]) // 2, CH - PAD - size[1]))
        bbox = frame.getchannel("A").getbbox()
        if bbox is None or bbox[0] < 3 or bbox[1] < 3 or bbox[2] > CW - 3 or bbox[3] > CH - 3:
            raise ValueError(f"{name}: empty or clipped output {bbox}")
        frames.append(frame)
        geometry.append({"sourceBounds": bounds, "outputAlphaBounds": bbox})
    return frames, {"file": source.name, "sha256": digest(source), "scale": scale,
                    "extraction": "alpha-components" if owners is not None else "separated-columns",
                    "ignoredDetachedAlphaPixels": ignored, "frames": geometry}

atlas = Image.new("RGBA", (CW * 8, CH * 11))
contact = Image.new("RGB", (CW * 8, (CH + 30) * len(ROWS)), "#f1f0ee")
draw = ImageDraw.Draw(contact)
font = ImageFont.load_default(size=17)
manifest = {"status": "candidate-awaiting-visual-review", "format": "desktop-app-atlas", "codexCompatible": False,
            "cellWidth": CW, "cellHeight": CH, "columns": 8, "rows": 11,
            "unusedRows": [5, 6, 7, 8], "sources": {}, "animations": {}}
for display_row, (name, atlas_row, count, duration) in enumerate(ROWS):
    frames, provenance = extract(name, count)
    manifest["sources"][name] = provenance
    manifest["animations"][name] = {"row": atlas_row, "frameCount": count, "frameIntervalMs": duration}
    gif = []
    for column, frame in enumerate(frames):
        atlas.alpha_composite(frame, (column * CW, atlas_row * CH))
        preview_y = display_row * (CH + 30)
        contact.paste(frame, (column * CW, preview_y + 30), frame)
        label = str((0 if name == "look-a" else 180) + column * 22.5) if name.startswith("look-") else f"{column + 1}/{count}"
        draw.text((column * CW + 5, preview_y + 5), f"{name} {label}", fill="#252525", font=font)
        background = Image.new("RGBA", frame.size, "#f1f0ee")
        background.alpha_composite(frame)
        gif.append(background.convert("RGB"))
    gif[0].save(OUT / f"{name}.gif", save_all=True, append_images=gif[1:], duration=duration, loop=0, disposal=2)
atlas.save(OUT / "spritesheet.webp", lossless=True, method=6)
contact.save(OUT / "contact-sheet.png")
manifest["atlasSha256"] = digest(OUT / "spritesheet.webp")
manifest["frameCount"] = sum(row[2] for row in ROWS)
(OUT / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf8")
print(json.dumps({"output": str(OUT), "frames": manifest["frameCount"], "size": atlas.size,
                  "unusedRows": manifest["unusedRows"], "status": manifest["status"]}))
