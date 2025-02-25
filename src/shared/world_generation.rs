use bevy::prelude::*;
use lightyear::client::components::ComponentSyncMode;
use lightyear::prelude::*;
use noise::{NoiseFn, Perlin, Seedable};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// World generation configuration
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct WorldConfig {
    pub seed: u32,
    pub chunk_size: usize,
    pub max_active_chunks: usize,
    pub biome_scale: f64,
    pub height_scale: f64,
    pub resource_density: f32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        WorldConfig {
            seed: 12345,
            chunk_size: 32,
            max_active_chunks: 64,
            biome_scale: 0.03,
            height_scale: 0.05,
            resource_density: 0.02,
        }
    }
}

// Coordinate system using signed integers for both chunk and world coordinates
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
}

// Tile types that can exist in the world
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileType {
    Grass,
    Water,
    Sand,
    Stone,
    Forest,
    Mountain,
    Snow,
}

// Resources that can be found in the world
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceType {
    None,
    Iron,
    Copper,
    Coal,
    Gold,
    Tree,
    Stone,
}

// Biomes used for world generation and determining tile types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BiomeType {
    Plains,
    Ocean,
    Desert,
    Forest,
    Mountain,
    Tundra,
}

// A single tile in the world
#[derive(Clone, Debug, Component, Serialize, Deserialize, PartialEq)]
pub struct Tile {
    pub tile_type: TileType,
    pub resource: ResourceType,
    pub height: f32,
    pub position: (i32, i32), // World coordinates
    pub traversable: bool,
}

// A chunk containing multiple tiles
#[derive(Clone, Debug, Component, Serialize, Deserialize, PartialEq)]
pub struct Chunk {
    pub coord: ChunkCoord,
    pub tiles: Vec<Vec<Tile>>,
    pub biome_type: BiomeType,
    pub last_accessed: f64, // Used for unloading inactive chunks
}

// Tracks the world state including all generated chunks
#[derive(Resource, Default)]
pub struct WorldState {
    pub chunks: HashMap<ChunkCoord, Entity>, // Maps chunk coords to their entity
    pub active_chunks: HashSet<ChunkCoord>,  // Currently active chunks
    pub generation_time: HashMap<ChunkCoord, f64>, // Performance tracking
    pub world_time: f64,                     // In-game time (could drive day/night cycles)
}

// Channel for world chunk data transmission
#[derive(Channel)]
pub struct ChunkChannel;

// Message for requesting chunks
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChunkRequest {
    pub coord: ChunkCoord,
}

// Message for sending chunk data
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChunkData {
    pub chunk: Chunk,
}

// Plugin for world generation
pub struct WorldGenerationPlugin;

impl Plugin for WorldGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldConfig>()
            .init_resource::<WorldState>()
            .add_event::<ChunkRequestEvent>()
            .add_systems(Startup, setup_world)
            .add_systems(Update, (handle_chunk_requests, manage_active_chunks));

        // Register this only on the server
        #[cfg(feature = "server")]
        {
            app.register_component::<Chunk>(ChannelDirection::ServerToClient)
                .add_interpolation(ComponentSyncMode::Full);

            app.register_component::<ChunkCoord>(ChannelDirection::ServerToClient)
                .add_interpolation(ComponentSyncMode::Once);

            // Register messages
            app.register_message::<ChunkRequest>(ChannelDirection::ClientToServer);
            app.register_message::<ChunkData>(ChannelDirection::ServerToClient);

            // Add channel for chunk data
            app.add_channel::<ChunkChannel>(ChannelSettings {
                mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
                ..default()
            });
        }
    }
}

// Initialize the world generation system
fn setup_world(
    mut commands: Commands,
    mut world_state: ResMut<WorldState>,
    world_config: Res<WorldConfig>,
) {
    info!("Initializing world with seed: {}", world_config.seed);

    // Generate the spawn chunk (0,0) and its neighbors
    let spawn_coords = [
        ChunkCoord { x: 0, y: 0 },
        ChunkCoord { x: -1, y: 0 },
        ChunkCoord { x: 1, y: 0 },
        ChunkCoord { x: 0, y: -1 },
        ChunkCoord { x: 0, y: 1 },
    ];

    for coord in spawn_coords.iter() {
        generate_chunk(coord, &mut commands, &mut world_state, &world_config);
    }
}

// Handle requests for new chunks (e.g., from player movement)
fn handle_chunk_requests(
    mut commands: Commands,
    mut world_state: ResMut<WorldState>,
    world_config: Res<WorldConfig>,
    mut chunk_request_events: EventReader<ChunkRequestEvent>,
) {
    for event in chunk_request_events.read() {
        if !world_state.chunks.contains_key(&event.coord) {
            generate_chunk(&event.coord, &mut commands, &mut world_state, &world_config);
        }

        // Mark the chunk as active
        world_state.active_chunks.insert(event.coord);
    }
}

// Manage active chunks, unload distant ones if needed
fn manage_active_chunks(
    mut commands: Commands,
    mut world_state: ResMut<WorldState>,
    world_config: Res<WorldConfig>,
    time: Res<Time>,
) {
    // Update world time
    world_state.world_time += time.delta_secs_f64();

    // If we're over the active chunk limit, unload the least recently accessed chunks
    if world_state.active_chunks.len() > world_config.max_active_chunks {
        let mut chunks_with_time: Vec<(ChunkCoord, f64)> = world_state
            .active_chunks
            .iter()
            .filter_map(|coord| {
                world_state
                    .generation_time
                    .get(coord)
                    .map(|time| (*coord, *time))
            })
            .collect();

        // Sort by access time (oldest first)
        chunks_with_time.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Calculate how many chunks to unload
        let to_unload = world_state.active_chunks.len() - world_config.max_active_chunks;

        // Unload the oldest chunks
        for (i, (coord, _)) in chunks_with_time.iter().enumerate() {
            if i >= to_unload {
                break;
            }

            if let Some(entity) = world_state.chunks.remove(coord) {
                commands.entity(entity).despawn();
                world_state.active_chunks.remove(coord);
                world_state.generation_time.remove(coord);
                debug!("Unloaded chunk at {:?}", coord);
            }
        }
    }
}

// Event for requesting chunk generation or loading
#[derive(Event)]
pub struct ChunkRequestEvent {
    pub coord: ChunkCoord,
    pub client_id: Option<ClientId>,
}

// Generate a single chunk at the given coordinates
fn generate_chunk(
    coord: &ChunkCoord,
    commands: &mut Commands,
    world_state: &mut WorldState,
    config: &WorldConfig,
) {
    let start_time = std::time::Instant::now();

    // Create noise generators with the world seed
    let perlin = Perlin::new(config.seed);
    let biome_noise = Perlin::new(config.seed + 1);
    let resource_noise = Perlin::new(config.seed + 2);

    // Determine dominant biome for this chunk
    let biome_value = biome_noise.get([
        coord.x as f64 * config.biome_scale,
        coord.y as f64 * config.biome_scale,
    ]);

    let biome_type = determine_biome(biome_value);

    // Generate the tiles for this chunk
    let mut tiles = vec![vec![create_empty_tile(); config.chunk_size]; config.chunk_size];

    for local_y in 0..config.chunk_size {
        for local_x in 0..config.chunk_size {
            // Calculate world coordinates
            let world_x = coord.x * config.chunk_size as i32 + local_x as i32;
            let world_y = coord.y * config.chunk_size as i32 + local_y as i32;

            // Get height value for this tile
            let height_value = perlin.get([
                world_x as f64 * config.height_scale,
                world_y as f64 * config.height_scale,
            ]) as f32;

            // Determine tile type based on biome and height
            let tile_type = determine_tile_type(biome_type, height_value);

            // Determine if there's a resource here
            let resource_value = resource_noise.get([
                world_x as f64 * config.height_scale * 2.0,
                world_y as f64 * config.height_scale * 2.0,
            ]) as f32;

            let resource = determine_resource(tile_type, resource_value, config.resource_density);

            // Create the tile
            tiles[local_y][local_x] = Tile {
                tile_type,
                resource,
                height: height_value,
                position: (world_x, world_y),
                traversable: is_traversable(tile_type, resource),
            };
        }
    }

    // Create the chunk entity
    let chunk = Chunk {
        coord: *coord,
        tiles,
        biome_type,
        last_accessed: world_state.world_time,
    };

    // Spawn the chunk entity
    let chunk_entity = commands.spawn(chunk).id();

    // Update world state
    world_state.chunks.insert(*coord, chunk_entity);
    world_state.active_chunks.insert(*coord);
    world_state
        .generation_time
        .insert(*coord, world_state.world_time);

    let generation_time = start_time.elapsed().as_millis();
    debug!("Generated chunk at {:?} in {}ms", coord, generation_time);
}

// Helper functions for world generation

fn create_empty_tile() -> Tile {
    Tile {
        tile_type: TileType::Grass,
        resource: ResourceType::None,
        height: 0.0,
        position: (0, 0),
        traversable: true,
    }
}

fn determine_biome(value: f64) -> BiomeType {
    match value {
        v if v < -0.6 => BiomeType::Ocean,
        v if v < -0.3 => BiomeType::Desert,
        v if v < 0.1 => BiomeType::Plains,
        v if v < 0.4 => BiomeType::Forest,
        v if v < 0.7 => BiomeType::Mountain,
        _ => BiomeType::Tundra,
    }
}

fn determine_tile_type(biome: BiomeType, height: f32) -> TileType {
    match biome {
        BiomeType::Ocean => {
            if height > 0.2 {
                TileType::Sand
            } else {
                TileType::Water
            }
        }
        BiomeType::Desert => {
            if height > 0.6 {
                TileType::Stone
            } else {
                TileType::Sand
            }
        }
        BiomeType::Plains => {
            if height > 0.7 {
                TileType::Stone
            } else {
                TileType::Grass
            }
        }
        BiomeType::Forest => {
            if height > 0.8 {
                TileType::Mountain
            } else {
                TileType::Forest
            }
        }
        BiomeType::Mountain => {
            if height > 0.6 {
                TileType::Mountain
            } else if height > 0.3 {
                TileType::Stone
            } else {
                TileType::Grass
            }
        }
        BiomeType::Tundra => {
            if height > 0.7 {
                TileType::Snow
            } else if height > 0.4 {
                TileType::Stone
            } else {
                TileType::Grass
            }
        }
    }
}

fn determine_resource(tile_type: TileType, resource_value: f32, density: f32) -> ResourceType {
    // Return None if below resource density threshold
    if resource_value.abs() < 1.0 - density {
        return ResourceType::None;
    }

    // Assign resources based on tile type
    match tile_type {
        TileType::Grass => {
            if resource_value > 0.8 {
                ResourceType::Tree
            } else {
                ResourceType::None
            }
        }
        TileType::Forest => ResourceType::Tree,
        TileType::Stone | TileType::Mountain => {
            let value = resource_value.abs();
            if value > 0.9 {
                ResourceType::Gold
            } else if value > 0.7 {
                ResourceType::Iron
            } else if value > 0.5 {
                ResourceType::Copper
            } else if value > 0.3 {
                ResourceType::Coal
            } else {
                ResourceType::Stone
            }
        }
        _ => ResourceType::None,
    }
}

fn is_traversable(tile_type: TileType, resource: ResourceType) -> bool {
    match (tile_type, resource) {
        (TileType::Water, _) => false,
        (TileType::Mountain, _) => false,
        (_, ResourceType::Tree) => false,
        _ => true,
    }
}

// System to serialize a chunk for network transmission
pub fn serialize_chunk(chunk: &Chunk) -> Vec<u8> {
    bincode::serialize(chunk).unwrap_or_else(|_| {
        error!("Failed to serialize chunk at {:?}", chunk.coord);
        Vec::new()
    })
}

// System to deserialize a chunk from network data
pub fn deserialize_chunk(data: &[u8]) -> Option<Chunk> {
    bincode::deserialize(data).ok()
}
