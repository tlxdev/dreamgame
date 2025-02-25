use crate::app::*;
use bevy::prelude::*;
use crate::protocol::ProtocolPlugin;
use crate::settings::get_settings;

#[derive(Component)]
struct Player;

#[cfg(feature = "server")]
mod server;
mod server_renderer;

#[cfg(feature = "client")]
mod client;
mod client_renderer;

mod protocol;

mod shared;
mod shared_config;

mod app;
mod settings;
mod settings_common;

#[cfg(feature = "gui")]
mod renderer;

fn main() {
    let cli = Cli::default();
    #[allow(unused_mut)]
    let mut settings = get_settings();
    #[cfg(target_family = "wasm")]
    lightyear_examples_common::settings::modify_digest_on_wasm(&mut settings.client);

    let mut app = Apps::new(settings, cli, env!("CARGO_PKG_NAME").to_string());

    app.add_lightyear_plugins();
    app.add_user_shared_plugin(ProtocolPlugin);
    app.add_user_shared_plugin(shared::world_generation::WorldGenerationPlugin);
    #[cfg(feature = "client")]
    app.add_user_client_plugin(client::ExampleClientPlugin);
    app.add_user_client_plugin(client::plugins::ClientWorldPlugin);

    #[cfg(feature = "server")]
    app.add_user_server_plugin(server::ExampleServerPlugin);
    app.add_user_server_plugin(server::plugins::ServerWorldPlugin);
    #[cfg(feature = "gui")]
    app.add_user_renderer_plugin(renderer::ExampleRendererPlugin);
    // run the app
    app.run();
}

// 2d camera
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

fn setup_player(mut commands: Commands) {
    // player sprite
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::WHITE,
                ..default()
            },
            transform: Transform {
                scale: Vec3::new(10.0, 10.0, 1.0),
                ..default()
            },
            ..default()
        })
        .insert(Player);
}

// player movement
fn player_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<&mut Transform, With<Player>>,
) {
    if keyboard_input.pressed(KeyCode::KeyW) {
        player_query.single_mut().translation.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        player_query.single_mut().translation.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        player_query.single_mut().translation.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        player_query.single_mut().translation.x += 1.0;
    }
}
