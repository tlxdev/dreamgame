use bevy::prelude::*;

use crate::protocol::*;

#[derive(Clone)]
pub struct ExampleRendererPlugin;

impl Plugin for ExampleRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init);
        app.add_systems(Update, draw_boxes);
    }
}

#[derive(Component)]
struct AnimateTranslation;

#[derive(Component)]
struct AnimateRotation;

#[derive(Component)]
struct AnimateScale;

fn init(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    let font = asset_server.load("fonts/FiraSans-Regular.ttf");

    let text_font = TextFont {
        font: font.clone(),
        font_size: 15.0,
        ..default()
    };

    let text_justification = JustifyText::Center;
    // 2d camera
    // Demonstrate changing translation
    commands.spawn((
        Text2d::new("translation"),
        text_font.clone(),
        TextLayout::new_with_justify(text_justification),
        AnimateTranslation,
    ));
}
/// System that draws the boxes of the player positions.
/// The components should be replicated from the server to the client
pub(crate) fn draw_boxes(
    mut gizmos: Gizmos,
    players: Query<(&PlayerPosition, &PlayerColor, &PlayerName)>,
    mut text_query: Query<&mut Transform, With<Text2d>>,
) {
    for (position, color, name) in &players {
        gizmos.rect_2d(
            Isometry2d::from_translation(position.0),
            Vec2::ONE * 50.0,
            color.0,
        );

        for mut text in &mut text_query {
            text.translation.x = position.0.x;
            text.translation.y = position.0.y + 35.0; // Offset above the rect
            text.translation.z = 0.0;
        }
    }
}
