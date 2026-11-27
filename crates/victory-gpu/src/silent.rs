//! (Scarlett D1, docs/silent-defects-audit.md) Switches and counters for silent
//! rendering defects: placed texture UAVs (B1), depth-plane twin sync before
//! draw tests (C4), sRGB view preservation on plain textures (A7), and
//! unaligned CBV own-copy fallback (B4).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub(crate) mod k {
    pub const DEPTH_SYNCED: usize = 0;
    pub const PLACED_UAV_MADE: usize = 1;
    pub const PLACED_UAV_NULL: usize = 2;
    pub const PLACED_UAV_ZERO: usize = 3;
    pub const SRGB_KEPT: usize = 4;
    pub const CBV_OWN: usize = 5;
    pub const COUNT: usize = 6;
}

static C: [AtomicU64; k::COUNT] = [const { AtomicU64::new(0) }; k::COUNT];

pub(crate) fn add(i: usize, n: u64) {
    C[i].fetch_add(n, Relaxed);
}

fn switch(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(v) => v.trim() != "0",
        Err(_) => default,
    }
}

/// `VICTORY_PLACED_UAV` (default 1; `0` leaves placed texture UAVs null):
/// mirrors unordered access views of textures the title placed in memory
/// onto the same host texture their shader views read (`texture_at`).
pub(crate) fn placed_uav() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| switch("VICTORY_PLACED_UAV", true))
}

/// `VICTORY_DEPTH_TWIN_SYNC` (default 1; `0` delays until list close):
/// draws a depth plane's UAV twin into the depth plane before any draw in the
/// same list tests against it, so tiled lighting sees fresh material classes.
pub(crate) fn depth_twin_sync() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| switch("VICTORY_DEPTH_TWIN_SYNC", true))
}

/// `VICTORY_SRGB_VIEWS` (default 1; `0` keeps typed UNORM copies):
/// plain textures in formats with an sRGB twin are allocated typeless so sRGB
/// shader views preserve hardware sRGB decode instead of reading linear.
pub(crate) fn srgb_views() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| switch("VICTORY_SRGB_VIEWS", true))
}

/// `VICTORY_CBV_OWN_COPY` (default 0; `1` gives unaligned CBVs in the 2M-slot
/// background heap a `shadow::exact` copy): off by default because live table
/// CBVs are already copied into 256-byte-aligned scratch by `draw.rs`, while
/// copying every background-heap CBV created ~48k `shadow::exact` buffers
/// (~2 GiB RAM, +100 ms/frame in `heap buffers` on Wine).
pub(crate) fn cbv_own_copy() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("VICTORY_CBV_OWN_COPY").is_ok_and(|v| v.trim() == "1"))
}

/// UNORM formats with an sRGB twin: R8G8B8A8, BC1, BC2, BC3, B8G8R8A8,
/// B8G8R8X8, BC7.
pub(crate) const SRGB_PAIRS: [(u32, u32); 7] = [
    (28, 29),
    (71, 72),
    (74, 75),
    (77, 78),
    (87, 91),
    (88, 93),
    (98, 99),
];

/// Typeless family format for UNORM/sRGB formats in [`SRGB_PAIRS`].
pub(crate) fn srgb_typeless(format: u32) -> Option<u32> {
    SRGB_PAIRS
        .iter()
        .any(|&(l, s)| l == format || s == format)
        .then(|| crate::resource::family(format))
        .filter(|&t| t != format)
}

/// Periodic diagnostic summary of silent-defect fixes active this period.
pub(crate) fn log(frames: f64) {
    let c: [u64; k::COUNT] = std::array::from_fn(|i| C[i].swap(0, Relaxed));
    if c.iter().all(|&v| v == 0) {
        return;
    }
    let f = |i: usize| c[i] as f64 / frames.max(1.0);
    tracing::info!(
        "silent-defect fixes per frame: depth-twin synced {:.2}, placed UAVs mirrored {:.2} (null {:.2}, addr0 {:.2}), sRGB views kept {:.2}, unaligned CBVs copied {:.2}",
        f(k::DEPTH_SYNCED),
        f(k::PLACED_UAV_MADE),
        f(k::PLACED_UAV_NULL),
        f(k::PLACED_UAV_ZERO),
        f(k::SRGB_KEPT),
        f(k::CBV_OWN),
    );
}
