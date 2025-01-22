import json
import re
import os
from datetime import datetime
from gallery_dl.extractor import twitter

def get_metadata(username):
    url = f"https://x.com/{username}/timeline"
    
    match = re.match(twitter.TwitterTimelineExtractor.pattern, url)
    if not match:
        raise ValueError(f"Invalid URL: {url}")

    extractor = twitter.TwitterTimelineExtractor(match)
    
    extractor.config = lambda key, default=None: {
        "cookies": {
            "auth_token": ""
        }
    }.get(key, default)
    
    extractor.initialize()
    
    output = {
        'account_info': {},
        'total_urls': 0,
        'timeline': []
    }
    
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
            
            if 'pbs.twimg.com' in media_url or 'video.twimg.com' in media_url:
                tweet_date = tweet_data.get('date', datetime.now())
                if isinstance(tweet_date, datetime):
                    tweet_date = tweet_date.strftime("%Y-%m-%d %H:%M:%S")
                
                timeline_entry = {
                    'url': media_url,
                    'date': tweet_date,
                    'tweet_id': tweet_data.get('tweet_id', 0),
                }
                
                if 'type' in tweet_data:
                    timeline_entry['type'] = tweet_data['type']
                
                output['timeline'].append(timeline_entry)
                output['total_urls'] += 1

    current_dir = os.path.dirname(os.path.abspath(__file__))
    output_file = os.path.join(current_dir, f"{username}.json")
    
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(output, f, ensure_ascii=False, indent=2)

    return output

username = ""
get_metadata(username)
