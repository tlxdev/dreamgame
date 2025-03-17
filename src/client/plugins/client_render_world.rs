use bevy::prelude::*;
use std::collections::HashMap;

use crate::protocol::PlayerPosition;
use crate::shared::world_generation::{Chunk, ChunkCoord, ResourceType, TileType, WorldConfig};
use lightyear::prelude::client::Predicted;

// Plugin to handle rendering of the world tiles
pub struct ClientWorldRenderPlugin;

impl Plugin for ClientWorldRenderPlugin {
    fn build(&self, app: &mut App) {
        info!("Building ClientWorldRenderPlugin");
        app.insert_resource(TileRenderState {
            rendered_chunks: HashMap::new(),
            tile_sprites: None,
        })
        .add_systems(Startup, setup_tile_sprites)
        .add_systems(
            Update,
            (
                render_new_chunks,
                update_visible_chunks.after(render_new_chunks),
                camera_follow_player,
            ),
        );
    }
}

// Resource to track which chunks have been rendered and store sprite handles
#[derive(Resource)]
pub struct TileRenderState {
    pub rendered_chunks: HashMap<ChunkCoord, Entity>, // Maps chunk coords to their render parent entity
    pub tile_sprites: Option<TileSprites>,            // Sprites for different tile types
}

// Sprites for rendering different tile types
#[derive(Resource, Clone)]
pub struct TileSprites {
    pub grass: Handle<Image>,
    pub water: Handle<Image>,
    pub sand: Handle<Image>,
    pub stone: Handle<Image>,
    pub forest: Handle<Image>,
    pub mountain: Handle<Image>,
    pub snow: Handle<Image>,

    // Resource images
    pub iron: Handle<Image>,
    pub copper: Handle<Image>,
    pub coal: Handle<Image>,
    pub gold: Handle<Image>,
    pub tree: Handle<Image>,
    pub resource_stone: Handle<Image>,
}

// Setup sprites for tile rendering - using colored sprites for simplicity
fn setup_tile_sprites(
    mut commands: Commands,
    mut tile_render_state: ResMut<TileRenderState>,
    asset_server: Res<AssetServer>,
) {
    info!("Setting up tile sprites");

    // We'll use solid-colored sprites for each tile type
    // In a real game, you'd load actual textures here
    let tile_sprites = TileSprites {
        // Base tile types
        grass: make_colored_image(Color::rgb(0.2, 0.8, 0.2), &asset_server),
        water: make_colored_image(Color::rgb(0.0, 0.3, 0.8), &asset_server),
        sand: make_colored_image(Color::rgb(0.9, 0.9, 0.5), &asset_server),
        stone: make_colored_image(Color::rgb(0.5, 0.5, 0.5), &asset_server),
        forest: make_colored_image(Color::rgb(0.0, 0.6, 0.0), &asset_server),
        mountain: make_colored_image(Color::rgb(0.4, 0.3, 0.2), &asset_server),
        snow: make_colored_image(Color::rgb(0.9, 0.9, 1.0), &asset_server),

        // Resource types
        iron: make_colored_image(Color::rgb(0.6, 0.6, 0.7), &asset_server),
        copper: make_colored_image(Color::rgb(0.8, 0.5, 0.2), &asset_server),
        coal: make_colored_image(Color::rgb(0.1, 0.1, 0.1), &asset_server),
        gold: make_colored_image(Color::rgb(0.9, 0.8, 0.0), &asset_server),
        tree: make_colored_image(Color::rgb(0.0, 0.4, 0.0), &asset_server),
        resource_stone: make_colored_image(Color::rgb(0.4, 0.4, 0.4), &asset_server),
    };

    // Store sprites in resource
    tile_render_state.tile_sprites = Some(tile_sprites);

    // Create a camera that works well for a 2D top-down game
    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(0.0, 0.0, 999.9),
        ..default()
    });
}

// Helper to create colored sprites
fn make_colored_image(color: Color, asset_server: &AssetServer) -> Handle<Image> {
    // Create a new 16x16 image filled with the specified color
    let mut image_data = Vec::with_capacity(16 * 16 * 4);
    // RGBA format (4 bytes per pixel)
    for _ in 0..(16 * 16) {
        // Convert color to RGBA bytes - handle different Bevy versions
        let r = (color.to_srgba().red * 255.0) as u8;
        let g = (color.to_srgba().green * 255.0) as u8;
        let b = (color.to_srgba().blue * 255.0) as u8;
        let a = (color.to_srgba().alpha * 255.0) as u8;
        image_data.push(r);
        image_data.push(g);
        image_data.push(b);
        image_data.push(a);
    }

    // Create an Image from the data
    let image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        image_data,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        bevy::render::render_asset::RenderAssetUsages::default(),
    );

    // Create a handle for the image
    asset_server.add(image)
}

// System to render new chunks as they are loaded
fn render_new_chunks(
    mut commands: Commands,
    chunks_query: Query<(Entity, &Chunk), Added<Chunk>>,
    world_config: Res<WorldConfig>,
    mut render_state: ResMut<TileRenderState>,
) {
    // Extract and clone sprites before doing any mutable operations
    let sprites_option = render_state.tile_sprites.clone();
    let Some(sprites) = sprites_option else {
        return;
    };

    let chunk_size = world_config.chunk_size as f32;

    for (entity, chunk) in chunks_query.iter() {
        // Check if we've already rendered this chunk
        if render_state.rendered_chunks.contains_key(&chunk.coord) {
            continue;
        }

        info!("Rendering chunk at {:?}", chunk.coord);

        // Create a parent entity for this chunk's tiles
        let chunk_parent = commands
            .spawn((
                SpatialBundle {
                    transform: Transform::from_xyz(
                        chunk.coord.x as f32 * chunk_size,
                        chunk.coord.y as f32 * chunk_size,
                        0.0,
                    ),
                    ..default()
                },
                chunk.coord,
            ))
            .id();

        // Add tiles as children of the chunk parent
        commands.entity(chunk_parent).with_children(|parent| {
            for y in 0..chunk.tiles.len() {
                for x in 0..chunk.tiles[y].len() {
                    let tile = &chunk.tiles[y][x];

                    // Get the sprite for this tile type
                    let tile_sprite = match tile.tile_type {
                        TileType::Grass => &sprites.grass,
                        TileType::Water => &sprites.water,
                        TileType::Sand => &sprites.sand,
                        TileType::Stone => &sprites.stone,
                        TileType::Forest => &sprites.forest,
                        TileType::Mountain => &sprites.mountain,
                        TileType::Snow => &sprites.snow,
                    };

                    // Spawn the tile as a sprite
                    let tile_size = 0.9; // Slightly smaller than 1.0 to have small gaps between tiles
                    let mut tile_entity = parent.spawn((
                        Sprite {
                            custom_size: Some(Vec2::new(tile_size, tile_size)),
                            color: Color::WHITE,
                            image: tile_sprite.clone(),
                            ..default()
                        },
                        Transform::from_xyz(x as f32, y as f32, 0.0),
                    ));

                    // If the tile has a resource, add a resource indicator on top
                    if tile.resource != ResourceType::None {
                        let resource_sprite = match tile.resource {
                            ResourceType::Iron => &sprites.iron,
                            ResourceType::Copper => &sprites.copper,
                            ResourceType::Coal => &sprites.coal,
                            ResourceType::Gold => &sprites.gold,
                            ResourceType::Tree => &sprites.tree,
                            ResourceType::Stone => &sprites.resource_stone,
                            ResourceType::None => continue,
                        };

                        // Add a smaller resource indicator on top of the tile
                        tile_entity.with_children(|resource_parent| {
                            resource_parent.spawn((
                                Sprite {
                                    custom_size: Some(Vec2::new(tile_size * 0.5, tile_size * 0.5)),
                                    color: Color::WHITE,
                                    image: resource_sprite.clone(),
                                    ..default()
                                },
                                Transform::from_xyz(0.0, 0.0, 0.1),
                            ));
                        });
                    }
                }
            }
        });

        // Store the rendered chunk in our state
        render_state
            .rendered_chunks
            .insert(chunk.coord, chunk_parent);
    }
}

// System to update existing rendered chunks (not needed for basic implementation)
fn update_visible_chunks(
    mut render_state: ResMut<TileRenderState>,
    chunk_visibility_query: Query<(Entity, &ChunkCoord), Without<Chunk>>,
) {
    // This is a placeholder for potential future enhancements like:
    // - LOD (Level of Detail) system
    // - Chunk visibility culling
    // - Dynamic updates to chunks
}

// System to make the camera follow the player
fn camera_follow_player(
    player_query: Query<&PlayerPosition, With<Predicted>>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
    world_config: Res<WorldConfig>,
) {
    // If we have a player and a camera, make the camera follow the player
    if let (Ok(player_pos), Ok(mut camera_transform)) =
        (player_query.get_single(), camera_query.get_single_mut())
    {
        // Calculate world position
        let chunk_size = world_config.chunk_size as f32;

        // Smooth follow with some scaling to ensure proper view of the world
        camera_transform.translation.x = player_pos.x;
        camera_transform.translation.y = player_pos.y;

        // Set an appropriate zoom level based on the chunk size
        // This can be adjusted based on preference
        let zoom_factor = chunk_size / 16.0; // Adjust this divisor to change the default zoom
        camera_transform.scale = Vec3::new(zoom_factor, zoom_factor, 1.0);
    }
}
