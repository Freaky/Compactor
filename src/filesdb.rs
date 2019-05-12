use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use lazy_static::lazy_static;

lazy_static! {
    static ref DB: Mutex<HashSet<PathBuf>> = Mutex::new(HashSet::<PathBuf>::new());
}

pub struct FilesDb;

impl FilesDb {
    // pub fn load() -> HashSet<PathBuf> {
    //     HashSet::new()
    // }

    // pub fn save(db: &HashSet<PathBuf>) -> io::Result<()> {
    //     Ok(())
    // }

    pub fn borrow() -> MutexGuard<'static, HashSet<PathBuf>> {
        DB.lock().expect("DB lock")
    }
}
