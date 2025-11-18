import sqlite3
import os
import json
import threading
from datetime import datetime
from pathlib import Path
from typing import List, Dict, Optional, Tuple

class DatabaseManager:
    def __init__(self, db_path: str):
        self.db_path = db_path
        self.conn = None
        self._lock = threading.Lock()
        self.init_database()

    def init_database(self):
        self.conn = sqlite3.connect(self.db_path, check_same_thread=False)
        self.conn.row_factory = sqlite3.Row
        
        self.conn.execute('PRAGMA foreign_keys = ON')
        
        cursor = self.conn.cursor()
        
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                created_at TEXT
            )
        ''')
        
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS accounts (
                username TEXT PRIMARY KEY COLLATE NOCASE,
                nick TEXT,
                followers INTEGER,
                following INTEGER,
                posts INTEGER,
                profile_image TEXT,
                fetch_timestamp TEXT,
                media_type TEXT,
                fetch_mode TEXT,
                timeline_type TEXT DEFAULT 'media',
                group_id INTEGER,
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE SET NULL
            )
        ''')
        
        try:
            cursor.execute("SELECT group_id FROM accounts LIMIT 1")
        except sqlite3.OperationalError:
            cursor.execute("ALTER TABLE accounts ADD COLUMN group_id INTEGER")
            self.conn.commit()
        
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS media (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT COLLATE NOCASE,
                tweet_id TEXT,
                url TEXT,
                date TEXT,
                type TEXT,
                FOREIGN KEY (username) REFERENCES accounts(username) ON DELETE CASCADE,
                UNIQUE(username, url)
            )
        ''')
        
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS downloads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT,
                filepath TEXT,
                filepath_normalized TEXT COLLATE NOCASE,
                username TEXT COLLATE NOCASE,
                tweet_id TEXT,
                download_date TEXT,
                file_size INTEGER,
                status TEXT,
                UNIQUE(url, filepath_normalized)
            )
        ''')
        
        cursor.execute('CREATE INDEX IF NOT EXISTS idx_filepath_normalized ON downloads(filepath_normalized)')
        cursor.execute('CREATE INDEX IF NOT EXISTS idx_url ON downloads(url)')
        cursor.execute('CREATE INDEX IF NOT EXISTS idx_username ON downloads(username)')
        cursor.execute('CREATE INDEX IF NOT EXISTS idx_media_username ON media(username)')
        
        self.conn.commit()
    
    def save_account(self, username: str, nick: str, followers: int, following: int,
                     posts: int, media_type: str, profile_image: str = None,
                     fetch_mode: str = 'all', timeline_type: str = 'media',
                     fetch_timestamp: str = None, group_id: int = None):
        with self._lock:
            cursor = self.conn.cursor()

            if fetch_timestamp is None:
                fetch_timestamp = datetime.now().isoformat()

            cursor.execute('''
                INSERT OR REPLACE INTO accounts
                (username, nick, followers, following, posts, profile_image,
                 fetch_timestamp, media_type, fetch_mode, timeline_type, group_id)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ''', (username, nick, followers, following, posts, profile_image,
                  fetch_timestamp, media_type, fetch_mode, timeline_type, group_id))

            self.conn.commit()
    
    def save_media_list(self, username: str, media_list: List[Dict]):
        with self._lock:
            cursor = self.conn.cursor()

            cursor.execute('DELETE FROM media WHERE username = ?', (username,))

            for item in media_list:
                cursor.execute('''
                    INSERT OR IGNORE INTO media (username, tweet_id, url, date, type)
                    VALUES (?, ?, ?, ?, ?)
                ''', (username, item.get('tweet_id', ''), item['url'],
                      item['date'], item.get('type', 'image')))

            self.conn.commit()
    
    def get_account(self, username: str, media_type: str = 'all') -> Optional[Dict]:
        cursor = self.conn.cursor()
        cursor.execute('''
            SELECT * FROM accounts 
            WHERE username = ? AND media_type = ?
        ''', (username, media_type))
        
        row = cursor.fetchone()
        if row:
            return dict(row)
        return None
    
    def get_media_list(self, username: str) -> List[Dict]:
        cursor = self.conn.cursor()
        cursor.execute('''
            SELECT tweet_id, url, date, type 
            FROM media 
            WHERE username = ?
            ORDER BY date DESC
        ''', (username,))
        
        return [dict(row) for row in cursor.fetchall()]
    
    def get_all_accounts(self) -> List[Dict]:
        cursor = self.conn.cursor()
        cursor.execute('SELECT * FROM accounts ORDER BY fetch_timestamp DESC')
        return [dict(row) for row in cursor.fetchall()]
    
    def delete_account(self, username: str, media_type: str = None):
        with self._lock:
            cursor = self.conn.cursor()

            if media_type:
                cursor.execute('DELETE FROM accounts WHERE username = ? AND media_type = ?',
                              (username, media_type))
            else:
                cursor.execute('DELETE FROM accounts WHERE username = ?', (username,))

            self.conn.commit()
    
    def is_file_downloaded(self, filepath: str) -> bool:
        cursor = self.conn.cursor()
        filepath_normalized = filepath.lower()
        
        cursor.execute('''
            SELECT 1 FROM downloads 
            WHERE filepath_normalized = ? AND status = 'downloaded'
            LIMIT 1
        ''', (filepath_normalized,))
        
        return cursor.fetchone() is not None
    
    def is_url_downloaded(self, url: str) -> bool:
        cursor = self.conn.cursor()
        
        cursor.execute('''
            SELECT 1 FROM downloads 
            WHERE url = ? AND status = 'downloaded'
            LIMIT 1
        ''', (url,))
        
        return cursor.fetchone() is not None
    
    def record_download(self, url: str, filepath: str, username: str,
                       tweet_id: str, status: str = 'downloaded', file_size: int = 0):
        with self._lock:
            cursor = self.conn.cursor()
            filepath_normalized = filepath.lower()
            download_date = datetime.now().isoformat()

            cursor.execute('''
                INSERT OR REPLACE INTO downloads
                (url, filepath, filepath_normalized, username, tweet_id,
                 download_date, file_size, status)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ''', (url, filepath, filepath_normalized, username, tweet_id,
                  download_date, file_size, status))

            self.conn.commit()
    
    def get_download_stats(self, username: str = None) -> Dict:
        cursor = self.conn.cursor()
        
        if username:
            cursor.execute('''
                SELECT 
                    COUNT(*) as total,
                    SUM(CASE WHEN status = 'downloaded' THEN 1 ELSE 0 END) as downloaded,
                    SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END) as skipped,
                    SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed,
                    SUM(file_size) as total_size
                FROM downloads
                WHERE username = ?
            ''', (username,))
        else:
            cursor.execute('''
                SELECT 
                    COUNT(*) as total,
                    SUM(CASE WHEN status = 'downloaded' THEN 1 ELSE 0 END) as downloaded,
                    SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END) as skipped,
                    SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed,
                    SUM(file_size) as total_size
                FROM downloads
            ''')
        
        row = cursor.fetchone()
        return dict(row) if row else {}
    
    def migrate_from_json(self, json_dir: str) -> Tuple[int, int]:
        if not os.path.exists(json_dir):
            return 0, 0
        
        json_files = [f for f in os.listdir(json_dir) if f.endswith('.json')]
        accounts_migrated = 0
        media_migrated = 0
        
        for json_file in json_files:
            try:
                json_path = os.path.join(json_dir, json_file)
                
                with open(json_path, 'r', encoding='utf-8') as f:
                    data = json.load(f)
                
                media_type = data.get('media_type', 'all')
                
                if media_type not in ['all', 'image', 'video', 'gif']:
                    filename_parts = json_file.replace('.json', '').split('_')
                    if len(filename_parts) >= 1:
                        last_part = filename_parts[-1]
                        if last_part in ['all', 'image', 'video', 'gif']:
                            media_type = last_part
                        else:
                            media_type = 'all'
                    else:
                        media_type = 'all'
                
                filename_parts = json_file.replace('.json', '').split('_')
                if len(filename_parts) >= 2:
                    second_last = filename_parts[-2]
                    if second_last in ['media', 'timeline', 'tweets', 'with']:
                        timeline_type = second_last
                    else:
                        timeline_type = 'media'
                else:
                    timeline_type = 'media'
                
                account_info = data.get('account_info', {})
                timeline = data.get('timeline', [])
                
                username = data.get('username') or account_info.get('name', filename_parts[0] if filename_parts else 'unknown')
                
                nick = data.get('nick') or account_info.get('nick') or account_info.get('name', username)
                followers = data.get('followers') or account_info.get('followers_count', 0)
                following = data.get('following') or account_info.get('friends_count', 0)
                posts = data.get('posts') or account_info.get('statuses_count', 0)
                profile_image = data.get('profile_image') or account_info.get('profile_image')
                
                self.save_account(
                    username=username,
                    nick=nick,
                    followers=followers,
                    following=following,
                    posts=posts,
                    media_type=media_type,
                    profile_image=profile_image,
                    fetch_mode=data.get('fetch_mode', 'all'),
                    timeline_type=timeline_type
                )
                accounts_migrated += 1
                
                media_list = timeline if timeline else data.get('media_list', [])
                if media_list:
                    self.save_media_list(username, media_list)
                    media_migrated += len(media_list)
                
                backup_path = json_path + '.backup'
                if not os.path.exists(backup_path):
                    os.rename(json_path, backup_path)
                
            except Exception as e:
                print(f"Error migrating {json_file}: {e}")
                continue
        
        return accounts_migrated, media_migrated
    
    def export_to_json(self, username: str, media_type: str, output_path: str):
        account = self.get_account(username, media_type)
        if not account:
            return False
        
        media_list = self.get_media_list(username)
        
        export_data = {
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
        
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(export_data, f, ensure_ascii=False, indent=2)
        
        return True
    
    def import_from_json(self, json_path: str) -> bool:
        try:
            with open(json_path, 'r', encoding='utf-8') as f:
                data = json.load(f)
            
            username = data.get('username')
            if not username:
                return False
            
            self.save_account(
                username=username,
                nick=data.get('nick', ''),
                followers=data.get('followers', 0),
                following=data.get('following', 0),
                posts=data.get('posts', 0),
                media_type=data.get('media_type', 'all'),
                profile_image=data.get('profile_image'),
                fetch_mode=data.get('fetch_mode', 'all')
            )
            
            media_list = data.get('media_list', [])
            if media_list:
                self.save_media_list(username, media_list)
            
            return True
            
        except Exception as e:
            print(f"Error importing JSON: {e}")
            return False
    
    def close(self):
        if self.conn:
            self.conn.close()
    
    def create_group(self, name: str) -> Optional[int]:
        with self._lock:
            try:
                cursor = self.conn.cursor()
                created_at = datetime.now().isoformat()
                cursor.execute('INSERT INTO groups (name, created_at) VALUES (?, ?)', (name, created_at))
                self.conn.commit()
                return cursor.lastrowid
            except sqlite3.IntegrityError:
                return None
    
    def get_all_groups(self) -> List[Dict]:
        cursor = self.conn.cursor()
        cursor.execute('SELECT * FROM groups ORDER BY name')
        return [dict(row) for row in cursor.fetchall()]
    
    def get_group(self, group_id: int) -> Optional[Dict]:
        cursor = self.conn.cursor()
        cursor.execute('SELECT * FROM groups WHERE id = ?', (group_id,))
        row = cursor.fetchone()
        return dict(row) if row else None
    
    def update_group(self, group_id: int, name: str) -> bool:
        with self._lock:
            try:
                cursor = self.conn.cursor()
                cursor.execute('UPDATE groups SET name = ? WHERE id = ?', (name, group_id))
                self.conn.commit()
                return True
            except sqlite3.IntegrityError:
                return False
    
    def delete_group(self, group_id: int):
        with self._lock:
            cursor = self.conn.cursor()
            cursor.execute('UPDATE accounts SET group_id = NULL WHERE group_id = ?', (group_id,))
            cursor.execute('DELETE FROM groups WHERE id = ?', (group_id,))
            self.conn.commit()
    
    def assign_account_to_group(self, username: str, group_id: Optional[int]):
        with self._lock:
            cursor = self.conn.cursor()
            cursor.execute('UPDATE accounts SET group_id = ? WHERE username = ?', (group_id, username))
            self.conn.commit()
    
    def get_accounts_by_group(self, group_id: Optional[int]) -> List[Dict]:
        cursor = self.conn.cursor()
        if group_id is None:
            cursor.execute('SELECT * FROM accounts WHERE group_id IS NULL ORDER BY fetch_timestamp DESC')
        else:
            cursor.execute('SELECT * FROM accounts WHERE group_id = ? ORDER BY fetch_timestamp DESC', (group_id,))
        return [dict(row) for row in cursor.fetchall()]
