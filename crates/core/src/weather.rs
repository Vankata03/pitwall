//! Names of the game weather codes; callers choose their context's unknown text.
const SKIES: [&str; 6] = [
    "Clear",
    "Light cloud",
    "Overcast",
    "Light rain",
    "Heavy rain",
    "Storm",
];
pub fn sky(code: u8) -> Option<&'static str> {
    SKIES.get(usize::from(code)).copied()
}
