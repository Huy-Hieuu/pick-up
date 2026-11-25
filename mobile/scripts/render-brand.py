"""Render the simple vector brand mark for Expo's native PNG requirements."""
from pathlib import Path
from PIL import Image, ImageDraw

assets = Path(__file__).resolve().parent.parent / "assets"
scale = 3
points = [(240, 512), (385, 512), (463, 306), (575, 718), (642, 512), (784, 512)]


def render(name, background, monochrome=False):
    image = Image.new("RGBA", (1024 * scale, 1024 * scale), background)
    draw = ImageDraw.Draw(image)
    color = "#FFFFFF" if monochrome else "#D7EE83"
    path = [(x * scale, y * scale) for x, y in points]
    draw.line(path, fill=color, width=60 * scale, joint="curve")
    radius = 30 * scale
    for x, y in path:
        draw.ellipse((x - radius, y - radius, x + radius, y + radius), fill=color)
    image.resize((1024, 1024), Image.Resampling.LANCZOS).save(assets / name)


render("icon.png", "#193E32")
render("adaptive-icon.png", (0, 0, 0, 0))
render("monochrome-icon.png", (0, 0, 0, 0), monochrome=True)
