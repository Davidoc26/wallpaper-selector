use adw::glib::once_cell::sync::Lazy;
use gettextrs::{gettext, LocaleCategory};
use gtk::{gio, glib};
use tokio::runtime::Runtime;

use crate::application::WallpaperSelectorApplication;
use crate::config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};

mod application;
#[rustfmt::skip]
mod config;
mod api;
mod image_data;
mod provider;
mod widgets;
mod window;

static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

fn main() {
    // Initialize logger
    pretty_env_logger::init();

    // Prepare i18n
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Wallpaper Selector"));

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    let app = WallpaperSelectorApplication::new();
    app.run();
}
