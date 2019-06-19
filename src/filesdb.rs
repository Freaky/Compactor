use std::sync::{Mutex, MutexGuard};

use hashfilter::HashFilter;

use lazy_static::lazy_static;

lazy_static! {
    static ref DB: Mutex<HashFilter> = Mutex::new(HashFilter::open("incompressible.dat"));
}

pub struct FilesDb;

impl FilesDb {
    pub fn borrow() -> MutexGuard<'static, HashFilter> {
        DB.lock().expect("DB lock")
    }
}
