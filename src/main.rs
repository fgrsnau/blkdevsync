use std::convert::TryInto;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::{Duration, Instant};
use std::process::exit;

#[derive(Clone, Copy)]
struct Stats {
    count_ok:  u64,
    count_bad: u64,
    blocksize: u64,
    total:     u64,
}

impl Stats {
    fn new(size: u64, blocksize: u32) -> Stats {
        let blocksize64 = u64::from(blocksize);
        let total = (size + blocksize64 + 1) / blocksize64;
        Stats { count_ok: 0, count_bad: 0, blocksize: blocksize64, total: total }
    }
}

fn output_progress(start: Instant, duration: Duration, stats: Stats) {
    let s = duration.as_secs() % 60;
    let m = duration.as_secs() / 60 % 60;
    let h = duration.as_secs() / 3600;
    let read = stats.count_ok + stats.count_bad;
    let to_read = stats.total - read;
    let percentage = 100.0 * read as f64 / stats.total as f64;
    let mibs = read * stats.blocksize / 1024 / 1024 / start.elapsed().as_secs();

    println!(
        "[{:02}:{:02}:{:02} | {:5.1}%] {} ok, {} bad, {} remaining ({:.2} MiB/s)",
        h, m, s, percentage, stats.count_ok, stats.count_bad, to_read, mibs
    );
}

fn sync_block_dev(src: &mut File, dst: &mut File, blocksize: u32) -> io::Result<()> {
    let src_size = src.seek(SeekFrom::End(0))?;
    let dst_size = dst.seek(SeekFrom::End(0))?;

    if src_size > dst_size {
        dst.set_len(src_size)?;
    }

    src.seek(SeekFrom::Start(0))?;
    dst.seek(SeekFrom::Start(0))?;

    let mut src_buf = vec![0; blocksize.try_into().unwrap()];
    let mut dst_buf = vec![0; blocksize.try_into().unwrap()];
    let mut to_read = src_size;

    let time_start = Instant::now();
    let time_every = Duration::from_secs(30);
    let mut time_duration = time_every.clone();
    let mut stats = Stats::new(src_size, blocksize);

    while to_read > 0 {
        if to_read < stats.blocksize {
            src_buf.truncate(to_read.try_into().unwrap());
            dst_buf.truncate(to_read.try_into().unwrap());
        }

        src.read_exact(&mut src_buf)?;
        dst.read_exact(&mut dst_buf)?;

        if src_buf == dst_buf {
            stats.count_ok += 1;
        } else {
            dst.seek(SeekFrom::Current(-i64::from(blocksize)))?;
            dst.write_all(&src_buf)?;
            stats.count_bad += 1;
        }

        if time_start.elapsed() >= time_duration {
            output_progress(time_start, time_duration, stats);
            time_duration += time_every;
        }

        to_read -= stats.blocksize;
    }

    output_progress(time_start, time_start.elapsed(), stats);

    Ok(())
}

fn main() {
    let src_filename = std::env::args().nth(1).expect("no source file provided");
    let dst_filename = std::env::args().nth(2).expect("no destination file provided");
    let blocksize_str = std::env::args().nth(3);

    let blocksize = match blocksize_str {
        Some(x) => x.parse().unwrap(),
        _ => 4096,
    };

    let mut src = OpenOptions::new()
        .read(true)
        .open(src_filename)
        .expect("error opening source file");

    let mut dst = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(dst_filename)
        .expect("error opening destination file");

    sync_block_dev(&mut src, &mut dst, blocksize).unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        exit(1);
    });
}
