use crate::spells::{SpellBar, SpellDef, SpellEffect};

pub fn spellbar() -> SpellBar {
    // Bard: different icon region so the bar looks distinct.
    let base = 24;
    [
        SpellDef {
            mana_cost: 15,
            icon_index: base,
            effect: SpellEffect::Dash(4.5),
        },
        SpellDef {
            mana_cost: 25,
            icon_index: base + 1,
            effect: SpellEffect::ManaBurst(20.0),
        },
        SpellDef {
            mana_cost: 22,
            icon_index: base + 2,
            effect: SpellEffect::Heal(10.0),
        },
        SpellDef {
            mana_cost: 40,
            icon_index: base + 3,
            effect: SpellEffect::DamagePool {
                dps: 18.0,
                radius: 2.6,
                duration: 4.0,
                range: 5.5,
            },
        },
        SpellDef {
            mana_cost: 28,
            icon_index: base + 4,
            effect: SpellEffect::ManaBurst(14.0),
        },
        SpellDef {
            mana_cost: 26,
            icon_index: base + 5,
            effect: SpellEffect::ElementalBlast {
                damage: 22.0,
                radius: 1.4,
                range: 7.5,
            },
        },
        SpellDef {
            mana_cost: 30,
            icon_index: base + 6,
            effect: SpellEffect::Heal(16.0),
        },
        SpellDef {
            mana_cost: 50,
            icon_index: base + 7,
            effect: SpellEffect::Dash(7.5),
        },
    ]
}
