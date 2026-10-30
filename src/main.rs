#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[tokio::main]
async fn main() -> Result<(), twitterxmediabatchdownloader::PlatformError> {
    twitterxmediabatchdownloader::run().await
}
