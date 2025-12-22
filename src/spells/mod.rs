use crate::talents::TalentClass;

pub mod bard;
pub mod cleric;
pub mod paladin;

pub const SPELL_SLOTS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DamageElement {
    Darkness,
    Sonic,
    Holy,
    Fire,
    Frost,
}

#[derive(Clone, Copy, Debug)]
pub enum SpellEffect {
    Heal(f32),
    Dash(f32),
    ManaBurst(f32),
    ElementalBlast {
        damage: f32,
        radius: f32,
        range: f32,
        element: DamageElement,
    },
    DamagePool {
        dps: f32,
        radius: f32,
        duration: f32,
        range: f32,
        element: DamageElement,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct SpellDef {
    pub mana_cost: u32,
    /// Row-major index into `assets/icons.png`.
    pub icon_index: usize,
    pub effect: SpellEffect,
}

pub type SpellBar = [SpellDef; SPELL_SLOTS];

const DASH_SLOT: usize = 5; // Q

pub fn spellbar_for_class(class: TalentClass) -> SpellBar {
    let mut bar = match class {
        TalentClass::Cleric => cleric::spellbar(),
        TalentClass::Bard => bard::spellbar(),
        TalentClass::Paladin => paladin::spellbar(),
    };

    // Every character gets dash on Q, always (but the spell definition decides the icon).
    // If a class file accidentally changes the slot, force the effect back to Dash while
    // preserving mana/icon.
    if !matches!(bar[DASH_SLOT].effect, SpellEffect::Dash(_)) {
        bar[DASH_SLOT].effect = SpellEffect::Dash(6.0);
    }

    // And nowhere else (enforced in debug builds).
    debug_assert!(bar.iter().enumerate().all(|(i, s)| {
        (i == DASH_SLOT && matches!(s.effect, SpellEffect::Dash(_)))
            || (i != DASH_SLOT && !matches!(s.effect, SpellEffect::Dash(_)))
    }));

    bar
}
