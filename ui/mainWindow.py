import sys
import os
import requests
import json
from datetime import datetime
from pathlib import Path
from packaging import version
import qdarktheme
from PyQt6.QtWidgets import (
    QApplication, QWidget, QVBoxLayout, QHBoxLayout, QPushButton, QLineEdit,
    QLabel, QFileDialog, QListWidget, QTextEdit, QTabWidget, QAbstractItemView, QProgressBar, QCheckBox, QDialog,
    QDialogButtonBox, QComboBox, QListWidgetItem, QMessageBox, QDateEdit, QInputDialog
)
from PyQt6.QtCore import Qt, QThread, pyqtSignal, QUrl, QTimer, QTime, QSettings, QSize, QDate
from PyQt6.QtNetwork import QNetworkAccessManager, QNetworkRequest
from PyQt6.QtGui import QIcon, QTextCursor, QDesktopServices, QPixmap, QPainter, QPainterPath
from PyQt6.QtSvg import QSvgRenderer

from core.databaseManager import DatabaseManager
from core.metadataWorker import MetadataFetchWorker
from core.downloadWorker import DownloadWorker
from core.accountModel import Account
from ui.dialogs import UpdateDialog

ICON_DIR = os.path.join(os.path.dirname(__file__), 'icons')

class TwitterXMediaBatchDownloaderGUI(QWidget):
    def __init__(self):
        super().__init__()
        self.current_version = "3.8"
        self.accounts = []
        self.temp_dir = os.path.join(Path.home(), ".twitterxmediabatchdownloader")
        os.makedirs(self.temp_dir, exist_ok=True)
        
        db_path = os.path.join(self.temp_dir, "twitterxmediabatchdownloader.db")
        self.db_manager = DatabaseManager(db_path)
        
        self.migrate_json_to_sqlite()
        
        self.reset_state()
        
        self.settings = QSettings('TwitterXMediaBatchDownloader', 'Settings')
        self.last_output_path = self.settings.value('output_path', str(Path.home() / "Pictures"))
        self.last_url = self.settings.value('twitter_url', '')
        self.last_auth_token = self.settings.value('auth_token', '')
        self.filename_format = self.settings.value('filename_format', 'username_date')
        self.download_batch_size = self.settings.value('download_batch_size', 25, type=int)        
        self.batch_mode = self.settings.value('batch_mode', False, type=bool)
        self.batch_size = self.settings.value('batch_size', 100, type=int)
        self.timeline_type = self.settings.value('timeline_type', 'timeline')
        self.media_type = self.settings.value('media_type', 'all')
        self.include_retweets = self.settings.value('include_retweets', False, type=bool)
        self.convert_gif = self.settings.value('convert_gif', False, type=bool)
        self.gif_resolution = self.settings.value('gif_resolution', 'original')
        self.gif_conversion_mode = self.settings.value('gif_conversion_mode', 'better')
        self.check_for_updates = self.settings.value('check_for_updates', True, type=bool)
        self.current_theme_color = self.settings.value('theme_color', '#2196F3')
        self.last_group_filter = self.settings.value('group_filter', None)
        
        self.is_auto_fetching = False
        self.current_fetch_username = None
        self.current_fetch_media_type = None
        self.current_fetch_metadata = None
        self.is_multiple_user_mode = False
        self.is_initializing = True
        
        self.profile_image_cache = {}
        self.pending_downloads = {}
        self.network_manager = QNetworkAccessManager()
        
        self.elapsed_time = QTime(0, 0, 0)
        self.timer = QTimer(self)
        self.timer.timeout.connect(self.update_timer)
        self.initUI()
        self.load_settings()
        self.load_all_cached_accounts()
        self.restore_filter_preferences()
        self.is_initializing = False
        
        if self.check_for_updates:
            QTimer.singleShot(0, self.check_updates)

    def get_time_ago(self, timestamp_str):
        if not timestamp_str:
            return "Unknown"
        
        try:
            fetch_time = datetime.fromisoformat(timestamp_str.replace('Z', '+00:00'))
            now = datetime.now(fetch_time.tzinfo) if fetch_time.tzinfo else datetime.now()

            diff = now - fetch_time
            days = diff.days
            hours = diff.seconds // 3600
            minutes = (diff.seconds % 3600) // 60

            if days > 0:
                return f"{days} day{'s' if days != 1 else ''} ago"
            elif hours > 0:
                return f"{hours} hour{'s' if hours != 1 else ''} ago"
            elif minutes > 0:
                return f"{minutes} minute{'s' if minutes != 1 else ''} ago"
            else:
                return "Just now"
        except (ValueError, TypeError, AttributeError):
            return "Unknown"

    def format_fetch_info(self, timestamp_str):
        if not timestamp_str:
            return "Fetched: Unknown"
        
        try:
            fetch_time = datetime.fromisoformat(timestamp_str.replace('Z', '+00:00'))
            formatted_datetime = fetch_time.strftime("%Y/%m/%d • %H:%M")
            age = self.get_time_ago(timestamp_str)
            return f"Fetched: {formatted_datetime} ({age})"
        except (ValueError, TypeError, AttributeError):
            return "Fetched: Unknown"

    def check_updates(self):
        try:
            response = requests.get("https://raw.githubusercontent.com/afkarxyz/Twitter-X-Media-Batch-Downloader/refs/heads/main/version.json")
            if response.status_code == 200:
                data = response.json()
                new_version = data.get("version")
                
                if new_version and version.parse(new_version) > version.parse(self.current_version):
                    dialog = UpdateDialog(self.current_version, new_version, self)
                    result = dialog.exec()
                    
                    if result == QDialog.DialogCode.Accepted:
                        QDesktopServices.openUrl(QUrl("https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases"))
                        
        except Exception as e:
            pass

    def reset_state(self):
        self.accounts.clear()

    def reset_ui(self):
        self.account_list.clear()
        self.log_output.clear()
        self.progress_bar.setValue(0)
        self.progress_bar.hide()
        self.stop_btn.hide()
        self.pause_resume_btn.hide()
        self.pause_resume_btn.setText(' Pause')
        icon_dir = ICON_DIR
        self.pause_resume_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), self.current_theme_color))
        self.pause_fetch_btn.hide()
        self.stop_fetch_btn.hide()
        self.is_auto_fetching = False
        self.is_multiple_user_mode = False
        self.enable_batch_buttons()
        self.hide_account_buttons()

    def initUI(self):
        self.setWindowTitle('Twitter/X Media Batch Downloader')
        self.setFixedWidth(650)
        self.setMinimumHeight(350)  
        
        icon_path = os.path.join(ICON_DIR, "icon.svg")
        if os.path.exists(icon_path):
            self.setWindowIcon(QIcon(icon_path))
            
        self.main_layout = QVBoxLayout()
        
        self.setup_twitter_section()
        self.setup_tabs()
        
        self.setLayout(self.main_layout)
        
    def setup_twitter_section(self):
        twitter_layout = QHBoxLayout()
        twitter_label = QLabel('Username/URL:')
        twitter_label.setFixedWidth(100)

        self.twitter_url = QLineEdit()
        self.twitter_url.setPlaceholderText("e.g. Takomayuyi or https://x.com/Takomayuyi or user1,user2,user3")
        self.twitter_url.setClearButtonEnabled(True)
        self.twitter_url.setText(self.last_url)
        self.twitter_url.textChanged.connect(self.save_url)
        self.fetch_btn = QPushButton('Fetch')
        self.fetch_btn.setFixedWidth(80)
        self.fetch_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.fetch_btn.clicked.connect(self.fetch_account)

        twitter_layout.addWidget(twitter_label)
        twitter_layout.addWidget(self.twitter_url)
        twitter_layout.addWidget(self.fetch_btn)
        self.main_layout.addLayout(twitter_layout)

        date_range_layout = QHBoxLayout()
        date_range_label = QLabel('Date Range:')
        date_range_label.setFixedWidth(100)

        start_date_label = QLabel('From:')
        self.start_date_edit = QDateEdit()
        self.start_date_edit.setCalendarPopup(True)
        self.start_date_edit.setDisplayFormat("yyyy-MM-dd")
        self.start_date_edit.setDate(QDate.currentDate().addMonths(-1))
        self.start_date_edit.setMinimumWidth(120)

        end_date_label = QLabel('To:')
        self.end_date_edit = QDateEdit()
        self.end_date_edit.setCalendarPopup(True)
        self.end_date_edit.setDisplayFormat("yyyy-MM-dd")
        self.end_date_edit.setDate(QDate.currentDate())
        self.end_date_edit.setMinimumWidth(120)

        self.use_date_range_checkbox = QCheckBox('Use Date Range')
        self.use_date_range_checkbox.setChecked(False)
        self.use_date_range_checkbox.toggled.connect(self.toggle_date_range)

        date_range_layout.addWidget(date_range_label)
        date_range_layout.addWidget(self.use_date_range_checkbox)
        date_range_layout.addWidget(start_date_label)
        date_range_layout.addWidget(self.start_date_edit)
        date_range_layout.addWidget(end_date_label)
        date_range_layout.addWidget(self.end_date_edit)
        date_range_layout.addStretch()
        self.main_layout.addLayout(date_range_layout)

        self.start_date_edit.setEnabled(False)
        self.end_date_edit.setEnabled(False)

    def setup_tabs(self):
        self.tab_widget = QTabWidget()
        self.tab_widget.currentChanged.connect(self.on_tab_changed)
        self.main_layout.addWidget(self.tab_widget)

        self.setup_dashboard_tab()
        self.setup_group_tab()
        self.setup_process_tab()
        self.setup_settings_tab()
        self.setup_theme_tab()
        self.setup_about_tab()

    def setup_dashboard_tab(self):
        dashboard_tab = QWidget()
        dashboard_layout = QVBoxLayout()

        search_sort_layout = QHBoxLayout()

        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("Search accounts...")
        self.search_input.textChanged.connect(self.filter_accounts)
        self.search_input.setClearButtonEnabled(True)
        search_sort_layout.addWidget(self.search_input, 3)

        group_label = QLabel("Group:")
        search_sort_layout.addWidget(group_label)

        self.group_filter_combo = QComboBox()
        self.group_filter_combo.addItem("All Groups", None)
        self.group_filter_combo.addItem("No Group", -1)
        self.group_filter_combo.currentIndexChanged.connect(self.on_group_filter_changed)
        self.group_filter_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        search_sort_layout.addWidget(self.group_filter_combo, 1)

        sort_label = QLabel("Sort by:")
        search_sort_layout.addWidget(sort_label)

        self.sort_combo = QComboBox()
        self.sort_combo.addItem("Latest Fetch", "fetch_timestamp_desc")
        self.sort_combo.addItem("Oldest Fetch", "fetch_timestamp_asc")
        self.sort_combo.addItem("Username (A-Z)", "username_asc")
        self.sort_combo.addItem("Username (Z-A)", "username_desc")
        self.sort_combo.addItem("Followers (High-Low)", "followers_desc")
        self.sort_combo.addItem("Followers (Low-High)", "followers_asc")
        self.sort_combo.addItem("Posts (High-Low)", "posts_desc")
        self.sort_combo.addItem("Posts (Low-High)", "posts_asc")
        self.sort_combo.addItem("Media Count (High-Low)", "media_count_desc")
        self.sort_combo.addItem("Media Count (Low-High)", "media_count_asc")
        self.sort_combo.currentIndexChanged.connect(self.on_sort_changed)
        self.sort_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        search_sort_layout.addWidget(self.sort_combo, 2)

        dashboard_layout.addLayout(search_sort_layout)

        self.account_list = QListWidget()
        self.account_list.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.account_list.itemSelectionChanged.connect(self.update_button_states)
        self.account_list.setIconSize(QSize(48, 48))
        self.account_list.setStyleSheet("""
            QListWidget {
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
                border: none;
                outline: none;
            }
            QListWidget::item:focus {
                border: none;
                outline: none;
            }
        """)
        
        dashboard_layout.addWidget(self.account_list)
        
        self.setup_account_buttons()
        dashboard_layout.addLayout(self.btn_layout)
        dashboard_tab.setLayout(dashboard_layout)
        self.tab_widget.addTab(dashboard_tab, "Dashboard")

        self.hide_account_buttons()
            
    def setup_account_buttons(self):
        self.btn_layout = QHBoxLayout()
        self.import_btn = QPushButton(' Import')
        self.export_btn = QPushButton(' Export')
        self.download_selected_btn = QPushButton(' Download')
        self.update_selected_btn = QPushButton(' Update')
        self.delete_btn = QPushButton(' Delete')
        
        icon_dir = ICON_DIR
        accent_color = self.current_theme_color
        
        self.import_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'database-import.svg'), accent_color))
        self.export_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'database-export.svg'), accent_color))
        self.download_selected_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'download.svg'), accent_color))
        self.update_selected_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'reload.svg'), accent_color))
        self.delete_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'trash.svg'), accent_color))
        
        for btn in [self.import_btn, self.export_btn, self.download_selected_btn, 
                    self.update_selected_btn, self.delete_btn]:
            btn.setMinimumWidth(100)
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
            btn.setIconSize(QSize(18, 18))
            
        self.import_btn.clicked.connect(self.import_accounts)
        self.export_btn.clicked.connect(self.export_accounts)
        self.download_selected_btn.clicked.connect(self.download_selected)
        self.update_selected_btn.clicked.connect(self.update_selected)
        self.delete_btn.clicked.connect(self.delete_accounts)
        
        self.btn_layout.addStretch()
        for btn in [self.import_btn, self.export_btn, self.download_selected_btn, 
                    self.update_selected_btn, self.delete_btn]:
            self.btn_layout.addWidget(btn, 1)
        self.btn_layout.addStretch()

    def update_button_icons(self):
        if not hasattr(self, 'import_btn'):
            return
        
        icon_dir = ICON_DIR
        accent_color = self.current_theme_color
        
        self.import_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'database-import.svg'), accent_color))
        self.export_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'database-export.svg'), accent_color))
        self.download_selected_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'download.svg'), accent_color))
        self.update_selected_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'reload.svg'), accent_color))
        self.delete_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'trash.svg'), accent_color))
        
        if hasattr(self, 'rename_group_btn'):
            self.rename_group_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'edit.svg'), accent_color))
        if hasattr(self, 'delete_group_btn'):
            self.delete_group_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'trash.svg'), accent_color))
        if hasattr(self, 'assign_to_group_btn'):
            self.assign_to_group_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'category-plus.svg'), accent_color))
        if hasattr(self, 'save_assignment_btn'):
            self.save_assignment_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'device-floppy.svg'), accent_color))
        
        if hasattr(self, 'stop_btn'):
            self.stop_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'cancel.svg'), accent_color))
        if hasattr(self, 'pause_resume_btn'):
            if self.pause_resume_btn.text() == ' Resume':
                self.pause_resume_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-play.svg'), accent_color))
            else:
                self.pause_resume_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), accent_color))
        if hasattr(self, 'stop_fetch_btn'):
            self.stop_fetch_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'cancel.svg'), accent_color))
        if hasattr(self, 'pause_fetch_btn'):
            if self.pause_fetch_btn.text() == ' Resume':
                self.pause_fetch_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-play.svg'), accent_color))
            else:
                self.pause_fetch_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), accent_color))

    def setup_group_tab(self):
        group_tab = QWidget()
        main_layout = QVBoxLayout()
        main_layout.setSpacing(5)
        main_layout.setContentsMargins(9, 9, 9, 9)

        content_layout = QHBoxLayout()
        
        left_panel = QWidget()
        left_layout = QVBoxLayout(left_panel)
        left_layout.setContentsMargins(0, 0, 0, 0)
        left_layout.setSpacing(5)
        
        group_label = QLabel("Groups:")
        left_layout.addWidget(group_label)
        
        create_group_layout = QHBoxLayout()
        self.group_name_input = QLineEdit()
        self.group_name_input.setPlaceholderText("Enter group name...")
        self.create_group_btn = QPushButton("Create")
        self.create_group_btn.setFixedWidth(80)
        self.create_group_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.create_group_btn.clicked.connect(self.create_group)
        create_group_layout.addWidget(self.group_name_input)
        create_group_layout.addWidget(self.create_group_btn)
        left_layout.addLayout(create_group_layout)
        
        self.group_list = QListWidget()
        self.group_list.itemSelectionChanged.connect(self.on_group_selected)
        left_layout.addWidget(self.group_list, 1)

        group_btn_layout = QHBoxLayout()
        group_btn_layout.setSpacing(5)
        
        self.rename_group_btn = QPushButton(" Rename")
        self.delete_group_btn = QPushButton(" Delete")
        
        icon_dir = ICON_DIR
        accent_color = self.current_theme_color
        
        self.rename_group_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'edit.svg'), accent_color))
        self.delete_group_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'trash.svg'), accent_color))
        
        for btn in [self.rename_group_btn, self.delete_group_btn]:
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
            btn.setIconSize(QSize(18, 18))
            btn.setEnabled(False)
        
        self.rename_group_btn.clicked.connect(self.rename_group)
        self.delete_group_btn.clicked.connect(self.delete_group)
        
        group_btn_layout.addWidget(self.rename_group_btn)
        group_btn_layout.addWidget(self.delete_group_btn)
        left_layout.addLayout(group_btn_layout)
        
        content_layout.addWidget(left_panel, 1)
        
        right_panel = QWidget()
        right_layout = QVBoxLayout(right_panel)
        right_layout.setContentsMargins(0, 0, 0, 0)
        right_layout.setSpacing(5)
        
        assign_label = QLabel("Assign Accounts:")
        right_layout.addWidget(assign_label)
        
        self.assign_search_input = QLineEdit()
        self.assign_search_input.setPlaceholderText("Search accounts...")
        self.assign_search_input.setClearButtonEnabled(True)
        self.assign_search_input.textChanged.connect(self.filter_assign_accounts)
        right_layout.addWidget(self.assign_search_input)
        
        self.assign_account_list = QListWidget()
        self.assign_account_list.setSelectionMode(QAbstractItemView.SelectionMode.MultiSelection)
        self.assign_account_list.itemChanged.connect(self.on_assign_item_changed)
        right_layout.addWidget(self.assign_account_list, 1)
        
        assign_btn_layout = QHBoxLayout()
        self.save_assignment_btn = QPushButton(" Save Assignment")
        self.save_assignment_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.save_assignment_btn.setEnabled(False)
        self.save_assignment_btn.clicked.connect(self.save_account_assignment)
        
        icon_dir = ICON_DIR
        self.save_assignment_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'device-floppy.svg'), accent_color))
        self.save_assignment_btn.setIconSize(QSize(18, 18))
        
        assign_btn_layout.addStretch()
        assign_btn_layout.addWidget(self.save_assignment_btn)
        assign_btn_layout.addStretch()
        right_layout.addLayout(assign_btn_layout)
        
        content_layout.addWidget(right_panel, 1)
        
        main_layout.addLayout(content_layout)

        group_tab.setLayout(main_layout)
        self.tab_widget.addTab(group_tab, "Group")
        
        self.load_groups()

    def setup_process_tab(self):
        self.process_tab = QWidget()
        process_layout = QVBoxLayout()
        process_layout.setSpacing(5)
        
        self.log_output = QTextEdit()
        self.log_output.setReadOnly(True)
        process_layout.addWidget(self.log_output)
        
        progress_time_layout = QVBoxLayout()
        progress_time_layout.setSpacing(2)
        
        self.progress_bar = QProgressBar()
        progress_time_layout.addWidget(self.progress_bar)
        
        self.time_label = QLabel("00:00:00")
        self.time_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        progress_time_layout.addWidget(self.time_label)
        
        process_layout.addLayout(progress_time_layout)
        
        download_control_layout = QHBoxLayout()
        self.stop_btn = QPushButton(' Stop')
        self.pause_resume_btn = QPushButton(' Pause')
        
        icon_dir = ICON_DIR
        accent_color = self.current_theme_color
        
        self.stop_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'cancel.svg'), accent_color))
        self.pause_resume_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), accent_color))
        
        self.stop_btn.setFixedWidth(120)
        self.pause_resume_btn.setFixedWidth(120)
        self.stop_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.pause_resume_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.stop_btn.setIconSize(QSize(18, 18))
        self.pause_resume_btn.setIconSize(QSize(18, 18))
        
        self.stop_btn.clicked.connect(self.stop_download)
        self.pause_resume_btn.clicked.connect(self.toggle_pause_resume)
        
        download_control_layout.addStretch()
        download_control_layout.addWidget(self.stop_btn)
        download_control_layout.addWidget(self.pause_resume_btn)
        download_control_layout.addStretch()
        process_layout.addLayout(download_control_layout)
        
        batch_control_layout = QHBoxLayout()
        self.pause_fetch_btn = QPushButton(' Pause')
        self.stop_fetch_btn = QPushButton(' Stop')
        
        self.pause_fetch_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), accent_color))
        self.stop_fetch_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'cancel.svg'), accent_color))
        
        for btn in [self.pause_fetch_btn, self.stop_fetch_btn]:
            btn.setFixedWidth(120)
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
            btn.setIconSize(QSize(18, 18))
        
        self.pause_fetch_btn.setToolTip("Pause batch fetching (can resume)")
        self.stop_fetch_btn.setToolTip("Stop batch fetching completely")
        
        self.pause_fetch_btn.clicked.connect(self.pause_batch_fetch)
        self.stop_fetch_btn.clicked.connect(self.stop_batch_fetch)
        
        batch_control_layout.addStretch()
        batch_control_layout.addWidget(self.pause_fetch_btn)
        batch_control_layout.addWidget(self.stop_fetch_btn)
        batch_control_layout.addStretch()
        process_layout.addLayout(batch_control_layout)
        
        self.process_tab.setLayout(process_layout)
        
        self.tab_widget.addTab(self.process_tab, "Process")
        
        self.progress_bar.hide()
        self.time_label.hide()
        self.stop_btn.hide()
        self.pause_resume_btn.hide()
        self.pause_fetch_btn.hide()
        self.stop_fetch_btn.hide()

    def setup_settings_tab(self):
        settings_tab = QWidget()
        settings_layout = QVBoxLayout()
        settings_layout.setSpacing(0)
        settings_layout.setContentsMargins(9, 9, 9, 9)

        output_group = QWidget()
        output_layout = QVBoxLayout(output_group)
        output_layout.setSpacing(5)
        
        output_label = QLabel('Output Directory')
        output_label.setStyleSheet("font-weight: bold;")
        output_layout.addWidget(output_label)
        
        output_dir_layout = QHBoxLayout()
        self.output_dir = QLineEdit()
        self.output_dir.setText(self.last_output_path)
        self.output_dir.textChanged.connect(self.save_settings)
        self.output_browse = QPushButton('Browse')
        self.output_browse.setFixedWidth(80)
        self.output_browse.setCursor(Qt.CursorShape.PointingHandCursor)
        self.output_browse.clicked.connect(self.browse_output)
        
        output_dir_layout.addWidget(self.output_dir)
        output_dir_layout.addWidget(self.output_browse)
        output_layout.addLayout(output_dir_layout)
        
        settings_layout.addWidget(output_group)

        auth_group = QWidget()
        auth_layout = QVBoxLayout(auth_group)
        auth_layout.setSpacing(5)
        
        auth_label = QLabel('Authentication')
        auth_label.setStyleSheet("font-weight: bold;")
        auth_layout.addWidget(auth_label)
        
        auth_token_layout = QHBoxLayout()
        auth_token_label = QLabel('Auth Token:')
        
        self.auth_token_input = QLineEdit()
        self.auth_token_input.setPlaceholderText("Enter Auth Token")
        self.auth_token_input.setText(self.last_auth_token)
        self.auth_token_input.textChanged.connect(self.save_settings)
        self.auth_token_input.setClearButtonEnabled(True)
        
        auth_token_layout.addWidget(auth_token_label)
        auth_token_layout.addWidget(self.auth_token_input)
        auth_layout.addLayout(auth_token_layout)
        
        settings_layout.addWidget(auth_group)

        gallery_dl_group = QWidget()
        gallery_dl_layout = QVBoxLayout(gallery_dl_group)
        gallery_dl_layout.setSpacing(5)
        
        gallery_dl_label = QLabel('gallery-dl Settings')
        gallery_dl_label.setStyleSheet("font-weight: bold;")
        gallery_dl_layout.addWidget(gallery_dl_label)
        
        first_row_layout = QHBoxLayout()
        first_row_layout.setSpacing(5)

        timeline_label = QLabel("Timeline:")
        self.timeline_type_combo = QComboBox()
        self.timeline_type_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.timeline_type_combo.setFixedWidth(75)
        timeline_types = [
            ('media', 'Media'), 
            ('timeline', 'Post'), 
            ('tweets', 'Tweets'), 
            ('with_replies', 'Replies')
        ]
        for value, display in timeline_types:
            self.timeline_type_combo.addItem(display, value)
        self.timeline_type_combo.currentTextChanged.connect(self.save_settings)
        first_row_layout.addWidget(timeline_label)
        first_row_layout.addWidget(self.timeline_type_combo)
        
        first_row_layout.addSpacing(5)

        media_label = QLabel("Media:")
        self.media_type_combo = QComboBox()
        self.media_type_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.media_type_combo.setFixedWidth(65)
        media_types = [('all', 'All'), ('image', 'Image'), ('video', 'Video'), ('gif', 'GIF')]
        for value, display in media_types:
            self.media_type_combo.addItem(display, value)
        self.media_type_combo.currentTextChanged.connect(self.save_settings)
        first_row_layout.addWidget(media_label)
        first_row_layout.addWidget(self.media_type_combo)
        
        first_row_layout.addSpacing(5)

        self.retweets_checkbox = QCheckBox("Include Retweets")
        self.retweets_checkbox.setCursor(Qt.CursorShape.PointingHandCursor)
        self.retweets_checkbox.setToolTip("Include retweets (not available for Media timeline)")
        self.retweets_checkbox.stateChanged.connect(self.handle_retweets_checkbox)
        first_row_layout.addWidget(self.retweets_checkbox)
        
        first_row_layout.addSpacing(5)

        self.batch_checkbox = QCheckBox("Fetch Batch")
        self.batch_checkbox.setCursor(Qt.CursorShape.PointingHandCursor)
        self.batch_checkbox.setToolTip("Enable for accounts with thousands of media")
        self.batch_checkbox.stateChanged.connect(self.handle_batch_checkbox)
        self.batch_checkbox.setChecked(self.batch_mode)
        first_row_layout.addWidget(self.batch_checkbox)
        
        self.size_label = QLabel("Size:")
        first_row_layout.addWidget(self.size_label)
        self.batch_size_combo = QComboBox()
        self.batch_size_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.batch_size_combo.setFixedWidth(60)
        for size in [50, 100, 150, 200]:
            self.batch_size_combo.addItem(str(size))
        self.batch_size_combo.setCurrentIndex(1)
        self.batch_size_combo.currentTextChanged.connect(self.save_batch_size)
        first_row_layout.addWidget(self.batch_size_combo)
        
        self.size_label.hide()
        self.batch_size_combo.hide()
        
        first_row_layout.addStretch()
        gallery_dl_layout.addLayout(first_row_layout)
        
        settings_layout.addWidget(gallery_dl_group)

        download_conversion_group = QWidget()
        download_conversion_layout = QVBoxLayout(download_conversion_group)
        download_conversion_layout.setSpacing(5)
        
        labels_layout = QHBoxLayout()
        
        download_label_container = QWidget()
        download_label_layout = QHBoxLayout(download_label_container)
        download_label_layout.setContentsMargins(0, 0, 0, 0)
        download_label = QLabel('Download Settings')
        download_label.setStyleSheet("font-weight: bold;")
        download_label_layout.addWidget(download_label)
        download_label_layout.addStretch()
        download_label_container.setFixedWidth(180)
        labels_layout.addWidget(download_label_container)
        
        conversion_label = QLabel('Conversion Settings')
        conversion_label.setStyleSheet("font-weight: bold;")
        labels_layout.addWidget(conversion_label)
        labels_layout.addStretch()
        download_conversion_layout.addLayout(labels_layout)
        
        controls_layout = QHBoxLayout()
        
        download_controls_container = QWidget()
        download_controls_layout = QHBoxLayout(download_controls_container)
        download_controls_layout.setContentsMargins(0, 0, 0, 0)
        
        batch_label = QLabel('Concurrents:')
        download_controls_layout.addWidget(batch_label)
        
        self.download_batch_combo = QComboBox()
        self.download_batch_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.download_batch_combo.setFixedWidth(80)
        self.download_batch_combo.addItem("1")
        for size in range(5, 101, 5):
            self.download_batch_combo.addItem(str(size))
        self.download_batch_combo.setCurrentText(str(self.download_batch_size))
        self.download_batch_combo.currentTextChanged.connect(self.save_settings)
        download_controls_layout.addWidget(self.download_batch_combo)
        download_controls_layout.addStretch()
        
        download_controls_container.setFixedWidth(180)
        controls_layout.addWidget(download_controls_container)
        
        self.convert_gif_checkbox = QCheckBox("Convert GIF")
        self.convert_gif_checkbox.setCursor(Qt.CursorShape.PointingHandCursor)
        self.convert_gif_checkbox.setChecked(self.convert_gif)
        self.convert_gif_checkbox.toggled.connect(self.handle_conversion_checkbox)
        controls_layout.addWidget(self.convert_gif_checkbox)
        
        controls_layout.addSpacing(5)
        
        mode_label = QLabel("Mode:")
        self.conversion_mode_combo = QComboBox()
        self.conversion_mode_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.conversion_mode_combo.setFixedWidth(80)
        conversion_modes = [('better', 'Better'), ('fast', 'Fast')]
        for value, display in conversion_modes:
            self.conversion_mode_combo.addItem(display, value)
        self.conversion_mode_combo.currentTextChanged.connect(self.save_settings)
        controls_layout.addWidget(mode_label)
        controls_layout.addWidget(self.conversion_mode_combo)
        
        controls_layout.addSpacing(5)
        
        quality_label = QLabel("Quality:")
        self.conversion_quality_combo = QComboBox()
        self.conversion_quality_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.conversion_quality_combo.setFixedWidth(85)
        gif_qualities = [
            ('original', 'Original'),
            ('high', 'High'),
            ('medium', 'Medium'),
            ('low', 'Low')
        ]
        for value, display in gif_qualities:
            self.conversion_quality_combo.addItem(display, value)
        self.conversion_quality_combo.currentTextChanged.connect(self.save_settings)
        controls_layout.addWidget(quality_label)
        controls_layout.addWidget(self.conversion_quality_combo)
        
        self.mode_label = mode_label
        self.quality_label = quality_label
        
        if not self.convert_gif:
            mode_label.hide()
            self.conversion_mode_combo.hide()
            quality_label.hide()
            self.conversion_quality_combo.hide()
        
        controls_layout.addStretch()
        download_conversion_layout.addLayout(controls_layout)
        
        settings_layout.addWidget(download_conversion_group)
        settings_layout.addStretch()
        
        settings_tab.setLayout(settings_layout)
        self.tab_widget.addTab(settings_tab, "Settings")
        
    def setup_theme_tab(self):
        theme_tab = QWidget()
        theme_layout = QVBoxLayout()
        theme_layout.setSpacing(8)
        theme_layout.setContentsMargins(8, 15, 15, 15)

        grid_layout = QVBoxLayout()
        
        self.color_buttons = {}
        
        first_row_palettes = [
            ("Red", [
                ("#FFCDD2", "100"), ("#EF9A9A", "200"), ("#E57373", "300"), ("#EF5350", "400"), ("#F44336", "500"), ("#E53935", "600"), ("#D32F2F", "700"), ("#C62828", "800"), ("#B71C1C", "900"), ("#FF8A80", "A100"), ("#FF5252", "A200"), ("#FF1744", "A400"), ("#D50000", "A700")
            ]),
            ("Pink", [
                ("#F8BBD0", "100"), ("#F48FB1", "200"), ("#F06292", "300"), ("#EC407A", "400"), ("#E91E63", "500"), ("#D81B60", "600"), ("#C2185B", "700"), ("#AD1457", "800"), ("#880E4F", "900"), ("#FF80AB", "A100"), ("#FF4081", "A200"), ("#F50057", "A400"), ("#C51162", "A700")
            ]),
            ("Purple", [
                ("#E1BEE7", "100"), ("#CE93D8", "200"), ("#BA68C8", "300"), ("#AB47BC", "400"), ("#9C27B0", "500"), ("#8E24AA", "600"), ("#7B1FA2", "700"), ("#6A1B9A", "800"), ("#4A148C", "900"), ("#EA80FC", "A100"), ("#E040FB", "A200"), ("#D500F9", "A400"), ("#AA00FF", "A700")
            ])
        ]
        
        second_row_palettes = [
            ("Deep Purple", [
                ("#D1C4E9", "100"), ("#B39DDB", "200"), ("#9575CD", "300"), ("#7E57C2", "400"), ("#673AB7", "500"), ("#5E35B1", "600"), ("#512DA8", "700"), ("#4527A0", "800"), ("#311B92", "900"), ("#B388FF", "A100"), ("#7C4DFF", "A200"), ("#651FFF", "A400"), ("#6200EA", "A700")
            ]),
            ("Indigo", [
                ("#C5CAE9", "100"), ("#9FA8DA", "200"), ("#7986CB", "300"), ("#5C6BC0", "400"), ("#3F51B5", "500"), ("#3949AB", "600"), ("#303F9F", "700"), ("#283593", "800"), ("#1A237E", "900"), ("#8C9EFF", "A100"), ("#536DFE", "A200"), ("#3D5AFE", "A400"), ("#304FFE", "A700")
            ]),
            ("Blue", [
                ("#BBDEFB", "100"), ("#90CAF9", "200"), ("#64B5F6", "300"), ("#42A5F5", "400"), ("#2196F3", "500"), ("#1E88E5", "600"), ("#1976D2", "700"), ("#1565C0", "800"), ("#0D47A1", "900"), ("#82B1FF", "A100"), ("#448AFF", "A200"), ("#2979FF", "A400"), ("#2962FF", "A700")
            ])
        ]
        
        third_row_palettes = [
            ("Light Blue", [
                ("#B3E5FC", "100"), ("#81D4FA", "200"), ("#4FC3F7", "300"), ("#29B6F6", "400"), ("#03A9F4", "500"), ("#039BE5", "600"), ("#0288D1", "700"), ("#0277BD", "800"), ("#01579B", "900"), ("#80D8FF", "A100"), ("#40C4FF", "A200"), ("#00B0FF", "A400"), ("#0091EA", "A700")
            ]),
            ("Cyan", [
                ("#B2EBF2", "100"), ("#80DEEA", "200"), ("#4DD0E1", "300"), ("#26C6DA", "400"), ("#00BCD4", "500"), ("#00ACC1", "600"), ("#0097A7", "700"), ("#00838F", "800"), ("#006064", "900"), ("#84FFFF", "A100"), ("#18FFFF", "A200"), ("#00E5FF", "A400"), ("#00B8D4", "A700")
            ]),
            ("Teal", [
                ("#B2DFDB", "100"), ("#80CBC4", "200"), ("#4DB6AC", "300"), ("#26A69A", "400"), ("#009688", "500"), ("#00897B", "600"), ("#00796B", "700"), ("#00695C", "800"), ("#004D40", "900"), ("#A7FFEB", "A100"), ("#64FFDA", "A200"), ("#1DE9B6", "A400"), ("#00BFA5", "A700")
            ])
        ]
        
        fourth_row_palettes = [
            ("Green", [
                ("#C8E6C9", "100"), ("#A5D6A7", "200"), ("#81C784", "300"), ("#66BB6A", "400"), ("#4CAF50", "500"), ("#43A047", "600"), ("#388E3C", "700"), ("#2E7D32", "800"), ("#1B5E20", "900"), ("#B9F6CA", "A100"), ("#69F0AE", "A200"), ("#00E676", "A400"), ("#00C853", "A700")
            ]),
            ("Light Green", [
                ("#DCEDC8", "100"), ("#C5E1A5", "200"), ("#AED581", "300"), ("#9CCC65", "400"), ("#8BC34A", "500"), ("#7CB342", "600"), ("#689F38", "700"), ("#558B2F", "800"), ("#33691E", "900"), ("#CCFF90", "A100"), ("#B2FF59", "A200"), ("#76FF03", "A400"), ("#64DD17", "A700")
            ]),
            ("Lime", [
                ("#F0F4C3", "100"), ("#E6EE9C", "200"), ("#DCE775", "300"), ("#D4E157", "400"), ("#CDDC39", "500"), ("#C0CA33", "600"), ("#AFB42B", "700"), ("#9E9D24", "800"), ("#827717", "900"), ("#F4FF81", "A100"), ("#EEFF41", "A200"), ("#C6FF00", "A400"), ("#AEEA00", "A700")
            ])
        ]
        
        fifth_row_palettes = [
            ("Yellow", [
                ("#FFF9C4", "100"), ("#FFF59D", "200"), ("#FFF176", "300"), ("#FFEE58", "400"), ("#FFEB3B", "500"), ("#FDD835", "600"), ("#FBC02D", "700"), ("#F9A825", "800"), ("#F57F17", "900"), ("#FFFF8D", "A100"), ("#FFFF00", "A200"), ("#FFEA00", "A400"), ("#FFD600", "A700")
            ]),
            ("Amber", [
                ("#FFECB3", "100"), ("#FFE082", "200"), ("#FFD54F", "300"), ("#FFCA28", "400"), ("#FFC107", "500"), ("#FFB300", "600"), ("#FFA000", "700"), ("#FF8F00", "800"), ("#FF6F00", "900"), ("#FFE57F", "A100"), ("#FFD740", "A200"), ("#FFC400", "A400"), ("#FFAB00", "A700")
            ]),
            ("Orange", [
                ("#FFE0B2", "100"), ("#FFCC80", "200"), ("#FFB74D", "300"), ("#FFA726", "400"), ("#FF9800", "500"), ("#FB8C00", "600"), ("#F57C00", "700"), ("#EF6C00", "800"), ("#E65100", "900"), ("#FFD180", "A100"), ("#FFAB40", "A200"), ("#FF9100", "A400"), ("#FF6D00", "A700")
            ])
        ]
        
        for row_palettes in [first_row_palettes, second_row_palettes, third_row_palettes, fourth_row_palettes, fifth_row_palettes]:
            row_layout = QHBoxLayout()
            row_layout.setSpacing(15)
            
            for palette_name, colors in row_palettes:
                column_layout = QVBoxLayout()
                column_layout.setSpacing(3)
                
                palette_label = QLabel(palette_name)
                palette_label.setStyleSheet("margin-bottom: 2px;")
                palette_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
                column_layout.addWidget(palette_label)
                
                color_buttons_layout = QHBoxLayout()
                color_buttons_layout.setSpacing(3)
                
                for color_hex, color_name in colors:
                    color_btn = QPushButton()
                    color_btn.setFixedSize(18, 18)
                    
                    is_current = color_hex == self.current_theme_color
                    border_style = "2px solid #fff" if is_current else "none"
                    
                    color_btn.setStyleSheet(f"""
                        QPushButton {{
                            background-color: {color_hex};
                            border: {border_style};
                            border-radius: 9px;
                        }}
                        QPushButton:hover {{
                            border: 2px solid #fff;
                        }}
                        QPushButton:pressed {{
                            border: 2px solid #fff;
                        }}
                    """)
                    color_btn.setCursor(Qt.CursorShape.PointingHandCursor)
                    color_btn.setToolTip(f"{palette_name} {color_name}\n{color_hex}")
                    color_btn.clicked.connect(lambda checked, color=color_hex, btn=color_btn: self.change_theme_color(color, btn))
                    
                    self.color_buttons[color_hex] = color_btn
                    
                    color_buttons_layout.addWidget(color_btn)
                
                column_layout.addLayout(color_buttons_layout)
                row_layout.addLayout(column_layout)
            
            grid_layout.addLayout(row_layout)

        theme_layout.addLayout(grid_layout)
        theme_layout.addStretch()

        theme_tab.setLayout(theme_layout)
        self.tab_widget.addTab(theme_tab, "Theme")

    def change_theme_color(self, color, clicked_btn=None):
        if hasattr(self, 'color_buttons'):
            for color_hex, btn in self.color_buttons.items():
                if color_hex == self.current_theme_color:
                    btn.setStyleSheet(f"""
                        QPushButton {{
                            background-color: {color_hex};
                            border: none;
                            border-radius: 9px;
                        }}
                        QPushButton:hover {{
                            border: 2px solid #fff;
                        }}
                        QPushButton:pressed {{
                            border: 2px solid #fff;
                        }}
                    """)
                    break
        
        self.current_theme_color = color
        self.settings.setValue('theme_color', color)
        self.settings.sync()
        
        if clicked_btn:
            clicked_btn.setStyleSheet(f"""
                QPushButton {{
                    background-color: {color};
                    border: 2px solid #fff;
                    border-radius: 9px;
                }}
                QPushButton:hover {{
                    border: 2px solid #fff;
                }}
                QPushButton:pressed {{
                    border: 2px solid #fff;
                }}
            """)
        
        qdarktheme.setup_theme(
            custom_colors={
                "[dark]": {
                    "primary": color,
                }
            }
        )
        
        self.update_button_icons()
        
    def setup_about_tab(self):
        about_tab = QWidget()
        about_layout = QVBoxLayout()
        about_layout.setAlignment(Qt.AlignmentFlag.AlignCenter)
        about_layout.setSpacing(15)

        sections = [
            ("Check for Updates", "Check", "https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases"),
            ("Report an Issue", "Report", "https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/issues"),
            ("gallery-dl Repository", "Visit", "https://github.com/mikf/gallery-dl")
        ]

        for title, button_text, url in sections:
            section_widget = QWidget()
            section_layout = QVBoxLayout(section_widget)
            section_layout.setSpacing(10)
            section_layout.setContentsMargins(0, 0, 0, 0)

            label = QLabel(title)
            label.setStyleSheet("font-weight: bold;")
            label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            section_layout.addWidget(label)

            button = QPushButton(button_text)
            button.setFixedSize(100, 25)
            button.setCursor(Qt.CursorShape.PointingHandCursor)
            button.clicked.connect(lambda _, url=url: QDesktopServices.openUrl(QUrl(url if url.startswith(('http://', 'https://')) else f'https://{url}')))
            section_layout.addWidget(button, alignment=Qt.AlignmentFlag.AlignCenter)

            about_layout.addWidget(section_widget)

        footer_label = QLabel(f"v{self.current_version} | gallery-dl v1.30.10 | November 2025")
        about_layout.addWidget(footer_label, alignment=Qt.AlignmentFlag.AlignCenter)

        about_tab.setLayout(about_layout)
        self.tab_widget.addTab(about_tab, "About")

    def save_url(self):
        self.settings.setValue('twitter_url', self.twitter_url.text().strip())
        self.settings.sync()

    def toggle_date_range(self, checked):
        self.start_date_edit.setEnabled(checked)
        self.end_date_edit.setEnabled(checked)

    def save_batch_size(self):
        self.batch_size = int(self.batch_size_combo.currentText())
        self.settings.setValue('batch_size', self.batch_size)
        self.settings.sync()
        
    def save_settings(self):
        self.settings.setValue('output_path', self.output_dir.text().strip())
        self.settings.setValue('auth_token', self.auth_token_input.text().strip())
        self.settings.setValue('media_type', self.media_type_combo.currentData())
        self.settings.setValue('timeline_type', self.timeline_type_combo.currentData())
        self.settings.setValue('download_batch_size', int(self.download_batch_combo.currentText()))

        if hasattr(self, 'convert_gif_checkbox'):
            self.settings.setValue('convert_gif', self.convert_gif_checkbox.isChecked())
            self.convert_gif = self.convert_gif_checkbox.isChecked()

        if hasattr(self, 'conversion_mode_combo'):
            self.settings.setValue('gif_conversion_mode', self.conversion_mode_combo.currentData())
            self.gif_conversion_mode = self.conversion_mode_combo.currentData()

        if hasattr(self, 'conversion_quality_combo'):
            self.settings.setValue('gif_resolution', self.conversion_quality_combo.currentData())
            self.gif_resolution = self.conversion_quality_combo.currentData()

        self.settings.sync()

    def load_settings(self):
        try:
            if not hasattr(self, 'batch_checkbox') or not hasattr(self, 'size_label') or not hasattr(self, 'batch_size_combo'):
                return
            
            include_retweets = self.settings.value('include_retweets', False, type=bool)
            if hasattr(self, 'retweets_checkbox'):
                self.retweets_checkbox.blockSignals(True)
                self.retweets_checkbox.setChecked(include_retweets)
                self.retweets_checkbox.blockSignals(False)
                
                if include_retweets:
                    for i in range(self.timeline_type_combo.count()):
                        if self.timeline_type_combo.itemData(i) == 'media':
                            model = self.timeline_type_combo.model()
                            item = model.item(i)
                            item.setEnabled(False)
                            break
                
            batch_mode = self.settings.value('batch_mode', False, type=bool)
            
            self.batch_checkbox.blockSignals(True)
            self.batch_checkbox.setChecked(batch_mode)
            self.batch_checkbox.blockSignals(False)
            
            if batch_mode:
                self.size_label.show()
                self.batch_size_combo.show()
                self.fetch_btn.setText('Fetch Batch')
            else:
                self.size_label.hide()
                self.batch_size_combo.hide()
                self.fetch_btn.setText('Fetch')
            
            fetch_batch_size = str(self.settings.value('batch_size', 100))
            index = self.batch_size_combo.findText(fetch_batch_size)
            if index >= 0:
                self.batch_size_combo.setCurrentIndex(index)
            else:
                self.batch_size_combo.setCurrentIndex(1)
                
            download_batch_size = str(self.settings.value('download_batch_size', 25))
            index = self.download_batch_combo.findText(download_batch_size)
            if index >= 0:
                self.download_batch_combo.setCurrentIndex(index)
            else:
                self.download_batch_combo.setCurrentIndex(0)
            media_type = self.settings.value('media_type', 'all')
            for i in range(self.media_type_combo.count()):
                if self.media_type_combo.itemData(i) == media_type:
                    self.media_type_combo.setCurrentIndex(i)
                    break
            
            timeline_type = self.settings.value('timeline_type', 'timeline')
            for i in range(self.timeline_type_combo.count()):
                if self.timeline_type_combo.itemData(i) == timeline_type:
                    self.timeline_type_combo.setCurrentIndex(i)
                    break
            
            convert_gif = self.settings.value('convert_gif', False, type=bool)
            if hasattr(self, 'convert_gif_checkbox'):
                self.convert_gif_checkbox.blockSignals(True)
                self.convert_gif_checkbox.setChecked(convert_gif)
                self.convert_gif_checkbox.blockSignals(False)
                
                if convert_gif:
                    if hasattr(self, 'mode_label'):
                        self.mode_label.show()
                    if hasattr(self, 'conversion_mode_combo'):
                        self.conversion_mode_combo.show()
                    if hasattr(self, 'quality_label'):
                        self.quality_label.show()
                    if hasattr(self, 'conversion_quality_combo'):
                        self.conversion_quality_combo.show()
                else:
                    if hasattr(self, 'mode_label'):
                        self.mode_label.hide()
                    if hasattr(self, 'conversion_mode_combo'):
                        self.conversion_mode_combo.hide()
                    if hasattr(self, 'quality_label'):
                        self.quality_label.hide()
                    if hasattr(self, 'conversion_quality_combo'):
                        self.conversion_quality_combo.hide()
            
            gif_conversion_mode = self.settings.value('gif_conversion_mode', 'better')
            if hasattr(self, 'conversion_mode_combo'):
                for i in range(self.conversion_mode_combo.count()):
                    if self.conversion_mode_combo.itemData(i) == gif_conversion_mode:
                        self.conversion_mode_combo.setCurrentIndex(i)
                        break
            
            gif_resolution = self.settings.value('gif_resolution', 'original')
            if hasattr(self, 'conversion_quality_combo'):
                for i in range(self.conversion_quality_combo.count()):
                    if self.conversion_quality_combo.itemData(i) == gif_resolution:
                        self.conversion_quality_combo.setCurrentIndex(i)
                        break
            
            last_username_url = self.settings.value('twitter_url', '')
            if last_username_url and hasattr(self, 'twitter_url'):
                self.twitter_url.setText(last_username_url)

            if hasattr(self, 'sort_combo'):
                sort_pref = self.settings.value('sort_preference', 'fetch_timestamp_desc')
                for i in range(self.sort_combo.count()):
                    if self.sort_combo.itemData(i) == sort_pref:
                        self.sort_combo.blockSignals(True)
                        self.sort_combo.setCurrentIndex(i)
                        self.sort_combo.blockSignals(False)
                        break
            
        except Exception as e:
            pass
    
    def restore_filter_preferences(self):
        try:
            if hasattr(self, 'group_filter_combo') and self.group_filter_combo.count() > 0:
                saved_filter = self.settings.value('group_filter')
                
                if saved_filter is None or saved_filter == '' or saved_filter == 'None':
                    group_filter = None
                elif saved_filter == '-1' or saved_filter == -1:
                    group_filter = -1
                else:
                    try:
                        group_filter = int(saved_filter)
                    except (ValueError, TypeError):
                        group_filter = None
                
                for i in range(self.group_filter_combo.count()):
                    item_data = self.group_filter_combo.itemData(i)
                    if item_data == group_filter:
                        self.group_filter_combo.blockSignals(True)
                        self.group_filter_combo.setCurrentIndex(i)
                        self.group_filter_combo.blockSignals(False)
                        if group_filter is not None:
                            self.filter_accounts()
                        break
        except Exception as e:
            pass

    def handle_retweets_checkbox(self, state):
        self.include_retweets = self.retweets_checkbox.isChecked()
        self.settings.setValue('include_retweets', self.include_retweets)
        
        if self.include_retweets:
            current_timeline = self.timeline_type_combo.currentData()
            if current_timeline == 'media':
                for i in range(self.timeline_type_combo.count()):
                    if self.timeline_type_combo.itemData(i) == 'timeline':
                        self.timeline_type_combo.setCurrentIndex(i)
                        self.settings.setValue('timeline_type', 'timeline')
                        break
            for i in range(self.timeline_type_combo.count()):
                if self.timeline_type_combo.itemData(i) == 'media':
                    model = self.timeline_type_combo.model()
                    item = model.item(i)
                    item.setEnabled(False)
                    break
        else:
            for i in range(self.timeline_type_combo.count()):
                if self.timeline_type_combo.itemData(i) == 'media':
                    model = self.timeline_type_combo.model()
                    item = model.item(i)
                    item.setEnabled(True)
                    break
        
        self.settings.sync()

    def handle_batch_checkbox(self, state):
        if not hasattr(self, 'size_label') or not hasattr(self, 'batch_size_combo'):
            return
            
        self.batch_mode = self.batch_checkbox.isChecked()
        self.settings.setValue('batch_mode', self.batch_mode)
        
        if self.batch_checkbox.isChecked():
            self.size_label.show()
            self.batch_size_combo.show()
            self.fetch_btn.setText('Fetch Batch')
        else:
            self.size_label.hide()
            self.batch_size_combo.hide()
            self.fetch_btn.setText('Fetch')
        self.update_button_states()

    def handle_conversion_checkbox(self, checked):
        self.convert_gif = checked
        self.save_settings()
        
        if checked:
            if hasattr(self, 'mode_label'):
                self.mode_label.show()
            if hasattr(self, 'conversion_mode_combo'):
                self.conversion_mode_combo.show()
            if hasattr(self, 'quality_label'):
                self.quality_label.show()
            if hasattr(self, 'conversion_quality_combo'):
                self.conversion_quality_combo.show()
        else:
            if hasattr(self, 'mode_label'):
                self.mode_label.hide()
            if hasattr(self, 'conversion_mode_combo'):
                self.conversion_mode_combo.hide()
            if hasattr(self, 'quality_label'):
                self.quality_label.hide()
            if hasattr(self, 'conversion_quality_combo'):
                self.conversion_quality_combo.hide()

    def browse_output(self):
        directory = QFileDialog.getExistingDirectory(self, "Select Output Directory")
        if directory:
            self.output_dir.setText(directory)
            self.save_settings()

    def migrate_json_to_sqlite(self):
        try:
            accounts_migrated, media_migrated = self.db_manager.migrate_from_json(self.temp_dir)
            if accounts_migrated > 0:
                print(f"Migrated {accounts_migrated} accounts and {media_migrated} media items from JSON to SQLite")
        except Exception as e:
            print(f"Migration error: {e}")

    def get_cache_file_path(self, username, media_type, is_batch=None):
        timeline_type = self.timeline_type_combo.currentData() or 'timeline'
        filename = f"{username}_{timeline_type}_{media_type}.json"
        return os.path.join(self.temp_dir, filename)

    def load_cached_data(self, username, media_type, is_batch=None):
        try:
            account = self.db_manager.get_account(username, media_type)
            if account:
                media_list = self.db_manager.get_media_list(username)
                return {
                    'username': account['username'],
                    'nick': account['nick'],
                    'followers': account['followers'],
                    'following': account['following'],
                    'posts': account['posts'],
                    'profile_image': account['profile_image'],
                    'fetch_timestamp': account['fetch_timestamp'],
                    'media_type': account['media_type'],
                    'fetch_mode': account['fetch_mode'],
                    'media_list': media_list
                }
        except Exception as e:
            print(f"Error loading cached data: {e}")
        return None

    def save_cached_data(self, username, media_type, data, is_batch=None):
        try:
            account_info = data.get('account_info', {})
            
            nick = account_info.get('nick', account_info.get('name', data.get('nick', '')))
            followers = account_info.get('followers_count', data.get('followers', 0))
            following = account_info.get('friends_count', data.get('following', 0))
            posts = account_info.get('statuses_count', data.get('posts', 0))
            profile_image = account_info.get('profile_image', data.get('profile_image'))
            
            existing_account = None
            for acc in self.accounts:
                if acc.username == username and acc.media_type == media_type:
                    existing_account = acc
                    break
            
            group_id = existing_account.group_id if existing_account else data.get('group_id')
            
            self.db_manager.save_account(
                username=username,
                nick=nick,
                followers=followers,
                following=following,
                posts=posts,
                media_type=media_type,
                profile_image=profile_image,
                fetch_mode=data.get('fetch_mode', 'batch' if is_batch else 'all'),
                fetch_timestamp=data.get('fetch_timestamp'),
                group_id=group_id
            )
            
            media_list = data.get('timeline', data.get('media_list', []))
            if media_list:
                self.db_manager.save_media_list(username, media_list)
        except Exception as e:
            print(f"Error saving cached data: {e}")
            import traceback
            traceback.print_exc()

    def load_all_cached_accounts(self):
        try:
            db_accounts = self.db_manager.get_all_accounts()

            for db_account in db_accounts:
                username = db_account['username']
                media_type = db_account['media_type']

                already_exists = any(
                    acc.username == username and acc.media_type == media_type
                    for acc in self.accounts
                )

                if not already_exists:
                    media_list = self.db_manager.get_media_list(username)

                    if media_list:
                        account = Account(
                            username=username,
                            nick=db_account['nick'],
                            followers=db_account['followers'],
                            following=db_account['following'],
                            posts=db_account['posts'],
                            media_type=media_type,
                            profile_image=db_account['profile_image'],
                            media_list=media_list,
                            fetch_mode=db_account.get('fetch_mode', 'all'),
                            fetch_timestamp=db_account.get('fetch_timestamp'),
                            group_id=db_account.get('group_id')
                        )

                        self.accounts.append(account)

            if self.accounts:
                if hasattr(self, 'sort_combo'):
                    self.sort_accounts()
                self.update_account_list()
                if hasattr(self, 'group_filter_combo') and self.group_filter_combo.currentData() is not None:
                    self.filter_accounts()

        except Exception as e:
            pass

    def fetch_account(self):
        url_input = self.twitter_url.text().strip()
        
        if not url_input:
            self.log_output.append('Warning: Please enter a Twitter username/URL.')
            return

        if not self.auth_token_input.text().strip():
            self.log_output.append('Warning: Please enter your auth token.')
            return

        urls = [url.strip() for url in url_input.split(',') if url.strip()]
        
        if len(urls) > 1:

            self.log_output.append(f'Processing {len(urls)} accounts: {", ".join(urls)}')
            self.tab_widget.setCurrentWidget(self.process_tab)
            
            self.accounts_to_fetch = []
            media_type = self.media_type_combo.currentData()
            
            for url in urls:
                username = self.normalize_url_to_username(url)
                if username:
                    self.accounts_to_fetch.append((username, media_type))
            
            if self.accounts_to_fetch:
                self.current_fetch_index = 0
                self.is_multiple_user_mode = True
                self.is_initial_fetch = True
                
                if self.batch_mode:
                    self.is_auto_fetching = True
                    self.pause_fetch_btn.hide()
                    self.stop_fetch_btn.show()
                    self.log_output.append('Multiple user batch mode: Auto-fetching all batches for all users...')
                else:
                    self.is_auto_fetching = True
                    self.pause_fetch_btn.hide()
                    self.stop_fetch_btn.show()
                    self.log_output.append('Multiple user mode: Auto-fetching all users...')
                
                self.fetch_next_account_in_batch()
            return
        
        url = urls[0]
        username = self.normalize_url_to_username(url)
        if not username:
            self.log_output.append('Warning: Invalid username/URL format.')
            return

        media_type = self.media_type_combo.currentData()
        
        existing_account = None
        for account in self.accounts:
            if account.username == username and account.media_type == media_type:
                existing_account = account
                break
        
        if existing_account:
            self.accounts.remove(existing_account)
            
            for is_batch in [True, False]:
                cache_file = self.get_cache_file_path(username, media_type, is_batch=is_batch)
                try:
                    if os.path.exists(cache_file):
                        os.remove(cache_file)
                        self.log_output.append(f'Removed existing cache: {os.path.basename(cache_file)}')
                except Exception as e:
                    self.log_output.append(f'Warning: Could not remove cache {os.path.basename(cache_file)}: {str(e)}')
            
            self.log_output.append(f'Preparing new batch for {username} ({media_type}) - cache cleared')
            self.update_account_list()

        if not existing_account:
            cached_data = self.load_cached_data(username, media_type, is_batch=self.batch_mode)
            if cached_data:
                try:
                    account_info = cached_data.get('account_info', {})
                    timeline = cached_data.get('timeline', [])
                    if account_info and timeline:
                        followers = account_info.get('followers_count', 0)
                        following = account_info.get('friends_count', 0)
                        posts = account_info.get('statuses_count', 0)
                        nick = account_info.get('nick', account_info.get('name', ''))
                        
                        account = Account(
                            username=username,
                            nick=nick,
                            followers=followers,
                            following=following,
                            posts=posts,
                            media_type=media_type,
                            media_list=timeline,
                            fetch_mode='batch' if cached_data.get('is_batch', False) else 'all',
                            fetch_timestamp=cached_data.get('fetch_timestamp')
                        )
                        
                        self.accounts.append(account)
                        self.update_account_list()
                        self.log_output.append(f'Loaded from cache: {username} - Followers: {followers:,} - Posts: {posts:,} • {media_type.title()}')
                        self.twitter_url.clear()
                        return
                except (KeyError, AttributeError, TypeError) as e:
                    pass

        try:
            self.reset_ui()
            
            self.is_auto_fetching = False
            self.is_multiple_user_mode = False
            self.current_fetch_username = username
            self.current_fetch_media_type = media_type
            
            
            self.log_output.append(f'Fetching metadata for {username}...')
            self.tab_widget.setCurrentWidget(self.process_tab)
            

            use_date_range = self.use_date_range_checkbox.isChecked()
            date_start = None
            date_end = None
            if use_date_range:
                date_start = self.start_date_edit.date().toString("yyyy-MM-dd")
                date_end = self.end_date_edit.date().toString("yyyy-MM-dd")
                self.log_output.append(f'Using date range: {date_start} to {date_end}')

            self.metadata_worker = MetadataFetchWorker(
                username,
                media_type,
                batch_mode=self.batch_mode,
                batch_size=self.batch_size if self.batch_mode else 0,
                page=0,
                timeline_type=self.timeline_type_combo.currentData() or 'timeline',
                include_retweets=self.include_retweets,
                use_date_range=use_date_range,
                date_start=date_start,
                date_end=date_end
            )
            self.metadata_worker.auth_token = self.auth_token_input.text().strip()
            self.metadata_worker.finished.connect(lambda data: self.on_metadata_fetched(data, username, media_type))
            self.metadata_worker.error.connect(self.on_metadata_error)            
            self.metadata_worker.start()
            
        except Exception as e:            
            self.log_output.append(f'Error: Failed to start metadata fetch: {str(e)}')
            self.update_account_list()
    
    def normalize_url_to_username(self, url_or_username):
        url_or_username = url_or_username.strip()
        username = url_or_username
        
        if "x.com/" in url_or_username or "twitter.com/" in url_or_username:
            parts = url_or_username.split('/')
            for i, part in enumerate(parts):
                if part in ['x.com', 'twitter.com'] and i + 1 < len(parts):
                    username = parts[i + 1]
                    username = username.split('/')[0]
                    break
        
        username = username.strip()
        return username if username else None
    
    def fetch_next_account_in_batch(self):

        if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode and not self.is_auto_fetching:
            self.log_output.append('Auto-fetch stopped by user. Multiple user processing halted.')

            if hasattr(self, 'accounts_to_fetch'):
                delattr(self, 'accounts_to_fetch')
            if hasattr(self, 'current_fetch_index'):
                delattr(self, 'current_fetch_index')
            if hasattr(self, 'is_multiple_user_mode'):
                delattr(self, 'is_multiple_user_mode')
            if hasattr(self, 'is_initial_fetch'):
                delattr(self, 'is_initial_fetch')
            self.twitter_url.clear()
            self.tab_widget.setCurrentIndex(0)
            return
        
        if not hasattr(self, 'accounts_to_fetch') or self.current_fetch_index >= len(self.accounts_to_fetch):
            total_accounts = len(self.accounts_to_fetch) if hasattr(self, 'accounts_to_fetch') else 0
            self.log_output.append(f'Batch processing completed! Processed {total_accounts} accounts.')
            self.twitter_url.clear()
            
            self.pause_fetch_btn.hide()
            self.stop_fetch_btn.hide()
            self.stop_fetch_btn.hide()
            self.is_auto_fetching = False
            
            if hasattr(self, 'accounts_to_fetch'):
                delattr(self, 'accounts_to_fetch')
            if hasattr(self, 'current_fetch_index'):
                delattr(self, 'current_fetch_index')
            if hasattr(self, 'is_multiple_user_mode'):
                delattr(self, 'is_multiple_user_mode')
            if hasattr(self, 'is_initial_fetch'):
                delattr(self, 'is_initial_fetch')
            self.tab_widget.setCurrentIndex(0)
            return
        
        username, media_type = self.accounts_to_fetch[self.current_fetch_index]
        self.log_output.append(f'Processing account {self.current_fetch_index + 1}/{len(self.accounts_to_fetch)}: {username} ({media_type})')
        
        existing_account = None
        for account in self.accounts:
            if account.username == username and account.media_type == media_type:
                existing_account = account
                break
        
        if existing_account and hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode and not getattr(self, 'is_initial_fetch', True):
            self.log_output.append(f'Resuming from existing progress for {username} ({media_type}) - {len(existing_account.media_list):,} items already fetched')
            
            cached_data = self.load_cached_data(username, media_type, is_batch=self.batch_mode)
            if cached_data and cached_data.get('metadata', {}).get('has_more', False):
                current_page = cached_data.get('metadata', {}).get('page', 0)
                next_page = current_page + 1
                self.log_output.append(f'Continuing from batch {next_page + 1} for {username}...')
                self.fetch_next_batch_for_current_user(username, media_type, next_page, cached_data.get('metadata', {}))
                return
            else:
                self.log_output.append(f'Account {username} already completed. Moving to next account.')
                self.current_fetch_index += 1
                self.fetch_next_account_in_batch()
                return
        elif existing_account:
            self.accounts.remove(existing_account)
            
            for is_batch in [True, False]:
                cache_file = self.get_cache_file_path(username, media_type, is_batch=is_batch)
                try:
                    if os.path.exists(cache_file):
                        os.remove(cache_file)
                        self.log_output.append(f'Removed existing cache: {os.path.basename(cache_file)}')
                except Exception as e:
                    self.log_output.append(f'Warning: Could not remove cache {os.path.basename(cache_file)}: {str(e)}')
            
            self.log_output.append(f'Preparing new batch for {username} ({media_type}) - cache cleared')
            self.update_account_list()

        if not existing_account:
            cached_data = self.load_cached_data(username, media_type, is_batch=self.batch_mode)
            if cached_data:
                try:
                    account_info = cached_data.get('account_info', {})
                    timeline = cached_data.get('timeline', [])
                    if account_info and timeline:
                        followers = account_info.get('followers_count', 0)
                        following = account_info.get('friends_count', 0)
                        posts = account_info.get('statuses_count', 0)
                        nick = account_info.get('nick', account_info.get('name', ''))
                        
                        account = Account(
                            username=username,
                            nick=nick,
                            followers=followers,
                            following=following,
                            posts=posts,
                            media_type=media_type,
                            media_list=timeline,
                            fetch_mode='batch' if cached_data.get('is_batch', False) else 'all',
                            fetch_timestamp=cached_data.get('fetch_timestamp')
                        )
                        
                        self.accounts.append(account)
                        self.update_account_list()
                        self.log_output.append(f'Loaded from cache: {username} - Followers: {followers:,} - Posts: {posts:,} • {media_type.title()}')

                        self.current_fetch_index += 1
                        self.fetch_next_account_in_batch()
                        return
                except (KeyError, AttributeError, TypeError) as e:
                    pass
        
        try:
            if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
                if self.batch_mode and not self.is_auto_fetching:
                    self.is_auto_fetching = True
                    self.pause_fetch_btn.hide()
                    self.stop_fetch_btn.show()
            else:
                self.is_auto_fetching = False
            
            self.current_fetch_username = username
            self.current_fetch_media_type = media_type
            
            
            self.metadata_worker = MetadataFetchWorker(
                username, 
                media_type, 
                batch_mode=self.batch_mode,
                batch_size=self.batch_size if self.batch_mode else 0,
                page=0,
                timeline_type=self.timeline_type_combo.currentData() or 'timeline',
                include_retweets=self.include_retweets
            )
            self.metadata_worker.auth_token = self.auth_token_input.text().strip()
            self.metadata_worker.finished.connect(lambda data: self.on_batch_metadata_fetched(data, username, media_type))
            self.metadata_worker.error.connect(self.on_batch_metadata_error)            
            self.metadata_worker.start()
            
        except Exception as e:            
            self.log_output.append(f'Error: Failed to start metadata fetch for {username}: {str(e)}')
            self.current_fetch_index += 1
            self.fetch_next_account_in_batch()
    
    def on_batch_metadata_fetched(self, data, username, media_type):
        try:
            if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode and not self.is_auto_fetching:
                self.log_output.append(f'Ignoring result for {username} - auto-fetch was stopped.')
                return
            
            if 'error' in data:
                self.log_output.append(f'Error fetching {username}: {data["error"]}')
                if self.is_auto_fetching:
                    self.current_fetch_index += 1
                    self.fetch_next_account_in_batch()
                return
                
            account_info = data.get('account_info', {})
            timeline = data.get('timeline', [])
            metadata = data.get('metadata', {})
            
            if not account_info:
                self.log_output.append(f'Error: Invalid account data received for {username}')
                self.current_fetch_index += 1
                self.fetch_next_account_in_batch()
                return
            
            if not timeline:
                media_type_display = media_type if media_type != 'all' else 'media'
                self.log_output.append(f'Warning: No {media_type_display} found for account {username}')
                self.current_fetch_index += 1
                self.fetch_next_account_in_batch()
                return
            
            existing_account = None
            for account in self.accounts:
                if account.username == username and account.media_type == media_type:
                    existing_account = account
                    break
            
            if existing_account:
                existing_account.media_list.extend(timeline)
                new_items = len(timeline)
                total_items = len(existing_account.media_list)
                self.log_output.append(f'Added {new_items:,} items to {username}. Total: {total_items:,} media items')
                
                updated_data = {
                    'account_info': account_info,
                    'timeline': existing_account.media_list,
                    'metadata': metadata,
                    'is_batch': self.batch_mode,
                    'fetch_timestamp': existing_account.fetch_timestamp
                }
                self.save_cached_data(username, media_type, updated_data, is_batch=self.batch_mode)
            else:
                followers = account_info.get('followers_count', 0)
                following = account_info.get('friends_count', 0)
                posts = account_info.get('statuses_count', 0)
                nick = account_info.get('nick', account_info.get('name', ''))
                profile_image_url = account_info.get('profile_image', '')
                
                account = Account(
                    username=username,
                    nick=nick,
                    followers=followers,
                    following=following,
                    posts=posts,
                    media_type=media_type,
                    profile_image=profile_image_url,
                    media_list=timeline,
                    fetch_mode='batch' if self.batch_mode else 'all',
                    fetch_timestamp=datetime.now().isoformat()
                )
                
                self.accounts.append(account)
                existing_account = account
                self.log_output.append(f'Successfully fetched: {username} - Followers: {followers:,} - Posts: {posts:,} • {media_type.title()}')
                
                updated_data = {
                    'account_info': account_info,
                    'timeline': timeline,
                    'metadata': metadata,
                    'is_batch': self.batch_mode,
                    'fetch_timestamp': account.fetch_timestamp
                }
                self.save_cached_data(username, media_type, updated_data, is_batch=self.batch_mode)
            
            self.update_account_list()
            
            if self.batch_mode and metadata.get('has_more', False):
                current_page = metadata.get('page', 0)
                next_page = current_page + 1
                
                self.is_auto_fetching = True
                self.log_output.append(f'Batch {current_page + 1} completed for {username}. Auto-fetching batch {next_page + 1}...')
                self.pause_fetch_btn.show()
                self.stop_fetch_btn.show()
                self.fetch_next_batch_for_current_user(username, media_type, next_page, metadata)
            else:
                if self.batch_mode:
                    total_items = len(existing_account.media_list)
                    self.log_output.append(f'All batches completed for {username}! Total: {total_items:,} media items')
                
                if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
                    if self.is_auto_fetching:
                        self.pause_fetch_btn.show()
                        self.stop_fetch_btn.show()
                        self.current_fetch_index += 1
                        self.fetch_next_account_in_batch()
                    else:
                        self.log_output.append('Auto-fetch stopped by user. Multiple user processing halted.')
                        return
                else:
                    self.current_fetch_index += 1
                    self.fetch_next_account_in_batch()
            
        except Exception as e:
            self.log_output.append(f'Error processing {username}: {str(e)}')
            self.current_fetch_index += 1
            self.fetch_next_account_in_batch()
    
    def fetch_next_batch_for_current_user(self, username, media_type, page, metadata):
        try:
            if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode and not self.is_auto_fetching:
                self.log_output.append('Auto-fetch stopped by user. Batch processing halted.')
                return
            
            if self.is_auto_fetching:
                self.pause_fetch_btn.show()
                self.stop_fetch_btn.show()
            
            self.metadata_worker = MetadataFetchWorker(
                username, 
                media_type, 
                batch_mode=True,
                batch_size=self.batch_size,
                page=page,
                timeline_type=self.timeline_type_combo.currentData() or 'timeline',
                include_retweets=self.include_retweets
            )
            self.metadata_worker.auth_token = self.auth_token_input.text().strip()
            self.metadata_worker.finished.connect(lambda data: self.on_batch_metadata_fetched(data, username, media_type))
            self.metadata_worker.error.connect(self.on_batch_metadata_error)            
            self.metadata_worker.start()
            
        except Exception as e:            
            self.log_output.append(f'Error: Failed to start next batch fetch for {username}: {str(e)}')
            if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode and not self.is_auto_fetching:
                return
            self.current_fetch_index += 1
            self.fetch_next_account_in_batch()
    
    def on_batch_metadata_error(self, error_message):
        if hasattr(self, 'accounts_to_fetch') and hasattr(self, 'current_fetch_index'):
            username, media_type = self.accounts_to_fetch[self.current_fetch_index]
            self.log_output.append(f'Error fetching {username}: {error_message}')
            
            if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode and not self.is_auto_fetching:
                self.log_output.append('Auto-fetch stopped by user. Error handling halted.')
                return
            
            if self.is_auto_fetching and hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
                self.pause_fetch_btn.show()
                self.stop_fetch_btn.show()
            
            self.current_fetch_index += 1
            self.fetch_next_account_in_batch()
        else:
            self.on_metadata_error(error_message)
        
    def on_metadata_fetched(self, data, username, media_type):
        try:
            if 'error' in data:
                self.log_output.append(f'Error: {data["error"]}')
                self.update_account_list()
                return
                
            account_info = data.get('account_info', {})
            timeline = data.get('timeline', [])
            metadata = data.get('metadata', {})
            if not account_info:
                self.log_output.append('Error: Invalid account data received')
                self.update_account_list()
                return
            
            if not timeline:
                media_type_display = media_type if media_type != 'all' else 'media'
                self.log_output.append(f'Error: No {media_type_display} found for account {username}')
                self.update_account_list()
                return
            existing_account = None
            for account in self.accounts:
                if account.username == username and account.media_type == media_type:
                    existing_account = account
                    break
            
            if existing_account:
                existing_account.media_list.extend(timeline)
                new_items = len(timeline)
                total_items = len(existing_account.media_list)
                self.log_output.append(f'Added {new_items:,} items. Total: {total_items:,} media items for {username}')
            else:
                followers = account_info.get('followers_count', 0)
                following = account_info.get('friends_count', 0)
                posts = account_info.get('statuses_count', 0)
                nick = account_info.get('nick', account_info.get('name', ''))
                profile_image_url = account_info.get('profile_image', '')
                
                account = Account(
                    username=username,
                    nick=nick,
                    followers=followers,
                    following=following,
                    posts=posts,
                    media_type=media_type,
                    profile_image=profile_image_url,
                    media_list=timeline,
                    fetch_mode='batch' if self.batch_mode else 'all',
                    fetch_timestamp=datetime.now().isoformat()
                )
                
                self.accounts.append(account)
                self.log_output.append(f'Successfully fetched: {username} - Followers: {followers:,} - Posts: {posts:,} • {media_type.title()}')
                existing_account = account
            
            updated_data = {
                'account_info': account_info,
                'timeline': existing_account.media_list,
                'metadata': metadata,
                'is_batch': self.batch_mode,
                'fetch_timestamp': existing_account.fetch_timestamp
            }
            self.save_cached_data(username, media_type, updated_data, is_batch=self.batch_mode)
            
            self.update_account_list()
            
            self.current_fetch_username = username
            self.current_fetch_media_type = media_type
            self.current_fetch_metadata = metadata
            
            if self.batch_mode and metadata.get('has_more', False):
                current_page = metadata.get('page', 0)
                next_page = current_page + 1
                self.log_output.append(f'Batch {current_page + 1} completed. Auto-fetching batch {next_page + 1}...')
                
                self.is_auto_fetching = True
                self.pause_fetch_btn.show()
                self.stop_fetch_btn.show()
                self.fetch_next_batch_internal()
            else:
                self.pause_fetch_btn.hide()
                self.stop_fetch_btn.hide()
                self.is_auto_fetching = False
                
                if self.batch_mode:
                    total_items = len(existing_account.media_list)
                    self.log_output.append(f'All batches complete! Total: {total_items:,} media items for {username}')
                
                self.twitter_url.clear()
                
                if hasattr(self, 'accounts_to_update') and hasattr(self, 'current_update_index'):
                    self.current_update_index += 1
                    self.update_next_account()
                else:
                    self.tab_widget.setCurrentIndex(0)
        except Exception as e:
            self.log_output.append(f'Error: {str(e)}')
            self.update_account_list()
            
    def on_metadata_error(self, error_message):
        self.log_output.append(f'Error: {error_message}')
        
        if self.is_auto_fetching:
            self.stop_auto_fetch()
        
        self.pause_fetch_btn.hide()
        self.stop_fetch_btn.hide()
        
        self.enable_batch_buttons()
        
        if hasattr(self, 'accounts_to_update') and hasattr(self, 'current_update_index'):
            self.current_update_index += 1
            self.update_next_account()
        
        self.update_account_list()

    def update_account_list(self):
        self.sort_accounts()
        
        self.account_list.clear()
        for i, account in enumerate(self.accounts, 1):
            media_count = len(account.media_list) if account.media_list else 0
            
            if account.media_type.lower() == "gif":
                media_type_display = "GIF"
            elif account.media_type.lower() == "all":
                media_type_display = "All"
            elif account.media_type.lower() == "image":
                media_type_display = "Image"
            elif account.media_type.lower() == "video":
                media_type_display = "Video"
            else:
                media_type_display = account.media_type.title()
            
            line1 = f"{i}. {account.username} ({account.nick})"
            line2 = f"Followers: {account.followers:,} • Following: {account.following:,} • Posts: {account.posts:,} • {media_type_display}: {media_count:,}"
            line3 = self.format_fetch_info(account.fetch_timestamp)
            display_text = f"{line1}\n{line2}\n{line3}"
            item = QListWidgetItem()
            item.setText(display_text)
            item.setSizeHint(QSize(0, 70))  
            
            if account.profile_image:
                if account.profile_image in self.profile_image_cache:
                    item.setIcon(QIcon(self.profile_image_cache[account.profile_image]))
                else:
                    self.download_profile_image(account.profile_image)
                    placeholder = self.create_placeholder_icon(48)
                    if placeholder:
                        item.setIcon(QIcon(placeholder))
            else:
                placeholder = self.create_placeholder_icon(48)
                if placeholder:
                    item.setIcon(QIcon(placeholder))
            
            self.account_list.addItem(item)

        if hasattr(self, 'search_input'):
            search_text = self.search_input.text()
            has_group_filter = hasattr(self, 'group_filter_combo') and self.group_filter_combo.currentData() is not None
            if search_text or has_group_filter:
                self.filter_accounts()

        self.update_button_states()

    def create_colored_icon(self, svg_path, color):
        with open(svg_path, 'r', encoding='utf-8') as f:
            svg_content = f.read()
        
        svg_content = svg_content.replace('stroke="currentColor"', f'stroke="{color}"')
        svg_content = svg_content.replace('fill="currentColor"', f'fill="{color}"')
        
        pixmap = QPixmap(24, 24)
        pixmap.fill(Qt.GlobalColor.transparent)
        
        from PyQt6.QtSvg import QSvgRenderer
        renderer = QSvgRenderer(svg_content.encode('utf-8'))
        painter = QPainter(pixmap)
        renderer.render(painter)
        painter.end()
        
        return QIcon(pixmap)

    def create_placeholder_icon(self, size=48):
        pixmap = QPixmap(size, size)
        pixmap.fill(Qt.GlobalColor.transparent)
        
        painter = QPainter(pixmap)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        painter.setBrush(Qt.GlobalColor.gray)
        painter.setPen(Qt.PenStyle.NoPen)
        painter.drawRoundedRect(0, 0, size, size, 8, 8)
        
        painter.setPen(Qt.GlobalColor.white)
        font_size = int(size * 0.4)
        painter.setFont(painter.font())
        painter.drawText(pixmap.rect(), Qt.AlignmentFlag.AlignCenter, "👤")
        
        painter.end()
        return pixmap

    def create_square_pixmap(self, original_pixmap, size=48):
        if original_pixmap.isNull():
            return self.create_placeholder_icon(size)
        
        square_pixmap = QPixmap(size, size)
        square_pixmap.fill(Qt.GlobalColor.transparent)
        
        painter = QPainter(square_pixmap)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        path = QPainterPath()
        path.addRoundedRect(0, 0, size, size, 8, 8)
        painter.setClipPath(path)
        
        scaled_pixmap = original_pixmap.scaled(
            size, size, 
            Qt.AspectRatioMode.KeepAspectRatioByExpanding, 
            Qt.TransformationMode.SmoothTransformation
        )
        
        x = (size - scaled_pixmap.width()) // 2
        y = (size - scaled_pixmap.height()) // 2
        painter.drawPixmap(x, y, scaled_pixmap)
        
        painter.end()
        return square_pixmap

    def download_profile_image(self, url):
        if not url or url in self.profile_image_cache or url in self.pending_downloads:
            return
        try:
            request = QNetworkRequest(QUrl(url))
            request.setHeader(QNetworkRequest.KnownHeaders.UserAgentHeader, 
                            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            
            reply = self.network_manager.get(request)
            self.pending_downloads[url] = reply
            reply.finished.connect(lambda: self.on_profile_image_downloaded(reply, url))
        except Exception as e:
            pass

    def on_profile_image_downloaded(self, reply, image_url):
        try:
            if reply.error() == reply.NetworkError.NoError:
                data = reply.readAll()
                pixmap = QPixmap()
                if pixmap.loadFromData(data):
                    square_pixmap = self.create_square_pixmap(pixmap, 48)
                    self.profile_image_cache[image_url] = square_pixmap
                    
                    self.update_account_list()
            
            if image_url in self.pending_downloads:
                del self.pending_downloads[image_url]
                
        except Exception as e:
            pass
        finally:
            reply.deleteLater()

    def filter_accounts(self):
        search_text = self.search_input.text().lower().strip()

        for i in range(self.account_list.count()):
            item = self.account_list.item(i)
            if search_text:
                item_text = item.text().lower()
                item.setHidden(search_text not in item_text)
            else:
                item.setHidden(False)

    def on_sort_changed(self):
        self.sort_accounts()
        self.update_account_list()
        self.filter_accounts()
        if hasattr(self, 'is_initializing') and not self.is_initializing:
            if hasattr(self, 'settings'):
                self.settings.setValue('sort_preference', self.sort_combo.currentData())
                self.settings.sync()

    def sort_accounts(self):
        if not hasattr(self, 'sort_combo'):
            return
            
        sort_by = self.sort_combo.currentData()

        if sort_by == "username_asc":
            self.accounts.sort(key=lambda x: x.username.lower())
        elif sort_by == "username_desc":
            self.accounts.sort(key=lambda x: x.username.lower(), reverse=True)
        elif sort_by == "followers_desc":
            self.accounts.sort(key=lambda x: x.followers, reverse=True)
        elif sort_by == "followers_asc":
            self.accounts.sort(key=lambda x: x.followers)
        elif sort_by == "posts_desc":
            self.accounts.sort(key=lambda x: x.posts, reverse=True)
        elif sort_by == "posts_asc":
            self.accounts.sort(key=lambda x: x.posts)
        elif sort_by == "media_count_desc":
            self.accounts.sort(key=lambda x: len(x.media_list) if x.media_list else 0, reverse=True)
        elif sort_by == "media_count_asc":
            self.accounts.sort(key=lambda x: len(x.media_list) if x.media_list else 0)
        elif sort_by == "fetch_timestamp_asc":
            self.accounts.sort(key=lambda x: (x.fetch_timestamp is None or x.fetch_timestamp == "", x.fetch_timestamp or "9999"))
        elif sort_by == "fetch_timestamp_desc":
            self.accounts.sort(key=lambda x: (x.fetch_timestamp is None or x.fetch_timestamp == "", x.fetch_timestamp or ""), reverse=True)
        

    def update_button_states(self):
        has_accounts = len(self.accounts) > 0
        selected_items = self.account_list.selectedItems()
        has_selected = len(selected_items) > 0

        self.import_btn.setEnabled(True)
        self.export_btn.setEnabled(has_accounts)
        self.download_selected_btn.setEnabled(has_accounts)
        self.update_selected_btn.setEnabled(has_accounts)
        self.delete_btn.setEnabled(has_accounts)

        self.import_btn.show()
        self.export_btn.show()
        self.download_selected_btn.show()
        self.update_selected_btn.show()
        self.delete_btn.show()
        
        if len(selected_items) == 1:
            item = selected_items[0]
            for account in self.accounts:
                item_text = item.text()
                if account.username in item_text:
                    self.twitter_url.setText(account.username)
                    break
    
    def hide_account_buttons(self):
        self.import_btn.setEnabled(True)
        self.export_btn.setEnabled(False)
        self.download_selected_btn.setEnabled(False)
        self.update_selected_btn.setEnabled(False)
        self.delete_btn.setEnabled(False)
        
        self.import_btn.show()
        self.export_btn.show()
        self.download_selected_btn.show()
        self.update_selected_btn.show()
        self.delete_btn.show()

    def download_selected(self):
        selected_items = self.account_list.selectedItems()
        
        if not selected_items:
            visible_indices = []
            for i in range(self.account_list.count()):
                item = self.account_list.item(i)
                if not item.isHidden():
                    visible_indices.append(i)
            
            visible_count = len(visible_indices)
            reply = QMessageBox.question(
                self,
                'Confirm Download All',
                f'No accounts selected. Download all {visible_count} visible account(s)?',
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if reply == QMessageBox.StandardButton.Yes:
                self.download_accounts(visible_indices)
            return
        
        self.download_accounts([self.account_list.row(item) for item in selected_items])

    def update_selected(self):
        selected_items = self.account_list.selectedItems()
        
        if not selected_items:
            visible_accounts = []
            for i in range(self.account_list.count()):
                item = self.account_list.item(i)
                if not item.isHidden() and i < len(self.accounts):
                    visible_accounts.append(self.accounts[i])
            
            visible_count = len(visible_accounts)
            reply = QMessageBox.question(
                self,
                'Confirm Update All',
                f'No accounts selected. Update all {visible_count} visible account(s)?',
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if reply == QMessageBox.StandardButton.No:
                return
            selected_accounts = visible_accounts
        else:
            selected_accounts = []
            for item in selected_items:
                index = self.account_list.row(item)
                if index < len(self.accounts):
                    selected_accounts.append(self.accounts[index])
        
        if not selected_accounts:
            return
            
        self.tab_widget.setCurrentWidget(self.process_tab)
        
        self.log_output.clear()
        self.log_output.append(f'Starting update for {len(selected_accounts)} account(s)...')
        
        self.accounts_to_update = selected_accounts.copy()
        self.current_update_index = 0
        
        self.update_next_account()

    def update_next_account(self):
        if not hasattr(self, 'accounts_to_update') or self.current_update_index >= len(self.accounts_to_update):
            self.log_output.append('All selected accounts have been updated!')
            if hasattr(self, 'accounts_to_update'):
                delattr(self, 'accounts_to_update')
            if hasattr(self, 'current_update_index'):
                delattr(self, 'current_update_index')
            self.tab_widget.setCurrentIndex(0)
            return
            
        account = self.accounts_to_update[self.current_update_index]
        self.log_output.append(f'Updating account {self.current_update_index + 1}/{len(self.accounts_to_update)}: {account.username} ({account.media_type})')
        self.update_account(account)

    def update_account(self, account):
        original_batch_mode = self.batch_mode
        original_batch_size = self.batch_size
        original_media_type = self.media_type_combo.currentData()
        
        self.batch_mode = (account.fetch_mode == 'batch')
        if self.batch_mode:
            self.batch_size = self.settings.value('batch_size', 100, type=int)
        
        for i in range(self.media_type_combo.count()):
            if self.media_type_combo.itemData(i) == account.media_type:
                self.media_type_combo.setCurrentIndex(i)
                break
        
        self.twitter_url.setText(account.username)
        
        self.tab_widget.setCurrentWidget(self.process_tab)
        
        self.log_output.clear()
        self.log_output.append(f'Updating {account.username} ({account.media_type}) using {account.fetch_mode} mode...')
        
        username = account.username
        media_type = account.media_type
        
        for is_batch in [True, False]:
            cache_file = self.get_cache_file_path(username, media_type, is_batch=is_batch)
            try:
                if os.path.exists(cache_file):
                    os.remove(cache_file)
                    self.log_output.append(f'Removed existing cache: {os.path.basename(cache_file)}')
            except Exception as e:
                self.log_output.append(f'Warning: Could not remove cache {os.path.basename(cache_file)}: {str(e)}')
        
        for i, existing_account in enumerate(self.accounts):
            if existing_account.username == account.username and existing_account.media_type == account.media_type:
                self.accounts.pop(i)
                break
        
        self.update_account_list()
        
        self.start_fetch_process(username, media_type)
        
        self.batch_mode = original_batch_mode
        self.batch_size = original_batch_size
        
        for i in range(self.media_type_combo.count()):
            if self.media_type_combo.itemData(i) == original_media_type:
                self.media_type_combo.setCurrentIndex(i)
                break

    def start_fetch_process(self, username, media_type):
        if not self.auth_token_input.text().strip():
            self.log_output.append("Error: Please enter your auth token")
            return
            
        
        self.worker = MetadataFetchWorker(
            username, 
            media_type, 
            batch_mode=self.batch_mode,
            batch_size=self.batch_size if self.batch_mode else 0,
            page=0,
            timeline_type=self.timeline_type_combo.currentData() or 'timeline',
            include_retweets=self.include_retweets
        )
        self.worker.auth_token = self.auth_token_input.text().strip()
        self.worker.finished.connect(lambda data: self.on_metadata_fetched(data, username, media_type))
        self.worker.error.connect(self.on_metadata_error)
        self.worker.start()

    def download_accounts(self, indices):
        self.log_output.clear()
        outpath = self.output_dir.text()
        if not os.path.exists(outpath):
            self.log_output.append('Warning: Invalid output directory.')
            return

        if not self.auth_token_input.text().strip():
            self.log_output.append("Error: Please enter your auth token")
            return

        accounts_to_download = [self.accounts[i] for i in indices]

        try:
            self.start_download_worker(accounts_to_download, outpath)
        except Exception as e:
            self.log_output.append(f"Error: An error occurred while starting the download: {str(e)}")

    def start_download_worker(self, accounts_to_download, outpath):
        self.worker = DownloadWorker(
            accounts_to_download,
            outpath,
            self.auth_token_input.text().strip(),
            self.filename_format,
            self.download_batch_size,
            self.convert_gif,
            self.gif_resolution,
            self.gif_conversion_mode,
            self.db_manager
        )
        self.worker.finished.connect(self.on_download_finished)
        self.worker.progress.connect(self.update_progress)
        self.worker.conversion_progress.connect(self.update_conversion_progress)
        self.worker.download_progress.connect(self.update_download_progress)
        self.worker.start()
        self.start_timer()
        self.update_ui_for_download_start(len(accounts_to_download))

    def update_ui_for_download_start(self, account_count):
        self.download_selected_btn.setEnabled(False)
        self.update_selected_btn.setEnabled(False)
        self.stop_btn.show()
        self.pause_resume_btn.show()
        
        self.pause_fetch_btn.hide()
        self.stop_fetch_btn.hide()
        
        self.progress_bar.show()
        self.progress_bar.setValue(0)
        
        self.tab_widget.setCurrentWidget(self.process_tab)

    def update_progress(self, message, percentage):
        cursor = self.log_output.textCursor()
        cursor.movePosition(QTextCursor.MoveOperation.End)
        cursor.select(QTextCursor.SelectionType.LineUnderCursor)
        current_text = cursor.selectedText()
        progress_indicators = [
            "Downloading",
            "Account",
            "files downloaded"
        ]
        
        should_replace = False
        for indicator in progress_indicators:
            if indicator in current_text and indicator in message:
                should_replace = True
                break
        if should_replace and current_text.strip():
            cursor.removeSelectedText()
            cursor.deletePreviousChar()
        
        self.log_output.append(message)
        self.log_output.moveCursor(QTextCursor.MoveOperation.End)
        
        if percentage > 0:
            self.progress_bar.setValue(percentage)
                
    def update_conversion_progress(self, message, percentage):
        cursor = self.log_output.textCursor()
        cursor.movePosition(QTextCursor.MoveOperation.End)
        cursor.select(QTextCursor.SelectionType.LineUnderCursor)
        current_text = cursor.selectedText()
        if "Converting GIF" in current_text:
            cursor.removeSelectedText()
            cursor.deletePreviousChar()
        
        self.log_output.append(message)
        self.log_output.moveCursor(QTextCursor.MoveOperation.End)
        
        if percentage > 0:
            self.progress_bar.setValue(percentage)

    def update_download_progress(self, message, percentage):
        cursor = self.log_output.textCursor()
        cursor.movePosition(QTextCursor.MoveOperation.End)
        cursor.select(QTextCursor.SelectionType.LineUnderCursor)
        
        current_text = cursor.selectedText()
        if "Downloading" in current_text and ":" in current_text and "/" in current_text:
            cursor.removeSelectedText()
            cursor.deletePreviousChar()
        
        self.log_output.append(message)
        self.log_output.moveCursor(QTextCursor.MoveOperation.End)
        
        if percentage > 0:
            self.progress_bar.setValue(percentage)

    def stop_download(self):
        if hasattr(self, 'worker'):
            self.worker.stop()
        self.stop_timer()
        self.on_download_finished(True, "Download stopped by user.")
        
    def on_download_finished(self, success, message):
        self.progress_bar.hide()
        self.stop_btn.hide()
        self.pause_resume_btn.hide()
        self.pause_resume_btn.setText(' Pause')
        icon_dir = ICON_DIR
        self.pause_resume_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), self.current_theme_color))
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
    
    def toggle_pause_resume(self):
        if hasattr(self, 'worker'):
            icon_dir = ICON_DIR
            accent_color = self.current_theme_color
            
            if self.worker.is_paused:
                self.worker.resume()
                self.pause_resume_btn.setText(' Pause')
                self.pause_resume_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), accent_color))
                self.timer.start(1000)
            else:
                self.worker.pause()
                self.pause_resume_btn.setText(' Resume')
                self.pause_resume_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-play.svg'), accent_color))

    def delete_accounts(self):
        selected_items = self.account_list.selectedItems()
        
        if not selected_items:
            visible_accounts = []
            visible_indices = []
            for i in range(self.account_list.count()):
                item = self.account_list.item(i)
                if not item.isHidden() and i < len(self.accounts):
                    visible_accounts.append(self.accounts[i])
                    visible_indices.append(i)
            
            visible_count = len(visible_accounts)
            reply = QMessageBox.question(
                self,
                'Confirm Delete All',
                f'No accounts selected. Delete all {visible_count} visible account(s) from database?',
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if reply == QMessageBox.StandardButton.No:
                return
            
            for account in visible_accounts:
                username = account.username
                if username:
                    try:
                        self.db_manager.delete_account(username, account.media_type)
                        self.log_output.append(f'Removed from database: {username} ({account.media_type})')
                    except Exception as e:
                        self.log_output.append(f'Warning: Could not remove {username}: {str(e)}')
            
            for index in sorted(visible_indices, reverse=True):
                self.accounts.pop(index)
            
            self.update_account_list()
            self.update_button_states()
        else:
            reply = QMessageBox.question(
                self,
                'Confirm Delete',
                f'Delete {len(selected_items)} selected account(s) from database?',
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if reply == QMessageBox.StandardButton.No:
                return
            
            selected_indices = sorted([self.account_list.row(item) for item in selected_items], reverse=True)
            
            for index in selected_indices:
                account = self.accounts[index]
                username = account.username
                if username:
                    try:
                        self.db_manager.delete_account(username, account.media_type)
                        self.log_output.append(f'Removed from database: {username} ({account.media_type})')
                    except Exception as e:
                        self.log_output.append(f'Warning: Could not remove {username}: {str(e)}')
                
                self.accounts.pop(index)
            
            self.update_account_list()
            self.update_button_states()

    def export_accounts(self):
        self.tab_widget.setCurrentWidget(self.process_tab)
        
        selected_items = self.account_list.selectedItems()
        
        if not selected_items:
            reply = QMessageBox.question(
                self,
                'Confirm Export All',
                f'No accounts selected. Export all {len(self.accounts)} account(s)?',
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
                QMessageBox.StandardButton.No
            )
            if reply == QMessageBox.StandardButton.No:
                self.log_output.append('Export cancelled')
                return
            selected_accounts = self.accounts.copy()
        else:
            selected_accounts = []
            for item in selected_items:
                row = self.account_list.row(item)
                if 0 <= row < len(self.accounts):
                    selected_accounts.append(self.accounts[row])
        
        if not selected_accounts:
            self.log_output.append('No valid accounts found for export')
            return
        
        now = datetime.now()
        date_str = now.strftime("%Y%m%d_%H%M%S")
        default_filename = f"twitterxmediabatchdownloader_{date_str}.db"
        
        file_path, _ = QFileDialog.getSaveFileName(
            self,
            "Export Database",
            os.path.join(self.last_output_path, default_filename),
            "Database Files (*.db)"
        )
        
        if not file_path:
            self.log_output.append('Export cancelled')
            return
        
        self.log_output.append(f'Starting export of {len(selected_accounts)} account(s) to database...')
        
        try:
            export_db_manager = DatabaseManager(file_path)
            exported_count = 0
            
            for account in selected_accounts:
                try:
                    export_db_manager.save_account(
                        username=account.username,
                        nick=account.nick,
                        followers=account.followers,
                        following=account.following,
                        posts=account.posts,
                        media_type=account.media_type,
                        profile_image=account.profile_image,
                        fetch_mode=account.fetch_mode,
                        fetch_timestamp=account.fetch_timestamp
                    )
                    
                    if account.media_list:
                        export_db_manager.save_media_list(account.username, account.media_list)
                    
                    exported_count += 1
                    media_count = len(account.media_list) if account.media_list else 0
                    self.log_output.append(f'Exported: {account.username} ({media_count:,} media)')
                    
                except Exception as e:
                    self.log_output.append(f'Error exporting {account.username}: {str(e)}')
            
            export_db_manager.close()
            self.log_output.append(f'Export completed: {exported_count}/{len(selected_accounts)} account(s) exported')
            self.log_output.append(f'Export location: {file_path}')
            
        except Exception as e:
            self.log_output.append(f'Export error: {str(e)}')

    def import_accounts(self):
        self.tab_widget.setCurrentWidget(self.process_tab)

        file_paths, selected_filter = QFileDialog.getOpenFileNames(
            self,
            "Select File(s) to Import (Multiple Selection Allowed)",
            self.last_output_path,
            "All Supported Files (*.db *.json);;Database Files (*.db);;JSON Files (*.json)"
        )

        if not file_paths:
            self.log_output.append('Import cancelled - No files selected')
            return

        self.log_output.append(f'Starting import of {len(file_paths)} file(s)...')

        total_imported = 0
        total_skipped = 0
        total_errors = 0

        for file_path in file_paths:
            self.log_output.append(f'\n--- Processing: {os.path.basename(file_path)} ---')

            imported_count = 0
            skipped_count = 0
            error_count = 0

            if file_path.endswith('.db'):
                self.log_output.append(f'Importing from database: {os.path.basename(file_path)}')

                try:
                    import_db_manager = DatabaseManager(file_path)
                    db_accounts = import_db_manager.get_all_accounts()

                    for db_account in db_accounts:
                        try:
                            username = db_account['username']
                            media_type = db_account['media_type']

                            existing_account = next(
                                (acc for acc in self.accounts
                                 if acc.username == username and acc.media_type == media_type),
                                None
                            )

                            if existing_account:
                                self.log_output.append(f'Skipped {username} ({media_type}): Already exists in dashboard')
                                skipped_count += 1
                                continue

                            media_list = import_db_manager.get_media_list(username)

                            account = Account(
                                username=username,
                                nick=db_account['nick'],
                                followers=db_account['followers'],
                                following=db_account['following'],
                                posts=db_account['posts'],
                                media_type=media_type,
                                profile_image=db_account['profile_image'],
                                media_list=media_list,
                                fetch_mode=db_account.get('fetch_mode', 'all'),
                                fetch_timestamp=db_account.get('fetch_timestamp')
                            )

                            self.accounts.append(account)

                            self.db_manager.save_account(
                                username=username,
                                nick=db_account['nick'],
                                followers=db_account['followers'],
                                following=db_account['following'],
                                posts=db_account['posts'],
                                media_type=media_type,
                                profile_image=db_account['profile_image'],
                                fetch_mode=db_account.get('fetch_mode', 'all'),
                                fetch_timestamp=db_account.get('fetch_timestamp')
                            )

                            if media_list:
                                self.db_manager.save_media_list(username, media_list)

                            imported_count += 1
                            media_count = len(media_list)
                            self.log_output.append(f'Imported: {username} ({media_type}) - {media_count:,} media')

                        except Exception as e:
                            self.log_output.append(f'Error importing account: {str(e)}')
                            error_count += 1

                    import_db_manager.close()

                except Exception as e:
                    self.log_output.append(f'Error opening database: {str(e)}')
                    error_count += 1

            elif file_path.endswith('.json'):
                self.log_output.append(f'Importing from JSON: {os.path.basename(file_path)}')

                try:
                    with open(file_path, 'r', encoding='utf-8') as f:
                        data = json.load(f)

                    username = data.get('username')
                    if not username:
                        self.log_output.append(f'Error: No username found in JSON file')
                        error_count += 1
                    else:
                        media_type = data.get('media_type', 'all')

                        existing_account = next(
                            (acc for acc in self.accounts
                             if acc.username == username and acc.media_type == media_type),
                            None
                        )

                        if existing_account:
                            self.log_output.append(f'Skipped {username} ({media_type}): Already exists in dashboard')
                            skipped_count += 1
                        else:
                            account = Account(
                                username=username,
                                nick=data.get('nick', ''),
                                followers=data.get('followers', 0),
                                following=data.get('following', 0),
                                posts=data.get('posts', 0),
                                media_type=media_type,
                                profile_image=data.get('profile_image'),
                                media_list=data.get('media_list', []),
                                fetch_mode=data.get('fetch_mode', 'all'),
                                fetch_timestamp=data.get('fetch_timestamp')
                            )

                            self.accounts.append(account)

                            self.db_manager.save_account(
                                username=username,
                                nick=account.nick,
                                followers=account.followers,
                                following=account.following,
                                posts=account.posts,
                                media_type=media_type,
                                profile_image=account.profile_image,
                                fetch_mode=account.fetch_mode,
                                fetch_timestamp=account.fetch_timestamp
                            )

                            if account.media_list:
                                self.db_manager.save_media_list(username, account.media_list)

                            imported_count += 1
                            media_count = len(account.media_list) if account.media_list else 0
                            self.log_output.append(f'Imported: {username} ({media_type}) - {media_count:,} media')

                except Exception as e:
                    self.log_output.append(f'Error importing JSON: {str(e)}')
                    error_count += 1

            else:
                self.log_output.append('Error: Unsupported file format')
                error_count += 1

            self.log_output.append(f'File result: {imported_count} imported, {skipped_count} skipped, {error_count} errors')
            total_imported += imported_count
            total_skipped += skipped_count
            total_errors += error_count

        self.update_account_list()
        self.update_button_states()

        self.log_output.append(f'\n=== TOTAL IMPORT SUMMARY ===')
        self.log_output.append(f'Files processed: {len(file_paths)}')
        self.log_output.append(f'Total imported: {total_imported}')
        self.log_output.append(f'Total skipped: {total_skipped}')
        self.log_output.append(f'Total errors: {total_errors}')

        self.tab_widget.setCurrentIndex(0)

    def update_timer(self):
        self.elapsed_time = self.elapsed_time.addSecs(1)
        self.time_label.setText(self.elapsed_time.toString("hh:mm:ss"))
    
    def start_timer(self):
        self.elapsed_time = QTime(0, 0, 0)
        self.time_label.setText("00:00:00")
        self.time_label.show()
        self.timer.start(1000)
    
    def stop_timer(self):
        self.timer.stop()
        self.time_label.hide()
    
    def fetch_next_batch(self):
        if not self.current_fetch_metadata or not self.current_fetch_metadata.get('has_more', False):
            self.log_output.append('No more batches available.')
            return
        
        self.fetch_next_batch_internal()
    
    def start_auto_batch(self):
        if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
            self.is_auto_fetching = True
            self.is_initial_fetch = False
            self.pause_fetch_btn.hide()
            self.stop_fetch_btn.hide()
            self.stop_fetch_btn.show()
            
            self.log_output.append('Resuming multiple user batch processing...')
            
            self.fetch_next_account_in_batch()
            return
        
        if not self.current_fetch_metadata or not self.current_fetch_metadata.get('has_more', False):
            self.log_output.append('No more batches available.')
            return
        
        self.is_auto_fetching = True
        self.pause_fetch_btn.show()
        self.stop_fetch_btn.show()
        
        self.log_output.append('Auto batch mode enabled.')
        self.fetch_next_batch_internal()
    
    def pause_batch_fetch(self):
        if not self.is_auto_fetching:
            return
            
        self.is_auto_fetching = False
        
        if hasattr(self, 'metadata_worker') and self.metadata_worker.isRunning():
            self.metadata_worker.terminate()
            self.metadata_worker.wait()
        
        self.pause_fetch_btn.setText(' Resume')
        icon_dir = ICON_DIR
        self.pause_fetch_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-play.svg'), self.current_theme_color))
        self.pause_fetch_btn.clicked.disconnect()
        self.pause_fetch_btn.clicked.connect(self.resume_batch_fetch)
        self.log_output.append('Batch fetching paused.')
    
    def resume_batch_fetch(self):
        self.is_auto_fetching = True
        self.pause_fetch_btn.setText(' Pause')
        icon_dir = ICON_DIR
        self.pause_fetch_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), self.current_theme_color))
        self.pause_fetch_btn.clicked.disconnect()
        self.pause_fetch_btn.clicked.connect(self.pause_batch_fetch)
        self.log_output.append('Resuming batch fetching...')
        
        if hasattr(self, 'current_fetch_metadata') and self.current_fetch_metadata:
            if self.current_fetch_metadata.get('has_more', False):
                self.fetch_next_batch_internal()
            elif hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
                self.fetch_next_account_in_batch()
    
    def stop_batch_fetch(self):
        self.is_auto_fetching = False
        self.pause_fetch_btn.hide()
        self.stop_fetch_btn.hide()
        
        if hasattr(self, 'metadata_worker') and self.metadata_worker.isRunning():
            self.metadata_worker.terminate()
            self.metadata_worker.wait()
        
        if self.pause_fetch_btn.text() == ' Resume':
            self.pause_fetch_btn.setText(' Pause')
            icon_dir = ICON_DIR
            self.pause_fetch_btn.setIcon(self.create_colored_icon(os.path.join(icon_dir, 'player-pause.svg'), self.current_theme_color))
            self.pause_fetch_btn.clicked.disconnect()
            self.pause_fetch_btn.clicked.connect(self.pause_batch_fetch)
        
        if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
            self.log_output.append('Batch processing stopped.')
            if hasattr(self, 'accounts_to_fetch'):
                delattr(self, 'accounts_to_fetch')
            if hasattr(self, 'current_fetch_index'):
                delattr(self, 'current_fetch_index')
            self.is_multiple_user_mode = False
        else:
            self.log_output.append('Batch fetching stopped.')
    
    def stop_auto_fetch(self):
        self.stop_batch_fetch()
    
    def cancel_multiple_user_fetch(self):
        self.stop_batch_fetch()
    
    def disable_batch_buttons(self):
        self.pause_fetch_btn.setEnabled(False)
        self.stop_fetch_btn.setEnabled(False)
        
    def enable_batch_buttons(self):
        self.pause_fetch_btn.setEnabled(True)
        self.stop_fetch_btn.setEnabled(True)

    def fetch_next_batch_internal(self):
        if not self.current_fetch_username or not self.current_fetch_media_type or not self.current_fetch_metadata:
            return
        
        current_page = self.current_fetch_metadata.get('page', 0)
        next_page = current_page + 1
        
        self.log_output.append(f'Fetching batch {next_page + 1} for {self.current_fetch_username}...')
        
        self.metadata_worker = MetadataFetchWorker(
            self.current_fetch_username, 
            self.current_fetch_media_type, 
            batch_mode=True,
            batch_size=self.batch_size,
            page=next_page,
            timeline_type=self.timeline_type_combo.currentData() or 'timeline',
            include_retweets=self.include_retweets
        )
        self.metadata_worker.auth_token = self.auth_token_input.text().strip()
        self.metadata_worker.finished.connect(lambda data: self.on_metadata_fetched(data, self.current_fetch_username, self.current_fetch_media_type))
        self.metadata_worker.error.connect(self.on_metadata_error)
        self.metadata_worker.start()

    def on_tab_changed(self, index):
        if index == 0:
            self.update_button_states()
    
    def load_groups(self):
        self.group_list.clear()
        self.group_filter_combo.clear()
        self.group_filter_combo.addItem("All Groups", None)
        self.group_filter_combo.addItem("No Group", -1)
        
        groups = self.db_manager.get_all_groups()
        for group in groups:
            accounts_in_group = self.db_manager.get_accounts_by_group(group['id'])
            account_count = len(accounts_in_group)
            item = QListWidgetItem(f"{group['name']} ({account_count})")
            item.setData(Qt.ItemDataRole.UserRole, group['id'])
            self.group_list.addItem(item)
            self.group_filter_combo.addItem(group['name'], group['id'])
    
    def create_group(self):
        name = self.group_name_input.text().strip()
        if not name:
            QMessageBox.warning(self, "Warning", "Please enter a group name.")
            return
        
        group_id = self.db_manager.create_group(name)
        if group_id:
            self.group_name_input.clear()
            self.load_groups()
            QMessageBox.information(self, "Success", f"Group '{name}' created successfully!")
        else:
            QMessageBox.warning(self, "Error", f"Group '{name}' already exists.")
    
    def on_group_selected(self):
        selected = len(self.group_list.selectedItems()) > 0
        self.rename_group_btn.setEnabled(selected)
        self.delete_group_btn.setEnabled(selected)
        
        if selected:
            self.load_assign_accounts()
        else:
            self.assign_account_list.clear()
            self.save_assignment_btn.setEnabled(False)
    
    def load_assign_accounts(self):
        selected_items = self.group_list.selectedItems()
        if not selected_items:
            return
        
        group_id = selected_items[0].data(Qt.ItemDataRole.UserRole)
        
        self.assign_account_list.blockSignals(True)
        self.assign_account_list.clear()
        
        for account in self.accounts:
            if account.group_id is not None and account.group_id != group_id:
                continue
            
            item_text = f"{account.username} ({account.media_type})"
            item = QListWidgetItem(item_text)
            item.setFlags(item.flags() | Qt.ItemFlag.ItemIsUserCheckable)
            item.setCheckState(Qt.CheckState.Checked if account.group_id == group_id else Qt.CheckState.Unchecked)
            item.setData(Qt.ItemDataRole.UserRole, (account.username, account.media_type))
            self.assign_account_list.addItem(item)
        
        self.assign_account_list.blockSignals(False)
        self.save_assignment_btn.setEnabled(True)
    
    def filter_assign_accounts(self):
        search_text = self.assign_search_input.text().lower().strip()
        for i in range(self.assign_account_list.count()):
            item = self.assign_account_list.item(i)
            if search_text:
                item_text = item.text().lower()
                item.setHidden(search_text not in item_text)
            else:
                item.setHidden(False)
    
    def on_assign_item_changed(self, item):
        pass
    
    def save_account_assignment(self):
        selected_group_items = self.group_list.selectedItems()
        if not selected_group_items:
            return
        
        group_id = selected_group_items[0].data(Qt.ItemDataRole.UserRole)
        group = self.db_manager.get_group(group_id)
        
        if not group:
            return
        
        checked_usernames = []
        unchecked_usernames = []
        
        for i in range(self.assign_account_list.count()):
            item = self.assign_account_list.item(i)
            username, media_type = item.data(Qt.ItemDataRole.UserRole)
            if item.checkState() == Qt.CheckState.Checked:
                checked_usernames.append(username)
            else:
                unchecked_usernames.append(username)
        
        for account in self.accounts:
            if account.username in checked_usernames:
                account.group_id = group_id
                self.db_manager.assign_account_to_group(account.username, group_id)
            elif account.username in unchecked_usernames and account.group_id == group_id:
                account.group_id = None
                self.db_manager.assign_account_to_group(account.username, None)
        
        self.load_groups()
        self.load_assign_accounts()
        self.update_account_list()
        QMessageBox.information(self, "Success", f"Accounts assigned to '{group['name']}'!")
    
    def rename_group(self):
        selected_items = self.group_list.selectedItems()
        if not selected_items:
            return
        
        item = selected_items[0]
        group_id = item.data(Qt.ItemDataRole.UserRole)
        group = self.db_manager.get_group(group_id)
        
        if not group:
            return
        
        new_name, ok = QInputDialog.getText(self, "Rename Group", "Enter new name:", text=group['name'])
        
        if ok and new_name.strip():
            if self.db_manager.update_group(group_id, new_name.strip()):
                self.load_groups()
                self.update_account_list()
                QMessageBox.information(self, "Success", f"Group renamed to '{new_name}'!")
            else:
                QMessageBox.warning(self, "Error", f"Group name '{new_name}' already exists.")
    
    def delete_group(self):
        selected_items = self.group_list.selectedItems()
        if not selected_items:
            return
        
        item = selected_items[0]
        group_id = item.data(Qt.ItemDataRole.UserRole)
        group = self.db_manager.get_group(group_id)
        
        if not group:
            return
        
        reply = QMessageBox.question(
            self,
            'Confirm Delete',
            f"Delete group '{group['name']}'?\nAccounts in this group will not be deleted.",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No
        )
        
        if reply == QMessageBox.StandardButton.Yes:
            self.db_manager.delete_group(group_id)
            self.assign_account_list.clear()
            self.save_assignment_btn.setEnabled(False)

            self.group_filter_combo.blockSignals(True)
            self.load_groups()
            self.group_filter_combo.setCurrentIndex(0)
            self.group_filter_combo.blockSignals(False)

            self.accounts.clear()
            self.load_all_cached_accounts()
            self.update_account_list()

            QMessageBox.information(self, "Success", f"Group '{group['name']}' deleted!")
    

    
    def on_group_filter_changed(self):
        self.filter_accounts()
        if hasattr(self, 'is_initializing') and not self.is_initializing:
            if hasattr(self, 'settings') and hasattr(self, 'group_filter_combo'):
                group_filter = self.group_filter_combo.currentData()
                if group_filter is None:
                    self.settings.setValue('group_filter', 'None')
                else:
                    self.settings.setValue('group_filter', int(group_filter))
                self.settings.sync()
    
    def filter_accounts(self):
        search_text = self.search_input.text().lower().strip()
        selected_group_id = self.group_filter_combo.currentData()

        for i in range(self.account_list.count()):
            item = self.account_list.item(i)
            account = self.accounts[i] if i < len(self.accounts) else None
            
            if not account:
                continue
            
            show_item = True
            
            if search_text:
                item_text = item.text().lower()
                if search_text not in item_text:
                    show_item = False
            
            if show_item and selected_group_id is not None:
                if selected_group_id == -1:
                    if account.group_id is not None:
                        show_item = False
                else:
                    if account.group_id != selected_group_id:
                        show_item = False
            
            item.setHidden(not show_item)
