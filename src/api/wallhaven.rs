pub mod client {
    use std::error::Error;

    use bytes::Bytes;

    use crate::api::wallhaven::response::{Image, Response, ThumbType};

    #[derive(Debug)]
    pub struct Client {
        client: reqwest::Client,
    }

    impl Client {
        pub fn new(client: Option<reqwest::Client>) -> Self {
            Self {
                client: client.unwrap_or_default(),
            }
        }

        pub async fn search(&self, options: SearchOptions) -> Result<Response, Box<dyn Error>> {
            let url = self.build_search_url(options);

            Ok(self
                .client
                .get(url)
                .send()
                .await?
                .json::<Response>()
                .await?)
        }

        fn build_search_url(&self, query: SearchOptions) -> String {
            let mut params = Vec::new();

            if let Some(page) = query.page {
                params.push(format!("page={}", page));
            }

            params.push(format!(
                "categories={}{}{}",
                query.category_general.unwrap_or(false) as u8,
                query.category_anime.unwrap_or(false) as u8,
                query.category_people.unwrap_or(false) as u8,
            ));

            if let Some(sorting) = query.sorting {
                params.push(format!("sorting={}", sorting.value()));
            }

            let query_string = format!("?{}", params.join("&"));

            format!("https://wallhaven.cc/api/v1/search{}", query_string)
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

    #[derive(Default)]
    pub struct SearchOptions {
        pub page: Option<u32>,
        pub sorting: Option<Sorting>,
        pub category_general: Option<bool>,
        pub category_anime: Option<bool>,
        pub category_people: Option<bool>,
    }

    impl SearchOptions {
        pub fn page(mut self, page: u32) -> Self {
            self.page = Some(page);
            self
        }

        pub fn sorting(mut self, sorting: Sorting) -> Self {
            self.sorting = Some(sorting);
            self
        }

        pub fn category_general(mut self, is_active: bool) -> Self {
            self.category_general = Some(is_active);
            self
        }

        pub fn category_anime(mut self, is_active: bool) -> Self {
            self.category_anime = Some(is_active);
            self
        }

        pub fn category_people(mut self, is_active: bool) -> Self {
            self.category_people = Some(is_active);
            self
        }
    }

    #[derive(Default)]
    pub enum Category {
        #[default]
        General,
        Anime,
        People,
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

    #[derive(Default)]
    pub enum Sorting {
        #[default]
        Latest,
        Hot,
        Toplist,
        Random,
    }

    impl Sorting {
        pub fn value(&self) -> &'static str {
            match self {
                Sorting::Latest => "date_added",
                Sorting::Hot => "hot",
                Sorting::Toplist => "toplist",
                Sorting::Random => "random",
            }
        }
    }

    impl From<&str> for Sorting {
        fn from(s: &str) -> Self {
            match s {
                "All" => Sorting::Latest,
                "Hot" => Sorting::Hot,
                "Toplist" => Sorting::Toplist,
                "Random" => Sorting::Random,
                _ => Sorting::Latest,
            }
        }
    }
    impl From<Sorting> for u32 {
        fn from(val: Sorting) -> Self {
            match val {
                Sorting::Latest => 0,
                Sorting::Toplist => 1,
                Sorting::Hot => 2,
                Sorting::Random => 3,
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
            match thumb {
                ThumbType::Large => self.thumbs.get("large").unwrap(),
                ThumbType::Original => self.thumbs.get("original").unwrap(),
                ThumbType::Small => self.thumbs.get("small").unwrap(),
            }
        }
    }

    pub enum ThumbType {
        Large,
        Original,
        Small,
    }
}
