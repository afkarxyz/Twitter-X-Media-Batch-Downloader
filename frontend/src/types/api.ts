export interface AccountInfo {
    name: string;
    nick: string;
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
    type: string;
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
    original_filename?: string;
    author_username?: string;
}
export interface ExtractMetadata {
    new_entries: number;
    page: number;
    batch_size: number;
    has_more: boolean;
    cursor?: string;
    completed?: boolean;
}
export interface TwitterResponse {
    account_info: AccountInfo;
    total_urls: number;
    timeline: TimelineEntry[];
    metadata: ExtractMetadata;
    cursor?: string;
    completed?: boolean;
}
export interface TimelineRequest {
    username: string;
    auth_token: string;
    timeline_type: string;
    batch_size: number;
    page: number;
    media_type: string;
    retweets: boolean;
    cursor?: string;
}
export interface DateRangeRequest {
    username: string;
    auth_token: string;
    start_date: string;
    end_date: string;
    media_filter: string;
    retweets: boolean;
}
export interface Settings {
    downloadPath: string;
    theme: string;
    themeMode: "auto" | "light" | "dark";
    sfxEnabled: boolean;
}
