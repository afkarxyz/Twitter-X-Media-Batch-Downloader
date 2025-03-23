import re
import json
from datetime import datetime
from gallery_dl.extractor import twitter

def get_metadata(username, auth_token, timeline_type="timeline", batch_size=0, page=0, media_type="all"):
    url = f"https://x.com/{username}/{timeline_type}"
    
    if timeline_type == "media":
        extractor_class = twitter.TwitterMediaExtractor
    elif timeline_type == "tweets":
        extractor_class = twitter.TwitterTweetsExtractor
    elif timeline_type == "with_replies":
        extractor_class = twitter.TwitterRepliesExtractor
    else:
        extractor_class = twitter.TwitterTimelineExtractor
    
    match = re.match(extractor_class.pattern, url)
    if not match:
        raise ValueError(f"Invalid URL for {timeline_type}: {url}")
    
    extractor = extractor_class(match)
    
    config_dict = {
        "cookies": {
            "auth_token": auth_token
        }
    }
    
    if batch_size > 0:
        config_dict["count"] = batch_size
    
    extractor.config = lambda key, default=None: config_dict.get(key, default)
    
    extractor.initialize()
    
    structured_output = {
        'account_info': {},
        'total_urls': 0,
        'timeline': []
    }
    
    iterator = iter(extractor)
    
    if batch_size > 0 and page > 0:
        items_to_skip = page * batch_size
        
        if hasattr(extractor, '_cursor') and extractor._cursor:
            pass
        else:
            skipped = 0
            try:
                for _ in range(items_to_skip):
                    next(iterator)
                    skipped += 1
            except StopIteration:
                pass
    
    new_timeline_entries = []
    
    items_to_fetch = batch_size if batch_size > 0 else float('inf')
    items_fetched = 0
    
    try:
        while items_fetched < items_to_fetch:
            item = next(iterator)
            items_fetched += 1
            
            if isinstance(item, tuple) and len(item) >= 3:
                media_url = item[1]
                tweet_data = item[2]
                
                if not structured_output['account_info'] and 'user' in tweet_data:
                    user = tweet_data['user']
                    user_date = user.get('date', '')
                    if isinstance(user_date, datetime):
                        user_date = user_date.strftime("%Y-%m-%d %H:%M:%S")
                    
                    structured_output['account_info'] = {
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
                    
                    if media_type == 'all' or (
                        (media_type == 'image' and 'pbs.twimg.com' in media_url and tweet_data.get('type') == 'photo') or
                        (media_type == 'video' and 'video.twimg.com' in media_url and tweet_data.get('type') == 'video') or
                        (media_type == 'gif' and 'video.twimg.com' in media_url and tweet_data.get('type') == 'animated_gif')
                    ):
                        new_timeline_entries.append(timeline_entry)
                        structured_output['total_urls'] += 1
    except StopIteration:
        pass
    
    structured_output['timeline'].extend(new_timeline_entries)
    
    cursor_info = None
    if hasattr(extractor, '_cursor') and extractor._cursor:
        cursor_info = extractor._cursor
    
    structured_output['metadata'] = {
        "new_entries": len(new_timeline_entries),
        "page": page,
        "batch_size": batch_size,
        "has_more": batch_size > 0 and items_fetched == batch_size,
        "cursor": cursor_info
    }
    
    if not structured_output['account_info']:
        raise ValueError("Failed to fetch account information. Please check the username and auth token.")
    
    return structured_output

def main():
    username = ""
    auth_token = ""
    timeline_type = "media"
    batch_size = 100
    page = 0
    media_type = "all"
    
    try:
        data = get_metadata(
            username=username,
            auth_token=auth_token,
            timeline_type=timeline_type,
            batch_size=batch_size,
            page=page,
            media_type=media_type
        )
        print(json.dumps(data, ensure_ascii=False, indent=2))
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))

if __name__ == '__main__':
    main()