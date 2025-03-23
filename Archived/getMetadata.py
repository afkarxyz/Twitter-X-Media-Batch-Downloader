import json
import re
import os
from datetime import datetime
from gallery_dl.extractor import twitter

def get_metadata(username, auth_token, timeline_type="timeline", batch_size=0, page=0, output_file=None):
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
    
    if output_file is None:
        current_dir = os.path.dirname(os.path.abspath(__file__))
        output_file = os.path.join(current_dir, f"{username}_{timeline_type}.json")
    
    structured_output = {
        'account_info': {},
        'total_urls': 0,
        'timeline': []
    }
    
    if page > 0 and os.path.exists(output_file):
        try:
            with open(output_file, 'r', encoding='utf-8') as f:
                structured_output = json.load(f)
            print(f"Loaded existing data with {structured_output['total_urls']} items from {output_file}")
        except json.JSONDecodeError:
            print(f"Warning: Could not load existing data from {output_file}, starting fresh")
    
    iterator = iter(extractor)
    
    if batch_size > 0 and page > 0:
        items_to_skip = page * batch_size
        print(f"Skipping {items_to_skip} items for page {page}...")
        
        if hasattr(extractor, '_cursor') and extractor._cursor:
            print(f"Using cursor: {extractor._cursor}")
        else:
            skipped = 0
            try:
                for _ in range(items_to_skip):
                    next(iterator)
                    skipped += 1
            except StopIteration:
                print(f"Warning: Could only skip {skipped} items")
    
    batch_items = []
    new_timeline_entries = []
    
    items_to_fetch = batch_size if batch_size > 0 else float('inf')
    items_fetched = 0
    
    try:
        while items_fetched < items_to_fetch:
            item = next(iterator)
            batch_items.append(item)
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
                    
                    new_timeline_entries.append(timeline_entry)
                    structured_output['total_urls'] += 1
    except StopIteration:
        pass
    
    structured_output['timeline'].extend(new_timeline_entries)
    
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(structured_output, f, ensure_ascii=False, indent=2)
    
    if batch_size > 0:
        print(f"Page {page}: Added {len(new_timeline_entries)} media URLs. Total URLs: {structured_output['total_urls']}")
    else:
        print(f"Added {len(new_timeline_entries)} media URLs. Total URLs: {structured_output['total_urls']}")
    
    cursor_info = None
    if hasattr(extractor, '_cursor') and extractor._cursor:
        cursor_info = extractor._cursor
        print(f"Current cursor: {cursor_info}")
    
    return {
        "items": batch_items,
        "structured_output": structured_output,
        "new_entries": len(new_timeline_entries),
        "total_urls": structured_output['total_urls'],
        "page": page,
        "batch_size": batch_size,
        "has_more": batch_size > 0 and items_fetched == batch_size,
        "cursor": cursor_info
    }

def continue_metadata_fetch(username, auth_token, timeline_type="media", batch_size=100, max_pages=None):
    current_dir = os.path.dirname(os.path.abspath(__file__))
    output_file = os.path.join(current_dir, f"{username}_{timeline_type}.json")
    
    page = 0
    total_urls = 0
    has_more = True
    
    if os.path.exists(output_file):
        try:
            with open(output_file, 'r', encoding='utf-8') as f:
                existing_data = json.load(f)
                total_urls = existing_data.get('total_urls', 0)
                page = total_urls // batch_size if batch_size > 0 else 0
                print(f"Continuing from page {page} with {total_urls} existing URLs")
        except json.JSONDecodeError:
            print(f"Warning: Could not load existing data from {output_file}, starting fresh")
    
    while has_more and (max_pages is None or page < max_pages):
        result = get_metadata(
            username=username,
            auth_token=auth_token,
            timeline_type=timeline_type,
            batch_size=batch_size,
            page=page,
            output_file=output_file
        )
        
        has_more = result["has_more"]
        total_urls = result["total_urls"]
        
        if not has_more:
            print(f"No more items to fetch. Total URLs: {total_urls}")
            break
            
        page += 1
    
    return total_urls

# Usage
if __name__ == "__main__":
    username = "takomayuyi"
    auth_token = ""
    
    # Example 1: Non-batch mode - fetch all items at once
    # This will fetch all items without pagination
    # result = get_metadata(username, auth_token, "media")
    # print(f"Non-batch mode: Added {result['new_entries']} media URLs")
    
    # Example 2: Batch mode - fetch items in pages
    # This will fetch only the specified page with the given batch size
    result = get_metadata(username, auth_token, "media", batch_size=100, page=0)
    print(f"Batch mode: Added {result['new_entries']} media URLs (page 0)")
    
    # To fetch the next page:
    # result = get_metadata(username, auth_token, "media", batch_size=100, page=1)
    
    # To automatically fetch all pages in batch mode:
    # total = continue_metadata_fetch(username, auth_token, "media", batch_size=100)
    # print(f"Total media URLs fetched across all pages: {total}")