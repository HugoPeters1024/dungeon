use crate::spells::{DamageElement, SpellBar, SpellDef, SpellEffect};

pub fn spellbar() -> SpellBar {
    // Icons are row-major indices into `assets/icons.png`.
    // Cleric: use an early region in the sheet.
    let base = 0;
    [
        SpellDef {
            mana_cost: 32,
            icon_index: base,
            effect: SpellEffect::ElementalBlast {
                damage: 28.0,
                radius: 1.6,
                range: 6.5,
                element: DamageElement::Darkness,
            },
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
            mana_cost: 38,
            icon_index: base + 3,
            effect: SpellEffect::DamagePool {
                dps: 14.0,
                radius: 2.2,
                duration: 4.5,
                range: 5.0,
                element: DamageElement::Darkness,
            },
        },
        SpellDef {
            mana_cost: 24,
            icon_index: base + 4,
            effect: SpellEffect::ElementalBlast {
                damage: 22.0,
                radius: 1.3,
                range: 7.5,
                element: DamageElement::Frost,
            },
        },
        SpellDef {
            // Q: Every class gets Dash here.
            mana_cost: 20,
            icon_index: base + 5,
            effect: SpellEffect::Dash(6.0),
        },
        SpellDef {
            mana_cost: 12,
            icon_index: base + 6,
            effect: SpellEffect::ElementalBlast {
                damage: 18.0,
                radius: 1.2,
                range: 8.0,
                element: DamageElement::Fire,
            },
        },
        SpellDef {
            mana_cost: 50,
            icon_index: base + 7,
            effect: SpellEffect::Heal(28.0),
        },
    ]
}
