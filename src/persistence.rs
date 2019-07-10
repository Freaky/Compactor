use directories::ProjectDirs;
use hashfilter::HashFilter;
use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};

use crate::config::ConfigFile;

lazy_static! {
    static ref PATHDB: Arc<RwLock<HashFilter>> = Arc::new(RwLock::new(HashFilter::default()));
    static ref CONFIG: Arc<RwLock<ConfigFile>> = Arc::new(RwLock::new(ConfigFile::default()));
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

pub fn config() -> Arc<RwLock<ConfigFile>> {
    CONFIG.clone()
}

pub fn pathdb() -> Arc<RwLock<HashFilter>> {
    PATHDB.clone()
}
