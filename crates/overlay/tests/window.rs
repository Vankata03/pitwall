#![cfg(windows)]

use pitwall_core::feed;
use pitwall_overlay::{settings::OverlaySettings, spawn};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

fn handles() -> u32 {
    let mut count = 0;
    // SAFETY: the current process and a live output pointer.
    unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count).unwrap() };
    count
}

#[test]
#[ignore = "requires an interactive Windows desktop; run with PITWALL_OVERLAY_RENDER=software"]
fn repeatedly_opening_and_closing_the_renderer_releases_its_handles() {
    assert_eq!(
        std::env::var("PITWALL_OVERLAY_RENDER").as_deref(),
        Ok("software")
    );
    let (_, reader) = feed::channel();
    // Warm up process-wide Direct2D/DirectWrite resources before counting.
    spawn(reader.clone(), OverlaySettings::default(), None)
        .unwrap()
        .close();
    let before = handles();
    for _ in 0..4 {
        let overlay = spawn(reader.clone(), OverlaySettings::default(), None).unwrap();
        assert!(
            overlay.metrics().frames >= 1,
            "spawn must present before returning"
        );
        assert_eq!(overlay.metrics().failures, 0);
        overlay.set_moving(true);
        overlay.set_moving(false);
        let saved = overlay.close();
        assert!(saved.visible);
        assert!(saved.position.is_some());
        assert!(saved.size.is_some());
    }
    assert!(
        handles() <= before,
        "renderer leaked handles: {before} -> {}",
        handles()
    );
}
