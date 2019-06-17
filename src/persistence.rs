
use std::path::PathBuf;
use std::fs::File;

use app_dirs::*;

use crate::settings;
use crate::filesdb::FilesDb;

const APP_INFO: AppInfo = AppInfo {
    name: "Compactor",
    author: "Freaky",
};

pub fn conf_dir() -> PathBuf {
    app_root(AppDataType::UserConfig, &APP_INFO).expect("config dir")
}

pub fn cache_dir() -> PathBuf {
    app_root(AppDataType::UserCache, &APP_INFO).expect("cache dir")
}

pub fn load() {
    FilesDb::borrow().set_backing(cache_dir().join("incompressible.dat"));
    settings::load(conf_dir().join("config.json"));
}