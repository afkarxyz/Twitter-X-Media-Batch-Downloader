from dataclasses import dataclass

@dataclass
class Account:
    username: str
    nick: str
    followers: int
    following: int
    posts: int
    media_type: str
    profile_image: str = None
    media_list: list = None
    fetch_mode: str = 'all'  
    fetch_timestamp: str = None
    group_id: int = None