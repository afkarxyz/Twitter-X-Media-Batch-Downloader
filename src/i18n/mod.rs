use std::collections::HashMap;

use crate::AppWindow;

const CATALOGS: [(&str, &str); 7] = [
    (
        "id",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/id.po")),
    ),
    (
        "de",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/de.po")),
    ),
    (
        "es",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/es.po")),
    ),
    (
        "fr",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/fr.po")),
    ),
    (
        "pt",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/pt.po")),
    ),
    (
        "tr",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/tr.po")),
    ),
    (
        "zh_Hans",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang/zh_Hans.po")),
    ),
];

pub fn register(app: &AppWindow) {
    let global = app.global::<crate::I18n>();
    global.on_lookup(|language, key| translate(language.as_str(), key.as_str()));
    global.on_lookup_string(|language, key, argument| {
        translate_with(
            language.as_str(),
            key.as_str(),
            std::slice::from_ref(&argument),
        )
    });
    global.on_lookup_int(|language, key, argument| {
        translate_with(language.as_str(), key.as_str(), &[argument.to_string()])
    });
    global.on_lookup_two_ints(|language, key, first, second| {
        translate_with(
            language.as_str(),
            key.as_str(),
            &[first.to_string(), second.to_string()],
        )
    });
}

pub fn apply_language(app: &AppWindow, mode: &str) {
    let normalized = match mode {
        "en" | "id" | "de" | "es" | "fr" | "pt" | "tr" | "zh_Hans" => mode,
        _ => "system",
    };
    let language = match normalized {
        "system" => system_language(),
        value => value.to_owned(),
    };
    app.set_language_mode(normalized.into());
    app.global::<crate::I18n>().set_language(language);
}

fn system_language() -> String {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    let language = locale
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or_default();
    match language {
        "id" | "de" | "es" | "fr" | "pt" | "tr" => language.to_owned(),
        "zh" => "zh_Hans".to_owned(),
        _ => "en".to_owned(),
    }
}

fn catalogs() -> &'static HashMap<String, HashMap<String, String>> {
    static CATALOGS_MAP: std::sync::OnceLock<HashMap<String, HashMap<String, String>>> =
        std::sync::OnceLock::new();
    CATALOGS_MAP.get_or_init(|| {
        CATALOGS
            .into_iter()
            .map(|(language, content)| (language.to_owned(), parse_po(content)))
            .collect()
    })
}

pub fn initialize() {
    let _ = catalogs();
}

fn parse_po(content: &str) -> HashMap<String, String> {
    let mut output = HashMap::new();
    let mut message_id = String::new();
    let mut message = String::new();
    let mut reading_message = false;
    let mut reading_translation = false;

    let flush = |output: &mut HashMap<String, String>, key: &str, value: &str| {
        if !key.is_empty() && !value.is_empty() {
            output.insert(key.to_owned(), value.to_owned());
        }
    };

    for line in content.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("msgid ") {
            flush(&mut output, &message_id, &message);
            message_id = decode_po_string(value);
            message.clear();
            reading_message = true;
            reading_translation = false;
        } else if let Some(value) = line.strip_prefix("msgstr ") {
            reading_message = false;
            reading_translation = true;
            message = decode_po_string(value);
        } else if line.starts_with("msgid_plural ") {
            reading_message = false;
        } else if let Some(value) = line.strip_prefix("msgstr[0] ") {
            reading_message = false;
            reading_translation = true;
            message = decode_po_string(value);
        } else if line.starts_with('"') {
            let value = decode_po_string(line);
            if reading_translation {
                message.push_str(&value);
            } else if reading_message {
                message_id.push_str(&value);
            }
        } else if line.is_empty() {
            flush(&mut output, &message_id, &message);
            message_id.clear();
            message.clear();
            reading_message = false;
            reading_translation = false;
        }
    }
    flush(&mut output, &message_id, &message);
    output
}

fn decode_po_string(raw: &str) -> String {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(raw);
    let mut output = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => output.push('\n'),
            Some('r') => output.push('\r'),
            Some('t') => output.push('\t'),
            Some('"') => output.push('"'),
            Some('\\') => output.push('\\'),
            Some(other) => output.push(other),
            None => output.push('\\'),
        }
    }
    output
}

pub fn translate(language: &str, key: &str) -> String {
    catalogs()
        .get(language)
        .and_then(|entries| entries.get(key))
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}

fn translate_with(language: &str, key: &str, arguments: &[String]) -> String {
    let template = translate(language, key);
    let mut output = String::with_capacity(template.len());
    let mut remainder = template.as_str();
    let mut arguments = arguments.iter();
    while let Some(index) = remainder.find("{}") {
        output.push_str(&remainder[..index]);
        if let Some(argument) = arguments.next() {
            output.push_str(argument);
        } else {
            output.push_str("{}");
        }
        remainder = &remainder[index + 2..];
    }
    output.push_str(remainder);
    output
}
