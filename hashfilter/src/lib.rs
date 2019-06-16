use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::hash::Hash;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::path::PathBuf;

use fs2::FileExt;
use siphasher::sip128::{Hasher128, SipHasher};

#[derive(Debug, Default)]
pub struct HashFilter {
    path: Option<PathBuf>,
    last_offset: u64,
    filter: HashSet<u128>,
    pending: Vec<u128>,
}

impl HashFilter {
    pub fn open<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: Some(path.as_ref().to_owned()),
            .. Self::default()
        }
    }

    pub fn set_backing<P: AsRef<Path>>(&mut self, path: P) {
        self.path = Some(path.as_ref().to_owned());
        self.last_offset = 0;
    }

    pub fn load(&mut self) -> io::Result<()> {
        if self.path.is_none() {
            return Ok(());
        }

        let mut file = match File::open(self.path.as_ref().unwrap()) {
            Ok(file) => file,
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        file.lock_shared()?;

        if self.last_offset > 0 {
            file.seek(SeekFrom::Start(self.last_offset))?;
        }

        let mut file = BufReader::new(file);
        let mut buf = [0; 16];
        loop {
            match file.read_exact(&mut buf) {
                Ok(()) => self.filter.insert(u128::from_le_bytes(buf)),
                Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            };

            self.last_offset += 16;
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        if self.path.is_none() || self.pending.is_empty() {
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(self.path.as_ref().unwrap())?;
        file.lock_exclusive()?;

        let end = file.metadata()?.len();
        if end % 16 != 0 {
            file.set_len(end - (end % 16))?;
        }
        file.seek(SeekFrom::End(0))?;

        let mut file = BufWriter::new(file);

        while let Some(key) = self.pending.pop() {
            file.write_all(&key.to_le_bytes())?;
        }

        if end == self.last_offset {
            self.last_offset = file.seek(SeekFrom::End(0))?;
        }

        file.into_inner()?.sync_all()?;

        Ok(())
    }

    pub fn insert<H: Hash>(&mut self, data: H) -> bool {
        let key = Self::key_for(data);

        if self.filter.insert(key) {
            self.pending.push(key);
            return true;
        }

        false
    }

    pub fn contains<H: Hash>(&self, data: H) -> bool {
        self.filter.contains(&Self::key_for(data))
    }

    fn key_for<H: Hash>(data: H) -> u128 {
        let mut hash = SipHasher::new();
        data.hash(&mut hash);
        let h = hash.finish128();
        (u128::from(h.h1) << 64) | u128::from(h.h2)
    }
}

#[test]
fn it_seems_to_work() {
    let dir = tempdir::TempDir::new("hashfilter-test").unwrap();
    let db = dir.path().join("test.dat");
    let mut hf = HashFilter::open(&db);
    let _ = hf.load();

    let paths = vec![
        PathBuf::from("/path/to/some/file0"),
        PathBuf::from("/path/to/some/file1"),
        PathBuf::from("/path/to/some/file2"),
        PathBuf::from("/path/to/some/file3"),
        PathBuf::from("/path/to/some/file4"),
        PathBuf::from("/path/to/some/file5"),
        PathBuf::from("/path/to/some/file6"),
        PathBuf::from("/path/to/some/file7"),
        PathBuf::from("/path/to/some/file8"),
        PathBuf::from("/path/to/some/file9"),
    ];

    for p in &paths {
        hf.insert(p);
    }

    hf.save().unwrap();

    let mut hf2 = HashFilter::open(&db);
    hf2.load().unwrap();
    hf2.insert(PathBuf::from("/path/to/some/file10"));
    hf2.save().unwrap();

    for p in &paths {
        assert!(hf2.contains(p));
    }

    hf.load().unwrap();
    assert!(hf.contains(PathBuf::from("/path/to/some/file10")));
}
