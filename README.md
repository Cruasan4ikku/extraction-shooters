# Extraction Game

A single-player PvE extraction shooter written in Rust: `eft-core` contains game rules, `eft-backend` stores the profile, and `eft-client` is the Bevy client. Uses demo data. Development roadmap: [plan.md](plan.md).

## Run

On Windows, run `run-game.cmd` or `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-game.ps1`. To create a portable package, run `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\package-windows.ps1`, extract the ZIP, then run `launch-game.cmd`.

For manual development, run `cargo +stable-x86_64-pc-windows-gnu run -p eft-backend`, then `cargo +stable-x86_64-pc-windows-gnu run -p eft-client`.

## Controls

Shelter: `WASD` move, click to look, `I` stash, `Esc` menu.

Raid: `WASD` move, mouse look/fire, `Shift` sprint, `Ctrl` crouch, `R` reload, `E` loot, `F` extract, `I` inventory, `U` heal, `Esc` pause. Carried gear is lost on death.

## Profile and API

Profiles and settings are stored in a game-specific folder under `%LOCALAPPDATA%`. The backend listens on `127.0.0.1:3000`; `INVENTORY_FILE`, `BIND_ADDR`, and `EFT_BACKEND_URL` override the file path and service address. Routes: `GET /health`, `GET/PUT /inventory`, `POST /inventory/validate`. Invalid inventories return `422`; a missing profile returns `404`. Saves are versioned and written atomically.

## Development

Run tests with `cargo +stable-x86_64-pc-windows-gnu test --workspace`. Do not enable `dev-dynamic-linking` with the Windows GNU toolchain; Bevy's final link step fails. A compatible graphics and audio driver is required.

Audio in `assets\audio`: [Gunshot Sounds](https://opengameart.org/content/gunshot-sounds) - CC BY 3.0, Vincent Sevedge; [Fantozzi's Footsteps](https://opengameart.org/content/fantozzis-footsteps-grasssand-stone), [Wind Woosh Loop](https://opengameart.org/content/wind-whoosh-loop), and [Impact](https://opengameart.org/content/impact) - CC0 (Impact by Iwan "qubodup" Gabovitch).
