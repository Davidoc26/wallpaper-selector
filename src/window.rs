use std::error::Error;
use std::fs::File;
use std::sync::Arc;

use adw::gdk::Texture;
use adw::gio::ListStore;
use adw::glib::clone;
use adw::glib::MainContext;
use adw::Toast;
use adw::{gio, glib};
use ashpd::desktop::wallpaper::SetOn;
use ashpd::desktop::ResponseError;
use ashpd::WindowIdentifier;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{GridView, Image, PositionType, ScrolledWindow, SignalListItemFactory, SingleSelection};

use crate::api::wallhaven::client::Category;
use crate::application::WallpaperSelectorApplication;
use crate::config::{APP_ID, PROFILE};
use crate::image_data::ImageData;
use crate::provider::wallhaven::{ProviderMessage, Wallhaven};
use crate::RUNTIME;

mod imp {
    use adw::subclass::application_window::AdwApplicationWindowImpl;
    use adw::ToastOverlay;
    use gtk::CompositeTemplate;

    use super::*;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/io/github/davidoc26/wallpaper_selector/ui/window.ui")]
    pub struct WallpaperSelectorWindow {
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub main_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub toast: TemplateChild<ToastOverlay>,
        pub settings: gio::Settings,
    }

    impl Default for WallpaperSelectorWindow {
        fn default() -> Self {
            Self {
                header_bar: Default::default(),
                main_box: Default::default(),
                toast: Default::default(),
                settings: gio::Settings::new(APP_ID),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WallpaperSelectorWindow {
        const NAME: &'static str = "WallpaperSelectorWindow";
        type Type = super::WallpaperSelectorWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for WallpaperSelectorWindow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            // Load latest window state
            obj.load_window_size();
        }
    }

    impl WidgetImpl for WallpaperSelectorWindow {}

    impl WindowImpl for WallpaperSelectorWindow {
        // Save window state on delete event
        fn close_request(&self, window: &Self::Type) -> gtk::Inhibit {
            if let Err(err) = window.save_window_size() {
                log::warn!("Failed to save window state, {}", &err);
            }

            // Pass close request on to the parent
            self.parent_close_request(window)
        }
    }

    impl ApplicationWindowImpl for WallpaperSelectorWindow {}

    impl AdwApplicationWindowImpl for WallpaperSelectorWindow {}
}

glib::wrapper! {
    pub struct WallpaperSelectorWindow(ObjectSubclass<imp::WallpaperSelectorWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
}

impl WallpaperSelectorWindow {
    pub fn new(app: &WallpaperSelectorApplication) -> Self {
        glib::Object::new(&[("application", app)])
            .expect("Failed to create WallpaperSelectorWindow")
    }

    pub fn build_grid(&self) {
        let model: Arc<ListStore> = Arc::new(
            ListStore::builder()
                .item_type(ImageData::static_type())
                .build(),
        );
        let client = crate::api::wallhaven::client::Client::new(None);
        let (sender, receiver) = MainContext::channel::<ProviderMessage>(glib::PRIORITY_DEFAULT);
        let sender = Arc::new(sender);

        let provider = Arc::new(Wallhaven::new(client));
        let selection_model = SingleSelection::new(Some(&*model));

        let grid_view = self.prepare_grid_view(Arc::clone(&provider), selection_model);
        let category = self.current_category();

        RUNTIME.spawn(clone!(@strong provider, @strong sender => async move {
            let sender = &sender;
            provider.load_images(&sender, category).await;
        }));

        receiver.attach(None, move |message| {
            match message {
                ProviderMessage::Image(path, texture) => {
                    let image_data = ImageData::new(path, texture);
                    model.append(&image_data);
                }
                ProviderMessage::Reset => {
                    model.remove_all();
                }
            }

            Continue(true)
        });

        let scrolled_window = ScrolledWindow::builder()
            .hexpand(false)
            .vexpand(true)
            .child(&grid_view)
            .build();

        scrolled_window.connect_edge_reached(
            clone!(@strong provider, @strong sender, @strong self as window => move|_window,position|{
                let category = window.current_category();
                if let PositionType::Bottom = position {
                    RUNTIME.spawn(clone!(@strong provider, @strong sender, @strong window => async move{
                        let sender = &sender;
                        provider.load_images(&sender, category).await;
                    }));
                }
            }),
        );

        self.imp().main_box.append(&scrolled_window);
    }

    fn current_category(&self) -> Category {
        Category::from(self.imp().settings.int("category"))
    }

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let imp = self.imp();

        let (width, height) = self.default_size();

        imp.settings.set_int("window-width", width)?;
        imp.settings.set_int("window-height", height)?;

        imp.settings
            .set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn prepare_grid_view(&self, provider: Arc<Wallhaven>, model: SingleSelection) -> GridView {
        let grid_view: GridView = GridView::builder()
            .model(&model)
            .factory(&self.prepare_factory())
            .build();

        grid_view.connect_activate(clone!(@strong self as window, @strong provider => move|grid_view, pos| {
            let model = grid_view.model().unwrap();
            let image_data = model.item(pos)
                .unwrap()
                .downcast::<ImageData>()
                .unwrap();

            let url = image_data.property::<String>("path");

            let (sender, receiver) = MainContext::channel::<Result<String, Box<dyn Error + Send + Sync>>>(glib::PRIORITY_DEFAULT);
            window.send_toast("Downloading your new wallpaper ????", Some(2));
            RUNTIME.spawn(clone!(@strong provider => async move{
                let path = provider.download_wallpaper(url.to_string()).await;
                sender.send(path).unwrap();
            }));

            receiver.attach(None,clone!(@strong window => move |result| {
                match result{
                    Ok(path) => {
                        MainContext::default().spawn_local(clone!(@strong window => async move {
                            let root = window.native().unwrap();
                            let identifier = WindowIdentifier::from_native(&root).await;
                            let file = File::open(path).unwrap();

                            let result = ashpd::desktop::wallpaper::set_from_file(&identifier, &file, true, SetOn::Background).await;

                            match result {
                                Ok(_) => window.send_toast("Enjoy ????", Some(3)),
                                Err(e) => {
                                    match e {
                                        ashpd::Error::Response(e) => {
                                            match e {
                                                ResponseError::Cancelled => {}
                                                ResponseError::Other => {
                                                    if let Err(_) = ashpd::desktop::open_uri::open_directory(&identifier, &file).await {
                                                        window.send_toast("Something went wrong", Some(3));
                                                    };
                                                },
                                            }
                                        }
                                        _ => {
                                            if let Err(_) = ashpd::desktop::open_uri::open_directory(&identifier, &file).await {
                                                window.send_toast("Something went wrong", Some(3));
                                            };
                                        }
                                    }
                                },
                            };
                        }));
                    },
                    Err(e) => window.send_toast(&e.to_string(), Some(3)),
                }

                Continue(false)
            }));
        }));

        grid_view
    }

    fn prepare_factory(&self) -> SignalListItemFactory {
        let factory = SignalListItemFactory::new();
        factory.connect_setup(|_, list_item| {
            let image = Image::builder()
                .width_request(300)
                .height_request(300)
                .build();

            list_item.set_child(Some(&image));
        });

        factory.connect_bind(|_, list_item| {
            let image_data = list_item
                .item()
                .expect("The item has to exist")
                .downcast::<ImageData>()
                .expect("The item has to be an `ImageData`");

            let texture = image_data.property::<Texture>("texture");

            list_item
                .child()
                .unwrap()
                .downcast::<Image>()
                .unwrap()
                .set_from_paintable(Some(&texture));
        });

        factory
    }

    fn load_window_size(&self) {
        let imp = self.imp();

        let width = imp.settings.int("window-width");
        let height = imp.settings.int("window-height");
        let is_maximized = imp.settings.boolean("is-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }

    pub fn send_toast(&self, message: &str, timeout: Option<u32>) {
        self.imp().toast.add_toast(
            &Toast::builder()
                .title(message)
                .timeout(timeout.unwrap_or(5))
                .build(),
        );
    }
}
