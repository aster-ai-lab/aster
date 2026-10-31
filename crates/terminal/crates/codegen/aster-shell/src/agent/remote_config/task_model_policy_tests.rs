use super::*;

fn snapshot(families: &[Option<&str>]) -> TaskModelCatalogSnapshot {
    TaskModelCatalogSnapshot {
        eligible: families
            .iter()
            .enumerate()
            .map(|(index, family)| EligibleTaskModel {
                id: format!("model-{index}"),
                model_family: family.map(str::to_owned),
            })
            .collect(),
        authority: CatalogAuthority::Complete,
    }
}

#[test]
fn selection_is_hidden_only_for_an_enabled_all_aster_catalog() {
    let all_aster = || snapshot(&[Some("aster"), Some("aster")]);
    assert_eq!(
        TaskModelSelection::Inherited,
        resolve_presentation(true, all_aster()).selection
    );
    assert_eq!(
        TaskModelSelection::Selectable,
        resolve_presentation(true, snapshot(&[Some("aster"), None])).selection
    );
    assert_eq!(
        TaskModelSelection::Selectable,
        resolve_presentation(false, all_aster()).selection
    );
    let mut provisional = all_aster();
    provisional.authority = CatalogAuthority::Provisional;
    assert_eq!(
        TaskModelSelection::Selectable,
        resolve_presentation(true, provisional).selection
    );
}
