pub mod client {
    use std::error::Error;

    use bytes::Bytes;

    use crate::api::wallhaven::response::{Image, Response, ThumbType};

    pub struct Client {
        client: reqwest::Client,
    }

    impl Client {
        pub fn new(client: Option<reqwest::Client>) -> Self {
            Self {
                client: client.unwrap_or_default(),
            }
        }

        pub async fn search(
            &self,
            page: Option<u32>,
            category: Option<Category>,
        ) -> Result<Response, Box<dyn Error>> {
            let url = format!(
                "https://wallhaven.cc/api/v1/search?page={}&categories={}",
                page.unwrap_or(1),
                category.unwrap_or_default().value()
            );

            Ok(self
                .client
                .get(url)
                .send()
                .await?
                .json::<Response>()
                .await?)
        }

        pub async fn image_thumb(
            &self,
            image: &Image,
            thumb: ThumbType,
        ) -> Result<Bytes, Box<dyn std::error::Error>> {
            Ok(self
                .client
                .get(image.thumb_url(thumb))
                .send()
                .await?
                .bytes()
                .await?)
        }

        pub async fn get_bytes(&self, path: &str) -> Result<Bytes, reqwest::Error> {
            self.client.get(path).send().await?.bytes().await
        }
    }

    pub enum Category {
        General,
        Anime,
        People,
    }

    impl Default for Category {
        fn default() -> Self {
            Category::General
        }
    }

    impl Category {
        pub fn value(&self) -> &str {
            match self {
                Category::General => "100",
                Category::Anime => "010",
                Category::People => "001",
            }
        }
    }

    impl From<i32> for Category {
        fn from(pos: i32) -> Self {
            match pos {
                0 => Category::General,
                1 => Category::Anime,
                2 => Category::People,
                _ => Self::default(),
            }
        }
    }
}

pub mod response {
    use std::collections::HashMap;

    use serde;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Response {
        #[serde(rename(deserialize = "data"))]
        images: Vec<Image>,
    }

    impl Response {
        pub fn get_images(self) -> Vec<Image> {
            self.images
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Image {
        url: String,
        path: String,
        thumbs: HashMap<String, String>,
    }

    impl Image {
        pub fn path(&self) -> &String {
            &self.path
        }

        pub fn get_path(self) -> String {
            self.path
        }

        pub fn thumb_url(&self, thumb: ThumbType) -> &String {
            return match thumb {
                ThumbType::Large => self.thumbs.get("large").unwrap(),
                ThumbType::Original => self.thumbs.get("original").unwrap(),
                ThumbType::Small => self.thumbs.get("small").unwrap(),
            };
        }
    }

    pub enum ThumbType {
        Large,
        Original,
        Small,
    }
}
