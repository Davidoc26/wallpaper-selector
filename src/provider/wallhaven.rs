use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};

use adw::gdk::Texture;
use adw::glib;
use adw::glib::{Bytes, Sender};
use adw::prelude::TextureExt;

use crate::api::wallhaven::client::Client;
use crate::api::wallhaven::response::ThumbType;
use crate::core::desktop::DesktopEnvironment;

pub struct Wallhaven {
    client: Arc<Client>,
    page: Arc<Mutex<u32>>,
    desktop: Arc<Mutex<Box<dyn DesktopEnvironment + Send>>>,
}

impl Wallhaven {
    pub fn new(client: Client, desktop: Box<dyn DesktopEnvironment + Send>) -> Self {
        Self {
            client: Arc::new(client),
            page: Arc::new(Mutex::new(1)),
            desktop: Arc::new(Mutex::new(desktop)),
        }
    }

    fn increment_page(&self) {
        let mut page = self.page.lock().unwrap();
        *page += 1;
    }

    pub async fn set_wallpaper(&self, url: String) -> Result<(), Box<dyn Error + Send + Sync>> {
        let bytes = self.client.get_bytes(&url).await?;
        let bytes = Bytes::from(&bytes);

        let texture = Texture::from_bytes(&bytes)?;
        let save_path = self.parse_url(&url);
        self.save_texture(&texture, Path::new(&save_path)).await?;

        self.desktop.lock().unwrap().set_wallpaper(&save_path)?;

        Ok(())
    }

    async fn save_texture(
        &self,
        texture: &Texture,
        filename: &Path,
    ) -> Result<(), glib::error::BoolError> {
        texture.save_to_png(filename)?;

        Ok(())
    }

    fn parse_url(&self, url: &str) -> String {
        let id = url
            .split("/")
            .collect::<Vec<&str>>()
            .last()
            .copied()
            .unwrap();

        format!("{}/{}", std::env::var("XDG_DATA_HOME").unwrap(), id)
    }

    pub async fn load_images(&self, sender: &Sender<ProviderMessage>) {
        let page = *self.page.lock().unwrap();
        self.increment_page();

        let images = self
            .client
            .search(Some(page), None)
            .await
            .unwrap()
            .get_images();

        for image in images {
            let bytes = self
                .client
                .get_bytes(image.thumb_url(ThumbType::Small))
                .await
                .unwrap();
            let bytes = Bytes::from(&bytes);
            let texture = Texture::from_bytes(&bytes).unwrap();

            sender
                .send(ProviderMessage::Image(image.get_path(), texture))
                .unwrap();
        }
    }
}

pub enum ProviderMessage {
    Image(String, Texture),
}
