use crate::spells::{SpellBar, SpellDef, SpellEffect};

pub fn spellbar() -> SpellBar {
    // Icons are row-major indices into `assets/icons.png`.
    // Cleric: use an early region in the sheet.
    let base = 0;
    [
        SpellDef {
            mana_cost: 20,
            icon_index: base,
            effect: SpellEffect::Dash(2.8),
        },
        SpellDef {
            mana_cost: 30,
            icon_index: base + 1,
            effect: SpellEffect::Heal(22.0),
        },
        SpellDef {
            mana_cost: 18,
            icon_index: base + 2,
            effect: SpellEffect::ManaBurst(12.0),
        },
        SpellDef {
            mana_cost: 45,
            icon_index: base + 3,
            effect: SpellEffect::Heal(40.0),
        },
        SpellDef {
            mana_cost: 25,
            icon_index: base + 4,
            effect: SpellEffect::Dash(4.0),
        },
        SpellDef {
            mana_cost: 12,
            icon_index: base + 5,
            effect: SpellEffect::ManaBurst(8.0),
        },
        SpellDef {
            mana_cost: 35,
            icon_index: base + 6,
            effect: SpellEffect::Heal(12.0),
        },
        SpellDef {
            mana_cost: 50,
            icon_index: base + 7,
            effect: SpellEffect::Heal(28.0),
        },
    ]
}
