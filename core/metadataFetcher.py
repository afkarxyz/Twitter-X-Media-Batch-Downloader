import re
import json
from datetime import datetime
from gallery_dl.extractor import twitter

def get_metadata_by_date(username, auth_token, date_start, date_end, media_filter="filter:media", output_file=None):
    query = f"from:{username} since:{date_start} until:{date_end}"
    if media_filter:
        query += f" {media_filter}"
    
    url = f"https://x.com/search?q={query}"
    
    extractor_class = twitter.TwitterSearchExtractor
    match = re.match(extractor_class.pattern, url)
    
    if not match:
        raise ValueError(f"Invalid search URL: {url}")
    
    extractor = extractor_class(match)
    
    config_dict = {
        "cookies": {
            "auth_token": auth_token
        },
        "retweets": False
    }
    
    extractor.config = lambda key, default=None: config_dict.get(key, default)
    
    try:
        extractor.initialize()
        
        api = twitter.TwitterAPI(extractor)
        
        try:
            user = api.user_by_screen_name(username)
            
            if "legacy" in user and user["legacy"].get("withheld_scope"):
                raise ValueError("withheld")
                
        except Exception as e:
            error_msg = str(e).lower()
            if "withheld" in error_msg or (hasattr(e, "response") and "withheld" in str(e.response.text).lower()):
                raise ValueError("withheld")
            raise
        
        user_data = extractor._transform_user(user)
        
        structured_output = {
            'account_info': {
                'name': user_data.get('name', ''),
                'nick': user_data.get('nick', ''),
                'date': user_data.get('date', ''),
                'followers_count': user_data.get('followers_count', 0),
                'friends_count': user_data.get('friends_count', 0),
                'profile_image': user_data.get('profile_image', ''),
                'statuses_count': user_data.get('statuses_count', 0)
            },
            'total_urls': 0,
            'timeline': [],
            'search_query': query,
            'date_filter': {
                'start': date_start,
                'end': date_end,
                'method': 'search_api'
            }
        }
        
        if isinstance(structured_output['account_info']['date'], datetime):
            structured_output['account_info']['date'] = structured_output['account_info']['date'].strftime("%Y-%m-%d %H:%M:%S")
        
        new_timeline_entries = []
        
        try:
            iterator = iter(extractor)
            
            while True:
                try:
                    item = next(iterator)
                    
                    if isinstance(item, tuple) and len(item) >= 3:
                        media_url = item[1]
                        tweet_data = item[2]
                        
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
                            
                            if 'retweet_id' in tweet_data and tweet_data['retweet_id']:
                                timeline_entry['retweet_id'] = tweet_data['retweet_id']
                                timeline_entry['is_retweet'] = True
                            else:
                                timeline_entry['is_retweet'] = False
                            
                            new_timeline_entries.append(timeline_entry)
                            structured_output['total_urls'] += 1
                            
                except StopIteration:
                    break
                    
        except Exception as e:
            pass
        
        structured_output['timeline'] = new_timeline_entries
        
        structured_output['metadata'] = {
            "new_entries": len(new_timeline_entries),
            "method": "search_api",
            "date_range": f"{date_start} to {date_end}"
        }
        
        if output_file:
            try:
                with open(output_file, 'w', encoding='utf-8') as f:
                    json.dump(structured_output, f, ensure_ascii=False, indent=2)
            except Exception as e:
                pass
        
        return structured_output
        
    except Exception as e:
        error_msg = str(e).lower()
        if "withheld" in error_msg or e.__class__.__name__ == "ValueError" and str(e) == "withheld":
            return {"error": "To download withheld accounts, use this userscript version: https://www.patreon.com/exyezed"}
        else:
            error_str = str(e)
            if error_str == "None":
                return {"error": "Failed to authenticate. Please verify your auth token is valid and not expired."}
            else:
                return {"error": error_str}

def get_metadata(username, auth_token, timeline_type="timeline", batch_size=0, page=0, media_type="all", retweets=False):
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
        },
        "retweets": retweets
    }
    
    if batch_size > 0:
        config_dict["count"] = batch_size
    
    extractor.config = lambda key, default=None: config_dict.get(key, default)
    
    try:
        extractor.initialize()
        
        api = twitter.TwitterAPI(extractor)
        try:
            if username.startswith("id:"):
                user = api.user_by_rest_id(username[3:])
            else:
                user = api.user_by_screen_name(username)
                
            if "legacy" in user and user["legacy"].get("withheld_scope"):
                raise ValueError("withheld")
                
        except Exception as e:
            error_msg = str(e).lower()
            if "withheld" in error_msg or (hasattr(e, "response") and "withheld" in str(e.response.text).lower()):
                raise ValueError("withheld")
            raise
        
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
                        
                        if 'retweet_id' in tweet_data and tweet_data['retweet_id']:
                            timeline_entry['retweet_id'] = tweet_data['retweet_id']
                            timeline_entry['is_retweet'] = True
                        else:
                            timeline_entry['is_retweet'] = False
                        
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
    
    except Exception as e:
        error_msg = str(e).lower()
        if "withheld" in error_msg or e.__class__.__name__ == "ValueError" and str(e) == "withheld":
            return {"error": "To download withheld accounts, use this userscript version: https://www.patreon.com/exyezed"}
        else:
            error_str = str(e)
            if error_str == "None":
                return {"error": "Failed to authenticate. Please verify your auth token is valid and not expired."}
            else:
                return {"error": error_str}

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
        error_str = str(e)
        if error_str == "None":
            print(json.dumps({"error": "Failed to authenticate. Please verify your auth token is valid and not expired."}, ensure_ascii=False))
        else:
            print(json.dumps({"error": error_str}, ensure_ascii=False))

if __name__ == '__main__':
    main()