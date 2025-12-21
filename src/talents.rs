use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use strum_macros::Display;

use crate::assets::MyStates;

pub struct TalentsPlugin;

impl Plugin for TalentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TalentUiState>()
            .init_resource::<SelectedTalentClass>()
            .init_resource::<ClassSelectUiState>()
            .init_resource::<EscapeMenuUiState>()
            .init_resource::<TalentPoints>()
            .init_resource::<TalentsState>()
            .init_resource::<TalentBonuses>()
            .init_resource::<TalentUiSelection>()
            .init_resource::<TalentLoadoutStore>()
            .init_resource::<CursorRestoreState>()
            .add_systems(
                OnEnter(MyStates::Next),
                (
                    spawn_talents_ui,
                    spawn_class_select_ui,
                    spawn_escape_menu_ui,
                ),
            )
            .add_systems(
                Update,
                (
                    enforce_class_selection_flow,
                    toggle_talents_ui,
                    toggle_escape_menu_ui,
                    sync_cursor_visibility_with_talents_ui,
                    refresh_class_dependent_text,
                    class_pick_button_interactions,
                    talent_ui_button_interactions,
                    update_talent_buttons_visuals,
                    update_talent_tooltip,
                    recompute_bonuses,
                )
                    .run_if(in_state(MyStates::Next)),
            );
    }
}

// --- Data model -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
pub enum TalentClass {
    Cleric,
    Bard,
    Paladin,
}

impl TalentClass {
    pub const ALL: [TalentClass; 3] =
        [TalentClass::Cleric, TalentClass::Bard, TalentClass::Paladin];
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct SelectedTalentClass(pub Option<TalentClass>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
pub enum TalentTree {
    Vigor,
    Guile,
    Sorcery,
}

impl TalentTree {
    pub const ALL: [TalentTree; 3] = [TalentTree::Vigor, TalentTree::Guile, TalentTree::Sorcery];
}

fn tree_title_for_class(class: TalentClass, tree: TalentTree) -> &'static str {
    match (class, tree) {
        // Cleric
        (TalentClass::Cleric, TalentTree::Vigor) => "Sanctuary",
        (TalentClass::Cleric, TalentTree::Guile) => "Judgement",
        (TalentClass::Cleric, TalentTree::Sorcery) => "Wards",
        // Bard
        (TalentClass::Bard, TalentTree::Vigor) => "Bladesong",
        (TalentClass::Bard, TalentTree::Guile) => "Ballads",
        (TalentClass::Bard, TalentTree::Sorcery) => "Trickery",
        // Paladin
        (TalentClass::Paladin, TalentTree::Vigor) => "Devotion",
        (TalentClass::Paladin, TalentTree::Guile) => "Vengeance",
        (TalentClass::Paladin, TalentTree::Sorcery) => "Grace",
    }
}

fn talent_display_name(_class: TalentClass, def: &TalentDef) -> String {
    def.name.to_string()
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
    /// -% fall damage per rank
    FallDamagePctPerRank(f32),
    /// +N extra mid-air jumps per rank
    ExtraAirJumpPerRank(u8),
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

pub const TIERS_PER_TREE: u8 = 7;
pub const SLOTS_PER_TIER: u8 = 2;

/// Classic-style tier requirements: tier 0 => 0 points, tier 1 => 5 points, ..., tier 7 => 35.
pub fn required_points_for_tier(tier: Tier) -> u8 {
    // We moved to a more "modern WoW" feel with fewer rows and fewer ranks per talent,
    // so the classic 5-points-per-row gating makes higher rows unreachable.
    // New gating: 0, 3, 6, 9, 12, 15, 18 ...
    tier.saturating_mul(3)
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

#[derive(Resource, Debug, Default, Clone)]
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
    pub fall_damage_mult: f32,
    pub extra_air_jumps: u8,
}

// --- UI state ---------------------------------------------------------------

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TalentUiState {
    pub open: bool,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ClassSelectUiState {
    pub open: bool,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct EscapeMenuUiState {
    pub open: bool,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
struct CursorRestoreState {
    has_saved: bool,
    visible: bool,
    grab_mode: CursorGrabMode,
    hit_test: bool,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TalentUiSelection {
    pub hovered: Option<TalentId>,
    pub hovered_entity: Option<Entity>,
}

#[derive(Component)]
struct TalentUiRoot;

#[derive(Component)]
struct ClassSelectUiRoot;

#[derive(Component)]
struct EscapeMenuUiRoot;

#[derive(Component)]
struct TalentButton {
    id: TalentId,
}

#[derive(Component)]
struct TalentRankText {
    id: TalentId,
}

#[derive(Component)]
struct TalentNameText {
    id: TalentId,
}

#[derive(Component)]
struct TreeTitleText {
    tree: TalentTree,
}

#[derive(Component)]
struct TalentTooltipRoot;

#[derive(Component)]
struct TalentTooltipTitle;

#[derive(Component)]
struct TalentTooltipBody;

#[derive(Component)]
struct TalentPointsText;

#[derive(Component)]
struct ResetTalentsButton;

#[derive(Component)]
struct RefundLastButton;

#[derive(Component)]
struct ClassPickButton {
    class: TalentClass,
}

#[derive(Component)]
struct SelectedClassText;

#[derive(Component)]
struct EscapeMenuTitleText;

#[derive(Resource, Debug, Default)]
struct TalentLoadoutStore {
    by_class: std::collections::HashMap<TalentClass, (TalentsState, TalentPoints)>,
}

// --- Talent definitions -----------------------------------------------------

pub const TALENTS: &[TalentDef] = &[
    // VIGOR (melee + movement)
    t(
        TalentTree::Vigor,
        0,
        0,
        "Fleet Footing",
        3,
        "+4% movement speed per rank.",
        None,
        TalentEffect::MoveSpeedPctPerRank(4.0),
    ),
    t(
        TalentTree::Vigor,
        0,
        1,
        "Firm Stance",
        3,
        "Placeholder: +3% resistance to knockback per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        1,
        0,
        "Longstrider",
        2,
        "+7% sprint effectiveness per rank.",
        None,
        TalentEffect::SprintPctPerRank(7.0),
    ),
    t(
        TalentTree::Vigor,
        1,
        1,
        "Hardened Soles",
        2,
        "-8% fall damage per rank.",
        None,
        TalentEffect::FallDamagePctPerRank(8.0),
    ),
    t(
        TalentTree::Vigor,
        2,
        0,
        "Spring Heels",
        3,
        "+7% jump height per rank.",
        None,
        TalentEffect::JumpHeightPctPerRank(7.0),
    ),
    t(
        TalentTree::Vigor,
        2,
        1,
        "Oaken Bones",
        2,
        "Placeholder: +5% max health per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        3,
        0,
        "Road-Worn Breath",
        2,
        "Placeholder: +8% stamina regen per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        3,
        1,
        "Sure Landing",
        2,
        "Floatier falls: -8% fall extra gravity per rank.",
        None,
        TalentEffect::FallExtraGravityPctPerRank(8.0),
    ),
    t(
        TalentTree::Vigor,
        4,
        0,
        "Brutal Timing",
        2,
        "Placeholder: +2% crit chance per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        4,
        1,
        "Iron Rhythm",
        2,
        "Placeholder: +6% attack speed per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        5,
        0,
        "Giant's Step",
        1,
        "Requires Fleet Footing. +10% movement speed.",
        Some(TalentId {
            tree: TalentTree::Vigor,
            tier: 0,
            slot: 0,
        }),
        TalentEffect::MoveSpeedPctPerRank(10.0),
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
        "Placeholder: after sprinting, keep +10% speed for 2s.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Vigor,
        6,
        1,
        "Hoplite's Leap",
        1,
        "Requires Spring Heels. +18% jump height.",
        Some(TalentId {
            tree: TalentTree::Vigor,
            tier: 2,
            slot: 0,
        }),
        TalentEffect::JumpHeightPctPerRank(18.0),
    ),
    // GUILE (control + tricks)
    t(
        TalentTree::Guile,
        0,
        0,
        "Lightstep",
        3,
        "+3% movement speed per rank.",
        None,
        TalentEffect::MoveSpeedPctPerRank(3.0),
    ),
    t(
        TalentTree::Guile,
        0,
        1,
        "Quick Fingers",
        3,
        "Placeholder: +6% pickup speed per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        1,
        0,
        "Duskwalker",
        2,
        "Placeholder: +8% stealth effectiveness per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        1,
        1,
        "Dirty Tricks",
        2,
        "Placeholder: +10% stun duration per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        2,
        0,
        "Short Fuse",
        3,
        "+5% sprint effectiveness per rank.",
        None,
        TalentEffect::SprintPctPerRank(5.0),
    ),
    t(
        TalentTree::Guile,
        2,
        1,
        "Catfall",
        2,
        "Floatier falls: -8% fall extra gravity per rank.",
        None,
        TalentEffect::FallExtraGravityPctPerRank(8.0),
    ),
    t(
        TalentTree::Guile,
        3,
        0,
        "Opportunist",
        2,
        "Placeholder: +4% bonus damage vs distracted enemies per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        3,
        1,
        "Fast Climb",
        2,
        "Placeholder: +12% climb speed per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        4,
        0,
        "Swift Reprisal",
        2,
        "Placeholder: after dodging, +8% speed for 2s per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        4,
        1,
        "Shadow Breath",
        2,
        "Placeholder: +10% stamina regen in darkness per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        5,
        0,
        "Prowler's Pace",
        1,
        "Requires Lightstep. +9% movement speed.",
        Some(TalentId {
            tree: TalentTree::Guile,
            tier: 0,
            slot: 0,
        }),
        TalentEffect::MoveSpeedPctPerRank(9.0),
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
        "Placeholder: while sprinting, +7% jump height per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Guile,
        6,
        1,
        "Featherfall",
        1,
        "Requires Catfall. -18% fall extra gravity.",
        Some(TalentId {
            tree: TalentTree::Guile,
            tier: 2,
            slot: 1,
        }),
        TalentEffect::FallExtraGravityPctPerRank(18.0),
    ),
    // SORCERY (mystic mobility)
    t(
        TalentTree::Sorcery,
        0,
        0,
        "Arcane Poise",
        3,
        "Placeholder: +4% mana regen per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        0,
        1,
        "Warded Boots",
        3,
        "+4% jump height per rank.",
        None,
        TalentEffect::JumpHeightPctPerRank(4.0),
    ),
    t(
        TalentTree::Sorcery,
        1,
        0,
        "Spellrunner",
        2,
        "+7% movement speed per rank.",
        None,
        TalentEffect::MoveSpeedPctPerRank(7.0),
    ),
    t(
        TalentTree::Sorcery,
        1,
        1,
        "Soft Descent",
        2,
        "Floatier falls: -10% fall extra gravity per rank.",
        None,
        TalentEffect::FallExtraGravityPctPerRank(10.0),
    ),
    t(
        TalentTree::Sorcery,
        2,
        0,
        "Flicker Step",
        3,
        "+5% sprint effectiveness per rank.",
        None,
        TalentEffect::SprintPctPerRank(5.0),
    ),
    t(
        TalentTree::Sorcery,
        2,
        1,
        "Aerial Ward",
        2,
        "Placeholder: -6% air control penalty per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        3,
        0,
        "Boundless Leap",
        2,
        "+9% jump height per rank.",
        None,
        TalentEffect::JumpHeightPctPerRank(9.0),
    ),
    t(
        TalentTree::Sorcery,
        3,
        1,
        "Leyline Stride",
        2,
        "Placeholder: +10% speed while near shrines per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        4,
        0,
        "Airwalk",
        1,
        "+1 mid-air jump.",
        None,
        TalentEffect::ExtraAirJumpPerRank(1),
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
        "Requires Flicker Step. +14% sprint effectiveness.",
        Some(TalentId {
            tree: TalentTree::Sorcery,
            tier: 2,
            slot: 0,
        }),
        TalentEffect::SprintPctPerRank(14.0),
    ),
    t(
        TalentTree::Sorcery,
        5,
        1,
        "Skyhook",
        1,
        "Requires Boundless Leap. +20% jump height.",
        Some(TalentId {
            tree: TalentTree::Sorcery,
            tier: 3,
            slot: 0,
        }),
        TalentEffect::JumpHeightPctPerRank(20.0),
    ),
    t(
        TalentTree::Sorcery,
        6,
        0,
        "Slip of Time",
        2,
        "Placeholder: +7% cooldown recovery per rank.",
        None,
        TalentEffect::Placeholder,
    ),
    t(
        TalentTree::Sorcery,
        6,
        1,
        "Feathered Sigil",
        2,
        "Floatier falls: -8% fall extra gravity per rank.",
        None,
        TalentEffect::FallExtraGravityPctPerRank(8.0),
    ),
];

#[allow(clippy::too_many_arguments)]
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
    class: Res<SelectedTalentClass>,
    class_select_ui: Res<ClassSelectUiState>,
    escape_ui: Res<EscapeMenuUiState>,
    root: Query<Entity, With<TalentUiRoot>>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::KeyT) {
        return;
    }
    // Don't allow opening talents until a class is selected, and don't open on top of Escape.
    if class.0.is_none() || class_select_ui.open || escape_ui.open {
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

fn toggle_escape_menu_ui(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut escape_ui: ResMut<EscapeMenuUiState>,
    mut talents_ui: ResMut<TalentUiState>,
    class_select_ui: Res<ClassSelectUiState>,
    root: Query<Entity, With<EscapeMenuUiRoot>>,
    talents_root: Query<Entity, With<TalentUiRoot>>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }

    // Priority: if the talents menu is open, Esc closes it (and does NOT open the escape menu).
    if talents_ui.open {
        talents_ui.open = false;
        if let Some(troot) = talents_root.iter().next() {
            commands.entity(troot).insert(Visibility::Hidden);
        }
        return;
    }

    // Forced class selection: Escape doesn't bypass it.
    if class_select_ui.open {
        return;
    }

    escape_ui.open = !escape_ui.open;

    // If opening escape menu, ensure talents UI is closed.
    if escape_ui.open {
        talents_ui.open = false;
        if let Some(troot) = talents_root.iter().next() {
            commands.entity(troot).insert(Visibility::Hidden);
        }
    }

    if let Some(root) = root.iter().next() {
        commands.entity(root).insert(if escape_ui.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
}

fn sync_cursor_visibility_with_talents_ui(
    ui_state: Res<TalentUiState>,
    class_select_ui: Res<ClassSelectUiState>,
    escape_ui: Res<EscapeMenuUiState>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut restore: ResMut<CursorRestoreState>,
) {
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };

    // Ensure the cursor is visible when the talent menu is open (so UI is usable),
    // but restore the previous cursor state when closing.
    let any_ui_open = ui_state.open || class_select_ui.open || escape_ui.open;
    if any_ui_open {
        if !restore.has_saved {
            restore.has_saved = true;
            restore.visible = cursor.visible;
            restore.grab_mode = cursor.grab_mode;
            restore.hit_test = cursor.hit_test;
        }
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
        cursor.hit_test = true;
    } else if restore.has_saved {
        cursor.visible = restore.visible;
        cursor.grab_mode = restore.grab_mode;
        cursor.hit_test = restore.hit_test;
        *restore = CursorRestoreState::default();
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

    let class_label = commands
        .spawn((
            SelectedClassText,
            Name::new("Talents Current Class Text"),
            Text::new("Class: —"),
            TextFont {
                font_size: 16.0,
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
    commands.entity(header).add_child(class_label);
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

    // Trees area (takes the whole body now; details are tooltip-on-hover)
    let trees = commands
        .spawn((
            Name::new("Talents Trees Area"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
        ))
        .id();
    commands.entity(body).add_child(trees);

    // Footer controls (reset / refund) so they're always available without a static details pane
    let footer = commands
        .spawn((
            Name::new("Talents Footer"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(44.0),
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
        ))
        .id();
    commands.entity(panel).add_child(footer);

    commands.entity(footer).with_child((
        RefundLastButton,
        Button,
        Name::new("Refund Last Button"),
        Node {
            width: Val::Px(120.0),
            height: Val::Px(34.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(wood),
        BorderColor::all(gold),
        children![(
            Text::new("Refund 1"),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.86)),
        )],
    ));
    commands.entity(footer).with_child((
        ResetTalentsButton,
        Button,
        Name::new("Reset Talents Button"),
        Node {
            width: Val::Px(120.0),
            height: Val::Px(34.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(wood),
        BorderColor::all(gold),
        children![(
            Text::new("Reset"),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.86)),
        )],
    ));

    // Build each tree column with 8 tiers.
    // Initial text is “Paladin”; a later system refreshes it from SelectedTalentClass.
    let default_class = TalentClass::Paladin;
    for tree in TalentTree::ALL {
        let tree_col = commands
            .spawn((
                Name::new(format!("Tree: {tree}")),
                Node {
                    width: Val::Percent(33.0),
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
            TreeTitleText { tree },
            Text::new(tree_title_for_class(default_class, tree)),
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
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            position_type: PositionType::Relative,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.35, 0.28, 0.18)),
                        BorderColor::all(gold),
                    ))
                    .id();

                let name = commands
                    .spawn((
                        TalentNameText { id: def.id },
                        Text::new(talent_display_name(default_class, def)),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.94, 0.90)),
                    ))
                    .id();

                let rank = commands
                    .spawn((
                        TalentRankText { id: def.id },
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(4.0),
                            bottom: Val::Px(2.0),
                            ..default()
                        },
                        ZIndex(20),
                        Text::new("0/0"),
                        TextFont {
                            font_size: 10.0,
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

    // Hover tooltip (absolute positioned; updated each frame while hovering)
    commands.entity(root).with_child((
        TalentTooltipRoot,
        Name::new("Talent Tooltip"),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(320.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(2.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.92, 0.88, 0.76)),
        BorderColor::all(wood),
        Visibility::Hidden,
        children![
            (
                TalentTooltipTitle,
                Text::new(""),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(ink),
            ),
            (
                TalentTooltipBody,
                Text::new(""),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(ink),
            ),
        ],
    ));
}

#[allow(clippy::type_complexity)]
fn refresh_class_dependent_text(
    selected: Res<SelectedTalentClass>,
    escape_ui: Res<EscapeMenuUiState>,
    mut set: ParamSet<(
        Query<&mut Text, With<EscapeMenuTitleText>>,
        Query<&mut Text, With<SelectedClassText>>,
        Query<(&TreeTitleText, &mut Text)>,
        Query<(&TalentNameText, &mut Text)>,
    )>,
) {
    if !selected.is_changed() && !escape_ui.is_changed() {
        return;
    }

    let class = selected.0.unwrap_or(TalentClass::Paladin);
    if let Ok(mut t) = set.p1().single_mut() {
        if let Some(sel) = selected.0 {
            *t = Text::new(format!("Class: {sel}"));
        } else {
            *t = Text::new("Class: —");
        }
    }

    if (selected.is_changed() || escape_ui.is_changed())
        && let Ok(mut t) = set.p0().single_mut()
    {
        if let Some(sel) = selected.0 {
            *t = Text::new(format!("Menu — Class: {sel}"));
        } else {
            *t = Text::new("Menu — Class: —");
        }
    }

    for (tt, mut text) in set.p2().iter_mut() {
        *text = Text::new(tree_title_for_class(class, tt.tree));
    }

    for (tn, mut text) in set.p3().iter_mut() {
        let Some(def) = talent_def(tn.id) else {
            continue;
        };
        *text = Text::new(talent_display_name(class, def));
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

    if let Some(pr) = def.prereq
        && talents.rank(pr) == 0
    {
        return (false, "Requires prerequisite talent");
    }

    (true, "OK")
}

fn talent_ui_button_interactions(
    interactions: Query<(Entity, &Interaction, &TalentButton), Changed<Interaction>>,
    reset_btn: Query<&Interaction, (Changed<Interaction>, With<ResetTalentsButton>)>,
    refund_btn: Query<&Interaction, (Changed<Interaction>, With<RefundLastButton>)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut talents: ResMut<TalentsState>,
    mut points: ResMut<TalentPoints>,
    mut selection: ResMut<TalentUiSelection>,
) {
    // Hover tracking (for details panel)
    for (entity, interaction, btn) in interactions.iter() {
        match *interaction {
            Interaction::Hovered => {
                selection.hovered = Some(btn.id);
                selection.hovered_entity = Some(entity);
            }
            Interaction::None => {
                if selection.hovered == Some(btn.id) {
                    selection.hovered = None;
                    selection.hovered_entity = None;
                }
            }
            Interaction::Pressed => {
                let shift_refund =
                    keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

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

    if let Some(interaction) = reset_btn.iter().next()
        && *interaction == Interaction::Pressed
    {
        talents.ranks.clear();
        talents.spent_stack.clear();
        points.available = 51;
    }

    if let Some(interaction) = refund_btn.iter().next()
        && *interaction == Interaction::Pressed
        && let Some(last) = talents.spent_stack.pop()
    {
        let current = talents.rank(last);
        if current > 0 {
            talents.set_rank(last, current - 1);
            points.available = points.available.saturating_add(1);
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_talent_buttons_visuals(
    talents: Res<TalentsState>,
    points: Res<TalentPoints>,
    mut buttons: Query<(&TalentButton, &mut BackgroundColor, &mut BorderColor)>,
    mut set: ParamSet<(
        Query<&mut Text, With<TalentPointsText>>,
        Query<(&TalentRankText, &mut Text)>,
    )>,
) {
    let spent = talents.total_points_spent();
    if let Ok(mut t) = set.p0().single_mut() {
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

    for (rt, mut text) in set.p1().iter_mut() {
        let Some(def) = talent_def(rt.id) else {
            continue;
        };
        let rank = talents.rank(rt.id);
        *text = Text::new(format!("{rank}/{max}", max = def.max_rank));
    }
}

fn effect_summary(def: &TalentDef, rank: u8) -> String {
    match def.effect {
        TalentEffect::MoveSpeedPctPerRank(p) => {
            if rank == 0 {
                format!("Effect: +{p:.0}% movement speed per rank")
            } else {
                format!(
                    "Effect: +{p:.0}% move speed per rank (current: +{cur:.0}%)",
                    cur = p * rank as f32
                )
            }
        }
        TalentEffect::SprintPctPerRank(p) => {
            if rank == 0 {
                format!("Effect: +{p:.0}% sprint effectiveness per rank")
            } else {
                format!(
                    "Effect: +{p:.0}% sprint per rank (current: +{cur:.0}%)",
                    cur = p * rank as f32
                )
            }
        }
        TalentEffect::JumpHeightPctPerRank(p) => {
            if rank == 0 {
                format!("Effect: +{p:.0}% jump height per rank")
            } else {
                format!(
                    "Effect: +{p:.0}% jump per rank (current: +{cur:.0}%)",
                    cur = p * rank as f32
                )
            }
        }
        TalentEffect::FallExtraGravityPctPerRank(p) => {
            if rank == 0 {
                format!("Effect: -{p:.0}% fall gravity per rank (floatier)")
            } else {
                format!(
                    "Effect: -{p:.0}% fall gravity per rank (current: -{cur:.0}%)",
                    cur = p * rank as f32
                )
            }
        }
        TalentEffect::FallDamagePctPerRank(p) => {
            if rank == 0 {
                format!("Effect: -{p:.0}% fall damage per rank")
            } else {
                format!(
                    "Effect: -{p:.0}% fall damage per rank (current: -{cur:.0}%)",
                    cur = p * rank as f32
                )
            }
        }
        TalentEffect::ExtraAirJumpPerRank(n) => {
            if rank == 0 {
                format!("Effect: +{n} mid-air jump")
            } else {
                format!(
                    "Effect: +{count} mid-air jump",
                    count = n as u32 * rank as u32
                )
            }
        }
        TalentEffect::Placeholder => "Effect: (placeholder)".to_string(),
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn update_talent_tooltip(
    ui_state: Res<TalentUiState>,
    selection: Res<TalentUiSelection>,
    selected_class: Res<SelectedTalentClass>,
    talents: Res<TalentsState>,
    points: Res<TalentPoints>,
    hovered_button: Query<(&ComputedNode, &UiGlobalTransform), With<TalentButton>>,
    mut tooltip: Query<(&mut Node, &mut Visibility), With<TalentTooltipRoot>>,
    mut set: ParamSet<(
        Query<&mut Text, With<TalentTooltipTitle>>,
        Query<&mut Text, With<TalentTooltipBody>>,
    )>,
) {
    if !ui_state.open {
        if let Ok((_, mut vis)) = tooltip.single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    }

    let Some(id) = selection.hovered else {
        if let Ok((_, mut vis)) = tooltip.single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let Some(def) = talent_def(id) else {
        return;
    };

    let Some(entity) = selection.hovered_entity else {
        if let Ok((_, mut vis)) = tooltip.single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };
    let Ok((computed, ui_xform)) = hovered_button.get(entity) else {
        if let Ok((_, mut vis)) = tooltip.single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let class = selected_class.0.unwrap_or(TalentClass::Paladin);
    let rank = talents.rank(id);
    let spent_in_tree = talents.points_spent_in_tree(id.tree);
    let tier_req = required_points_for_tier(id.tier);

    // Anchor tooltip to the hovered talent's lower-right corner.
    // UiGlobalTransform is in physical pixels and represents the node center.
    let center_physical = ui_xform.translation;
    let br_physical = center_physical + computed.size() * 0.5;
    let inv = computed.inverse_scale_factor;
    let br_logical = br_physical * inv;

    if let Ok((mut node, mut vis)) = tooltip.single_mut() {
        node.left = Val::Px(br_logical.x + 10.0);
        node.top = Val::Px(br_logical.y + 10.0);
        *vis = Visibility::Visible;
    }

    if let Ok(mut t) = set.p0().single_mut() {
        *t = Text::new(talent_display_name(class, def));
    }

    let prereq_line = def.prereq.map_or(String::new(), |pr| {
        let pr_name = talent_def(pr)
            .map(|d| talent_display_name(class, d))
            .unwrap_or_else(|| "Unknown".to_string());
        format!("Requires: {pr_name}\n")
    });

    let (ok, _) = can_invest(&talents, &points, id);
    if let Ok(mut b) = set.p1().single_mut() {
        *b = Text::new(format!(
            "Rank: {rank}/{max}\n{effect}\nUnlock row: {spent}/{req}\n{prereq}{desc}\n\n{hint}",
            max = def.max_rank,
            effect = effect_summary(def, rank),
            spent = spent_in_tree,
            req = tier_req,
            prereq = prereq_line,
            desc = def.description,
            hint = if ok {
                "Click to invest | Shift+Click to refund"
            } else {
                "Shift+Click to refund"
            }
        ));
    }
}

// --- Class selection + Escape menu -----------------------------------------

fn spawn_class_select_ui(mut commands: Commands) {
    let overlay = Color::srgba(0.02, 0.02, 0.02, 0.82);
    let parchment = Color::srgb(0.90, 0.85, 0.72);
    let wood = Color::srgb(0.22, 0.13, 0.08);
    let ink = Color::srgb(0.08, 0.05, 0.03);
    let gold = Color::srgb(0.78, 0.67, 0.30);

    commands
        .spawn((
            ClassSelectUiRoot,
            Name::new("Class Select UI Root"),
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
        .with_child((
            Name::new("Class Select Panel"),
            Node {
                width: Val::Px(560.0),
                height: Val::Px(320.0),
                padding: UiRect::all(Val::Px(18.0)),
                border: UiRect::all(Val::Px(3.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(parchment),
            BorderColor::all(wood),
            children![
                (
                    Text::new("Choose Your Calling"),
                    TextFont {
                        font_size: 28.0,
                        ..default()
                    },
                    TextColor(ink),
                ),
                (
                    Text::new("You must choose a class before entering the dungeon."),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(ink),
                ),
                (
                    Name::new("Class Select Buttons"),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(60.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    },
                    children![
                        class_pick_button(TalentClass::Cleric, wood, gold),
                        class_pick_button(TalentClass::Bard, wood, gold),
                        class_pick_button(TalentClass::Paladin, wood, gold),
                    ]
                ),
                (
                    Text::new("Later: press Esc to switch class."),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(ink),
                ),
            ],
        ));
}

fn class_pick_button(class: TalentClass, wood: Color, gold: Color) -> impl Bundle {
    (
        ClassPickButton { class },
        Button,
        Name::new(format!("Pick Class: {class}")),
        Node {
            width: Val::Px(165.0),
            height: Val::Px(44.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(wood),
        BorderColor::all(gold),
        children![(
            Text::new(class.to_string()),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.86)),
        )],
    )
}

fn spawn_escape_menu_ui(mut commands: Commands) {
    let overlay = Color::srgba(0.02, 0.02, 0.02, 0.70);
    let parchment = Color::srgb(0.90, 0.85, 0.72);
    let wood = Color::srgb(0.22, 0.13, 0.08);
    let ink = Color::srgb(0.08, 0.05, 0.03);
    let gold = Color::srgb(0.78, 0.67, 0.30);

    commands
        .spawn((
            EscapeMenuUiRoot,
            Name::new("Escape Menu UI Root"),
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
        .with_child((
            Name::new("Escape Menu Panel"),
            Node {
                width: Val::Px(520.0),
                height: Val::Px(340.0),
                padding: UiRect::all(Val::Px(18.0)),
                border: UiRect::all(Val::Px(3.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(parchment),
            BorderColor::all(wood),
            children![
                (
                    EscapeMenuTitleText,
                    Text::new("Menu — Class: —"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(ink),
                ),
                (
                    Text::new("Switch Class"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(ink),
                ),
                (
                    Name::new("Escape Menu Class Buttons"),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(60.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    },
                    children![
                        class_pick_button(TalentClass::Cleric, wood, gold),
                        class_pick_button(TalentClass::Bard, wood, gold),
                        class_pick_button(TalentClass::Paladin, wood, gold),
                    ]
                ),
                (
                    Text::new("Press Esc to close."),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(ink),
                ),
            ],
        ));
}

#[allow(clippy::too_many_arguments)]
fn enforce_class_selection_flow(
    class: Res<SelectedTalentClass>,
    mut class_ui: ResMut<ClassSelectUiState>,
    mut escape_ui: ResMut<EscapeMenuUiState>,
    mut talents_ui: ResMut<TalentUiState>,
    class_root: Query<Entity, With<ClassSelectUiRoot>>,
    escape_root: Query<Entity, With<EscapeMenuUiRoot>>,
    talents_root: Query<Entity, With<TalentUiRoot>>,
    mut commands: Commands,
) {
    // If no class is chosen yet, force the class select overlay open and close other UIs.
    if class.0.is_none() {
        if !class_ui.open {
            class_ui.open = true;
        }
        if escape_ui.open {
            escape_ui.open = false;
            if let Some(er) = escape_root.iter().next() {
                commands.entity(er).insert(Visibility::Hidden);
            }
        }
        if talents_ui.open {
            talents_ui.open = false;
            if let Some(tr) = talents_root.iter().next() {
                commands.entity(tr).insert(Visibility::Hidden);
            }
        }
        if let Some(cr) = class_root.iter().next() {
            commands.entity(cr).insert(Visibility::Visible);
        }
    } else if class_ui.open {
        class_ui.open = false;
        if let Some(cr) = class_root.iter().next() {
            commands.entity(cr).insert(Visibility::Hidden);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn class_pick_button_interactions(
    mut interactions: Query<(&Interaction, &ClassPickButton), Changed<Interaction>>,
    mut selected: ResMut<SelectedTalentClass>,
    mut hovered: ResMut<TalentUiSelection>,
    mut store: ResMut<TalentLoadoutStore>,
    mut talents: ResMut<TalentsState>,
    mut points: ResMut<TalentPoints>,
    mut escape_ui: ResMut<EscapeMenuUiState>,
    escape_root: Query<Entity, With<EscapeMenuUiRoot>>,
    mut commands: Commands,
) {
    for (interaction, btn) in interactions.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        // Save current class loadout before switching.
        if let Some(current) = selected.0 {
            store
                .by_class
                .insert(current, ((*talents).clone(), *points));
        }

        // Load or init new class loadout.
        if let Some((saved_talents, saved_points)) = store.by_class.get(&btn.class) {
            *talents = saved_talents.clone();
            *points = *saved_points;
        } else {
            *talents = TalentsState::default();
            *points = TalentPoints::default();
        }

        selected.0 = Some(btn.class);
        hovered.hovered = None;

        // If we picked via Escape menu, close it.
        if escape_ui.open {
            escape_ui.open = false;
            if let Some(er) = escape_root.iter().next() {
                commands.entity(er).insert(Visibility::Hidden);
            }
        }
    }
}

fn recompute_bonuses(talents: Res<TalentsState>, mut bonuses: ResMut<TalentBonuses>) {
    if !talents.is_changed() {
        return;
    }

    let mut out = TalentBonuses {
        move_speed_mult: 1.0,
        sprint_mult: 1.0,
        jump_height_mult: 1.0,
        fall_extra_gravity_mult: 1.0,
        fall_damage_mult: 1.0,
        extra_air_jumps: 0,
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
            TalentEffect::FallDamagePctPerRank(p) => {
                out.fall_damage_mult *= 1.0 - (p / 100.0) * rank;
            }
            TalentEffect::ExtraAirJumpPerRank(n) => {
                out.extra_air_jumps = out.extra_air_jumps.saturating_add((n as f32 * rank) as u8);
            }
            TalentEffect::Placeholder => {}
        }
    }

    // Clamp to sane bounds (avoid negative/zero gravity multipliers from stacking).
    out.fall_extra_gravity_mult = out.fall_extra_gravity_mult.clamp(0.35, 1.0);

    *bonuses = out;
}
