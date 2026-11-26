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
2. **16 GB VRAM / Low-RAM stability & visual fidelity**
   - **`VICTORY_DSTORAGE_IO=direct`:** streams `.rpf` assets via `FILE_FLAG_NO_BUFFERING` (`O_DIRECT` under Wine), preventing Linux page-cache bloat from evicting the game's working set on 16 GB RAM hosts.
   - **Idle VRAM offload:** shifts more window/buffer/BVH arenas (`VICTORY_WINDOWS_BAR_MB=2560`, `VICTORY_BUFFERS_BAR_MB=2048`, `VICTORY_CARD_BUFFERS_MB=3072`, `VICTORY_MEMORY_CARD_MB=2560`, `VICTORY_BAR_ROOM_PERCENT=88`) out of tight 16 GB system RAM into free 16 GB VRAM (cutting swap usage by ~1.7 GB and reaching **5.0–8.6 FPS** even in `diag` builds with full RT).
   - `bar_room_percent()` defaults to `room_percent() - 4` (`VICTORY_BAR_ROOM_PERCENT`), keeping ReBAR buffer/window pools strictly below the texture VRAM budget so world textures never starve (`the card has no room`).
   - `descriptor/host.rs`: `keep_shown` is gated by `crate::resource::room_lately()` (ported from `Belobug/VIctory-Belobug-AMD`).
   - Portable Low-RAM launchers clamping retained host pools (`VICTORY_KEEP_FREED=0`, `VICTORY_LIST_POOL_MB=32`, `VICTORY_SNAPSHOT_POOL_MB=128`, `VICTORY_STAGING_BLOCKS=2`, `VICTORY_RIPE_BLOCKS=2`).
3. **Correctness, lighting, mesh/LOD, & texture ports**
   - **Real Ray Tracing (`VICTORY_BVH_BUILDER=1`)** enabled by default (all 53 BVH builder pipelines compile and run on RADV; ~25 ms GPU frame time on RX 9060 XT when VRAM-backed buffers are used).
   - **Typed 32-bit integer buffer views (`bit_exact_format`, PR `#684`):** maps `R32*_{UINT,SINT}` typed buffer views to matching `FLOAT` formats so `bufferStore.f32` preserves packed meshlet culling indices bit-for-bit on RADV/D3D12.
   - **RDNA2 `v_mad_legacy_f32` / `v_mac_legacy_f32` semantics (PR `#1026`):** fused FMA + subnormal flush (`ftz`) + zero-factor rule (`0 * inf = 0`).
   - **Skin/decoration bake source uploads (`VICTORY_BAKE_SOURCES=1`, PR `#1264`):** uploads each skin-bake draw's own sampled source textures ahead of the draw (`0` missing skin-bake sources; restores character facial/skin/bruise details).
   - **Blank/stale placed texture recovery (`VICTORY_REASK_BLANK=1`, `fxceless 06702fe`):** re-queues placed texture slots left stuck NULL/zero after aliasing invalidation once their memory is committed, plus `VICTORY_TEXTURE_LOOK_MS=500` so waiting textures don't back off for 30+ seconds.
   - **Lighting fallbacks (`VICTORY_BVH_BUILDER=0`):** skips `CS_ScreenProbes_ActivateRays` without rays (`belobug 6986a57`), adds `VICTORY_AMBIENT_REFLECT=0.5` reflection clamping on `CS_UpdateHistory`, `VICTORY_AMBIENT_AIR=1.5`, `VICTORY_AMBIENT_SETTLE=250,3,1.5`, and RDNA4 magenta screen-probe filter.
   - **Renderer Port 6 & 7** (`VICTORY_PORT_GUARD=1`, `VICTORY_PORT_JOURNAL=1`), null CBV clear (`#2449`), typed UAV store format check (`#2462`), BC7 mode 4/5 rotation fix (`#3111`), and `VICTORY_SETTLE_ROOM=1`.

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

- **`scripts/play-linux-lowram.sh`** *(Recommended for 16 GB RAM + 16 GB VRAM)* — Xbox Series X Low-RAM 1080p mode with **Real Ray Tracing (`VICTORY_BVH_BUILDER=1`)**, `VICTORY_DSTORAGE_IO=direct`, `VICTORY_BAKE_SOURCES=1`, `VICTORY_REASK_BLANK=1`, `VICTORY_TEXTURE_LOOK_MS=500`, and VRAM-backed buffers (`PORT_CONSTANTS=0`, `PORT_CBV=0`, `INPLACE=0`). Measured **5.0–8.6 FPS (116.8 ms/frame, 6–12% GPU busy, ~7.4–9.3 GB VRAM)** with crisp character faces, expressions, skin details, and RT lighting on RX 9060 XT 16 GB + Arch Linux / Wine 11.22.
- **`scripts/play-linux-quality.sh`** — Same Real Ray Tracing profile with higher pipeline wait budget (`300 ms`).
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
| `VICTORY_BVH_BUILDER=1` | `1` *(default)* | All 53 BVH builder pipelines run on RADV (~25 ms GPU time); restores real RT GI, reflections, and nighttime/interior lighting |
| `VICTORY_PORT_CONSTANTS=0` + `VICTORY_INPLACE=0` | `0` *(Wine/RADV)* | Keeps GPU buffers in fast VRAM instead of PCIe host RAM (`VK_EXT_external_memory_host`), cutting GPU time **15×** (380 ms → 25 ms) and fixing missing roads/vehicles in the opening cutscene |
| `VICTORY_PORT_DESCRIPTORS=1` | `1` + `CHECK=4` | `6/6` stores patched (99.93% captured); `327 → 4` heap pages, `32.15 → 0.00–1.80 ms` asking |
| `VICTORY_PORT_GUARD=1` + `VICTORY_PORT_JOURNAL=1` | `1` | Cuts `GUARDS` lock waits 10–100× and 64-way shards `JOURNAL` lock (`0.00 ms` lock wait) |
| `VICTORY_DSTORAGE_IO=direct` | `direct` | Bypasses Linux page cache (`O_DIRECT`) when streaming `.rpf` files so 16 GB RAM isn't flooded by file cache |
| `VICTORY_BAKE_SOURCES=1` + `VICTORY_REASK_BLANK=1` | `1` | `0` missing skin-bake sources (restores face/skin/bruise detail) and re-queues stuck blank placed texture slots |
| `VICTORY_TEXTURE_LOOK_MS=500` | `500` ms | Cuts texture upload latency in half (`1100 ms → 574 ms`, `90% < 1 s`) instead of 30+ s backoff |
| `VICTORY_WINDOWS_BAR_MB=2560` + `VICTORY_BUFFERS_BAR_MB=2048` + `VICTORY_CARD_BUFFERS_MB=3072` | VRAM offload | Moves buffers out of tight 16 GB system RAM into idle 16 GB VRAM (~1.7 GB less swap, up to 8.6 FPS in diag) |
| `VICTORY_RESIDENCY=1` + `VICTORY_RESIDENCY_LIMITED=1` | `1` | Protects 16 GB host against OOM during heavy world streaming |
