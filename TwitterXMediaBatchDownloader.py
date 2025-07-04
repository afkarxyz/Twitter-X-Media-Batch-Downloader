import sys
import os
import asyncio
import aiohttp
import requests
import subprocess
import imageio_ffmpeg
import json
import tempfile
from datetime import datetime
from pathlib import Path
from packaging import version
from dataclasses import dataclass
from PyQt6.QtWidgets import (
    QApplication, QWidget, QVBoxLayout, QHBoxLayout, QPushButton, QLineEdit,
    QLabel, QFileDialog, QListWidget, QTextEdit, QTabWidget, QAbstractItemView, QSpacerItem, QSizePolicy, QProgressBar, QCheckBox, QDialog,
    QDialogButtonBox, QComboBox, QListWidgetItem
)
from PyQt6.QtCore import Qt, QThread, pyqtSignal, QUrl, QTimer, QTime, QSettings, QSize
from PyQt6.QtNetwork import QNetworkAccessManager, QNetworkRequest
from PyQt6.QtGui import QIcon, QTextCursor, QDesktopServices, QPixmap, QPainter, QPainterPath
from getMetadata import get_metadata

@dataclass
class Account:
    username: str
    nick: str
    followers: int
    following: int
    posts: int
    media_type: str
    profile_image: str = None
    media_list: list = None

class MetadataFetchWorker(QThread):
    finished = pyqtSignal(dict)
    error = pyqtSignal(str)
    
    def __init__(self, username, media_type='all', batch_mode=False, batch_size=0, page=0):
        super().__init__()
        self.username = username
        self.media_type = media_type
        self.batch_mode = batch_mode
        self.batch_size = batch_size
        self.page = page
        self.auth_token = None
        
    def normalize_url(self, url_or_username):
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
        return username
        
    def run(self):
        try:
            normalized = self.normalize_url(self.username)
            data = get_metadata(
                username=normalized,
                auth_token=self.auth_token,
                timeline_type='media',
                batch_size=self.batch_size if self.batch_mode else 0,
                page=self.page,
                media_type=self.media_type
            )
            self.finished.emit(data)
        except Exception as e:
            self.error.emit(str(e))

class DownloadWorker(QThread):
    finished = pyqtSignal(bool, str)
    progress = pyqtSignal(str, int)
    conversion_progress = pyqtSignal(str, int)
    download_progress = pyqtSignal(str, int)
    
    def __init__(self, accounts, outpath, auth_token, filename_format='username_date',
                 download_batch_size=25, convert_gif=False, gif_resolution='original'):
        super().__init__()
        self.accounts = accounts
        self.outpath = outpath
        self.auth_token = auth_token
        self.filename_format = filename_format
        self.download_batch_size = download_batch_size
        self.convert_gif = convert_gif
        self.gif_resolution = gif_resolution
        self.is_paused = False
        self.is_stopped = False
        self.filepath_map = []

    async def download_file(self, session, url, filepath):
        try:
            if os.path.exists(filepath):
                return True, True
            if self.is_stopped:
                return False, False
            
            os.makedirs(os.path.dirname(filepath), exist_ok=True)
            
            async with session.get(url) as response:
                if response.status == 200:
                    with open(filepath, 'wb') as f:
                        f.write(await response.read())
                    return True, False
                return False, False
        except Exception as e:
            return False, False

    async def download_account_media(self, account):
        if not account.media_list:
            return 0, 0, 0
            
        account_output_dir = os.path.join(self.outpath, account.username)
        os.makedirs(account_output_dir, exist_ok=True)        
        timeout = aiohttp.ClientTimeout(total=30)
        connector = aiohttp.TCPConnector(limit=self.download_batch_size)
        
        async with aiohttp.ClientSession(timeout=timeout, connector=connector) as session:            
            total = len(account.media_list)
            completed = 0
            skipped = 0
            failed = 0
            
            used_filenames = set()
            
            for i in range(0, total, self.download_batch_size):
                if self.is_stopped:
                    break
                    
                while self.is_paused:
                    if self.is_stopped:
                        return completed, skipped, failed
                    await asyncio.sleep(0.1)
                
                batch = account.media_list[i:i + self.download_batch_size]
                tasks = []
                
                for item in batch:
                    url = item['url']
                    date = datetime.strptime(item['date'], "%Y-%m-%d %H:%M:%S")
                    formatted_date = date.strftime("%Y%m%d_%H%M%S")
                    tweet_id = str(item.get('tweet_id', ''))
                    
                    item_type = item.get('type', '')
                    if item_type == 'animated_gif':
                        media_type_folder = 'gif'
                        extension = 'mp4'
                    elif item_type == 'video' or 'video.twimg.com' in url:
                        media_type_folder = 'video'
                        extension = 'mp4'
                    else:
                        media_type_folder = 'image'
                        extension = 'jpg'
                    media_output_dir = os.path.join(account_output_dir, media_type_folder)
                    
                    if self.filename_format == "username_date":
                        base_filename = f"{account.username}_{formatted_date}_{tweet_id}"
                    else:
                        base_filename = f"{formatted_date}_{account.username}_{tweet_id}"
                    
                    filename = f"{base_filename}.{extension}"
                    counter = 1
                    while filename in used_filenames:
                        filename = f"{base_filename}_{counter:02d}.{extension}"
                        counter += 1
                    
                    used_filenames.add(filename)
                    filepath = os.path.join(media_output_dir, filename)
                    self.filepath_map.append((item, filepath))
                    task = asyncio.create_task(self.download_file(session, url, filepath))
                    tasks.append(task)
                results = await asyncio.gather(*tasks, return_exceptions=True)
                for result in results:
                    if isinstance(result, tuple):
                        success, was_skipped = result
                        if success:
                            completed += 1
                            if was_skipped:
                                skipped += 1
                        else:
                            failed += 1
                    else:
                        failed += 1
                    
                    progress_percent = int((completed + failed) / total * 100)
                    media_type_display = account.media_type if account.media_type != 'all' else 'media'
                    self.download_progress.emit(f"Downloading {account.username}'s {media_type_display}: {completed + failed}/{total}", progress_percent)
                
                await asyncio.sleep(0.1)
            
            return completed, skipped, failed

    def run(self):
        try:
            total_accounts = len(self.accounts)
            overall_completed = 0
            overall_skipped = 0
            overall_failed = 0
            
            for i, account in enumerate(self.accounts):
                if self.is_stopped:
                    break
                    
                while self.is_paused:
                    if self.is_stopped:
                        return
                    self.msleep(100)
                media_type_display = account.media_type if account.media_type != 'all' else 'media'
                self.progress.emit(f"Downloading from account: {account.username} ({media_type_display}) - ({i+1}/{total_accounts})", 
                                int((i) / total_accounts * 100))
                
                completed, skipped, failed = asyncio.run(self.download_account_media(account))
                overall_completed += completed
                overall_skipped += skipped
                overall_failed += failed
                self.progress.emit(f"Account {account.username} ({media_type_display}): {completed} downloaded, {skipped} skipped, {failed} failed", 
                                int((i + 1) / total_accounts * 100))

            if not self.is_stopped:
                if self.convert_gif:
                    try:
                        ffmpeg_exe = imageio_ffmpeg.get_ffmpeg_exe()
                        gif_items = [(item, fp) for item, fp in self.filepath_map if item.get('type') == 'animated_gif']
                        total_gifs = len(gif_items)
                        if total_gifs > 0:
                            creationflags = subprocess.CREATE_NO_WINDOW if sys.platform == 'win32' else 0
                            self.progress.emit("Starting GIF conversion...", 0)
                            for idx, (item, fp) in enumerate(gif_items, start=1):
                                if self.is_stopped:
                                    break
                                gif_fp = fp.rsplit('.', 1)[0] + '.gif'
                                if self.gif_resolution == 'high':
                                    ffmpeg_args = [ffmpeg_exe, '-i', fp, '-vf', 'scale=800:-1:flags=lanczos', '-r', '15', gif_fp]
                                elif self.gif_resolution == 'medium':
                                    ffmpeg_args = [ffmpeg_exe, '-i', fp, '-vf', 'scale=600:-1:flags=lanczos', '-r', '10', gif_fp]
                                elif self.gif_resolution == 'low':
                                    ffmpeg_args = [ffmpeg_exe, '-i', fp, '-vf', 'scale=400:-1:flags=lanczos', '-r', '8', gif_fp]
                                else:
                                    ffmpeg_args = [ffmpeg_exe, '-i', fp, gif_fp]
                                
                                result = subprocess.run(ffmpeg_args, capture_output=True, creationflags=creationflags)
                                if result.returncode == 0 and os.path.exists(gif_fp):
                                    try:
                                        os.remove(fp)
                                    except Exception:
                                        pass
                                conv_progress = int((idx / total_gifs) * 100)
                                self.conversion_progress.emit(f"Converting GIF {idx}/{total_gifs} ({self.gif_resolution})", conv_progress)
                            self.progress.emit("GIF conversion completed", 100)
                    except Exception as conv_e:
                        self.progress.emit(f"GIF conversion error: {conv_e}", 0)
                
                success_message = f"Download completed! {overall_completed} files downloaded"
                if overall_skipped > 0:
                    success_message += f", {overall_skipped} skipped"
                if overall_failed > 0:
                    success_message += f", {overall_failed} failed"
                self.finished.emit(True, success_message)
                
        except Exception as e:
            self.finished.emit(False, str(e))

    def pause(self):
        self.is_paused = True

    def resume(self):
        self.is_paused = False

    def stop(self): 
        self.is_stopped = True
        self.is_paused = False

class UpdateDialog(QDialog):
    def __init__(self, current_version, new_version, parent=None):
        super().__init__(parent)
        self.setWindowTitle("Update Available")
        self.setFixedWidth(400)
        self.setModal(True)

        layout = QVBoxLayout()

        message = QLabel(f"A new version of Twitter/X Media Batch Downloader is available!\n\n"
                        f"Current version: v{current_version}\n"
                        f"New version: v{new_version}")
        message.setWordWrap(True)
        layout.addWidget(message)

        self.disable_check = QCheckBox("Turn off update checking")
        self.disable_check.setCursor(Qt.CursorShape.PointingHandCursor)
        layout.addWidget(self.disable_check)

        button_box = QDialogButtonBox()
        self.update_button = QPushButton("Update")
        self.update_button.setCursor(Qt.CursorShape.PointingHandCursor)
        self.cancel_button = QPushButton("Cancel")
        self.cancel_button.setCursor(Qt.CursorShape.PointingHandCursor)
        
        button_box.addButton(self.update_button, QDialogButtonBox.ButtonRole.AcceptRole)
        button_box.addButton(self.cancel_button, QDialogButtonBox.ButtonRole.RejectRole)
        
        layout.addWidget(button_box)

        self.setLayout(layout)

        self.update_button.clicked.connect(self.accept)
        self.cancel_button.clicked.connect(self.reject)
        
class TwitterMediaDownloaderGUI(QWidget):
    def __init__(self):
        super().__init__()
        self.current_version = "3.0"
        self.accounts = []
        self.temp_dir = os.path.join(tempfile.gettempdir(), "twitterxmediabatchdownloader")
        os.makedirs(self.temp_dir, exist_ok=True)
        self.reset_state()
        
        self.settings = QSettings('TwitterMediaDownloader', 'Settings')
        self.last_output_path = self.settings.value('output_path', str(Path.home() / "Pictures"))
        self.last_url = self.settings.value('twitter_url', '')
        self.last_auth_token = self.settings.value('auth_token', '')
        self.filename_format = self.settings.value('filename_format', 'username_date')
        self.download_batch_size = self.settings.value('download_batch_size', 25, type=int)        
        self.batch_mode = self.settings.value('batch_mode', False, type=bool)
        self.batch_size = self.settings.value('batch_size', 100, type=int)
        self.timeline_type = self.settings.value('timeline_type', 'media')
        self.media_type = self.settings.value('media_type', 'all')
        self.convert_gif = self.settings.value('convert_gif', False, type=bool)
        self.gif_resolution = self.settings.value('gif_resolution', 'original')
        self.check_for_updates = self.settings.value('check_for_updates', True, type=bool)
        
        self.current_page = 0
        self.media_info = None
        
        self.profile_image_cache = {}
        self.pending_downloads = {}
        self.network_manager = QNetworkAccessManager()
        
        self.elapsed_time = QTime(0, 0, 0)
        self.timer = QTimer(self)
        self.timer.timeout.connect(self.update_timer)
        self.initUI()
        self.load_settings()
        self.load_all_cached_accounts()
        
        if self.check_for_updates:
            QTimer.singleShot(0, self.check_updates)

    def check_updates(self):
        try:
            response = requests.get("https://raw.githubusercontent.com/afkarxyz/Twitter-X-Media-Batch-Downloader/refs/heads/main/version.json")
            if response.status_code == 200:
                data = response.json()
                new_version = data.get("version")
                
                if new_version and version.parse(new_version) > version.parse(self.current_version):
                    dialog = UpdateDialog(self.current_version, new_version, self)
                    result = dialog.exec()
                    
                    if dialog.disable_check.isChecked():
                        self.settings.setValue('check_for_updates', False)
                        self.check_for_updates = False
                    
                    if result == QDialog.DialogCode.Accepted:
                        QDesktopServices.openUrl(QUrl("https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases"))
                        
        except Exception as e:
            print(f"Error checking for updates: {e}")

    def reset_state(self):
        self.accounts.clear()

    def reset_ui(self):
        self.account_list.clear()
        self.log_output.clear()
        self.progress_bar.setValue(0)
        self.progress_bar.hide()
        self.stop_btn.hide()
        self.pause_resume_btn.hide()
        self.pause_resume_btn.setText('Pause')
        self.hide_account_buttons()

    def initUI(self):
        self.setWindowTitle('Twitter/X Media Batch Downloader')
        self.setFixedWidth(650)
        self.setFixedHeight(350)
        
        icon_path = os.path.join(os.path.dirname(__file__), "icon.svg")
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
        self.twitter_url.setPlaceholderText("e.g. Takomayuyi or https://x.com/Takomayuyi")
        self.twitter_url.setClearButtonEnabled(True)
        self.twitter_url.setText(self.last_url)
        self.twitter_url.textChanged.connect(self.save_url)        
        self.fetch_btn = QPushButton('Fetch')
        self.fetch_btn.setFixedWidth(100)
        self.fetch_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.fetch_btn.clicked.connect(self.fetch_account)
        
        twitter_layout.addWidget(twitter_label)
        twitter_layout.addWidget(self.twitter_url)
        twitter_layout.addWidget(self.fetch_btn)
        self.main_layout.addLayout(twitter_layout)

    def setup_tabs(self):
        self.tab_widget = QTabWidget()
        self.main_layout.addWidget(self.tab_widget)

        self.setup_dashboard_tab()
        self.setup_process_tab()
        self.setup_settings_tab()
        self.setup_about_tab()

    def setup_dashboard_tab(self):
        dashboard_tab = QWidget()
        dashboard_layout = QVBoxLayout()

        self.account_list = QListWidget()
        self.account_list.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.account_list.itemSelectionChanged.connect(self.update_button_states)
        self.account_list.setIconSize(QSize(36, 36))
        self.account_list.setStyleSheet("""
            QListWidget {
                background-color: palette(base);
                border: 1px solid palette(mid);
                border-radius: 4px;
                padding: 0px;
                outline: none;
            }
            QListWidget::item {
                padding: 8px 12px;
                margin: 2px 0px;
                border: none;
                border-radius: 4px;
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
                background-color: palette(midlight);
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
        self.download_selected_btn = QPushButton('Download Selected')
        self.download_all_btn = QPushButton('Download All')
        self.remove_btn = QPushButton('Remove Selected')
        self.clear_btn = QPushButton('Clear')
        
        for btn in [self.download_selected_btn, self.download_all_btn, self.remove_btn, self.clear_btn]:
            btn.setFixedWidth(150)
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
            
        self.download_selected_btn.clicked.connect(self.download_selected)
        self.download_all_btn.clicked.connect(self.download_all)
        self.remove_btn.clicked.connect(self.remove_selected_accounts)
        self.clear_btn.clicked.connect(self.clear_accounts)
        
        self.btn_layout.addStretch()
        for btn in [self.download_selected_btn, self.download_all_btn, self.remove_btn, self.clear_btn]:
            self.btn_layout.addWidget(btn)
        self.btn_layout.addStretch()

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
        
        control_layout = QHBoxLayout()
        self.stop_btn = QPushButton('Stop')
        self.pause_resume_btn = QPushButton('Pause')
        
        self.stop_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.pause_resume_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        
        self.stop_btn.clicked.connect(self.stop_download)
        self.pause_resume_btn.clicked.connect(self.toggle_pause_resume)
        control_layout.addWidget(self.stop_btn)
        control_layout.addWidget(self.pause_resume_btn)
        process_layout.addLayout(control_layout)
        
        self.process_tab.setLayout(process_layout)
        
        self.tab_widget.addTab(self.process_tab, "Process")
        
        self.progress_bar.hide()
        self.time_label.hide()
        self.stop_btn.hide()
        self.pause_resume_btn.hide()

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
        self.auth_token_input.setPlaceholderText("Enter your Twitter/X auth_token")
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

        self.batch_checkbox = QCheckBox("Batch")
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
        
        first_row_layout.addSpacing(5)

        timeline_label = QLabel("Timeline Type:")
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

        media_type_label = QLabel("Media Type:")
        self.media_type_combo = QComboBox()
        self.media_type_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.media_type_combo.setFixedWidth(85)
        media_types = [('all', 'All'), ('image', 'Image'), ('video', 'Video'), ('gif', 'GIF')]
        for value, display in media_types:
            self.media_type_combo.addItem(display, value)
        self.media_type_combo.currentTextChanged.connect(self.save_settings)
        first_row_layout.addWidget(media_type_label)
        first_row_layout.addWidget(self.media_type_combo)
        
        first_row_layout.addStretch()
        gallery_dl_layout.addLayout(first_row_layout)
        
        settings_layout.addWidget(gallery_dl_group)

        download_group = QWidget()
        download_layout = QVBoxLayout(download_group)
        download_layout.setSpacing(5)
        
        download_label = QLabel('Download Settings')
        download_label.setStyleSheet("font-weight: bold;")
        download_layout.addWidget(download_label)
        
        batch_layout = QHBoxLayout()
        batch_label = QLabel('Batch Size:')
        
        self.download_batch_combo = QComboBox()
        self.download_batch_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.download_batch_combo.setFixedWidth(80)
        for size in range(5, 101, 5):
            self.download_batch_combo.addItem(str(size))
        self.download_batch_combo.setCurrentText(str(self.download_batch_size))
        self.download_batch_combo.currentTextChanged.connect(self.save_settings)
        
        self.convert_gif_checkbox = QCheckBox("Convert GIF")
        self.convert_gif_checkbox.setCursor(Qt.CursorShape.PointingHandCursor)
        self.convert_gif_checkbox.setChecked(self.convert_gif)
        self.convert_gif_checkbox.toggled.connect(self.handle_gif_checkbox)
        
        batch_layout.addWidget(batch_label)
        batch_layout.addWidget(self.download_batch_combo)
        batch_layout.addWidget(self.convert_gif_checkbox)
        
        self.gif_resolution_label = QLabel("GIF Quality:")
        self.gif_resolution_combo = QComboBox()
        self.gif_resolution_combo.setCursor(Qt.CursorShape.PointingHandCursor)
        self.gif_resolution_combo.setFixedWidth(85)
        gif_resolutions = [
            ('original', 'Original'),
            ('high', 'High'),
            ('medium', 'Medium'),
            ('low', 'Low')
        ]
        for value, display in gif_resolutions:
            self.gif_resolution_combo.addItem(display, value)
        self.gif_resolution_combo.currentTextChanged.connect(self.save_settings)
        batch_layout.addWidget(self.gif_resolution_label)
        batch_layout.addWidget(self.gif_resolution_combo)
        
        if not self.convert_gif:
            self.gif_resolution_label.hide()
            self.gif_resolution_combo.hide()
        
        batch_layout.addStretch()
        download_layout.addLayout(batch_layout)
        
        settings_layout.addWidget(download_group)
        settings_layout.addStretch()
        
        settings_tab.setLayout(settings_layout)
        self.tab_widget.addTab(settings_tab, "Settings")
        
    def setup_about_tab(self):
        about_tab = QWidget()
        about_layout = QVBoxLayout()
        about_layout.setAlignment(Qt.AlignmentFlag.AlignCenter)
        about_layout.setSpacing(3)

        sections = [
            ("Check for Updates", "https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases"),
            ("Report an Issue", "https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/issues"),
            ("gallery-dl Repository", "https://github.com/mikf/gallery-dl")
        ]

        for title, url in sections:
            section_widget = QWidget()
            section_layout = QVBoxLayout(section_widget)
            section_layout.setSpacing(10)
            section_layout.setContentsMargins(0, 0, 0, 0)

            label = QLabel(title)
            label.setStyleSheet("color: palette(text); font-weight: bold;")
            label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            section_layout.addWidget(label)

            button = QPushButton("Click Here!")
            button.setFixedWidth(150)
            button.setStyleSheet("""
                QPushButton {
                    background-color: palette(button);
                    color: palette(button-text);
                    border: 1px solid palette(mid);
                    padding: 6px;
                    border-radius: 15px;
                }
                QPushButton:hover {
                    background-color: palette(light);
                }
                QPushButton:pressed {
                    background-color: palette(midlight);
                }            
                """)
            button.setCursor(Qt.CursorShape.PointingHandCursor)
            button.clicked.connect(lambda _, url=url: QDesktopServices.openUrl(QUrl(url if url.startswith(('http://', 'https://')) else f'https://{url}')))
            section_layout.addWidget(button, alignment=Qt.AlignmentFlag.AlignCenter)            
            about_layout.addWidget(section_widget)
            if sections.index((title, url)) < len(sections) - 1:
                spacer = QSpacerItem(20, 6, QSizePolicy.Policy.Minimum, QSizePolicy.Policy.Fixed)
                about_layout.addItem(spacer)

        footer_label = QLabel("v3.0 | gallery-dl v1.30.0 | July 2025")
        footer_label.setStyleSheet("font-size: 12px; margin-top: 10px;")
        about_layout.addWidget(footer_label, alignment=Qt.AlignmentFlag.AlignCenter)
        about_tab.setLayout(about_layout)
        self.tab_widget.addTab(about_tab, "About")

    def save_url(self):
        self.settings.setValue('twitter_url', self.twitter_url.text().strip())
        self.settings.sync()

    def save_batch_size(self):
        self.batch_size = int(self.batch_size_combo.currentText())
        self.settings.setValue('batch_size', self.batch_size)
        self.settings.sync()
        
    def save_settings(self):
        self.settings.setValue('output_path', self.output_dir.text().strip())
        self.settings.setValue('auth_token', self.auth_token_input.text().strip())
        self.settings.setValue('media_type', self.media_type_combo.currentData())
        self.settings.setValue('download_batch_size', int(self.download_batch_combo.currentText()))
        
        if hasattr(self, 'convert_gif_checkbox'):
            self.settings.setValue('convert_gif', self.convert_gif_checkbox.isChecked())
            self.convert_gif = self.convert_gif_checkbox.isChecked()
        
        if hasattr(self, 'gif_resolution_combo'):
            self.settings.setValue('gif_resolution', self.gif_resolution_combo.currentData())
            self.gif_resolution = self.gif_resolution_combo.currentData()
        
        self.settings.sync()

    def load_settings(self):
        try:
            if not hasattr(self, 'batch_checkbox') or not hasattr(self, 'size_label') or not hasattr(self, 'batch_size_combo'):
                return
                
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
            
            convert_gif = self.settings.value('convert_gif', False, type=bool)
            if hasattr(self, 'convert_gif_checkbox'):
                self.convert_gif_checkbox.blockSignals(True)
                self.convert_gif_checkbox.setChecked(convert_gif)
                self.convert_gif_checkbox.blockSignals(False)
                
                if convert_gif:
                    self.gif_resolution_label.show()
                    self.gif_resolution_combo.show()
                else:
                    self.gif_resolution_label.hide()
                    self.gif_resolution_combo.hide()
            
            gif_resolution = self.settings.value('gif_resolution', 'original')
            if hasattr(self, 'gif_resolution_combo'):
                for i in range(self.gif_resolution_combo.count()):
                    if self.gif_resolution_combo.itemData(i) == gif_resolution:
                        self.gif_resolution_combo.setCurrentIndex(i)
                        break
            
            last_username_url = self.settings.value('twitter_url', '')
            if last_username_url and hasattr(self, 'twitter_url'):
                self.twitter_url.setText(last_username_url)
                
        except Exception as e:
            print(f"Error loading settings: {e}")

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

    def handle_gif_checkbox(self, checked):
        self.convert_gif = checked
        self.save_settings()
        
        if checked:
            self.gif_resolution_label.show()
            self.gif_resolution_combo.show()
        else:
            self.gif_resolution_label.hide()
            self.gif_resolution_combo.hide()

    def browse_output(self):
        directory = QFileDialog.getExistingDirectory(self, "Select Output Directory")
        if directory:
            self.output_dir.setText(directory)
            self.save_settings()

    def get_cache_file_path(self, username, media_type, is_batch=None):
        timeline_type = self.timeline_type_combo.currentData() or 'media'
        
        filename = f"{username}_{timeline_type}_{media_type}.json"
        return os.path.join(self.temp_dir, filename)

    def load_cached_data(self, username, media_type, is_batch=None):
        cache_path = self.get_cache_file_path(username, media_type, is_batch)
        if os.path.exists(cache_path):
            try:
                with open(cache_path, 'r', encoding='utf-8') as f:
                    return json.load(f)
            except:
                pass
        return None

    def save_cached_data(self, username, media_type, data, is_batch=None):
        cache_path = self.get_cache_file_path(username, media_type, is_batch)
        try:
            os.makedirs(os.path.dirname(cache_path), exist_ok=True)
            with open(cache_path, 'w', encoding='utf-8') as f:
                json.dump(data, f, ensure_ascii=False, indent=2)
        except:
            pass

    def load_all_cached_accounts(self):
        try:
            if not os.path.exists(self.temp_dir):
                return
                
            cache_files = [f for f in os.listdir(self.temp_dir) if f.endswith('.json')]
            
            if not cache_files:
                return
                
            loaded_count = 0
            
            for cache_file in cache_files:
                try:
                    filename_parts = cache_file.replace('.json', '').split('_')
                    
                    if len(filename_parts) >= 3:
                        media_type = filename_parts[-1]
                        
                        if len(filename_parts) == 4 and filename_parts[1] == 'batch':
                            username = filename_parts[0]
                            timeline_type = filename_parts[2]
                            is_batch_file = True
                        elif len(filename_parts) == 3:
                            username = filename_parts[0]
                            timeline_type = filename_parts[1]
                            is_batch_file = False
                        else:
                            username = '_'.join(filename_parts[:-2])
                            timeline_type = filename_parts[-2]
                            is_batch_file = False
                        
                        already_exists = any(
                            acc.username == username and acc.media_type == media_type 
                            for acc in self.accounts
                        )
                        if not already_exists:
                            cached_data = self.load_cached_data(username, media_type, is_batch=is_batch_file)
                            if cached_data:
                                account_info = cached_data.get('account_info', {})
                                timeline = cached_data.get('timeline', [])
                                
                                if account_info and timeline:
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
                                        media_list=timeline                                    
                                        )
                                    
                                    self.accounts.append(account)
                                    loaded_count += 1
                except Exception as e:
                    continue
                    
            if self.accounts:
                self.update_account_list()
                
        except Exception as e:
            pass

    def fetch_account(self):
        url = self.twitter_url.text().strip()
        
        if not url:
            self.log_output.append('Warning: Please enter a Twitter username/URL.')
            return

        if not self.auth_token_input.text().strip():
            self.log_output.append('Warning: Please enter your auth token.')
            return

        username = url
        if "x.com/" in url or "twitter.com/" in url:
            parts = url.split('/')
            for i, part in enumerate(parts):
                if part in ['x.com', 'twitter.com'] and i + 1 < len(parts):
                    username = parts[i + 1]
                    username = username.split('/')[0]
                    break
        username = username.strip()

        media_type = self.media_type_combo.currentData()
        
        for account in self.accounts:
            if account.username == username and account.media_type == media_type:
                self.log_output.append(f'Account {username} with {media_type} media type already in list.')
                return

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
                        media_list=timeline
                    )
                    
                    self.accounts.append(account)
                    self.update_account_list()
                    self.log_output.append(f'Loaded from cache: {username} - Followers: {followers:,} - Posts: {posts:,} • {media_type.title()}')
                    self.twitter_url.clear()
                    return
            except:
                pass

        try:
            self.reset_ui()
            
            self.log_output.append(f'Fetching metadata for {username}...')
            self.tab_widget.setCurrentWidget(self.process_tab)
            
            self.metadata_worker = MetadataFetchWorker(
                username, 
                media_type, 
                batch_mode=self.batch_mode,
                batch_size=self.batch_size if self.batch_mode else 0,
                page=0
            )
            self.metadata_worker.auth_token = self.auth_token_input.text().strip()
            self.metadata_worker.finished.connect(lambda data: self.on_metadata_fetched(data, username, media_type))
            self.metadata_worker.error.connect(self.on_metadata_error)            
            self.metadata_worker.start()
            
        except Exception as e:            
            self.log_output.append(f'Error: Failed to start metadata fetch: {str(e)}')
            self.update_account_list()
    
    def on_metadata_fetched(self, data, username, media_type):
        try:
            if 'error' in data:
                self.log_output.append(f'Error: {data["error"]}')
                self.update_account_list()
                return
                
            account_info = data.get('account_info', {})
            timeline = data.get('timeline', [])
            metadata = data.get('metadata', {})
            if not account_info or not timeline:
                self.log_output.append('Error: Invalid data received')
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
                    media_list=timeline
                )
                
                self.accounts.append(account)
                self.log_output.append(f'Successfully fetched: {username} - Followers: {followers:,} - Posts: {posts:,} • {media_type.title()}')
                existing_account = account
            
            updated_data = {
                'account_info': account_info,
                'timeline': existing_account.media_list,
                'metadata': metadata
            }
            self.save_cached_data(username, media_type, updated_data, is_batch=self.batch_mode)
            
            self.update_account_list()
            
            if self.batch_mode and metadata.get('has_more', False):
                current_page = metadata.get('page', 0)
                next_page = current_page + 1
                
                self.log_output.append(f'Auto-fetching batch {next_page + 1} for {username}...')
                
                self.metadata_worker = MetadataFetchWorker(
                    username, 
                    media_type, 
                    batch_mode=True,
                    batch_size=self.batch_size,
                    page=next_page
                )
                self.metadata_worker.auth_token = self.auth_token_input.text().strip()
                self.metadata_worker.finished.connect(lambda data: self.on_metadata_fetched(data, username, media_type))
                self.metadata_worker.error.connect(self.on_metadata_error)
                self.metadata_worker.start()
            else:
                if self.batch_mode:
                    total_items = len(existing_account.media_list)
                    self.log_output.append(f'Auto-fetch complete! Total: {total_items:,} media items for {username}')
                self.twitter_url.clear()
                self.tab_widget.setCurrentIndex(0)
        except Exception as e:
            self.log_output.append(f'Error: {str(e)}')
            self.update_account_list()
            
    def on_metadata_error(self, error_message):
        self.log_output.append(f'Error: {error_message}')
        
        self.update_account_list()

    def update_account_list(self):
        self.account_list.clear()
        for i, account in enumerate(self.accounts, 1):
            media_count = len(account.media_list) if account.media_list else 0
            media_type_display = "GIF" if account.media_type.lower() == "gif" else account.media_type.title()
            
            line1 = f"{i}. {account.username} ({account.nick})"
            line2 = f"Followers: {account.followers:,} • Following: {account.following:,} • Posts: {account.posts:,} • {media_type_display}: {media_count:,}"
            display_text = f"{line1}\n{line2}"
            item = QListWidgetItem()
            item.setText(display_text)
            item.setSizeHint(QSize(0, 52))
            
            if account.profile_image:
                if account.profile_image in self.profile_image_cache:
                    item.setIcon(QIcon(self.profile_image_cache[account.profile_image]))
                else:
                    self.download_profile_image(account.profile_image)
                    placeholder = self.create_placeholder_icon(52)
                    if placeholder:
                        item.setIcon(QIcon(placeholder))
            else:
                placeholder = self.create_placeholder_icon(52)
                if placeholder:
                    item.setIcon(QIcon(placeholder))
            
            self.account_list.addItem(item)
        
        self.update_button_states()

    def create_placeholder_icon(self, size=36):
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

    def create_square_pixmap(self, original_pixmap, size=36):
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
            print(f"Error downloading profile image: {e}")

    def on_profile_image_downloaded(self, reply, image_url):
        try:
            if reply.error() == reply.NetworkError.NoError:
                data = reply.readAll()
                pixmap = QPixmap()
                if pixmap.loadFromData(data):
                    square_pixmap = self.create_square_pixmap(pixmap, 52)
                    self.profile_image_cache[image_url] = square_pixmap
                    
                    self.update_account_list()
            
            if image_url in self.pending_downloads:
                del self.pending_downloads[image_url]
                
        except Exception as e:
            print(f"Error processing profile image: {e}")
        finally:
            reply.deleteLater()

    def update_button_states(self):
        has_accounts = len(self.accounts) > 0
        
        self.download_selected_btn.setEnabled(has_accounts)
        self.download_all_btn.setEnabled(has_accounts)
        self.remove_btn.setEnabled(has_accounts)
        self.clear_btn.setEnabled(has_accounts)
        
        if has_accounts:
            self.download_selected_btn.show()
            self.download_all_btn.show()
            self.remove_btn.show()
            self.clear_btn.show()
        else:            
            self.hide_account_buttons()
    
    def hide_account_buttons(self):
        buttons = [
            self.download_selected_btn,
            self.download_all_btn,
            self.remove_btn,
            self.clear_btn
        ]
        for btn in buttons:
            btn.hide()

    def download_selected(self):
        selected_items = self.account_list.selectedItems()
        if not selected_items:
            self.log_output.append('Warning: Please select accounts to download.')
            return
        self.download_accounts([self.account_list.row(item) for item in selected_items])

    def download_all(self):
        self.download_accounts(range(len(self.accounts)))

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
            self.gif_resolution
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
        self.download_all_btn.setEnabled(False)
        self.stop_btn.show()
        self.pause_resume_btn.show()
        
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
        self.pause_resume_btn.setText('Pause')
        self.stop_timer()
        
        self.download_selected_btn.setEnabled(True)
        self.download_all_btn.setEnabled(True)
        
        if success:
            self.log_output.append(f"\nStatus: {message}")
        else:
            self.log_output.append(f"Error: {message}")

        self.tab_widget.setCurrentWidget(self.process_tab)
    
    def toggle_pause_resume(self):
        if hasattr(self, 'worker'):
            if self.worker.is_paused:
                self.worker.resume()
                self.pause_resume_btn.setText('Pause')
                self.timer.start(1000)
            else:
                self.worker.pause()
                self.pause_resume_btn.setText('Resume')

    def remove_selected_accounts(self):
        selected_indices = sorted([self.account_list.row(item) for item in self.account_list.selectedItems()], reverse=True)
        
        if not selected_indices:
            return
        
        for index in selected_indices:
            account = self.accounts[index]
            username = account.username
            if username:
                for is_batch in [True, False]:
                    cache_file = self.get_cache_file_path(username, account.media_type, is_batch=is_batch)
                    try:
                        if os.path.exists(cache_file):
                            os.remove(cache_file)
                            self.log_output.append(f'Removed temp file: {os.path.basename(cache_file)}')
                    except Exception as e:
                        self.log_output.append(f'Warning: Could not remove temp file {os.path.basename(cache_file)}: {str(e)}')
            
            self.accounts.pop(index)
        
        self.update_account_list()
        self.update_button_states()

    def clear_accounts(self):
        for account in self.accounts:
            username = account.username
            if username:
                for is_batch in [True, False]:
                    cache_file = self.get_cache_file_path(username, account.media_type, is_batch=is_batch)
                    try:
                        if os.path.exists(cache_file):
                            os.remove(cache_file)
                            self.log_output.append(f'Removed temp file: {os.path.basename(cache_file)}')
                    except Exception as e:
                        self.log_output.append(f'Warning: Could not remove temp file {os.path.basename(cache_file)}: {str(e)}')
        
        self.reset_state()
        self.reset_ui()
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

    def show_account_buttons(self):
        self.download_selected_btn.show()
        self.download_all_btn.show()
        self.remove_btn.show()
        self.clear_btn.show()

def main():
    if getattr(sys, 'frozen', False):
        os.chdir(os.path.dirname(sys.executable))

    app = QApplication(sys.argv)
    window = TwitterMediaDownloaderGUI()
    window.show()
    sys.exit(app.exec())

if __name__ == "__main__":
    main()