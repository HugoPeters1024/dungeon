use crate::talents::TalentClass;

pub mod bard;
pub mod cleric;
pub mod paladin;

pub const SPELL_SLOTS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub enum SpellEffect {
    Heal(f32),
    Dash(f32),
    ManaBurst(f32),
    ElementalBlast {
        damage: f32,
        radius: f32,
        range: f32,
    },
    DamagePool {
        dps: f32,
        radius: f32,
        duration: f32,
        range: f32,
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

pub fn spellbar_for_class(class: TalentClass) -> SpellBar {
    match class {
        TalentClass::Cleric => cleric::spellbar(),
        TalentClass::Bard => bard::spellbar(),
        TalentClass::Paladin => paladin::spellbar(),
    }
}
