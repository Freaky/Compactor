use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use lz4::EncoderBuilder;

#[derive(Debug, Default)]
struct WriteCount {
    written: u64,
}

impl Write for WriteCount {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written += buf.len() as u64;

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum Confidence {
    C80,
    C85,
    C90,
    C95,
    C99,
}

impl From<Confidence> for f32 {
    fn from(c: Confidence) -> f32 {
        match c {
            Confidence::C80 => 1.28,
            Confidence::C85 => 1.44,
            Confidence::C90 => 1.65,
            Confidence::C95 => 1.96,
            Confidence::C99 => 2.58,
        }
    }
}

fn sample_size(pop: u64, moe: u8, confidence: Confidence) -> f32 {
    let pop = pop as f32;
    let n_naught = 0.25 * (f32::from(confidence) / (f32::from(moe) / 100.0)).powi(2);
    ((pop * n_naught) / (n_naught + pop - 1.0)).ceil()
}

pub fn compresstinate<P: AsRef<Path>>(path: P) -> io::Result<f32> {
    let mut input = File::open(path)?;
    let len = input.metadata()?.len();
    let output = WriteCount::default();

    let mut encoder = EncoderBuilder::new().level(1).build(output).expect("lz4");

    if len < 64 * 1024 {
        std::io::copy(&mut input, &mut encoder)?;
        let (output, result) = encoder.finish();
        result?;
        return Ok(output.written as f32 / len as f32);
    }

    let blocks = len / 4096;
    let samples = sample_size(blocks, 15, Confidence::C90) as u64;
    let step = 4096 * (blocks / samples);

    let mut buf = [0; 4096];

    for i in 0..samples {
        input.seek(SeekFrom::Start(step * i))?;
        input.read_exact(&mut buf)?;
        encoder.write_all(&buf)?;
    }

    let (output, result) = encoder.finish();
    result?;

    Ok(output.written as f32 / (4096 * samples) as f32)
}
