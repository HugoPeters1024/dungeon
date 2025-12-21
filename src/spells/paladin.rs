use crate::spells::{DamageElement, SpellBar, SpellDef, SpellEffect};

pub fn spellbar() -> SpellBar {
    // Paladin: different icon region so the bar looks distinct.
    let base = 38;
    [
        SpellDef {
            mana_cost: 18,
            icon_index: (10 * 19) + 1,
            effect: SpellEffect::ElementalBlast {
                damage: 30.0,
                radius: 1.8,
                range: 6.0,
                element: DamageElement::Holy,
            },
        },
        SpellDef {
            mana_cost: 40,
            icon_index: (10 * 19),
            effect: SpellEffect::Heal(30.0),
        },
        SpellDef {
            mana_cost: 28,
            icon_index: (11 * 19) + 1,
            effect: SpellEffect::DamagePool {
                dps: 20.0,
                radius: 2.4,
                duration: 4.0,
                range: 4.8,
                element: DamageElement::Fire,
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
            icon_index: 15,
            effect: SpellEffect::Dash(7.5),
        },
        SpellDef {
            // E: Every class gets a pool.
            mana_cost: 10,
            icon_index: 6,
            effect: SpellEffect::DamagePool {
                dps: 27.0,
                radius: 2.0,
                duration: 3.1,
                range: 7.0,
                element: DamageElement::Holy,
            },
        },
        SpellDef {
            // R: Every class gets a heal.
            mana_cost: 50,
            icon_index: 10,
            effect: SpellEffect::Heal(64.0),
        },
    ]
}
