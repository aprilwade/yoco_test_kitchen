use std::time::Duration;

use bevy::{prelude::*, time::Stopwatch, window::PresentMode};
use bevy_tweening::{
    lens::TransformPositionLens, Animator, EaseMethod, Tween, TweenCompleted, TweeningPlugin,
};
use rand::prelude::*;

const PIECE_WIDTH: f32 = 64.0;
const PIECE_HEIGHT: f32 = 64.0;

const FPS: f32 = 60.0;
const FRAME_TIME: f32 = 1.0 / FPS;

const PIECE_SLICE_DURATION: f32 = FRAME_TIME * 5.0;

const PIECE_SLIDE_COMPLETED: u64 = 1;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cookie Test Kitchen".into(),
                resolution: (640., 480.).into(),
                present_mode: PresentMode::AutoVsync,
                // Tells wasm to resize the window according to the available canvas
                fit_canvas_to_parent: true,
                // Tells wasm not to override default event handling, like F5, Ctrl+R etc.
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugin(TweeningPlugin)
        .add_startup_system(setup)
        .add_systems((update_input, move_player_cursor, maybe_reset_board).chain())
        .add_system(update_complete_count)
        .add_system(randomly_fill_board)
        .run();
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
enum Piece {
    Mascot,
    Checkered,
    Donut,
    Flower,
    Green,
    Heart,
}

impl Piece {
    fn all_pieces() -> &'static [Piece] {
        &[
            Piece::Mascot,
            Piece::Checkered,
            Piece::Donut,
            Piece::Flower,
            Piece::Green,
            Piece::Heart,
        ]
    }

    fn texture_index(self) -> usize {
        match self {
            Piece::Mascot => 0,
            Piece::Checkered => 1,
            Piece::Donut => 2,
            Piece::Flower => 3,
            Piece::Green => 4,
            Piece::Heart => 5,
        }
    }
}

#[derive(Clone, Debug, Resource)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
struct PiecesSpriteSheet(Handle<TextureAtlas>);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
struct PieceState {
    piece: Option<Piece>,
    entity: Entity,
}

fn board_has_clear(board: &[[Piece; 5]; 5]) -> bool {
    for y in 0..5 {
        let p0 = board[y][0];
        if board[y].iter().all(|pn| p0 == *pn) {
            return true;
        }
    }
    for x in 0..5 {
        let p0 = board[0][x];
        if (0..5).all(|y| p0 == board[y][x]) {
            return true;
        }
    }
    false
}

#[derive(Resource, Debug)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
struct BoardState {
    piece_state: [[PieceState; 5]; 5],
    texture_atlas_handle: Handle<TextureAtlas>,

    // Used for sliding pieces
    extra_entity: Entity,
}

impl BoardState {
    fn empty(commands: &mut Commands, texture_atlas_handle: Handle<TextureAtlas>) -> Self {
        let mut piece_state = [[PieceState {
            piece: None,
            entity: Entity::PLACEHOLDER,
        }; 5]; 5];
        for y in 0..5 {
            for x in 0..5 {
                let world_pos = piece_location_to_world_coords(x as i8, y as i8);
                piece_state[y][x].entity = commands
                    .spawn((
                        BoardLocation {
                            x: x as u8,
                            y: y as u8,
                        },
                        SpriteSheetBundle {
                            texture_atlas: texture_atlas_handle.clone(),
                            sprite: TextureAtlasSprite::new(0),
                            transform: Transform::from_xyz(world_pos.x, world_pos.y, 0.0),
                            ..default()
                        },
                        Animator::new(Tween::new(
                            EaseMethod::Linear,
                            Duration::from_secs(1),
                            TransformPositionLens {
                                start: world_pos.extend(0.0),
                                end: world_pos.extend(0.0),
                            },
                        )),
                    ))
                    .id();
            }
        }

        let extra_world_pos = piece_location_to_world_coords(5, 5);
        BoardState {
            piece_state,
            extra_entity: commands
                .spawn((
                    SpriteSheetBundle {
                        texture_atlas: texture_atlas_handle.clone(),
                        sprite: TextureAtlasSprite::new(0),
                        transform: Transform::from_xyz(extra_world_pos.x, extra_world_pos.y, 0.0),
                        ..default()
                    },
                    Animator::new(Tween::new(
                        EaseMethod::Linear,
                        Duration::from_secs(1),
                        TransformPositionLens {
                            start: extra_world_pos.extend(0.0),
                            end: extra_world_pos.extend(0.0),
                        },
                    )),
                ))
                .id(),

            texture_atlas_handle,
        }
    }

    fn has_empty(&self) -> bool {
        self.piece_state
            .iter()
            .flat_map(|row| row)
            .any(|ps| ps.piece.is_none())
    }

    fn count_clears(&self) -> u8 {
        let mut cnt = 0;
        let mut ignored_rows = 0;
        let mut ignored_cols = 0;

        loop {
            let prev_cnt = cnt;
            for nrow in 0..5 {
                if ignored_rows & (1 << nrow) != 0 {
                    continue;
                }
                let all_eq = (0..5)
                    .filter(|ncol| ignored_cols & (1 << ncol) == 0)
                    .map(|ncol| self.piece_state[nrow][ncol].piece.unwrap())
                    .all_equal();
                if all_eq {
                    cnt += 1;
                    ignored_rows |= 1 << nrow;
                }
            }
            for ncol in 0..5 {
                if ignored_cols & (1 << ncol) != 0 {
                    continue;
                }
                let all_eq = (0..5)
                    .filter(|nrow| ignored_rows & (1 << nrow) == 0)
                    .map(|nrow| self.piece_state[nrow][ncol].piece.unwrap())
                    .all_equal();
                if all_eq {
                    cnt += 1;
                    ignored_cols |= 1 << ncol;
                }
            }

            // We're faking a do-while here
            if prev_cnt == cnt || ignored_rows == 0b1111 || ignored_cols == 0b1111 {
                return cnt;
            }
        }
    }
}

fn piece_location_to_world_coords(x: i8, y: i8) -> Vec2 {
    let x = 64.0 * (x - 2) as f32;
    let y = 64.0 * (y - 2) as f32;
    Vec2::new(x, y)
}

#[derive(Copy, Clone, Debug, Component)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
struct BoardLocation {
    x: u8,
    y: u8,
}

#[derive(Copy, Clone, Debug, Component)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
struct PlayerCursor;

#[derive(Copy, Clone, Debug, Component)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
struct PieceMarker;

// We keep track of the previous input. If the last input happened too long ago, ignore it

#[derive(Resource, Debug, Default)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
struct PreviousInput {
    elapsed: Stopwatch,
    direction: Option<Direction>,
    shift_held: bool,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    clear_color: Res<ClearColor>,
) {
    // TODO: Force the camera to a fixed resolution?
    commands.spawn(Camera2dBundle::default());

    let texture_handle = asset_server.load("sprite sheet.png");
    let atlas = TextureAtlas::from_grid(
        texture_handle,
        Vec2::new(PIECE_WIDTH, PIECE_HEIGHT),
        6,
        1,
        None,
        None,
    );
    let atlas_handle = texture_atlases.add(atlas);

    let texture_handle = asset_server.load("cursor.png");
    commands.spawn((
        SpriteBundle {
            texture: texture_handle,
            ..default()
        },
        BoardLocation { x: 2, y: 2 },
        PlayerCursor,
    ));

    let board_state = BoardState::empty(&mut commands, atlas_handle);
    commands.insert_resource(board_state);

    commands.insert_resource(PreviousInput::default());

    // Top border
    commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: clear_color.0,
            custom_size: Some(Vec2::new(PIECE_WIDTH * 7.0, PIECE_WIDTH * 3.0)),
            ..default()
        },
        transform: Transform::from_xyz(0.0, PIECE_WIDTH * 4.0, 1.0),
        ..default()
    });
    // Bottom border
    commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: clear_color.0,
            custom_size: Some(Vec2::new(PIECE_WIDTH * 7.0, PIECE_WIDTH * 3.0)),
            ..default()
        },
        transform: Transform::from_xyz(0.0, -PIECE_WIDTH * 4.0, 1.0),
        ..default()
    });
    // Right border
    commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: clear_color.0,
            custom_size: Some(Vec2::new(PIECE_WIDTH * 3.0, PIECE_WIDTH * 7.0)),
            ..default()
        },
        transform: Transform::from_xyz(PIECE_HEIGHT * 4.0, 0.0, 1.0),
        ..default()
    });
    // Left border
    commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: clear_color.0,
            custom_size: Some(Vec2::new(PIECE_WIDTH * 3.0, PIECE_WIDTH * 7.0)),
            ..default()
        },
        transform: Transform::from_xyz(-PIECE_HEIGHT * 4.0, 0.0, 1.0),
        ..default()
    });

    commands.spawn((
        TextBundle::from_sections([
            TextSection::new(
                " Number of clears: ",
                TextStyle {
                    font: asset_server.load("FiraSans-Bold.ttf"),
                    font_size: 30.0,
                    color: Color::WHITE,
                },
            ),
            TextSection::new(
                "0",
                TextStyle {
                    font: asset_server.load("FiraSans-Bold.ttf"),
                    font_size: 30.0,
                    color: Color::WHITE,
                },
            ),
        ]),
        ClearCountText,
    ));
}

#[derive(Copy, Clone, Debug, Component)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
struct ClearCountText;

#[derive(Copy, Clone, Debug)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Reflect, FromReflect))]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

fn move_player_cursor(
    mut player_query: Query<(&mut BoardLocation, &mut Transform), With<PlayerCursor>>,
    mut piece_query: Query<
        (
            &mut Transform,
            &mut TextureAtlasSprite,
            &mut Animator<Transform>,
        ),
        Without<PlayerCursor>,
    >,
    mut prev_input: ResMut<PreviousInput>,
    mut board_state: ResMut<BoardState>,
) {
    // While animations are playing, don't act on input.
    let anim_in_progress = piece_query
        .iter()
        .any(|(_, _, anim)| anim.tweenable().progress() < 1.0);
    if anim_in_progress || board_state.has_empty() {
        return;
    }

    let prev_input = &mut *prev_input;
    if let Some(direction) = prev_input.direction.take() {
        if prev_input.elapsed.elapsed_secs() > FRAME_TIME * 3.0 {
            return;
        }

        let (mut board_location, mut transform) = player_query.single_mut();

        if prev_input.shift_held {
            // We need to move the pieces to their new location
            let (indices, offset_x, offset_y) = match direction {
                Direction::Up => {
                    let idx = board_location.x as usize;
                    let col = [(idx, 0), (idx, 1), (idx, 2), (idx, 3), (idx, 4)];
                    (col, 0, 1)
                }
                Direction::Down => {
                    let idx = board_location.x as usize;
                    let col = [(idx, 0), (idx, 1), (idx, 2), (idx, 3), (idx, 4)];
                    (col, 0, 4)
                }
                Direction::Left => {
                    let idx = board_location.y as usize;
                    let row = [(0, idx), (1, idx), (2, idx), (3, idx), (4, idx)];
                    (row, 4, 0)
                }
                Direction::Right => {
                    let idx = board_location.y as usize;
                    let row = [(0, idx), (1, idx), (2, idx), (3, idx), (4, idx)];
                    (row, 1, 0)
                }
            };
            let mut piece_types = [
                board_state.piece_state[indices[0].1][indices[0].0]
                    .piece
                    .unwrap(),
                board_state.piece_state[indices[1].1][indices[1].0]
                    .piece
                    .unwrap(),
                board_state.piece_state[indices[2].1][indices[2].0]
                    .piece
                    .unwrap(),
                board_state.piece_state[indices[3].1][indices[3].0]
                    .piece
                    .unwrap(),
                board_state.piece_state[indices[4].1][indices[4].0]
                    .piece
                    .unwrap(),
            ];
            piece_types.rotate_right(std::cmp::max(offset_x, offset_y));

            let offset_x = match offset_x {
                0 => 0,
                4 => 1,
                _ => -1,
            };
            let offset_y = match offset_y {
                0 => 0,
                4 => 1,
                _ => -1,
            };

            for ((x_idx, y_idx), piece_type) in indices.iter().zip(piece_types) {
                let piece_state = &mut board_state.piece_state[*y_idx][*x_idx];
                piece_state.piece = Some(piece_type);
                let (mut transform, mut sprite, mut animator) =
                    piece_query.get_mut(piece_state.entity).unwrap();
                sprite.index = piece_type.texture_index();

                let start_pos = piece_location_to_world_coords(
                    *x_idx as i8 + offset_x,
                    *y_idx as i8 + offset_y,
                )
                .extend(0.0);
                let end_pos =
                    piece_location_to_world_coords(*x_idx as i8, *y_idx as i8).extend(0.0);

                // Start the animation for the piece moving
                transform.translation = start_pos;
                animator.set_tweenable(Tween::new(
                    EaseMethod::Linear,
                    Duration::from_secs_f32(PIECE_SLICE_DURATION),
                    TransformPositionLens {
                        start: start_pos,
                        end: end_pos,
                    },
                ));
            }

            // Set up the extra piece entity so a piece appears to slice off the end
            let (mut transform, mut sprite, mut animator) =
                piece_query.get_mut(board_state.extra_entity).unwrap();

            let last_index = if offset_x != 0 { offset_x } else { offset_y };
            let last_index = if last_index == -1 { 4 } else { 0 };

            // We already rotated, so we have to do this little bit of math instead of using
            // the index directly.
            sprite.index = piece_types[4 - last_index].texture_index();

            let (x_idx, y_idx) = indices[last_index];
            let start_pos = piece_location_to_world_coords(x_idx as i8, y_idx as i8).extend(0.0);
            let end_pos =
                piece_location_to_world_coords(x_idx as i8 - offset_x, y_idx as i8 - offset_y)
                    .extend(0.0);
            transform.translation = start_pos;
            // TODO: Watch for this particular animation to finish so we can update the number of
            //       clears
            animator.set_tweenable(
                Tween::new(
                    EaseMethod::Linear,
                    Duration::from_secs_f32(PIECE_SLICE_DURATION),
                    TransformPositionLens {
                        start: start_pos,
                        end: end_pos,
                    },
                )
                .with_completed_event(PIECE_SLIDE_COMPLETED),
            );

            return;
        }

        match direction {
            Direction::Up => board_location.y = (board_location.y + 1) % 5,
            Direction::Down => board_location.y = (board_location.y + 4) % 5,
            Direction::Left => board_location.x = (board_location.x + 4) % 5,
            Direction::Right => board_location.x = (board_location.x + 1) % 5,
        }
        let world_pos =
            piece_location_to_world_coords(board_location.x as i8, board_location.y as i8);
        transform.translation.x = world_pos.x;
        transform.translation.y = world_pos.y;
    }
}

fn update_input(mut prev_input: ResMut<PreviousInput>, time: Res<Time>, keys: Res<Input<KeyCode>>) {
    let direction_pressed = if keys.just_pressed(KeyCode::E) || keys.just_pressed(KeyCode::Up) {
        Direction::Up
    } else if keys.just_pressed(KeyCode::D) || keys.just_pressed(KeyCode::Down) {
        Direction::Down
    } else if keys.just_pressed(KeyCode::S) || keys.just_pressed(KeyCode::Left) {
        Direction::Left
    } else if keys.just_pressed(KeyCode::F) || keys.just_pressed(KeyCode::Right) {
        Direction::Right
    } else {
        if prev_input.direction.is_some() {
            prev_input.elapsed.tick(time.delta());
        }
        return;
    };

    prev_input.elapsed.reset();
    prev_input.direction = Some(direction_pressed);
    prev_input.shift_held = keys.pressed(KeyCode::LShift) || keys.pressed(KeyCode::RShift);
}

fn maybe_reset_board(keys: Res<Input<KeyCode>>, mut board_state: ResMut<BoardState>) {
    if keys.just_pressed(KeyCode::Space) {
        for piece_state_row in board_state.piece_state.iter_mut() {
            for piece_state in piece_state_row.iter_mut() {
                piece_state.piece = None;
            }
        }
    }
}

fn update_complete_count(
    mut reader: EventReader<TweenCompleted>,
    mut query: Query<&mut Text, With<ClearCountText>>,
    board_state: Res<BoardState>,
) {
    for event in reader.iter() {
        if event.user_data == PIECE_SLIDE_COMPLETED {
            let mut text = query.single_mut();
            text.sections[1].value = format!("{}", board_state.count_clears());
        }
    }
}

fn randomly_fill_board(
    mut board_state: ResMut<BoardState>,
    mut query: Query<&mut TextureAtlasSprite>,
) {
    // Only attempt to fill in empty spaces if some actually exist
    if !board_state.has_empty() {
        return;
    }

    let mut rng = rand::thread_rng();
    let starting_board = board_state.piece_state.clone();
    let filled_board = loop {
        let mut filled_board = [[Piece::Mascot; 5]; 5];
        for (y, row) in starting_board.iter().enumerate() {
            for (x, piece_state) in row.iter().enumerate() {
                filled_board[y][x] = if let Some(piece) = piece_state.piece {
                    piece
                } else {
                    *Piece::all_pieces().choose(&mut rng).unwrap()
                };
            }
        }
        if !board_has_clear(&filled_board) {
            break filled_board;
        }
    };
    for (state_row, board_row) in board_state.piece_state.iter_mut().zip(filled_board) {
        for (piece_state, piece) in state_row.iter_mut().zip(board_row) {
            if piece_state.piece.is_some() {
                continue;
            }

            piece_state.piece = Some(piece);
            let mut sprite = query.get_mut(piece_state.entity).unwrap();
            sprite.index = piece.texture_index();
        }
    }
}

trait IteratorExt: Iterator {
    fn all_equal(&mut self) -> bool
    where
        Self: Sized,
        Self::Item: PartialEq,
    {
        match self.next() {
            None => true,
            Some(a) => self.all(|x| a == x),
        }
    }
}
impl<I: Iterator> IteratorExt for I {}
