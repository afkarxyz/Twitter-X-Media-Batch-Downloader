export namespace backend {
	
	export class AccountListItem {
	    id: number;
	    username: string;
	    name: string;
	    profile_image: string;
	    total_media: number;
	    last_fetched: string;
	    group_name: string;
	    group_color: string;
	    media_type: string;
	    cursor: string;
	    completed: boolean;
	    followers_count: number;
	    statuses_count: number;
	
	    static createFrom(source: any = {}) {
	        return new AccountListItem(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.username = source["username"];
	        this.name = source["name"];
	        this.profile_image = source["profile_image"];
	        this.total_media = source["total_media"];
	        this.last_fetched = source["last_fetched"];
	        this.group_name = source["group_name"];
	        this.group_color = source["group_color"];
	        this.media_type = source["media_type"];
	        this.cursor = source["cursor"];
	        this.completed = source["completed"];
	        this.followers_count = source["followers_count"];
	        this.statuses_count = source["statuses_count"];
	    }
	}

}

export namespace main {
	
	export class ConvertGIFsRequest {
	    folder_path: string;
	    quality: string;
	    resolution: string;
	    delete_original: boolean;
	
	    static createFrom(source: any = {}) {
	        return new ConvertGIFsRequest(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.folder_path = source["folder_path"];
	        this.quality = source["quality"];
	        this.resolution = source["resolution"];
	        this.delete_original = source["delete_original"];
	    }
	}
	export class ConvertGIFsResponse {
	    success: boolean;
	    converted: number;
	    failed: number;
	    message: string;
	
	    static createFrom(source: any = {}) {
	        return new ConvertGIFsResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.success = source["success"];
	        this.converted = source["converted"];
	        this.failed = source["failed"];
	        this.message = source["message"];
	    }
	}
	export class DateRangeRequest {
	    username: string;
	    auth_token: string;
	    start_date: string;
	    end_date: string;
	    media_filter: string;
	    retweets: boolean;
	
	    static createFrom(source: any = {}) {
	        return new DateRangeRequest(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.username = source["username"];
	        this.auth_token = source["auth_token"];
	        this.start_date = source["start_date"];
	        this.end_date = source["end_date"];
	        this.media_filter = source["media_filter"];
	        this.retweets = source["retweets"];
	    }
	}
	export class DownloadMediaRequest {
	    urls: string[];
	    output_dir: string;
	    username: string;
	    proxy?: string;
	
	    static createFrom(source: any = {}) {
	        return new DownloadMediaRequest(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.urls = source["urls"];
	        this.output_dir = source["output_dir"];
	        this.username = source["username"];
	        this.proxy = source["proxy"];
	    }
	}
	export class DownloadMediaResponse {
	    success: boolean;
	    downloaded: number;
	    skipped: number;
	    failed: number;
	    message: string;
	
	    static createFrom(source: any = {}) {
	        return new DownloadMediaResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.success = source["success"];
	        this.downloaded = source["downloaded"];
	        this.skipped = source["skipped"];
	        this.failed = source["failed"];
	        this.message = source["message"];
	    }
	}
	export class MediaItemRequest {
	    url: string;
	    date: string;
	    tweet_id: number;
	    type: string;
	    content?: string;
	    original_filename?: string;
	    author_username?: string;
	
	    static createFrom(source: any = {}) {
	        return new MediaItemRequest(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.url = source["url"];
	        this.date = source["date"];
	        this.tweet_id = source["tweet_id"];
	        this.type = source["type"];
	        this.content = source["content"];
	        this.original_filename = source["original_filename"];
	        this.author_username = source["author_username"];
	    }
	}
	export class DownloadMediaWithMetadataRequest {
	    items: MediaItemRequest[];
	    output_dir: string;
	    username: string;
	    proxy?: string;
	
	    static createFrom(source: any = {}) {
	        return new DownloadMediaWithMetadataRequest(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.items = this.convertValues(source["items"], MediaItemRequest);
	        this.output_dir = source["output_dir"];
	        this.username = source["username"];
	        this.proxy = source["proxy"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class ImportAccountResponse {
	    success: boolean;
	    username: string;
	    message: string;
	
	    static createFrom(source: any = {}) {
	        return new ImportAccountResponse(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.success = source["success"];
	        this.username = source["username"];
	        this.message = source["message"];
	    }
	}
	
	export class TimelineRequest {
	    username: string;
	    auth_token: string;
	    timeline_type: string;
	    batch_size: number;
	    page: number;
	    media_type: string;
	    retweets: boolean;
	    cursor?: string;
	
	    static createFrom(source: any = {}) {
	        return new TimelineRequest(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.username = source["username"];
	        this.auth_token = source["auth_token"];
	        this.timeline_type = source["timeline_type"];
	        this.batch_size = source["batch_size"];
	        this.page = source["page"];
	        this.media_type = source["media_type"];
	        this.retweets = source["retweets"];
	        this.cursor = source["cursor"];
	    }
	}

}

