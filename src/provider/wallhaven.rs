use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};

use adw::gdk::Texture;
use adw::glib;
use adw::glib::{Bytes, Sender};
use adw::prelude::*;

use crate::api::wallhaven::client::{Category, Client};
use crate::api::wallhaven::response::ThumbType;

// Load only 15 pages, then clear GridView
const IMAGE_PER_PAGE: u16 = 24;
const MAX_IMAGE_COUNT: u16 = IMAGE_PER_PAGE * 15;

pub struct Wallhaven {
    client: Arc<Client>,
    page: Arc<Mutex<u32>>,
    image_count: Arc<Mutex<u16>>,
}

impl Wallhaven {
    pub fn new(client: Client) -> Self {
        Self {
            client: Arc::new(client),
            page: Arc::new(Mutex::new(1)),
            image_count: Arc::new(Mutex::new(0)),
        }
    }

    fn increment_page(&self) {
        let mut page = self.page.lock().unwrap();
        *page += 1;
    }

    fn increment_image_count(&self) {
        *self.image_count.lock().unwrap() += 1;
    }

    fn reset_image_count(&self) {
        *self.image_count.lock().unwrap() = 0;
    }

    fn reached_max_image_count(&self) -> bool {
        *self.image_count.lock().unwrap() == MAX_IMAGE_COUNT
    }

    pub async fn download_wallpaper(
        &self,
        url: String,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let bytes = self.client.get_bytes(&url).await?;
        let bytes = Bytes::from(&bytes);

        let texture = Texture::from_bytes(&bytes)?;
        let save_path = self.parse_url(&url);
        self.save_texture(&texture, Path::new(&save_path)).await?;

        Ok(save_path)
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

    pub async fn load_images(&self, sender: &Sender<ProviderMessage>, category: Category) {
        let page = *self.page.lock().unwrap();
        self.increment_page();

        let images = self
            .client
            .search(Some(page), Some(category))
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

            if self.reached_max_image_count() {
                sender.send(ProviderMessage::Reset).unwrap();
                self.reset_image_count();
            }

            sender
                .send(ProviderMessage::Image(image.get_path(), texture))
                .unwrap();

            self.increment_image_count();
        }
    }
}

pub enum ProviderMessage {
    Image(String, Texture),
    Reset,
}
