use crate::spells::{SpellBar, SpellDef, SpellEffect};

pub fn spellbar() -> SpellBar {
    // Paladin: different icon region so the bar looks distinct.
    let base = 48;
    [
        SpellDef {
            mana_cost: 20,
            icon_index: base,
            effect: SpellEffect::Dash(6.0),
        },
        SpellDef {
            mana_cost: 40,
            icon_index: base + 1,
            effect: SpellEffect::Heal(30.0),
        },
        SpellDef {
            mana_cost: 18,
            icon_index: base + 2,
            effect: SpellEffect::ManaBurst(10.0),
        },
        SpellDef {
            mana_cost: 25,
            icon_index: base + 3,
            effect: SpellEffect::Dash(4.0),
        },
        SpellDef {
            mana_cost: 30,
            icon_index: base + 4,
            effect: SpellEffect::Heal(14.0),
        },
        SpellDef {
            mana_cost: 22,
            icon_index: base + 5,
            effect: SpellEffect::ManaBurst(12.0),
        },
        SpellDef {
            mana_cost: 35,
            icon_index: base + 6,
            effect: SpellEffect::Heal(18.0),
        },
        SpellDef {
            mana_cost: 50,
            icon_index: base + 7,
            effect: SpellEffect::Dash(8.0),
        },
    ]
}
