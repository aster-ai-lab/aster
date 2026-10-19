use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct MenuLabels {
    #[serde(rename = "native_menu.about")]
    pub about: String,
    #[serde(rename = "native_menu.quit")]
    pub quit: String,
    #[serde(rename = "native_menu.hide")]
    pub hide: String,
    #[serde(rename = "native_menu.hide_others")]
    pub hide_others: String,
    #[serde(rename = "native_menu.edit")]
    pub edit: String,
    #[serde(rename = "native_menu.undo")]
    pub undo: String,
    #[serde(rename = "native_menu.redo")]
    pub redo: String,
    #[serde(rename = "native_menu.cut")]
    pub cut: String,
    #[serde(rename = "native_menu.copy")]
    pub copy: String,
    #[serde(rename = "native_menu.paste")]
    pub paste: String,
    #[serde(rename = "native_menu.select_all")]
    pub select_all: String,
}

impl MenuLabels {
    pub fn load(language: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(if language == "en-US" {
            include_str!("../../../src/renderer/i18n/locales/en-us.json")
        } else {
            include_str!("../../../src/renderer/i18n/locales/zh-cn.json")
        })
    }
}

#[cfg(test)]
#[path = "menu_labels_tests.rs"]
mod tests;
