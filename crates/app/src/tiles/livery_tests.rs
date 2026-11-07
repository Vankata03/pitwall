//! Car image mount behavior and opt-in evidence from read-only local art and recordings.
use super::car_status::{Inputs, Memory, view};
use pitwall_core::{
    livery::{LiveryCar, LiveryChoice},
    model::SessionModel,
    paths::AppPaths,
    pitrec::Reader,
    telemetry::{Packet, parse_datagram},
    units::Units,
};
use pitwall_livery_art::{LiveryImages, decode_png};
use std::{collections::BTreeSet, fs, path::PathBuf, time::Duration};

#[test]
fn returning_from_compact_remounts_the_car_image_and_reads_added_and_replaced_art() {
    use image::ImageEncoder;
    let folder = std::env::temp_dir().join(format!("pitwall-car-remount-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let car = LiveryCar {
        team_id: 3,
        my_team: false,
        num_colours: 0,
        colours: pitwall_core::livery::NEUTRAL_COLOURS,
    };
    let mut memory = Memory::new(folder.clone());
    let initial = memory.picture(Some(&car));
    let mut inputs = Inputs::from_model(
        &SessionModel::default(),
        None,
        false,
        Units::default(),
        (400.0, 300.0),
    );
    inputs.livery = Some(car);
    for colour in [[10, 20, 30, 255], [40, 50, 60, 128]] {
        // Compact unmounts CarImage in the web tile. Re-expansion must re-list and decode.
        inputs.width = 400.0;
        inputs.height = 300.0;
        drop(super::car_status::render(view(&inputs), &mut memory));
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&colour, 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        fs::write(folder.join("3.png"), bytes).unwrap();
        inputs.width = 700.0;
        inputs.height = 460.0;
        drop(super::car_status::render(view(&inputs), &mut memory));
        let next = memory.picture(Some(&car));
        assert_eq!(
            next.as_bytes(0).unwrap().len(),
            4,
            "remount must load the added pack PNG"
        );
        assert_eq!(
            next.as_bytes(0).unwrap(),
            &[colour[2], colour[1], colour[0], colour[3]]
        );
        assert!(!std::sync::Arc::ptr_eq(&initial, &next));
        assert!(
            std::sync::Arc::ptr_eq(&next, &memory.picture(Some(&car))),
            "ordinary frames keep the upload"
        );
    }
    fs::remove_dir_all(folder).unwrap();
}

#[test]
#[ignore = "read-only probe of a real local pack and recording participant packets"]
fn actual_pack_and_recorded_teams_reach_native_bgra_frames() {
    let root = std::env::var_os("PITWALL_REAL_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| AppPaths::resolve().unwrap().root);
    let folder = root.join("liveries");
    let images = LiveryImages::new(folder.clone());
    assert!(!images.files().is_empty(), "probe requires a pack");
    let mut memory = Memory::new(folder.clone());
    for name in images.files() {
        let decoded = decode_png(&fs::read(folder.join(&name)).unwrap()).unwrap();
        assert!(decoded.pixels.chunks_exact(4).any(|p| p[3] > 0));
        println!("pack {name}: {}x{}", decoded.width, decoded.height);
    }
    let mut paths: Vec<_> = fs::read_dir(root.join("recordings"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "pitrec"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "probe requires recordings");
    let mut teams = BTreeSet::new();
    for path in paths {
        let reader = Reader::new(fs::File::open(&path).unwrap()).unwrap();
        let format = reader.header().packet_format;
        let mut model = SessionModel::default();
        let mut found = false;
        let mut player_checked = false;
        for record in reader {
            let record = record.unwrap();
            let Ok(parsed) = parse_datagram(&record.datagram) else {
                continue;
            };
            model.apply(Duration::from_micros(record.arrival_us), &parsed);
            if found {
                let inputs = Inputs::from_model(
                    &model,
                    model.player_car_index(),
                    false,
                    Units::default(),
                    (800.0, 600.0),
                );
                if let Some(car) = inputs.livery {
                    assert!(matches!(
                        images.choice(Some(&car)),
                        LiveryChoice::Pack { .. }
                    ));
                    assert!(view(&inputs).show_car);
                    println!(
                        "model player team={} at-us={}",
                        car.team_id, record.arrival_us
                    );
                    player_checked = true;
                    break;
                }
                continue;
            }
            let Packet::Participants(packet) = &parsed.packet else {
                continue;
            };
            let mut grid = BTreeSet::new();
            for index in 0..usize::from(packet.num_active_cars) {
                let Some(participant) = packet.participants[index].as_ref() else {
                    continue;
                };
                let car = LiveryCar::from_participant(participant);
                let inputs = Inputs::from_model(
                    &model,
                    Some(index),
                    false,
                    Units::default(),
                    (800.0, 600.0),
                );
                if model.car(index).is_some() {
                    assert_eq!(inputs.livery, Some(car));
                }
                assert!(view(&inputs).show_car);
                let LiveryChoice::Pack { file, .. } = images.choice(Some(&car)) else {
                    panic!("no pack match for team {}", car.team_id);
                };
                let decoded = decode_png(&fs::read(folder.join(&file)).unwrap()).unwrap();
                let frame = memory.picture(Some(&car));
                let bytes = frame.as_bytes(0).unwrap();
                assert_eq!(bytes.len(), decoded.pixels.len());
                for (bgra, rgba) in bytes.chunks_exact(4).zip(decoded.pixels.chunks_exact(4)) {
                    assert_eq!(bgra, &[rgba[2], rgba[1], rgba[0], rgba[3]]);
                }
                grid.insert((car.team_id, car.my_team, file));
                teams.insert(car.team_id);
            }
            println!(
                "{} format={format} first-participants-us={} player={:?} grid={grid:?}",
                path.file_name().unwrap().to_string_lossy(),
                record.arrival_us,
                model.player_car_index()
            );
            found = true;
        }
        if !found {
            println!(
                "{}: no participant packet; livery choice not verifiable",
                path.file_name().unwrap().to_string_lossy()
            );
        } else {
            assert!(
                player_checked,
                "no model-to-tile player livery in {}",
                path.display()
            );
        }
    }
    println!("recorded teams: {teams:?}");
}
