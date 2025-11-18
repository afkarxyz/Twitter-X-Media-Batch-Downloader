from PyQt6.QtCore import QThread, pyqtSignal
from core.metadataFetcher import get_metadata, get_metadata_by_date

class MetadataFetchWorker(QThread):
    finished = pyqtSignal(dict)
    error = pyqtSignal(str)

    def __init__(self, username, media_type='all', batch_mode=False, batch_size=0, page=0, timeline_type='media', include_retweets=False, use_date_range=False, date_start=None, date_end=None):
        super().__init__()
        self.username = username
        self.media_type = media_type
        self.batch_mode = batch_mode
        self.batch_size = batch_size
        self.page = page
        self.timeline_type = timeline_type
        self.include_retweets = include_retweets
        self.use_date_range = use_date_range
        self.date_start = date_start
        self.date_end = date_end
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
            if self.use_date_range and self.date_start and self.date_end:
                data = get_metadata_by_date(
                    username=normalized,
                    auth_token=self.auth_token,
                    date_start=self.date_start,
                    date_end=self.date_end,
                    media_filter="filter:media" if self.media_type == 'all' else f"filter:{self.media_type}"
                )
            else:
                data = get_metadata(
                    username=normalized,
                    auth_token=self.auth_token,
                    timeline_type=self.timeline_type,
                    batch_size=self.batch_size if self.batch_mode else 0,
                    page=self.page,
                    media_type=self.media_type,
                    retweets=self.include_retweets
                )
            self.finished.emit(data)
        except Exception as e:
            self.error.emit(str(e))