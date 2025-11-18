import sys
import os
import qdarktheme
from PyQt6.QtWidgets import QApplication
from PyQt6.QtCore import QSettings
from ui.mainWindow import TwitterXMediaBatchDownloaderGUI

def main():
    if getattr(sys, 'frozen', False):
        os.chdir(os.path.dirname(sys.executable))

    app = QApplication(sys.argv)
    
    settings = QSettings('TwitterXMediaBatchDownloader', 'Settings')
    theme_color = settings.value('theme_color', '#2196F3')
    
    qdarktheme.setup_theme(
        custom_colors={
            "[dark]": {
                "primary": theme_color,
            }
        }
    )
    window = TwitterXMediaBatchDownloaderGUI()
    window.show()
    sys.exit(app.exec())

if __name__ == "__main__":
    main()