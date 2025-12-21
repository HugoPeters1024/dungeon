use crate::spells::{SpellBar, SpellDef, SpellEffect};

pub fn spellbar() -> SpellBar {
    // Paladin: different icon region so the bar looks distinct.
    let base = 48;
    [
        SpellDef {
            mana_cost: 40,
            icon_index: base,
            effect: SpellEffect::Heal(30.0),
        },
        SpellDef {
            mana_cost: 18,
            icon_index: base + 1,
            effect: SpellEffect::ElementalBlast {
                damage: 30.0,
                radius: 1.8,
                range: 6.0,
            },
        },
        SpellDef {
            mana_cost: 28,
            icon_index: base + 2,
            effect: SpellEffect::DamagePool {
                dps: 20.0,
                radius: 2.4,
                duration: 4.0,
                range: 4.8,
            },
        },
        SpellDef {
            mana_cost: 30,
            icon_index: base + 3,
            effect: SpellEffect::Heal(14.0),
        },
        SpellDef {
            mana_cost: 35,
            icon_index: base + 4,
            effect: SpellEffect::Heal(18.0),
        },
        SpellDef {
            // Q: Every class gets Dash here.
            mana_cost: 20,
            icon_index: base + 5,
            effect: SpellEffect::Dash(7.5),
        },
        SpellDef {
            mana_cost: 25,
            icon_index: base + 6,
            effect: SpellEffect::ManaBurst(14.0),
        },
        SpellDef {
            mana_cost: 32,
            icon_index: base + 7,
            effect: SpellEffect::ManaBurst(10.0),
        },
    ]
}
