//! Port of dashboard/src/tiles/traces/lap-traces.test.ts. The web cache fetched traces over
//! HTTP; natively a request reads the trace from a source closure (normally the model
//! snapshot) synchronously, so the in-flight and late-reply cases have no counterpart.
use std::cell::Cell;

use pitwall_core::lap_traces::{LapTraceCache, SourcedTrace};

/// A stand-in trace: its 5 m bucket distances. The cache is generic over the trace type;
/// the app stores `Arc<pitwall_core::model::LapTrace>`.
type Trace = Vec<f32>;

fn trace(distances: &[f32]) -> Trace {
    distances.to_vec()
}

/// A source reply: the trace of `lap` as the model at `epoch` has it.
fn ok(epoch: u64, lap: u8, trace: Trace) -> Option<SourcedTrace<Trace>> {
    Some(SourcedTrace { epoch, lap, trace })
}

#[test]
fn requests_a_lap_for_the_current_epoch_and_then_serves_it() {
    let mut cache = LapTraceCache::<Trace>::default();
    let version = cache.version();
    let asked = Cell::new(None);
    cache.request(Some(7), 4, 3, |car, lap| {
        asked.set(Some((car, lap)));
        ok(7, 3, trace(&[0.0, 5.0, 10.0]))
    });
    assert_eq!(asked.get(), Some((4, 3)));
    assert_eq!(cache.get(Some(7), 4, 3).unwrap().len(), 3);
    assert!(cache.version() > version);
}

#[test]
fn a_lap_already_held_is_not_sourced_again() {
    let mut cache = LapTraceCache::<Trace>::default();
    let calls = Cell::new(0);
    let source = |_, _| {
        calls.set(calls.get() + 1);
        ok(1, 3, trace(&[0.0]))
    };
    cache.request(Some(1), 0, 3, source);
    cache.request(Some(1), 0, 3, source);
    assert_eq!(calls.get(), 1);
    assert!(cache.get(Some(1), 0, 3).is_some());
}

#[test]
fn a_trace_from_another_epoch_than_requested_is_dropped() {
    let mut cache = LapTraceCache::<Trace>::default();
    cache.request(Some(5), 0, 3, |_, _| ok(4, 3, trace(&[0.0])));
    assert!(cache.get(Some(5), 0, 3).is_none());
}

#[test]
fn a_trace_of_another_lap_than_requested_is_dropped() {
    let mut cache = LapTraceCache::<Trace>::default();
    cache.request(Some(1), 0, 3, |_, _| ok(1, 4, trace(&[0.0])));
    assert!(cache.get(Some(1), 0, 3).is_none());
}

#[test]
fn held_laps_are_forgotten_when_the_epoch_changes() {
    let mut cache = LapTraceCache::<Trace>::default();
    cache.request(Some(1), 0, 3, |_, _| ok(1, 3, trace(&[0.0])));
    assert!(cache.get(Some(1), 0, 3).is_some());
    let version = cache.version();
    assert!(cache.get(Some(2), 0, 3).is_none());
    assert!(
        cache.version() > version,
        "forgetting held laps is a change"
    );
    let asked = Cell::new(false);
    cache.request(Some(2), 0, 3, |_, _| {
        asked.set(true);
        ok(2, 3, trace(&[0.0]))
    });
    assert!(asked.get());
    assert!(cache.get(Some(2), 0, 3).is_some());
}

#[test]
fn nothing_is_requested_before_the_first_snapshot() {
    let mut cache = LapTraceCache::<Trace>::default();
    let asked = Cell::new(false);
    cache.request(None, 0, 3, |_, _| {
        asked.set(true);
        None
    });
    assert!(!asked.get());
}

#[test]
fn an_unavailable_lap_is_not_retried_until_invalidated() {
    let mut cache = LapTraceCache::<Trace>::default();
    let calls = Cell::new(0);
    cache.request(Some(1), 0, 3, |_, _| {
        calls.set(calls.get() + 1);
        None
    });
    cache.request(Some(1), 0, 3, |_, _| {
        calls.set(calls.get() + 1);
        None
    });
    assert_eq!(calls.get(), 1);
    assert!(cache.get(Some(1), 0, 3).is_none());

    cache.invalidate_unavailable(Some(1)); // a lap completed: new data may exist now
    cache.request(Some(1), 0, 3, |_, _| {
        calls.set(calls.get() + 1);
        ok(1, 3, trace(&[0.0]))
    });
    assert_eq!(calls.get(), 2);
    assert!(cache.get(Some(1), 0, 3).is_some());
}

#[test]
fn an_unavailable_lap_is_retried_in_a_new_epoch() {
    let mut cache = LapTraceCache::<Trace>::default();
    cache.request(Some(1), 0, 3, |_, _| None);
    cache.request(Some(2), 0, 3, |_, _| ok(2, 3, trace(&[0.0])));
    assert!(cache.get(Some(2), 0, 3).is_some());
}

#[test]
fn lap_completion_replaces_a_cached_partial_current_lap_with_the_complete_trace() {
    let mut cache = LapTraceCache::<Trace>::default();
    cache.request(Some(1), 0, 3, |_, _| ok(1, 3, trace(&[0.0])));
    assert_eq!(cache.get(Some(1), 0, 3).unwrap().len(), 1);
    let version = cache.version();
    cache.invalidate(Some(1), 0, 3);
    assert!(cache.version() > version);
    assert!(cache.get(Some(1), 0, 3).is_none());
    cache.request(Some(1), 0, 3, |_, _| ok(1, 3, trace(&[0.0, 5.0, 10.0])));
    assert_eq!(cache.get(Some(1), 0, 3).unwrap().len(), 3);
}
