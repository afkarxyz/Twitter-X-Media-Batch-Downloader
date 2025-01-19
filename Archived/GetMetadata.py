import json
import subprocess
import sys
import os
from datetime import datetime

def get_metadata(username):
    if sys.platform == 'win32':
        sys.stdout.reconfigure(encoding='utf-8')
    
    url = f"https://x.com/{username}/timeline"
    
    current_dir = os.path.dirname(os.path.abspath(__file__))
    
    cookies_path = os.path.join(current_dir, 'cookies.json')
    output_path = os.path.join(current_dir, f'{username}.json')
    gallery_dl_path = os.path.join(current_dir, 'gallery-dl.exe')
    
    config = {
        "extractor": {
            "twitter": {
                "cookies": {
                    "auth_token": ""
                }
            }
        }
    }
    
    with open(cookies_path, 'w') as f:
        json.dump(config, f)
    
    result = subprocess.run([gallery_dl_path, '-j', '--config', cookies_path, url], 
                          capture_output=True, 
                          text=True, 
                          encoding='utf-8',
                          check=True)
    
    os.remove(cookies_path)
    
    data = json.loads(result.stdout)
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
    
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(output, f, ensure_ascii=False, indent=2)

username = ""
get_metadata(username)
