use std::io;
use std::path::PathBuf;

use compresstimator::Compresstimator;
use crossbeam_channel::{Receiver, Sender};

use crate::background::Background;
use crate::background::ControlToken;
use crate::compact::{self, Compression};

#[derive(Debug)]
pub struct BackgroundCompactor {
    compression: Option<Compression>,
    files_in: Receiver<Option<(PathBuf, u64)>>,
    files_out: Sender<(PathBuf, io::Result<bool>)>,
}

impl BackgroundCompactor {
    pub fn new(
        compression: Option<Compression>,
        files_in: Receiver<Option<(PathBuf, u64)>>,
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
        let est = Compresstimator::with_block_size(8192);

        for file in &self.files_in {
            if control.is_cancelled_with_pause() {
                break;
            }

            match file {
                Some((file, len)) => {
                    let ret = match self.compression {
                        Some(compression) => match est.compresstimate_file_len(&file, len) {
                            Ok(ratio) if ratio < 0.95 => compact::compress_file(&file, compression),
                            Ok(_) => Ok(false),
                            Err(e) => Err(e),
                        },
                        None => compact::uncompress_file(&file).map(|_| true),
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
