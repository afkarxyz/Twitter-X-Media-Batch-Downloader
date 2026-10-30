//! Shared application state and commands used by the egui views and workers.
#![allow(dead_code)]
use super::*;
type Handler<F> = Option<Rc<F>>;
#[derive(Clone, Default)]
pub struct LibraryJob {
    pub key: String,
    pub status: String,
    pub media_count: i32,
    pub error: String,
}
#[derive(Clone, Default)]
pub struct LibraryData {
    pub items: ModelRc<HistoryItem>,
    pub query: String,
    pub select_mode: bool,
    pub selection: Vec<String>,
    pub groups: Vec<String>,
    pub group_filter: String,
    pub queue: Vec<LibraryJob>,
    pub account: String,
    pub title: String,
    pub folder: String,
    pub available: i32,
    pub missing: i32,
    pub pending: i32,
    pub job: String,
    pub feedback_active: bool,
    pub status: String,
    pub progress: f32,
    pub progress_detail: String,
    pub error: String,
    pub view_ready: bool,
    pub files_view: bool,
    pub workspace_active: bool,
    pub workspace_pending: bool,
    pub loading: bool,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountProfile {
    pub available: bool,
    pub display_name: String,
    pub username: String,
    pub avatar_image: Image,
    pub avatar_url: String,
    pub has_avatar: bool,
    pub avatar_square: bool,
    pub banner_image: Image,
    pub banner_url: String,
    pub has_banner: bool,
    pub verified: bool,
    pub protected: bool,
    pub bio: String,
    pub location: String,
    pub website_url: String,
    pub website_label: String,
    pub joined: String,
    pub posts: String,
    pub following: String,
    pub followers: String,
    pub media: String,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaItem {
    pub author_name: String,
    pub author_handle: String,
    pub author_verified: bool,
    pub author_avatar_url: String,
    pub author_avatar_square: bool,
    pub author_avatar_image: Image,
    pub alt_text: String,
    pub quotes: String,
    pub bookmarks: String,
    pub is_repost: bool,
    pub reposted_by: String,
    pub replies: String,
    pub reposts: String,
    pub likes: String,
    pub views: String,
    pub preview_date: String,
    pub file_extension: String,
    pub title: String,
    pub detail: String,
    pub media_type: String,
    pub status: String,
    pub url: String,
    pub tweet_url: String,
    pub preview_url: String,
    pub playback_url: String,
    pub thumbnail_url: String,
    pub preview_image: Image,
    pub detail_image: Image,
    pub tweet_text: String,
    pub created_at: String,
    pub source: String,
    pub dimensions: String,
    pub engagement: String,
    pub saved_path: String,
    pub media_index: i32,
    pub is_new: bool,
    pub payload_pending: bool,
    pub sort_key: String,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub source_url: String,
    pub media_count: i32,
    pub saved_count: i32,
    pub handle: String,
    pub avatar_image: Image,
    pub avatar_url: String,
    pub avatar_square: bool,
    pub has_avatar: bool,
    pub age_key: String,
    pub age_count: i32,
    pub fetched_at: String,
    pub output_dir: String,
    pub size_text: String,
    pub earlier_download: bool,
    pub session_count: i32,
    pub group_label: String,
    pub update_eligible: bool,
    pub account_labels: String,
    pub account_color: Color,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DownloadedFile {
    pub media_index: i32,
    pub path: String,
    pub folder: String,
    pub filename: String,
    pub kind: String,
    pub size_text: String,
    pub present: bool,
    pub integrity_state: String,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchFetchItem {
    pub index: i32,
    pub source: String,
    pub status: String,
    pub history_id: String,
    pub media_count: i32,
    pub error: String,
    pub avatar_image: Image,
    pub avatar_url: String,
    pub avatar_square: bool,
    pub has_avatar: bool,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdateInfo {
    pub name: String,
    pub installed: String,
    pub latest: String,
    pub status: String,
    pub url: String,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountSearchItem {
    pub handle: String,
    pub name: String,
    pub bio: String,
    pub followers: String,
    pub verified: bool,
    pub protected: bool,
    pub avatar_url: String,
    pub avatar: Image,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthTokenItem {
    pub index: i32,
    pub token: String,
    pub status: String,
    pub username: String,
    pub checked_at: String,
}
#[derive(Default)]
pub struct State {
    pub appwindow: AppWindowData,
    pub mediaresults: MediaResultsData,
    pub mediaselection: MediaSelectionData,
    pub previewplayback: PreviewPlaybackData,
    pub historyselection: HistorySelectionData,
    pub renderersettings: RendererSettingsData,
    pub i18n: I18nData,
    pub apptheme: AppThemeData,
}
pub struct AppWindowData {
    pub library: LibraryData,
    pub library_open: Handler<dyn Fn(String)>,
    pub library_refetch: Handler<dyn Fn(String)>,
    pub library_refetch_selected: Handler<dyn Fn()>,
    pub library_download: Handler<dyn Fn(i32)>,
    pub library_refresh: Handler<dyn Fn()>,
    pub page_changed: Handler<dyn Fn(i32)>,
    pub history_loaded: Vec<Rc<dyn Fn()>>,
    pub history_revision: i32,
    pub selected_tab: i32,
    pub home_results_hidden: bool,
    pub settings_category: i32,
    pub theme_mode: String,
    pub language_mode: String,
    pub source_url: String,
    pub batch_mode: bool,
    pub batch_running: bool,
    pub batch_result_open: bool,
    pub batch_result_index: i32,
    pub batch_pending_count: i32,
    pub batch_sources_text: String,
    pub batch_fetch_items: ModelRc<BatchFetchItem>,
    pub date_filter_enabled: bool,
    pub fetch_start_date: String,
    pub fetch_end_date: String,
    pub fetch_options_open: bool,
    pub fetch_source_index: i32,
    pub fetch_options_error: String,
    pub auth_token: String,
    pub saved_auth_token: String,
    pub auth_token_history: ModelRc<AuthTokenItem>,
    pub auth_check_busy: bool,
    pub auth_check_index: i32,
    pub auth_validation_status: String,
    pub auth_validation_username: String,
    pub auth_validation_checked_at: String,
    pub is_extracting: bool,
    pub extraction_generation: i32,
    pub media_type_index: i32,
    pub include_photos: bool,
    pub include_videos: bool,
    pub include_gifs: bool,
    pub media_filters_custom: bool,
    pub retweets_index: i32,
    pub no_videos: bool,
    pub text_tweets: bool,
    pub video_quality_index: i32,
    pub image_size_index: i32,
    pub avatar_size_index: i32,
    pub banner_size_index: i32,
    pub convert_gif: bool,
    pub gif_quality_index: i32,
    pub gif_resolution_index: i32,
    pub concurrent_downloads_index: i32,
    pub retry_attempts: i32,
    pub filename_format: String,
    pub filename_preview: String,
    pub folder_name_format: String,
    pub folder_name_preview: String,
    pub output_dir: String,
    pub is_busy: bool,
    pub progress: f32,
    pub progress_detail: String,
    pub status: String,
    pub toast_page: i32,
    pub toast_message: String,
    pub updates_busy: bool,
    pub updates_error: String,
    pub update_items: ModelRc<UpdateInfo>,
    pub account_search_open: bool,
    pub account_search_busy: bool,
    pub account_search_generation: i32,
    pub account_search_query: String,
    pub account_search_result_query: String,
    pub account_search_error: String,
    pub account_search_cursor: String,
    pub account_search_items: ModelRc<AccountSearchItem>,
    pub integrity_open: bool,
    pub integrity_busy: bool,
    pub integrity_report: String,
    pub integrity_status: String,
    pub backup_busy: bool,
    pub backup_error: String,
    pub backup_summary: String,
    pub backup_restore_open: bool,
    pub incremental_fetch: bool,
    pub notify_updates: bool,
    pub startup_update_version: String,
    pub proxy_url: String,
    pub proxy_error: String,
    pub tool_update_busy: bool,
    pub tool_update_status: String,
    pub tool_update_detail: String,
    pub tool_update_error: String,
    pub tool_update_progress: f32,
    pub last_error: String,
    pub log_viewer_open: bool,
    pub logs_busy: bool,
    pub logs_polling: bool,
    pub logs_live: bool,
    pub logs_follow: bool,
    pub logs_generation: i32,
    pub log_sessions: ModelRc<String>,
    pub log_session_index: i32,
    pub log_text: String,
    pub log_truncated: bool,
    pub logs_error: String,
    pub toast_kind: String,
    pub toast_visible: bool,
    pub toast_generation: i32,
    pub media_items: ModelRc<MediaItem>,
    pub account_profile: AccountProfile,
    pub selected_media_index: i32,
    pub history_items: ModelRc<HistoryItem>,
    pub history_selection: ModelRc<String>,
    pub history_select_mode: bool,
    pub history_groups: ModelRc<String>,
    pub history_group_filter: String,
    pub history_group_index: i32,
    pub account_labels_open: bool,
    pub account_labels_error: String,
    pub batch_history_profiles: bool,
    pub fetch_sessions_key: String,
    pub fetch_sessions_title: String,
    pub fetch_sessions_label: String,
    pub fetch_sessions_items: ModelRc<HistoryItem>,
    pub history_fetch_count: i32,
    pub history_saved_count: i32,
    pub history_show_saved: bool,
    pub history_search_query: String,
    pub history_busy: bool,
    pub download_batch_id: String,
    pub archive_conversion_busy: bool,
    pub archive_conversion_open: bool,
    pub archive_gif_count: i32,
    pub download_batch_title: String,
    pub download_batch_folder: String,
    pub download_batch_size: String,
    pub download_batch_files: ModelRc<DownloadedFile>,
    pub separate_media_folders: bool,
    pub embed_metadata: bool,
    pub fetch_timeout_secs: i32,
    pub http_timeout_secs: i32,
    pub timeout_error: String,
    pub download_speed_kib: i32,
    pub download_delay_ms: i32,
    pub download_jitter_ms: i32,
    pub download_limits_error: String,
    pub results_view: String,
    pub results_sort: i32,
    pub media_count: i32,
    pub media_count_text: String,
    pub has_output: bool,
    pub has_more: bool,
    pub app_version: String,
    pub storage_busy: bool,
    pub storage_partial: bool,
    pub storage_free: String,
    pub storage_total: String,
    pub storage_used: String,
    pub storage_other_used: String,
    pub storage_used_ratio: f32,
    pub storage_downloads_ratio: f32,
    pub storage_capacity_known: bool,
    pub storage_files: i32,
    pub ffmpeg_busy: bool,
    pub xtractor_busy: bool,
    pub xtractor_installed: bool,
    pub xtractor_setup_required: bool,
    pub xtractor_version: String,
    pub xtractor_latest: String,
    pub xtractor_update_available: bool,
    pub xtractor_status: String,
    pub xtractor_progress: f32,
    pub xtractor_detail: String,
    pub ffmpeg_ready: bool,
    pub ffmpeg_status: String,
    pub ffmpeg_progress: f32,
    pub ffmpeg_detail: String,
    pub choose_fetch_source: Handler<dyn Fn(i32)>,
    pub reset_settings: Handler<dyn Fn(i32) -> bool>,
    pub sync_fetch_source: Handler<dyn Fn(String)>,
    pub check_auth_token: Handler<dyn Fn(i32)>,
    pub use_auth_token_history: Handler<dyn Fn(i32)>,
    pub remove_auth_token_history: Handler<dyn Fn(i32)>,
    pub search_accounts: Handler<dyn Fn(String, bool)>,
    pub close_account_search: Handler<dyn Fn()>,
    pub check_downloads: Handler<dyn Fn(bool)>,
    pub backup_database: Handler<dyn Fn()>,
    pub choose_database_backup: Handler<dyn Fn()>,
    pub restore_database: Handler<dyn Fn()>,
    pub check_all_updates: Handler<dyn Fn()>,
    pub apply_proxy: Handler<dyn Fn(String) -> bool>,
    pub update_tool: Handler<dyn Fn(String)>,
    pub cancel_tool_update: Handler<dyn Fn()>,
    pub open_logs: Handler<dyn Fn()>,
    pub view_logs: Handler<dyn Fn()>,
    pub refresh_logs: Handler<dyn Fn()>,
    pub load_log: Handler<dyn Fn(i32)>,
    pub copy_log: Handler<dyn Fn()>,
    pub export_log: Handler<dyn Fn()>,
    pub export_log_errors: Handler<dyn Fn()>,
    pub copy_error: Handler<dyn Fn()>,
    pub filter_history_group: Handler<dyn Fn(i32)>,
    pub apply_account_labels: Handler<dyn Fn(String, String, String) -> bool>,
    pub export_accounts_txt: Handler<dyn Fn()>,
    pub toggle_history_selection: Handler<dyn Fn(String)>,
    pub select_visible_history: Handler<dyn Fn()>,
    pub update_history_accounts: Handler<dyn Fn(ModelRc<String>)>,
    pub apply_timeouts: Handler<dyn Fn(String, String) -> bool>,
    pub apply_download_limits: Handler<dyn Fn(String, String, String) -> bool>,
    pub start_extraction: Handler<dyn Fn()>,
    pub convert_archived_gifs: Handler<dyn Fn(bool, i32, i32)>,
    pub start_batch: Handler<dyn Fn()>,
    pub switch_home_mode: Handler<dyn Fn(bool)>,
    pub clear_batch: Handler<dyn Fn()>,
    pub reset_batch: Handler<dyn Fn()>,
    pub open_batch_result: Handler<dyn Fn(i32)>,
    pub apply_fetch_options: Handler<dyn Fn(bool, bool, String, String, String) -> bool>,
    pub clear_results: Handler<dyn Fn()>,
    pub download_profile_image: Handler<dyn Fn(String)>,
    pub dismiss_status: Handler<dyn Fn()>,
    pub paste_from_clipboard: Handler<dyn Fn()>,
    pub continue_extraction: Handler<dyn Fn()>,
    pub start_downloads: Handler<dyn Fn(String)>,
    pub download_selected_media: Handler<dyn Fn(i32)>,
    pub load_media_preview: Handler<dyn Fn(i32)>,
    pub release_preview_images: Handler<dyn Fn(i32)>,
    pub load_thumbnail: Handler<dyn Fn(i32)>,
    pub load_profile_images: Handler<dyn Fn()>,
    pub load_history_avatars: Handler<dyn Fn()>,
    pub preview_changed: Handler<dyn Fn(i32)>,
    pub open_link: Handler<dyn Fn(String)>,
    pub open_post: Handler<dyn Fn(String)>,
    pub open_history: Handler<dyn Fn(String)>,
    pub open_fetch_sessions: Handler<dyn Fn(String)>,
    pub open_download_batch: Handler<dyn Fn(String)>,
    pub open_download_path: Handler<dyn Fn(String, bool)>,
    pub clear_history: Handler<dyn Fn()>,
    pub remove_history: Handler<dyn Fn(String)>,
    pub export_history: Handler<dyn Fn(String)>,
    pub import_history: Handler<dyn Fn()>,
    pub search_history: Handler<dyn Fn(String, bool)>,
    pub cancel_operation: Handler<dyn Fn()>,
    pub dismiss_toast: Handler<dyn Fn()>,
    pub browse_output_folder: Handler<dyn Fn()>,
    pub export_json: Handler<dyn Fn()>,
    pub open_output_folder: Handler<dyn Fn()>,
    pub save_theme_mode: Handler<dyn Fn(String)>,
    pub save_theme_preset: Handler<dyn Fn(String)>,
    pub save_language: Handler<dyn Fn(String)>,
    pub save_auth_token: Handler<dyn Fn()>,
    pub settings_changed: Handler<dyn Fn()>,
    pub refresh_storage: Handler<dyn Fn()>,
    pub copy_address: Handler<dyn Fn(String)>,
    pub ensure_ffmpeg: Handler<dyn Fn()>,
    pub check_xtractor_update: Handler<dyn Fn()>,
    pub update_xtractor: Handler<dyn Fn()>,
}
impl Default for AppWindowData {
    fn default() -> Self {
        Self {
            library: LibraryData::default(),
            library_open: None,
            library_refetch: None,
            library_refetch_selected: None,
            library_download: None,
            library_refresh: None,
            page_changed: None,
            history_loaded: Vec::new(),
            history_revision: 0,
            selected_tab: 0,
            home_results_hidden: false,
            settings_category: 0,
            theme_mode: "system".into(),
            language_mode: "system".into(),
            source_url: "".into(),
            batch_mode: false,
            batch_running: false,
            batch_result_open: false,
            batch_result_index: -1,
            batch_pending_count: 0,
            batch_sources_text: "".into(),
            batch_fetch_items: Default::default(),
            date_filter_enabled: false,
            fetch_start_date: "".into(),
            fetch_end_date: "".into(),
            fetch_options_open: false,
            fetch_source_index: 0,
            fetch_options_error: "".into(),
            auth_token: "".into(),
            saved_auth_token: "".into(),
            auth_token_history: Default::default(),
            auth_check_busy: false,
            auth_check_index: -2,
            auth_validation_status: Default::default(),
            auth_validation_username: Default::default(),
            auth_validation_checked_at: Default::default(),
            is_extracting: false,
            extraction_generation: 0,
            media_type_index: 0,
            include_photos: true,
            include_videos: true,
            include_gifs: true,
            media_filters_custom: false,
            retweets_index: 0,
            no_videos: false,
            text_tweets: false,
            video_quality_index: 0,
            image_size_index: 0,
            avatar_size_index: 0,
            banner_size_index: 0,
            convert_gif: false,
            gif_quality_index: 0,
            gif_resolution_index: 0,
            concurrent_downloads_index: 0,
            retry_attempts: 2,
            filename_format: "{date}_{handle}_{tweet_id}_{index}".into(),
            filename_preview: "".into(),
            folder_name_format: "{handle}".into(),
            folder_name_preview: "".into(),
            output_dir: "".into(),
            is_busy: false,
            progress: 0.0,
            progress_detail: "".into(),
            status: "Ready for an X/Twitter URL.".into(),
            toast_page: -1,
            toast_message: "".into(),
            updates_busy: false,
            updates_error: "".into(),
            update_items: Default::default(),
            account_search_open: false,
            account_search_busy: false,
            account_search_generation: 0,
            account_search_query: "".into(),
            account_search_result_query: "".into(),
            account_search_error: "".into(),
            account_search_cursor: "".into(),
            account_search_items: Default::default(),
            integrity_open: false,
            integrity_busy: false,
            integrity_report: "".into(),
            integrity_status: "".into(),
            backup_busy: false,
            backup_error: "".into(),
            backup_summary: "".into(),
            backup_restore_open: false,
            incremental_fetch: true,
            notify_updates: true,
            startup_update_version: "".into(),
            proxy_url: "".into(),
            proxy_error: "".into(),
            tool_update_busy: false,
            tool_update_status: "".into(),
            tool_update_detail: "".into(),
            tool_update_error: "".into(),
            tool_update_progress: 0.0,
            last_error: "".into(),
            log_viewer_open: false,
            logs_busy: false,
            logs_polling: false,
            logs_live: true,
            logs_follow: true,
            logs_generation: 0,
            log_sessions: Default::default(),
            log_session_index: -1,
            log_text: "".into(),
            log_truncated: false,
            logs_error: "".into(),
            toast_kind: "info".into(),
            toast_visible: false,
            toast_generation: 0,
            media_items: Default::default(),
            account_profile: Default::default(),
            selected_media_index: -1,
            history_items: Default::default(),
            history_selection: Default::default(),
            history_select_mode: false,
            history_groups: ModelRc::new(VecModel::from(vec!["All groups".into()])),
            history_group_filter: "".into(),
            history_group_index: 0,
            account_labels_open: false,
            account_labels_error: "".into(),
            batch_history_profiles: false,
            fetch_sessions_key: "".into(),
            fetch_sessions_title: "".into(),
            fetch_sessions_label: "".into(),
            fetch_sessions_items: Default::default(),
            history_fetch_count: 0,
            history_saved_count: 0,
            history_show_saved: false,
            history_search_query: "".into(),
            history_busy: false,
            download_batch_id: "".into(),
            archive_conversion_busy: false,
            archive_conversion_open: false,
            archive_gif_count: 0,
            download_batch_title: "".into(),
            download_batch_folder: "".into(),
            download_batch_size: "".into(),
            download_batch_files: Default::default(),
            separate_media_folders: false,
            embed_metadata: false,
            fetch_timeout_secs: 330,
            http_timeout_secs: 120,
            timeout_error: "".into(),
            download_speed_kib: 0,
            download_delay_ms: 0,
            download_jitter_ms: 0,
            download_limits_error: "".into(),
            results_view: "grid".into(),
            results_sort: 0,
            media_count: 0,
            media_count_text: "0".into(),
            has_output: false,
            has_more: false,
            app_version: "".into(),
            storage_busy: false,
            storage_partial: false,
            storage_free: "—".into(),
            storage_total: "—".into(),
            storage_used: "—".into(),
            storage_other_used: "—".into(),
            storage_used_ratio: 0.0,
            storage_downloads_ratio: 0.0,
            storage_capacity_known: false,
            storage_files: 0,
            ffmpeg_busy: false,
            xtractor_busy: false,
            xtractor_installed: false,
            xtractor_setup_required: false,
            xtractor_version: "".into(),
            xtractor_latest: "".into(),
            xtractor_update_available: false,
            xtractor_status: "".into(),
            xtractor_progress: 0.0,
            xtractor_detail: "".into(),
            ffmpeg_ready: false,
            ffmpeg_status: "".into(),
            ffmpeg_progress: 0.0,
            ffmpeg_detail: "".into(),
            choose_fetch_source: None,
            reset_settings: None,
            sync_fetch_source: None,
            check_auth_token: None,
            use_auth_token_history: None,
            remove_auth_token_history: None,
            search_accounts: None,
            close_account_search: None,
            check_downloads: None,
            backup_database: None,
            choose_database_backup: None,
            restore_database: None,
            check_all_updates: None,
            apply_proxy: None,
            update_tool: None,
            cancel_tool_update: None,
            open_logs: None,
            view_logs: None,
            refresh_logs: None,
            load_log: None,
            copy_log: None,
            export_log: None,
            export_log_errors: None,
            copy_error: None,
            filter_history_group: None,
            apply_account_labels: None,
            export_accounts_txt: None,
            toggle_history_selection: None,
            select_visible_history: None,
            update_history_accounts: None,
            apply_timeouts: None,
            apply_download_limits: None,
            start_extraction: None,
            convert_archived_gifs: None,
            start_batch: None,
            switch_home_mode: None,
            clear_batch: None,
            reset_batch: None,
            open_batch_result: None,
            apply_fetch_options: None,
            clear_results: None,
            download_profile_image: None,
            dismiss_status: None,
            paste_from_clipboard: None,
            continue_extraction: None,
            start_downloads: None,
            download_selected_media: None,
            load_media_preview: None,
            release_preview_images: None,
            load_thumbnail: None,
            load_profile_images: None,
            load_history_avatars: None,
            preview_changed: None,
            open_link: None,
            open_post: None,
            open_history: None,
            open_fetch_sessions: None,
            open_download_batch: None,
            open_download_path: None,
            clear_history: None,
            remove_history: None,
            export_history: None,
            import_history: None,
            search_history: None,
            cancel_operation: None,
            dismiss_toast: None,
            browse_output_folder: None,
            export_json: None,
            open_output_folder: None,
            save_theme_mode: None,
            save_theme_preset: None,
            save_language: None,
            save_auth_token: None,
            settings_changed: None,
            refresh_storage: None,
            copy_address: None,
            ensure_ffmpeg: None,
            check_xtractor_update: None,
            update_xtractor: None,
        }
    }
}
impl AppWindow {
    pub fn get_batch_result_open(&self) -> bool {
        self.state.borrow().appwindow.batch_result_open
    }
    pub fn set_batch_result_open(&self, value: bool) {
        self.state.borrow_mut().appwindow.batch_result_open = value;
    }
    pub fn get_batch_result_index(&self) -> i32 {
        self.state.borrow().appwindow.batch_result_index
    }
    pub fn set_batch_result_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.batch_result_index = value;
    }
    pub fn on_switch_home_mode(&self, callback: impl Fn(bool) + 'static) {
        self.state.borrow_mut().appwindow.switch_home_mode = Some(Rc::new(callback));
    }
    pub fn invoke_switch_home_mode(&self, batch: bool) {
        let callback = self.state.borrow().appwindow.switch_home_mode.clone();
        if let Some(callback) = callback {
            callback(batch);
        } else {
            self.set_batch_mode(batch);
            self.set_batch_result_open(false);
        }
    }
    pub fn open_batch_history_result(&self, index: i32, id: String) {
        self.set_batch_result_index(index);
        self.set_batch_result_open(true);
        self.set_home_results_hidden(false);
        let callback = self.state.borrow().appwindow.open_history.clone();
        if let Some(callback) = callback {
            callback(id);
        }
    }
    pub fn get_home_results_hidden(&self) -> bool {
        self.state.borrow().appwindow.home_results_hidden
    }
    pub fn set_home_results_hidden(&self, value: bool) {
        self.state.borrow_mut().appwindow.home_results_hidden = value;
    }
    pub fn get_selected_tab(&self) -> i32 {
        self.state.borrow().appwindow.selected_tab
    }
    pub fn set_selected_tab(&self, value: i32) {
        if self.get_selected_tab() == value {
            return;
        }
        self.state.borrow_mut().appwindow.selected_tab = value;
        self.set_selected_media_index(-1);
        let callback = self.state.borrow().appwindow.page_changed.clone();
        if let Some(callback) = callback {
            callback(value);
        }
        self.invoke_load_history_avatars();
    }
    pub fn get_library(&self) -> LibraryData {
        self.state.borrow().appwindow.library.clone()
    }
    pub fn update_library(&self, update: impl FnOnce(&mut LibraryData)) {
        update(&mut self.state.borrow_mut().appwindow.library);
    }
    pub fn on_history_loaded(&self, callback: impl Fn() + 'static) {
        self.state
            .borrow_mut()
            .appwindow
            .history_loaded
            .push(Rc::new(callback));
    }
    pub fn invoke_history_loaded(&self) {
        let callbacks = {
            let mut state = self.state.borrow_mut();
            state.appwindow.history_revision = state.appwindow.history_revision.wrapping_add(1);
            state.appwindow.history_loaded.clone()
        };
        for callback in callbacks {
            callback();
        }
    }
    pub fn get_history_revision(&self) -> i32 {
        self.state.borrow().appwindow.history_revision
    }
    pub fn on_page_changed(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.page_changed = Some(Rc::new(callback));
    }
    pub fn on_library_open(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.library_open = Some(Rc::new(callback));
    }
    pub fn invoke_library_open(&self, key: String) {
        let callback = self.state.borrow().appwindow.library_open.clone();
        if let Some(callback) = callback {
            callback(key);
        }
    }
    pub fn on_library_refetch(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.library_refetch = Some(Rc::new(callback));
    }
    pub fn invoke_library_refetch(&self, key: String) {
        let callback = self.state.borrow().appwindow.library_refetch.clone();
        if let Some(callback) = callback {
            callback(key);
        }
    }
    pub fn on_library_refetch_selected(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.library_refetch_selected = Some(Rc::new(callback));
    }
    pub fn invoke_library_refetch_selected(&self) {
        let callback = self
            .state
            .borrow()
            .appwindow
            .library_refetch_selected
            .clone();
        if let Some(callback) = callback {
            callback();
        }
    }
    pub fn on_library_download(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.library_download = Some(Rc::new(callback));
    }
    pub fn invoke_library_download(&self, index: i32) {
        let callback = self.state.borrow().appwindow.library_download.clone();
        if let Some(callback) = callback {
            callback(index);
        }
    }
    pub fn on_library_refresh(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.library_refresh = Some(Rc::new(callback));
    }
    pub fn invoke_library_refresh(&self) {
        let callback = self.state.borrow().appwindow.library_refresh.clone();
        if let Some(callback) = callback {
            callback();
        }
    }
    pub fn get_settings_category(&self) -> i32 {
        self.state.borrow().appwindow.settings_category
    }
    pub fn set_settings_category(&self, value: i32) {
        self.state.borrow_mut().appwindow.settings_category = value;
    }
    pub fn get_theme_mode(&self) -> String {
        self.state.borrow().appwindow.theme_mode.clone()
    }
    pub fn set_theme_mode(&self, value: String) {
        self.state.borrow_mut().appwindow.theme_mode = value.clone();
    }
    pub fn get_language_mode(&self) -> String {
        self.state.borrow().appwindow.language_mode.clone()
    }
    pub fn set_language_mode(&self, value: String) {
        self.state.borrow_mut().appwindow.language_mode = value.clone();
    }
    pub fn get_source_url(&self) -> String {
        self.state.borrow().appwindow.source_url.clone()
    }
    pub fn set_result_source_url(&self, value: String) {
        self.state.borrow_mut().appwindow.source_url = value;
    }
    pub fn set_source_url(&self, value: String) {
        if self.get_source_url() == value {
            return;
        }
        self.state.borrow_mut().appwindow.source_url = value.clone();
        self.invoke_sync_fetch_source(value);
    }
    pub fn get_batch_mode(&self) -> bool {
        self.state.borrow().appwindow.batch_mode
    }
    pub fn set_batch_mode(&self, value: bool) {
        self.state.borrow_mut().appwindow.batch_mode = value;
    }
    pub fn get_batch_running(&self) -> bool {
        self.state.borrow().appwindow.batch_running
    }
    pub fn set_batch_running(&self, value: bool) {
        self.state.borrow_mut().appwindow.batch_running = value;
    }
    pub fn get_batch_pending_count(&self) -> i32 {
        self.state.borrow().appwindow.batch_pending_count
    }
    pub fn set_batch_pending_count(&self, value: i32) {
        self.state.borrow_mut().appwindow.batch_pending_count = value;
    }
    pub fn get_batch_sources_text(&self) -> String {
        self.state.borrow().appwindow.batch_sources_text.clone()
    }
    pub fn set_batch_sources_text(&self, value: String) {
        self.state.borrow_mut().appwindow.batch_sources_text = value.clone();
    }
    pub fn get_batch_fetch_items(&self) -> ModelRc<BatchFetchItem> {
        self.state.borrow().appwindow.batch_fetch_items.clone()
    }
    pub fn set_batch_fetch_items(&self, value: ModelRc<BatchFetchItem>) {
        self.state.borrow_mut().appwindow.batch_fetch_items = value.clone();
    }
    pub fn get_date_filter_enabled(&self) -> bool {
        self.state.borrow().appwindow.date_filter_enabled
    }
    pub fn set_date_filter_enabled(&self, value: bool) {
        self.state.borrow_mut().appwindow.date_filter_enabled = value;
    }
    pub fn get_fetch_start_date(&self) -> String {
        self.state.borrow().appwindow.fetch_start_date.clone()
    }
    pub fn set_fetch_start_date(&self, value: String) {
        self.state.borrow_mut().appwindow.fetch_start_date = value.clone();
    }
    pub fn get_fetch_end_date(&self) -> String {
        self.state.borrow().appwindow.fetch_end_date.clone()
    }
    pub fn set_fetch_end_date(&self, value: String) {
        self.state.borrow_mut().appwindow.fetch_end_date = value.clone();
    }
    pub fn get_fetch_options_open(&self) -> bool {
        self.state.borrow().appwindow.fetch_options_open
    }
    pub fn set_fetch_options_open(&self, value: bool) {
        self.state.borrow_mut().appwindow.fetch_options_open = value;
    }
    pub fn get_fetch_source_index(&self) -> i32 {
        self.state.borrow().appwindow.fetch_source_index
    }
    pub fn set_fetch_source_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.fetch_source_index = value;
    }
    pub fn get_fetch_options_error(&self) -> String {
        self.state.borrow().appwindow.fetch_options_error.clone()
    }
    pub fn set_fetch_options_error(&self, value: String) {
        self.state.borrow_mut().appwindow.fetch_options_error = value.clone();
    }
    pub fn get_auth_token(&self) -> String {
        self.state.borrow().appwindow.auth_token.clone()
    }
    pub fn set_auth_token(&self, value: String) {
        self.state.borrow_mut().appwindow.auth_token = value.clone();
    }
    pub fn get_saved_auth_token(&self) -> String {
        self.state.borrow().appwindow.saved_auth_token.clone()
    }
    pub fn set_saved_auth_token(&self, value: String) {
        self.state.borrow_mut().appwindow.saved_auth_token = value.clone();
    }
    pub fn get_auth_token_history(&self) -> ModelRc<AuthTokenItem> {
        self.state.borrow().appwindow.auth_token_history.clone()
    }
    pub fn set_auth_token_history(&self, value: ModelRc<AuthTokenItem>) {
        self.state.borrow_mut().appwindow.auth_token_history = value.clone();
    }
    pub fn get_auth_check_busy(&self) -> bool {
        self.state.borrow().appwindow.auth_check_busy
    }
    pub fn set_auth_check_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.auth_check_busy = value;
    }
    pub fn get_auth_check_index(&self) -> i32 {
        self.state.borrow().appwindow.auth_check_index
    }
    pub fn set_auth_check_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.auth_check_index = value;
    }
    pub fn get_auth_validation_status(&self) -> String {
        self.state.borrow().appwindow.auth_validation_status.clone()
    }
    pub fn set_auth_validation_status(&self, value: String) {
        self.state.borrow_mut().appwindow.auth_validation_status = value.clone();
    }
    pub fn get_auth_validation_username(&self) -> String {
        self.state
            .borrow()
            .appwindow
            .auth_validation_username
            .clone()
    }
    pub fn set_auth_validation_username(&self, value: String) {
        self.state.borrow_mut().appwindow.auth_validation_username = value.clone();
    }
    pub fn get_auth_validation_checked_at(&self) -> String {
        self.state
            .borrow()
            .appwindow
            .auth_validation_checked_at
            .clone()
    }
    pub fn set_auth_validation_checked_at(&self, value: String) {
        self.state.borrow_mut().appwindow.auth_validation_checked_at = value.clone();
    }
    pub fn get_is_extracting(&self) -> bool {
        self.state.borrow().appwindow.is_extracting
    }
    pub fn set_is_extracting(&self, value: bool) {
        if value
            && !self.get_is_extracting()
            && !self.get_batch_running()
            && self.get_selected_tab() != crate::library::TAB
            && self.get_library().job.is_empty()
        {
            self.set_home_results_hidden(false);
        }
        self.state.borrow_mut().appwindow.is_extracting = value;
    }
    pub fn get_extraction_generation(&self) -> i32 {
        self.state.borrow().appwindow.extraction_generation
    }
    pub fn set_extraction_generation(&self, value: i32) {
        self.state.borrow_mut().appwindow.extraction_generation = value;
    }
    pub fn get_media_type_index(&self) -> i32 {
        self.state.borrow().appwindow.media_type_index
    }
    pub fn set_media_type_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.media_type_index = value;
    }
    pub fn get_include_photos(&self) -> bool {
        self.state.borrow().appwindow.include_photos
    }
    pub fn set_include_photos(&self, value: bool) {
        self.state.borrow_mut().appwindow.include_photos = value;
    }
    pub fn get_include_videos(&self) -> bool {
        self.state.borrow().appwindow.include_videos
    }
    pub fn set_include_videos(&self, value: bool) {
        self.state.borrow_mut().appwindow.include_videos = value;
    }
    pub fn get_include_gifs(&self) -> bool {
        self.state.borrow().appwindow.include_gifs
    }
    pub fn set_include_gifs(&self, value: bool) {
        self.state.borrow_mut().appwindow.include_gifs = value;
    }
    pub fn get_media_filters_custom(&self) -> bool {
        self.state.borrow().appwindow.media_filters_custom
    }
    pub fn set_media_filters_custom(&self, value: bool) {
        self.state.borrow_mut().appwindow.media_filters_custom = value;
    }
    pub fn get_retweets_index(&self) -> i32 {
        self.state.borrow().appwindow.retweets_index
    }
    pub fn set_retweets_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.retweets_index = value;
    }
    pub fn get_no_videos(&self) -> bool {
        self.state.borrow().appwindow.no_videos
    }
    pub fn set_no_videos(&self, value: bool) {
        self.state.borrow_mut().appwindow.no_videos = value;
    }
    pub fn get_text_tweets(&self) -> bool {
        self.state.borrow().appwindow.text_tweets
    }
    pub fn set_text_tweets(&self, value: bool) {
        self.state.borrow_mut().appwindow.text_tweets = value;
    }
    pub fn get_video_quality_index(&self) -> i32 {
        self.state.borrow().appwindow.video_quality_index
    }
    pub fn set_video_quality_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.video_quality_index = value;
    }
    pub fn get_image_size_index(&self) -> i32 {
        self.state.borrow().appwindow.image_size_index
    }
    pub fn set_image_size_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.image_size_index = value;
    }
    pub fn get_avatar_size_index(&self) -> i32 {
        self.state.borrow().appwindow.avatar_size_index
    }
    pub fn set_avatar_size_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.avatar_size_index = value;
    }
    pub fn get_banner_size_index(&self) -> i32 {
        self.state.borrow().appwindow.banner_size_index
    }
    pub fn set_banner_size_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.banner_size_index = value;
    }
    pub fn get_convert_gif(&self) -> bool {
        self.state.borrow().appwindow.convert_gif
    }
    pub fn set_convert_gif(&self, value: bool) {
        self.state.borrow_mut().appwindow.convert_gif = value;
    }
    pub fn get_gif_quality_index(&self) -> i32 {
        self.state.borrow().appwindow.gif_quality_index
    }
    pub fn set_gif_quality_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.gif_quality_index = value;
    }
    pub fn get_gif_resolution_index(&self) -> i32 {
        self.state.borrow().appwindow.gif_resolution_index
    }
    pub fn set_gif_resolution_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.gif_resolution_index = value;
    }
    pub fn get_concurrent_downloads_index(&self) -> i32 {
        self.state.borrow().appwindow.concurrent_downloads_index
    }
    pub fn set_concurrent_downloads_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.concurrent_downloads_index = value;
    }
    pub fn get_retry_attempts(&self) -> i32 {
        self.state.borrow().appwindow.retry_attempts
    }
    pub fn set_retry_attempts(&self, value: i32) {
        self.state.borrow_mut().appwindow.retry_attempts = value;
    }
    pub fn get_filename_format(&self) -> String {
        self.state.borrow().appwindow.filename_format.clone()
    }
    pub fn set_filename_format(&self, value: String) {
        self.state.borrow_mut().appwindow.filename_format = value.clone();
    }
    pub fn get_filename_preview(&self) -> String {
        self.state.borrow().appwindow.filename_preview.clone()
    }
    pub fn set_filename_preview(&self, value: String) {
        self.state.borrow_mut().appwindow.filename_preview = value.clone();
    }
    pub fn get_folder_name_format(&self) -> String {
        self.state.borrow().appwindow.folder_name_format.clone()
    }
    pub fn set_folder_name_format(&self, value: String) {
        self.state.borrow_mut().appwindow.folder_name_format = value.clone();
    }
    pub fn get_folder_name_preview(&self) -> String {
        self.state.borrow().appwindow.folder_name_preview.clone()
    }
    pub fn set_folder_name_preview(&self, value: String) {
        self.state.borrow_mut().appwindow.folder_name_preview = value.clone();
    }
    pub fn get_output_dir(&self) -> String {
        self.state.borrow().appwindow.output_dir.clone()
    }
    pub fn set_output_dir(&self, value: String) {
        self.state.borrow_mut().appwindow.output_dir = value.clone();
    }
    pub fn get_is_busy(&self) -> bool {
        self.state.borrow().appwindow.is_busy
    }
    pub fn set_is_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.is_busy = value;
    }
    pub fn get_progress(&self) -> f32 {
        self.state.borrow().appwindow.progress
    }
    pub fn set_progress(&self, value: f32) {
        let mut state = self.state.borrow_mut();
        if state.appwindow.library.feedback_active {
            state.appwindow.library.progress = value;
        } else {
            state.appwindow.progress = value;
        }
    }
    pub fn get_progress_detail(&self) -> String {
        self.state.borrow().appwindow.progress_detail.clone()
    }
    pub fn set_progress_detail(&self, value: String) {
        let mut state = self.state.borrow_mut();
        if state.appwindow.library.feedback_active {
            state.appwindow.library.progress_detail = value;
        } else {
            state.appwindow.progress_detail = value;
        }
    }
    pub fn get_status(&self) -> String {
        self.state.borrow().appwindow.status.clone()
    }
    pub fn set_status(&self, value: String) {
        let mut state = self.state.borrow_mut();
        if state.appwindow.library.feedback_active {
            state.appwindow.library.status = value;
        } else {
            state.appwindow.status = value;
        }
    }
    pub fn get_toast_page(&self) -> i32 {
        self.state.borrow().appwindow.toast_page
    }
    pub fn set_toast_page(&self, value: i32) {
        self.state.borrow_mut().appwindow.toast_page = value;
    }
    pub fn get_toast_message(&self) -> String {
        self.state.borrow().appwindow.toast_message.clone()
    }
    pub fn set_toast_message(&self, value: String) {
        self.state.borrow_mut().appwindow.toast_message = value.clone();
    }
    pub fn get_updates_busy(&self) -> bool {
        self.state.borrow().appwindow.updates_busy
    }
    pub fn set_updates_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.updates_busy = value;
    }
    pub fn get_updates_error(&self) -> String {
        self.state.borrow().appwindow.updates_error.clone()
    }
    pub fn set_updates_error(&self, value: String) {
        self.state.borrow_mut().appwindow.updates_error = value.clone();
    }
    pub fn get_update_items(&self) -> ModelRc<UpdateInfo> {
        self.state.borrow().appwindow.update_items.clone()
    }
    pub fn set_update_items(&self, value: ModelRc<UpdateInfo>) {
        self.state.borrow_mut().appwindow.update_items = value.clone();
    }
    pub fn get_account_search_open(&self) -> bool {
        self.state.borrow().appwindow.account_search_open
    }
    pub fn set_account_search_open(&self, value: bool) {
        self.state.borrow_mut().appwindow.account_search_open = value;
    }
    pub fn get_account_search_busy(&self) -> bool {
        self.state.borrow().appwindow.account_search_busy
    }
    pub fn set_account_search_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.account_search_busy = value;
    }
    pub fn get_account_search_generation(&self) -> i32 {
        self.state.borrow().appwindow.account_search_generation
    }
    pub fn set_account_search_generation(&self, value: i32) {
        self.state.borrow_mut().appwindow.account_search_generation = value;
    }
    pub fn get_account_search_query(&self) -> String {
        self.state.borrow().appwindow.account_search_query.clone()
    }
    pub fn set_account_search_query(&self, value: String) {
        self.state.borrow_mut().appwindow.account_search_query = value.clone();
    }
    pub fn get_account_search_result_query(&self) -> String {
        self.state
            .borrow()
            .appwindow
            .account_search_result_query
            .clone()
    }
    pub fn set_account_search_result_query(&self, value: String) {
        self.state
            .borrow_mut()
            .appwindow
            .account_search_result_query = value.clone();
    }
    pub fn get_account_search_error(&self) -> String {
        self.state.borrow().appwindow.account_search_error.clone()
    }
    pub fn set_account_search_error(&self, value: String) {
        self.state.borrow_mut().appwindow.account_search_error = value.clone();
    }
    pub fn get_account_search_cursor(&self) -> String {
        self.state.borrow().appwindow.account_search_cursor.clone()
    }
    pub fn set_account_search_cursor(&self, value: String) {
        self.state.borrow_mut().appwindow.account_search_cursor = value.clone();
    }
    pub fn get_account_search_items(&self) -> ModelRc<AccountSearchItem> {
        self.state.borrow().appwindow.account_search_items.clone()
    }
    pub fn set_account_search_items(&self, value: ModelRc<AccountSearchItem>) {
        self.state.borrow_mut().appwindow.account_search_items = value.clone();
    }
    pub fn get_integrity_open(&self) -> bool {
        self.state.borrow().appwindow.integrity_open
    }
    pub fn set_integrity_open(&self, value: bool) {
        self.state.borrow_mut().appwindow.integrity_open = value;
    }
    pub fn get_integrity_busy(&self) -> bool {
        self.state.borrow().appwindow.integrity_busy
    }
    pub fn set_integrity_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.integrity_busy = value;
    }
    pub fn get_integrity_report(&self) -> String {
        self.state.borrow().appwindow.integrity_report.clone()
    }
    pub fn set_integrity_report(&self, value: String) {
        self.state.borrow_mut().appwindow.integrity_report = value.clone();
    }
    pub fn get_integrity_status(&self) -> String {
        self.state.borrow().appwindow.integrity_status.clone()
    }
    pub fn set_integrity_status(&self, value: String) {
        self.state.borrow_mut().appwindow.integrity_status = value.clone();
    }
    pub fn get_backup_busy(&self) -> bool {
        self.state.borrow().appwindow.backup_busy
    }
    pub fn set_backup_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.backup_busy = value;
    }
    pub fn get_backup_error(&self) -> String {
        self.state.borrow().appwindow.backup_error.clone()
    }
    pub fn set_backup_error(&self, value: String) {
        self.state.borrow_mut().appwindow.backup_error = value.clone();
    }
    pub fn get_backup_summary(&self) -> String {
        self.state.borrow().appwindow.backup_summary.clone()
    }
    pub fn set_backup_summary(&self, value: String) {
        self.state.borrow_mut().appwindow.backup_summary = value.clone();
    }
    pub fn get_backup_restore_open(&self) -> bool {
        self.state.borrow().appwindow.backup_restore_open
    }
    pub fn set_backup_restore_open(&self, value: bool) {
        self.state.borrow_mut().appwindow.backup_restore_open = value;
    }
    pub fn get_incremental_fetch(&self) -> bool {
        self.state.borrow().appwindow.incremental_fetch
    }
    pub fn set_incremental_fetch(&self, value: bool) {
        self.state.borrow_mut().appwindow.incremental_fetch = value;
    }
    pub fn get_notify_updates(&self) -> bool {
        self.state.borrow().appwindow.notify_updates
    }
    pub fn set_notify_updates(&self, value: bool) {
        self.state.borrow_mut().appwindow.notify_updates = value;
    }
    pub fn get_startup_update_version(&self) -> String {
        self.state.borrow().appwindow.startup_update_version.clone()
    }
    pub fn set_startup_update_version(&self, value: String) {
        self.state.borrow_mut().appwindow.startup_update_version = value.clone();
    }
    pub fn get_proxy_url(&self) -> String {
        self.state.borrow().appwindow.proxy_url.clone()
    }
    pub fn set_proxy_url(&self, value: String) {
        self.state.borrow_mut().appwindow.proxy_url = value.clone();
    }
    pub fn get_proxy_error(&self) -> String {
        self.state.borrow().appwindow.proxy_error.clone()
    }
    pub fn set_proxy_error(&self, value: String) {
        self.state.borrow_mut().appwindow.proxy_error = value.clone();
    }
    pub fn get_tool_update_busy(&self) -> bool {
        self.state.borrow().appwindow.tool_update_busy
    }
    pub fn set_tool_update_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.tool_update_busy = value;
    }
    pub fn get_tool_update_status(&self) -> String {
        self.state.borrow().appwindow.tool_update_status.clone()
    }
    pub fn set_tool_update_status(&self, value: String) {
        self.state.borrow_mut().appwindow.tool_update_status = value.clone();
    }
    pub fn get_tool_update_detail(&self) -> String {
        self.state.borrow().appwindow.tool_update_detail.clone()
    }
    pub fn set_tool_update_detail(&self, value: String) {
        self.state.borrow_mut().appwindow.tool_update_detail = value.clone();
    }
    pub fn get_tool_update_error(&self) -> String {
        self.state.borrow().appwindow.tool_update_error.clone()
    }
    pub fn set_tool_update_error(&self, value: String) {
        self.state.borrow_mut().appwindow.tool_update_error = value.clone();
    }
    pub fn get_tool_update_progress(&self) -> f32 {
        self.state.borrow().appwindow.tool_update_progress
    }
    pub fn set_tool_update_progress(&self, value: f32) {
        self.state.borrow_mut().appwindow.tool_update_progress = value;
    }
    pub fn get_last_error(&self) -> String {
        self.state.borrow().appwindow.last_error.clone()
    }
    pub fn set_last_error(&self, value: String) {
        self.state.borrow_mut().appwindow.last_error = value.clone();
    }
    pub fn get_log_viewer_open(&self) -> bool {
        self.state.borrow().appwindow.log_viewer_open
    }
    pub fn set_log_viewer_open(&self, value: bool) {
        self.state.borrow_mut().appwindow.log_viewer_open = value;
    }
    pub fn get_logs_busy(&self) -> bool {
        self.state.borrow().appwindow.logs_busy
    }
    pub fn set_logs_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.logs_busy = value;
    }
    pub fn get_logs_polling(&self) -> bool {
        self.state.borrow().appwindow.logs_polling
    }
    pub fn set_logs_polling(&self, value: bool) {
        self.state.borrow_mut().appwindow.logs_polling = value;
    }
    pub fn get_logs_live(&self) -> bool {
        self.state.borrow().appwindow.logs_live
    }
    pub fn set_logs_live(&self, value: bool) {
        self.state.borrow_mut().appwindow.logs_live = value;
    }
    pub fn get_logs_follow(&self) -> bool {
        self.state.borrow().appwindow.logs_follow
    }
    pub fn set_logs_follow(&self, value: bool) {
        self.state.borrow_mut().appwindow.logs_follow = value;
    }
    pub fn get_logs_generation(&self) -> i32 {
        self.state.borrow().appwindow.logs_generation
    }
    pub fn set_logs_generation(&self, value: i32) {
        self.state.borrow_mut().appwindow.logs_generation = value;
    }
    pub fn get_log_sessions(&self) -> ModelRc<String> {
        self.state.borrow().appwindow.log_sessions.clone()
    }
    pub fn set_log_sessions(&self, value: ModelRc<String>) {
        self.state.borrow_mut().appwindow.log_sessions = value.clone();
    }
    pub fn get_log_session_index(&self) -> i32 {
        self.state.borrow().appwindow.log_session_index
    }
    pub fn set_log_session_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.log_session_index = value;
    }
    pub fn get_log_text(&self) -> String {
        self.state.borrow().appwindow.log_text.clone()
    }
    pub fn set_log_text(&self, value: String) {
        self.state.borrow_mut().appwindow.log_text = value.clone();
    }
    pub fn get_log_truncated(&self) -> bool {
        self.state.borrow().appwindow.log_truncated
    }
    pub fn set_log_truncated(&self, value: bool) {
        self.state.borrow_mut().appwindow.log_truncated = value;
    }
    pub fn get_logs_error(&self) -> String {
        self.state.borrow().appwindow.logs_error.clone()
    }
    pub fn set_logs_error(&self, value: String) {
        self.state.borrow_mut().appwindow.logs_error = value.clone();
    }
    pub fn get_toast_kind(&self) -> String {
        self.state.borrow().appwindow.toast_kind.clone()
    }
    pub fn set_toast_kind(&self, value: String) {
        self.state.borrow_mut().appwindow.toast_kind = value.clone();
    }
    pub fn get_toast_visible(&self) -> bool {
        self.state.borrow().appwindow.toast_visible
    }
    pub fn set_toast_visible(&self, value: bool) {
        self.state.borrow_mut().appwindow.toast_visible = value;
    }
    pub fn get_toast_generation(&self) -> i32 {
        self.state.borrow().appwindow.toast_generation
    }
    pub fn set_toast_generation(&self, value: i32) {
        self.state.borrow_mut().appwindow.toast_generation = value;
    }
    pub fn get_media_items(&self) -> ModelRc<MediaItem> {
        self.state.borrow().appwindow.media_items.clone()
    }
    pub fn set_media_items(&self, value: ModelRc<MediaItem>) {
        self.state.borrow_mut().appwindow.media_items = value.clone();
    }
    pub fn get_account_profile(&self) -> AccountProfile {
        self.state.borrow().appwindow.account_profile.clone()
    }
    pub fn set_account_profile(&self, value: AccountProfile) {
        if self.get_account_profile() == value {
            return;
        }
        self.state.borrow_mut().appwindow.account_profile = value;
        self.invoke_load_profile_images();
    }
    pub fn get_selected_media_index(&self) -> i32 {
        self.state.borrow().appwindow.selected_media_index
    }
    pub fn set_selected_media_index(&self, value: i32) {
        if self.get_selected_media_index() == value {
            return;
        }
        self.state.borrow_mut().appwindow.selected_media_index = value;
        self.invoke_preview_changed(value);
        self.invoke_release_preview_images(value);
    }
    pub fn get_history_items(&self) -> ModelRc<HistoryItem> {
        self.state.borrow().appwindow.history_items.clone()
    }
    pub fn set_history_items(&self, value: ModelRc<HistoryItem>) {
        self.state.borrow_mut().appwindow.history_items = value.clone();
    }
    pub fn get_history_selection(&self) -> ModelRc<String> {
        self.state.borrow().appwindow.history_selection.clone()
    }
    pub fn set_history_selection(&self, value: ModelRc<String>) {
        self.state.borrow_mut().appwindow.history_selection = value.clone();
    }
    pub fn get_history_select_mode(&self) -> bool {
        self.state.borrow().appwindow.history_select_mode
    }
    pub fn set_history_select_mode(&self, value: bool) {
        self.state.borrow_mut().appwindow.history_select_mode = value;
    }
    pub fn get_history_groups(&self) -> ModelRc<String> {
        self.state.borrow().appwindow.history_groups.clone()
    }
    pub fn set_history_groups(&self, value: ModelRc<String>) {
        self.state.borrow_mut().appwindow.history_groups = value.clone();
    }
    pub fn get_history_group_filter(&self) -> String {
        self.state.borrow().appwindow.history_group_filter.clone()
    }
    pub fn set_history_group_filter(&self, value: String) {
        self.state.borrow_mut().appwindow.history_group_filter = value.clone();
    }
    pub fn get_history_group_index(&self) -> i32 {
        self.state.borrow().appwindow.history_group_index
    }
    pub fn set_history_group_index(&self, value: i32) {
        self.state.borrow_mut().appwindow.history_group_index = value;
    }
    pub fn get_account_labels_open(&self) -> bool {
        self.state.borrow().appwindow.account_labels_open
    }
    pub fn set_account_labels_open(&self, value: bool) {
        self.state.borrow_mut().appwindow.account_labels_open = value;
    }
    pub fn get_account_labels_error(&self) -> String {
        self.state.borrow().appwindow.account_labels_error.clone()
    }
    pub fn set_account_labels_error(&self, value: String) {
        self.state.borrow_mut().appwindow.account_labels_error = value.clone();
    }
    pub fn get_batch_history_profiles(&self) -> bool {
        self.state.borrow().appwindow.batch_history_profiles
    }
    pub fn set_batch_history_profiles(&self, value: bool) {
        self.state.borrow_mut().appwindow.batch_history_profiles = value;
    }
    pub fn get_fetch_sessions_key(&self) -> String {
        self.state.borrow().appwindow.fetch_sessions_key.clone()
    }
    pub fn set_fetch_sessions_key(&self, value: String) {
        self.state.borrow_mut().appwindow.fetch_sessions_key = value.clone();
    }
    pub fn get_fetch_sessions_title(&self) -> String {
        self.state.borrow().appwindow.fetch_sessions_title.clone()
    }
    pub fn set_fetch_sessions_title(&self, value: String) {
        self.state.borrow_mut().appwindow.fetch_sessions_title = value.clone();
    }
    pub fn get_fetch_sessions_label(&self) -> String {
        self.state.borrow().appwindow.fetch_sessions_label.clone()
    }
    pub fn set_fetch_sessions_label(&self, value: String) {
        self.state.borrow_mut().appwindow.fetch_sessions_label = value.clone();
    }
    pub fn get_fetch_sessions_items(&self) -> ModelRc<HistoryItem> {
        self.state.borrow().appwindow.fetch_sessions_items.clone()
    }
    pub fn set_fetch_sessions_items(&self, value: ModelRc<HistoryItem>) {
        self.state.borrow_mut().appwindow.fetch_sessions_items = value.clone();
    }
    pub fn get_history_fetch_count(&self) -> i32 {
        self.state.borrow().appwindow.history_fetch_count
    }
    pub fn set_history_fetch_count(&self, value: i32) {
        self.state.borrow_mut().appwindow.history_fetch_count = value;
    }
    pub fn get_history_saved_count(&self) -> i32 {
        self.state.borrow().appwindow.history_saved_count
    }
    pub fn set_history_saved_count(&self, value: i32) {
        self.state.borrow_mut().appwindow.history_saved_count = value;
    }
    pub fn get_history_show_saved(&self) -> bool {
        self.state.borrow().appwindow.history_show_saved
    }
    pub fn set_history_show_saved(&self, value: bool) {
        self.state.borrow_mut().appwindow.history_show_saved = value;
    }
    pub fn get_history_search_query(&self) -> String {
        self.state.borrow().appwindow.history_search_query.clone()
    }
    pub fn set_history_search_query(&self, value: String) {
        self.state.borrow_mut().appwindow.history_search_query = value.clone();
    }
    pub fn get_history_busy(&self) -> bool {
        self.state.borrow().appwindow.history_busy
    }
    pub fn set_history_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.history_busy = value;
    }
    pub fn get_download_batch_id(&self) -> String {
        self.state.borrow().appwindow.download_batch_id.clone()
    }
    pub fn set_download_batch_id(&self, value: String) {
        self.state.borrow_mut().appwindow.download_batch_id = value.clone();
    }
    pub fn get_archive_conversion_busy(&self) -> bool {
        self.state.borrow().appwindow.archive_conversion_busy
    }
    pub fn set_archive_conversion_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.archive_conversion_busy = value;
    }
    pub fn get_archive_conversion_open(&self) -> bool {
        self.state.borrow().appwindow.archive_conversion_open
    }
    pub fn set_archive_conversion_open(&self, value: bool) {
        self.state.borrow_mut().appwindow.archive_conversion_open = value;
    }
    pub fn get_archive_gif_count(&self) -> i32 {
        self.state.borrow().appwindow.archive_gif_count
    }
    pub fn set_archive_gif_count(&self, value: i32) {
        self.state.borrow_mut().appwindow.archive_gif_count = value;
    }
    pub fn get_download_batch_title(&self) -> String {
        self.state.borrow().appwindow.download_batch_title.clone()
    }
    pub fn set_download_batch_title(&self, value: String) {
        self.state.borrow_mut().appwindow.download_batch_title = value.clone();
    }
    pub fn get_download_batch_folder(&self) -> String {
        self.state.borrow().appwindow.download_batch_folder.clone()
    }
    pub fn set_download_batch_folder(&self, value: String) {
        self.state.borrow_mut().appwindow.download_batch_folder = value.clone();
    }
    pub fn get_download_batch_size(&self) -> String {
        self.state.borrow().appwindow.download_batch_size.clone()
    }
    pub fn set_download_batch_size(&self, value: String) {
        self.state.borrow_mut().appwindow.download_batch_size = value.clone();
    }
    pub fn get_download_batch_files(&self) -> ModelRc<DownloadedFile> {
        self.state.borrow().appwindow.download_batch_files.clone()
    }
    pub fn set_download_batch_files(&self, value: ModelRc<DownloadedFile>) {
        self.state.borrow_mut().appwindow.download_batch_files = value.clone();
    }
    pub fn get_separate_media_folders(&self) -> bool {
        self.state.borrow().appwindow.separate_media_folders
    }
    pub fn set_separate_media_folders(&self, value: bool) {
        self.state.borrow_mut().appwindow.separate_media_folders = value;
    }
    pub fn get_embed_metadata(&self) -> bool {
        self.state.borrow().appwindow.embed_metadata
    }
    pub fn set_embed_metadata(&self, value: bool) {
        self.state.borrow_mut().appwindow.embed_metadata = value;
    }
    pub fn get_fetch_timeout_secs(&self) -> i32 {
        self.state.borrow().appwindow.fetch_timeout_secs
    }
    pub fn set_fetch_timeout_secs(&self, value: i32) {
        self.state.borrow_mut().appwindow.fetch_timeout_secs = value;
    }
    pub fn get_http_timeout_secs(&self) -> i32 {
        self.state.borrow().appwindow.http_timeout_secs
    }
    pub fn set_http_timeout_secs(&self, value: i32) {
        self.state.borrow_mut().appwindow.http_timeout_secs = value;
    }
    pub fn get_timeout_error(&self) -> String {
        self.state.borrow().appwindow.timeout_error.clone()
    }
    pub fn set_timeout_error(&self, value: String) {
        self.state.borrow_mut().appwindow.timeout_error = value.clone();
    }
    pub fn get_download_speed_kib(&self) -> i32 {
        self.state.borrow().appwindow.download_speed_kib
    }
    pub fn set_download_speed_kib(&self, value: i32) {
        self.state.borrow_mut().appwindow.download_speed_kib = value;
    }
    pub fn get_download_delay_ms(&self) -> i32 {
        self.state.borrow().appwindow.download_delay_ms
    }
    pub fn set_download_delay_ms(&self, value: i32) {
        self.state.borrow_mut().appwindow.download_delay_ms = value;
    }
    pub fn get_download_jitter_ms(&self) -> i32 {
        self.state.borrow().appwindow.download_jitter_ms
    }
    pub fn set_download_jitter_ms(&self, value: i32) {
        self.state.borrow_mut().appwindow.download_jitter_ms = value;
    }
    pub fn get_download_limits_error(&self) -> String {
        self.state.borrow().appwindow.download_limits_error.clone()
    }
    pub fn set_download_limits_error(&self, value: String) {
        self.state.borrow_mut().appwindow.download_limits_error = value.clone();
    }
    pub fn get_results_view(&self) -> String {
        self.state.borrow().appwindow.results_view.clone()
    }
    pub fn set_results_view(&self, value: String) {
        self.state.borrow_mut().appwindow.results_view = value.clone();
    }
    pub fn get_results_sort(&self) -> i32 {
        self.state.borrow().appwindow.results_sort
    }
    pub fn set_results_sort(&self, value: i32) {
        self.state.borrow_mut().appwindow.results_sort = value;
    }
    pub fn get_media_count(&self) -> i32 {
        self.state.borrow().appwindow.media_count
    }
    pub fn set_media_count(&self, value: i32) {
        self.state.borrow_mut().appwindow.media_count = value;
    }
    pub fn get_media_count_text(&self) -> String {
        self.state.borrow().appwindow.media_count_text.clone()
    }
    pub fn set_media_count_text(&self, value: String) {
        self.state.borrow_mut().appwindow.media_count_text = value.clone();
    }
    pub fn get_has_output(&self) -> bool {
        self.state.borrow().appwindow.has_output
    }
    pub fn set_has_output(&self, value: bool) {
        self.state.borrow_mut().appwindow.has_output = value;
    }
    pub fn get_has_more(&self) -> bool {
        self.state.borrow().appwindow.has_more
    }
    pub fn set_has_more(&self, value: bool) {
        self.state.borrow_mut().appwindow.has_more = value;
    }
    pub fn get_app_version(&self) -> String {
        self.state.borrow().appwindow.app_version.clone()
    }
    pub fn set_app_version(&self, value: String) {
        self.state.borrow_mut().appwindow.app_version = value.clone();
    }
    pub fn get_storage_busy(&self) -> bool {
        self.state.borrow().appwindow.storage_busy
    }
    pub fn set_storage_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.storage_busy = value;
    }
    pub fn get_storage_partial(&self) -> bool {
        self.state.borrow().appwindow.storage_partial
    }
    pub fn set_storage_partial(&self, value: bool) {
        self.state.borrow_mut().appwindow.storage_partial = value;
    }
    pub fn get_storage_free(&self) -> String {
        self.state.borrow().appwindow.storage_free.clone()
    }
    pub fn set_storage_free(&self, value: String) {
        self.state.borrow_mut().appwindow.storage_free = value.clone();
    }
    pub fn get_storage_total(&self) -> String {
        self.state.borrow().appwindow.storage_total.clone()
    }
    pub fn set_storage_total(&self, value: String) {
        self.state.borrow_mut().appwindow.storage_total = value.clone();
    }
    pub fn get_storage_used(&self) -> String {
        self.state.borrow().appwindow.storage_used.clone()
    }
    pub fn set_storage_used(&self, value: String) {
        self.state.borrow_mut().appwindow.storage_used = value.clone();
    }
    pub fn get_storage_other_used(&self) -> String {
        self.state.borrow().appwindow.storage_other_used.clone()
    }
    pub fn set_storage_other_used(&self, value: String) {
        self.state.borrow_mut().appwindow.storage_other_used = value.clone();
    }
    pub fn get_storage_used_ratio(&self) -> f32 {
        self.state.borrow().appwindow.storage_used_ratio
    }
    pub fn set_storage_used_ratio(&self, value: f32) {
        self.state.borrow_mut().appwindow.storage_used_ratio = value;
    }
    pub fn get_storage_downloads_ratio(&self) -> f32 {
        self.state.borrow().appwindow.storage_downloads_ratio
    }
    pub fn set_storage_downloads_ratio(&self, value: f32) {
        self.state.borrow_mut().appwindow.storage_downloads_ratio = value;
    }
    pub fn get_storage_capacity_known(&self) -> bool {
        self.state.borrow().appwindow.storage_capacity_known
    }
    pub fn set_storage_capacity_known(&self, value: bool) {
        self.state.borrow_mut().appwindow.storage_capacity_known = value;
    }
    pub fn get_storage_files(&self) -> i32 {
        self.state.borrow().appwindow.storage_files
    }
    pub fn set_storage_files(&self, value: i32) {
        self.state.borrow_mut().appwindow.storage_files = value;
    }
    pub fn get_ffmpeg_busy(&self) -> bool {
        self.state.borrow().appwindow.ffmpeg_busy
    }
    pub fn set_ffmpeg_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.ffmpeg_busy = value;
    }
    pub fn get_xtractor_busy(&self) -> bool {
        self.state.borrow().appwindow.xtractor_busy
    }
    pub fn set_xtractor_busy(&self, value: bool) {
        self.state.borrow_mut().appwindow.xtractor_busy = value;
    }
    pub fn get_xtractor_installed(&self) -> bool {
        self.state.borrow().appwindow.xtractor_installed
    }
    pub fn get_xtractor_setup_required(&self) -> bool {
        self.state.borrow().appwindow.xtractor_setup_required
    }
    pub fn set_xtractor_setup_required(&self, value: bool) {
        self.state.borrow_mut().appwindow.xtractor_setup_required = value;
    }
    pub fn set_xtractor_installed(&self, value: bool) {
        self.state.borrow_mut().appwindow.xtractor_installed = value;
    }
    pub fn get_xtractor_version(&self) -> String {
        self.state.borrow().appwindow.xtractor_version.clone()
    }
    pub fn set_xtractor_version(&self, value: String) {
        self.state.borrow_mut().appwindow.xtractor_version = value.clone();
    }
    pub fn get_xtractor_latest(&self) -> String {
        self.state.borrow().appwindow.xtractor_latest.clone()
    }
    pub fn set_xtractor_latest(&self, value: String) {
        self.state.borrow_mut().appwindow.xtractor_latest = value.clone();
    }
    pub fn get_xtractor_update_available(&self) -> bool {
        self.state.borrow().appwindow.xtractor_update_available
    }
    pub fn set_xtractor_update_available(&self, value: bool) {
        self.state.borrow_mut().appwindow.xtractor_update_available = value;
    }
    pub fn get_xtractor_status(&self) -> String {
        self.state.borrow().appwindow.xtractor_status.clone()
    }
    pub fn set_xtractor_status(&self, value: String) {
        self.state.borrow_mut().appwindow.xtractor_status = value.clone();
    }
    pub fn get_xtractor_progress(&self) -> f32 {
        self.state.borrow().appwindow.xtractor_progress
    }
    pub fn set_xtractor_progress(&self, value: f32) {
        self.state.borrow_mut().appwindow.xtractor_progress = value;
    }
    pub fn get_xtractor_detail(&self) -> String {
        self.state.borrow().appwindow.xtractor_detail.clone()
    }
    pub fn set_xtractor_detail(&self, value: String) {
        self.state.borrow_mut().appwindow.xtractor_detail = value.clone();
    }
    pub fn get_ffmpeg_ready(&self) -> bool {
        self.state.borrow().appwindow.ffmpeg_ready
    }
    pub fn set_ffmpeg_ready(&self, value: bool) {
        self.state.borrow_mut().appwindow.ffmpeg_ready = value;
    }
    pub fn get_ffmpeg_status(&self) -> String {
        self.state.borrow().appwindow.ffmpeg_status.clone()
    }
    pub fn set_ffmpeg_status(&self, value: String) {
        self.state.borrow_mut().appwindow.ffmpeg_status = value.clone();
    }
    pub fn get_ffmpeg_progress(&self) -> f32 {
        self.state.borrow().appwindow.ffmpeg_progress
    }
    pub fn set_ffmpeg_progress(&self, value: f32) {
        self.state.borrow_mut().appwindow.ffmpeg_progress = value;
    }
    pub fn get_ffmpeg_detail(&self) -> String {
        self.state.borrow().appwindow.ffmpeg_detail.clone()
    }
    pub fn set_ffmpeg_detail(&self, value: String) {
        self.state.borrow_mut().appwindow.ffmpeg_detail = value.clone();
    }
    pub fn on_choose_fetch_source(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.choose_fetch_source = Some(Rc::new(callback));
    }
    pub fn invoke_choose_fetch_source(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.choose_fetch_source.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_reset_settings(&self, callback: impl Fn(i32) -> bool + 'static) {
        self.state.borrow_mut().appwindow.reset_settings = Some(Rc::new(callback));
    }
    pub fn invoke_reset_settings(&self, arg0: i32) -> bool {
        let callback = self.state.borrow().appwindow.reset_settings.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_sync_fetch_source(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.sync_fetch_source = Some(Rc::new(callback));
    }
    pub fn invoke_sync_fetch_source(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.sync_fetch_source.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_check_auth_token(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.check_auth_token = Some(Rc::new(callback));
    }
    pub fn invoke_check_auth_token(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.check_auth_token.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_use_auth_token_history(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.use_auth_token_history = Some(Rc::new(callback));
    }
    pub fn invoke_use_auth_token_history(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.use_auth_token_history.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_remove_auth_token_history(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.remove_auth_token_history = Some(Rc::new(callback));
    }
    pub fn invoke_remove_auth_token_history(&self, arg0: i32) {
        let callback = self
            .state
            .borrow()
            .appwindow
            .remove_auth_token_history
            .clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_search_accounts(&self, callback: impl Fn(String, bool) + 'static) {
        self.state.borrow_mut().appwindow.search_accounts = Some(Rc::new(callback));
    }
    pub fn invoke_search_accounts(&self, arg0: String, arg1: bool) {
        let callback = self.state.borrow().appwindow.search_accounts.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_close_account_search(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.close_account_search = Some(Rc::new(callback));
    }
    pub fn invoke_close_account_search(&self) {
        let callback = self.state.borrow().appwindow.close_account_search.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_check_downloads(&self, callback: impl Fn(bool) + 'static) {
        self.state.borrow_mut().appwindow.check_downloads = Some(Rc::new(callback));
    }
    pub fn invoke_check_downloads(&self, arg0: bool) {
        let callback = self.state.borrow().appwindow.check_downloads.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_backup_database(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.backup_database = Some(Rc::new(callback));
    }
    pub fn invoke_backup_database(&self) {
        let callback = self.state.borrow().appwindow.backup_database.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_choose_database_backup(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.choose_database_backup = Some(Rc::new(callback));
    }
    pub fn invoke_choose_database_backup(&self) {
        let callback = self.state.borrow().appwindow.choose_database_backup.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_restore_database(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.restore_database = Some(Rc::new(callback));
    }
    pub fn invoke_restore_database(&self) {
        let callback = self.state.borrow().appwindow.restore_database.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_check_all_updates(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.check_all_updates = Some(Rc::new(callback));
    }
    pub fn invoke_check_all_updates(&self) {
        let callback = self.state.borrow().appwindow.check_all_updates.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_apply_proxy(&self, callback: impl Fn(String) -> bool + 'static) {
        self.state.borrow_mut().appwindow.apply_proxy = Some(Rc::new(callback));
    }
    pub fn invoke_apply_proxy(&self, arg0: String) -> bool {
        let callback = self.state.borrow().appwindow.apply_proxy.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_update_tool(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.update_tool = Some(Rc::new(callback));
    }
    pub fn invoke_update_tool(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.update_tool.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_cancel_tool_update(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.cancel_tool_update = Some(Rc::new(callback));
    }
    pub fn invoke_cancel_tool_update(&self) {
        let callback = self.state.borrow().appwindow.cancel_tool_update.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_open_logs(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.open_logs = Some(Rc::new(callback));
    }
    pub fn invoke_open_logs(&self) {
        let callback = self.state.borrow().appwindow.open_logs.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_view_logs(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.view_logs = Some(Rc::new(callback));
    }
    pub fn invoke_view_logs(&self) {
        let callback = self.state.borrow().appwindow.view_logs.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_refresh_logs(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.refresh_logs = Some(Rc::new(callback));
    }
    pub fn invoke_refresh_logs(&self) {
        let callback = self.state.borrow().appwindow.refresh_logs.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_load_log(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.load_log = Some(Rc::new(callback));
    }
    pub fn invoke_load_log(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.load_log.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_copy_log(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.copy_log = Some(Rc::new(callback));
    }
    pub fn invoke_copy_log(&self) {
        let callback = self.state.borrow().appwindow.copy_log.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_export_log(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.export_log = Some(Rc::new(callback));
    }
    pub fn invoke_export_log(&self) {
        let callback = self.state.borrow().appwindow.export_log.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_export_log_errors(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.export_log_errors = Some(Rc::new(callback));
    }
    pub fn invoke_export_log_errors(&self) {
        let callback = self.state.borrow().appwindow.export_log_errors.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_copy_error(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.copy_error = Some(Rc::new(callback));
    }
    pub fn invoke_copy_error(&self) {
        let callback = self.state.borrow().appwindow.copy_error.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_filter_history_group(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.filter_history_group = Some(Rc::new(callback));
    }
    pub fn invoke_filter_history_group(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.filter_history_group.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_apply_account_labels(
        &self,
        callback: impl Fn(String, String, String) -> bool + 'static,
    ) {
        self.state.borrow_mut().appwindow.apply_account_labels = Some(Rc::new(callback));
    }
    pub fn invoke_apply_account_labels(&self, arg0: String, arg1: String, arg2: String) -> bool {
        let callback = self.state.borrow().appwindow.apply_account_labels.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2))
            .unwrap_or_default()
    }
    pub fn on_export_accounts_txt(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.export_accounts_txt = Some(Rc::new(callback));
    }
    pub fn invoke_export_accounts_txt(&self) {
        let callback = self.state.borrow().appwindow.export_accounts_txt.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_toggle_history_selection(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.toggle_history_selection = Some(Rc::new(callback));
    }
    pub fn invoke_toggle_history_selection(&self, arg0: String) {
        let callback = self
            .state
            .borrow()
            .appwindow
            .toggle_history_selection
            .clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_select_visible_history(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.select_visible_history = Some(Rc::new(callback));
    }
    pub fn invoke_select_visible_history(&self) {
        let callback = self.state.borrow().appwindow.select_visible_history.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_update_history_accounts(&self, callback: impl Fn(ModelRc<String>) + 'static) {
        self.state.borrow_mut().appwindow.update_history_accounts = Some(Rc::new(callback));
    }
    pub fn invoke_update_history_accounts(&self, arg0: ModelRc<String>) {
        let callback = self
            .state
            .borrow()
            .appwindow
            .update_history_accounts
            .clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_apply_timeouts(&self, callback: impl Fn(String, String) -> bool + 'static) {
        self.state.borrow_mut().appwindow.apply_timeouts = Some(Rc::new(callback));
    }
    pub fn invoke_apply_timeouts(&self, arg0: String, arg1: String) -> bool {
        let callback = self.state.borrow().appwindow.apply_timeouts.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_apply_download_limits(
        &self,
        callback: impl Fn(String, String, String) -> bool + 'static,
    ) {
        self.state.borrow_mut().appwindow.apply_download_limits = Some(Rc::new(callback));
    }
    pub fn invoke_apply_download_limits(&self, arg0: String, arg1: String, arg2: String) -> bool {
        let callback = self.state.borrow().appwindow.apply_download_limits.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2))
            .unwrap_or_default()
    }
    pub fn on_start_extraction(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.start_extraction = Some(Rc::new(callback));
    }
    pub fn invoke_start_extraction(&self) {
        self.set_home_results_hidden(false);
        let callback = self.state.borrow().appwindow.start_extraction.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_convert_archived_gifs(&self, callback: impl Fn(bool, i32, i32) + 'static) {
        self.state.borrow_mut().appwindow.convert_archived_gifs = Some(Rc::new(callback));
    }
    pub fn invoke_convert_archived_gifs(&self, arg0: bool, arg1: i32, arg2: i32) {
        let callback = self.state.borrow().appwindow.convert_archived_gifs.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2))
            .unwrap_or_default()
    }
    pub fn on_start_batch(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.start_batch = Some(Rc::new(callback));
    }
    pub fn invoke_start_batch(&self) {
        self.set_home_results_hidden(false);
        let callback = self.state.borrow().appwindow.start_batch.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_clear_batch(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.clear_batch = Some(Rc::new(callback));
    }
    pub fn invoke_clear_batch(&self) {
        let callback = self.state.borrow().appwindow.clear_batch.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_reset_batch(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.reset_batch = Some(Rc::new(callback));
    }
    pub fn invoke_reset_batch(&self) {
        let callback = self.state.borrow().appwindow.reset_batch.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_open_batch_result(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.open_batch_result = Some(Rc::new(callback));
    }
    pub fn invoke_open_batch_result(&self, arg0: i32) {
        self.set_home_results_hidden(false);
        let callback = self.state.borrow().appwindow.open_batch_result.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_apply_fetch_options(
        &self,
        callback: impl Fn(bool, bool, String, String, String) -> bool + 'static,
    ) {
        self.state.borrow_mut().appwindow.apply_fetch_options = Some(Rc::new(callback));
    }
    pub fn invoke_apply_fetch_options(
        &self,
        arg0: bool,
        arg1: bool,
        arg2: String,
        arg3: String,
        arg4: String,
    ) -> bool {
        let callback = self.state.borrow().appwindow.apply_fetch_options.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2, arg3, arg4))
            .unwrap_or_default()
    }
    pub fn on_clear_results(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.clear_results = Some(Rc::new(callback));
    }
    pub fn invoke_clear_results(&self) {
        let callback = self.state.borrow().appwindow.clear_results.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_download_profile_image(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.download_profile_image = Some(Rc::new(callback));
    }
    pub fn invoke_download_profile_image(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.download_profile_image.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_dismiss_status(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.dismiss_status = Some(Rc::new(callback));
    }
    pub fn invoke_dismiss_status(&self) {
        let callback = self.state.borrow().appwindow.dismiss_status.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_paste_from_clipboard(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.paste_from_clipboard = Some(Rc::new(callback));
    }
    pub fn invoke_paste_from_clipboard(&self) {
        let callback = self.state.borrow().appwindow.paste_from_clipboard.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_continue_extraction(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.continue_extraction = Some(Rc::new(callback));
    }
    pub fn invoke_continue_extraction(&self) {
        let callback = self.state.borrow().appwindow.continue_extraction.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_start_downloads(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.start_downloads = Some(Rc::new(callback));
    }
    pub fn invoke_start_downloads(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.start_downloads.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_download_selected_media(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.download_selected_media = Some(Rc::new(callback));
    }
    pub fn invoke_download_selected_media(&self, arg0: i32) {
        if self.get_selected_tab() == crate::library::TAB && self.get_library().view_ready {
            self.invoke_library_download(arg0);
            return;
        }
        self.invoke_home_download_selected_media(arg0);
    }
    pub fn invoke_home_download_selected_media(&self, arg0: i32) {
        let callback = self
            .state
            .borrow()
            .appwindow
            .download_selected_media
            .clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_load_media_preview(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.load_media_preview = Some(Rc::new(callback));
    }
    pub fn invoke_load_media_preview(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.load_media_preview.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_release_preview_images(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.release_preview_images = Some(Rc::new(callback));
    }
    pub fn invoke_release_preview_images(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.release_preview_images.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_load_thumbnail(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.load_thumbnail = Some(Rc::new(callback));
    }
    pub fn invoke_load_thumbnail(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.load_thumbnail.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_load_profile_images(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.load_profile_images = Some(Rc::new(callback));
    }
    pub fn invoke_load_profile_images(&self) {
        let callback = self.state.borrow().appwindow.load_profile_images.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_load_history_avatars(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.load_history_avatars = Some(Rc::new(callback));
    }
    pub fn invoke_load_history_avatars(&self) {
        let callback = self.state.borrow().appwindow.load_history_avatars.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_preview_changed(&self, callback: impl Fn(i32) + 'static) {
        self.state.borrow_mut().appwindow.preview_changed = Some(Rc::new(callback));
    }
    pub fn invoke_preview_changed(&self, arg0: i32) {
        let callback = self.state.borrow().appwindow.preview_changed.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_open_link(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.open_link = Some(Rc::new(callback));
    }
    pub fn invoke_open_link(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.open_link.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_open_post(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.open_post = Some(Rc::new(callback));
    }
    pub fn invoke_open_post(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.open_post.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_open_history(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.open_history = Some(Rc::new(callback));
    }
    pub fn invoke_open_history(&self, arg0: String) {
        self.set_batch_result_open(false);
        self.set_batch_result_index(-1);
        self.set_home_results_hidden(false);
        let callback = self.state.borrow().appwindow.open_history.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_open_fetch_sessions(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.open_fetch_sessions = Some(Rc::new(callback));
    }
    pub fn invoke_open_fetch_sessions(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.open_fetch_sessions.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_open_download_batch(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.open_download_batch = Some(Rc::new(callback));
    }
    pub fn invoke_open_download_batch(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.open_download_batch.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_open_download_path(&self, callback: impl Fn(String, bool) + 'static) {
        self.state.borrow_mut().appwindow.open_download_path = Some(Rc::new(callback));
    }
    pub fn invoke_open_download_path(&self, arg0: String, arg1: bool) {
        let callback = self.state.borrow().appwindow.open_download_path.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_clear_history(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.clear_history = Some(Rc::new(callback));
    }
    pub fn invoke_clear_history(&self) {
        let callback = self.state.borrow().appwindow.clear_history.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_remove_history(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.remove_history = Some(Rc::new(callback));
    }
    pub fn invoke_remove_history(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.remove_history.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_export_history(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.export_history = Some(Rc::new(callback));
    }
    pub fn invoke_export_history(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.export_history.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_import_history(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.import_history = Some(Rc::new(callback));
    }
    pub fn invoke_import_history(&self) {
        let callback = self.state.borrow().appwindow.import_history.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_search_history(&self, callback: impl Fn(String, bool) + 'static) {
        self.state.borrow_mut().appwindow.search_history = Some(Rc::new(callback));
    }
    pub fn invoke_search_history(&self, arg0: String, arg1: bool) {
        let callback = self.state.borrow().appwindow.search_history.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_cancel_operation(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.cancel_operation = Some(Rc::new(callback));
    }
    pub fn invoke_cancel_operation(&self) {
        let callback = self.state.borrow().appwindow.cancel_operation.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_dismiss_toast(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.dismiss_toast = Some(Rc::new(callback));
    }
    pub fn invoke_dismiss_toast(&self) {
        let callback = self.state.borrow().appwindow.dismiss_toast.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_browse_output_folder(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.browse_output_folder = Some(Rc::new(callback));
    }
    pub fn invoke_browse_output_folder(&self) {
        let callback = self.state.borrow().appwindow.browse_output_folder.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_export_json(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.export_json = Some(Rc::new(callback));
    }
    pub fn invoke_export_json(&self) {
        let callback = self.state.borrow().appwindow.export_json.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_open_output_folder(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.open_output_folder = Some(Rc::new(callback));
    }
    pub fn invoke_open_output_folder(&self) {
        let callback = self.state.borrow().appwindow.open_output_folder.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_save_theme_mode(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.save_theme_mode = Some(Rc::new(callback));
    }
    pub fn invoke_save_theme_mode(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.save_theme_mode.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_save_theme_preset(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.save_theme_preset = Some(Rc::new(callback));
    }
    pub fn invoke_save_theme_preset(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.save_theme_preset.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_save_language(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.save_language = Some(Rc::new(callback));
    }
    pub fn invoke_save_language(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.save_language.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_save_auth_token(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.save_auth_token = Some(Rc::new(callback));
    }
    pub fn invoke_save_auth_token(&self) {
        let callback = self.state.borrow().appwindow.save_auth_token.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_settings_changed(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.settings_changed = Some(Rc::new(callback));
    }
    pub fn invoke_settings_changed(&self) {
        let callback = self.state.borrow().appwindow.settings_changed.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_refresh_storage(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.refresh_storage = Some(Rc::new(callback));
    }
    pub fn invoke_refresh_storage(&self) {
        let callback = self.state.borrow().appwindow.refresh_storage.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_copy_address(&self, callback: impl Fn(String) + 'static) {
        self.state.borrow_mut().appwindow.copy_address = Some(Rc::new(callback));
    }
    pub fn invoke_copy_address(&self, arg0: String) {
        let callback = self.state.borrow().appwindow.copy_address.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_ensure_ffmpeg(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.ensure_ffmpeg = Some(Rc::new(callback));
    }
    pub fn invoke_ensure_ffmpeg(&self) {
        let callback = self.state.borrow().appwindow.ensure_ffmpeg.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_check_xtractor_update(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.check_xtractor_update = Some(Rc::new(callback));
    }
    pub fn invoke_check_xtractor_update(&self) {
        let callback = self.state.borrow().appwindow.check_xtractor_update.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_update_xtractor(&self, callback: impl Fn() + 'static) {
        self.state.borrow_mut().appwindow.update_xtractor = Some(Rc::new(callback));
    }
    pub fn invoke_update_xtractor(&self) {
        let callback = self.state.borrow().appwindow.update_xtractor.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
}
#[derive(Default)]
pub struct MediaResultsData {
    pub matches_kind: Handler<dyn Fn(String, String) -> bool>,
    pub toggle_kind: Handler<dyn Fn(String, String) -> String>,
    pub filter: Handler<dyn Fn(ModelRc<MediaItem>, String, i32) -> ModelRc<MediaItem>>,
    pub count: Handler<dyn Fn(ModelRc<MediaItem>, String) -> i32>,
    pub ordered: Handler<dyn Fn(ModelRc<MediaItem>, String, i32, i32) -> ModelRc<MediaItem>>,
    pub new_count: Handler<dyn Fn(ModelRc<MediaItem>) -> i32>,
    pub position: Handler<dyn Fn(ModelRc<MediaItem>, i32) -> i32>,
    pub at: Handler<dyn Fn(ModelRc<MediaItem>, i32) -> MediaItem>,
}
#[derive(Clone)]
pub struct MediaResults {
    app: AppWindow,
}
impl Global for MediaResults {
    fn from_app(app: &AppWindow) -> Self {
        Self { app: app.clone() }
    }
}
impl MediaResults {
    pub fn on_matches_kind(&self, callback: impl Fn(String, String) -> bool + 'static) {
        self.app.state.borrow_mut().mediaresults.matches_kind = Some(Rc::new(callback));
    }
    pub fn invoke_matches_kind(&self, arg0: String, arg1: String) -> bool {
        let callback = self.app.state.borrow().mediaresults.matches_kind.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_toggle_kind(&self, callback: impl Fn(String, String) -> String + 'static) {
        self.app.state.borrow_mut().mediaresults.toggle_kind = Some(Rc::new(callback));
    }
    pub fn invoke_toggle_kind(&self, arg0: String, arg1: String) -> String {
        let callback = self.app.state.borrow().mediaresults.toggle_kind.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_filter(
        &self,
        callback: impl Fn(ModelRc<MediaItem>, String, i32) -> ModelRc<MediaItem> + 'static,
    ) {
        self.app.state.borrow_mut().mediaresults.filter = Some(Rc::new(callback));
    }
    pub fn invoke_filter(
        &self,
        arg0: ModelRc<MediaItem>,
        arg1: String,
        arg2: i32,
    ) -> ModelRc<MediaItem> {
        let callback = self.app.state.borrow().mediaresults.filter.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2))
            .unwrap_or_default()
    }
    pub fn on_count(&self, callback: impl Fn(ModelRc<MediaItem>, String) -> i32 + 'static) {
        self.app.state.borrow_mut().mediaresults.count = Some(Rc::new(callback));
    }
    pub fn invoke_count(&self, arg0: ModelRc<MediaItem>, arg1: String) -> i32 {
        let callback = self.app.state.borrow().mediaresults.count.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_ordered(
        &self,
        callback: impl Fn(ModelRc<MediaItem>, String, i32, i32) -> ModelRc<MediaItem> + 'static,
    ) {
        self.app.state.borrow_mut().mediaresults.ordered = Some(Rc::new(callback));
    }
    pub fn invoke_ordered(
        &self,
        arg0: ModelRc<MediaItem>,
        arg1: String,
        arg2: i32,
        arg3: i32,
    ) -> ModelRc<MediaItem> {
        let callback = self.app.state.borrow().mediaresults.ordered.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2, arg3))
            .unwrap_or_default()
    }
    pub fn on_new_count(&self, callback: impl Fn(ModelRc<MediaItem>) -> i32 + 'static) {
        self.app.state.borrow_mut().mediaresults.new_count = Some(Rc::new(callback));
    }
    pub fn invoke_new_count(&self, arg0: ModelRc<MediaItem>) -> i32 {
        let callback = self.app.state.borrow().mediaresults.new_count.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_position(&self, callback: impl Fn(ModelRc<MediaItem>, i32) -> i32 + 'static) {
        self.app.state.borrow_mut().mediaresults.position = Some(Rc::new(callback));
    }
    pub fn invoke_position(&self, arg0: ModelRc<MediaItem>, arg1: i32) -> i32 {
        let callback = self.app.state.borrow().mediaresults.position.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_at(&self, callback: impl Fn(ModelRc<MediaItem>, i32) -> MediaItem + 'static) {
        self.app.state.borrow_mut().mediaresults.at = Some(Rc::new(callback));
    }
    pub fn invoke_at(&self, arg0: ModelRc<MediaItem>, arg1: i32) -> MediaItem {
        let callback = self.app.state.borrow().mediaresults.at.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
}
#[derive(Default)]
pub struct MediaSelectionData {
    pub indices: ModelRc<i32>,
    pub count: i32,
    pub is_selected: Handler<dyn Fn(ModelRc<i32>, i32) -> bool>,
    pub all_selected: Handler<dyn Fn(ModelRc<i32>, ModelRc<MediaItem>, String) -> bool>,
    pub toggle: Handler<dyn Fn(i32)>,
    pub toggle_filter: Handler<dyn Fn(String)>,
    pub clear: Handler<dyn Fn()>,
}
#[derive(Clone)]
pub struct MediaSelection {
    app: AppWindow,
}
impl Global for MediaSelection {
    fn from_app(app: &AppWindow) -> Self {
        Self { app: app.clone() }
    }
}
impl MediaSelection {
    pub fn get_indices(&self) -> ModelRc<i32> {
        self.app.state.borrow().mediaselection.indices.clone()
    }
    pub fn set_indices(&self, value: ModelRc<i32>) {
        self.app.state.borrow_mut().mediaselection.indices = value.clone();
    }
    pub fn get_count(&self) -> i32 {
        self.get_indices().row_count() as i32
    }
    pub fn set_count(&self, value: i32) {
        self.app.state.borrow_mut().mediaselection.count = value;
    }
    pub fn on_is_selected(&self, callback: impl Fn(ModelRc<i32>, i32) -> bool + 'static) {
        self.app.state.borrow_mut().mediaselection.is_selected = Some(Rc::new(callback));
    }
    pub fn invoke_is_selected(&self, arg0: ModelRc<i32>, arg1: i32) -> bool {
        let callback = self.app.state.borrow().mediaselection.is_selected.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_all_selected(
        &self,
        callback: impl Fn(ModelRc<i32>, ModelRc<MediaItem>, String) -> bool + 'static,
    ) {
        self.app.state.borrow_mut().mediaselection.all_selected = Some(Rc::new(callback));
    }
    pub fn invoke_all_selected(
        &self,
        arg0: ModelRc<i32>,
        arg1: ModelRc<MediaItem>,
        arg2: String,
    ) -> bool {
        let callback = self.app.state.borrow().mediaselection.all_selected.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2))
            .unwrap_or_default()
    }
    pub fn on_toggle(&self, callback: impl Fn(i32) + 'static) {
        self.app.state.borrow_mut().mediaselection.toggle = Some(Rc::new(callback));
    }
    pub fn invoke_toggle(&self, arg0: i32) {
        let callback = self.app.state.borrow().mediaselection.toggle.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_toggle_filter(&self, callback: impl Fn(String) + 'static) {
        self.app.state.borrow_mut().mediaselection.toggle_filter = Some(Rc::new(callback));
    }
    pub fn invoke_toggle_filter(&self, arg0: String) {
        let callback = self.app.state.borrow().mediaselection.toggle_filter.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_clear(&self, callback: impl Fn() + 'static) {
        self.app.state.borrow_mut().mediaselection.clear = Some(Rc::new(callback));
    }
    pub fn invoke_clear(&self) {
        let callback = self.app.state.borrow().mediaselection.clear.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
}
pub struct PreviewPlaybackData {
    pub current_index: i32,
    pub frame: Option<Arc<egui::ColorImage>>,
    pub frame_count: i32,
    pub loading: bool,
    pub playing: bool,
    pub ready: bool,
    pub can_seek: bool,
    pub muted: bool,
    pub volume: f32,
    pub position: f32,
    pub duration: f32,
    pub position_text: String,
    pub duration_text: String,
    pub error: String,
    pub toggle: Handler<dyn Fn()>,
    pub seek: Handler<dyn Fn(f32)>,
    pub mute: Handler<dyn Fn(bool)>,
    pub change_volume: Handler<dyn Fn(f32)>,
    pub retry: Handler<dyn Fn()>,
}
impl Default for PreviewPlaybackData {
    fn default() -> Self {
        Self {
            current_index: -1,
            frame: Default::default(),
            frame_count: 0,
            loading: false,
            playing: false,
            ready: false,
            can_seek: false,
            muted: false,
            volume: 1.0,
            position: 0.0,
            duration: 0.0,
            position_text: "0:00".into(),
            duration_text: "0:00".into(),
            error: "".into(),
            toggle: None,
            seek: None,
            mute: None,
            change_volume: None,
            retry: None,
        }
    }
}
#[derive(Clone)]
pub struct PreviewPlayback {
    app: AppWindow,
}
impl Global for PreviewPlayback {
    fn from_app(app: &AppWindow) -> Self {
        Self { app: app.clone() }
    }
}
impl PreviewPlayback {
    pub fn get_current_index(&self) -> i32 {
        self.app.state.borrow().previewplayback.current_index
    }
    pub fn set_current_index(&self, value: i32) {
        self.app.state.borrow_mut().previewplayback.current_index = value;
    }
    pub fn get_frame(&self) -> Option<Arc<egui::ColorImage>> {
        self.app.state.borrow().previewplayback.frame.clone()
    }
    pub fn set_frame(&self, value: Option<Arc<egui::ColorImage>>) {
        self.app.state.borrow_mut().previewplayback.frame = value;
    }
    pub fn get_frame_count(&self) -> i32 {
        self.app.state.borrow().previewplayback.frame_count
    }
    pub fn set_frame_count(&self, value: i32) {
        self.app.state.borrow_mut().previewplayback.frame_count = value;
    }
    pub fn get_loading(&self) -> bool {
        self.app.state.borrow().previewplayback.loading
    }
    pub fn set_loading(&self, value: bool) {
        self.app.state.borrow_mut().previewplayback.loading = value;
    }
    pub fn get_playing(&self) -> bool {
        self.app.state.borrow().previewplayback.playing
    }
    pub fn set_playing(&self, value: bool) {
        self.app.state.borrow_mut().previewplayback.playing = value;
    }
    pub fn get_ready(&self) -> bool {
        self.app.state.borrow().previewplayback.ready
    }
    pub fn set_ready(&self, value: bool) {
        self.app.state.borrow_mut().previewplayback.ready = value;
    }
    pub fn get_can_seek(&self) -> bool {
        self.app.state.borrow().previewplayback.can_seek
    }
    pub fn set_can_seek(&self, value: bool) {
        self.app.state.borrow_mut().previewplayback.can_seek = value;
    }
    pub fn get_muted(&self) -> bool {
        self.app.state.borrow().previewplayback.muted
    }
    pub fn set_muted(&self, value: bool) {
        self.app.state.borrow_mut().previewplayback.muted = value;
    }
    pub fn get_volume(&self) -> f32 {
        self.app.state.borrow().previewplayback.volume
    }
    pub fn set_volume(&self, value: f32) {
        self.app.state.borrow_mut().previewplayback.volume = value;
    }
    pub fn get_position(&self) -> f32 {
        self.app.state.borrow().previewplayback.position
    }
    pub fn set_position(&self, value: f32) {
        self.app.state.borrow_mut().previewplayback.position = value;
    }
    pub fn get_duration(&self) -> f32 {
        self.app.state.borrow().previewplayback.duration
    }
    pub fn set_duration(&self, value: f32) {
        self.app.state.borrow_mut().previewplayback.duration = value;
    }
    pub fn get_position_text(&self) -> String {
        self.app
            .state
            .borrow()
            .previewplayback
            .position_text
            .clone()
    }
    pub fn set_position_text(&self, value: String) {
        self.app.state.borrow_mut().previewplayback.position_text = value.clone();
    }
    pub fn get_duration_text(&self) -> String {
        self.app
            .state
            .borrow()
            .previewplayback
            .duration_text
            .clone()
    }
    pub fn set_duration_text(&self, value: String) {
        self.app.state.borrow_mut().previewplayback.duration_text = value.clone();
    }
    pub fn get_error(&self) -> String {
        self.app.state.borrow().previewplayback.error.clone()
    }
    pub fn set_error(&self, value: String) {
        self.app.state.borrow_mut().previewplayback.error = value.clone();
    }
    pub fn on_toggle(&self, callback: impl Fn() + 'static) {
        self.app.state.borrow_mut().previewplayback.toggle = Some(Rc::new(callback));
    }
    pub fn invoke_toggle(&self) {
        let callback = self.app.state.borrow().previewplayback.toggle.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
    pub fn on_seek(&self, callback: impl Fn(f32) + 'static) {
        self.app.state.borrow_mut().previewplayback.seek = Some(Rc::new(callback));
    }
    pub fn invoke_seek(&self, arg0: f32) {
        let callback = self.app.state.borrow().previewplayback.seek.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_mute(&self, callback: impl Fn(bool) + 'static) {
        self.app.state.borrow_mut().previewplayback.mute = Some(Rc::new(callback));
    }
    pub fn invoke_mute(&self, arg0: bool) {
        let callback = self.app.state.borrow().previewplayback.mute.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_change_volume(&self, callback: impl Fn(f32) + 'static) {
        self.app.state.borrow_mut().previewplayback.change_volume = Some(Rc::new(callback));
    }
    pub fn invoke_change_volume(&self, arg0: f32) {
        let callback = self
            .app
            .state
            .borrow()
            .previewplayback
            .change_volume
            .clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_retry(&self, callback: impl Fn() + 'static) {
        self.app.state.borrow_mut().previewplayback.retry = Some(Rc::new(callback));
    }
    pub fn invoke_retry(&self) {
        let callback = self.app.state.borrow().previewplayback.retry.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
}
#[derive(Default)]
pub struct HistorySelectionData {
    pub is_selected: Handler<dyn Fn(ModelRc<String>, String) -> bool>,
}
#[derive(Clone)]
pub struct HistorySelection {
    app: AppWindow,
}
impl Global for HistorySelection {
    fn from_app(app: &AppWindow) -> Self {
        Self { app: app.clone() }
    }
}
impl HistorySelection {
    pub fn on_is_selected(&self, callback: impl Fn(ModelRc<String>, String) -> bool + 'static) {
        self.app.state.borrow_mut().historyselection.is_selected = Some(Rc::new(callback));
    }
    pub fn invoke_is_selected(&self, arg0: ModelRc<String>, arg1: String) -> bool {
        let callback = self.app.state.borrow().historyselection.is_selected.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
}
pub struct RendererSettingsData {
    pub preferred: String,
    pub active: String,
    pub gpu_supported: bool,
    pub error: String,
    pub prompt_open: bool,
    pub restarting: bool,
    pub restart_required: bool,
    pub choose: Handler<dyn Fn(String)>,
    pub restart_now: Handler<dyn Fn()>,
}
impl Default for RendererSettingsData {
    fn default() -> Self {
        Self {
            preferred: "gpu".into(),
            active: "gpu".into(),
            gpu_supported: false,
            error: "".into(),
            prompt_open: false,
            restarting: false,
            restart_required: Default::default(),
            choose: None,
            restart_now: None,
        }
    }
}
#[derive(Clone)]
pub struct RendererSettings {
    app: AppWindow,
}
impl Global for RendererSettings {
    fn from_app(app: &AppWindow) -> Self {
        Self { app: app.clone() }
    }
}
impl RendererSettings {
    pub fn get_preferred(&self) -> String {
        self.app.state.borrow().renderersettings.preferred.clone()
    }
    pub fn set_preferred(&self, value: String) {
        self.app.state.borrow_mut().renderersettings.preferred = value.clone();
    }
    pub fn get_active(&self) -> String {
        self.app.state.borrow().renderersettings.active.clone()
    }
    pub fn set_active(&self, value: String) {
        self.app.state.borrow_mut().renderersettings.active = value.clone();
    }
    pub fn get_gpu_supported(&self) -> bool {
        self.app.state.borrow().renderersettings.gpu_supported
    }
    pub fn set_gpu_supported(&self, value: bool) {
        self.app.state.borrow_mut().renderersettings.gpu_supported = value;
    }
    pub fn get_error(&self) -> String {
        self.app.state.borrow().renderersettings.error.clone()
    }
    pub fn set_error(&self, value: String) {
        self.app.state.borrow_mut().renderersettings.error = value.clone();
    }
    pub fn get_prompt_open(&self) -> bool {
        self.app.state.borrow().renderersettings.prompt_open
    }
    pub fn set_prompt_open(&self, value: bool) {
        self.app.state.borrow_mut().renderersettings.prompt_open = value;
    }
    pub fn get_restarting(&self) -> bool {
        self.app.state.borrow().renderersettings.restarting
    }
    pub fn set_restarting(&self, value: bool) {
        self.app.state.borrow_mut().renderersettings.restarting = value;
    }
    pub fn get_restart_required(&self) -> bool {
        self.get_preferred() != self.get_active()
    }
    pub fn set_restart_required(&self, value: bool) {
        self.app
            .state
            .borrow_mut()
            .renderersettings
            .restart_required = value;
    }
    pub fn on_choose(&self, callback: impl Fn(String) + 'static) {
        self.app.state.borrow_mut().renderersettings.choose = Some(Rc::new(callback));
    }
    pub fn invoke_choose(&self, arg0: String) {
        let callback = self.app.state.borrow().renderersettings.choose.clone();
        callback.map(|callback| callback(arg0)).unwrap_or_default()
    }
    pub fn on_restart_now(&self, callback: impl Fn() + 'static) {
        self.app.state.borrow_mut().renderersettings.restart_now = Some(Rc::new(callback));
    }
    pub fn invoke_restart_now(&self) {
        let callback = self.app.state.borrow().renderersettings.restart_now.clone();
        callback.map(|callback| callback()).unwrap_or_default()
    }
}
pub struct I18nData {
    pub language: String,
    pub lookup: Handler<dyn Fn(String, String) -> String>,
    pub lookup_string: Handler<dyn Fn(String, String, String) -> String>,
    pub lookup_int: Handler<dyn Fn(String, String, i32) -> String>,
    pub lookup_two_ints: Handler<dyn Fn(String, String, i32, i32) -> String>,
}
impl Default for I18nData {
    fn default() -> Self {
        Self {
            language: "en".into(),
            lookup: None,
            lookup_string: None,
            lookup_int: None,
            lookup_two_ints: None,
        }
    }
}
#[derive(Clone)]
pub struct I18n {
    app: AppWindow,
}
impl Global for I18n {
    fn from_app(app: &AppWindow) -> Self {
        Self { app: app.clone() }
    }
}
impl I18n {
    pub fn get_language(&self) -> String {
        self.app.state.borrow().i18n.language.clone()
    }
    pub fn set_language(&self, value: String) {
        self.app.state.borrow_mut().i18n.language = value.clone();
    }
    pub fn on_lookup(&self, callback: impl Fn(String, String) -> String + 'static) {
        self.app.state.borrow_mut().i18n.lookup = Some(Rc::new(callback));
    }
    pub fn invoke_lookup(&self, arg0: String, arg1: String) -> String {
        let callback = self.app.state.borrow().i18n.lookup.clone();
        callback
            .map(|callback| callback(arg0, arg1))
            .unwrap_or_default()
    }
    pub fn on_lookup_string(&self, callback: impl Fn(String, String, String) -> String + 'static) {
        self.app.state.borrow_mut().i18n.lookup_string = Some(Rc::new(callback));
    }
    pub fn invoke_lookup_string(&self, arg0: String, arg1: String, arg2: String) -> String {
        let callback = self.app.state.borrow().i18n.lookup_string.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2))
            .unwrap_or_default()
    }
    pub fn on_lookup_int(&self, callback: impl Fn(String, String, i32) -> String + 'static) {
        self.app.state.borrow_mut().i18n.lookup_int = Some(Rc::new(callback));
    }
    pub fn invoke_lookup_int(&self, arg0: String, arg1: String, arg2: i32) -> String {
        let callback = self.app.state.borrow().i18n.lookup_int.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2))
            .unwrap_or_default()
    }
    pub fn on_lookup_two_ints(
        &self,
        callback: impl Fn(String, String, i32, i32) -> String + 'static,
    ) {
        self.app.state.borrow_mut().i18n.lookup_two_ints = Some(Rc::new(callback));
    }
    pub fn invoke_lookup_two_ints(
        &self,
        arg0: String,
        arg1: String,
        arg2: i32,
        arg3: i32,
    ) -> String {
        let callback = self.app.state.borrow().i18n.lookup_two_ints.clone();
        callback
            .map(|callback| callback(arg0, arg1, arg2, arg3))
            .unwrap_or_default()
    }
}
pub struct AppThemeData {
    pub preset: String,
}
impl Default for AppThemeData {
    fn default() -> Self {
        Self {
            preset: crate::ui::schemes::DEFAULT.into(),
        }
    }
}
#[derive(Clone)]
pub struct AppTheme {
    app: AppWindow,
}
impl Global for AppTheme {
    fn from_app(app: &AppWindow) -> Self {
        Self { app: app.clone() }
    }
}
impl AppTheme {
    pub fn get_preset(&self) -> String {
        self.app.state.borrow().apptheme.preset.clone()
    }
    pub fn set_preset(&self, value: String) {
        self.app.state.borrow_mut().apptheme.preset = value.clone();
    }
}
