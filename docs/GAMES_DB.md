# Games Database Notes

This repo maintains a small curated list of popular games in `src/games/database.rs`.

The goal is to:

- Give users consistent benchmark guidance.
- Gate UI toggles (RT / DLSS / FSR) so we do not present options that likely do not exist for a game.

## Flag Semantics

- `has_benchmark`: The game ships with an in-game benchmark mode, or an official standalone benchmark tool.
- `supports_rt`: The game exposes a user-facing ray tracing option/preset/effects toggle (not just "uses RT internally").
- `supports_dlss`: The game exposes an NVIDIA DLSS option in graphics settings.
- `supports_fsr`: The game exposes an AMD FSR option in graphics settings.

## Curation Rules

- Prefer conservative defaults. If unsure, set the capability flag to `false` and keep `benchmark_notes` helpful.
- Avoid unreleased/speculative titles (no placeholders).
- Keep `benchmark_notes` user-facing and repeatability-focused:
  - what to run,
  - for how long (typically 60-120s),
  - how to reduce variance (same route, same map, avoid patch days).

## Evidence Links (Examples)

These are examples of sources used when updating capability flags:

- Rainbow Six Siege DLSS (NVIDIA): https://www.nvidia.com/en-us/geforce/news/rainbow-six-siege-vulkan-nvidia-dlss/
- Warzone DLSS (NVIDIA): https://www.nvidia.com/en-us/geforce/news/call-of-duty-vanguard-warzone-dlss-reflex-ray-tracing/
- The Finals DLSS + RTXGI (NVIDIA): https://www.nvidia.com/en-us/geforce/news/the-finals-dlss-3-rtxgi/
- Warframe DLSS/Reflex/RT (Warframe official news): https://www.warframe.com/news/whispers-in-the-walls
- Black Myth: Wukong official benchmark tool (Steam): https://store.steampowered.com/app/3132990/Black_Myth_Wukong_Benchmark_Tool/

