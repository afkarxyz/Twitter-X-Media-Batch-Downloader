# Building Extractor Binary

This guide explains how to build the `extractor` binary from `twitter_cli.py`.

## Quick Start - Use Pre-built Binary

**You don't need to build the binary yourself!** After running the main application once, the pre-built binary is automatically extracted to:

```
Windows: C:\Users\YourUsername\.twitterxmediabatchdownloader\extractor.exe
Linux:   ~/.twitterxmediabatchdownloader/extractor
macOS:   ~/.twitterxmediabatchdownloader/extractor
```

You can use this binary directly for CLI operations without rebuilding.

### Using the Pre-built Binary

```powershell
# Windows
C:\Users\YourUsername\.twitterxmediabatchdownloader\extractor.exe https://x.com/username/media --limit 10 --json --guest

# Linux/macOS
~/.twitterxmediabatchdownloader/extractor https://x.com/username/media --limit 10 --json --guest
```

---

## Building from Source (Optional)

If you need to rebuild the binary (e.g., after modifying `twitter_cli.py`):

### Prerequisites

```bash
pip install pyinstaller
# or
pip install nuitka
```

### Build Commands

```bash
# PyInstaller (faster build, larger file)
pyinstaller --onefile --name extractor --collect-all gallery_dl twitter_cli.py

# Nuitka (slower build, better performance, smaller file)
python -m nuitka --onefile --output-filename=extractor.exe --include-package=gallery_dl --include-package-data=gallery_dl twitter_cli.py
```

### Output Location

- PyInstaller: `dist/extractor` or `dist/extractor.exe`
- Nuitka: `extractor` or `extractor.exe` (in current directory)