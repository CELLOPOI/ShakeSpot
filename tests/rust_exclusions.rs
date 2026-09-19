use shakespot::{exclusions::Exclusions, settings};

#[test]
fn names_match_exactly_without_case_or_separator_sensitivity() {
    let rules = Exclusions::parse("Notepad.EXE\nnotepad.exe\n").unwrap();
    assert_eq!(rules.entries().len(), 1);
    assert!(rules.matches("C:/Windows/System32/NOTEPAD.exe"));
    assert!(!rules.matches(r"C:\Tools\Notepad++.exe"));
    assert!(!rules.matches(r"C:\Tools\my-notepad.exe"));
}

#[test]
fn full_path_exclusions_do_not_block_another_installation() {
    let rules = Exclusions::parse(r#""D:\绘图工具\Paint.exe""#).unwrap();
    assert!(rules.matches(r"d:\绘图工具\PAINT.EXE"));
    assert!(!rules.matches(r"C:\Other\Paint.exe"));
}

#[test]
fn legacy_configuration_and_new_exclusions_roundtrip() {
    let legacy = settings::parse("sensitivity=4\nenabled=0\n").unwrap();
    assert_eq!(legacy.sensitivity, 4);
    assert!(!legacy.enabled);
    assert!(legacy.excluded_apps.is_empty());
    let value = settings::parse(
        "sensitivity=2\nenabled=0\nexcludedApp=NOTEPAD.EXE\nexcludedApp=D:\\Apps\\Draw.exe\n",
    )
    .unwrap();
    assert_eq!(settings::parse(&settings::encode(&value)).unwrap(), value);
    assert!(!value.enabled);
}

#[test]
fn invalid_rules_are_reported_instead_of_silently_removed() {
    for text in [
        "*.exe",
        "subdir\\app.exe",
        "C:\\Apps\\folder",
        "evil\\0.exe",
        "C:\\Apps\\a\\nb.exe",
    ] {
        let text = text.replace("\\0", "\0").replace("\\n", "\n");
        assert!(Exclusions::parse(&text).is_err(), "{text:?}");
    }
    assert!(settings::parse("excludedApp=*.exe\n").is_err());
}

#[test]
fn exclusion_count_and_total_size_are_bounded() {
    let many = (0..33)
        .map(|i| format!("app{i}.exe"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(Exclusions::parse(&many).is_err());
    assert!(Exclusions::parse(&format!("{}.exe", "a".repeat(4096))).is_err());
}

#[test]
fn maximum_length_rules_survive_saving_and_normalization_is_bounded() {
    let path = format!(
        r"C:\{}.exe",
        "a".repeat(shakespot::exclusions::MAX_EXCLUSION_BYTES - 7)
    );
    let value = shakespot::core::Settings {
        excluded_apps: Exclusions::parse(&path).unwrap(),
        ..Default::default()
    };
    assert_eq!(settings::parse(&settings::encode(&value)).unwrap(), value);
    assert!(Exclusions::parse(&format!("{}.exe", "İ".repeat(1365))).is_err());
}
