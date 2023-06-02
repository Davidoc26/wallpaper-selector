use gettextrs::gettext;
use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};
use log::{debug, info};

use crate::config::{APP_ID, PKGDATADIR, PROFILE, VERSION};
use crate::widgets::PreferencesWindow;
use crate::window::WallpaperSelectorWindow;

mod imp {
    use adw::subclass::application::AdwApplicationImpl;
    use adw::Application;
    use glib::WeakRef;
    use std::cell::OnceCell;

    use crate::window::WallpaperSelectorWindow;

    use super::*;

    #[derive(Debug, Default)]
    pub struct WallpaperSelectorApplication {
        pub window: OnceCell<WeakRef<WallpaperSelectorWindow>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WallpaperSelectorApplication {
        const NAME: &'static str = "WallpaperSelectorApplication";
        type Type = super::WallpaperSelectorApplication;
        type ParentType = Application;
    }

    impl ObjectImpl for WallpaperSelectorApplication {}

    impl ApplicationImpl for WallpaperSelectorApplication {
        fn activate(&self) {
            debug!("AdwApplication<WallpaperSelectorApplication>::activate");
            self.parent_activate();
            let app = self.obj();

            if let Some(window) = self.window.get() {
                let window = window.upgrade().unwrap();
                window.present();
                return;
            }

            let window = WallpaperSelectorWindow::new(&app);
            self.window
                .set(window.downgrade())
                .expect("Window already set.");

            app.main_window().build_grid();
            app.main_window().present();
        }

        fn startup(&self) {
            debug!("AdwApplication<WallpaperSelectorApplication>::startup");
            self.parent_startup();
            let app = self.obj();
            // Set icons for shell
            gtk::Window::set_default_icon_name(APP_ID);

            app.setup_css();
            app.setup_gactions();
            app.setup_accels();
        }
    }

    impl AdwApplicationImpl for WallpaperSelectorApplication {}

    impl GtkApplicationImpl for WallpaperSelectorApplication {}
}

glib::wrapper! {
    pub struct WallpaperSelectorApplication(ObjectSubclass<imp::WallpaperSelectorApplication>)
        @extends gio::Application, gtk::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl WallpaperSelectorApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("resource-base-path", "/io/github/davidoc26/wallpaper_selector/")
            .build()
    }

    fn main_window(&self) -> WallpaperSelectorWindow {
        self.imp().window.get().unwrap().upgrade().unwrap()
    }

    fn setup_gactions(&self) {
        // Quit
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(clone!(@weak self as app => move |_, _| {
            // This is needed to trigger the delete event and saving the window state
            app.main_window().close();
            app.quit();
        }));
        self.add_action(&action_quit);

        // About
        let action_about = gio::SimpleAction::new("about", None);
        action_about.connect_activate(clone!(@weak self as app => move |_, _| {
            app.show_about_dialog();
        }));
        self.add_action(&action_about);

        // Preferences
        let action_preferences = gio::SimpleAction::new("preferences", None);
        action_preferences.connect_activate(clone!(@weak self as app => move |_,_|{
            app.show_preferences_window();
        }));
        self.add_action(&action_preferences);
    }

    fn show_preferences_window(&self) {
        let preferences = PreferencesWindow::new();
        preferences.set_transient_for(Some(&self.main_window()));

        preferences.present();
    }

    // Sets up keyboard shortcuts
    fn setup_accels(&self) {
        self.set_accels_for_action("app.quit", &["<Control>q"]);
        self.set_accels_for_action("app.preferences", &["<primary>comma"]);
    }

    fn setup_css(&self) {
        let provider = gtk::CssProvider::new();
        provider.load_from_resource("/io/github/davidoc26/wallpaper_selector/style.css");
        if let Some(display) = gdk::Display::default() {
            gtk::StyleContext::add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    fn show_about_dialog(&self) {
        let dialog = gtk::AboutDialog::builder()
            .logo_icon_name(APP_ID)
            .license_type(gtk::License::Gpl30)
            .website("https://github.com/davidoc26/wallpaper-selector/")
            .version(VERSION)
            .transient_for(&self.main_window())
            .translator_credits(&gettext("translator-credits"))
            .modal(true)
            .authors(vec!["David Eritsyan"])
            .build();

        dialog.present();
    }

    pub fn run(&self) {
        info!("Wallpaper Selector ({})", APP_ID);
        info!("Version: {} ({})", VERSION, PROFILE);
        info!("Datadir: {}", PKGDATADIR);

        ApplicationExtManual::run(self);
    }
}

impl Default for WallpaperSelectorApplication {
    fn default() -> Self {
        Self::new()
    }
}
