import sys
import os
import asyncio
import aiohttp
import subprocess
import imageio_ffmpeg
from datetime import datetime
from PyQt6.QtCore import QThread, pyqtSignal

DEFAULT_BATCH_SIZE = 25
TIMEOUT_SECONDS_MAC = 60
TIMEOUT_SECONDS_DEFAULT = 30
BATCH_SLEEP_INTERVAL = 0.1
PAUSE_CHECK_INTERVAL = 0.1

class DownloadWorker(QThread):
    finished = pyqtSignal(bool, str)
    progress = pyqtSignal(str, int)
    conversion_progress = pyqtSignal(str, int)
    download_progress = pyqtSignal(str, int)
    
    def __init__(self, accounts, outpath, auth_token, filename_format='username_date',
                 download_batch_size=DEFAULT_BATCH_SIZE, convert_gif=False, gif_resolution='original', gif_conversion_mode='better', db_manager=None):
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
        self.db_manager = db_manager

    async def download_file(self, session, url, filepath, username, tweet_id):
        try:
            if self.db_manager and self.db_manager.is_url_downloaded(url):
                return True, True
            
            if os.path.exists(filepath):
                if self.db_manager:
                    file_size = os.path.getsize(filepath)
                    self.db_manager.record_download(url, filepath, username, tweet_id, 'downloaded', file_size)
                return True, True
            
            if self.is_stopped:
                return False, False
            
            try:
                os.makedirs(os.path.dirname(filepath), exist_ok=True)
            except OSError as e:
                return False, False
            
            async with session.get(url) as response:
                if response.status == 200:
                    content = await response.read()
                    
                    try:
                        with open(filepath, 'wb') as f:
                            f.write(content)
                    except (IOError, OSError) as e:
                        if self.db_manager:
                            self.db_manager.record_download(url, filepath, username, tweet_id, 'failed', 0)
                        return False, False
                    
                    if self.db_manager:
                        self.db_manager.record_download(url, filepath, username, tweet_id, 'downloaded', len(content))
                    
                    return True, False
                return False, False
        except asyncio.TimeoutError:
            if self.db_manager:
                self.db_manager.record_download(url, filepath, username, tweet_id, 'failed', 0)
            return False, False
        except aiohttp.ClientError as e:
            if self.db_manager:
                self.db_manager.record_download(url, filepath, username, tweet_id, 'failed', 0)
            return False, False
        except Exception as e:
            if self.db_manager:
                self.db_manager.record_download(url, filepath, username, tweet_id, 'failed', 0)
            return False, False

    async def download_account_media(self, account):
        if not account.media_list:
            return 0, 0, 0

        safe_username = os.path.basename(account.username.replace('/', '_').replace('\\', '_'))
        account_output_dir = os.path.join(self.outpath, safe_username)
        os.makedirs(account_output_dir, exist_ok=True)
        
        timeout_seconds = TIMEOUT_SECONDS_MAC if sys.platform == 'darwin' else TIMEOUT_SECONDS_DEFAULT
        timeout = aiohttp.ClientTimeout(total=timeout_seconds)
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
                    await asyncio.sleep(PAUSE_CHECK_INTERVAL)
                
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
                        base_filename = f"{safe_username}_{formatted_date}_{tweet_id}"
                    else:
                        base_filename = f"{formatted_date}_{safe_username}_{tweet_id}"
                    
                    filename = f"{base_filename}.{extension}"
                    counter = 1
                    while filename in used_filenames:
                        filename = f"{base_filename}_{counter:02d}.{extension}"
                        counter += 1
                    
                    used_filenames.add(filename)
                    filepath = os.path.join(media_output_dir, filename)
                    self.filepath_map.append((item, filepath))
                    task = asyncio.create_task(self.download_file(session, url, filepath, account.username, tweet_id))
                    tasks.append(task)
                results = await asyncio.gather(*tasks, return_exceptions=True)
                for result in results:
                    if isinstance(result, tuple):
                        success, was_skipped = result
                        if success:
                            if was_skipped:
                                skipped += 1
                            else:
                                completed += 1
                        else:
                            failed += 1
                    else:
                        failed += 1

                    progress_percent = int((completed + skipped + failed) / total * 100)
                    media_type_display = account.media_type if account.media_type != 'all' else 'media'
                    self.download_progress.emit(f"Downloading {account.username}'s {media_type_display}: {completed + skipped + failed:,}/{total:,}", progress_percent)

                await asyncio.sleep(BATCH_SLEEP_INTERVAL)
            
            return completed, skipped, failed

    def run(self):
        try:
            if sys.platform == 'darwin':
                asyncio.set_event_loop_policy(asyncio.DefaultEventLoopPolicy())
            
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
                            if sys.platform == 'win32':
                                creationflags = subprocess.CREATE_NO_WINDOW
                            else:
                                creationflags = 0
                            
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
                                
                                try:
                                    if sys.platform == 'win32':
                                        result = subprocess.run(ffmpeg_args, capture_output=True, creationflags=creationflags)
                                    else:
                                        result = subprocess.run(ffmpeg_args, capture_output=True, stderr=subprocess.PIPE, stdout=subprocess.PIPE)
                                    
                                    if result.returncode == 0 and os.path.exists(gif_fp):
                                        try:
                                            os.remove(fp)
                                            converted_count += 1
                                        except Exception:
                                            pass
                                    elif result.returncode != 0:
                                        error_msg = result.stderr.decode('utf-8', errors='ignore') if result.stderr else 'Unknown error'
                                        self.progress.emit(f"GIF conversion warning for file {idx}: {error_msg[:100]}", 0)
                                except Exception as e:
                                    self.progress.emit(f"GIF conversion error for file {idx}: {str(e)[:100]}", 0)
                                    
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