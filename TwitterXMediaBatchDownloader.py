import sys
import os
import asyncio
import aiohttp
import requests
import re
import subprocess
from datetime import datetime
from pathlib import Path
from packaging import version
from PyQt6.QtWidgets import (QApplication, QMainWindow, QWidget, QVBoxLayout, 
                        QHBoxLayout, QLabel, QLineEdit, QPushButton, 
                        QProgressBar, QFileDialog, QRadioButton, QComboBox,
                        QCheckBox, QDialog, QDialogButtonBox, QTabWidget)
from PyQt6.QtCore import QThread, pyqtSignal, Qt, QSettings, QTimer, QUrl
from PyQt6.QtGui import QIcon, QPixmap, QCursor, QPainter, QPainterPath, QDesktopServices
from gallery_dl.extractor import twitter

class ImageDownloader(QThread):
    finished = pyqtSignal(bytes)

    def __init__(self, url):
        super().__init__()
        self.url = url
        
    async def download_image(self):
        async with aiohttp.ClientSession() as session:
            async with session.get(self.url) as response:
                if response.status == 200:
                    return await response.read()
        return None
        
    def run(self):
        image_data = asyncio.run(self.download_image())
        if image_data:
            self.finished.emit(image_data)

class MetadataFetcher(QThread):
    finished = pyqtSignal(dict)
    error = pyqtSignal(str)

    def __init__(self, username, timeline_type='media', media_type='all', use_api=False, batch_mode=False, batch_size=100, page=0):
        super().__init__()
        self.username = username
        self.auth_token = None
        self.timeline_type = timeline_type
        self.media_type = media_type
        self.use_api = use_api
        self.batch_mode = batch_mode
        self.batch_size = batch_size
        self.page = page
        
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

    async def fetch_from_api(self, username, auth_token):
        try:
            async with aiohttp.ClientSession() as session:
                if self.batch_mode:
                    url = f"https://gallerydl.vercel.app/metadata/{self.timeline_type}/{self.batch_size}/{self.page}/{self.media_type}/{username}/{auth_token}"
                else:
                    url = f"https://gallerydl.vercel.app/metadata/{self.timeline_type}/{self.media_type}/{username}/{auth_token}"
                
                async with session.get(url) as response:
                    if response.status == 200:
                        data = await response.json()
                        return data
                    else:
                        raise ValueError(f"API request failed with status {response.status}")
        except Exception as e:
            raise ValueError(f"API request failed: {str(e)}")

    async def fetch_local(self, normalized):
        try:
            if self.timeline_type == "media":
                extractor_class = twitter.TwitterMediaExtractor
            elif self.timeline_type == "tweets":
                extractor_class = twitter.TwitterTweetsExtractor
            elif self.timeline_type == "with_replies":
                extractor_class = twitter.TwitterRepliesExtractor
            else:
                extractor_class = twitter.TwitterTimelineExtractor
                
            match = re.match(extractor_class.pattern, f"https://x.com/{normalized}/{self.timeline_type}")
            if not match:
                raise ValueError(f"Invalid username: {normalized}")
                
            extractor = extractor_class(match)
            extractor.config = lambda key, default=None: {
                "cookies": {
                    "auth_token": self.auth_token
                }
            }.get(key, default)
            
            extractor.initialize()
            
            output = {
                'account_info': {},
                'timeline': [],
                'metadata': {
                    'batch_size': self.batch_size if self.batch_mode else 0,
                    'page': self.page if self.batch_mode else 0,
                    'has_more': False,
                    'new_entries': 0,
                    'cursor': None
                }
            }
            
            items_to_fetch = self.batch_size if self.batch_mode else float('inf')
            items_fetched = 0
            
            for item in extractor:
                if isinstance(item, tuple) and len(item) >= 3:
                    media_url = item[1]
                    tweet_data = item[2]
                    
                    if not output['account_info']:
                        if 'user' in tweet_data:
                            user = tweet_data['user']
                            user_date = user.get('date', '')
                            if isinstance(user_date, datetime):
                                user_date = user_date.strftime("%Y-%m-%d %H:%M:%S")
                                
                            output['account_info'] = {
                                'name': user.get('name', ''),
                                'nick': user.get('nick', ''),
                                'date': user_date,
                                'followers_count': user.get('followers_count', 0),
                                'friends_count': user.get('friends_count', 0),
                                'profile_image': user.get('profile_image', ''),
                                'statuses_count': user.get('statuses_count', 0)
                            }
                    
                    should_include = False
                    media_type = tweet_data.get('type', '')
                    
                    if self.media_type == 'all':
                        should_include = True
                    elif self.media_type == 'image' and media_type == 'photo':
                        should_include = True
                    elif self.media_type == 'video' and media_type == 'video':
                        should_include = True
                    elif self.media_type == 'gif' and media_type == 'animated_gif':
                        should_include = True
                    
                    if should_include:
                        tweet_date = tweet_data.get('date', datetime.now())
                        if isinstance(tweet_date, datetime):
                            tweet_date = tweet_date.strftime("%Y-%m-%d %H:%M:%S")
                        
                        output['timeline'].append({
                            'url': media_url,
                            'date': tweet_date,
                            'type': media_type,
                            'tweet_id': tweet_data.get('tweet_id', '')
                        })
                        
                        items_fetched += 1
                        if self.batch_mode and items_fetched >= items_to_fetch:
                            output['metadata']['has_more'] = True
                            break
            
            output['metadata']['new_entries'] = len(output['timeline'])
            output['total_urls'] = len(output['timeline'])
            
            if not output['account_info']:
                raise ValueError("Failed to fetch account information. Please check the username and auth token.")
            
            if not output['timeline']:
                message = f"No {self.media_type} found in {self.timeline_type}"
                raise ValueError(message)
            
            return output
            
        except Exception as e:
            raise ValueError(f"Local gallery-dl error: {str(e)}")

    def run(self):
        try:
            normalized = self.normalize_url(self.username)
            
            if self.use_api:
                try:
                    data = asyncio.run(self.fetch_from_api(normalized, self.auth_token))
                    self.finished.emit(data)
                    return
                except Exception as api_error:
                    pass
            
            try:
                data = asyncio.run(self.fetch_local(normalized))
                self.finished.emit(data)
            except Exception as local_error:
                if self.use_api:
                    raise ValueError(f"Both API and local methods failed.\nAPI error: {str(api_error)}\nLocal error: {str(local_error)}")
                else:
                    raise ValueError(f"Local gallery-dl failed: {str(local_error)}")
                    
        except Exception as e:
            self.error.emit(str(e))

class MediaDownloader(QThread):
    progress = pyqtSignal(int)
    finished = pyqtSignal(str)
    error = pyqtSignal(str)
    status_update = pyqtSignal(str)

    def __init__(self, media_list, output_dir, filename_format, username, media_type='all', batch_size=10):
        super().__init__()
        self.media_list = media_list
        self.output_dir = output_dir
        self.username = username
        self.filename_format = filename_format
        self.media_type = media_type
        self.batch_size = batch_size
        self._is_cancelled = False
        self._download_lock = asyncio.Lock()

    def cancel(self):
        self._is_cancelled = True

    async def download_file(self, session, url, filepath):
        try:
            if os.path.exists(filepath):
                return True, True
            
            if self._is_cancelled:
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

    async def download_all(self):
        os.makedirs(self.output_dir, exist_ok=True)
        
        timeout = aiohttp.ClientTimeout(total=30)
        connector = aiohttp.TCPConnector(limit=self.batch_size)
        
        async with aiohttp.ClientSession(timeout=timeout, connector=connector) as session:
            total = len(self.media_list)
            completed = 0
            skipped = 0
            failed = 0
            
            used_filenames = set()
            
            for i in range(0, total, self.batch_size):
                if self._is_cancelled:
                    break
                
                batch = self.media_list[i:i + self.batch_size]
                tasks = []
                
                for item in batch:
                    url = item['url']
                    date = datetime.strptime(item['date'], "%Y-%m-%d %H:%M:%S")
                    formatted_date = date.strftime("%Y%m%d_%H%M%S")
                    tweet_id = str(item.get('tweet_id', ''))
                    
                    extension = 'mp4' if 'video.twimg.com' in url else 'jpg'
                    
                    if self.filename_format == "username_date":
                        base_filename = f"{self.username}_{formatted_date}_{tweet_id}"
                    else:
                        base_filename = f"{formatted_date}_{self.username}_{tweet_id}"
                    
                    filename = f"{base_filename}.{extension}"
                    counter = 1
                    while filename in used_filenames:
                        filename = f"{base_filename}_{counter:02d}.{extension}"
                        counter += 1
                    
                    used_filenames.add(filename)
                    filepath = os.path.join(self.output_dir, filename)
                    
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
                
                progress = int((completed / total) * 100)
                self.progress.emit(progress)
                
                completed_str = f"{completed:,}" if completed >= 1000 else str(completed)
                total_str = f"{total:,}" if total >= 1000 else str(total)
                skipped_str = f" ({skipped:,} skipped)" if skipped > 0 else ""
                failed_str = f" ({failed:,} failed)" if failed > 0 else ""
                
                media_type_str = "files"
                if self.media_type == "image":
                    media_type_str = "images"
                elif self.media_type == "video":
                    media_type_str = "videos"
                elif self.media_type == "gif":
                    media_type_str = "GIFs"
                
                self.status_update.emit(
                    f"Downloaded {completed_str} of {total_str} {media_type_str}{skipped_str}{failed_str}..."
                )
                
                await asyncio.sleep(0.1)
            
            return completed, skipped, failed

    def run(self):
        try:
            completed, skipped, failed = asyncio.run(self.download_all())
            
            if self._is_cancelled:
                self.status_update.emit("Download cancelled")
                return
                
            completed_str = f"{completed:,}" if completed >= 1000 else str(completed)
            skipped_str = f" ({skipped:,} skipped)" if skipped > 0 else ""
            failed_str = f" ({failed:,} failed)" if failed > 0 else ""
            
            media_type_str = "files"
            if self.media_type == "image":
                media_type_str = "images"
            elif self.media_type == "video":
                media_type_str = "videos"
            elif self.media_type == "gif":
                media_type_str = "GIFs"
            
            self.finished.emit(
                f"Downloaded {completed_str} {media_type_str}{skipped_str}{failed_str} successfully!"
            )
        except Exception as e:
            self.error.emit(str(e))

class UpdateDialog(QDialog):
    def __init__(self, current_version, new_version, parent=None):
        super().__init__(parent)
        self.setWindowTitle("Update Available")
        self.setFixedWidth(400)
        self.setModal(True)

        layout = QVBoxLayout()

        message = QLabel(f"A new version of Twitter/X Media Batch Downloader is available!\n\nCurrent version: v{current_version}\nNew version: v{new_version}")
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
    
class TwitterMediaDownloaderGUI(QMainWindow):
    def __init__(self):
        super().__init__()
        self.current_version = "1.8" 
        self.setWindowTitle("Twitter/X Media Batch Downloader")
        
        self.settings = QSettings('TwitterMediaDownloader', 'Settings')
        self.settings.setDefaultFormat(QSettings.Format.IniFormat)
        
        self.config_dir = os.path.join(os.path.expanduser("~"), ".twitter_media_downloader")
        os.makedirs(self.config_dir, exist_ok=True)
        
        self.check_for_updates = self.settings.value('check_for_updates', True, type=bool)
        
        icon_path = os.path.join(os.path.dirname(__file__), "icon.svg")
        if os.path.exists(icon_path):
            self.setWindowIcon(QIcon(icon_path))
                
        self.setFixedWidth(600)
        
        self.default_pictures_dir = str(Path.home() / "Pictures")
        os.makedirs(self.default_pictures_dir, exist_ok=True)
        
        self.media_info = None
        self.init_ui()
        self.load_settings()
        self.setup_auto_save()
        self.clean_username = None
        self.current_page = 0
        
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
            pass

    def init_ui(self):
        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        self.main_layout = QVBoxLayout(central_widget)
        self.main_layout.setContentsMargins(10, 10, 10, 10)

        self.tab_widget = QTabWidget()
        self.main_tab = QWidget()
        self.settings_tab = QWidget()
        
        self.main_tab_layout = QVBoxLayout(self.main_tab)
        self.settings_tab_layout = QVBoxLayout(self.settings_tab)
        
        self.tab_widget.addTab(self.main_tab, "Main")
        self.tab_widget.addTab(self.settings_tab, "Settings")
        
        self.main_layout.addWidget(self.tab_widget)

        self.input_widget = QWidget()
        input_layout = QVBoxLayout(self.input_widget)
        input_layout.setSpacing(10)

        url_layout = QHBoxLayout()
        url_label = QLabel("Username/URL:")
        url_label.setFixedWidth(100)
        
        self.url_input = QLineEdit()
        self.url_input.setPlaceholderText("e.g. Takomayuyi or https://x.com/Takomayuyi")
        self.url_input.setClearButtonEnabled(True)
        
        self.fetch_button = QPushButton("Fetch")
        self.fetch_button.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.fetch_button.setFixedWidth(100)
        self.fetch_button.clicked.connect(self.fetch_metadata)
        
        url_layout.addWidget(url_label)
        url_layout.addWidget(self.url_input)
        url_layout.addWidget(self.fetch_button)
        input_layout.addLayout(url_layout)

        dir_layout = QHBoxLayout()
        dir_label = QLabel("Output Directory:")
        dir_label.setFixedWidth(100)
        
        self.dir_input = QLineEdit(self.default_pictures_dir)
        
        self.dir_button = QPushButton("Browse")
        self.dir_button.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.dir_button.setFixedWidth(100)
        self.dir_button.clicked.connect(self.select_directory)
        
        dir_layout.addWidget(dir_label)
        dir_layout.addWidget(self.dir_input)
        dir_layout.addWidget(self.dir_button)
        input_layout.addLayout(dir_layout)

        auth_layout = QHBoxLayout()
        auth_label = QLabel("Auth Token:")
        auth_label.setFixedWidth(100)
        
        self.auth_input = QLineEdit()
        self.auth_input.setPlaceholderText("Enter your Twitter/X auth_token")
        self.auth_input.setClearButtonEnabled(True)
        
        auth_layout.addWidget(auth_label)
        auth_layout.addWidget(self.auth_input)
        input_layout.addLayout(auth_layout)

        self.main_tab_layout.addWidget(self.input_widget)

        gallery_dl_settings_label = QLabel("<b>gallery-dl Settings</b>")
        self.settings_tab_layout.addWidget(gallery_dl_settings_label)
        
        first_row_layout = QHBoxLayout()
        first_row_layout.setSpacing(10)

        self.use_api_checkbox = QCheckBox("Use API")
        self.use_api_checkbox.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.use_api_checkbox.setToolTip("Use API instead of local gallery-dl")
        first_row_layout.addWidget(self.use_api_checkbox)
        
        self.batch_checkbox = QCheckBox("Batch")
        self.batch_checkbox.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.batch_checkbox.stateChanged.connect(self.handle_batch_checkbox)
        first_row_layout.addWidget(self.batch_checkbox)
        
        first_row_layout.addSpacing(10)

        timeline_label = QLabel("Timeline Type:")
        self.timeline_type_combo = QComboBox()
        self.timeline_type_combo.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.timeline_type_combo.setFixedWidth(100)
        timeline_types = [
            ('media', 'Media'), 
            ('timeline', 'Post'), 
            ('tweets', 'Tweets'), 
            ('with_replies', 'Replies')
        ]
        for value, display in timeline_types:
            self.timeline_type_combo.addItem(display, value)
        first_row_layout.addWidget(timeline_label)
        first_row_layout.addWidget(self.timeline_type_combo)
        
        first_row_layout.addSpacing(20)

        media_type_label = QLabel("Item Type:")
        self.media_type_combo = QComboBox()
        self.media_type_combo.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.media_type_combo.setFixedWidth(100)
        media_types = [
            ('all', 'All Media'), 
            ('image', 'Image'), 
            ('video', 'Video'), 
            ('gif', 'GIF')
        ]
        for value, display in media_types:
            self.media_type_combo.addItem(display, value)
        first_row_layout.addWidget(media_type_label)
        first_row_layout.addWidget(self.media_type_combo)
        
        first_row_layout.addStretch()
        self.settings_tab_layout.addLayout(first_row_layout)

        download_settings_label = QLabel("<b>Download Settings</b>")
        self.settings_tab_layout.addWidget(download_settings_label)
        
        batch_download_item_layout = QHBoxLayout()
        batch_download_label = QLabel("Batch Download Items:")
        self.download_batch_combo = QComboBox()
        self.download_batch_combo.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.download_batch_combo.setFixedWidth(80)
        for size in range(5, 101, 5):
            self.download_batch_combo.addItem(str(size))
        index = self.download_batch_combo.findText("25")
        if index >= 0:
            self.download_batch_combo.setCurrentIndex(index)
        batch_download_item_layout.addWidget(batch_download_label)
        batch_download_item_layout.addWidget(self.download_batch_combo)
        batch_download_item_layout.addStretch()
        self.settings_tab_layout.addLayout(batch_download_item_layout)
        
        filename_layout = QHBoxLayout()
        filename_label = QLabel("Filename Format:")
        self.format_username = QRadioButton("Username - Post Date")
        self.format_date = QRadioButton("Post Date - Username")
        self.format_username.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.format_date.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        filename_layout.addWidget(filename_label)
        filename_layout.addWidget(self.format_username)
        filename_layout.addWidget(self.format_date)
        filename_layout.addStretch()
        self.settings_tab_layout.addLayout(filename_layout)
        
        self.settings_tab_layout.addStretch()

        self.profile_widget = QWidget()
        self.profile_widget.hide()
        profile_layout = QHBoxLayout(self.profile_widget)
        profile_layout.setContentsMargins(0, 0, 0, 0)
        profile_layout.setSpacing(10)

        profile_container = QWidget()
        profile_image_layout = QVBoxLayout(profile_container)
        profile_image_layout.setContentsMargins(0, 0, 0, 0)
        profile_image_layout.setAlignment(Qt.AlignmentFlag.AlignTop)
        
        self.profile_image_label = QLabel()
        self.profile_image_label.setFixedSize(100, 100)
        self.profile_image_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        profile_image_layout.addWidget(self.profile_image_label)
        profile_layout.addWidget(profile_container)

        profile_details_container = QWidget()
        profile_details_layout = QVBoxLayout(profile_details_container)
        profile_details_layout.setContentsMargins(0, 0, 0, 0)
        profile_details_layout.setSpacing(2)
        profile_details_layout.setAlignment(Qt.AlignmentFlag.AlignTop)

        self.name_label = QLabel()
        self.name_label.setStyleSheet("font-size: 14px;")
        self.name_label.setWordWrap(True)
        self.name_label.setMinimumWidth(400)
        
        self.join_date_label = QLabel()
        self.join_date_label.setStyleSheet("font-size: 12px;")
        self.join_date_label.setWordWrap(True)
        self.join_date_label.setMinimumWidth(400)
        
        self.followers_label = QLabel()
        self.followers_label.setStyleSheet("font-size: 12px;")
        self.followers_label.setWordWrap(True)
        self.followers_label.setMinimumWidth(400)

        self.following_label = QLabel()
        self.following_label.setStyleSheet("font-size: 12px;")
        self.following_label.setWordWrap(True)
        self.following_label.setMinimumWidth(400)

        self.posts_label = QLabel()
        self.posts_label.setStyleSheet("font-size: 12px;")
        self.posts_label.setWordWrap(True)
        self.posts_label.setMinimumWidth(400)

        profile_details_layout.addWidget(self.name_label)
        profile_details_layout.addWidget(self.join_date_label)
        profile_details_layout.addWidget(self.followers_label)
        profile_details_layout.addWidget(self.following_label)
        profile_details_layout.addWidget(self.posts_label)
        profile_layout.addWidget(profile_details_container, stretch=1)
        profile_layout.addStretch()

        self.main_tab_layout.addWidget(self.profile_widget)

        self.next_batch_button = QPushButton("Next Batch")
        self.next_batch_button.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.next_batch_button.setFixedWidth(100)
        self.next_batch_button.clicked.connect(self.fetch_next_batch)
        self.next_batch_button.hide()

        self.download_button = QPushButton("Download")
        self.download_button.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.download_button.setFixedWidth(100)
        self.download_button.clicked.connect(self.start_download)
        self.download_button.hide()

        self.cancel_button = QPushButton("Cancel")
        self.cancel_button.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.cancel_button.setFixedWidth(100)
        self.cancel_button.clicked.connect(self.cancel_clicked)
        self.cancel_button.hide()

        self.open_button = QPushButton("Open")
        self.open_button.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.open_button.setFixedWidth(100)
        self.open_button.clicked.connect(self.open_output_directory)
        self.open_button.hide()

        button_layout = QHBoxLayout()
        button_layout.addStretch()
        button_layout.addWidget(self.open_button)
        button_layout.addWidget(self.next_batch_button)
        button_layout.addWidget(self.download_button)
        button_layout.addWidget(self.cancel_button)
        button_layout.addStretch()
        self.main_tab_layout.addLayout(button_layout)

        self.progress_bar = QProgressBar()
        self.progress_bar.hide()
        self.main_tab_layout.addWidget(self.progress_bar)

        bottom_layout = QHBoxLayout()
        
        self.status_label = QLabel("")
        bottom_layout.addWidget(self.status_label, stretch=1)
        
        self.update_button = QPushButton()
        icon_path = os.path.join(os.path.dirname(__file__), "update.svg")
        if os.path.exists(icon_path):
            self.update_button.setIcon(QIcon(icon_path))
        self.update_button.setFixedSize(16, 16)
        self.update_button.setStyleSheet("""
            QPushButton {
                border: none;
                background: transparent;
            }
            QPushButton:hover {
                background: transparent;
            }
        """)
        self.update_button.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.update_button.setToolTip("Check for Updates")
        self.update_button.clicked.connect(self.open_update_page)
        
        bottom_layout.addWidget(self.update_button)
        
        self.main_layout.addLayout(bottom_layout)

    def handle_batch_checkbox(self, state):
        if hasattr(self, 'media_info') and self.media_info:
            if state == Qt.CheckState.Checked and self.media_info.get('metadata', {}).get('has_more', False):
                self.next_batch_button.show()
            else:
                self.next_batch_button.hide()

    def open_update_page(self):
        import webbrowser
        webbrowser.open('https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases')
    
    def setup_auto_save(self):
        self.url_input.textChanged.connect(self.auto_save_settings)
        self.auth_input.textChanged.connect(self.auto_save_settings)
        self.dir_input.textChanged.connect(self.auto_save_settings)
        self.format_username.toggled.connect(self.auto_save_settings)
        self.media_type_combo.currentTextChanged.connect(self.auto_save_settings)
        self.timeline_type_combo.currentTextChanged.connect(self.auto_save_settings)
        self.use_api_checkbox.stateChanged.connect(self.auto_save_settings)
        self.batch_checkbox.stateChanged.connect(self.auto_save_settings)
        self.download_batch_combo.currentTextChanged.connect(self.auto_save_settings)
    
    def auto_save_settings(self):
        try:
            self.settings.setValue('url_input', self.url_input.text())
            self.settings.setValue('auth_token', self.auth_input.text())
            self.settings.setValue('output_dir', self.dir_input.text())
            self.settings.setValue('filename_format', 
                                'username_date' if self.format_username.isChecked() else 'date_username')
            self.settings.setValue('media_type', self.media_type_combo.currentData())
            self.settings.setValue('timeline_type', self.timeline_type_combo.currentData())
            self.settings.setValue('use_api', self.use_api_checkbox.isChecked())
            
            batch_mode = self.batch_checkbox.isChecked()
            self.settings.setValue('batch_mode', batch_mode)
            
            self.settings.setValue('download_batch_size', self.download_batch_combo.currentText())
            self.settings.sync()
        except Exception as e:
            pass

    def load_settings(self):
        try:
            self.url_input.setText(self.settings.value('url_input', '', str))
            self.auth_input.setText(self.settings.value('auth_token', '', str))
            self.dir_input.setText(self.settings.value('output_dir', self.default_pictures_dir, str))
            
            format_setting = self.settings.value('filename_format', 'username_date')
            self.format_username.setChecked(format_setting == 'username_date')
            self.format_date.setChecked(format_setting == 'date_username')
            
            media_type = self.settings.value('media_type', 'all')
            for i in range(self.media_type_combo.count()):
                if self.media_type_combo.itemData(i) == media_type:
                    self.media_type_combo.setCurrentIndex(i)
                    break
                    
            timeline_type = self.settings.value('timeline_type', 'media')
            for i in range(self.timeline_type_combo.count()):
                if self.timeline_type_combo.itemData(i) == timeline_type:
                    self.timeline_type_combo.setCurrentIndex(i)
                    break

            use_api = self.settings.value('use_api', False, type=bool)
            self.use_api_checkbox.setChecked(use_api)
            
            batch_mode = self.settings.value('batch_mode', False, type=bool)
            
            self.batch_checkbox.blockSignals(True)
            self.batch_checkbox.setChecked(batch_mode)
            self.batch_checkbox.blockSignals(False)
            
            download_batch_size = str(self.settings.value('download_batch_size', '25'))
            index = self.download_batch_combo.findText(download_batch_size)
            if index >= 0:
                self.download_batch_combo.setCurrentIndex(index)
            else:
                index = self.download_batch_combo.findText("25")
                if index >= 0:
                    self.download_batch_combo.setCurrentIndex(index)
                else:
                    self.download_batch_combo.setCurrentIndex(0)
        except Exception as e:
            pass

    def select_directory(self):
        directory = QFileDialog.getExistingDirectory(self, "Select Output Directory")
        if directory:
            os.makedirs(directory, exist_ok=True)
            self.dir_input.setText(directory)

    def open_output_directory(self):
        output_dir = self.dir_input.text().strip() or self.default_pictures_dir
        if os.path.exists(output_dir):
            if sys.platform == 'win32':
                os.startfile(output_dir)
            elif sys.platform == 'darwin':
                subprocess.run(['open', output_dir])
            else:
                subprocess.run(['xdg-open', output_dir])

    def fetch_metadata(self):
        self.current_page = 0
        username = self.url_input.text().strip()
        if not username:
            self.status_label.setText("Please enter a username or URL")
            return

        auth_token = self.auth_input.text().strip()
        if not auth_token:
            self.status_label.setText("Please enter your auth token")
            return

        self.fetch_button.setEnabled(False)
        self.status_label.setText("Fetching profile information...")
        
        timeline_type = self.timeline_type_combo.currentData()
        media_type = self.media_type_combo.currentData()
        use_api = self.use_api_checkbox.isChecked()
        batch_mode = self.batch_checkbox.isChecked()
        batch_size = 100
        
        self.fetcher = MetadataFetcher(
            username, 
            timeline_type, 
            media_type, 
            use_api, 
            batch_mode, 
            batch_size, 
            self.current_page
        )
        self.fetcher.auth_token = auth_token
        self.fetcher.finished.connect(self.handle_profile_info)
        self.fetcher.error.connect(self.handle_fetch_error)
        self.fetcher.start()

    def fetch_next_batch(self):
        if not self.media_info:
            self.status_label.setText("No profile information available")
            return
            
        username = self.url_input.text().strip()
        auth_token = self.auth_input.text().strip()
        
        if not username or not auth_token:
            self.status_label.setText("Username or auth token missing")
            return
            
        self.current_page += 1
        self.next_batch_button.setEnabled(False)
        self.status_label.setText(f"Fetching batch {self.current_page + 1}...")
        
        timeline_type = self.timeline_type_combo.currentData()
        media_type = self.media_type_combo.currentData()
        use_api = self.use_api_checkbox.isChecked()
        batch_size = 100
        
        self.fetcher = MetadataFetcher(
            username, 
            timeline_type, 
            media_type, 
            use_api, 
            True,
            batch_size, 
            self.current_page
        )
        self.fetcher.auth_token = auth_token
        self.fetcher.finished.connect(self.handle_next_batch)
        self.fetcher.error.connect(self.handle_fetch_error)
        self.fetcher.start()

    def handle_next_batch(self, info):
        try:
            if not info or not isinstance(info, dict):
                raise ValueError("Invalid info data received")
                
            timeline = info.get('timeline', [])
            if not timeline:
                self.status_label.setText("No more media items found")
                self.next_batch_button.setEnabled(True)
                return
                
            self.media_info['timeline'].extend(timeline)
            
            if 'metadata' in info:
                self.media_info['metadata'] = info['metadata']
                
            media_count = len(self.media_info['timeline'])
            new_items = len(timeline)
            
            self.status_label.setText(f"Added {new_items:,} items. Total: {media_count:,} media items")
            self.next_batch_button.setEnabled(True)
            
            has_more = info.get('metadata', {}).get('has_more', False)
            if not has_more:
                self.next_batch_button.hide()
                self.status_label.setText(f"All {media_count:,} media items fetched. No more items available.")
                
        except Exception as e:
            self.status_label.setText(f"Error processing batch: {str(e)}")
            self.next_batch_button.setEnabled(True)

    def handle_profile_info(self, info):
        try:
            if not info or not isinstance(info, dict):
                raise ValueError("Invalid info data received")

            account_info = info.get('account_info', {})
            if not account_info:
                self.status_label.setText("Failed to fetch profile information")
                self.fetch_button.setEnabled(True)
                return

            self.media_info = info
            self.fetch_button.setEnabled(True)
            
            is_withheld = not account_info.get('nick') and not account_info.get('profile_image')
            
            name = account_info.get('name', 'Unknown')
            nick = "Withheld Account" if is_withheld else account_info.get('nick', 'Unknown')
            date_str = account_info.get('date', '')
            followers = account_info.get('followers_count', 0)
            following = account_info.get('friends_count', 0)
            posts = account_info.get('statuses_count', 0)
            
            try:
                join_date = datetime.strptime(date_str, "%Y-%m-%d %H:%M:%S")
                join_date_str = join_date.strftime("%A, %d %B %Y - %H:%M")
            except (ValueError, TypeError):
                join_date_str = "Unknown Date"
            
            try:
                self.name_label.setText(f"<b>{name}</b> ({nick})")
                self.join_date_label.setText(f"<b>Join Date:</b> {join_date_str}")
                self.followers_label.setText(f"<b>Followers:</b> {followers:,}")
                self.following_label.setText(f"<b>Following:</b> {following:,}")
                self.posts_label.setText(f"<b>Posts:</b> {posts:,}")

                timeline = info.get('timeline', [])
                media_count = len(timeline) if isinstance(timeline, list) else 0
                
                media_type = self.media_type_combo.currentData()
                media_type_str = "media"
                if media_type == "image":
                    media_type_str = "image"
                elif media_type == "video":
                    media_type_str = "video"
                elif media_type == "gif":
                    media_type_str = "GIF"
                
                self.status_label.setText(f"Successfully fetched {media_count:,} {media_type_str} items")
            except Exception as ui_error:
                self.status_label.setText(f"Error updating UI: {str(ui_error)}")
                return

            if is_withheld:
                try:
                    withheld_icon_path = os.path.join(os.path.dirname(__file__), "withheld.svg")
                    if os.path.exists(withheld_icon_path):
                        icon = QIcon(withheld_icon_path)
                        pixmap = icon.pixmap(90, 90)
                        
                        painter = QPainter(pixmap)
                        painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_SourceIn)
                        painter.fillRect(pixmap.rect(), self.palette().text().color())
                        painter.end()
                        
                        scaled_pixmap = pixmap.scaled(
                            90, 90, 
                            Qt.AspectRatioMode.KeepAspectRatio, 
                            Qt.TransformationMode.SmoothTransformation
                        )
                        self.profile_image_label.setPixmap(scaled_pixmap)
                    else:
                        pass
                except Exception as icon_error:
                    pass
            else:
                profile_image_url = account_info.get('profile_image', '')
                if profile_image_url:
                    try:
                        self.image_downloader = ImageDownloader(profile_image_url)
                        self.image_downloader.finished.connect(self.update_profile_image)
                        self.image_downloader.start()
                    except Exception as img_error:
                        pass
            
            try:
                username = self.url_input.text().strip()
                if "x.com/" in username or "twitter.com/" in username:
                    parts = username.split('/')
                    for i, part in enumerate(parts):
                        if part in ['x.com', 'twitter.com'] and i + 1 < len(parts):
                            username = parts[i + 1]
                            username = username.split('/')[0]
                            break
                self.clean_username = username.strip()
            except Exception as username_error:
                self.clean_username = "unknown_user"

            self.input_widget.hide()
            self.profile_widget.show()
            self.download_button.show()
            self.cancel_button.show()
            self.update_button.hide()
            
            if self.batch_checkbox.isChecked() and info.get('metadata', {}).get('has_more', False):
                self.next_batch_button.show()
            else:
                self.next_batch_button.hide()
            
            try:
                output_base = self.dir_input.text().strip() or self.default_pictures_dir
                self.user_output_dir = os.path.join(output_base, self.clean_username)
                os.makedirs(self.user_output_dir, exist_ok=True)
            except Exception as dir_error:
                self.status_label.setText(f"Error creating output directory: {str(dir_error)}")
                return

        except Exception as e:
            self.status_label.setText(f"Error processing profile info: {str(e)}")
            self.fetch_button.setEnabled(True)

    def update_profile_image(self, image_data):
        original_pixmap = QPixmap()
        original_pixmap.loadFromData(image_data)
        
        scaled_pixmap = original_pixmap.scaled(100, 100, 
                                             Qt.AspectRatioMode.KeepAspectRatio, 
                                             Qt.TransformationMode.SmoothTransformation)
        
        rounded_pixmap = QPixmap(scaled_pixmap.size())
        rounded_pixmap.fill(Qt.GlobalColor.transparent)
        
        painter = QPainter(rounded_pixmap)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        path = QPainterPath()
        path.addRoundedRect(0, 0, scaled_pixmap.width(), scaled_pixmap.height(), 10, 10)
        
        painter.setClipPath(path)
        painter.drawPixmap(0, 0, scaled_pixmap)
        painter.end()
        
        self.profile_image_label.setPixmap(rounded_pixmap)

    def handle_fetch_error(self, error):
        self.fetch_button.setEnabled(True)
        self.next_batch_button.setEnabled(True)
        
        error_str = str(error)
        if "Local gallery-dl error: None" in error_str and not self.use_api_checkbox.isChecked():
            self.status_label.setText("Please check the 'Use API' option to fetch metadata using the API method.")
        else:
            self.status_label.setText(f"Error fetching profile info: {error}")

    def start_download(self):
        if not self.media_info:
            self.status_label.setText("Please fetch profile information first")
            return

        auth_token = self.auth_input.text().strip()
        if not auth_token:
            self.status_label.setText("Please enter your auth token")
            return

        self.download_button.hide()
        self.next_batch_button.hide()
        self.cancel_button.show()
        self.progress_bar.show()
        self.progress_bar.setValue(0)
        self.status_label.setText("Starting download...")

        filename_format = "username_date" if self.format_username.isChecked() else "date_username"
        batch_size = int(self.download_batch_combo.currentText())
        media_type = self.media_type_combo.currentData()

        self.worker = MediaDownloader(
            self.media_info['timeline'],
            self.user_output_dir,
            filename_format,
            self.clean_username,
            media_type,
            batch_size
        )
        self.worker.progress.connect(self.update_progress)
        self.worker.finished.connect(self.download_finished)
        self.worker.error.connect(self.download_error)
        self.worker.status_update.connect(self.status_label.setText)
        self.worker.start()

    def update_progress(self, value):
        self.progress_bar.setValue(value)

    def download_finished(self, message):
        self.progress_bar.hide()
        self.status_label.setText(message)
        self.open_button.show()
        
        if self.batch_checkbox.isChecked() and self.media_info and self.media_info.get('metadata', {}).get('has_more', False):
            self.next_batch_button.show()
            self.download_button.show()
            self.cancel_button.show()
        else:
            self.next_batch_button.hide()
            self.download_button.hide()
            self.cancel_button.show()

    def download_error(self, error_message):
        self.progress_bar.hide()
        self.status_label.setText(f"Download error: {error_message}")
        self.download_button.setText("Retry")
        self.download_button.show()
        self.cancel_button.show()
        
        if self.batch_checkbox.isChecked() and self.media_info and self.media_info.get('metadata', {}).get('has_more', False):
            self.next_batch_button.show()

    def cancel_clicked(self):
        if hasattr(self, 'worker') and self.worker.isRunning():
            self.worker.cancel()
            self.status_label.setText("Cancelling download...")
            return
        
        self.profile_widget.hide()
        self.input_widget.show()
        self.download_button.hide()
        self.next_batch_button.hide()
        self.open_button.hide()
        self.cancel_button.hide()
        self.progress_bar.hide()
        self.progress_bar.setValue(0)
        self.status_label.clear()
        self.media_info = None
        self.current_page = 0
        self.update_button.show()
        self.fetch_button.setEnabled(True)
        self.download_button.setText("Download")

def main():
    if getattr(sys, 'frozen', False):
        os.chdir(os.path.dirname(sys.executable))

    app = QApplication(sys.argv)
    window = TwitterMediaDownloaderGUI()
    window.show()
    sys.exit(app.exec())

if __name__ == "__main__":
    main()