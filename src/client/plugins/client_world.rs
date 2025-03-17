use bevy::prelude::*;
use lightyear::prelude::client::*;
use std::collections::{HashMap, HashSet};

use crate::protocol::*;
use crate::shared::world_generation::{
    deserialize_chunk, Chunk, ChunkChannel, ChunkCoord, ChunkData, ChunkRequest, ResourceType,
    TileType, WorldConfig,
};

// Client-side plugin for handling world data
pub struct ClientWorldPlugin;

impl Plugin for ClientWorldPlugin {
    fn build(&self, app: &mut App) {
        info!("Building ClientWorldPlugin");
        app.insert_resource(ClientWorldState {
            visible_chunks: HashSet::new(),
            loaded_chunks: HashSet::new(),
            requested_chunks: HashMap::new(),
            player_chunk: None,
            view_distance: 2, // Default view distance in chunks
            frame_counter: 0, // Track how many frames we've processed
        })
        .add_systems(
            Update,
            (
                // First update player position and calculate visible chunks
                update_visible_chunks,
                // Clean up chunks that are no longer visible
                cleanup_invisible_chunks,
                // Then process any received chunk data
                handle_chunk_data,
                // Finally request any chunks we still need
                request_visible_chunks,
                // Debug system to monitor chunk state
                debug_chunk_state,
            )
                .chain(), // Ensure these systems run in order
        );
    }
}

// Client-specific world state
#[derive(Resource)]
pub struct ClientWorldState {
    pub visible_chunks: HashSet<ChunkCoord>,
    pub loaded_chunks: HashSet<ChunkCoord>,
    pub requested_chunks: HashMap<ChunkCoord, u32>, // Map of requested chunks and the frame they were requested
    pub player_chunk: Option<ChunkCoord>,
    pub view_distance: i32,
    pub frame_counter: u32, // Track frames for debugging
}

// System to track which chunk the player is in and update visible chunks
fn update_visible_chunks(
    mut player_query: Query<&mut PlayerPosition, With<Predicted>>,
    world_config: Res<WorldConfig>,
    mut client_world: ResMut<ClientWorldState>,
) {
    // Increment frame counter to track system calls
    client_world.frame_counter += 1;

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

        // Update player chunk and visible chunks if this is the first run
        // or if the player has moved to a different chunk
        let should_update =
            client_world.player_chunk.is_none() || client_world.player_chunk != Some(current_chunk);

        if should_update {
            info!(
                "Updating visible chunks - reason: {}, frame: {}",
                if client_world.player_chunk.is_none() {
                    "first run"
                } else {
                    "player moved chunks"
                },
                client_world.frame_counter
            );

            client_world.player_chunk = Some(current_chunk);

            // Save the old visible chunks for comparison
            let old_visible = client_world.visible_chunks.clone();

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

            // Calculate new chunks that have become visible
            let new_chunks_count = new_visible.difference(&old_visible).count();

            // Update visible chunks
            client_world.visible_chunks = new_visible;

            info!(
                "Updated visible chunks: now tracking {} chunks, {} loaded, {} new chunks visible",
                client_world.visible_chunks.len(),
                client_world.loaded_chunks.len(),
                new_chunks_count
            );
        }
    }
}

// System to clean up chunks that are no longer visible
fn cleanup_invisible_chunks(
    mut commands: Commands,
    mut client_world: ResMut<ClientWorldState>,
    chunk_query: Query<(Entity, &ChunkCoord)>,
) {
    // Find chunks to remove (loaded but no longer visible)
    let mut chunks_to_remove = HashSet::new();

    for coord in &client_world.loaded_chunks {
        if !client_world.visible_chunks.contains(coord) {
            chunks_to_remove.insert(*coord);
        }
    }

    // If we have chunks to remove, despawn them and update our tracking
    if !chunks_to_remove.is_empty() {
        info!(
            "Cleaning up {} chunks that are no longer visible",
            chunks_to_remove.len()
        );

        // Remove from loaded set
        for coord in &chunks_to_remove {
            client_world.loaded_chunks.remove(coord);
        }

        // Despawn the entities
        for (entity, coord) in chunk_query.iter() {
            if chunks_to_remove.contains(coord) {
                commands.entity(entity).despawn();
            }
        }
    }

    // Also clean up requested chunks that are no longer visible
    let mut requested_to_remove = Vec::new();
    for (coord, _) in &client_world.requested_chunks {
        if !client_world.visible_chunks.contains(coord) {
            requested_to_remove.push(*coord);
        }
    }

    for coord in requested_to_remove {
        client_world.requested_chunks.remove(&coord);
    }
}

// System to request chunks from the server
fn request_visible_chunks(
    mut client_world: ResMut<ClientWorldState>,
    mut client: ResMut<ConnectionManager>,
) {
    // Only process if we have a player with a known position
    if client_world.player_chunk.is_none() {
        return;
    }

    // Define threshold for re-requesting (only re-request chunks after 120 frames/~2 seconds)
    const REQUEST_TIMEOUT: u32 = 120;
    
    // Collect all data we need first to avoid borrowing conflicts
    let current_frame = client_world.frame_counter;
    
    // Find chunks that need to be requested (visible but not loaded)
    let mut chunks_to_request = Vec::new();
    
    for &coord in &client_world.visible_chunks {
        // Skip if already loaded
        if client_world.loaded_chunks.contains(&coord) {
            continue;
        }
        
        // Check if already requested recently
        match client_world.requested_chunks.get(&coord) {
            // If not requested or requested a long time ago, add to request list
            None => chunks_to_request.push(coord),
            Some(&frame) if current_frame - frame > REQUEST_TIMEOUT => chunks_to_request.push(coord),
            _ => {}
        }
    }
    
    // Now process all the chunks we need to request
    let requests_count = chunks_to_request.len();
    
    for coord in &chunks_to_request {
        // Send a request to the server for this chunk
        client.send_message::<ChunkChannel, _>(&ChunkRequest { coord: *coord });
        
        // Mark as requested on this frame
        client_world.requested_chunks.insert(*coord, current_frame);
    }

    // Only log if we actually requested chunks
    if !chunks_to_request.is_empty() {
        info!(
            "Frame {}: Requested {} new chunks (loaded: {}/{}, requested:  {})",
            current_frame,
            requests_count,
            client_world.loaded_chunks.len(),
            client_world.visible_chunks.len(),
            client_world.requested_chunks.len()
        );

        if requests_count <= 10 {
            // If only a few chunks, log them individually for debugging
            info!("Requested chunks: {:?}", chunks_to_request);
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

        // Skip if no longer visible (player moved away while request was in flight)
        if !client_world.visible_chunks.contains(&coord) {
            info!(
                "Received chunk at {:?} but it's no longer visible, ignoring",
                coord
            );
            continue;
        }

        // Check if we've already loaded this chunk to avoid duplicates
        if client_world.loaded_chunks.contains(&coord) {
            info!("Already have chunk at {:?}, skipping", coord);
            continue;
        }

        // Store the chunk entity
        commands.spawn((chunk_data.chunk.clone(), coord));

        // Mark as loaded and remove from requested
        client_world.loaded_chunks.insert(coord);
        client_world.requested_chunks.remove(&coord);

        info!(
            "Frame {}: Received and spawned chunk at {:?}, now have {}/{} loaded chunks",
            client_world.frame_counter,
            coord,
            client_world.loaded_chunks.len(),
            client_world.visible_chunks.len()
        );
    }
}

// Debug system to monitor the state of loaded chunks
fn debug_chunk_state(client_world: Res<ClientWorldState>) {
    // Only log every 300 frames (about every 5 seconds at 60 FPS)
    if client_world.frame_counter % 300 == 0 {
        // Check if we have the expected number of loaded chunks
        let loaded = client_world.loaded_chunks.len();
        let visible = client_world.visible_chunks.len();
        let requested = client_world.requested_chunks.len();

        info!(
            "DIAGNOSTIC: Frame {}: Loaded: {}/{} visible chunks. {} chunks pending.",
            client_world.frame_counter, loaded, visible, requested
        );

        if loaded < visible {
            // Check for chunks that are visible but not loaded or requested
            let missing: Vec<_> = client_world
                .visible_chunks
                .iter()
                .filter(|c| {
                    !client_world.loaded_chunks.contains(c)
                        && !client_world.requested_chunks.contains_key(c)
                })
                .collect();

            if !missing.is_empty() {
                info!(
                    "Found {} chunks that are visible but not loaded or requested!",
                    missing.len()
                );
                if missing.len() <= 10 {
                    info!("Missing chunks: {:?}", missing);
                }
            }

            // Check for stale requests
            let stale: Vec<_> = client_world
                .requested_chunks
                .iter()
                .filter(|(_, frame)| client_world.frame_counter - **frame > 300) // Over 5 seconds old
                .collect();

            if !stale.is_empty() {
                info!("Found {} stale chunk requests (waiting >5s)!", stale.len());
                if stale.len() <= 10 {
                    info!("Stale requests: {:?}", stale);
                }
            }
        }
    }
}
