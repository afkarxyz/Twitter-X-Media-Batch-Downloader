//! Color schemes from the Ox Content theme presets, https://ox-content.dev/theme-presets/.
//! Values come from the `colors` and `darkColors` of the `@ox-content/theme-color-*`
//! 3.4.0 packages (MIT License, Copyright (c) 2024 ubugeeei). Translucent
//! colors are composited onto the scheme background.

pub const DEFAULT: &str = "catppuccin";

#[derive(Clone, Copy)]
pub struct Colors {
    pub primary: u32,
    pub background: u32,
    pub background_alt: u32,
    pub text: u32,
    pub text_muted: u32,
    pub border: u32,
}
impl Colors {
    const fn new(
        [
            primary,
            background,
            background_alt,
            text,
            text_muted,
            border,
        ]: [u32; 6],
    ) -> Self {
        Self {
            primary,
            background,
            background_alt,
            text,
            text_muted,
            border,
        }
    }
}

pub struct Scheme {
    pub id: &'static str,
    pub label: &'static str,
    pub light: Colors,
    pub dark: Colors,
}

pub const SCHEMES: &[Scheme] = &[
    Scheme {
        id: "github",
        label: "GitHub",
        light: Colors::new([0x0969da, 0xffffff, 0xf6f8fa, 0x1f2328, 0x59636e, 0xd1d9e0]),
        dark: Colors::new([0x4493f8, 0x0d1117, 0x151b23, 0xf0f6fc, 0x9198a1, 0x3d444d]),
    },
    Scheme {
        id: "tokyo-night",
        label: "Tokyo Night",
        light: Colors::new([0x235fb1, 0xe1e2e7, 0xd6d8de, 0x3760bf, 0x516094, 0xa8aecb]),
        dark: Colors::new([0x7aa2f7, 0x1a1b26, 0x24283b, 0xc0caf5, 0x8386a1, 0x2f334d]),
    },
    Scheme {
        id: "mono",
        label: "Mono",
        light: Colors::new([0x111111, 0xffffff, 0xf2f2f2, 0x111111, 0x666666, 0xd4d4d4]),
        dark: Colors::new([0xf5f5f5, 0x0a0a0a, 0x161616, 0xf5f5f5, 0x9b9b9b, 0x2e2e2e]),
    },
    Scheme {
        id: "dracula",
        label: "Dracula",
        light: Colors::new([0x644ac9, 0xfffbeb, 0xf5f0dc, 0x1f1f1f, 0x6c664b, 0xcfc9ad]),
        dark: Colors::new([0xbd93f9, 0x282a36, 0x343746, 0xf8f8f2, 0x9ca3b8, 0x44475a]),
    },
    Scheme {
        id: "one-dark",
        label: "One Dark",
        light: Colors::new([0x386ad5, 0xfafafa, 0xeaeaeb, 0x383a42, 0x696c77, 0xd0d0d1]),
        dark: Colors::new([0x61afef, 0x282c34, 0x21252b, 0xabb2bf, 0x8e939c, 0x3e4451]),
    },
    Scheme {
        id: "retro",
        label: "Retro",
        light: Colors::new([0xa8500a, 0xf4ecd8, 0xe8dcc0, 0x3b2f1e, 0x756647, 0xc9b895]),
        dark: Colors::new([0xffcc33, 0x140f04, 0x211906, 0xffb000, 0xa8741a, 0x4a3608]),
    },
    Scheme {
        id: "snow",
        label: "Snow",
        light: Colors::new([0x0b7db1, 0xffffff, 0xf4f7fb, 0x0f172a, 0x64748b, 0xdbe3ee]),
        dark: Colors::new([0x38bdf8, 0x0b1120, 0x131c31, 0xe2e8f0, 0x94a3b8, 0x26324a]),
    },
    Scheme {
        id: "catppuccin",
        label: "Catppuccin",
        light: Colors::new([0x1d62eb, 0xeff1f5, 0xe6e9ef, 0x4c4f69, 0x686b80, 0xccd0da]),
        dark: Colors::new([0x89b4fa, 0x1e1e2e, 0x181825, 0xcdd6f4, 0xa6adc8, 0x313244]),
    },
    Scheme {
        id: "nord",
        label: "Nord",
        light: Colors::new([0x4f6c90, 0xeceff4, 0xe5e9f0, 0x2e3440, 0x4c566a, 0xd8dee9]),
        dark: Colors::new([0x88c0d0, 0x2e3440, 0x3b4252, 0xeceff4, 0xa5aec0, 0x434c5e]),
    },
    Scheme {
        id: "gruvbox",
        label: "Gruvbox",
        light: Colors::new([0xaf3a03, 0xfbf1c7, 0xf2e5bc, 0x3c3836, 0x665c54, 0xd5c4a1]),
        dark: Colors::new([0xfe8019, 0x282828, 0x32302f, 0xebdbb2, 0xa89984, 0x504945]),
    },
    Scheme {
        id: "rose-pine",
        label: "Rosé Pine",
        light: Colors::new([0x79668e, 0xfaf4ed, 0xfffaf3, 0x575279, 0x6f6c87, 0xdfdad9]),
        dark: Colors::new([0xc4a7e7, 0x191724, 0x1f1d2e, 0xe0def4, 0x908caa, 0x26233a]),
    },
    Scheme {
        id: "solarized",
        label: "Solarized",
        light: Colors::new([0x2075b0, 0xfdf6e3, 0xeee8d5, 0x073642, 0x5d7179, 0xded8c4]),
        dark: Colors::new([0x3794d6, 0x002b36, 0x073642, 0xeee8d5, 0x93a1a1, 0x0d4a5a]),
    },
    Scheme {
        id: "everforest",
        label: "Everforest",
        light: Colors::new([0x2e769e, 0xfdf6e3, 0xf4f0d9, 0x5c6a72, 0x687467, 0xe0dcc7]),
        dark: Colors::new([0x7fbbb3, 0x2d353b, 0x343f44, 0xd3c6aa, 0x9da9a0, 0x475258]),
    },
    Scheme {
        id: "ayu",
        label: "Ayu",
        light: Colors::new([0xa36229, 0xfcfcfc, 0xf3f4f5, 0x5c6166, 0x6e747a, 0xe7e8e9]),
        dark: Colors::new([0xffcc66, 0x1f2430, 0x242936, 0xcccac2, 0x8a9199, 0x343b4b]),
    },
    Scheme {
        id: "vitesse",
        label: "Vitesse",
        light: Colors::new([0x1e754f, 0xffffff, 0xf7f7f6, 0x393a34, 0x75756c, 0xe3e3dd]),
        dark: Colors::new([0x4d9375, 0x121212, 0x1b1b1b, 0xcecabe, 0x84848c, 0x2c2c2c]),
    },
    Scheme {
        id: "night-owl",
        label: "Night Owl",
        light: Colors::new([0x0a7e82, 0xfbfbfb, 0xf0f0f0, 0x403f53, 0x706f7b, 0xd9d9d9]),
        dark: Colors::new([0x82aaff, 0x011627, 0x0b2942, 0xd6deeb, 0x8badc1, 0x1d3b53]),
    },
    Scheme {
        id: "monokai",
        label: "Monokai",
        light: Colors::new([0xbc4584, 0xfdfdfd, 0xf2f2f2, 0x2c292d, 0x6b6669, 0xdcdcdc]),
        dark: Colors::new([0xff6188, 0x2d2a2e, 0x353236, 0xfcfcfa, 0x939293, 0x4a464b]),
    },
    Scheme {
        id: "kanagawa",
        label: "Kanagawa",
        light: Colors::new([0x4d699b, 0xf2ecbc, 0xe7dba0, 0x545464, 0x6c6a5d, 0xdcd5ac]),
        dark: Colors::new([0x7e9cd8, 0x1f1f28, 0x2a2a37, 0xdcd7ba, 0xa09cac, 0x363646]),
    },
    Scheme {
        id: "poimandres",
        label: "Poimandres",
        light: Colors::new([0x0b7285, 0xfcfcfc, 0xf1f3f5, 0x1b1e28, 0x5f6672, 0xdfe3e8]),
        dark: Colors::new([0xadd7ff, 0x1b1e28, 0x252b37, 0xe4f0fb, 0x8186a5, 0x303340]),
    },
    Scheme {
        id: "sepia",
        label: "Sepia",
        light: Colors::new([0x8a5a2b, 0xf6efe3, 0xefe6d6, 0x3a3229, 0x6f6355, 0xddd0ba]),
        dark: Colors::new([0xd9a066, 0x1c1814, 0x26211b, 0xe8dcc8, 0xa5977f, 0x3a322a]),
    },
    Scheme {
        id: "high-contrast",
        label: "High Contrast",
        light: Colors::new([0x0000cc, 0xffffff, 0xf0f0f0, 0x000000, 0x3d3d3d, 0x000000]),
        dark: Colors::new([0x66d9ff, 0x000000, 0x0f0f0f, 0xffffff, 0xc4c4c4, 0xffffff]),
    },
    Scheme {
        id: "synthwave",
        label: "Synthwave",
        light: Colors::new([0xc026a3, 0xfdf0f6, 0xf7e2ee, 0x2b1b3d, 0x6d5680, 0xe6c9dc]),
        dark: Colors::new([0xff7edb, 0x241b2f, 0x2d2140, 0xffe6ff, 0xa48fc0, 0x463063]),
    },
    Scheme {
        id: "voltage",
        label: "Voltage",
        light: Colors::new([0x7c2fe0, 0xffffff, 0xf4f2fa, 0x14111f, 0x5d5673, 0xe0dced]),
        dark: Colors::new([0xb57bff, 0x0a0812, 0x141020, 0xf2effd, 0x9a92b8, 0x2a2140]),
    },
    Scheme {
        id: "flexoki",
        label: "Flexoki",
        light: Colors::new([0x237e76, 0xfffcf0, 0xf2f0e5, 0x100f0f, 0x6f6e69, 0xe6e4d9]),
        dark: Colors::new([0x3aa99f, 0x100f0f, 0x1c1b1a, 0xfffcf0, 0x878580, 0x282726]),
    },
    Scheme {
        id: "iceberg",
        label: "Iceberg",
        light: Colors::new([0x2d539e, 0xe8e9ec, 0xdcdfe7, 0x33374c, 0x62677e, 0xcbd0d9]),
        dark: Colors::new([0x84a0c6, 0x161821, 0x1e2132, 0xc6c8d1, 0x7d8197, 0x2a2f45]),
    },
    Scheme {
        id: "zenburn",
        label: "Zenburn",
        light: Colors::new([0x62704b, 0xf4f4ec, 0xe8e8dc, 0x33322c, 0x6b6a5e, 0xd4d3c4]),
        dark: Colors::new([0x8cd0d3, 0x3f3f3f, 0x4a4a4a, 0xdcdccc, 0xabab9c, 0x585858]),
    },
    Scheme {
        id: "oceanic",
        label: "Oceanic",
        light: Colors::new([0x0b7285, 0xfafbfc, 0xeef1f4, 0x1b2b34, 0x5b6b76, 0xdbe1e6]),
        dark: Colors::new([0x6699cc, 0x1b2b34, 0x22333c, 0xcdd3de, 0x87949e, 0x31424c]),
    },
    Scheme {
        id: "palenight",
        label: "Palenight",
        light: Colors::new([0x5c53b8, 0xfafafa, 0xeeeef2, 0x2f3350, 0x5f6485, 0xdedee6]),
        dark: Colors::new([0xc792ea, 0x292d3e, 0x32364a, 0xa6accd, 0x9096b6, 0x3d4259]),
    },
    Scheme {
        id: "material",
        label: "Material",
        light: Colors::new([0x4d7a76, 0xfafafa, 0xfafafa, 0x627076, 0x627076, 0xdbdbdb]),
        dark: Colors::new([0x80cbc4, 0x263238, 0x263238, 0xeeffff, 0x8b9ca5, 0x1f292d]),
    },
    Scheme {
        id: "horizon",
        label: "Horizon",
        light: Colors::new([0xb24d44, 0xfdf0ed, 0xf9e3de, 0x1c1e26, 0x686b8d, 0xeccec6]),
        dark: Colors::new([0xe95678, 0x1c1e26, 0x232530, 0xd5d8da, 0x8b8fa3, 0x2e303e]),
    },
    Scheme {
        id: "modus",
        label: "Modus",
        light: Colors::new([0x3548cf, 0xffffff, 0xf2f2f2, 0x000000, 0x505050, 0xd0d0d0]),
        dark: Colors::new([0x79a8ff, 0x000000, 0x1e1e1e, 0xffffff, 0xa8a8a8, 0x3a3a3a]),
    },
    Scheme {
        id: "melange",
        label: "Melange",
        light: Colors::new([0x7d5c39, 0xf1f1f1, 0xe9e1db, 0x54433a, 0x7d6658, 0xd9cdc3]),
        dark: Colors::new([0xebc06d, 0x292522, 0x34302c, 0xece1d7, 0xa08e7a, 0x403a35]),
    },
    Scheme {
        id: "graphite",
        label: "Graphite",
        light: Colors::new([0x3b5bfd, 0xfbfbfc, 0xf1f2f4, 0x16161a, 0x6a6c73, 0xe4e5e9]),
        dark: Colors::new([0x8ba0ff, 0x101014, 0x18181d, 0xe9eaee, 0x8b8d96, 0x26262e]),
    },
    Scheme {
        id: "sand",
        label: "Sand",
        light: Colors::new([0xb4552d, 0xfaf7f2, 0xf0ebe2, 0x1f1b16, 0x6d6255, 0xe2dacd]),
        dark: Colors::new([0xe08a5c, 0x191510, 0x221d17, 0xf0e9dd, 0x9d9082, 0x332b22]),
    },
    Scheme {
        id: "moss",
        label: "Moss",
        light: Colors::new([0x2f6b4f, 0xf7f6f0, 0xecebe1, 0x17211b, 0x5c6b60, 0xdbdccf]),
        dark: Colors::new([0x7fc79b, 0x111813, 0x18211b, 0xe2e9e2, 0x8b9a8e, 0x243027]),
    },
    Scheme {
        id: "slate",
        label: "Slate",
        light: Colors::new([0x4a7a12, 0xf6f7f9, 0xeceef2, 0x14181f, 0x61697a, 0xdfe3ea]),
        dark: Colors::new([0xb6e84f, 0x0f1216, 0x171b21, 0xe6e9ef, 0x868f9f, 0x232830]),
    },
    Scheme {
        id: "plum",
        label: "Plum",
        light: Colors::new([0x8a3d75, 0xfbf8fa, 0xf2ecf0, 0x1e1520, 0x6b5a6d, 0xe6dbe3]),
        dark: Colors::new([0xdfa3c9, 0x150f18, 0x1d1621, 0xefe4ec, 0x9d8aa0, 0x2b2130]),
    },
    Scheme {
        id: "ink",
        label: "Ink",
        light: Colors::new([0x1f3d75, 0xf8f9fb, 0xeef1f5, 0x0f1524, 0x5d6779, 0xe0e5ec]),
        dark: Colors::new([0xe3c08a, 0x0c1220, 0x131b2c, 0xe8ecf3, 0x8390a8, 0x1e2739]),
    },
    Scheme {
        id: "porcelain",
        label: "Porcelain",
        light: Colors::new([0x4b6b8a, 0xfcfbf9, 0xf3f1ec, 0x1c1b19, 0x6f6c66, 0xe6e3dc]),
        dark: Colors::new([0x9dbcd8, 0x171614, 0x1f1e1b, 0xeceae4, 0x9a978f, 0x2c2a26]),
    },
    Scheme {
        id: "cacao",
        label: "Cacao",
        light: Colors::new([0x96591f, 0xfaf5ef, 0xf0e8de, 0x231a14, 0x6f6055, 0xe2d6c8]),
        dark: Colors::new([0xdda86a, 0x1a1310, 0x241b16, 0xf0e6da, 0x9d8d7e, 0x332720]),
    },
    Scheme {
        id: "coral",
        label: "Coral",
        light: Colors::new([0xbc4d3c, 0xfdfaf8, 0xf6eeea, 0x1c1614, 0x6d5f5a, 0xebdcd5]),
        dark: Colors::new([0xff8a75, 0x161110, 0x1f1817, 0xf2e6e1, 0x9c8b85, 0x2c2321]),
    },
    Scheme {
        id: "arctic",
        label: "Arctic",
        light: Colors::new([0x0f6f86, 0xf9fbfc, 0xeef3f6, 0x0f1a1f, 0x5b6c75, 0xdde7ec]),
        dark: Colors::new([0x6fd0e0, 0x0b1216, 0x121b21, 0xe4eef2, 0x7f929c, 0x1d272d]),
    },
    Scheme {
        id: "fuji",
        label: "Fuji",
        light: Colors::new([0x0f7a5f, 0xfbfaf5, 0xf0efe4, 0x16211c, 0x5b6a5f, 0xdcdccd]),
        dark: Colors::new([0x4fd39b, 0x101815, 0x17211c, 0xe6ede7, 0x8a9a8f, 0x22302a]),
    },
    Scheme {
        id: "stage",
        label: "Stage",
        light: Colors::new([0x4b32c3, 0xf8f7fc, 0xeeecf7, 0x14122b, 0x5c5880, 0xddd9ee]),
        dark: Colors::new([0xff6b5b, 0x0f0d1f, 0x161331, 0xe9e6f6, 0x8c86ad, 0x221d42]),
    },
    Scheme {
        id: "emerald",
        label: "Emerald",
        light: Colors::new([0x0e8462, 0xfbfdfc, 0xeef5f2, 0x1b2b33, 0x526d78, 0xd9eae3]),
        dark: Colors::new([0x42d99f, 0x101c22, 0x17262e, 0xe7f2ee, 0x82a099, 0x22343c]),
    },
    Scheme {
        id: "commander",
        label: "Commander",
        light: Colors::new([0x0000aa, 0xcfeceb, 0xb8e0df, 0x00292b, 0x12565a, 0x7fc4c2]),
        dark: Colors::new([0x55ffff, 0x000000, 0x0d0d0d, 0xaaaaaa, 0x7f7f7f, 0x555555]),
    },
];

pub fn get(id: &str) -> Option<&'static Scheme> {
    SCHEMES.iter().find(|scheme| scheme.id == id)
}

pub fn find(id: &str) -> &'static Scheme {
    get(id).or_else(|| get(DEFAULT)).unwrap()
}

pub fn rgb(value: u32) -> egui::Color32 {
    egui::Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}
