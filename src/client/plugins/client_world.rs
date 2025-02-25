use bevy::prelude::*;
use lightyear::prelude::client::*;
use std::collections::HashSet;

use crate::protocol::*; // Assuming this is where your Lightyear protocol is defined
                        // Include your world generation module
use crate::shared::world_generation::{
    deserialize_chunk, Chunk, ChunkChannel, ChunkCoord, ChunkData, ChunkRequest, ResourceType,
    TileType, WorldConfig,
};

// Client-side plugin for handling world data
pub struct ClientWorldPlugin;

impl Plugin for ClientWorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientWorldState>().add_systems(
            Update,
            (
                request_visible_chunks,
                handle_chunk_data,
                update_visible_chunks,
            ),
        );
    }
}

// Client-specific world state
#[derive(Resource, Default)]
pub struct ClientWorldState {
    pub visible_chunks: HashSet<ChunkCoord>,
    pub loaded_chunks: HashSet<ChunkCoord>,
    pub player_chunk: Option<ChunkCoord>,
    pub view_distance: i32,
}

// System to track which chunk the player is in
fn update_visible_chunks(
    player_query: Query<&PlayerPosition>,
    world_config: Res<WorldConfig>,
    mut client_world: ResMut<ClientWorldState>,
) {
    // Only process if we have a player
    if let Ok(player_pos) = player_query.get_single() {
        // Calculate which chunk the player is in
        let chunk_size = world_config.chunk_size as i32;
        let chunk_x = (player_pos.x as i32).div_euclid(chunk_size);
        let chunk_y = (player_pos.y as i32).div_euclid(chunk_size);
        let current_chunk = ChunkCoord {
            x: chunk_x,
            y: chunk_y,
        };

        // Update player chunk if changed
        if client_world.player_chunk != Some(current_chunk) {
            client_world.player_chunk = Some(current_chunk);

            // Determine visible chunks based on view distance
            let view_dist = client_world.view_distance;
            let mut new_visible = HashSet::new();

            for y in -view_dist..=view_dist {
                for x in -view_dist..=view_dist {
                    new_visible.insert(ChunkCoord {
                        x: current_chunk.x + x,
                        y: current_chunk.y + y,
                    });
                }
            }

            client_world.visible_chunks = new_visible;
        }
    }
}

// System to request chunks from the server
fn request_visible_chunks(
    client_world: Res<ClientWorldState>,
    mut client: ResMut<ConnectionManager>,
) {
    // Request chunks that are visible but not yet loaded
    for coord in &client_world.visible_chunks {
        if !client_world.loaded_chunks.contains(coord) {
            // Send a request to the server for this chunk
            client.send_message::<ChunkChannel, _>(&ChunkRequest { coord: *coord });
        }
    }
}

// System to handle receiving chunk data from the server
fn handle_chunk_data(
    mut commands: Commands,
    mut events: EventReader<MessageEvent<ChunkData>>,
    mut client_world: ResMut<ClientWorldState>,
) {
    for event in events.read() {
        let chunk_data = &event.message;
        let coord = chunk_data.chunk.coord;

        // Store the chunk entity
        commands.spawn((chunk_data.chunk.clone(), coord));

        // Mark as loaded
        client_world.loaded_chunks.insert(coord);

        debug!("Received and spawned chunk at {:?}", coord);
    }
}

// Function to initialize client world state
pub fn init_client_world(commands: &mut Commands) {
    commands.insert_resource(ClientWorldState {
        visible_chunks: HashSet::new(),
        loaded_chunks: HashSet::new(),
        player_chunk: None,
        view_distance: 2, // Default view distance in chunks
    });
}
