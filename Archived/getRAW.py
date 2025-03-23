import json
import re
import os
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
    
    all_items = []
    if page > 0 and os.path.exists(output_file):
        try:
            with open(output_file, 'r', encoding='utf-8') as f:
                all_items = json.load(f)
            print(f"Loaded {len(all_items)} existing items from {output_file}")
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
    if batch_size > 0:
        try:
            for _ in range(batch_size):
                item = next(iterator)
                batch_items.append(item)
        except StopIteration:
            pass
    else:
        batch_items = list(iterator)
    
    all_items.extend(batch_items)
    
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(all_items, f, indent=2, default=str)
    
    if batch_size > 0:
        print(f"Page {page}: Retrieved {len(batch_items)} items. Total items: {len(all_items)}")
    else:
        print(f"Retrieved {len(batch_items)} items total")
    
    cursor_info = None
    if hasattr(extractor, '_cursor') and extractor._cursor:
        cursor_info = extractor._cursor
        print(f"Current cursor: {cursor_info}")
    
    return {
        "items": batch_items,
        "total_items": len(all_items),
        "page": page,
        "batch_size": batch_size,
        "has_more": batch_size > 0 and len(batch_items) == batch_size,
        "cursor": cursor_info
    }

def continue_metadata_fetch(username, auth_token, timeline_type="media", batch_size=100, max_pages=None):
    current_dir = os.path.dirname(os.path.abspath(__file__))
    output_file = os.path.join(current_dir, f"{username}_{timeline_type}.json")
    
    page = 0
    total_items = 0
    has_more = True
    
    if os.path.exists(output_file):
        try:
            with open(output_file, 'r', encoding='utf-8') as f:
                existing_data = json.load(f)
                total_items = len(existing_data)
                page = total_items // batch_size
                print(f"Continuing from page {page} with {total_items} existing items")
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
        total_items = result["total_items"]
        
        if not has_more:
            print(f"No more items to fetch. Total items: {total_items}")
            break
            
        page += 1
    
    return total_items

# Usage
if __name__ == "__main__":
    username = "takomayuyi"
    auth_token = ""
    
    # Example 1: Non-batch mode - fetch all items at once
    # This will fetch all items without pagination
    # result = get_metadata(username, auth_token, "media")
    # print(f"Non-batch mode: Retrieved {len(result['items'])} items")
    
    # Example 2: Batch mode - fetch items in pages
    # This will fetch only the specified page with the given batch size
    result = get_metadata(username, auth_token, "media", batch_size=100, page=0)
    print(f"Batch mode: Retrieved {len(result['items'])} items (page 0)")
    
    # To fetch the next page:
    # result = get_metadata(username, auth_token, "media", batch_size=100, page=1)
    
    # To automatically fetch all pages in batch mode:
    # total = continue_metadata_fetch(username, auth_token, "media", batch_size=100)
    # print(f"Total items fetched across all pages: {total}")