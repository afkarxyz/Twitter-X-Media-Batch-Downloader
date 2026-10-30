pub(crate) mod accounts;
pub(crate) mod app;
pub(crate) mod download;
pub(crate) mod fetch;
pub(crate) mod history;
pub(crate) mod i18n;
pub(crate) mod library;
pub(crate) mod media;
pub(crate) mod network;
pub(crate) mod platform;
pub(crate) mod ui;
pub(crate) mod updates;

pub use app::run;
pub(crate) use ui::runtime::AppWindow;
pub use ui::runtime::PlatformError;
pub(crate) use ui::runtime::state::*;
pub(crate) const APP_ID: &str = "twitterxmediabatchdownloader";
