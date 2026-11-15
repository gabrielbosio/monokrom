mod app;
mod audio;
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
use config::{
    APU_MASTER_VOLUME, APU_PANNING, MAP_REGION_START, SFX_REGION_SIZE, SFX_REGION_START,
    SPRITE_REGION_START, WAVE_RAM_SIZE, WAVE_RAM_START, WAVE_TABLE_REGION_SIZE,
    WAVE_TABLE_REGION_START,
};
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
    let waves_start = map_start + 4096;
    if payload.len() >= waves_start + WAVE_TABLE_REGION_SIZE {
        vm.memory[WAVE_TABLE_REGION_START..WAVE_TABLE_REGION_START + WAVE_TABLE_REGION_SIZE]
            .copy_from_slice(&payload[waves_start..waves_start + WAVE_TABLE_REGION_SIZE]);
        vm.memory[WAVE_RAM_START..WAVE_RAM_START + WAVE_RAM_SIZE]
            .copy_from_slice(&payload[waves_start..waves_start + WAVE_RAM_SIZE]);
    }
    let sfx_start = waves_start + WAVE_TABLE_REGION_SIZE;
    if payload.len() > sfx_start {
        vm.memory[SFX_REGION_START..SFX_REGION_START + SFX_REGION_SIZE].fill(0);
        let sfx = &payload[sfx_start..];
        let sfx_len = sfx.len().min(SFX_REGION_SIZE);
        vm.memory[SFX_REGION_START..SFX_REGION_START + sfx_len].copy_from_slice(&sfx[..sfx_len]);
    }
    vm.memory[APU_MASTER_VOLUME] = 0xFF;
    vm.memory[APU_PANNING] = 0xFF;
    Some(vm)
}

#[cfg(target_arch = "wasm32")]
fn detect_embedded_game() -> Option<Vm> {
    let full_source = get_embedded_source()?;
    let (source, spr, map, waves, sfx, _ui) = split_data_sections(&full_source);
    let bc = compile(source).ok()?;
    let mut vm = Vm::new(&bc, 0.0).ok()?;
    vm.memory[SPRITE_REGION_START..SPRITE_REGION_START + 4096].copy_from_slice(&spr);
    vm.memory[MAP_REGION_START..MAP_REGION_START + 4096].copy_from_slice(&map);
    vm.memory[WAVE_TABLE_REGION_START..WAVE_TABLE_REGION_START + WAVE_TABLE_REGION_SIZE]
        .copy_from_slice(&waves);
    vm.memory[SFX_REGION_START..SFX_REGION_START + SFX_REGION_SIZE].fill(0);
    let sfx_len = sfx.len().min(SFX_REGION_SIZE);
    vm.memory[SFX_REGION_START..SFX_REGION_START + sfx_len].copy_from_slice(&sfx[..sfx_len]);
    vm.memory[APU_MASTER_VOLUME] = 0xFF;
    vm.memory[APU_PANNING] = 0xFF;
    vm.memory[WAVE_RAM_START..WAVE_RAM_START + WAVE_RAM_SIZE]
        .copy_from_slice(&waves[..WAVE_RAM_SIZE]);
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
        app.process_audio();

        next_frame().await;
    }
}
