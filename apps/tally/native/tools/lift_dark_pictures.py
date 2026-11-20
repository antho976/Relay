"""Bring the near-black Dark pictures up to the charcoal ground the others have, so every Dark card
is the same shade. The charcoal ones share a soft warm glow behind the object fading to ~10 at the
edges; this rebuilds that ground and screen-blends it over the picture (black becomes the ground,
bright stays bright)."""
import numpy as np
from PIL import Image
BASE = np.array([10.0, 9.0, 10.0])
GLOW = np.array([63.0, 55.0, 54.0])
def plate(w, h):
    # Fitted on umbrella_d, wallet_d, suitcase_d, bank_d at 720x823.
    k = 720.0 / w
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    d2 = ((xx * k - 360) ** 2 + (yy * k - 313) ** 2) / 244.0 ** 2
    return BASE + GLOW * np.exp(-d2)[..., None]
def floor(a):
    h, w, _ = a.shape
    return float(np.concatenate([a[: h // 6, : w // 6].reshape(-1, 3), a[: h // 6, -w // 6 :].reshape(-1, 3)]).mean())
def lift(im):
    a = np.asarray(im.convert("RGB")).astype(np.float32)
    if floor(a) >= 8:
        return im
    p = plate(im.width, im.height)
    out = 255 - (255 - a) * (255 - p) / 255
    return Image.fromarray(out.clip(0, 255).round().astype(np.uint8))
