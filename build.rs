use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::{ffi::OsStr, fs};

// Executable resources exist on Windows only.
#[cfg(windows)]
const APP_NAME: &str = "Twitter/X Media Batch Downloader";
#[cfg(windows)]
const APP_DESCRIPTION: &str =
    "Downloads original-quality images and videos from Twitter/X accounts.";
#[cfg(windows)]
const EXE_NAME: &str = "TwitterXMediaBatchDownloader";

fn main() {
    println!("cargo:rerun-if-changed=assets/logo/logo.png");
    let out_dir =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo did not provide OUT_DIR"));
    prepare_ui_icon(&out_dir);

    #[cfg(windows)]
    embed_windows_icon(&out_dir);
}

fn prepare_ui_icon(out_dir: &Path) {
    use image::ImageReader;
    use image::imageops::FilterType;

    let icon = ImageReader::open("assets/logo/logo.png")
        .expect("Could not open application icon")
        .decode()
        .expect("Could not decode application icon");
    let ui_icon = icon.resize_exact(144, 144, FilterType::Lanczos3);
    ui_icon
        .save(out_dir.join("logo-ui.png"))
        .expect("Could not save the downsampled UI icon");
}

#[cfg(windows)]
fn embed_windows_icon(out_dir: &Path) {
    use image::ImageReader;
    use image::imageops::FilterType;

    let image = ImageReader::open("assets/logo/logo.png")
        .expect("Could not open application icon")
        .decode()
        .expect("Could not decode application icon");

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16, 24, 32, 48, 64, 128, 256] {
        let resized = image
            .resize_exact(size, size, FilterType::Lanczos3)
            .to_rgba8();
        let icon_image = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        icon_dir.add_entry(ico::IconDirEntry::encode(&icon_image).expect("Could not encode icon"));
    }

    let icon_path = out_dir.join("logo.ico");
    icon_dir
        .write(fs::File::create(&icon_path).expect("Could not create ICO file"))
        .expect("Could not write ICO file");

    let rc_path = out_dir.join("icon.rc");
    let (file_version, dotted_version) = windows_version_strings();
    let resources = format!(
        r#"1 ICON "{icon_file}"

VS_VERSION_INFO VERSIONINFO
 FILEVERSION {file_version}
 PRODUCTVERSION {file_version}
 FILEFLAGSMASK 0x3fL
 FILEFLAGS 0x0L
 FILEOS 0x40004L
 FILETYPE 0x1L
 FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "{APP_NAME}\0"
            VALUE "FileDescription", "{APP_DESCRIPTION}\0"
            VALUE "FileVersion", "{dotted_version}\0"
            VALUE "InternalName", "{EXE_NAME}\0"
            VALUE "OriginalFilename", "{EXE_NAME}.exe\0"
            VALUE "ProductName", "{APP_NAME}\0"
            VALUE "ProductVersion", "{dotted_version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
        icon_file = icon_path.to_string_lossy().replace('\\', "/"),
    );
    fs::write(&rc_path, resources).expect("Could not write Windows resource file");
    embed_resource::compile(&rc_path, std::iter::empty::<&OsStr>())
        .manifest_required()
        .expect("Could not compile Windows application resources");
}

#[cfg(windows)]
fn windows_version_strings() -> (String, String) {
    let package_version =
        std::env::var("CARGO_PKG_VERSION").expect("Cargo did not provide CARGO_PKG_VERSION");
    let mut components = package_version.split('.');
    let major = components
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .unwrap_or(0);
    let minor = components
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .unwrap_or(0);
    let patch = components
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .unwrap_or(0);
    (
        format!("{major},{minor},{patch},0"),
        format!("{major}.{minor}.{patch}.0"),
    )
}
