//! Livery choice and the fallback car against the web build, for every team ID.
//!
//! The golden is written by `node tools/livery-art/golden.mts` from
//! `dashboard/src/liveries/choose.ts` and `fallbackCar.ts`.
use std::path::PathBuf;

use pitwall_core::livery::{LiveryCar, LiveryChoice, Rgb, fallback_car_svg};
use pitwall_livery_art::LiveryImages;
use serde_json::Value;

/// The golden generator's game colours for a team.
fn game_colours(team_id: u16) -> [Rgb; 4] {
    let id = u32::from(team_id);
    std::array::from_fn(|slot| {
        let slot = slot as u32;
        Rgb::new(
            ((id * 37 + slot * 61) % 256) as u8,
            ((id * 91 + slot * 17 + 5) % 256) as u8,
            ((id * 13 + slot * 101 + 9) % 256) as u8,
        )
    })
}

fn hex(colours: &[Rgb; 4]) -> String {
    colours
        .iter()
        .map(|c| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b))
        .collect()
}

fn fnv1a64(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// A fresh pack folder holding `files` (empty files: the choice only lists names).
fn pack_folder(name: &str, files: &[String]) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("pitwall-livery-art-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for file in files {
        std::fs::write(dir.join(file), b"").unwrap();
    }
    dir
}

#[test]
fn livery_choice_and_fallback_car_match_the_web_build_for_every_team() {
    let golden: Value = serde_json::from_str(include_str!("fixtures/web-liveries.json")).unwrap();
    let mut images = Vec::new();
    for (set, files) in golden["fileSets"].as_object().unwrap() {
        let files: Vec<String> = files
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap().to_string())
            .collect();
        let folder = pack_folder(&set.replace('+', "-"), &files);
        images.push((set.clone(), LiveryImages::new(folder.clone()), folder));
    }
    let rows = golden["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1118);
    for row in rows {
        let set = row[0].as_str().unwrap();
        let team_id = row[1].as_u64().unwrap() as u16;
        let car = LiveryCar {
            team_id,
            my_team: row[2].as_u64().unwrap() != 0,
            num_colours: row[3].as_u64().unwrap() as u8,
            colours: game_colours(team_id),
        };
        let (_, livery_images, _) = images.iter().find(|(name, ..)| name == set).unwrap();
        let choice = livery_images.choice(Some(&car));
        let file = match &choice {
            LiveryChoice::Pack { file, .. } => Some(file.as_str()),
            LiveryChoice::Fallback { .. } => None,
        };
        assert_eq!(file, row[4].as_str(), "file for {row}");
        assert_eq!(
            hex(choice.colours()),
            row[5].as_str().unwrap(),
            "colours for {row}"
        );
        assert_eq!(
            fnv1a64(&fallback_car_svg(choice.colours())),
            row[6].as_str().unwrap(),
            "fallback car for {row}"
        );
    }

    let none = images[0].1.choice(None);
    assert_eq!(
        hex(none.colours()),
        golden["noParticipant"][0].as_str().unwrap()
    );
    assert_eq!(
        fnv1a64(&fallback_car_svg(none.colours())),
        golden["noParticipant"][1].as_str().unwrap()
    );
    for (_, _, folder) in images {
        let _ = std::fs::remove_dir_all(folder);
    }
}
