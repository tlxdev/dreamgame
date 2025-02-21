use bevy::prelude::*;

#[derive(Component)]
struct Player;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_camera)
        .add_systems(Startup, setup_player)
        .add_systems(Startup, init)
        .add_systems(Update, player_movement)
        .run();
}

fn init(mut commands: Commands) {
    commands.connect_client();
}

// 2d camera
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

fn setup_player(mut commands: Commands) {
    // player sprite
    commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: Color::WHITE,
            ..default()
        },
        transform: Transform {
            scale: Vec3::new(10.0, 10.0, 1.0),
            ..default()
        },
        ..default()
    }).insert(Player);
}

