use adw::glib;
use adw::glib::Object;
use adw::prelude::*;
use adw::subclass::prelude::ObjectSubclassIsExt;
use gtk::gio::SettingsBindFlags;

mod imp {
    use adw::gio::Settings;
    use adw::glib;
    use adw::subclass::prelude::*;
    use adw::{gio, SwitchRow};
    use gtk::CompositeTemplate;

    use crate::config::APP_ID;
    use crate::glib::subclass::InitializingObject;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/io/github/davidoc26/wallpaper_selector/ui/preferences.ui")]
    pub struct PreferencesWindow {
        pub settings: gio::Settings,
        #[template_child]
        pub category_general: TemplateChild<SwitchRow>,
        #[template_child]
        pub category_anime: TemplateChild<SwitchRow>,
        #[template_child]
        pub category_people: TemplateChild<SwitchRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "PreferencesDialog";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesDialog;

        fn new() -> Self {
            Self {
                settings: Settings::new(APP_ID),
                category_general: TemplateChild::default(),
                category_anime: TemplateChild::default(),
                category_people: TemplateChild::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl AdwDialogImpl for PreferencesWindow {}

    impl PreferencesDialogImpl for PreferencesWindow {}

    impl ObjectImpl for PreferencesWindow {}

    impl WidgetImpl for PreferencesWindow {}

    impl WindowImpl for PreferencesWindow {}
}

glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends gtk::Widget, gtk::Dialog, adw::Dialog, adw::PreferencesDialog;
}

impl PreferencesWindow {
    pub fn new() -> Self {
        let window: Self = Object::builder().build();
        window.bind_settings();

        window
    }

    pub fn bind_settings(&self) {
        self.imp()
            .settings
            .bind("category-general", &*self.imp().category_general, "active")
            .flags(SettingsBindFlags::DEFAULT)
            .build();
        self.imp()
            .settings
            .bind("category-anime", &*self.imp().category_anime, "active")
            .flags(SettingsBindFlags::DEFAULT)
            .build();
        self.imp()
            .settings
            .bind("category-people", &*self.imp().category_people, "active")
            .flags(SettingsBindFlags::DEFAULT)
            .build();
    }
}

impl Default for PreferencesWindow {
    fn default() -> Self {
        Self::new()
    }
}
