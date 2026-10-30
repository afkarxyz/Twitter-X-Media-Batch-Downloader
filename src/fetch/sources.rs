use crate::AppWindow;
use std::{cell::RefCell, rc::Rc};

pub fn kind(source: &str) -> i32 {
    let source = source.trim();
    let Ok(url) = reqwest::Url::parse(source) else {
        return 0;
    };
    if !matches!(url.scheme(), "http" | "https")
        || !matches!(
            url.host_str(),
            Some("x.com" | "www.x.com" | "twitter.com" | "www.twitter.com")
        )
    {
        return 0;
    }
    let path = url.path().trim_matches('/');
    if path == "i/bookmarks" || path.starts_with("i/bookmarks/") {
        2
    } else if path.ends_with("/likes") {
        1
    } else {
        0
    }
}

#[derive(Clone)]
struct PublicOptions {
    source: crate::ui::runtime::SharedString,
    batch: bool,
    dates: bool,
    start: crate::ui::runtime::SharedString,
    end: crate::ui::runtime::SharedString,
    accounts: crate::ui::runtime::SharedString,
}

pub fn register(app: &AppWindow) {
    let public = Rc::new(RefCell::new(None::<PublicOptions>));
    let ui = app.as_weak();
    app.on_sync_fetch_source(move |source| {
        if let Some(app) = ui.upgrade() {
            let source_kind = kind(&source);
            app.set_fetch_source_index(source_kind);
            if source_kind > 0 && !app.get_is_busy() && !app.get_batch_running() {
                app.invoke_apply_fetch_options(
                    false,
                    false,
                    app.get_fetch_start_date(),
                    app.get_fetch_end_date(),
                    app.get_batch_sources_text(),
                );
                app.set_batch_mode(false);
                app.set_date_filter_enabled(false);
            }
        }
    });
    let ui = app.as_weak();
    app.on_choose_fetch_source(move |index| {
        let Some(app) = ui.upgrade() else {
            return;
        };
        if app.get_is_busy() {
            return;
        }
        let source = app.get_source_url();
        let private = kind(&source) > 0;
        match index {
            1 | 2 => {
                if !private {
                    *public.borrow_mut() = Some(PublicOptions {
                        source,
                        batch: app.get_batch_mode(),
                        dates: app.get_date_filter_enabled(),
                        start: app.get_fetch_start_date(),
                        end: app.get_fetch_end_date(),
                        accounts: app.get_batch_sources_text(),
                    });
                }
                app.invoke_apply_fetch_options(
                    false,
                    false,
                    app.get_fetch_start_date(),
                    app.get_fetch_end_date(),
                    app.get_batch_sources_text(),
                );
                app.set_batch_mode(false);
                app.set_date_filter_enabled(false);
                app.set_fetch_options_error("".into());
                app.set_source_url(
                    if index == 1 {
                        "https://x.com/i/likes"
                    } else {
                        "https://x.com/i/bookmarks"
                    }
                    .into(),
                );
            }
            _ if private => {
                let previous = public.borrow_mut().take();
                if let Some(previous) = previous {
                    app.set_source_url(previous.source);
                    app.invoke_apply_fetch_options(
                        previous.batch,
                        previous.dates,
                        previous.start,
                        previous.end,
                        previous.accounts,
                    );
                } else {
                    app.set_source_url("".into());
                }
            }
            _ => {}
        }
        app.set_fetch_source_index(kind(&app.get_source_url()));
    });
    app.set_fetch_source_index(kind(&app.get_source_url()));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_source_detection_does_not_classify_public_names_or_searches_as_private() {
        for source in [
            "demo",
            "https://x.com/likes",
            "https://x.com/demo/media",
            "https://x.com/search?q=bookmarks",
            "https://example.com/demo/likes",
            "ftp://x.com/demo/likes",
        ] {
            assert_eq!(kind(source), 0);
        }
        assert_eq!(kind("https://x.com/demo/likes?s=20"), 1);
        assert_eq!(kind("https://x.com/i/likes"), 1);
        assert_eq!(kind("https://twitter.com/i/bookmarks/"), 2);
    }
}
