# victory-lowram

Low-RAM (16 GB host RAM) & Linux/Wine (`vkd3d-proton` + RADV) fork of [`avatarmanz/VIctory-7900XTX`](https://github.com/avatarmanz/VIctory-7900XTX).

Targeted and tested on:
- **RAM:** 16 GB system RAM (+ swap/zram)
- **GPU:** AMD Radeon RX 9060 XT 16 GB (RDNA4, ReBAR enabled)
- **OS:** Arch Linux + Wine 11.22 + `vkd3d-proton` 3.2.0 + Mesa RADV

## What `victory-lowram` adds on top of upstream

1. **Wine / `vkd3d-proton` compatibility & zero-copy Renderer Ports (1 + 2 + 6 + 7)**
   - **Renderer Port 1 (`VICTORY_PORT_CONSTANTS=1`) on Wine:** `xmem.rs` maps the 185 MB 32-bit graphics reservation as `MEM_PRIVATE | MEM_WRITE_WATCH` under Wine (`on_wine() && VICTORY_PORT_CONSTANTS=1`) and `port.rs` opens `OpenExistingHeapFromAddress1` directly on it — unlocking full zero-copy CPU+GPU reads, copies, and shader UAV writes via `vkd3d-proton`'s `VK_EXT_external_memory_host` on RADV (`reads true, writes true, 0 refused`).
   - **Renderer Port 2 (`VICTORY_PORT_DESCRIPTORS=1`) 6/6 store patching:** `apps/victory/src/run.rs` applies `descriptor_sites` before `write_sites` so auto-discovered `write-sites.txt` entries (such as `0x4d9f117`) can never overwrite loop 2; captures **99.93%** of descriptor writes (`327 → 4` heap pages, `670k → 41k` slots, `32.15 → 0.00–1.80 ms` asking).
   - `inplace.rs`: Wine FXC-compatible `RWStructuredBuffer<uint>` + code-built root signature in the in-place probe.
   - `descriptor.rs`: allows `sync_heaps` to process Port 2 adopted heaps even if `heap.written` is absent.
   - `ambient.rs`: code-built D3D12 root signature and `RWStructuredBuffer<uint>` stats instead of `RWByteAddressBuffer::InterlockedAdd` (which failed Wine's FXC with `0x80004005`).
   - `compose.rs`: code-built D3D12 root signature (no `[RootSignature(RS)]` HLSL attribute) so Xbox Series S two-plane compose works cleanly under Wine.
   - Coalesced alias `VirtualProtect` calls and fixed `look_at_holes()` vs `xmem::map()` race (`title_memory_reading_now()`).
   - Pipeline compile timeout & known-deadlock skip list (`VICTORY_SKIP_PIPELINES`) for `vkd3d-proton`.
2. **16 GB VRAM / Low-RAM stability**
   - `bar_room_percent()` defaults to `room_percent() - 4` (`VICTORY_BAR_ROOM_PERCENT`), keeping ReBAR buffer/window pools strictly below the texture VRAM budget so world textures never starve (`the card has no room`).
   - `descriptor/host.rs`: `keep_shown` is gated by `crate::resource::room_lately()`, releasing old replaced textures immediately when VRAM is tight (ported from `Belobug/VIctory-Belobug-AMD`).
   - Portable Low-RAM launchers clamping retained host pools (`VICTORY_KEEP_FREED=0`, `VICTORY_LIST_POOL_MB=32`, `VICTORY_SNAPSHOT_POOL_MB=128`, `VICTORY_STAGING_BLOCKS=2`, `VICTORY_RIPE_BLOCKS=2`).
3. **Correctness & throughput ports**
   - **Renderer Port 6 & 7** (`VICTORY_PORT_GUARD=1`, `VICTORY_PORT_JOURNAL=1`): 10–20× reduction in `GUARDS` lock waits and 64-way sharded `JOURNAL` lock.
   - **RDNA4 magenta screen-probe filter** in `ambient.rs` (ported from `Belobug/VIctory-Belobug-AMD`).
   - **BC7 mode 4/5 rotation fix** in `crates/victory-formats/src/bcpack.rs` (upstream PR `#3111`).
   - **`VICTORY_SETTLE_ROOM=1` / `VICTORY_MERGE_RUNS`**: doubles `late_most` (256 → 512 in-flight lists) and raises shader-write merge run budget (512 → 4096).

## Linux Host Prerequisites (Critical — 3×+ speedup!)

1. **Kernel `userfaultfd` write-watch** so Wine uses `UFFDIO_WRITEPROTECT` + `PAGEMAP_SCAN` instead of slow signal-based page faults:

```sh
sudo sysctl -w vm.unprivileged_userfaultfd=1
# To persist across reboots:
echo 'vm.unprivileged_userfaultfd = 1' | sudo tee /etc/sysctl.d/99-userfaultfd.conf
```

2. **Kernel `ntsync` (`/dev/ntsync`)** — available in Linux 6.14+ and automatically used by Wine 11.x (`anon_inode:ntsync`) for near-native Win32 mutex/event synchronization.
3. **Swap / zram** — on 16 GB host RAM, having `zram` (~8 GiB) + swapfile avoids OOM during initial world streaming while `VICTORY_RESIDENCY=1` keeps working set inside RAM + zram (~5.5 GiB peak swap).

## Building from Linux (`cargo-xwin`)

```sh
# Release binary
cargo xwin build --release --target x86_64-pc-windows-msvc

# Diagnostics build (optional, for frame/timing logs)
cargo xwin build --profile diag --features diagnostics --target x86_64-pc-windows-msvc
```

## Running on Linux / Wine

Three portable launchers are included in [`scripts/`](scripts/):

- **`scripts/play-linux-lowram.sh`** *(Recommended for 16 GB RAM + 16 GB VRAM)* — Xbox Series X Low-RAM 1080p mode with ambient stand-in. Measured **5.3–6.7 FPS (148.9 ms/frame, 84% GPU busy, 13.6 GB VRAM, 0 settles/frame)** on RX 9060 XT 16 GB + Arch Linux / Wine 11.22.
- **`scripts/play-linux-quality.sh`** — Xbox Series X Low-RAM 1080p mode with BVH/Ray Tracing enabled.
- **`scripts/play-linux-series-s.sh`** — Xbox Series S mode (`--console series-s`, 7.6 GiB console RAM). Note: because the 7.6 GiB console budget causes heavier in-game texture streaming churn on GTA VI, `play-linux-lowram.sh` (Series X) is significantly faster on 16 GB RAM + zram hosts.

Example:

```sh
VICTORY_GAME_LINUX_DIR="$HOME/Games/GTAVI" \
VICTORY_RUNTIME_DIR="$HOME/Games/GTAVI-runtime" \
WINEPREFIX="$HOME/.wine-gta" \
./scripts/play-linux-lowram.sh
```

## Key Low-RAM & Wine Tuning Flags

| Flag | Value | Measured Impact |
| --- | --- | --- |
| `VICTORY_PORT_CONSTANTS=1` | `1` *(Wine `MEM_PRIVATE`)* | Zero-copy 174 MB 32-bit heap via `VK_EXT_external_memory_host` (`5,464 fills/frame`, `10.6 ms` copies, `96–99%` GPU busy, `13.28 GB` VRAM) |
| `VICTORY_PORT_DESCRIPTORS=1` | `1` + `CHECK=4` | `6/6` stores patched (99.93% captured); `327 → 4` heap pages, `32.15 → 0.00–1.80 ms` asking |
| `VICTORY_PORT_GUARD=1` | `1` | Cuts `GUARDS` lock waits 10–100× (`0.25–1.02 ms`, `17–32 faults/frame` with Port 1) |
| `VICTORY_GUARD_HOT=150` | `150` ms | Cuts hot-block fault storms and claim spins (`92k → 21–228`) |
| `VICTORY_PORT_JOURNAL=1` | `1` | 64-way sharded `JOURNAL` lock (`0.00 ms` lock wait, `0.01 ms` heap-record wait) |
| `VICTORY_SETTLE_ROOM=1` | `1` | Fewer mid-frame settles (`512` lists, `4096` merged runs) |
| `VICTORY_RESIDENCY=1` + `VICTORY_RESIDENCY_LIMITED=1` | `1` | Caps `UPLOAD_AHEAD` to 32 MiB and drops idle staging/scratch when free RAM < 3 GiB |
| `VICTORY_TEXTURE_STAGE_MS=35` + `VICTORY_TEXTURE_CATCHUP=0` | `35` ms / `64` MB | Cuts `heap buffers` recording-thread stalls **15×** (`471–592 ms → 10–40 ms`) |
| `VICTORY_PIPELINE_WAIT_MS=50` | `50` ms | Prevents background shader compiles from locking cutscenes to 2 FPS |
| `VICTORY_PORT_CBV=1` | **OFF by default** | Changes shader cache key; only enable if pre-warming shaders |
