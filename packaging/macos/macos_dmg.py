"""Finder-independent, Retina-aware macOS drag-to-install disk image layout."""
from pathlib import Path
import plistlib
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[2]
WINDOW_SIZE = (720, 460)
ICON_LOCATIONS = {"TwitterXMediaBatchDownloader.app": (192, 236), "Applications": (528, 236)}


def png_size(path):
    with Path(path).open("rb") as source:
        header = source.read(24)
    if len(header) != 24 or header[:8] != b"\x89PNG\r\n\x1a\n" or header[12:16] != b"IHDR":
        raise ValueError(f"Invalid DMG background PNG: {path}")
    return struct.unpack(">II", header[16:24])


def settings(app):
    app = Path(app)
    background = ROOT / "packaging/macos/dmg-background.png"
    retina = background.with_name("dmg-background@2x.png")
    if png_size(background) != WINDOW_SIZE or png_size(retina) != tuple(v * 2 for v in WINDOW_SIZE):
        raise ValueError("DMG background dimensions do not match the logical window size")
    if app.name != "TwitterXMediaBatchDownloader.app" or not app.is_dir():
        raise ValueError("Expected the signed TwitterXMediaBatchDownloader.app bundle")
    return {
        "format": "UDZO", "filesystem": "HFS+",
        "files": [str(app)],
        "symlinks": {"Applications": "/Applications"},
        "icon": str(app / "Contents/Resources/AppIcon.icns"),
        # dmgbuild combines the 1x PNG and its @2x sibling into a HiDPI TIFF.
        "background": str(background),
        "window_rect": ((120, 120), WINDOW_SIZE),
        "icon_locations": ICON_LOCATIONS.copy(),
        "icon_size": 112, "text_size": 13, "label_pos": "bottom",
        "default_view": "icon-view", "arrange_by": None,
        "show_status_bar": False, "show_tab_view": False, "show_toolbar": False,
        "show_pathbar": False, "show_sidebar": False, "show_icon_preview": False,
        "include_icon_view_settings": True, "include_list_view_settings": False,
    }


def verify_mounted_layout(mount):
    """Validate the actual mounted artifact on the macOS packaging runner."""
    from ds_store import DSStore
    mount = Path(mount)
    app = mount / "TwitterXMediaBatchDownloader.app"
    applications = mount / "Applications"
    with (app / "Contents/Info.plist").open("rb") as source:
        info = plistlib.load(source)
    if info.get("CFBundleIconFile", "").removesuffix(".icns") != "AppIcon":
        raise ValueError("DMG app does not reference its bundled icon")
    if info.get("CFBundleIconName") != "AppIcon":
        raise ValueError("DMG app does not reference its native icon catalog")
    car = app / "Contents/Resources/Assets.car"
    if not car.is_file() or car.stat().st_size == 0:
        raise ValueError("DMG native icon catalog is missing")
    if not (app / "Contents/Resources/AppIcon.icns").is_file():
        raise ValueError("DMG app icon is missing")
    if not app.is_dir() or not (app / "Contents/MacOS/TwitterXMediaBatchDownloader").is_file():
        raise ValueError("DMG does not contain the Twitter/X Media Batch Downloader app")
    if not applications.is_symlink() or str(applications.readlink()) != "/Applications":
        raise ValueError("DMG Applications shortcut is missing or incorrect")
    if not (mount / ".background.tiff").is_file() or not (mount / ".VolumeIcon.icns").is_file():
        raise ValueError("DMG background or volume icon is missing")
    with DSStore.open(str(mount / ".DS_Store"), "r") as store:
        for name, position in ICON_LOCATIONS.items():
            if tuple(store[name]["Iloc"]) != position:
                raise ValueError(f"Incorrect DMG icon location: {name}")
        view = store["."]["icvp"]
        window = store["."]["bwsp"]
        if view["backgroundType"] != 2 or view["iconSize"] != 112 or view["arrangeBy"] != "none":
            raise ValueError("DMG icon view settings were not saved")
        if not view.get("backgroundImageAlias"):
            raise ValueError("DMG background image alias is missing")
        if any(window[key] for key in ("ShowToolbar", "ShowStatusBar", "ShowSidebar", "ShowPathbar", "ShowTabView")):
            raise ValueError("DMG window chrome does not match the install layout")
        expected = "{{120, 120}, {720, 460}}"
        if window["WindowBounds"] != expected:
            raise ValueError("DMG window bounds do not match the background")
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)


def build(app, asset, work):
    try:
        import dmgbuild
    except ImportError as error:
        raise RuntimeError("Install packaging/macos/requirements.txt with pip --require-hashes before building the macOS DMG") from error
    layout = settings(app)
    dmgbuild.build_dmg(str(asset), 'Twitter/X Media Batch Downloader', settings=layout)
    subprocess.run(["hdiutil", "verify", str(asset)], check=True)
    mount = Path(work) / "verify-dmg"
    mount.mkdir()
    subprocess.run(["hdiutil", "attach", str(asset), "-readonly", "-nobrowse", "-noautoopen", "-mountpoint", str(mount)], check=True)
    try:
        verify_mounted_layout(mount)
    finally:
        try:
            subprocess.run(["hdiutil", "detach", str(mount)], check=True)
        except subprocess.CalledProcessError:
            # Only the read-only verification image at our private mountpoint.
            subprocess.run(["hdiutil", "detach", "-force", str(mount)], check=True)


def main():
    import argparse
    import tempfile
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app", type=Path)
    parser.add_argument("asset", type=Path)
    args = parser.parse_args()
    args.asset.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="macos-dmg-") as directory:
        build(args.app.resolve(), args.asset.resolve(), Path(directory))


if __name__ == "__main__":
    main()
