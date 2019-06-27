
use directories::ProjectDirs;

use crate::filesdb::FilesDb;
use crate::settings;

pub fn load() {
    let dirs = ProjectDirs::from("", "Freaky", "Compactor").expect("dirs");

    if !dirs.cache_dir().exists() {
        let _ = std::fs::create_dir_all(dirs.cache_dir());
    }

    FilesDb::borrow().set_backing(dirs.cache_dir().join("incompressible.dat"));


    if !dirs.config_dir().exists() {
        let _ = std::fs::create_dir_all(dirs.config_dir());
    }

    settings::load(dirs.config_dir().join("config.json"));
}
