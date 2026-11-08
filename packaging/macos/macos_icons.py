"""Compile the owner's Icon Composer source and a macOS 11.0 bitmap fallback."""
import json
import re
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
ICON_NAME = "AppIcon"
PROJECT_NAME = "twitter-dl.icon"
MINIMUM_TARGET = "11.0"


def validate_source():
    project = ROOT / "packaging/macos" / PROJECT_NAME
    model = json.loads((project / "icon.json").read_text(encoding="utf-8"))
    assets = (project / "Assets").resolve()
    references = []
    for group in model.get("groups", []):
        for layer in group.get("layers", []):
            if "image-name" in layer:
                references.append(layer["image-name"])
            for variant in layer.get("image-name-specializations", []):
                if isinstance(variant.get("value"), str):
                    references.append(variant["value"])
    if not references:
        raise ValueError("Icon Composer project has no artwork")
    for name in references:
        candidate = (assets / name).resolve()
        if not candidate.is_relative_to(assets) or not candidate.is_file():
            raise ValueError(f"Missing or invalid Icon Composer artwork: {name}")
    return project


def build(resources, work):
    resources, work = Path(resources), Path(work)
    source = validate_source()
    xcode = subprocess.check_output(["xcodebuild", "-version"], text=True)
    version = re.search(r"^Xcode (\d+)", xcode)
    if not version or int(version[1]) < 26:
        raise ValueError("Adaptive macOS icons require full Xcode 26 or newer")
    # Give actool a stable logical name shared with the legacy appiconset.
    project = work / f"{ICON_NAME}.icon"
    shutil.copytree(source, project)
    # Retain the existing padded bitmap for systems predating Icon Composer.
    fallback = ROOT / "packaging/macos/app-icon.png"
    iconset = work / "AppIcon.iconset"
    iconset.mkdir()
    catalog = work / "LegacyIcons.xcassets"
    appicons = catalog / f"{ICON_NAME}.appiconset"
    appicons.mkdir(parents=True)
    images = []
    for size in (16, 32, 128, 256, 512):
        for scale, suffix in ((1, ""), (2, "@2x")):
            filename = f"icon_{size}x{size}{suffix}.png"
            raster = iconset / filename
            subprocess.run([
                "sips", "-z", str(size * scale), str(size * scale),
                str(fallback), "--out", str(raster),
            ], check=True, stdout=subprocess.DEVNULL)
            shutil.copy2(raster, appicons / filename)
            images.append({"idiom": "mac", "size": f"{size}x{size}", "scale": f"{scale}x", "filename": filename})
    info = {"author": "xcode", "version": 1}
    (catalog / "Contents.json").write_text(json.dumps({"info": info}), encoding="utf-8")
    (appicons / "Contents.json").write_text(json.dumps({"images": images, "info": info}), encoding="utf-8")
    subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(resources / "AppIcon.icns")], check=True)
    compiled = work / "compiled-icons"
    compiled.mkdir()
    subprocess.run([
        "xcrun", "actool", "--output-format=human-readable-text", "--notices", "--warnings", "--errors",
        "--platform=macosx", "--target-device=mac", f"--minimum-deployment-target={MINIMUM_TARGET}",
        "--lightweight-asset-runtime-mode=enabled",
        "--enable-icon-stack-fallback-generation=disabled", "--include-all-app-icons",
        f"--app-icon={ICON_NAME}", f"--output-partial-info-plist={work / 'icon-info.plist'}",
        f"--compile={compiled}", str(catalog), str(project),
    ], check=True)
    car = compiled / "Assets.car"
    if not car.is_file() or car.stat().st_size == 0:
        raise ValueError("actool did not produce the adaptive icon catalog; Xcode 26 or newer is required")
    shutil.copy2(car, resources / "Assets.car")
    return {"CFBundleIconFile": "AppIcon", "CFBundleIconName": ICON_NAME}


def main():
    import argparse
    import plistlib
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("resources", type=Path)
    parser.add_argument("work", type=Path)
    parser.add_argument("plist", type=Path)
    args = parser.parse_args()
    with args.plist.open("rb") as source:
        info = plistlib.load(source)
    if info.get("LSMinimumSystemVersion") != MINIMUM_TARGET:
        raise ValueError("Icon deployment target must match the app's existing macOS minimum")
    info.update(build(args.resources, args.work))
    with args.plist.open("wb") as output:
        plistlib.dump(info, output)


if __name__ == "__main__":
    main()
