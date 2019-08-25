use directories::ProjectDirs;
use hashfilter::HashFilter;
use lazy_static::lazy_static;
use std::sync::RwLock;

use crate::config::ConfigFile;

lazy_static! {
    static ref PATHDB: RwLock<HashFilter> = RwLock::new(HashFilter::default());
    static ref CONFIG: RwLock<ConfigFile> = RwLock::new(ConfigFile::default());
}

pub fn init() {
    if let Some(dirs) = ProjectDirs::from("", "Freaky", "Compactor") {
        pathdb()
            .write()
            .unwrap()
            .set_backing(dirs.cache_dir().join("incompressible.dat"));
        *config().write().unwrap() = ConfigFile::new(dirs.config_dir().join("config.json"));
    }
}

pub fn config() -> &'static RwLock<ConfigFile> {
    &CONFIG
}

pub fn pathdb() -> &'static RwLock<HashFilter> {
    &PATHDB
}
