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

#[macroquad::main(window_conf)]
async fn main() {
    let mut app = App::new().await;

    loop {
        app.update();
        app.draw();

        next_frame().await;
    }
}
