import json
import re
import os
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
    
    all_items = []
    for item in extractor:
        all_items.append(item)
    
    current_dir = os.path.dirname(os.path.abspath(__file__))
    output_file = os.path.join(current_dir, f"{username}.json")
    
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(all_items, f, indent=2, default=str)

username = ""
get_metadata(username)
