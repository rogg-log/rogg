use gpui_kit::AssetSource;
use rogg_ui::assets::Assets;
use rogg_ui::icons::IconName;

#[test]
fn every_icon_is_embedded_and_mask_safe() {
    for icon in IconName::ALL {
        let bytes = Assets.load(icon.path()).unwrap().unwrap_or_else(|| panic!("{icon:?} missing"));
        let svg = std::str::from_utf8(&bytes).unwrap();
        assert!(svg.contains(r#"viewBox="0 0 24 24""#), "{icon:?} viewBox");
        assert!(!svg.contains("var("), "{icon:?} uses a CSS variable; masks cannot resolve it");
    }
}

#[test]
fn icon_paths_are_unique() {
    let mut paths: Vec<_> = IconName::ALL.iter().map(|i| i.path()).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), IconName::ALL.len());
}

#[test]
fn kit_icons_still_resolve_and_list_includes_ours() {
    assert!(Assets.load("icons/search.svg").unwrap().is_some());
    let listed = Assets.list("icons/rogg/").unwrap();
    assert_eq!(listed.len(), IconName::ALL.len());
}
