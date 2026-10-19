#![cfg(test)]

use super::MenuLabels;

#[test]
fn native_menu_uses_the_same_catalog_as_the_renderer() {
    let chinese = MenuLabels::load("zh-CN").unwrap();
    assert_eq!(
        chinese.about.replace("{{product}}", "ToWork"),
        "关于 ToWork"
    );
    assert_eq!(chinese.edit, "编辑");
    assert_eq!(chinese.quit.replace("{{product}}", "ToWork"), "退出 ToWork");
    let english = MenuLabels::load("en-US").unwrap();
    assert_eq!(english.copy, "Copy");
    assert_eq!(english.edit, "Edit");
    assert_eq!(MenuLabels::load("").unwrap().copy, "复制");
}
