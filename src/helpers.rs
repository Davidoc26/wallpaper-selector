use ashpd::desktop::wallpaper::{SetOn, WallpaperRequest};
use ashpd::desktop::Request;
use ashpd::{Error, WindowIdentifier};
use std::fs::File;

pub async fn set_wallpaper(
    identifier: WindowIdentifier,
    file: &File,
) -> Result<Request<()>, Error> {
    WallpaperRequest::default()
        .set_on(SetOn::Background)
        .identifier(identifier)
        .show_preview(Some(true))
        .build_file(file)
        .await
}
