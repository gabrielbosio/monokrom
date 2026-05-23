mod app;
mod compiler;
mod config;
mod editor;
mod filesystem;
mod input;
mod render;
mod terminal;
mod ui;
mod vm;

use macroquad::prelude::*;

use app::App;
use config::{MAP_REGION_START, SPRITE_REGION_START};
use vm::Vm;

#[cfg(not(target_arch = "wasm32"))]
use compiler::bytecode::Bytecode;

#[cfg(target_arch = "wasm32")]
use app::split_data_sections;
#[cfg(target_arch = "wasm32")]
use compiler::compile;
#[cfg(target_arch = "wasm32")]
use filesystem::web_io::get_embedded_source;

fn window_conf() -> Conf {
    Conf {
        window_title: "Monokrom".to_string(),
        window_width: 480,  // 3x native (fits 1024x600)
        window_height: 432, // 3x native
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_embedded_game() -> Option<Vm> {
    let exe = std::env::current_exe().ok()?;
    let data = std::fs::read(exe).ok()?;
    if data.len() < 8 {
        return None;
    }
    if &data[data.len() - 4..] != b"MKRM" {
        return None;
    }
    let payload_len =
        u32::from_le_bytes(data[data.len() - 8..data.len() - 4].try_into().ok()?) as usize;
    if data.len() < 8 + payload_len {
        return None;
    }
    let payload_start = data.len() - 8 - payload_len;
    let payload = &data[payload_start..data.len() - 8];
    if payload.len() < 4 {
        return None;
    }
    let bc_len = u32::from_le_bytes(payload[..4].try_into().ok()?) as usize;
    if payload.len() < 4 + bc_len {
        return None;
    }
    let bc = Bytecode::deserialize(&payload[4..4 + bc_len]).ok()?;
    let mut vm = Vm::new(&bc, 0.0).ok()?;
    let spr_start = 4 + bc_len;
    if payload.len() >= spr_start + 4096 {
        vm.memory[SPRITE_REGION_START..SPRITE_REGION_START + 4096]
            .copy_from_slice(&payload[spr_start..spr_start + 4096]);
    }
    let map_start = spr_start + 4096;
    if payload.len() >= map_start + 4096 {
        vm.memory[MAP_REGION_START..MAP_REGION_START + 4096]
            .copy_from_slice(&payload[map_start..map_start + 4096]);
    }
    Some(vm)
}

#[cfg(target_arch = "wasm32")]
fn detect_embedded_game() -> Option<Vm> {
    let full_source = get_embedded_source()?;
    let (source, spr, map) = split_data_sections(&full_source);
    let bc = compile(source).ok()?;
    let mut vm = Vm::new(&bc, 0.0).ok()?;
    vm.memory[SPRITE_REGION_START..SPRITE_REGION_START + 4096].copy_from_slice(&spr);
    vm.memory[MAP_REGION_START..MAP_REGION_START + 4096].copy_from_slice(&map);
    Some(vm)
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut app = match detect_embedded_game() {
        Some(vm) => App::new_player(vm).await,
        None => App::new().await,
    };

    loop {
        app.update();
        app.draw();

        next_frame().await;
    }
}
