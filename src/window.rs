use crate::api::wallhaven::client::{Client, SearchOptions, Sorting};
use crate::application::WallpaperSelectorApplication;
use crate::config::{APP_ID, PROFILE};
use crate::helpers::set_wallpaper;
use crate::image_data::ImageData;
use crate::provider::wallhaven::{ProviderMessage, Wallhaven};
use crate::RUNTIME;
use adw::gdk::gdk_pixbuf::Pixbuf;
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
use std::os::fd::AsFd;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

mod imp {
    use adw::subclass::application_window::AdwApplicationWindowImpl;
    use adw::ToastOverlay;
    use glib::SourceId;
    use gtk::CompositeTemplate;
    use std::cell::{Cell, OnceCell, RefCell};
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
        #[template_child]
        pub wallpapers_sorting: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub spinner: TemplateChild<adw::Spinner>,
        pub downloads_model: ListStore,
        pub settings: gio::Settings,
        pub is_loading: AtomicBool,
        pub downloads_loaded: Cell<bool>,
        pub model: OnceCell<ListStore>,
        pub provider_sender: OnceCell<Arc<Sender<ProviderMessage>>>,
        pub category_debounce: RefCell<Option<SourceId>>,
        pub provider: OnceCell<Arc<Wallhaven>>,
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
                wallpapers_sorting: Default::default(),
                spinner: Default::default(),
                settings: gio::Settings::new(APP_ID),
                is_loading: AtomicBool::new(false),
                downloads_loaded: Cell::new(false),
                model: Default::default(),
                provider_sender: OnceCell::new(),
                category_debounce: RefCell::new(None),
                provider: OnceCell::new(),
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

            self.stack.connect_visible_child_notify(clone!(
                #[weak(rename_to = window)]
                self,
                move |stack| {
                    if !window.downloads_loaded.get() {
                        if let Some(child) = stack.visible_child() {
                            if stack.page(&child) == window.downloads_page.get() {
                                window.obj().build_downloads_page();
                                window.downloads_loaded.set(true);
                            }
                        }
                    }
                }
            ));

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
        @extends gtk::Widget, gtk::Window, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
}

impl WallpaperSelectorWindow {
    pub fn new(app: &WallpaperSelectorApplication) -> Self {
        Object::builder().property("application", app).build()
    }

    fn provider_sender(&self) -> Arc<Sender<ProviderMessage>> {
        Arc::clone(
            self.imp()
                .provider_sender
                .get()
                .expect("provider_sender accessed before build_grid was called"),
        )
    }

    fn load(&self) {
        // Return if images are loading
        if self.imp().is_loading.load(Ordering::Relaxed) {
            return;
        }

        self.imp().is_loading.store(true, Ordering::Relaxed);
        self.lock_sorting_dropdown();
        let category_general = self.is_general_category_enabled();
        let category_anime = self.is_anime_category_enabled();
        let category_people = self.is_people_category_enabled();
        let sorting = self.current_sorting();
        let sender = self.provider_sender();
        let provider = self.provider();
        self.imp().spinner.show();

        RUNTIME.spawn(async move {
            let search_options = SearchOptions::default()
                .sorting(sorting)
                .category_general(category_general)
                .category_anime(category_anime)
                .category_people(category_people);
            provider.load_images(&sender, search_options).await;
            sender
                .send(ProviderMessage::ImagesLoaded)
                .await
                .expect("Failed to send ProviderMessage::Loading");
        });
    }

    pub fn show_spinner(&self) {
        self.imp().spinner.set_visible(true);
    }

    pub fn hide_spinner(&self) {
        self.imp().spinner.set_visible(false);
    }

    pub fn lock_sorting_dropdown(&self) {
        self.imp().wallpapers_sorting.set_sensitive(false);
    }

    pub fn unlock_sorting_dropdown(&self) {
        self.imp().wallpapers_sorting.set_sensitive(true);
    }

    pub fn set_sorting(&self) {
        self.imp()
            .wallpapers_sorting
            .set_selected(self.current_sorting().into());
    }

    pub fn is_general_category_enabled(&self) -> bool {
        self.imp().settings.boolean("category-general")
    }

    pub fn is_anime_category_enabled(&self) -> bool {
        self.imp().settings.boolean("category-anime")
    }

    pub fn is_people_category_enabled(&self) -> bool {
        self.imp().settings.boolean("category-people")
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

        grid_view.connect_activate(clone!(
            #[strong(rename_to = window)]
            self,
            move |grid_view, pos| {
                let model = grid_view.model().unwrap();
                let image_data = model.item(pos).unwrap().downcast::<ImageData>().unwrap();

                spawn_future_local(clone!(
                    #[strong]
                    window,
                    async move {
                        let path = image_data.property::<String>("path");
                        let file = File::open(path).unwrap();
                        let root = window.native().unwrap();
                        let identifier = WindowIdentifier::from_native(&root).await;
                        let result = set_wallpaper(identifier, &file).await;

                        match result {
                            Ok(_) => window.send_toast(&gettext("Enjoy 🤘"), Some(3)),
                            Err(e) => match e {
                                ashpd::Error::Response(e) => match e {
                                    ResponseError::Cancelled => {}
                                    ResponseError::Other => {
                                        if OpenDirectoryRequest::default()
                                            .send(&file.as_fd())
                                            .await
                                            .is_err()
                                        {
                                            window.send_toast(
                                                &gettext("Something went wrong"),
                                                Some(3),
                                            );
                                        }
                                    }
                                },
                                _ => {
                                    if OpenDirectoryRequest::default()
                                        .send(&file.as_fd())
                                        .await
                                        .is_err()
                                    {
                                        window
                                            .send_toast(&gettext("Something went wrong"), Some(3));
                                    }
                                }
                            },
                        }
                    }
                ));
            }
        ));

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

        spawn_future_local(clone!(
            #[strong(rename_to = window)]
            self,
            async move {
                while let Ok(path) = receiver.recv().await {
                    if let Ok(texture) = Pixbuf::from_file_at_scale(&path, 200, 200, true) {
                        let image_data = ImageData::new(
                            path.to_string_lossy().to_string(),
                            Texture::for_pixbuf(&texture),
                        );
                        window.imp().downloads_model.append(&image_data);
                        timeout_future_seconds(0).await;
                    }
                }
            }
        ));

        self.imp().downloads_box.append(&scrolled_window);
    }

    pub fn set_model(&self) {
        self.imp()
            .model
            .set(ListStore::new::<ImageData>())
            .expect("set_model can only be called once");
    }

    pub fn get_model(&self) -> ListStore {
        self.imp()
            .model
            .get()
            .expect("get_model called before model initialization")
            .clone()
    }

    pub fn add_image_to_model(&self, image: &ImageData) {
        self.get_model().append(image);
    }

    pub fn clear_model(&self) {
        self.get_model().remove_all();
    }

    pub fn build_grid(&self) {
        self.set_model();
        let client = Client::new(None);

        let (sender, receiver) = async_channel::unbounded::<ProviderMessage>();
        let sender = Arc::new(sender);
        let provider = Arc::new(Wallhaven::new(client));

        self.imp()
            .provider
            .set(Arc::clone(&provider))
            .expect("build_grid must be called only once");

        self.imp()
            .provider_sender
            .set(Arc::clone(&sender))
            .expect("build_grid must be called only once");

        spawn_future_local(clone!(
            #[strong(rename_to = window)]
            self,
            async move {
                while let Ok(provider_message) = receiver.recv().await {
                    match provider_message {
                        ProviderMessage::Image(path, texture) => {
                            let image_data = ImageData::new(path, texture);
                            window.add_image_to_model(&image_data);
                        }
                        ProviderMessage::Reset => window.clear_model(),
                        ProviderMessage::ImagesLoaded => {
                            window.imp().is_loading.store(false, Ordering::Relaxed);
                            window.unlock_sorting_dropdown();
                            window.hide_spinner();
                        }
                    }
                }
            }
        ));

        let selection_model = SingleSelection::new(Some(self.get_model()));

        let grid_view = self.prepare_grid_view(selection_model);
        self.load();

        // Reset grid on category change (needs refactoring)
        self.imp().settings.connect_changed(
            Some("category"),
            clone!(
                #[strong(rename_to = window)]
                self,
                move |_, _| {
                    window.provider().reset();
                    window.clear_model();
                    window.load();
                }
            ),
        );

        self.connect_category_filters_change();

        self.imp()
            .wallpapers_sorting
            .connect_selected_item_notify(clone!(
                #[strong(rename_to = window)]
                self,
                move |item| {
                    let item = item
                        .selected_item()
                        .and_downcast::<gtk::StringObject>()
                        .unwrap();

                    window
                        .imp()
                        .settings
                        .set("wallpapers-sorting", item.string().as_str())
                        .unwrap();
                    window.provider().reset();
                    window.clear_model();
                    window.load();
                }
            ));

        let scrolled_window = ScrolledWindow::builder()
            .hexpand(false)
            .vexpand(true)
            .child(&grid_view)
            .build();

        scrolled_window.connect_edge_reached(clone!(
            #[strong(rename_to = window)]
            self,
            move |_, position| {
                if let PositionType::Bottom = position {
                    window.load();
                }
            }
        ));

        self.imp().wallpapers_box.append(&scrolled_window);
    }

    fn connect_category_filters_change(&self) {
        for key in ["category-general", "category-anime", "category-people"] {
            self.imp().settings.connect_changed(
                Some(key),
                clone!(
                    #[strong(rename_to = window)]
                    self,
                    move |_, _| {
                        window.schedule_reload();
                    }
                ),
            );
        }
    }

    fn schedule_reload(&self) {
        if let Some(source_id) = self.imp().category_debounce.borrow_mut().take() {
            source_id.remove();
        }

        let source_id = glib::timeout_add_local_once(
            Duration::from_millis(1500),
            clone!(
                #[strong(rename_to = window)]
                self,
                move || {
                    window.send_toast(&gettext("Applying changes"), Some(3));
                    window.imp().category_debounce.borrow_mut().take();
                    window.provider().reset();
                    window.clear_model();
                    window.load();
                }
            ),
        );

        *self.imp().category_debounce.borrow_mut() = Some(source_id);
    }

    fn current_sorting(&self) -> Sorting {
        let val = self.imp().settings.string("wallpapers-sorting");
        Sorting::from(val.as_str())
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

    fn provider(&self) -> Arc<Wallhaven> {
        Arc::clone(
            self.imp()
                .provider
                .get()
                .expect("provider accessed before build_grid was called"),
        )
    }

    fn prepare_grid_view(&self, model: SingleSelection) -> GridView {
        let grid_view: GridView = GridView::builder()
            .model(&model)
            .factory(&self.prepare_factory())
            .build();

        grid_view.connect_activate(clone!(
            #[strong(rename_to = window)]
            self,
            move |grid_view, pos| {
                let model = grid_view.model().unwrap();
                let image_data = model.item(pos).unwrap().downcast::<ImageData>().unwrap();

                let url = image_data.property::<String>("path");

                let (sender, receiver) = async_channel::unbounded();
                let provider = window.provider();
                window.send_toast(&gettext("Downloading your new wallpaper 🙂"), Some(2));

                RUNTIME.spawn(clone!(
                    #[strong]
                    provider,
                    async move {
                        let path = provider.download_wallpaper(url.to_string()).await;
                        sender.send(path).await.unwrap();
                    }
                ));

                spawn_future_local(clone!(
                    #[strong]
                    window,
                    #[strong]
                    receiver,
                    async move {
                        while let Ok(message) = receiver.recv().await {
                            match message {
                                Ok(path) => {
                                    let root = window.native().unwrap();
                                    let identifier = WindowIdentifier::from_native(&root).await;
                                    let file = File::open(&path).unwrap();
                                    let result = set_wallpaper(identifier, &file).await;
                                    if window.imp().downloads_loaded.get() {
                                        window.imp().downloads_model.append(&ImageData::new(
                                            path.clone(),
                                            Texture::from_filename(&path).unwrap(),
                                        ));
                                    }
                                    match result {
                                        Ok(_) => window.send_toast(&gettext("Enjoy 🤘"), Some(3)),
                                        Err(e) => match e {
                                            ashpd::Error::Response(e) => match e {
                                                ResponseError::Cancelled => {}
                                                ResponseError::Other => {
                                                    if OpenDirectoryRequest::default()
                                                        .send(&file.as_fd())
                                                        .await
                                                        .is_err()
                                                    {
                                                        window.send_toast(
                                                            &gettext("Something went wrong"),
                                                            Some(3),
                                                        );
                                                    }
                                                }
                                            },
                                            _ => {
                                                if OpenDirectoryRequest::default()
                                                    .send(&file.as_fd())
                                                    .await
                                                    .is_err()
                                                {
                                                    window.send_toast(
                                                        &gettext("Something went wrong"),
                                                        Some(3),
                                                    );
                                                }
                                            }
                                        },
                                    }
                                }
                                Err(e) => window.send_toast(&e.to_string(), Some(3)),
                            }
                        }
                    }
                ));
            }
        ));

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
                .set_paintable(Some(&texture));
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
