import sys
import os
from PyQt6.QtWidgets import QApplication
from ui.mainWindow_Unix import TwitterXMediaBatchDownloaderGUI_Unix

def main():
    if getattr(sys, 'frozen', False):
        os.chdir(os.path.dirname(sys.executable))

    app = QApplication(sys.argv)
    app.setStyle('Fusion')
    
    window = TwitterXMediaBatchDownloaderGUI_Unix()
    window.show()
    sys.exit(app.exec())

if __name__ == "__main__":
    main()