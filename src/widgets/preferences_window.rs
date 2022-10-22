use adw::glib;
use adw::glib::Object;
use adw::prelude::*;
use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::gio::SettingsBindFlags;

mod imp {
    use adw::gio::Settings;
    use adw::glib;
    use adw::subclass::prelude::*;
    use adw::{gio, ComboRow};
    use gtk::prelude::InitializingWidgetExt;
    use gtk::subclass::prelude::*;
    use gtk::CompositeTemplate;

    use crate::config::APP_ID;
    use crate::glib::subclass::InitializingObject;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/io/github/davidoc26/wallpaper_selector/ui/preferences.ui")]
    pub struct PreferencesWindow {
        pub settings: gio::Settings,
        #[template_child]
        pub category_selector: TemplateChild<ComboRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "PreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesWindow;

        fn new() -> Self {
            Self {
                settings: Settings::new(APP_ID),
                category_selector: TemplateChild::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl AdwWindowImpl for PreferencesWindow {}

    impl PreferencesWindowImpl for PreferencesWindow {}

    impl ObjectImpl for PreferencesWindow {}

    impl WidgetImpl for PreferencesWindow {}

    impl WindowImpl for PreferencesWindow {}
}

glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends gtk::Widget, gtk::Window, adw::Window, adw::PreferencesWindow;
}

impl PreferencesWindow {
    pub fn new() -> Self {
        let window: Self = Object::new(&[]).expect("Failed to create PreferencesWindow");
        window.bind_settings();

        window
    }

    pub fn bind_settings(&self) {
        self.imp()
            .settings
            .bind("category", &*self.imp().category_selector, "selected")
            .flags(SettingsBindFlags::DEFAULT)
            .build();
    }
}

impl Default for PreferencesWindow {
    fn default() -> Self {
        Self::new()
    }
}
