#[cfg(test)]
mod tests {
    use hub_api::declaration::{INVENTORY_MARK, READY_MARK};
    use unit::{document as d, record};

    const TOML: &str = include_str!("../../unit.toml");

    fn document() -> d::Unit<String> {
        toml_edit::de::from_str(TOML).unwrap()
    }

    fn assert_legacy_policy<S: AsRef<str>>(value: &d::Unit<S>) {
        let legacy = &hub_declaration::UNIT;
        assert_eq!(value.schema, 1);
        assert_eq!(value.id.as_ref(), legacy.name);
        assert_eq!(value.loader.image.as_ref(), legacy.loader.image);
        assert!(matches!(legacy.loader.space, env::ProgramKind::User));
        assert_eq!(value.loader.space, d::Space::User);
        assert_eq!(
            value.loader.args,
            legacy
                .loader
                .args
                .iter()
                .map(|arg| *arg as u64)
                .collect::<Vec<_>>()
        );
        assert_eq!(value.loader.stack, legacy.loader.stack as u64);

        assert_eq!(
            value.operator.namespace.len(),
            legacy.operator.namespace.len()
        );
        for (current, previous) in value
            .operator
            .namespace
            .iter()
            .zip(legacy.operator.namespace)
        {
            assert_eq!(current.key.as_ref(), previous.key.name());
            assert_eq!(current.path.as_ref(), previous.path);
            assert!(matches!(
                previous.placement,
                legacy_unit::operator::Placement::Shared
            ));
            assert_eq!(current.placement, d::Placement::Shared);
            match (&current.scope, previous.scope) {
                (d::Scope::Subtree, legacy_unit::operator::Scope::Subtree) => {}
                (d::Scope::Names(names), legacy_unit::operator::Scope::Names(previous)) => {
                    assert_eq!(
                        names.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
                        previous
                    );
                }
                _ => panic!("namespace scope changed"),
            }
            assert_eq!(current.ops, previous.ops.bits());
            match (current.permit, previous.permit) {
                (None, None)
                | (Some(d::Permit::Public), Some(system_api::operator::Permit::Public)) => {}
                _ => panic!("namespace permit changed"),
            }
            assert_eq!(current.source_mark, previous.source_mark);
            assert!(previous.bindings.is_empty());
            assert!(current.bindings.is_empty());
        }
        assert_eq!(
            value
                .identity
                .aliases
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>(),
            legacy.identity.aliases
        );
        assert_eq!(
            value
                .identity
                .grants
                .iter()
                .map(|code| { system_api::identity::Grant::from_action(*code).unwrap() })
                .collect::<Vec<_>>(),
            legacy.identity.grants
        );

        assert_eq!(value.control.creation, d::Creation::Startup);
        assert!(value.control.requires.is_empty());
        assert_eq!(
            legacy.control.after.unwrap(),
            [
                legacy_unit::Dependency::Bootstrap(legacy_unit::Bootstrap::Operator),
                legacy_unit::Dependency::Bootstrap(legacy_unit::Bootstrap::Identity),
            ]
        );
        // System completes these bootstrap services before creating any Unit.
        assert!(value.control.after.is_empty());
        match (value.control.ready, legacy.control.ready) {
            (d::Ready::Signal(current), legacy_unit::Ready::Signal(previous)) => {
                assert_eq!(current, previous.get());
                assert_eq!(current, READY_MARK.get());
            }
            _ => panic!("ready policy changed"),
        }
        assert!(matches!(
            legacy.control.ending,
            Some(legacy_unit::Ending::Resident)
        ));
        assert_eq!(value.control.ending, d::Ending::Resident);
        assert_eq!(
            value.control.supervision.as_ref().map(AsRef::as_ref),
            legacy.control.supervision
        );
        assert!(legacy.control.construction.is_empty());
        assert!(value.control.construction.is_empty());
        assert!(value.control.groups.is_empty());
        assert!(!value.control.guarded);
        assert_eq!(
            value.control.retirement.len(),
            legacy.control.retirement.len()
        );
        for (current, previous) in value
            .control
            .retirement
            .iter()
            .zip(legacy.control.retirement)
        {
            assert_eq!(current.provider.as_ref(), previous.provider);
            assert_eq!(usize::from(current.namespace), previous.namespace);
            assert_eq!(current.face.as_ref(), previous.face);
        }

        assert_eq!(value.supply.bindings.len(), legacy.supply.bindings.len());
        for (current, previous) in value.supply.bindings.iter().zip(legacy.supply.bindings) {
            assert_eq!(current.source.as_ref(), previous.source.name());
            assert_eq!(current.resource.as_ref(), previous.resource.name());
            assert_eq!(current.slot, previous.slot.get());
            assert_eq!(current.permission, previous.permission.bits());
        }
        assert_eq!(
            value.operator.namespace[2].source_mark,
            Some(hub_api::Grant::Dma.mark().get())
        );
        assert_eq!(value.supply.bindings[0].slot, INVENTORY_MARK.get());
        value.validate().unwrap();
    }

    #[test]
    fn toml_preserves_every_legacy_policy_field() {
        assert_legacy_policy(&document());
    }

    #[test]
    fn binary_roundtrip_preserves_every_field_and_borrows_strings() {
        let declaration = document();
        let bytes = record::encode(&declaration).unwrap();
        let decoded = record::decode(&bytes).unwrap();
        assert_legacy_policy(&decoded);
        let text = toml_edit::ser::to_string(&decoded).unwrap();
        let owned: d::Unit<String> = toml_edit::de::from_str(&text).unwrap();
        assert_eq!(owned, declaration);
        let begin = bytes.as_ptr() as usize;
        let end = begin + bytes.len();
        for text in [
            decoded.id,
            decoded.loader.image,
            decoded.supply.bindings[0].resource,
        ] {
            let pointer = text.as_ptr() as usize;
            assert!(pointer >= begin && pointer + text.len() <= end);
        }
    }

    #[test]
    fn quoted_decimal_and_hex_marks_keep_the_native_u64_values() {
        let mut hexadecimal = TOML.to_owned();
        for mark in [READY_MARK, INVENTORY_MARK, hub_api::Grant::Dma.mark()] {
            assert!(mark.get() > i64::MAX as u64);
            hexadecimal = hexadecimal.replace(
                &format!("\"{}\"", mark.get()),
                &format!("\"0x{:016x}\"", mark.get()),
            );
        }
        let decoded: d::Unit<String> = toml_edit::de::from_str(&hexadecimal).unwrap();
        assert_eq!(decoded, document());
        assert_legacy_policy(&record::decode(&record::encode(&decoded).unwrap()).unwrap());
    }

    #[test]
    fn image_names_the_existing_cargo_binary_and_preserves_the_build_contract() {
        let build = &hub_declaration::BUILD;
        assert_eq!(document().loader.image, build.image);
        assert_eq!(build.manifest, "server/Cargo.toml");
        let manifest: toml_edit::DocumentMut =
            include_str!("../../server/Cargo.toml").parse().unwrap();
        assert_eq!(manifest["package"]["name"].as_str(), Some(build.package));
        let bins = manifest["bin"].as_array_of_tables().unwrap();
        assert_eq!(bins.len(), 1);
        let binary = bins.get(0).unwrap();
        assert_eq!(binary["name"].as_str(), Some(build.bin));
        assert_eq!(build.bin, build.image);
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../server")
            .join(binary["path"].as_str().unwrap());
        assert!(entry.is_file());
        assert!(build.features.is_empty());
        assert!(build.default_features);
        assert_eq!(build.target, "riscv64gc-unknown-none-elf");
        assert_eq!(build.profile, "release");
    }
}
