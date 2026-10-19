"""Find the dark slot in a slide-captcha screenshot.

The horizontal answer stays on the server. This matches the picture on screen
to the same artwork the window shows, then places the piece shape on that row.
It does not read the server answer.
"""

from __future__ import annotations

from pathlib import Path

IMAGE_W = 600
IMAGE_H = 340
PAD = 12
PIECE_W = 56
ASSETS = Path(__file__).resolve().parents[4] / "web" / "public" / "captcha"


def answer_x(stage, piece_top_ratio: float) -> int:
    """Return the slot x in the 600-wide puzzle image."""
    from PIL import Image

    if not ASSETS.is_dir():
        raise RuntimeError("captcha artwork is not available")
    image = stage.convert("RGB").resize((IMAGE_W, IMAGE_H), Image.Resampling.BOX)
    piece_path = ASSETS / f"piece-{_identify(image)}.png"
    piece = Image.open(piece_path).convert("RGBA")
    origin_y = int(round(piece_top_ratio * IMAGE_H))
    origin_x, _score = _match(image, piece, origin_y)
    return origin_x + PAD


def _identify(stage) -> str:
    from PIL import Image

    signature = _signature(stage)
    best_id = ""
    best_score: float | None = None
    for path in sorted(ASSETS.glob("bg-*.jpg")):
        artwork = Image.open(path).convert("RGB")
        score = _mae(signature, _signature(artwork))
        if best_score is None or score < best_score:
            best_score = score
            best_id = path.stem.removeprefix("bg-")
    if not best_id:
        raise RuntimeError("captcha artwork is empty")
    return best_id


def _signature(image):
    from PIL import Image

    small = image.convert("RGB").resize((100, 57), Image.Resampling.BOX)
    return list(small.getdata())


def _mae(left, right) -> float:
    """Ignore the left band, where the loose piece covers the picture."""
    total = 0
    count = 0
    for index, (one, other) in enumerate(zip(left, right)):
        if index % 100 < 18:
            continue
        total += abs(one[0] - other[0]) + abs(one[1] - other[1]) + abs(one[2] - other[2])
        count += 1
    return total / max(1, count)


def _match(stage, piece, origin_y: int) -> tuple[int, float]:
    alpha = piece.load()
    pixels = stage.load()
    piece_w, piece_h = piece.size
    opaque = [
        (x, y)
        for y in range(0, piece_h, 2)
        for x in range(0, piece_w, 2)
        if alpha[x, y][3] > 160
    ]
    solid = {(x, y) for y in range(piece_h) for x in range(piece_w) if alpha[x, y][3] > 160}
    ring = []
    for x, y in list(solid)[::6]:
        for dx, dy in ((-4, 0), (4, 0), (0, -4), (0, 4)):
            nx, ny = x + dx, y + dy
            if 0 <= nx < piece_w and 0 <= ny < piece_h and (nx, ny) not in solid:
                ring.append((nx, ny))
    best: tuple[float, int] | None = None
    limit = IMAGE_W - piece_w
    for offset in range(-4, 5):
        top = min(max(0, origin_y + offset), IMAGE_H - piece_h)
        for left in range(0, limit + 1):
            inside = 0
            for dx, dy in opaque:
                red, green, blue = pixels[left + dx, top + dy]
                if red < 50 and green < 50 and blue < 50:
                    inside += 1
            outside = 0
            for dx, dy in ring:
                sample_x = left + dx
                sample_y = top + dy
                if not (0 <= sample_x < IMAGE_W and 0 <= sample_y < IMAGE_H):
                    continue
                red, green, blue = pixels[sample_x, sample_y]
                if red < 50 and green < 50 and blue < 50:
                    outside += 1
            score = inside / max(1, len(opaque)) - outside / max(1, len(ring))
            if best is None or score > best[0]:
                best = (score, left)
    if best is None:
        raise RuntimeError("captcha slot was not visible")
    return best[1], best[0]
