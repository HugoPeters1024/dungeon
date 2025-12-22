use crate::spells::{DamageElement, SpellBar, SpellDef, SpellEffect};

pub fn spellbar() -> SpellBar {
    // Bard: different icon region so the bar looks distinct.
    let base = 38 * 2;
    [
        SpellDef {
            mana_cost: 35,
            icon_index: base,
            effect: SpellEffect::ElementalBlast {
                damage: 22.0,
                radius: 1.4,
                range: 7.5,
                element: DamageElement::Sonic,
            },
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
                element: DamageElement::Sonic,
            },
        },
        SpellDef {
            mana_cost: 28,
            icon_index: 12 * 19 + 2,
            effect: SpellEffect::ElementalBlast {
                damage: 18.0,
                radius: 1.2,
                range: 8.5,
                element: DamageElement::Fire,
            },
        },
        SpellDef {
            // Q: Every class gets Dash here.
            mana_cost: 20,
            icon_index: 15,
            effect: SpellEffect::Dash(7.0),
        },
        SpellDef {
            // E: Every class gets a pool.
            mana_cost: 10,
            icon_index: 7,
            effect: SpellEffect::DamagePool {
                dps: 6.0,
                radius: 12.0,
                duration: 4.5,
                range: 9.0,
                element: DamageElement::Sonic,
            },
        },
        SpellDef {
            // R: Every class gets a heal.
            mana_cost: 40,
            icon_index: 10,
            effect: SpellEffect::Heal(28.0),
        },
    ]
}
