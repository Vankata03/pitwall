//! Arrival-time replay. Reads the recording without touching live UDP or its recorder.
mod routes;
use crate::{
    ingest::{Disposition, Ingest},
    model::{Readout, SessionModel},
    pitrec::{Cursor, Reader, Record},
};
pub use routes::{Controller, routes};
use std::{collections::BTreeMap, fs::File, io, time::Duration};

struct Checkpoint {
    at: Duration,
    cursor: Cursor,
    pending: Option<Record>,
    model: SessionModel,
    ingest: Ingest,
}

pub struct Replay {
    reader: Reader<File>,
    pending: Option<Record>,
    checkpoints: Vec<Checkpoint>,
    laps: BTreeMap<(usize, u8), Duration>,
    model: SessionModel,
    ingest: Ingest,
    position: Duration,
    duration: Duration,
    speed: f64,
    paused: bool,
}
impl Replay {
    /// Scan once, publishing byte progress. Checkpoints and lap indices stay in memory;
    /// raw recording data stays on disk, apart from the reader's current block.
    pub fn load(file: File, mut progress: impl FnMut(f64)) -> io::Result<Self> {
        let size = file.metadata()?.len().max(1);
        let reader = Reader::new(file)?;
        if !matches!(reader.header().packet_format, 2025 | 2026) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported telemetry format",
            ));
        }
        let mut this = Self {
            reader,
            pending: None,
            checkpoints: vec![],
            laps: BTreeMap::new(),
            model: SessionModel::default(),
            ingest: Ingest::default(),
            position: Duration::ZERO,
            duration: Duration::ZERO,
            speed: 1.0,
            paused: false,
        };
        progress(0.0);
        this.save_checkpoint(Duration::ZERO)?;
        let mut next = Duration::from_secs(30);
        let mut previous = [0u8; 24];
        let mut reported = 0.0;
        while let Some(record) = this.reader.next().transpose()? {
            let at = Duration::from_micros(record.arrival_us);
            this.pending = Some(record);
            while at > next {
                this.model.advance(next);
                this.save_checkpoint(next)?;
                next += Duration::from_secs(30);
            }
            let record = this.pending.take().unwrap();
            this.apply(&record);
            for (car, old) in previous.iter_mut().enumerate() {
                if let Some(view) = this.model.car(car)
                    && let Readout::Live(lap) | Readout::Stale(lap) = view.lap()
                    && lap.current_lap_num != *old
                {
                    *old = lap.current_lap_num;
                    this.laps.insert((car, *old), at);
                }
            }
            this.duration = at;
            // Reader progress may reach the final block before its records are indexed.
            let offset = this.reader.offset()?;
            let fraction = (offset as f64 / size as f64).min(0.99);
            if fraction - reported >= 0.01 {
                progress(fraction);
                reported = fraction;
            }
        }
        if this.duration.is_zero() && this.model.identity().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "empty recording",
            ));
        }
        this.seek(Duration::ZERO)?;
        progress(1.0);
        Ok(this)
    }
    fn save_checkpoint(&mut self, at: Duration) -> io::Result<()> {
        self.checkpoints.push(Checkpoint {
            at,
            cursor: self.reader.cursor()?,
            pending: self.pending.clone(),
            model: self.model.clone(),
            ingest: self.ingest.clone(),
        });
        Ok(())
    }
    fn apply(&mut self, record: &Record) {
        if let Ok((parsed, Disposition::Apply)) = self.ingest.ingest(
            record.arrival_us,
            &record.datagram,
            &mut |_: u64, _: &[u8]| {},
        ) {
            self.model
                .apply(Duration::from_micros(record.arrival_us), &parsed);
        }
    }
    fn advance_to(&mut self, target: Duration) -> io::Result<()> {
        loop {
            if self.pending.is_none() {
                self.pending = self.reader.next().transpose()?;
            }
            if !self
                .pending
                .as_ref()
                .is_some_and(|r| Duration::from_micros(r.arrival_us) <= target)
            {
                break;
            }
            let record = self.pending.take().unwrap();
            self.apply(&record);
        }
        self.model.advance(target);
        self.position = target;
        if target == self.duration {
            self.paused = true;
        }
        Ok(())
    }
    pub fn seek(&mut self, target: Duration) -> io::Result<()> {
        if target > self.duration {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek outside recording",
            ));
        }
        let checkpoint = self
            .checkpoints
            .iter()
            .rev()
            .find(|c| c.at <= target)
            .unwrap();
        self.reader.restore(&checkpoint.cursor)?;
        self.pending = checkpoint.pending.clone();
        self.model = checkpoint.model.clone();
        self.ingest = checkpoint.ingest.clone();
        self.advance_to(target)
    }
    pub fn tick(&mut self, elapsed: Duration) -> io::Result<()> {
        if !self.paused {
            self.advance_to(
                self.position
                    .saturating_add(elapsed.mul_f64(self.speed))
                    .min(self.duration),
            )?;
        }
        Ok(())
    }
    pub fn jump_lap(&mut self, car: usize, lap: u8) -> io::Result<()> {
        let at = *self
            .laps
            .get(&(car, lap))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "lap unavailable"))?;
        self.seek(at)
    }
    pub fn set_speed(&mut self, speed: f64) -> io::Result<()> {
        if ![0.5, 1.0, 2.0, 4.0].contains(&speed) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported replay speed",
            ));
        }
        self.speed = speed;
        Ok(())
    }
    pub fn pause(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn model(&self) -> &SessionModel {
        &self.model
    }
    pub fn position(&self) -> Duration {
        self.position
    }
    pub fn state(&self, background_live: Option<String>) -> crate::wire::ReplayState {
        crate::wire::ReplayState {
            position: self.position.as_secs_f64(),
            speed: self.speed,
            paused: self.paused,
            banner: if self.position == self.duration {
                "Replay ended — restart or exit"
            } else {
                "Replay"
            }
            .into(),
            load_progress: 1.0,
            background_live,
        }
    }
}
