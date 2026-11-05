//! Livery art for the native app, independent of any UI framework (spec section 13).
//!
//! [`LiveryImages`] turns a car into the RGBA picture to draw: its image from the livery
//! pack folder, else the fallback car from [`pitwall_core::livery`] in the car's colours.
//! [`pitwall_core::livery::team_colour`] is the colour of the timing tower's team marker and the track map's
//! dots. Which picture a car gets is [`pitwall_core::livery::choose_livery`], and the
//! fallback car's SVG is byte-identical to the web build's, so both match the web build.
//!
//! The fallback car is rasterised with resvg: pure Rust, no system libraries, and the
//! renderer selected for the shipped SVG paths. Pack PNGs are
//! decoded with the `png` crate that resvg's tiny-skia already depends on.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pitwall_core::liveries::{pack_files, pack_image};
use pitwall_core::livery::{
    LiveryCar, LiveryChoice, NEUTRAL_COLOURS, Rgb, choose_livery, fallback_car_svg,
};
use resvg::{tiny_skia, usvg};

/// An 8-bit RGBA picture, rows top to bottom, straight (not premultiplied) alpha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes: R, G, B, A per pixel.
    pub pixels: Vec<u8>,
}

/// Why an SVG or PNG could not become an [`RgbaImage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageError(pub String);

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ImageError {}

/// Rasterise an SVG document at its own size.
pub fn rasterise_svg(svg: &str) -> Result<RgbaImage, ImageError> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|error| ImageError(format!("SVG: {error}")))?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| ImageError("SVG has no area".into()))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Ok(RgbaImage {
        width: size.width(),
        height: size.height(),
        pixels,
    })
}

/// The fallback car painted with `colours`, 960×720.
pub fn fallback_car_image(colours: &[Rgb; 4]) -> RgbaImage {
    rasterise_svg(&fallback_car_svg(colours)).expect("the fallback car is valid SVG")
}

/// Decode a PNG of any colour type and bit depth to 8-bit RGBA.
pub fn decode_png(bytes: &[u8]) -> Result<RgbaImage, ImageError> {
    let fail = |error: png::DecodingError| ImageError(format!("PNG: {error}"));
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(fail)?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).map_err(fail)?;
    let samples = &buffer[..info.buffer_size()];
    let pixels = match info.color_type {
        png::ColorType::Rgba => samples.to_vec(),
        png::ColorType::Rgb => samples
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => samples
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => samples.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return Err(ImageError("PNG: palette was not expanded".into())),
    };
    Ok(RgbaImage {
        width: info.width,
        height: info.height,
        pixels,
    })
}

/// Fallback cars kept before the cache starts over; a grid has at most 22 colour sets.
const MAX_FALLBACKS: usize = 64;

/// The picture each car is drawn with: its pack image from the livery folder, else the
/// fallback car in its colours. Pictures are decoded or rasterised once and shared.
///
/// One instance can serve every window and thread. The pack listing is read on
/// [`new`](Self::new) and [`refresh`](Self::refresh), as the web build lists it when a tile
/// mounts; nothing watches the folder.
pub struct LiveryImages {
    folder: PathBuf,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    files: Vec<String>,
    /// Decoded pack images by file name; `None` when the file failed to load.
    packs: HashMap<String, Option<Arc<RgbaImage>>>,
    fallbacks: HashMap<[u8; 12], Arc<RgbaImage>>,
}

fn colour_key(colours: &[Rgb; 4]) -> [u8; 12] {
    let mut key = [0; 12];
    for (slot, c) in colours.iter().enumerate() {
        key[slot * 3..slot * 3 + 3].copy_from_slice(&[c.r, c.g, c.b]);
    }
    key
}

impl LiveryImages {
    /// Images for the pack in `folder`, normally `AppPaths::liveries`. A missing folder is
    /// an empty pack.
    pub fn new(folder: PathBuf) -> Self {
        let files = pack_files(&folder);
        Self {
            folder,
            state: Mutex::new(State {
                files,
                ..State::default()
            }),
        }
    }

    /// Re-list the pack folder and forget decoded pack images, so added, replaced and
    /// removed files take effect. Rasterised fallback cars are kept.
    pub fn refresh(&self) {
        let files = pack_files(&self.folder);
        let mut state = self.lock();
        state.files = files;
        state.packs.clear();
    }

    /// The pack image names from the last listing, sorted.
    pub fn files(&self) -> Vec<String> {
        self.lock().files.clone()
    }

    /// Which picture `car` gets ([`choose_livery`] over the current listing).
    pub fn choice(&self, car: Option<&LiveryCar>) -> LiveryChoice {
        choose_livery(car, &self.lock().files)
    }

    /// The picture for `car` (`None`: no participant data yet). A pack image that cannot
    /// be read or decoded is drawn as the fallback car in the same colours.
    pub fn image_for(&self, car: Option<&LiveryCar>) -> Arc<RgbaImage> {
        let mut state = self.lock();
        let choice = choose_livery(car, &state.files);
        if let LiveryChoice::Pack { file, .. } = &choice {
            let folder = &self.folder;
            let pack = state
                .packs
                .entry(file.clone())
                .or_insert_with(|| {
                    let path = pack_image(folder, file)?;
                    let bytes = std::fs::read(path).ok()?;
                    decode_png(&bytes).ok().map(Arc::new)
                })
                .clone();
            if let Some(image) = pack {
                return image;
            }
        }
        let colours = choice.colours();
        let key = colour_key(colours);
        if !state.fallbacks.contains_key(&key) && state.fallbacks.len() >= MAX_FALLBACKS {
            state.fallbacks.clear();
        }
        state
            .fallbacks
            .entry(key)
            .or_insert_with(|| Arc::new(fallback_car_image(colours)))
            .clone()
    }

    /// The pack image for `team_id` (not My Team), else the fallback car in neutral
    /// colours. Prefer [`image_for`](Self::image_for) when the car's Participants entry is
    /// known: it adds My Team and the game's colours.
    pub fn for_team(&self, team_id: u16) -> Arc<RgbaImage> {
        self.image_for(Some(&LiveryCar {
            team_id,
            my_team: false,
            num_colours: 0,
            colours: NEUTRAL_COLOURS,
        }))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // The state is a cache; one left by a panicking thread is still usable.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
