mod app;
mod config;
mod editor;
mod filesystem;
mod input;
mod render;
mod ui;

use macroquad::prelude::*;

use app::App;
use config::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Monokrom".to_string(),
        window_width: WINDOW_WIDTH as i32,
        window_height: WINDOW_HEIGHT as i32,
        window_resizable: false,
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
