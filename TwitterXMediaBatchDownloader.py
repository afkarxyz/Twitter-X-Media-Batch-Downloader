import sys
import os
import json
import asyncio
import aiohttp
import subprocess
from datetime import datetime
from pathlib import Path
from PyQt6.QtWidgets import (QApplication, QMainWindow, QWidget, QVBoxLayout, 
                            QHBoxLayout, QLabel, QLineEdit, QPushButton, 
                            QProgressBar, QFileDialog, QRadioButton, QComboBox)
from PyQt6.QtCore import QThread, pyqtSignal, Qt, QSettings
from PyQt6.QtGui import QIcon, QPixmap, QCursor

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
    
    def __init__(self, username):
        super().__init__()
        self.username = username
        
    def run(self):
        try:
            username = self.username.strip()
            if "x.com/" in username or "twitter.com/" in username:
                username = username.split("/")[-1]
                if username.endswith("/timeline"):
                    username = username[:-9]
            
            url = f"https://x.com/{username}/timeline"
            
            if getattr(sys, 'frozen', False):
                base_path = sys._MEIPASS
            else:
                base_path = os.path.dirname(os.path.abspath(__file__))
            
            cookies_path = os.path.join(base_path, 'cookies.json')
            gallery_dl_path = os.path.join(base_path, 'gallery-dl.exe')
            
            if not os.path.exists(gallery_dl_path):
                gallery_dl_path = os.path.join(os.getcwd(), 'gallery-dl.exe')
            
            if not os.path.exists(gallery_dl_path):
                gallery_dl_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'gallery-dl.exe')
            
            if not os.path.exists(gallery_dl_path):
                raise FileNotFoundError(
                    "gallery-dl.exe not found. Please ensure it's in the same directory as the executable."
                )
            
            config = {
                "extractor": {
                    "twitter": {
                        "cookies": {
                            "auth_token": self.auth_token
                        }
                    }
                }
            }
            
            try:
                cookies_dir = os.path.dirname(cookies_path)
                if not os.path.exists(cookies_dir):
                    os.makedirs(cookies_dir)
                
                with open(cookies_path, 'w', encoding='utf-8') as f:
                    json.dump(config, f)
            except Exception as e:
                raise Exception(f"Failed to write cookies file: {str(e)}")

            startupinfo = None
            if sys.platform == 'win32':
                startupinfo = subprocess.STARTUPINFO()
                startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW
            
            try:
                process = subprocess.run(
                    [gallery_dl_path, '-j', '--config', cookies_path, url],
                    capture_output=True,
                    text=True,
                    encoding='utf-8',
                    check=True,
                    startupinfo=startupinfo
                )
            except subprocess.CalledProcessError as e:
                raise Exception(f"gallery-dl.exe failed: {e.stderr}")
            except Exception as e:
                raise Exception(f"Failed to execute gallery-dl.exe: {str(e)}")
            finally:
                try:
                    if os.path.exists(cookies_path):
                        os.remove(cookies_path)
                except Exception:
                    pass
            
            data = json.loads(process.stdout)
            output = {
                'account_info': {},
                'timeline': []
            }
            
            for item in data:
                if isinstance(item, list) and len(item) > 2 and isinstance(item[2], dict):
                    if not output['account_info'] and 'user' in item[2]:
                        user = item[2]['user']
                        output['account_info'] = {
                            'date': user.get('date', ''),
                            'followers_count': user.get('followers_count', ''),
                            'friends_count': user.get('friends_count', ''),
                            'name': user.get('name', ''),
                            'nick': user.get('nick', ''),
                            'profile_image': user.get('profile_image', ''),
                            'statuses_count': user.get('statuses_count', '')
                        }
                    if isinstance(item[1], str):
                        url = item[1]
                        if ('pbs.twimg.com' in url and 'format=jpg&name=orig' in url) or 'video.twimg.com' in url:
                            tweet_date = item[2].get('date')
                            if not tweet_date:
                                tweet_date = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                            
                            output['timeline'].append({
                                'url': url,
                                'date': tweet_date
                            })
            
            self.finished.emit(output)
            
        except Exception as e:
            self.error.emit(str(e))

class MediaDownloader(QThread):
    progress = pyqtSignal(int)
    finished = pyqtSignal(str)
    error = pyqtSignal(str)
    status_update = pyqtSignal(str)

    def __init__(self, media_list, output_dir, filename_format, username, batch_size=10):
        super().__init__()
        self.media_list = media_list
        self.output_dir = output_dir
        self.filename_format = filename_format
        self.username = username
        self.batch_size = batch_size
        
    async def download_file(self, session, url, filepath):
        try:
            async with session.get(url) as response:
                if response.status == 200:
                    with open(filepath, 'wb') as f:
                        f.write(await response.read())
                    return True
                return False
        except Exception:
            return False

    async def download_all(self):
        async with aiohttp.ClientSession() as session:
            total = len(self.media_list)
            completed = 0
            
            tasks = []
            for item in self.media_list:
                url = item['url']
                date = datetime.strptime(item['date'], "%Y-%m-%d %H:%M:%S")
                formatted_date = date.strftime("%Y-%m-%d")
                
                if self.filename_format == "username_date":
                    filename = f"{self.username}_{formatted_date}"
                else:
                    filename = f"{formatted_date}_{self.username}"
                
                filename = f"{filename}_{len(tasks):03d}.{'mp4' if 'video' in url else 'jpg'}"
                
                filepath = os.path.join(self.output_dir, filename)
                
                task = asyncio.create_task(self.download_file(session, url, filepath))
                tasks.append(task)
                
                if len(tasks) >= self.batch_size:
                    results = await asyncio.gather(*tasks)
                    completed += sum(1 for r in results if r)
                    progress = int((completed / total) * 100)
                    self.progress.emit(progress)
                    self.status_update.emit(f"Downloaded {completed} of {total} files...")
                    tasks = []
            
            if tasks:
                results = await asyncio.gather(*tasks)
                completed += sum(1 for r in results if r)
                progress = int((completed / total) * 100)
                self.progress.emit(progress)
                
            return completed

    def run(self):
        try:
            completed = asyncio.run(self.download_all())
            self.finished.emit(f"Downloaded {completed} files successfully!")
        except Exception as e:
            self.error.emit(str(e))

class TwitterMediaDownloaderGUI(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Twitter/X Media Batch Downloader")
        
        icon_path = os.path.join(os.path.dirname(__file__), "icon.svg")
        if os.path.exists(icon_path):
            self.setWindowIcon(QIcon(icon_path))
            
        self.setFixedWidth(600)
        self.setFixedHeight(180)
        
        self.default_pictures_dir = str(Path.home() / "Pictures")
        if not os.path.exists(self.default_pictures_dir):
            os.makedirs(self.default_pictures_dir)
        
        self.media_info = None
        self.settings = QSettings('TwitterMediaDownloader', 'Settings')
        
        self.init_ui()
        self.load_settings()
        self.setup_auto_save()

    def init_ui(self):
        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        self.main_layout = QVBoxLayout(central_widget)
        self.main_layout.setContentsMargins(10, 10, 10, 10)

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

        combined_layout = QHBoxLayout()
        
        batch_label = QLabel("Batch Size:")
        batch_label.setFixedWidth(100)
        
        self.batch_size_combo = QComboBox()
        self.batch_size_combo.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.batch_size_combo.setFixedWidth(70)
        for size in range(10, 101, 10):
            self.batch_size_combo.addItem(str(size))
        
        format_label = QLabel("Filename:")
        format_label.setFixedWidth(50)
        
        self.format_username_date = QRadioButton("Username - Date")
        self.format_date_username = QRadioButton("Date - Username")
        self.format_username_date.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.format_date_username.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        
        combined_layout.addWidget(batch_label)
        combined_layout.addWidget(self.batch_size_combo)
        combined_layout.addSpacing(10)
        combined_layout.addWidget(format_label)
        combined_layout.addWidget(self.format_username_date)
        combined_layout.addWidget(self.format_date_username)
        combined_layout.addStretch()
        
        input_layout.addLayout(combined_layout)

        self.main_layout.addWidget(self.input_widget)

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

        self.main_layout.addWidget(self.profile_widget)

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
        button_layout.addWidget(self.download_button)
        button_layout.addWidget(self.cancel_button)
        button_layout.addStretch()
        self.main_layout.addLayout(button_layout)

        self.progress_bar = QProgressBar()
        self.progress_bar.hide()
        self.main_layout.addWidget(self.progress_bar)

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

    def open_update_page(self):
        import webbrowser
        webbrowser.open('https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases')
        
    def setup_auto_save(self):
        self.url_input.textChanged.connect(self.auto_save_settings)
        self.auth_input.textChanged.connect(self.auto_save_settings)
        self.dir_input.textChanged.connect(self.auto_save_settings)
        self.format_username_date.toggled.connect(self.auto_save_settings)
        self.batch_size_combo.currentTextChanged.connect(self.auto_save_settings)
    
    def auto_save_settings(self):
        self.settings.setValue('url_input', self.url_input.text())
        self.settings.setValue('auth_token', self.auth_input.text())
        self.settings.setValue('output_dir', self.dir_input.text())
        self.settings.setValue('filename_format', 
                             'username_date' if self.format_username_date.isChecked() else 'date_username')
        self.settings.setValue('batch_size', self.batch_size_combo.currentText())
        self.settings.sync()

    def load_settings(self):
        self.url_input.setText(self.settings.value('url_input', '', str))
        self.auth_input.setText(self.settings.value('auth_token', '', str))
        self.dir_input.setText(self.settings.value('output_dir', self.default_pictures_dir, str))
        
        format_setting = self.settings.value('filename_format', 'username_date')
        self.format_username_date.setChecked(format_setting == 'username_date')
        self.format_date_username.setChecked(format_setting == 'date_username')
        
        batch_size = self.settings.value('batch_size', '10')
        index = self.batch_size_combo.findText(batch_size)
        if index >= 0:
            self.batch_size_combo.setCurrentIndex(index)

    def select_directory(self):
        directory = QFileDialog.getExistingDirectory(self, "Select Output Directory")
        if directory:
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
        
        self.fetcher = MetadataFetcher(username)
        self.fetcher.auth_token = auth_token
        self.fetcher.finished.connect(self.handle_profile_info)
        self.fetcher.error.connect(self.handle_fetch_error)
        self.fetcher.start()

    def handle_profile_info(self, info):
        if not info.get('account_info'):
            self.status_label.setText("Failed to fetch profile information")
            self.fetch_button.setEnabled(True)
            return
            
        self.media_info = info
        self.fetch_button.setEnabled(True)
        
        account_info = info['account_info']
        name = account_info['name']
        nick = account_info['nick']
        join_date = datetime.strptime(account_info['date'], "%Y-%m-%d %H:%M:%S")
        followers = account_info['followers_count']
        following = account_info['friends_count']
        posts = account_info['statuses_count']
        
        join_date_str = join_date.strftime("%A, %d %B %Y - %H:%M")
        
        self.name_label.setText(f"<b>{name}</b> ({nick})")
        self.join_date_label.setText(f"<b>Join Date:</b> {join_date_str}")
        self.followers_label.setText(f"<b>Followers:</b> {followers:,}")
        self.following_label.setText(f"<b>Following:</b> {following:,}")
        self.posts_label.setText(f"<b>Posts:</b> {posts:,}")

        media_count = len(info['timeline'])
        self.status_label.setText(f"Successfully fetched {media_count:,} media items")
        
        profile_image_url = account_info['profile_image']
        self.image_downloader = ImageDownloader(profile_image_url)
        self.image_downloader.finished.connect(self.update_profile_image)
        self.image_downloader.start()
        
        self.input_widget.hide()
        self.profile_widget.show()
        self.download_button.show()
        self.cancel_button.show()
        self.update_button.hide()

        username = self.url_input.text().strip()
        if "x.com/" in username or "twitter.com/" in username:
            username = username.split("/")[-1]
            if username.endswith("/timeline"):
                username = username[:-9]
        
        output_base = self.dir_input.text().strip() or self.default_pictures_dir
        self.user_output_dir = os.path.join(output_base, username)
        if not os.path.exists(self.user_output_dir):
            os.makedirs(self.user_output_dir)

    def update_profile_image(self, image_data):
        pixmap = QPixmap()
        pixmap.loadFromData(image_data)
        scaled_pixmap = pixmap.scaled(100, 100, Qt.AspectRatioMode.KeepAspectRatio, 
                                    Qt.TransformationMode.SmoothTransformation)
        self.profile_image_label.setPixmap(scaled_pixmap)

    def handle_fetch_error(self, error):
        self.fetch_button.setEnabled(True)
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
        self.cancel_button.hide()
        self.progress_bar.show()
        self.progress_bar.setValue(0)
        self.status_label.setText("Starting download...")

        filename_format = "username_date" if self.format_username_date.isChecked() else "date_username"
        batch_size = int(self.batch_size_combo.currentText())
        
        username = self.url_input.text().strip()
        if "x.com/" in username or "twitter.com/" in username:
            username = username.split("/")[-1]
            if username.endswith("/timeline"):
                username = username[:-9]

        self.worker = MediaDownloader(
            self.media_info['timeline'],
            self.user_output_dir,
            filename_format,
            username,
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
        self.download_button.setText("Clear")
        self.download_button.clicked.disconnect()
        self.download_button.clicked.connect(self.clear_form)
        self.download_button.show()
        self.cancel_button.hide()

    def clear_form(self):
        self.url_input.clear()
        self.profile_widget.hide()
        self.input_widget.show()
        self.download_button.hide()
        self.cancel_button.hide()
        self.open_button.hide()
        self.progress_bar.hide()
        self.progress_bar.setValue(0)
        self.status_label.clear()
        self.media_info = None
        self.update_button.show()
        self.fetch_button.setEnabled(True)
        self.download_button.setText("Download")
        self.download_button.clicked.disconnect()
        self.download_button.clicked.connect(self.start_download)

    def download_error(self, error_message):
        self.progress_bar.hide()
        self.status_label.setText(f"Download error: {error_message}")
        self.download_button.setText("Retry")
        self.download_button.show()
        self.cancel_button.show()

    def cancel_clicked(self):
        self.profile_widget.hide()
        self.input_widget.show()
        self.download_button.hide()
        self.cancel_button.hide()
        self.open_button.hide()
        self.progress_bar.hide()
        self.progress_bar.setValue(0)
        self.status_label.clear()
        self.media_info = None
        self.update_button.show()
        self.fetch_button.setEnabled(True)

def main():
    if getattr(sys, 'frozen', False):
        os.chdir(os.path.dirname(sys.executable))
    
    app = QApplication(sys.argv)
    window = TwitterMediaDownloaderGUI()
    window.show()
    sys.exit(app.exec())

if __name__ == "__main__":
    main()
