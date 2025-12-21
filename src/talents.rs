use bevy::prelude::*;

use crate::assets::MyStates;

pub struct TalentsPlugin;

impl Plugin for TalentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TalentUiState>()
            .init_resource::<TalentPoints>()
            .init_resource::<TalentsState>()
            .init_resource::<TalentBonuses>()
            .init_resource::<TalentUiSelection>()
            .add_systems(OnEnter(MyStates::Next), spawn_talents_ui)
            .add_systems(
                Update,
                (
                    toggle_talents_ui,
                    talent_ui_button_interactions,
                    update_talent_buttons_visuals,
                    update_details_panel,
                    recompute_bonuses,
                )
                    .run_if(in_state(MyStates::Next)),
            );
    }
}

// --- Data model -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TalentTree {
    Vigor,
    Guile,
    Sorcery,
}

impl TalentTree {
    pub const ALL: [TalentTree; 3] = [TalentTree::Vigor, TalentTree::Guile, TalentTree::Sorcery];

    pub fn title(self) -> &'static str {
        match self {
            TalentTree::Vigor => "Vigor",
            TalentTree::Guile => "Guile",
            TalentTree::Sorcery => "Sorcery",
        }
    }
}

/// 0-based tier index (tier 0 == "Row 1" in WoW UI).
pub type Tier = u8;

/// Slot within a tier for a given tree (0..=1 currently).
pub type Slot = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TalentId {
    pub tree: TalentTree,
    pub tier: Tier,
    pub slot: Slot,
}

#[derive(Debug, Clone, Copy)]
pub enum TalentEffect {
    /// +% move speed per rank
    MoveSpeedPctPerRank(f32),
    /// +% sprint multiplier per rank (applied on top of the base sprint factor)
    SprintPctPerRank(f32),
    /// +% jump height per rank
    JumpHeightPctPerRank(f32),
    /// -% extra fall gravity per rank (1.0 means unchanged, lower means floatier)
    FallExtraGravityPctPerRank(f32),
    /// Placeholder (no runtime effect yet)
    Placeholder,
}

#[derive(Debug, Clone, Copy)]
pub struct TalentDef {
    pub id: TalentId,
    pub name: &'static str,
    pub max_rank: u8,
    pub description: &'static str,
    /// Tier requirement (points in this tree) is derived from `tier`, like classic WoW.
    pub prereq: Option<TalentId>,
    pub effect: TalentEffect,
}

pub const TIERS_PER_TREE: u8 = 8;
pub const SLOTS_PER_TIER: u8 = 2;

/// Classic-style tier requirements: tier 0 => 0 points, tier 1 => 5 points, ..., tier 7 => 35.
pub fn required_points_for_tier(tier: Tier) -> u8 {
    tier.saturating_mul(5)
}

/// A “level 60” style placeholder budget so you can actually play with the tree right now.
#[derive(Resource, Debug, Clone, Copy)]
pub struct TalentPoints {
    pub available: u32,
}

impl Default for TalentPoints {
    fn default() -> Self {
        Self { available: 51 }
    }
}

#[derive(Resource, Debug, Default)]
pub struct TalentsState {
    ranks: std::collections::HashMap<TalentId, u8>,
    // For quick “undo”/refund behavior
    spent_stack: Vec<TalentId>,
}

impl TalentsState {
    pub fn rank(&self, id: TalentId) -> u8 {
        self.ranks.get(&id).copied().unwrap_or(0)
    }

    pub fn set_rank(&mut self, id: TalentId, rank: u8) {
        if rank == 0 {
            self.ranks.remove(&id);
        } else {
            self.ranks.insert(id, rank);
        }
    }

    pub fn points_spent_in_tree(&self, tree: TalentTree) -> u32 {
        self.ranks
            .iter()
            .filter(|(id, _)| id.tree == tree)
            .map(|(_, r)| *r as u32)
            .sum()
    }

    pub fn total_points_spent(&self) -> u32 {
        self.ranks.values().map(|r| *r as u32).sum()
    }
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TalentBonuses {
    pub move_speed_mult: f32,
    pub sprint_mult: f32,
    pub jump_height_mult: f32,
    pub fall_extra_gravity_mult: f32,
}

// --- UI state ---------------------------------------------------------------

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TalentUiState {
    pub open: bool,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TalentUiSelection {
    pub hovered: Option<TalentId>,
}

#[derive(Component)]
struct TalentUiRoot;

#[derive(Component)]
struct TalentButton {
    id: TalentId,
}

#[derive(Component)]
struct TalentRankText {
    id: TalentId,
}

#[derive(Component)]
struct TalentDetailsName;

#[derive(Component)]
struct TalentDetailsBody;

#[derive(Component)]
struct TalentPointsText;

#[derive(Component)]
struct ResetTalentsButton;

#[derive(Component)]
struct RefundLastButton;

// --- Talent definitions -----------------------------------------------------

pub const TALENTS: &[TalentDef] = &[
    // VIGOR (melee + movement)
    t(
        TalentTree::Vigor,
        0,
        0,
        "Fleet Footing",
        5,
        "+2% movement speed per rank.",
        None,
        TalentEffect::MoveSpeedPctPerRank(2.0),
    ),
    t(
        TalentTree::Vigor,
        0,
        1,
        "Firm Stance",
        5,
        "Placeholder: +1% resistance to knockback per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        1,
        0,
        "Longstrider",
        3,
        "+3% sprint effectiveness per rank.",
        None,
        TalentEffect::SprintPctPerRank(3.0),
    ),
    t(
        TalentTree::Vigor,
        1,
        1,
        "Hardened Soles",
        2,
        "Placeholder: -5% fall damage per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        2,
        0,
        "Spring Heels",
        5,
        "+3% jump height per rank.",
        None,
        TalentEffect::JumpHeightPctPerRank(3.0),
    ),
    t(
        TalentTree::Vigor,
        2,
        1,
        "Oaken Bones",
        3,
        "Placeholder: +2% max health per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        3,
        0,
        "Road-Worn Breath",
        2,
        "Placeholder: +5% stamina regen per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        3,
        1,
        "Sure Landing",
        3,
        "Slightly floatier falls: -4% fall extra gravity per rank.",
        None,
        TalentEffect::FallExtraGravityPctPerRank(4.0),
    ),
    t(
        TalentTree::Vigor,
        4,
        0,
        "Brutal Timing",
        3,
        "Placeholder: +1% crit chance per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        4,
        1,
        "Iron Rhythm",
        2,
        "Placeholder: +3% attack speed per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        5,
        0,
        "Giant's Step",
        1,
        "Requires Fleet Footing. +5% movement speed.",
        Some(TalentId {
            tree: TalentTree::Vigor,
            tier: 0,
            slot: 0,
        }),
        TalentEffect::MoveSpeedPctPerRank(5.0),
    ),
    t(
        TalentTree::Vigor,
        5,
        1,
        "Stoneheart",
        1,
        "Placeholder: +10% armor.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        6,
        0,
        "Relentless Pursuit",
        2,
        "Placeholder: after sprinting, keep +5% speed for 2s.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        6,
        1,
        "Hoplite's Leap",
        1,
        "Requires Spring Heels. +10% jump height.",
        Some(TalentId {
            tree: TalentTree::Vigor,
            tier: 2,
            slot: 0,
        }),
        TalentEffect::JumpHeightPctPerRank(10.0),
    ),
    t(
        TalentTree::Vigor,
        7,
        0,
        "Veteran's Gait",
        1,
        "Placeholder: +15% out-of-combat speed.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        7,
        1,
        "Unbroken",
        1,
        "Placeholder: +1 free death save.",
        None,
        TalentEffect::Placeholder,
    ),
    // GUILE (control + tricks)
    t(
        TalentTree::Guile,
        0,
        0,
        "Lightstep",
        5,
        "+1% movement speed per rank.",
        None,
        TalentEffect::MoveSpeedPctPerRank(1.0),
    ),
    t(
        TalentTree::Guile,
        0,
        1,
        "Quick Fingers",
        5,
        "Placeholder: +2% pickup speed per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        1,
        0,
        "Duskwalker",
        3,
        "Placeholder: +3% stealth effectiveness per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        1,
        1,
        "Dirty Tricks",
        2,
        "Placeholder: +5% stun duration per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        2,
        0,
        "Short Fuse",
        5,
        "+2% sprint effectiveness per rank.",
        None,
        TalentEffect::SprintPctPerRank(2.0),
    ),
    t(
        TalentTree::Guile,
        2,
        1,
        "Catfall",
        3,
        "Slightly floatier falls: -3% fall extra gravity per rank.",
        None,
        TalentEffect::FallExtraGravityPctPerRank(3.0),
    ),
    t(
        TalentTree::Guile,
        3,
        0,
        "Opportunist",
        3,
        "Placeholder: +1% bonus damage vs distracted enemies per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        3,
        1,
        "Fast Climb",
        2,
        "Placeholder: +6% climb speed per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        4,
        0,
        "Swift Reprisal",
        2,
        "Placeholder: after dodging, +3% speed for 2s per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        4,
        1,
        "Shadow Breath",
        2,
        "Placeholder: +5% stamina regen in darkness per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        5,
        0,
        "Prowler's Pace",
        1,
        "Requires Lightstep. +4% movement speed.",
        Some(TalentId {
            tree: TalentTree::Guile,
            tier: 0,
            slot: 0,
        }),
        TalentEffect::MoveSpeedPctPerRank(4.0),
    ),
    t(
        TalentTree::Guile,
        5,
        1,
        "Never Caught",
        1,
        "Placeholder: first hit against you misses.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        6,
        0,
        "Slipstream",
        2,
        "Placeholder: while sprinting, +3% jump height per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        6,
        1,
        "Featherfall",
        1,
        "Requires Catfall. -10% fall extra gravity.",
        Some(TalentId {
            tree: TalentTree::Guile,
            tier: 2,
            slot: 1,
        }),
        TalentEffect::FallExtraGravityPctPerRank(10.0),
    ),
    t(
        TalentTree::Guile,
        7,
        0,
        "Ghostfoot",
        1,
        "Placeholder: +20% speed for 1s after taking damage.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        7,
        1,
        "Master Thief",
        1,
        "Placeholder: can pick locked chests.",
        None,
        TalentEffect::Placeholder,
    ),
    // SORCERY (mystic mobility)
    t(
        TalentTree::Sorcery,
        0,
        0,
        "Arcane Poise",
        5,
        "Placeholder: +1% mana regen per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        0,
        1,
        "Warded Boots",
        5,
        "+1% jump height per rank.",
        None,
        TalentEffect::JumpHeightPctPerRank(1.0),
    ),
    t(
        TalentTree::Sorcery,
        1,
        0,
        "Spellrunner",
        3,
        "+2% movement speed per rank.",
        None,
        TalentEffect::MoveSpeedPctPerRank(2.0),
    ),
    t(
        TalentTree::Sorcery,
        1,
        1,
        "Soft Descent",
        2,
        "Slightly floatier falls: -5% fall extra gravity per rank.",
        None,
        TalentEffect::FallExtraGravityPctPerRank(5.0),
    ),
    t(
        TalentTree::Sorcery,
        2,
        0,
        "Flicker Step",
        5,
        "+2% sprint effectiveness per rank.",
        None,
        TalentEffect::SprintPctPerRank(2.0),
    ),
    t(
        TalentTree::Sorcery,
        2,
        1,
        "Aerial Ward",
        3,
        "Placeholder: -2% air control penalty per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        3,
        0,
        "Boundless Leap",
        3,
        "+4% jump height per rank.",
        None,
        TalentEffect::JumpHeightPctPerRank(4.0),
    ),
    t(
        TalentTree::Sorcery,
        3,
        1,
        "Leyline Stride",
        2,
        "Placeholder: +5% speed while near shrines per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        4,
        0,
        "Airwalk",
        2,
        "Placeholder: +1 mid-air jump per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        4,
        1,
        "Gravity Knot",
        2,
        "Placeholder: slow enemies on landing.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        5,
        0,
        "Blinkrunner",
        1,
        "Requires Flicker Step. +6% sprint effectiveness.",
        Some(TalentId {
            tree: TalentTree::Sorcery,
            tier: 2,
            slot: 0,
        }),
        TalentEffect::SprintPctPerRank(6.0),
    ),
    t(
        TalentTree::Sorcery,
        5,
        1,
        "Skyhook",
        1,
        "Requires Boundless Leap. +12% jump height.",
        Some(TalentId {
            tree: TalentTree::Sorcery,
            tier: 3,
            slot: 0,
        }),
        TalentEffect::JumpHeightPctPerRank(12.0),
    ),
    t(
        TalentTree::Sorcery,
        6,
        0,
        "Slip of Time",
        2,
        "Placeholder: +3% cooldown recovery per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        6,
        1,
        "Feathered Sigil",
        2,
        "Slightly floatier falls: -4% fall extra gravity per rank.",
        None,
        TalentEffect::FallExtraGravityPctPerRank(4.0),
    ),
    t(
        TalentTree::Sorcery,
        7,
        0,
        "Archmage's Stride",
        1,
        "Placeholder: +15% cast speed.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        7,
        1,
        "Starsong",
        1,
        "Placeholder: your footsteps leave stardust.",
        None,
        TalentEffect::Placeholder,
    ),
];

const fn t(
    tree: TalentTree,
    tier: Tier,
    slot: Slot,
    name: &'static str,
    max_rank: u8,
    description: &'static str,
    prereq: Option<TalentId>,
    effect: TalentEffect,
) -> TalentDef {
    TalentDef {
        id: TalentId { tree, tier, slot },
        name,
        max_rank,
        description,
        prereq,
        effect,
    }
}

fn talent_def(id: TalentId) -> Option<&'static TalentDef> {
    TALENTS.iter().find(|d| d.id == id)
}

fn talent_def_by_slot(tree: TalentTree, tier: Tier, slot: Slot) -> Option<&'static TalentDef> {
    let id = TalentId { tree, tier, slot };
    talent_def(id)
}

// --- Systems ----------------------------------------------------------------

fn toggle_talents_ui(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<TalentUiState>,
    root: Query<Entity, With<TalentUiRoot>>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::KeyT) {
        return;
    }
    ui_state.open = !ui_state.open;

    if let Some(root) = root.iter().next() {
        let vis = if ui_state.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        commands.entity(root).insert(vis);
    }
}

fn spawn_talents_ui(mut commands: Commands) {
    // Colors tuned for “medieval parchment + dark wood” vibe.
    let overlay = Color::srgba(0.02, 0.02, 0.02, 0.75);
    let parchment = Color::srgb(0.88, 0.83, 0.70);
    let wood = Color::srgb(0.22, 0.13, 0.08);
    let ink = Color::srgb(0.08, 0.05, 0.03);
    let gold = Color::srgb(0.78, 0.67, 0.30);

    let root = commands
        .spawn((
            TalentUiRoot,
            Name::new("Talents UI Root"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(overlay),
            Visibility::Hidden,
        ))
        .id();

    // Center panel
    let panel = commands
        .spawn((
            Name::new("Talents UI Panel"),
            Node {
                width: Val::Px(980.0),
                height: Val::Px(640.0),
                padding: UiRect::all(Val::Px(18.0)),
                border: UiRect::all(Val::Px(3.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(parchment),
            BorderColor::all(wood),
        ))
        .id();

    commands.entity(root).add_child(panel);

    // Header row (title + points)
    let header = commands
        .spawn((
            Name::new("Talents UI Header"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .id();

    let title = commands
        .spawn((
            Name::new("Talents Title"),
            Text::new("Talents"),
            TextFont {
                font_size: 28.0,
                ..default()
            },
            TextColor(ink),
        ))
        .id();

    let points = commands
        .spawn((
            TalentPointsText,
            Name::new("Talents Points Text"),
            Text::new("Points: 51 (spent: 0)"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(ink),
        ))
        .id();

    commands.entity(header).add_child(title);
    commands.entity(header).add_child(points);
    commands.entity(panel).add_child(header);

    // Main content row: trees (left) + details (right)
    let body = commands
        .spawn((
            Name::new("Talents Body"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                ..default()
            },
        ))
        .id();
    commands.entity(panel).add_child(body);

    // Trees area
    let trees = commands
        .spawn((
            Name::new("Talents Trees Area"),
            Node {
                width: Val::Px(700.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                ..default()
            },
        ))
        .id();
    commands.entity(body).add_child(trees);

    // Details panel
    let details = commands
        .spawn((
            Name::new("Talents Details Panel"),
            Node {
                width: Val::Px(240.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.92, 0.88, 0.76)),
            BorderColor::all(wood),
        ))
        .id();
    commands.entity(body).add_child(details);

    let details_name = commands
        .spawn((
            TalentDetailsName,
            Name::new("Talent Details Name"),
            Text::new("Hover a talent"),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(ink),
        ))
        .id();
    let details_body = commands
        .spawn((
            TalentDetailsBody,
            Name::new("Talent Details Body"),
            Text::new("Press T to open/close.\nClick to invest.\nShift+Click to refund."),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(ink),
        ))
        .id();

    let controls_row = commands
        .spawn((
            Name::new("Talents Controls Row"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(42.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .id();

    let reset = commands
        .spawn((
            ResetTalentsButton,
            Button,
            Name::new("Reset Talents Button"),
            Node {
                width: Val::Px(112.0),
                height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(wood),
            BorderColor::all(gold),
        ))
        .with_child((
            Text::new("Reset"),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.86)),
        ))
        .id();

    let refund = commands
        .spawn((
            RefundLastButton,
            Button,
            Name::new("Refund Last Button"),
            Node {
                width: Val::Px(112.0),
                height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(wood),
            BorderColor::all(gold),
        ))
        .with_child((
            Text::new("Refund 1"),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.86)),
        ))
        .id();

    commands.entity(controls_row).add_child(reset);
    commands.entity(controls_row).add_child(refund);

    commands.entity(details).add_child(details_name);
    commands.entity(details).add_child(details_body);
    commands.entity(details).add_child(controls_row);

    // Build each tree column with 8 tiers.
    for tree in TalentTree::ALL {
        let tree_col = commands
            .spawn((
                Name::new(format!("Tree: {}", tree.title())),
                Node {
                    width: Val::Px(226.0),
                    height: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.90, 0.86, 0.74)),
                BorderColor::all(wood),
            ))
            .id();
        commands.entity(trees).add_child(tree_col);

        // Tree title
        commands.entity(tree_col).with_child((
            Text::new(tree.title()),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(ink),
        ));

        for tier in 0..TIERS_PER_TREE {
            let tier_row = commands
                .spawn((
                    Name::new(format!("Tier {tier}")),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(62.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .id();
            commands.entity(tree_col).add_child(tier_row);

            for slot in 0..SLOTS_PER_TIER {
                let Some(def) = talent_def_by_slot(tree, tier, slot) else {
                    // Empty placeholder slot (keeps layout aligned if you ever remove defs)
                    commands.entity(tier_row).with_child(Node {
                        width: Val::Px(104.0),
                        height: Val::Px(56.0),
                        ..default()
                    });
                    continue;
                };

                let button = commands
                    .spawn((
                        TalentButton { id: def.id },
                        Button,
                        Name::new(format!("Talent: {}", def.name)),
                        Node {
                            width: Val::Px(104.0),
                            height: Val::Px(56.0),
                            padding: UiRect::all(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.35, 0.28, 0.18)),
                        BorderColor::all(gold),
                    ))
                    .id();

                let name = commands
                    .spawn((
                        Text::new(def.name),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.94, 0.90)),
                    ))
                    .id();

                let rank = commands
                    .spawn((
                        TalentRankText { id: def.id },
                        Text::new("0/0"),
                        TextFont {
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.94, 0.90)),
                    ))
                    .id();

                commands.entity(button).add_child(name);
                commands.entity(button).add_child(rank);
                commands.entity(tier_row).add_child(button);
            }
        }
    }
}

fn can_invest(talents: &TalentsState, points: &TalentPoints, id: TalentId) -> (bool, &'static str) {
    let Some(def) = talent_def(id) else {
        return (false, "Unknown talent");
    };

    let current = talents.rank(id);
    if current >= def.max_rank {
        return (false, "Already maxed");
    }
    if points.available == 0 {
        return (false, "No points available");
    }

    // Tier requirement: points in this tree
    let spent_in_tree = talents.points_spent_in_tree(id.tree) as u8;
    let req = required_points_for_tier(id.tier);
    if spent_in_tree < req {
        return (false, "Not enough points in this tree");
    }

    if let Some(pr) = def.prereq {
        if talents.rank(pr) == 0 {
            return (false, "Requires prerequisite talent");
        }
    }

    (true, "OK")
}

fn talent_ui_button_interactions(
    interactions: Query<(&Interaction, &TalentButton), Changed<Interaction>>,
    reset_btn: Query<&Interaction, (Changed<Interaction>, With<ResetTalentsButton>)>,
    refund_btn: Query<&Interaction, (Changed<Interaction>, With<RefundLastButton>)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut talents: ResMut<TalentsState>,
    mut points: ResMut<TalentPoints>,
    mut selection: ResMut<TalentUiSelection>,
) {
    // Hover tracking (for details panel)
    for (interaction, btn) in interactions.iter() {
        match *interaction {
            Interaction::Hovered => {
                selection.hovered = Some(btn.id);
            }
            Interaction::None => {
                if selection.hovered == Some(btn.id) {
                    selection.hovered = None;
                }
            }
            Interaction::Pressed => {
                let shift_refund = keyboard.pressed(KeyCode::ShiftLeft)
                    || keyboard.pressed(KeyCode::ShiftRight);

                if shift_refund {
                    let current = talents.rank(btn.id);
                    if current > 0 {
                        talents.set_rank(btn.id, current - 1);
                        points.available = points.available.saturating_add(1);
                    }
                } else {
                    let (ok, _reason) = can_invest(&talents, &points, btn.id);
                    if ok {
                        let current = talents.rank(btn.id);
                        talents.set_rank(btn.id, current + 1);
                        points.available = points.available.saturating_sub(1);
                        talents.spent_stack.push(btn.id);
                    }
                }
            }
        }
    }

    if let Some(interaction) = reset_btn.iter().next() {
        if *interaction == Interaction::Pressed {
            talents.ranks.clear();
            talents.spent_stack.clear();
            points.available = 51;
        }
    }

    if let Some(interaction) = refund_btn.iter().next() {
        if *interaction == Interaction::Pressed {
            if let Some(last) = talents.spent_stack.pop() {
                let current = talents.rank(last);
                if current > 0 {
                    talents.set_rank(last, current - 1);
                    points.available = points.available.saturating_add(1);
                }
            }
        }
    }
}

fn update_talent_buttons_visuals(
    talents: Res<TalentsState>,
    points: Res<TalentPoints>,
    mut points_text: Query<&mut Text, With<TalentPointsText>>,
    mut buttons: Query<(&TalentButton, &mut BackgroundColor, &mut BorderColor)>,
    mut rank_texts: Query<(&TalentRankText, &mut Text)>,
) {
    let spent = talents.total_points_spent();
    if let Ok(mut t) = points_text.single_mut() {
        *t = Text::new(format!("Points: {} (spent: {})", points.available, spent));
    }

    for (btn, mut bg, mut border) in buttons.iter_mut() {
        let Some(def) = talent_def(btn.id) else {
            continue;
        };
        let rank = talents.rank(btn.id);
        let (ok, _reason) = can_invest(&talents, &points, btn.id);

        // Locked/available/maxed coloring
        if rank >= def.max_rank {
            *bg = BackgroundColor(Color::srgb(0.24, 0.30, 0.20)); // maxed: greenish
            *border = BorderColor::all(Color::srgb(0.70, 0.88, 0.55));
        } else if ok {
            *bg = BackgroundColor(Color::srgb(0.36, 0.28, 0.16)); // available: warm
            *border = BorderColor::all(Color::srgb(0.86, 0.76, 0.38));
        } else if rank > 0 {
            *bg = BackgroundColor(Color::srgb(0.30, 0.26, 0.18)); // invested but currently gated
            *border = BorderColor::all(Color::srgb(0.80, 0.70, 0.35));
        } else {
            *bg = BackgroundColor(Color::srgb(0.20, 0.18, 0.14)); // locked: dark
            *border = BorderColor::all(Color::srgb(0.45, 0.38, 0.20));
        }
    }

    for (rt, mut text) in rank_texts.iter_mut() {
        let Some(def) = talent_def(rt.id) else {
            continue;
        };
        let rank = talents.rank(rt.id);
        *text = Text::new(format!("{rank}/{max}", max = def.max_rank));
    }
}

fn update_details_panel(
    selection: Res<TalentUiSelection>,
    talents: Res<TalentsState>,
    points: Res<TalentPoints>,
    mut name: Query<&mut Text, With<TalentDetailsName>>,
    mut body: Query<&mut Text, With<TalentDetailsBody>>,
) {
    let Some(id) = selection.hovered else {
        return;
    };
    let Some(def) = talent_def(id) else {
        return;
    };

    if let Ok(mut n) = name.single_mut() {
        *n = Text::new(def.name);
    }

    let rank = talents.rank(id);
    let spent_in_tree = talents.points_spent_in_tree(id.tree);
    let tier_req = required_points_for_tier(id.tier);
    let (ok, reason) = can_invest(&talents, &points, id);

    let prereq_line = def.prereq.map_or(String::new(), |pr| {
        let pr_name = talent_def(pr).map(|d| d.name).unwrap_or("Unknown");
        format!("Prereq: {pr_name}\n")
    });

    let status = if ok { "Available" } else { reason };

    if let Ok(mut b) = body.single_mut() {
        *b = Text::new(format!(
            "{tree} — Row {row}\nRank: {rank}/{max}\nSpent in tree: {spent}/{req}+ (to unlock row)\n{prereq}{desc}\n\nStatus: {status}\n\nTips:\n- Click: invest\n- Shift+Click: refund",
            tree = def.id.tree.title(),
            row = def.id.tier + 1,
            max = def.max_rank,
            spent = spent_in_tree,
            req = tier_req,
            prereq = prereq_line,
            desc = def.description,
            status = status
        ));
    }
}

fn recompute_bonuses(
    talents: Res<TalentsState>,
    mut bonuses: ResMut<TalentBonuses>,
) {
    if !talents.is_changed() {
        return;
    }

    let mut out = TalentBonuses {
        move_speed_mult: 1.0,
        sprint_mult: 1.0,
        jump_height_mult: 1.0,
        fall_extra_gravity_mult: 1.0,
    };

    for def in TALENTS.iter() {
        let rank = talents.rank(def.id) as f32;
        if rank <= 0.0 {
            continue;
        }
        match def.effect {
            TalentEffect::MoveSpeedPctPerRank(p) => {
                out.move_speed_mult *= 1.0 + (p / 100.0) * rank;
            }
            TalentEffect::SprintPctPerRank(p) => {
                out.sprint_mult *= 1.0 + (p / 100.0) * rank;
            }
            TalentEffect::JumpHeightPctPerRank(p) => {
                out.jump_height_mult *= 1.0 + (p / 100.0) * rank;
            }
            TalentEffect::FallExtraGravityPctPerRank(p) => {
                out.fall_extra_gravity_mult *= 1.0 - (p / 100.0) * rank;
            }
            TalentEffect::Placeholder => {}
        }
    }

    // Clamp to sane bounds (avoid negative/zero gravity multipliers from stacking).
    out.fall_extra_gravity_mult = out.fall_extra_gravity_mult.clamp(0.35, 1.0);

    *bonuses = out;
}


