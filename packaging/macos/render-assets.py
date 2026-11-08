"""Regenerate macOS icon padding and DMG artwork from existing app assets."""
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
NS = "http://www.w3.org/2000/svg"


def outlined_svg():
    from fontTools.ttLib import TTFont
    from fontTools.pens.svgPathPen import SVGPathPen
    tree = ET.parse(ROOT / "packaging/macos/dmg-background.svg")
    import base64
    for element in tree.getroot().iter(f"{{{NS}}}image"):
        path = (ROOT / "packaging/macos" / element.attrib["href"]).resolve()
        element.set("href", "data:image/png;base64," + base64.b64encode(path.read_bytes()).decode())
    fonts = {}
    for parent in tree.getroot().iter():
        for element in list(parent):
            if element.tag != f"{{{NS}}}text":
                continue
            text = "".join(element.itertext())
            size = float(element.get("font-size", "13"))
            weight = int(element.get("font-weight", "400"))
            face = "Bold" if weight >= 700 else "Medium" if weight >= 500 else "Regular"
            if face not in fonts:
                fonts[face] = TTFont(ROOT / f"assets/fonts/Cera_Round_{face}.ttf")
            font = fonts[face]
            glyphs = font.getGlyphSet()
            cmap = font.getBestCmap()
            names = [cmap[ord(character)] for character in text]
            kern = {}
            if "kern" in font:
                for table in font["kern"].kernTables:
                    kern.update(table.kernTable)
            scale = size / font["head"].unitsPerEm
            spacing = float(element.get("letter-spacing", "0"))
            positions = []
            advance = 0.0
            previous = None
            for name in names:
                if previous:
                    advance += kern.get((previous, name), 0) * scale + spacing
                positions.append(advance)
                advance += font["hmtx"][name][0] * scale
                previous = name
            x, y = float(element.get("x", "0")), float(element.get("y", "0"))
            if element.get("text-anchor") == "middle":
                x -= advance / 2
            group = ET.Element(f"{{{NS}}}g", {"fill": element.get("fill", "#000"), "aria-label": text, "data-font": f"Cera Round {face}"})
            for name, offset in zip(names, positions):
                pen = SVGPathPen(glyphs)
                glyphs[name].draw(pen)
                data = pen.getCommands()
                if data:
                    ET.SubElement(group, f"{{{NS}}}path", {"d": data, "transform": f"translate({x + offset:.6f} {y}) scale({scale:.8f} {-scale:.8f})"})
            parent.insert(list(parent).index(element), group)
            parent.remove(element)
    ET.register_namespace("", NS)
    return ET.tostring(tree.getroot(), encoding="utf-8", xml_declaration=True)



def render_icon():
    from PIL import Image
    with Image.open(ROOT / "assets/logo/logo.png") as original:
        image = original.convert("RGBA")
    bounds = image.getchannel("A").getbbox()
    if not bounds:
        raise ValueError("App icon is empty")
    image = image.crop(bounds)
    image.thumbnail((824, 824), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (1024, 1024))
    canvas.alpha_composite(image, ((1024 - image.width) // 2, (1024 - image.height) // 2))
    canvas.save(ROOT / "packaging/macos/app-icon.png")


def main():
    render_icon()
    with tempfile.TemporaryDirectory(prefix="macos-dmg-art-") as directory:
        source = Path(directory) / "outlined.svg"
        source.write_bytes(outlined_svg())
        for scale, suffix in ((1, ""), (2, "@2x")):
            subprocess.run(["rsvg-convert", "-w", str(720 * scale), "-h", str(460 * scale), "-o", str(ROOT / f"packaging/macos/dmg-background{suffix}.png"), str(source)], check=True)
    print("Rendered bundled font outlines at 1x/2x and a padded standard app icon.")


if __name__ == "__main__":
    main()
