use crate::ui::schemes;

pub fn normalize(name: &str) -> &'static str {
    if let Some(scheme) = schemes::get(name) {
        return scheme.id;
    }
    // Palettes from earlier releases move to the closest Ox Content scheme.
    match name {
        "zinc" => "graphite",
        "stone" => "porcelain",
        "mauve" => "plum",
        "mist" => "arctic",
        "blue" | "sky" => "github",
        "teal" | "cyan" => "oceanic",
        "green" => "emerald",
        "violet" | "purple" | "indigo" => "voltage",
        "rose" | "red" | "pink" => "horizon",
        "amber" => "cacao",
        _ => schemes::DEFAULT,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn selected_schemes_and_older_settings_resolve_to_supported_options() {
        for scheme in crate::ui::schemes::SCHEMES {
            assert_eq!(super::normalize(scheme.id), scheme.id);
        }
        for (old, new) in [
            ("zinc", "graphite"),
            ("slate", "slate"),
            ("stone", "porcelain"),
            ("mauve", "plum"),
            ("mist", "arctic"),
            ("blue", "github"),
            ("teal", "oceanic"),
            ("emerald", "emerald"),
            ("violet", "voltage"),
            ("amber", "cacao"),
            ("rose", "horizon"),
            ("purple", "voltage"),
            ("red", "horizon"),
        ] {
            assert_eq!(super::normalize(old), new, "{old}");
        }
        assert_eq!(super::normalize("default"), "catppuccin");
        assert_eq!(super::normalize("unknown"), "catppuccin");
    }
}
