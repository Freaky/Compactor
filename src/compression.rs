use std::io;
use std::path::PathBuf;

use crossbeam_channel::{Receiver, Sender};

use crate::background::Background;
use crate::background::ControlToken;
use crate::compact::{Compact, Compression};

#[derive(Debug)]
pub struct BackgroundCompactor {
    compression: Option<Compression>,
    files_in: Receiver<Option<PathBuf>>,
    files_out: Sender<(PathBuf, io::Result<bool>)>,
}

impl BackgroundCompactor {
    pub fn new(
        compression: Option<Compression>,
        files_in: Receiver<Option<PathBuf>>,
        files_out: Sender<(PathBuf, io::Result<bool>)>,
    ) -> Self {
        Self {
            compression,
            files_in,
            files_out,
        }
    }
}

impl Background for BackgroundCompactor {
    type Output = ();
    type Status = ();

    fn run(&self, control: &ControlToken<Self::Status>) -> Self::Output {
        for file in &self.files_in {
            if control.is_cancelled_with_pause() {
                break;
            }

            match file {
                Some(file) => {
                    let ret = match self.compression {
                        Some(compression) => Compact::compress_file(&file, compression),
                        None => Compact::uncompress_file(&file).map(|_| true),
                    };

                    if self.files_out.send((file, ret)).is_err() {
                        break;
                    }
                }
                None => {
                    break;
                }
            }
        }
    }
}
