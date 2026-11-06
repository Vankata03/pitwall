//! The Gaps tile as data (spec GAPS-01), ported from `dashboard/src/tiles/types/gaps.tsx`.

use crate::field::FieldCar;
use crate::model::Readout;

#[derive(Debug, Clone, PartialEq)]
pub struct GapsView {
    /// Interval to the car ahead, `+1.250 s`.
    pub ahead: Readout<String>,
    /// Interval of the car one position behind, `−2.750 s`; stale when either car's lap
    /// data is stale.
    pub behind: Readout<String>,
    /// `P<n> · <label>` of the car ahead and behind, when listed.
    pub ahead_note: Option<String>,
    pub behind_note: Option<String>,
}

/// `car`'s interval to the car in front: positive for the car ahead of us, negative for
/// the car behind. No interval reads absent.
fn interval(car: &FieldCar, behind: bool) -> Readout<String> {
    let sign = if behind { '\u{2212}' } else { '+' };
    let text = |ms: u32| format!("{sign}{:.3} s", f64::from(ms) / 1000.0);
    match car.gaps {
        Readout::Live(g) => g
            .to_car_in_front_ms
            .map_or(Readout::Absent, |ms| Readout::Live(text(ms))),
        Readout::Stale(g) => g
            .to_car_in_front_ms
            .map_or(Readout::Absent, |ms| Readout::Stale(text(ms))),
        Readout::Restricted => Readout::Restricted,
        Readout::Absent => Readout::Absent,
    }
}

/// The Gaps tile for the bound car `index` of `field`.
pub fn gaps_view(field: &[FieldCar], index: usize) -> GapsView {
    let Some(mine) = field.iter().find(|car| car.index == index) else {
        return GapsView {
            ahead: Readout::Absent,
            behind: Readout::Absent,
            ahead_note: None,
            behind_note: None,
        };
    };
    let position = mine.position();
    // P1 has no car ahead; the game still sends a zero interval for it in races.
    let ahead = if position == Some(1) {
        Readout::Absent
    } else {
        interval(mine, false)
    };
    let mut behind = Readout::Absent;
    if let Some(position) = position {
        let next = field.iter().find(|other| {
            position
                .checked_add(1)
                .is_some_and(|wanted| other.position() == Some(wanted))
        });
        if let Some(next) = next {
            behind = interval(next, true);
            let tainted =
                matches!(mine.lap, Readout::Stale(_)) || matches!(next.lap, Readout::Stale(_));
            if let (true, Readout::Live(text)) = (tainted, &behind) {
                behind = Readout::Stale(text.clone());
            }
        }
    }
    let position = position.unwrap_or(0);
    let who = |at: u8| -> Option<String> {
        if position == 0 {
            return None;
        }
        field
            .iter()
            .find(|car| car.label.is_some() && car.position() == Some(at))
            .map(|car| format!("P{at} \u{b7} {}", car.label.as_deref().unwrap_or_default()))
    };
    GapsView {
        ahead,
        behind,
        ahead_note: position.checked_sub(1).and_then(who),
        behind_note: position.checked_add(1).and_then(who),
    }
}
