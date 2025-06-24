use crate::api::wallhaven::client::{Category, Client};
use crate::application::WallpaperSelectorApplication;
use crate::config::{APP_ID, PROFILE};
use crate::helpers::set_wallpaper;
use crate::image_data::ImageData;
use crate::provider::wallhaven::{ProviderMessage, Wallhaven};
use crate::RUNTIME;
use adw::gdk::Texture;
use adw::gio::ListStore;
use adw::glib::{clone, timeout_future_seconds, Object};
use adw::Toast;
use adw::{gio, glib};
use ashpd::desktop::open_uri::OpenDirectoryRequest;
use ashpd::desktop::ResponseError;
use ashpd::WindowIdentifier;
use async_channel::Sender;
use gettextrs::gettext;
use gtk::glib::spawn_future_local;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{GridView, Image, PositionType, ScrolledWindow, SignalListItemFactory, SingleSelection};
use std::fs::File;
use std::sync::atomic::Ordering;
use std::sync::Arc;

mod imp {
    use adw::subclass::application_window::AdwApplicationWindowImpl;
    use adw::ToastOverlay;
    use gtk::CompositeTemplate;
    use std::cell::Cell;
    use std::sync::atomic::AtomicBool;

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
        #[template_child]
        pub wallpapers_page: TemplateChild<adw::ViewStackPage>,
        #[template_child]
        pub downloads_box: TemplateChild<gtk::Box>,
        #[template_child]
        downloads_page: TemplateChild<adw::ViewStackPage>,
        #[template_child]
        pub wallpapers_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub stack: TemplateChild<adw::ViewStack>,
        pub downloads_model: ListStore,
        pub settings: gio::Settings,
        pub is_loading: AtomicBool,
        pub downloads_loaded: Cell<bool>,
    }

    impl Default for WallpaperSelectorWindow {
        fn default() -> Self {
            Self {
                header_bar: Default::default(),
                main_box: Default::default(),
                toast: Default::default(),
                wallpapers_page: Default::default(),
                downloads_box: Default::default(),
                downloads_page: Default::default(),
                wallpapers_box: Default::default(),
                downloads_model: ListStore::new::<ImageData>(),
                stack: Default::default(),
                settings: gio::Settings::new(APP_ID),
                is_loading: AtomicBool::new(false),
                downloads_loaded: Cell::new(false),
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
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            self.stack
                .connect_visible_child_notify(clone!(@weak self as window => move |stack| {
                if !window.downloads_loaded.get() {
                    if let Some(child) = stack.visible_child() {
                    if stack.page(&child) == window.downloads_page.get() {
                        window.obj().build_downloads_page();
                            window.downloads_loaded.set(true);
                        }
                }
                }}));

            // Load latest window state
            obj.load_window_size();
        }
    }

    impl WidgetImpl for WallpaperSelectorWindow {}

    impl WindowImpl for WallpaperSelectorWindow {
        // Save window state on delete event
        fn close_request(&self) -> glib::Propagation {
            if let Err(err) = self.obj().save_window_size() {
                log::warn!("Failed to save window state, {}", &err);
            }

            // Pass close request on to the parent
            self.parent_close_request()
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
        Object::builder().property("application", app).build()
    }

    fn load(&self, provider: Arc<Wallhaven>, sender: Arc<Sender<ProviderMessage>>) {
        // Return if images are loading
        if self.imp().is_loading.load(Ordering::Relaxed) {
            return;
        }

        self.imp().is_loading.store(true, Ordering::Relaxed);
        let category = self.current_category();
        RUNTIME.spawn(async move {
            provider.load_images(&sender, category).await;
            sender
                .send(ProviderMessage::ImagesLoaded)
                .await
                .expect("Failed to send ProviderMessage::Loading");
        });
    }

    pub fn build_downloads_page(&self) {
        let selection_model = SingleSelection::new(Some(self.imp().downloads_model.clone()));
        let grid_view: GridView = GridView::builder()
            .model(&selection_model)
            .factory(&self.prepare_factory())
            .build();
        let scrolled_window = ScrolledWindow::builder()
            .hexpand(false)
            .vexpand(true)
            .child(&grid_view)
            .build();

        grid_view.connect_activate(clone!(@strong self as window => move|grid_view, pos| {
            let model = grid_view.model().unwrap();
            let image_data = model.item(pos)
                .unwrap()
                .downcast::<ImageData>()
                .unwrap();

            spawn_future_local(clone!(@strong window => async move{
                            let path = image_data.property::<String>("path");
                            let file = File::open(path).unwrap();
                            let root = window.native().unwrap();
                            let identifier = WindowIdentifier::from_native(&root).await;
                            let result = set_wallpaper(identifier, &file).await;

                            match result {
                                Ok(_) => window.send_toast(&gettext("Enjoy 🤘"), Some(3)),
                                Err(e) => {
                                    match e {
                                        ashpd::Error::Response(e) => {
                                            match e {
                                                ResponseError::Cancelled => {}
                                                ResponseError::Other => {
                                                    if OpenDirectoryRequest::default().send(&file).await.is_err(){
                                                        window.send_toast(&gettext("Something went wrong"), Some(3));
                                                    }
                                                },
                                            }
                                        }
                                        _ => {
                                            if OpenDirectoryRequest::default().send(&file).await.is_err(){
                                                window.send_toast(&gettext("Something went wrong"), Some(3));
                                            }
                                        }
                                    }
                                },
                            }
            }));
        }));

        let (sender, receiver) = async_channel::unbounded::<std::path::PathBuf>();

        RUNTIME.spawn(async move {
            let path = std::env::var("XDG_DATA_HOME").unwrap();

            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            if ["png", "jpg"].contains(&ext.to_lowercase().as_str()) {
                                sender.send(path).await.unwrap();
                            }
                        }
                    }
                }
            }
        });

        spawn_future_local(clone!(@strong self as window => async move {
            while let Ok(path) = receiver.recv().await {
                if let Ok(texture) = Texture::from_filename(&path) {
                    let image_data = ImageData::new(path.to_string_lossy().to_string(), texture);
                    window.imp().downloads_model.append(&image_data);
                    timeout_future_seconds(0).await;
                }
            }
        }));

        self.imp().downloads_box.append(&scrolled_window);
    }

    pub fn build_grid(&self) {
        let model = ListStore::new::<ImageData>();
        let client = Client::new(None);

        let (sender, receiver) = async_channel::unbounded::<ProviderMessage>();
        let sender = Arc::new(sender);

        spawn_future_local(clone!(@strong self as window, @strong model => async move{
            while let Ok(provider_message) = receiver.recv().await {
                match provider_message {
                    ProviderMessage::Image(path, texture) => {
                        let image_data = ImageData::new(path,texture);
                        model.append(&image_data);
                    },
                        ProviderMessage::Reset => model.remove_all(),
                        ProviderMessage::ImagesLoaded => {
                        window.imp().is_loading.store(false, Ordering::Relaxed);
                    }}
                }
        }));

        let provider = Arc::new(Wallhaven::new(client));
        let selection_model = SingleSelection::new(Some(model.clone()));

        let grid_view = self.prepare_grid_view(Arc::clone(&provider), selection_model);
        self.load(Arc::clone(&provider), Arc::clone(&sender));

        // Reset grid on category change (needs refactoring)
        self.imp().settings.connect_changed(Some("category"), clone!(@strong model, @strong self as window, @strong provider, @strong sender =>  move|_, _| {
            provider.reset();
            model.remove_all();
            window.load(Arc::clone(&provider), Arc::clone(&sender));
        }));

        let scrolled_window = ScrolledWindow::builder()
            .hexpand(false)
            .vexpand(true)
            .child(&grid_view)
            .build();

        scrolled_window.connect_edge_reached(
            clone!(@strong provider, @strong sender, @strong self as window => move|_,position|{
                if let PositionType::Bottom = position {
                    window.load(Arc::clone(&provider), Arc::clone(&sender));
                }
            }),
        );

        self.imp().wallpapers_box.append(&scrolled_window);
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

            let (sender, receiver) = async_channel::unbounded();
            window.send_toast(&gettext("Downloading your new wallpaper 🙂"), Some(2));
            RUNTIME.spawn(clone!(@strong provider => async move{
                let path = provider.download_wallpaper(url.to_string()).await;
                sender.send(path).await.unwrap();
            }));

        spawn_future_local(clone ! ( @ strong window, @ strong receiver => async move{
            while let Ok(message) = receiver.recv().await {
                match message {
                    Ok(path) => {
                        let root = window.native().unwrap();
                        let identifier = WindowIdentifier::from_native(&root).await;
                        let file = File::open( & path).unwrap();
                        let result = set_wallpaper(identifier, &file).await;
                        window.imp().downloads_model.append(&ImageData::new(path.clone(), Texture::from_filename(&path).unwrap()));
                            match result {
                                Ok(_) => window.send_toast(&gettext("Enjoy 🤘"), Some(3)),
                                Err(e) => {
                                    match e {
                                        ashpd::Error::Response(e) => {
                                            match e {
                                                ResponseError::Cancelled => {}
                                                ResponseError::Other => {
                                                    if OpenDirectoryRequest::default().send(&file).await.is_err(){
                                                        window.send_toast(&gettext("Something went wrong"), Some(3));
                                                    }
                                                },
                                            }
                                        }
                                        _ => {
                                            if OpenDirectoryRequest::default().send(&file).await.is_err(){
                                                window.send_toast(&gettext("Something went wrong"), Some(3));
                                            }
                                        }
                                    }
                                },
                            }

                        },
                        Err(e) => window.send_toast(&e.to_string(), Some(3)),
                    }
                }
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
        self.add_toast(
            Toast::builder()
                .title(message)
                .timeout(timeout.unwrap_or(5))
                .build(),
        )
    }
    pub fn add_toast(&self, toast: Toast) {
        self.imp().toast.add_toast(toast);
    }
}
