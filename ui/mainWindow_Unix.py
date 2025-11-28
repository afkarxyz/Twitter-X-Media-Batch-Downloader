import os
import qtawesome as qta
from PyQt6.QtWidgets import QApplication, QLabel
from PyQt6.QtCore import Qt, QSize, QEvent
from PyQt6.QtGui import QIcon, QPalette, QColor
from ui.mainWindow import TwitterXMediaBatchDownloaderGUI

class TwitterXMediaBatchDownloaderGUI_Unix(TwitterXMediaBatchDownloaderGUI):
    def __init__(self):
        super().__init__()
        self.setup_unix_specific()
        
        QApplication.instance().installEventFilter(self)
    
    def eventFilter(self, obj, event):
        if event.type() == QEvent.Type.ApplicationPaletteChange:
            self.update_button_icons_unix()
            self.update_ui_colors()
        return super().eventFilter(obj, event)
    
    def update_ui_colors(self):
        from PyQt6.QtWidgets import QLineEdit, QTextEdit, QComboBox, QCheckBox, QPushButton, QProgressBar, QDateEdit, QListWidget, QTabWidget
        
        new_palette = QApplication.palette()
        
        self.account_list.setStyleSheet("""
            QListWidget {
                background-color: palette(base);
                padding: 4px;
                outline: none;
            }
            QListWidget::item {
                padding: 3px 6px;
                margin: 0px 0px;
                border: none;
                outline: none;
            }
            QListWidget::item:selected {
                background-color: palette(highlight);
                border: none;
                outline: none;
            }
            QListWidget::item:focus {
                border: none;
                outline: none;
            }
            QListWidget::item:hover {
                background-color: palette(highlight);
            }
        """)
        
        widget_types = [QLabel, QLineEdit, QTextEdit, QComboBox, QCheckBox, QPushButton, QProgressBar, QDateEdit, QListWidget, QTabWidget]
        
        for widget_type in widget_types:
            for widget in self.findChildren(widget_type):
                widget.setPalette(new_palette)
                widget.update()
        
        for i in range(self.account_list.count()):
            item = self.account_list.item(i)
            if item:
                self.account_list.update(self.account_list.indexFromItem(item))
        
        self.setPalette(new_palette)
        self.update()
    
    def setup_unix_specific(self):
        self.setFixedWidth(670)
        
        if hasattr(self, 'fetch_btn'):
            self.fetch_btn.setFixedWidth(100)
        
        if hasattr(self, 'start_date_edit'):
            self.start_date_edit.setMinimumWidth(130)
        
        if hasattr(self, 'end_date_edit'):
            self.end_date_edit.setMinimumWidth(130)
        
        if hasattr(self, 'output_browse'):
            self.output_browse.setFixedWidth(100)
        
        self.account_list.setStyleSheet("""
            QListWidget {
                background-color: palette(base);
                padding: 4px;
                outline: none;
            }
            QListWidget::item {
                padding: 3px 6px;
                margin: 0px 0px;
                border: none;
                outline: none;
            }
            QListWidget::item:selected {
                background-color: palette(highlight);
                border: none;
                outline: none;
            }
            QListWidget::item:focus {
                border: none;
                outline: none;
            }
            QListWidget::item:hover {
                background-color: palette(highlight);
            }
        """)
        
        self.update_button_icons_unix()
    
    def create_colored_icon(self, icon_name, color=None):
        palette = QApplication.palette()
        text_color = palette.color(QPalette.ColorRole.Text)
        color_hex = text_color.name()
        return qta.icon(icon_name, color=color_hex)
    
    def update_button_icons(self):
        self.update_button_icons_unix()
    
    def update_button_icons_unix_dashboard(self):
        if not hasattr(self, 'import_btn'):
            return
        
        self.import_btn.setIcon(self.create_colored_icon('fa6s.file-import'))
        self.export_btn.setIcon(self.create_colored_icon('fa6s.file-export'))
        self.download_selected_btn.setIcon(self.create_colored_icon('fa6s.download'))
        self.update_selected_btn.setIcon(self.create_colored_icon('fa6s.arrows-rotate'))
        self.delete_btn.setIcon(self.create_colored_icon('fa6s.trash'))
    
    def update_button_icons_unix(self):
        self.update_button_icons_unix_dashboard()
        
        if hasattr(self, 'stop_btn'):
            self.stop_btn.setIcon(self.create_colored_icon('fa6s.circle-xmark'))
        if hasattr(self, 'pause_resume_btn'):
            if self.pause_resume_btn.text() == ' Resume':
                self.pause_resume_btn.setIcon(self.create_colored_icon('fa6s.play'))
            else:
                self.pause_resume_btn.setIcon(self.create_colored_icon('fa6s.pause'))
        if hasattr(self, 'stop_fetch_btn'):
            self.stop_fetch_btn.setIcon(self.create_colored_icon('fa6s.circle-xmark'))
        if hasattr(self, 'pause_fetch_btn'):
            if self.pause_fetch_btn.text() == ' Resume':
                self.pause_fetch_btn.setIcon(self.create_colored_icon('fa6s.play'))
            else:
                self.pause_fetch_btn.setIcon(self.create_colored_icon('fa6s.pause'))
    
    def setup_theme_tab(self):
        pass
    
    def change_theme_color(self, color, clicked_btn=None):
        pass
    
    def toggle_pause_resume(self):
        if hasattr(self, 'worker'):
            if self.worker.is_paused:
                self.worker.resume()
                self.pause_resume_btn.setText(' Pause')
                self.pause_resume_btn.setIcon(self.create_colored_icon('fa6s.pause'))
                self.timer.start(1000)
            else:
                self.worker.pause()
                self.pause_resume_btn.setText(' Resume')
                self.pause_resume_btn.setIcon(self.create_colored_icon('fa6s.play'))
    
    def pause_batch_fetch(self):
        self.is_auto_fetching = False
        
        if hasattr(self, 'metadata_worker') and self.metadata_worker.isRunning():
            self.metadata_worker.terminate()
            self.metadata_worker.wait()
        
        self.pause_fetch_btn.setText(' Resume')
        self.pause_fetch_btn.setIcon(self.create_colored_icon('fa6s.play'))
        self.pause_fetch_btn.clicked.disconnect()
        self.pause_fetch_btn.clicked.connect(self.resume_batch_fetch)
        self.log_output.append('Batch fetching paused.')
    
    def resume_batch_fetch(self):
        self.is_auto_fetching = True
        self.pause_fetch_btn.setText(' Pause')
        self.pause_fetch_btn.setIcon(self.create_colored_icon('fa6s.pause'))
        self.pause_fetch_btn.clicked.disconnect()
        self.pause_fetch_btn.clicked.connect(self.pause_batch_fetch)
        self.log_output.append('Resuming batch fetching...')
        
        if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
            self.fetch_next_account_in_batch()
        else:
            self.fetch_next_batch_internal()
    
    def stop_batch_fetch(self):
        self.is_auto_fetching = False
        self.pause_fetch_btn.hide()
        self.stop_fetch_btn.hide()
        
        if hasattr(self, 'metadata_worker') and self.metadata_worker.isRunning():
            self.metadata_worker.terminate()
            self.metadata_worker.wait()
        
        if self.pause_fetch_btn.text() == ' Resume':
            self.pause_fetch_btn.setText(' Pause')
            self.pause_fetch_btn.setIcon(self.create_colored_icon('fa6s.pause'))
            self.pause_fetch_btn.clicked.disconnect()
            self.pause_fetch_btn.clicked.connect(self.pause_batch_fetch)
        
        if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
            self.log_output.append('Batch processing stopped.')
            self.is_multiple_user_mode = False
            if hasattr(self, 'accounts_to_fetch'):
                del self.accounts_to_fetch
        else:
            self.log_output.append('Auto batch mode stopped.')
        
        self.enable_batch_buttons()
    
    def reset_ui(self):
        super().reset_ui()
        if hasattr(self, 'pause_resume_btn'):
            self.pause_resume_btn.setIcon(self.create_colored_icon('fa6s.pause'))
    
    def on_download_finished(self, success, message):
        self.progress_bar.hide()
        self.stop_btn.hide()
        self.pause_resume_btn.hide()
        self.pause_resume_btn.setText(' Pause')
        self.pause_resume_btn.setIcon(self.create_colored_icon('fa6s.pause'))
        self.stop_timer()
        
        self.download_selected_btn.setEnabled(True)
        self.update_selected_btn.setEnabled(True)
        
        self.pause_fetch_btn.hide()
        self.stop_fetch_btn.hide()
        
        if success:
            self.log_output.append(f"\nStatus: {message}")
        else:
            self.log_output.append(f"Error: {message}")

        self.tab_widget.setCurrentWidget(self.process_tab)
