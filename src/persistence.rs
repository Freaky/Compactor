use directories::ProjectDirs;

use crate::filesdb::FilesDb;
use crate::settings;

pub fn load() {
    let dirs = ProjectDirs::from("", "Freaky", "Compactor").expect("dirs");

    FilesDb::borrow().set_backing(dirs.cache_dir().join("incompressible.dat"));
    settings::load(dirs.config_dir().join("config.json"));
}
