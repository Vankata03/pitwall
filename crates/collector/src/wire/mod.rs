//! Publication is outside the pure model. `tick` accepts the currently selected
//! live or replay instance and an explicit monotonic transport clock.
pub mod frame;
mod routes;
mod types;
use crate::model::SessionModel;
pub use routes::routes;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::sync::Notify;
pub use types::*;
#[derive(Debug, Clone)]
pub enum Message {
    Text(Value),
    Frame(Vec<u8>),
}
#[derive(Default)]
struct Queue {
    reliable: VecDeque<Message>,
    history: usize,
    frame: Option<Message>,
    state: Option<Message>,
    closed: bool,
}
#[derive(Default)]
struct ScreenInner {
    queue: Mutex<Queue>,
    notify: Notify,
}
#[derive(Clone)]
pub struct Screen(Arc<ScreenInner>);
impl Screen {
    pub fn pop(&self) -> Option<Message> {
        let mut q = self.0.queue.lock().unwrap();
        let msg = q.reliable.pop_front();
        if let Some(Message::Text(v)) = &msg {
            q.history = q
                .history
                .saturating_sub(v["entries"].as_array().map_or(0, Vec::len));
        }
        msg.or_else(|| q.state.take()).or_else(|| q.frame.take())
    }
    pub fn closed(&self) -> bool {
        self.0.queue.lock().unwrap().closed
    }
    pub async fn ready(&self) {
        self.0.notify.notified().await;
    }
    fn push(&self, message: Message) {
        let mut q = self.0.queue.lock().unwrap();
        if q.closed {
            return;
        }
        match &message {
            Message::Frame(_) => q.frame = Some(message),
            Message::Text(v) if v["type"] == "state" => q.state = Some(message),
            Message::Text(v) => {
                q.history += v["entries"].as_array().map_or(0, Vec::len);
                if v["type"] == "snapshot" {
                    q.frame = None;
                    q.state = None;
                }
                if q.history > 4096 || q.reliable.len() >= 4096 {
                    q.closed = true;
                    q.reliable.clear();
                    q.frame = None;
                    q.state = None;
                } else {
                    q.reliable.push_back(message);
                }
            }
        }
        drop(q);
        self.0.notify.notify_one();
    }
}
struct Inner {
    build: String,
    epoch: u32,
    model_epoch: u64,
    source: Source,
    seq: u32,
    state: State,
    /// Last published entry and its serialized form, keyed by entry ID.
    history: BTreeMap<String, (HistoryEntry, Value)>,
    /// Entry IDs per history group (24 cars, then race control), in snapshot order.
    history_groups: Vec<Vec<String>>,
    model: SessionModel,
    screens: Vec<Weak<ScreenInner>>,
    last_frame: Option<Vec<u8>>,
    frame_at: Option<Duration>,
    state_at: Option<Duration>,
}
#[derive(Clone)]
pub struct Stream(Arc<Mutex<Inner>>);
impl Stream {
    pub fn new(build: impl Into<String>) -> Self {
        let model = SessionModel::default();
        Self(Arc::new(Mutex::new(Inner {
            build: build.into(),
            epoch: 0,
            model_epoch: 0,
            source: Source::Live,
            seq: 0,
            state: State::capture(&model, Context::default()),
            model,
            history: BTreeMap::new(),
            history_groups: vec![Vec::new(); HISTORY_GROUPS],
            screens: vec![],
            last_frame: None,
            frame_at: None,
            state_at: None,
        })))
    }
    pub fn subscribe(&self) -> Screen {
        let mut i = self.0.lock().unwrap();
        let screen = Screen(Arc::default());
        screen.push(Message::Text(
            json!({"type":"hello","buildId":i.build,"epoch":i.epoch,"source":i.source}),
        ));
        screen.push(i.snapshot());
        if i.last_frame.is_some() {
            screen.push(Message::Frame(frame::encode(&i.model, i.epoch, i.seq)));
        }
        i.screens.push(Arc::downgrade(&screen.0));
        screen
    }
    /// Force a snapshot on the next tick, e.g. after replay seeks to a checkpoint
    /// whose model epoch happens to equal the previous one.
    pub fn reset(&self) {
        let mut i = self.0.lock().unwrap();
        i.model_epoch = u64::MAX;
    }
    pub fn tick(&self, now: Duration, source: Source, model: &SessionModel, context: Context) {
        let mut i = self.0.lock().unwrap();
        let reset = i.model_epoch != model.epoch() || i.source != source;
        // History generations change on summary corrections and per-car stint updates,
        // not on every live sample. Only groups whose shared history changed are
        // re-serialized, so tick cost stays flat as a long session accumulates laps.
        let mut changes = Vec::new();
        let mut removed = Vec::new();
        for group in 0..HISTORY_GROUPS {
            let shared = if group == HISTORY_GROUPS - 1 {
                i.model.shares_race_control(model)
            } else {
                i.model.shares_car_history(model, group)
            };
            if !reset && shared {
                continue;
            }
            let entries = history_group(model, group);
            let order: Vec<_> = entries.iter().map(|e| e.id.clone()).collect();
            let mut current: BTreeMap<_, _> =
                entries.into_iter().map(|e| (e.id.clone(), e)).collect();
            let previous = std::mem::take(&mut i.history_groups[group]);
            for id in previous {
                if !current.contains_key(&id) {
                    i.history.remove(&id);
                    removed.push(id);
                }
            }
            for id in &order {
                // A repeated ID is published once.
                let Some(entry) = current.remove(id) else {
                    continue;
                };
                // An equal entry serializes identically, so skip the encode. Unequal
                // entries may differ only in unserialized bookkeeping; compare JSON.
                if i.history.get(id).is_some_and(|(last, _)| *last == entry) {
                    continue;
                }
                let value = serde_json::to_value(&entry).unwrap();
                if i.history.get(id).is_none_or(|(_, last)| *last != value) {
                    changes.push(value.clone());
                }
                i.history.insert(id.clone(), (entry, value));
            }
            i.history_groups[group] = order;
        }
        i.model = model.clone();
        let state_due = i
            .state_at
            .is_none_or(|at| now.saturating_sub(at) >= Duration::from_millis(200));
        if reset || state_due {
            i.state = State::capture(model, context);
        }
        if reset {
            i.epoch = i.epoch.wrapping_add(1);
            i.model_epoch = model.epoch();
            i.source = source;
            i.last_frame = None;
            let msg = i.snapshot();
            i.send(msg);
        } else if !changes.is_empty() || !removed.is_empty() {
            let epoch = i.epoch;
            i.send(Message::Text(
                json!({"type":"history","epoch":epoch,"entries":changes,"removed":removed}),
            ));
        }
        if i.frame_at
            .is_none_or(|at| now.saturating_sub(at) >= Duration::from_secs(1) / 60)
        {
            i.frame_at = Some(slot(now, Duration::from_secs(1) / 60));
            let mut bytes = frame::encode(model, i.epoch, 0);
            if i.last_frame.as_ref() != Some(&bytes) {
                i.last_frame = Some(bytes.clone());
                i.seq = i.seq.wrapping_add(1);
                bytes[4..8].copy_from_slice(&i.seq.to_le_bytes());
                i.send(Message::Frame(bytes));
            }
        }
        if i.state_at
            .is_none_or(|at| now.saturating_sub(at) >= Duration::from_millis(200))
        {
            i.state_at = Some(slot(now, Duration::from_millis(200)));
            let mut v = serde_json::to_value(&i.state).unwrap();
            v["type"] = json!("state");
            v["epoch"] = json!(i.epoch);
            i.send(Message::Text(v));
        }
    }
    pub fn epoch(&self) -> u32 {
        self.0.lock().unwrap().epoch
    }
    pub fn trace(&self, index: usize, lap: u8, epoch: u32) -> Result<Trace, u16> {
        let i = self.0.lock().unwrap();
        if epoch != i.epoch {
            return Err(409);
        }
        let c = i.model.car(index).ok_or(404u16)?;
        let trace = [
            c.current_lap_trace(),
            c.lap_trace(crate::model::Reference::Previous),
            c.lap_trace(crate::model::Reference::Best),
        ]
        .into_iter()
        .flatten()
        .find(|t| t.lap_number == lap)
        .ok_or(404u16)?;
        Ok(Trace {
            epoch,
            lap,
            distance: trace.samples.iter().map(|s| s.distance_m).collect(),
            time: trace.samples.iter().map(|s| s.time_ms).collect(),
            speed: trace.samples.iter().map(|s| s.speed_kph).collect(),
            throttle: trace.samples.iter().map(|s| s.throttle).collect(),
            brake: trace.samples.iter().map(|s| s.brake).collect(),
        })
    }
}
impl Inner {
    fn snapshot(&self) -> Message {
        Message::Text(
            json!({"type":"snapshot","epoch":self.epoch,"source":self.source,"state":self.state,"history":self.history_groups.iter().flatten().map(|id| &self.history[id].1).collect::<Vec<_>>()}),
        )
    }
    fn send(&mut self, msg: Message) {
        self.screens.retain(|s| {
            if let Some(s) = s.upgrade() {
                Screen(s).push(msg.clone());
                true
            } else {
                false
            }
        });
    }
}

/// Regenerate the dashboard contracts. CI's contract test compares these files.
pub fn generate_contract(dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    use ts_rs::TS;
    std::fs::create_dir_all(dir)?;
    let config = ts_rs::Config::default()
        .with_large_int("number")
        .with_out_dir(dir);
    TextMessage::export_all(&config)?;
    Trace::export_all(&config)?;
    std::fs::write(dir.join("frame.ts"), frame::decoder())?;
    Ok(())
}

fn slot(now: Duration, period: Duration) -> Duration {
    now - Duration::from_nanos((now.as_nanos() % period.as_nanos()) as u64)
}
