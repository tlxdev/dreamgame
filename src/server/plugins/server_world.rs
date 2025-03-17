use bevy::prelude::*;

use crate::shared::world_generation::{
    Chunk, ChunkChannel, ChunkCoord, ChunkData, ChunkRequest, ChunkRequestEvent, WorldConfig,
    WorldState,
};

use lightyear::prelude::client::{Confirmed, Predicted};
use lightyear::prelude::server::*;
use lightyear::prelude::*;

use serde::{Deserialize, Serialize};

use lightyear::client::components::{ComponentSyncMode, LerpFn};
use lightyear::prelude::client::{self};
use lightyear::prelude::server::{Replicate, SyncTarget};

use crate::protocol::PlayerId;

// Handle client requests for chunks
pub fn handle_chunk_network_requests(
    mut commands: Commands,
    mut events: EventReader<MessageEvent<ChunkRequest>>,
    mut world_state: ResMut<WorldState>,
    world_config: Res<WorldConfig>,
    mut chunk_request_events: EventWriter<ChunkRequestEvent>,
    mut connection_manager: ResMut<ConnectionManager>,
    chunks: Query<&Chunk>, // Add this query to access Chunk components
) {
    for event in events.read() {
        let client_id = event.from();
        let coord = event.message().coord;
        info!("Client {:?} requested chunk at {:?}", client_id, coord);
        // Convert to internal event
        chunk_request_events.send(ChunkRequestEvent {
            coord,
            client_id: Some(client_id),
        });
        // If the chunk is already generated, send it immediately
        if let Some(chunk_entity) = world_state.chunks.get(&coord) {
            if let Ok(chunk) = chunks.get(*chunk_entity) {
                // Use the Query instead
                // Send the chunk data to the requesting client
                let _ = connection_manager.send_message::<ChunkChannel, _>(
                    client_id,
                    &mut ChunkData {
                        chunk: chunk.clone(),
                    },
                );
                info!("Sent existing chunk {:?} to client {:?}", coord, client_id);
            }
        }
    }
}

// System to send newly generated chunks to clients who need them
// System to send newly generated chunks to clients who need them
pub fn send_new_chunks(
    mut commands: Commands,
    mut world_state: ResMut<WorldState>,
    chunk_query: Query<(Entity, &Chunk), Added<Chunk>>,
    player_query: Query<(&PlayerId, &Transform)>,
    mut connection_manager: ResMut<ConnectionManager>,
) {
    // For each newly generated chunk
    for (entity, chunk) in chunk_query.iter() {
        let coord = chunk.coord;

        // Find players who should receive this chunk (those close enough)
        for (player_id, transform) in player_query.iter() {
            // Here you'd calculate if this player needs this chunk
            // This is a simple implementation - in practice, you might use distance checks

            // Send the chunk data to the client
            // Use player_id.0 which is the ClientId that connection_manager expects
            let _ = connection_manager.send_message::<ChunkChannel, _>(
                player_id.client_id(), // This is now correct - using the ClientId inside PlayerId
                &mut ChunkData {
                    chunk: chunk.clone(),
                },
            );

            // Add Replicate component to ensure the chunk is replicated to the client
            commands.entity(entity).insert(Replicate {
                sync: SyncTarget {
                    interpolation: NetworkTarget::All,
                    ..default()
                },
                relevance_mode: NetworkRelevanceMode::All,
                ..default()
            });

            debug!("Sent new chunk {:?} to player {:?}", coord, player_id);
        }
    }
}

// Generate chunks around player when they move to a new area
pub fn generate_chunks_around_players(
    mut commands: Commands,
    mut world_state: ResMut<WorldState>,
    world_config: Res<WorldConfig>,
    player_query: Query<(&PlayerId, &Transform), Changed<Transform>>,
    mut chunk_request_events: EventWriter<ChunkRequestEvent>,
) {
    let chunk_size = world_config.chunk_size as f32;

    for (_, transform) in player_query.iter() {
        // Calculate which chunk the player is in
        let chunk_x = (transform.translation.x / chunk_size).floor() as i32;
        let chunk_y = (transform.translation.y / chunk_size).floor() as i32;
        let player_chunk = ChunkCoord {
            x: chunk_x,
            y: chunk_y,
        };

        // Generate chunks in a radius around the player
        let view_distance = 128; // Customize based on your needs

        for y in -view_distance..=view_distance {
            for x in -view_distance..=view_distance {
                let coord = ChunkCoord {
                    x: player_chunk.x + x,
                    y: player_chunk.y + y,
                };

                // Request this chunk if it's not already generated
                if !world_state.chunks.contains_key(&coord) {
                    chunk_request_events.send(ChunkRequestEvent {
                        coord,
                        client_id: None,
                    });
                }
            }
        }
    }
}

// Server plugin for world management with networking
pub struct ServerWorldPlugin;

impl Plugin for ServerWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_chunk_network_requests,
                send_new_chunks,
                generate_chunks_around_players,
            ),
        );
    }
}
