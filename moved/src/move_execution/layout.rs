use move_core_types::value::MoveTypeLayout;

/// Analyses **type** layout for invariants that require deserializing and analysing **value**.
///
/// If this function returns:
/// * `true`: has invariants based on **value** that need to be checked.
/// * `false`: has no invariants based on **value** that need to be checked.
///
/// The intended use-case is to optimize **parameter** validation. If a **type** has no invariants
/// to apply on a concrete **value** of itself, you may skip its deserialization and validation.
pub fn has_value_invariants(layout: &MoveTypeLayout) -> bool {
    match layout {
        // Signer needs value validation to see if it contains expected signer
        MoveTypeLayout::Signer => true,
        // Check if it is a vector of a type that needs validation
        MoveTypeLayout::Vector(layout) => has_value_invariants(layout.as_ref()),
        // Struct needs value validation to see if it is allowed AND perform struct type specific
        // value validation. For example, string is a struct that needs UTF-8 encoding validation.
        MoveTypeLayout::Struct(..) => true,
        // Check if this custom layout represents type that needs validation
        MoveTypeLayout::Native(_, layout) => has_value_invariants(layout.as_ref()),
        // No other types have invariants
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, move_core_types::value::MoveStructLayout};

    #[test]
    fn test_string_needs_value_validation() {
        let string_layout =
            MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![MoveTypeLayout::U8]));
        assert!(has_value_invariants(&string_layout));
    }
}
