//! Session recorder: turns the raw datagram stream into one `.pitrec` per
//! session, with the boundary rules, lifecycle and retention of spec section 4.
//!
//! The recorder is the ingest `DatagramTap`. The tap only copies bytes into a
//! bounded queue, so ingest never waits on disk; a full queue drops the
//! datagram and counts it (`Recorder::dropped`). One writer thread owns the
//! boundary rules and the open file. A second, maintenance thread recompresses
//! closed files and applies retention, so neither competes with the game.
//!
//! Replay marks the recording it loads with `Recorder::mark_loaded`, which keeps
//! retention from deleting it while the guard lives.
mod library;

use crate::{
    ingest::DatagramTap,
    pitrec::{Header, Writer},
    telemetry::HEADER_SIZE,
};
use library::{FINAL_CLASSIFICATION_PACKET, RECOMPRESS_LEVEL, Summary};
pub use library::{Listing, Sidecar, list, read_sidecar, set_pinned};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, BufWriter},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, RecvTimeoutError, Sender, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const QUEUE_CAPACITY: usize = 1 << 16;

#[derive(Clone, Debug)]
pub struct RecorderConfig {
    /// The `recordings` folder.
    pub dir: PathBuf,
    /// Total size cap; the oldest unpinned, unloaded recordings go first.
    pub size_cap_bytes: u64,
    /// Datagrams buffered between ingest and the writer thread.
    pub queue_capacity: usize,
}

impl RecorderConfig {
    pub fn new(dir: PathBuf, size_cap_bytes: u64) -> Self {
        Self {
            dir,
            size_cap_bytes,
            queue_capacity: QUEUE_CAPACITY,
        }
    }
}

enum Message {
    Datagram(u64, Vec<u8>),
    Sync(Sender<io::Result<()>>),
    Shutdown,
}

enum Job {
    /// A recording was just closed: recompress it, then apply retention.
    Finalized(PathBuf),
    Delete(String, Sender<io::Result<()>>),
    Rescan,
    Sync(Sender<io::Result<()>>, io::Result<()>),
    Stop,
}

pub struct Recorder {
    queue: SyncSender<Message>,
    jobs: Sender<Job>,
    cap_bytes: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    loaded: Arc<Mutex<HashMap<String, usize>>>,
    threads: Vec<JoinHandle<()>>,
}

/// Keeps a recording out of automatic deletion while replay has it loaded.
pub struct LoadedGuard {
    loaded: Arc<Mutex<HashMap<String, usize>>>,
    file_name: String,
}

impl Drop for LoadedGuard {
    fn drop(&mut self) {
        let mut loaded = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(count) = loaded.get_mut(&self.file_name) {
            *count -= 1;
            if *count == 0 {
                loaded.remove(&self.file_name);
            }
        }
    }
}

/// `(session_uid, packet_format)` of a datagram that may be recorded. Anything
/// shorter than a header, in an unsupported format, or with UID 0 is rejected.
fn identity(bytes: &[u8]) -> Option<(u64, u16)> {
    if bytes.len() < HEADER_SIZE {
        return None;
    }
    let format = u16::from_le_bytes([bytes[0], bytes[1]]);
    let uid = u64::from_le_bytes(bytes[7..15].try_into().unwrap());
    (matches!(format, 2025 | 2026) && uid != 0).then_some((uid, format))
}

impl Recorder {
    /// Recover files left by a crash, then start recording. `clock` returns the
    /// current UTC time in Unix seconds and is read once per opened recording.
    pub fn start(
        config: RecorderConfig,
        clock: impl Fn() -> i64 + Send + 'static,
    ) -> io::Result<Self> {
        fs::create_dir_all(&config.dir)?;
        let (jobs, job_rx) = mpsc::channel();
        for part in library::part_files(&config.dir)? {
            if let Some(recording) = library::recover(&config.dir, &part)? {
                let _ = jobs.send(Job::Finalized(recording));
            }
        }
        library::repair(&config.dir)?;
        let _ = jobs.send(Job::Rescan);

        let cap_bytes = Arc::new(AtomicU64::new(config.size_cap_bytes));
        let loaded = Arc::new(Mutex::new(HashMap::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let maintenance = {
            let (dir, cap) = (config.dir.clone(), cap_bytes.clone());
            let (loaded, stop) = (loaded.clone(), stop.clone());
            thread::Builder::new()
                .name("pitwall-recorder-maintenance".into())
                .spawn(move || maintain(job_rx, &dir, &cap, &loaded, &stop))?
        };
        let (queue, queue_rx) = mpsc::sync_channel(config.queue_capacity.max(1));
        let writer = {
            let (jobs, stop) = (jobs.clone(), stop.clone());
            thread::Builder::new()
                .name("pitwall-recorder".into())
                .spawn(move || {
                    Writing::new(config.dir, Box::new(clock), jobs).run(&queue_rx);
                    stop.store(true, Ordering::Relaxed);
                })?
        };
        Ok(Self {
            queue,
            jobs,
            cap_bytes,
            dropped: Arc::new(AtomicU64::new(0)),
            loaded,
            threads: vec![writer, maintenance],
        })
    }

    /// Datagrams lost because the writer fell behind; ingest never waits instead.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Wait until everything queued so far is written, closed files are
    /// recompressed and retention has run. For tests and orderly hosts, never ingest.
    /// Returns an error if the writer failed or stopped before acknowledging.
    pub fn sync(&self) -> io::Result<()> {
        let (ack, done) = mpsc::channel();
        self.queue
            .send(Message::Sync(ack))
            .map_err(|_| io::Error::other("recorder writer stopped"))?;
        done.recv()
            .map_err(|_| io::Error::other("recorder stopped before sync"))?
    }

    /// Change the size cap (a host setting); applied at the next retention pass.
    pub fn set_size_cap(&self, bytes: u64) {
        self.cap_bytes.store(bytes, Ordering::Relaxed);
        self.rescan();
    }

    /// Rebuild missing sidecars and apply retention, for files copied into the folder.
    pub fn rescan(&self) {
        let _ = self.jobs.send(Job::Rescan);
    }

    /// Delete a closed recording on the maintenance queue, after recompression.
    /// Loaded replays are protected; callers must supply a validated file name.
    pub fn delete(&self, name: &str) -> io::Result<mpsc::Receiver<io::Result<()>>> {
        validate_recording_name(name)?;
        let (ack, result) = mpsc::channel();
        self.jobs
            .send(Job::Delete(name.into(), ack))
            .map_err(|_| io::Error::other("recorder stopped"))?;
        Ok(result)
    }

    /// Mark a recording (by file name) as loaded in replay until the guard drops.
    pub fn mark_loaded(&self, file_name: &str) -> LoadedGuard {
        *self
            .loaded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(file_name.to_owned())
            .or_default() += 1;
        LoadedGuard {
            loaded: self.loaded.clone(),
            file_name: file_name.to_owned(),
        }
    }

    /// Collector exit: close the open recording and stop. Unfinished
    /// recompression is abandoned; the closed file is already valid.
    pub fn shutdown(self) {}
}

impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.queue.send(Message::Shutdown);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl DatagramTap for Recorder {
    fn datagram(&mut self, arrival_time: u64, bytes: &[u8]) {
        if identity(bytes).is_none() {
            return;
        }
        if self
            .queue
            .try_send(Message::Datagram(arrival_time, bytes.to_vec()))
            .is_err()
        {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

struct Open {
    identity: (u64, u16),
    part: PathBuf,
    writer: Writer<BufWriter<fs::File>>,
    header: Header,
    summary: Summary,
    last_us: u64,
}

struct Writing {
    dir: PathBuf,
    clock: Box<dyn Fn() -> i64 + Send>,
    jobs: Sender<Job>,
    /// Identities closed during this run; never reopened, never persisted.
    closed: HashSet<(u64, u16)>,
    current: Option<Open>,
    last_error: Option<(io::ErrorKind, String)>,
}

impl Writing {
    fn new(dir: PathBuf, clock: Box<dyn Fn() -> i64 + Send>, jobs: Sender<Job>) -> Self {
        Self {
            dir,
            clock,
            jobs,
            closed: HashSet::new(),
            current: None,
            last_error: None,
        }
    }

    fn run(mut self, queue: &mpsc::Receiver<Message>) {
        loop {
            match queue.recv_timeout(Duration::from_secs(1)) {
                Ok(Message::Datagram(at, bytes)) => self.datagram(at, &bytes),
                Ok(Message::Sync(ack)) => {
                    self.flush();
                    let result = self.last_error.as_ref().map_or(Ok(()), |(kind, message)| {
                        Err(io::Error::new(*kind, message.clone()))
                    });
                    // Forward through maintenance so the caller also waits for it.
                    let _ = self.jobs.send(Job::Sync(ack, result));
                }
                // Silence closes nothing; it only makes buffered records durable.
                Err(RecvTimeoutError::Timeout) => self.flush(),
                Ok(Message::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        self.close_current();
        let _ = self.jobs.send(Job::Stop);
    }

    fn flush(&mut self) {
        if let Some(open) = &mut self.current
            && let Err(error) = open.writer.flush_block()
        {
            self.fail(error);
        }
    }

    fn datagram(&mut self, at: u64, bytes: &[u8]) {
        let Some(id) = identity(bytes) else { return };
        if self.closed.contains(&id) {
            return;
        }
        if self.current.as_ref().is_some_and(|o| o.identity != id) {
            self.close_current();
        }
        if self.current.is_none() {
            match self.open(id, bytes) {
                Ok(open) => self.current = Some(open),
                Err(error) => {
                    eprintln!("Recorder could not open a file for session {id:?}: {error}");
                    self.last_error = Some((error.kind(), error.to_string()));
                    self.closed.insert(id);
                    return;
                }
            }
        }
        let open = self.current.as_mut().expect("opened above");
        // The writer requires monotonic times; clamp a clock hiccup.
        let at = at.max(open.last_us);
        open.last_us = at;
        if let Err(error) = open.writer.write(at, bytes) {
            return self.fail(error);
        }
        open.summary.observe(at, bytes);
        if bytes[6] == FINAL_CLASSIFICATION_PACKET
            && crate::telemetry::parse_datagram(bytes).is_ok()
        {
            self.close_current();
        }
    }

    fn open(&self, identity: (u64, u16), first: &[u8]) -> io::Result<Open> {
        let part = self
            .dir
            .join(format!("{}-{}.pitrec.part", identity.0, identity.1));
        let header = Header {
            packet_format: identity.1,
            game_version: format!("{}.{}.{}", first[2], first[3], first[4]),
            pitwall_version: env!("CARGO_PKG_VERSION").to_owned(),
            started_utc: library::rfc3339((self.clock)()),
        };
        let file = fs::File::create(&part)?;
        Ok(Open {
            identity,
            part,
            writer: Writer::new(BufWriter::new(file), &header)?,
            header,
            summary: Summary::default(),
            last_us: 0,
        })
    }

    /// A write failed: stop recording this session and keep what was flushed.
    fn fail(&mut self, error: io::Error) {
        self.last_error = Some((error.kind(), error.to_string()));
        if let Some(open) = self.current.take() {
            eprintln!("Recorder stopped session {:?}: {error}", open.identity);
            self.closed.insert(open.identity);
        }
    }

    fn close_current(&mut self) {
        let Some(open) = self.current.take() else {
            return;
        };
        self.closed.insert(open.identity);
        let Open {
            writer,
            part,
            header,
            summary,
            ..
        } = open;
        let result = writer
            .finish()
            .and_then(|out| out.into_inner().map_err(|e| e.into_error()))
            .and_then(|file| file.sync_all())
            .and_then(|()| library::publish(&self.dir, &part, &header, &summary));
        match result {
            Ok(Some(recording)) => {
                let _ = self.jobs.send(Job::Finalized(recording));
            }
            Ok(None) => {}
            Err(error) => {
                self.last_error = Some((error.kind(), error.to_string()));
                eprintln!("Recorder could not close {}: {error}", part.display());
            }
        }
    }
}

fn maintain(
    jobs: mpsc::Receiver<Job>,
    dir: &Path,
    cap_bytes: &AtomicU64,
    loaded: &Mutex<HashMap<String, usize>>,
    stop: &AtomicBool,
) {
    let sweep = || {
        if let Err(error) = library::sweep(dir, cap_bytes, loaded) {
            eprintln!("Recorder retention failed: {error}");
        }
    };
    while let Ok(job) = jobs.recv() {
        match job {
            Job::Finalized(recording) => {
                if !stop.load(Ordering::Relaxed)
                    && let Err(error) = library::recompress(&recording, RECOMPRESS_LEVEL, stop)
                    && !stop.load(Ordering::Relaxed)
                {
                    eprintln!(
                        "Recorder could not recompress {}: {error}",
                        recording.display()
                    );
                }
                sweep();
            }
            Job::Rescan => {
                let _ = library::repair(dir);
                sweep();
            }
            Job::Sync(ack, result) => {
                let _ = ack.send(result);
            }
            Job::Delete(name, ack) => {
                let loaded = loaded.lock().unwrap();
                let result = if loaded.contains_key(&name) {
                    Err(io::Error::new(
                        io::ErrorKind::ResourceBusy,
                        "recording loaded in replay",
                    ))
                } else {
                    let result_path = recording_path(dir, &name);
                    result_path
                        .and_then(|path| fs::remove_file(&path).map(|()| path))
                        .and_then(|path| match fs::remove_file(path.with_extension("json")) {
                            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                            _ => Ok(()),
                        })
                };
                let _ = ack.send(result);
            }
            Job::Stop => break,
        }
    }
}

/// Validate a single recording filename before joining it to the library directory.
pub fn validate_recording_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.starts_with('.')
        || name.contains(['/', '\\', ':'])
        || !name.ends_with(".pitrec")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid recording name",
        ));
    }
    Ok(())
}

/// Resolve an existing regular recording, rejecting symlinks and traversal.
pub fn recording_path(dir: &Path, name: &str) -> io::Result<PathBuf> {
    validate_recording_name(name)?;
    let path = dir.join(name);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid recording file",
        ));
    }
    Ok(path)
}
