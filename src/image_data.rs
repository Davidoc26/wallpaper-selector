use adw::gdk::Texture;
use adw::glib;
use adw::glib::Object;

mod imp {
    use std::cell::RefCell;

    use adw::gdk::Texture;
    use adw::glib;
    use adw::glib::once_cell::sync::Lazy;
    use adw::glib::{ParamFlags, ParamSpecObject, ParamSpecString, StaticType, ToValue};
    use adw::subclass::prelude::{ObjectImpl, ObjectSubclass};

    use crate::gio::glib::{ParamSpec, Value};

    #[derive(Default)]
    pub struct ImageData {
        path: RefCell<String>,
        texture: RefCell<Option<Texture>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ImageData {
        const NAME: &'static str = "WallpaperSelectorImageData";
        type Type = super::ImageData;
    }

    impl ObjectImpl for ImageData {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::new("path", "path", "path", None, ParamFlags::READWRITE),
                    ParamSpecObject::new(
                        "texture",
                        "texture",
                        "texture",
                        Texture::static_type(),
                        ParamFlags::READWRITE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _obj: &Self::Type, _id: usize, value: &Value, _pspec: &ParamSpec) {
            match _pspec.name() {
                "path" => {
                    let path = value.get::<String>().expect("Value must be String");
                    self.path.replace(path);
                }
                "texture" => {
                    let texture = value.get::<Texture>().unwrap();
                    self.texture.replace(Some(texture));
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "path" => self.path.borrow().to_value(),
                "texture" => self.texture.borrow().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct ImageData(ObjectSubclass<imp::ImageData>);
}

impl ImageData {
    pub fn new(path: String, texture: Texture) -> Self {
        Object::new(&[("path", &path), ("texture", &texture)]).expect("Can't create `ImageData`")
    }
}
