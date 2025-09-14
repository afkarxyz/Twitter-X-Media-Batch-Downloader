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
    QLabel, QFileDialog, QListWidget, QTextEdit, QTabWidget, QAbstractItemView, QProgressBar, QCheckBox, QDialog,
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
    fetch_mode: str = 'all'  
    fetch_timestamp: str = None  

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
                 download_batch_size=25, convert_gif=False, gif_resolution='original', gif_conversion_mode='better'):
        super().__init__()
        self.accounts = accounts
        self.outpath = outpath
        self.auth_token = auth_token
        self.filename_format = filename_format
        self.download_batch_size = download_batch_size
        self.convert_gif = convert_gif
        self.gif_resolution = gif_resolution
        self.gif_conversion_mode = gif_conversion_mode
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
                    self.download_progress.emit(f"Downloading {account.username}'s {media_type_display}: {completed + failed:,}/{total:,}", progress_percent)
                
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
                self.progress.emit(f"Account {account.username} ({media_type_display}): {completed:,} downloaded, {skipped:,} skipped, {failed:,} failed", 
                                int((i + 1) / total_accounts * 100))

            if not self.is_stopped:
                if self.convert_gif:
                    try:
                        ffmpeg_exe = imageio_ffmpeg.get_ffmpeg_exe()
                        gif_items = [(item, fp) for item, fp in self.filepath_map if item.get('type') == 'animated_gif']
                        total_gifs = len(gif_items)
                        if total_gifs > 0:
                            creationflags = subprocess.CREATE_NO_WINDOW if sys.platform == 'win32' else 0
                            converted_count = 0
                            skipped_count = 0
                            self.progress.emit("Starting GIF conversion...", 0)
                            
                            for idx, (item, fp) in enumerate(gif_items, start=1):
                                if self.is_stopped:
                                    break
                                    
                                while self.is_paused:
                                    if self.is_stopped:
                                        break
                                    self.msleep(100)
                                
                                if self.is_stopped:
                                    break
                                    
                                gif_fp = fp.rsplit('.', 1)[0] + '.gif'
                                
                                if os.path.exists(gif_fp):
                                    try:
                                        os.remove(fp)
                                        skipped_count += 1
                                        conv_progress = int((idx / total_gifs) * 100)
                                        quality_display = f"({self.gif_conversion_mode}) - ({self.gif_resolution})"
                                        self.conversion_progress.emit(f"Converting GIF {idx:,}/{total_gifs:,} {quality_display} - Skipped (exists)", conv_progress)
                                        continue
                                    except Exception:
                                        pass
                                

                                if self.gif_conversion_mode == 'fast':
                                    ffmpeg_args = [ffmpeg_exe, '-i', fp, '-y', gif_fp]
                                else:
                                    if self.gif_resolution == 'high':
                                        ffmpeg_args = [ffmpeg_exe, '-i', fp, '-lavfi', 'scale=800:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=800:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a', '-r', '15', '-y', gif_fp]
                                    elif self.gif_resolution == 'medium':
                                        ffmpeg_args = [ffmpeg_exe, '-i', fp, '-lavfi', 'scale=600:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=600:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a', '-r', '10', '-y', gif_fp]
                                    elif self.gif_resolution == 'low':
                                        ffmpeg_args = [ffmpeg_exe, '-i', fp, '-lavfi', 'scale=400:-1:flags=lanczos,palettegen=stats_mode=full[palette];[0:v]scale=400:-1:flags=lanczos[scaled];[scaled][palette]paletteuse=dither=sierra2_4a', '-r', '8', '-y', gif_fp]
                                    else:
                                        ffmpeg_args = [ffmpeg_exe, '-i', fp, '-lavfi', 'palettegen=stats_mode=full[palette];[0:v][palette]paletteuse=dither=sierra2_4a', '-y', gif_fp]
                                
                                result = subprocess.run(ffmpeg_args, capture_output=True, creationflags=creationflags)
                                if result.returncode == 0 and os.path.exists(gif_fp):
                                    try:
                                        os.remove(fp)
                                        converted_count += 1
                                    except Exception:
                                        pass
                                else:
                                    pass
                                    
                                conv_progress = int((idx / total_gifs) * 100)
                                quality_display = f"({self.gif_conversion_mode}) - ({self.gif_resolution})"
                                self.conversion_progress.emit(f"Converting GIF {idx:,}/{total_gifs:,} {quality_display}", conv_progress)
                            if converted_count > 0 or skipped_count > 0:
                                completion_msg = f"GIF conversion completed: {converted_count:,} converted"
                                if skipped_count > 0:
                                    completion_msg += f", {skipped_count:,} skipped (already exists)"
                                self.progress.emit(completion_msg, 100)
                            else:
                                self.progress.emit("GIF conversion completed", 100)
                    except Exception as conv_e:
                        self.progress.emit(f"GIF conversion error: {conv_e}", 0)
                
                success_message = f"Download completed! {overall_completed:,} files downloaded"
                if overall_skipped > 0:
                    success_message += f", {overall_skipped:,} skipped"
                if overall_failed > 0:
                    success_message += f", {overall_failed:,} failed"
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
        self.setWindowTitle("Update Now")
        self.setFixedWidth(400)
        self.setModal(True)

        layout = QVBoxLayout()

        message = QLabel(f"Twitter/X Media Batch Downloader v{new_version} Available!")
        message.setWordWrap(True)
        layout.addWidget(message)

        button_box = QDialogButtonBox()
        self.update_button = QPushButton("Check")
        self.update_button.setCursor(Qt.CursorShape.PointingHandCursor)
        self.cancel_button = QPushButton("Later")
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
        self.current_version = "3.6"
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
        self.gif_conversion_mode = self.settings.value('gif_conversion_mode', 'better')
        self.gif_conversion_quality = self.settings.value('gif_conversion_quality', 'original')
        self.check_for_updates = self.settings.value('check_for_updates', True, type=bool)
        
        self.is_auto_fetching = False
        self.current_fetch_username = None
        self.current_fetch_media_type = None
        self.current_fetch_metadata = None
        self.is_multiple_user_mode = False
        
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
        except:
            return "Unknown"

    def format_fetch_info(self, timestamp_str):
        if not timestamp_str:
            return "Fetched: Unknown"
        
        try:
            fetch_time = datetime.fromisoformat(timestamp_str.replace('Z', '+00:00'))
            formatted_datetime = fetch_time.strftime("%Y/%m/%d • %H:%M")
            age = self.get_time_ago(timestamp_str)
            return f"Fetched: {formatted_datetime} ({age})"
        except:
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
        self.pause_resume_btn.setText('Pause')
        self.next_batch_btn.hide()
        self.auto_batch_btn.hide()
        self.stop_fetch_btn.hide()
        self.cancel_fetch_btn.hide()
        self.is_auto_fetching = False
        self.is_multiple_user_mode = False
        self.enable_batch_buttons()
        self.hide_account_buttons()

    def initUI(self):
        self.setWindowTitle('Twitter/X Media Batch Downloader')
        self.setMinimumWidth(670)
        self.setMinimumHeight(350)  
        
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
        self.twitter_url.setPlaceholderText("e.g. Takomayuyi or https://x.com/Takomayuyi or user1,user2,user3")
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
        self.account_list.setIconSize(QSize(48, 48))
        self.account_list.setWordWrap(True)
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
        
        dashboard_layout.addWidget(self.account_list)
        
        self.setup_account_buttons()
        dashboard_layout.addLayout(self.btn_layout)
        dashboard_tab.setLayout(dashboard_layout)
        self.tab_widget.addTab(dashboard_tab, "Dashboard")

        self.hide_account_buttons()
            
    def setup_account_buttons(self):
        self.btn_layout = QHBoxLayout()
        self.download_selected_btn = QPushButton('Download Selected')
        self.update_selected_btn = QPushButton('Update Selected')
        self.remove_btn = QPushButton('Remove Selected')
        self.clear_btn = QPushButton('Clear')
        
        for btn in [self.download_selected_btn, self.update_selected_btn, self.remove_btn, self.clear_btn]:
            btn.setMinimumWidth(120)
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
            
        self.download_selected_btn.clicked.connect(self.download_selected)
        self.update_selected_btn.clicked.connect(self.update_selected)
        self.remove_btn.clicked.connect(self.remove_selected_accounts)
        self.clear_btn.clicked.connect(self.clear_accounts)
        
        self.btn_layout.addStretch()
        for btn in [self.download_selected_btn, self.update_selected_btn, self.remove_btn, self.clear_btn]:
            self.btn_layout.addWidget(btn, 1)
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
        
        download_control_layout = QHBoxLayout()
        self.stop_btn = QPushButton('Stop')
        self.pause_resume_btn = QPushButton('Pause')
        
        self.stop_btn.setFixedWidth(120)
        self.pause_resume_btn.setFixedWidth(120)
        self.stop_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.pause_resume_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        
        self.stop_btn.clicked.connect(self.stop_download)
        self.pause_resume_btn.clicked.connect(self.toggle_pause_resume)
        
        download_control_layout.addStretch()
        download_control_layout.addWidget(self.stop_btn)
        download_control_layout.addWidget(self.pause_resume_btn)
        download_control_layout.addStretch()
        process_layout.addLayout(download_control_layout)
        
        batch_control_layout = QHBoxLayout()
        self.next_batch_btn = QPushButton('Next Batch')
        self.auto_batch_btn = QPushButton('Auto Batch')
        self.stop_fetch_btn = QPushButton('Stop Fetch')
        self.cancel_fetch_btn = QPushButton('Cancel Fetch')
        
        for btn in [self.next_batch_btn, self.auto_batch_btn, self.stop_fetch_btn, self.cancel_fetch_btn]:
            btn.setFixedWidth(120)
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
        
        self.next_batch_btn.setToolTip("Fetch next batch manually")
        self.auto_batch_btn.setToolTip("Start automatic batch fetching")
        self.stop_fetch_btn.setToolTip("Stop automatic batch fetching")
        self.cancel_fetch_btn.setToolTip("Cancel current fetch operation")
        
        self.next_batch_btn.clicked.connect(self.fetch_next_batch)
        self.auto_batch_btn.clicked.connect(self.start_auto_batch)
        self.stop_fetch_btn.clicked.connect(self.stop_auto_fetch)
        self.cancel_fetch_btn.clicked.connect(self.cancel_multiple_user_fetch)
        
        batch_control_layout.addStretch()
        batch_control_layout.addWidget(self.next_batch_btn)
        batch_control_layout.addWidget(self.auto_batch_btn)
        batch_control_layout.addWidget(self.stop_fetch_btn)
        batch_control_layout.addWidget(self.cancel_fetch_btn)
        batch_control_layout.addStretch()
        process_layout.addLayout(batch_control_layout)
        
        self.process_tab.setLayout(process_layout)
        
        self.tab_widget.addTab(self.process_tab, "Process")
        
        self.progress_bar.hide()
        self.time_label.hide()
        self.stop_btn.hide()
        self.pause_resume_btn.hide()
        self.next_batch_btn.hide()
        self.auto_batch_btn.hide()
        self.stop_fetch_btn.hide()
        self.cancel_fetch_btn.hide()

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
        self.output_browse.setFixedWidth(100)
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
        self.media_type_combo.setFixedWidth(85)
        media_types = [('all', 'All'), ('image', 'Image'), ('video', 'Video'), ('gif', 'GIF')]
        for value, display in media_types:
            self.media_type_combo.addItem(display, value)
        self.media_type_combo.currentTextChanged.connect(self.save_settings)
        first_row_layout.addWidget(media_label)
        first_row_layout.addWidget(self.media_type_combo)
        
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

        footer_label = QLabel(f"v{self.current_version} | gallery-dl v1.30.6 | September 2025")
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
        
        if hasattr(self, 'conversion_mode_combo'):
            self.settings.setValue('gif_conversion_mode', self.conversion_mode_combo.currentData())
            self.gif_conversion_mode = self.conversion_mode_combo.currentData()
            
        if hasattr(self, 'conversion_quality_combo'):
            self.settings.setValue('gif_conversion_quality', self.conversion_quality_combo.currentData())
            self.gif_conversion_quality = self.conversion_quality_combo.currentData()
        
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
            
            gif_conversion_quality = self.settings.value('gif_conversion_quality', 'original')
            if hasattr(self, 'conversion_quality_combo'):
                for i in range(self.conversion_quality_combo.count()):
                    if self.conversion_quality_combo.itemData(i) == gif_conversion_quality:
                        self.conversion_quality_combo.setCurrentIndex(i)
                        break
            
            last_username_url = self.settings.value('twitter_url', '')
            if last_username_url and hasattr(self, 'twitter_url'):
                self.twitter_url.setText(last_username_url)
                
        except Exception as e:
            pass

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
                                        media_list=timeline,
                                        fetch_mode='batch' if cached_data.get('is_batch', False) else 'all',
                                        fetch_timestamp=cached_data.get('fetch_timestamp')
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
                    self.auto_batch_btn.hide()
                    self.next_batch_btn.hide()
                    self.cancel_fetch_btn.hide()
                    self.stop_fetch_btn.show()
                    self.log_output.append('Multiple user batch mode: Auto-fetching all batches for all users...')
                else:
                    self.is_auto_fetching = True
                    self.auto_batch_btn.hide()
                    self.next_batch_btn.hide()
                    self.cancel_fetch_btn.hide()
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
                except:
                    pass

        try:
            self.reset_ui()
            
            self.is_auto_fetching = False
            self.is_multiple_user_mode = False
            self.current_fetch_username = username
            self.current_fetch_media_type = media_type
            
            self.disable_batch_buttons()
            
            self.log_output.append(f'Fetching metadata for {username}...')
            self.tab_widget.setCurrentWidget(self.process_tab)
            
            self.cancel_fetch_btn.show()
            
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
            
            self.auto_batch_btn.hide()
            self.next_batch_btn.hide()
            self.stop_fetch_btn.hide()
            self.cancel_fetch_btn.hide()
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
                except:
                    pass
        
        try:
            if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
                if self.batch_mode and not self.is_auto_fetching:
                    self.is_auto_fetching = True
                    self.auto_batch_btn.hide()
                    self.next_batch_btn.hide()
                    self.cancel_fetch_btn.hide()
                    self.stop_fetch_btn.show()
            else:
                self.is_auto_fetching = False
            
            self.current_fetch_username = username
            self.current_fetch_media_type = media_type
            
            self.disable_batch_buttons()
            
            self.metadata_worker = MetadataFetchWorker(
                username, 
                media_type, 
                batch_mode=self.batch_mode,
                batch_size=self.batch_size if self.batch_mode else 0,
                page=0
            )
            self.metadata_worker.auth_token = self.auth_token_input.text().strip()
            self.metadata_worker.finished.connect(lambda data: self.on_batch_metadata_fetched(data, username, media_type))
            self.metadata_worker.error.connect(self.on_batch_metadata_error)            
            self.metadata_worker.start()
            
        except Exception as e:            
            self.log_output.append(f'Error: Failed to start metadata fetch for {username}: {str(e)}')
            if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode and not self.is_auto_fetching:
                return
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
                
                if self.is_auto_fetching:
                    self.log_output.append(f'Batch {current_page + 1} completed for {username}. Auto-fetching batch {next_page + 1}...')
                    self.auto_batch_btn.hide()
                    self.next_batch_btn.hide()
                    self.cancel_fetch_btn.hide()
                    self.stop_fetch_btn.show()
                    self.fetch_next_batch_for_current_user(username, media_type, next_page, metadata)
                else:
                    self.log_output.append(f'Batch {current_page + 1} completed for {username}. Next batch ({next_page + 1}) available.')
                    
                    if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
                        if not self.is_auto_fetching:
                            self.is_auto_fetching = True
                        self.auto_batch_btn.hide()
                        self.next_batch_btn.hide()
                        self.cancel_fetch_btn.hide()
                        self.stop_fetch_btn.show()
                        self.log_output.append(f'Auto-fetching next batch for {username}...')
                    elif self.is_auto_fetching:
                        self.auto_batch_btn.hide()
                        self.next_batch_btn.hide()
                        self.cancel_fetch_btn.hide()
                        self.stop_fetch_btn.show()
                    
                    self.fetch_next_batch_for_current_user(username, media_type, next_page, metadata)
            else:
                if self.batch_mode:
                    total_items = len(existing_account.media_list)
                    self.log_output.append(f'All batches completed for {username}! Total: {total_items:,} media items')
                
                if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
                    if self.is_auto_fetching:
                        self.auto_batch_btn.hide()
                        self.next_batch_btn.hide()
                        self.cancel_fetch_btn.hide()
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
                self.auto_batch_btn.hide()
                self.next_batch_btn.hide()
                self.cancel_fetch_btn.hide()
                self.stop_fetch_btn.show()
            
            self.metadata_worker = MetadataFetchWorker(
                username, 
                media_type, 
                batch_mode=True,
                batch_size=self.batch_size,
                page=page
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
                self.auto_batch_btn.hide()
                self.next_batch_btn.hide()
                self.cancel_fetch_btn.hide()
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
                self.log_output.append(f'Batch {current_page + 1} completed. Next batch ({next_page + 1}) available.')
                
                if self.is_auto_fetching:
                    self.next_batch_btn.hide()
                    self.auto_batch_btn.hide()
                    self.cancel_fetch_btn.hide()
                    self.stop_fetch_btn.show()
                    self.fetch_next_batch_internal()
                else:
                    self.next_batch_btn.show()
                    self.auto_batch_btn.show()
                    self.stop_fetch_btn.hide()
                    self.cancel_fetch_btn.show()
                    self.enable_batch_buttons()
                    self.log_output.append('Use "Next Batch" for manual fetch or "Auto Batch" for automatic fetching.')
            else:
                self.next_batch_btn.hide()
                self.auto_batch_btn.hide()
                self.stop_fetch_btn.hide()
                self.cancel_fetch_btn.hide()
                self.is_auto_fetching = False
                self.enable_batch_buttons()
                
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
        
        self.next_batch_btn.hide()
        self.auto_batch_btn.hide()
        self.stop_fetch_btn.hide()
        self.cancel_fetch_btn.hide()
        
        self.enable_batch_buttons()
        
        if hasattr(self, 'accounts_to_update') and hasattr(self, 'current_update_index'):
            self.current_update_index += 1
            self.update_next_account()
        
        self.update_account_list()

    def update_account_list(self):
        self.account_list.clear()
        for i, account in enumerate(self.accounts, 1):
            media_count = len(account.media_list) if account.media_list else 0
            media_type_display = "GIF" if account.media_type.lower() == "gif" else account.media_type.title()
            
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
        
        self.update_button_states()

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

    def update_button_states(self):
        has_accounts = len(self.accounts) > 0
        
        has_selected = len(self.account_list.selectedItems()) > 0
        
        self.download_selected_btn.setEnabled(has_accounts and has_selected)
        self.update_selected_btn.setEnabled(has_accounts and has_selected)
        self.remove_btn.setEnabled(has_accounts and has_selected)
        self.clear_btn.setEnabled(has_accounts)
        
        if has_accounts:
            self.download_selected_btn.show()
            self.update_selected_btn.show()
            self.remove_btn.show()
            self.clear_btn.show()
        else:            
            self.hide_account_buttons()
    
    def hide_account_buttons(self):
        buttons = [
            self.download_selected_btn,
            self.update_selected_btn,
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

    def update_selected(self):
        selected_items = self.account_list.selectedItems()
        if not selected_items:
            self.log_output.append('Warning: Please select accounts to update.')
            return
        
        selected_accounts = []
        for item in selected_items:
            index = self.account_list.row(item)
            if index < len(self.accounts):
                selected_accounts.append(self.accounts[index])
        
        if not selected_accounts:
            return
            
        self.tab_widget.setCurrentIndex(1)  
        
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
        
        self.tab_widget.setCurrentIndex(1)  
        
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
            
        self.current_page = 0
        self.disable_batch_buttons()
        
        self.worker = MetadataFetchWorker(
            username, 
            media_type, 
            batch_mode=self.batch_mode,
            batch_size=self.batch_size if self.batch_mode else 0,
            page=0
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
            self.gif_conversion_quality,
            self.gif_conversion_mode
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
        
        self.next_batch_btn.hide()
        self.auto_batch_btn.hide()
        self.stop_fetch_btn.hide()
        self.cancel_fetch_btn.hide()
        
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
        self.update_selected_btn.setEnabled(True)
        
        if (self.batch_mode and self.current_fetch_metadata and 
            self.current_fetch_metadata.get('has_more', False) and not self.is_auto_fetching):
            self.next_batch_btn.show()
            self.auto_batch_btn.show()
            self.enable_batch_buttons()
        
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
    
    def fetch_next_batch(self):
        if not self.current_fetch_metadata or not self.current_fetch_metadata.get('has_more', False):
            self.log_output.append('No more batches available.')
            return
        
        self.disable_batch_buttons()
        self.fetch_next_batch_internal()
    
    def start_auto_batch(self):
        if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
            self.is_auto_fetching = True
            self.is_initial_fetch = False
            self.auto_batch_btn.hide()
            self.next_batch_btn.hide()
            self.cancel_fetch_btn.hide()
            self.stop_fetch_btn.show()
            
            self.log_output.append('Resuming multiple user batch processing...')
            self.fetch_next_account_in_batch()
            return
        
        if not self.current_fetch_metadata or not self.current_fetch_metadata.get('has_more', False):
            self.log_output.append('No more batches available.')
            return
        
        self.is_auto_fetching = True
        self.auto_batch_btn.hide()
        self.next_batch_btn.hide()
        self.cancel_fetch_btn.hide()
        self.stop_fetch_btn.show()
        
        self.log_output.append('Auto batch mode enabled.')
        self.fetch_next_batch_internal()
    
    def stop_auto_fetch(self):
        self.is_auto_fetching = False
        self.stop_fetch_btn.hide()
        
        if hasattr(self, 'metadata_worker') and self.metadata_worker.isRunning():
            self.metadata_worker.terminate()
            self.metadata_worker.wait()
        
        if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
            self.auto_batch_btn.show()
            self.auto_batch_btn.setEnabled(True)
            self.next_batch_btn.hide()
            self.cancel_fetch_btn.show()
            self.log_output.append('Multiple user batch processing stopped.')
        elif self.current_fetch_metadata and self.current_fetch_metadata.get('has_more', False):
            self.next_batch_btn.show()
            self.auto_batch_btn.show()
            self.cancel_fetch_btn.show()
            self.enable_batch_buttons()
            self.log_output.append('Auto batch mode stopped.')
        else:
            self.cancel_fetch_btn.show()
            self.log_output.append('Auto batch mode stopped.')
    
    def cancel_multiple_user_fetch(self):
        if hasattr(self, 'metadata_worker') and self.metadata_worker.isRunning():
            self.metadata_worker.terminate()
            self.metadata_worker.wait()
        
        if hasattr(self, 'is_multiple_user_mode') and self.is_multiple_user_mode:
            self.log_output.append('Multiple user batch processing cancelled.')
            
            if hasattr(self, 'accounts_to_fetch'):
                delattr(self, 'accounts_to_fetch')
            if hasattr(self, 'current_fetch_index'):
                delattr(self, 'current_fetch_index')
            if hasattr(self, 'is_multiple_user_mode'):
                delattr(self, 'is_multiple_user_mode')
            if hasattr(self, 'is_initial_fetch'):
                delattr(self, 'is_initial_fetch')
        else:
            self.log_output.append('Single user batch processing cancelled.')
            
            self.current_fetch_username = None
            self.current_fetch_media_type = None
            self.current_fetch_metadata = None
        
        self.auto_batch_btn.hide()
        self.next_batch_btn.hide()
        self.stop_fetch_btn.hide()
        self.cancel_fetch_btn.hide()
        self.is_auto_fetching = False
        
        self.twitter_url.clear()
        self.tab_widget.setCurrentIndex(0)
    
    def disable_batch_buttons(self):
        self.next_batch_btn.setEnabled(False)
        self.auto_batch_btn.setEnabled(False)
        
    def enable_batch_buttons(self):
        self.next_batch_btn.setEnabled(True)
        self.auto_batch_btn.setEnabled(True)

    def fetch_next_batch_internal(self):
        if not self.current_fetch_username or not self.current_fetch_media_type or not self.current_fetch_metadata:
            return
        
        self.disable_batch_buttons()
        
        current_page = self.current_fetch_metadata.get('page', 0)
        next_page = current_page + 1
        
        self.log_output.append(f'Fetching batch {next_page + 1} for {self.current_fetch_username}...')
        
        self.metadata_worker = MetadataFetchWorker(
            self.current_fetch_username, 
            self.current_fetch_media_type, 
            batch_mode=True,
            batch_size=self.batch_size,
            page=next_page
        )
        self.metadata_worker.auth_token = self.auth_token_input.text().strip()
        self.metadata_worker.finished.connect(lambda data: self.on_metadata_fetched(data, self.current_fetch_username, self.current_fetch_media_type))
        self.metadata_worker.error.connect(self.on_metadata_error)
        self.metadata_worker.start()

    def show_account_buttons(self):
        self.download_selected_btn.show()
        self.update_selected_btn.show()
        self.remove_btn.show()
        self.clear_btn.show()

def main():
    if getattr(sys, 'frozen', False):
        os.chdir(os.path.dirname(sys.executable))

    app = QApplication(sys.argv)
    app.setStyle('fusion')
    window = TwitterMediaDownloaderGUI()
    window.show()
    sys.exit(app.exec())

if __name__ == "__main__":
    main()