//! Visual tyre compounds, shared by status, timing and stint presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compound {
    Soft,
    Medium,
    Hard,
    Intermediate,
    Wet,
    SuperSoft,
    Unknown(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Soft,
    Medium,
    Hard,
    Inter,
    Wet,
    Unknown,
}
impl Compound {
    pub fn from_visual(id: u8) -> Self {
        match id {
            16 | 20 => Self::Soft,
            17 | 21 => Self::Medium,
            18 | 22 => Self::Hard,
            7 => Self::Intermediate,
            8 => Self::Wet,
            19 => Self::SuperSoft,
            n => Self::Unknown(n),
        }
    }
    pub fn family(self) -> Family {
        match self {
            Self::Soft | Self::SuperSoft => Family::Soft,
            Self::Medium => Family::Medium,
            Self::Hard => Family::Hard,
            Self::Intermediate => Family::Inter,
            Self::Wet => Family::Wet,
            Self::Unknown(_) => Family::Unknown,
        }
    }
    pub fn name(self) -> String {
        match self {
            Self::Soft => "Soft",
            Self::Medium => "Medium",
            Self::Hard => "Hard",
            Self::Intermediate => "Intermediate",
            Self::Wet => "Wet",
            Self::SuperSoft => "Super soft",
            Self::Unknown(n) => return format!("Compound {n}"),
        }
        .into()
    }
    pub fn badge(self) -> String {
        match self {
            Self::SuperSoft => "SS".into(),
            Self::Unknown(n) => n.to_string(),
            _ => self.family().symbol().into(),
        }
    }
    pub fn from_badge(badge: &str) -> Option<Self> {
        [
            Self::Soft,
            Self::Medium,
            Self::Hard,
            Self::Intermediate,
            Self::Wet,
            Self::SuperSoft,
        ]
        .into_iter()
        .find(|c| c.badge() == badge)
    }
}
impl Family {
    /// Stints group super-soft with soft, matching the web tile's colour and symbol.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Soft => "S",
            Self::Medium => "M",
            Self::Hard => "H",
            Self::Inter => "I",
            Self::Wet => "W",
            Self::Unknown => "?",
        }
    }
}
