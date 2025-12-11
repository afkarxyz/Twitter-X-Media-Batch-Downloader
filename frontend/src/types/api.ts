// Twitter/X Media Batch Downloader Types

export interface AccountInfo {
  name: string;      // username/handle from extractor
  nick: string;      // display name from extractor
  date: string;
  followers_count: number;
  friends_count: number;
  profile_image: string;
  statuses_count: number;
}

export interface TimelineEntry {
  url: string;
  date: string;
  tweet_id: string;
  type: string; // photo, video, gif, text
  is_retweet: boolean;
  extension: string;
  width: number;
  height: number;
  content?: string;
  view_count?: number;
  bookmark_count?: number;
  favorite_count?: number;
  retweet_count?: number;
  reply_count?: number;
  source?: string;
  verified?: boolean;
  original_filename?: string; // Original filename from API
  author_username?: string; // Username of tweet author (for bookmarks and likes)
}

export interface ExtractMetadata {
  new_entries: number;
  page: number;
  batch_size: number;
  has_more: boolean;
  cursor?: string;      // Cursor for resume capability
  completed?: boolean;  // True if all media fetched
}

export interface TwitterResponse {
  account_info: AccountInfo;
  total_urls: number;
  timeline: TimelineEntry[];
  metadata: ExtractMetadata;
  cursor?: string;      // Cursor for next fetch (from CLI)
  completed?: boolean;  // True if fetch completed
}

export interface TimelineRequest {
  username: string;
  auth_token: string;
  timeline_type: string; // media, timeline, tweets, with_replies, likes
  batch_size: number;
  page: number;
  media_type: string; // all, image, video, gif
  retweets: boolean;
  cursor?: string; // Resume from this cursor position
}

export interface DateRangeRequest {
  username: string;
  auth_token: string;
  start_date: string; // YYYY-MM-DD
  end_date: string; // YYYY-MM-DD
  media_filter: string;
  retweets: boolean;
}

// Settings types
export interface Settings {
  downloadPath: string;
  theme: string;
  themeMode: "auto" | "light" | "dark";
  sfxEnabled: boolean;
}
