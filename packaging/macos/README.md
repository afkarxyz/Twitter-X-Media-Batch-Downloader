# macOS packaging

The app uses the owner's original `twitter-dl.icon` project, copied byte-for-byte
from the MacBook export. `icon-source.json` records the imported file hashes.
Packaging compiles it with full Xcode 26+ into `Assets.car`, under the shared
`AppIcon` bundle name, before signing the app. The existing padded `.icns` and
legacy catalog remain available on older macOS versions; the deployment floor
stays at 11.0. The running app preserves the bundle icon.

Only the macOS build runner requires Xcode 26+. Native compilation and appearance
selection are handled by Apple's tools; no layers, scales or appearance settings
in the supplied project are rewritten. Update the `.icon` source in Icon Composer
and replace the package here to change the native design.

The DMG has a 720 x 460 point layout, a 1x/2x background using the bundled
Cera Round font, a centre arrow without icon backplates, and the system's
`/Applications` shortcut. Finder chooses that folder's icon on the user's Mac.
`macos_dmg.py` uses [dmgbuild](https://dmgbuild.readthedocs.io/en/latest/settings.html)
without Finder automation. Dependencies are pinned with wheel hashes in
`requirements.txt` and installed into a private virtual environment by packaging.

After creation, the packaging runner verifies the image, mounts it read-only,
and checks the bundled executable, native icon catalog and legacy icon, the Applications symlink, background,
Finder window bounds and icon positions, then verifies the app signature.
The verification mount is detached even when validation fails.

Regenerate artwork after editing the SVG or app logo:

```sh
python packaging/macos/render-assets.py
```

Regeneration requires Pillow, fontTools and `rsvg-convert`. Text is outlined
from the existing bundled fonts, so release builds do not depend on host fonts.
`app-icon.png` fits the original artwork within 824 px of a transparent 1024 px
canvas, preserving aspect ratio and consistent Dock margins. The original logo
stays unchanged for Windows, Linux and the interface.

Portable packaging contract checks (no macOS tools required):

```sh
python -B -m unittest discover -s packaging/macos -p 'test_*.py'
```

Native DMG generation, signing and Finder appearance require a macOS runner;
they cannot be exercised on Windows.
