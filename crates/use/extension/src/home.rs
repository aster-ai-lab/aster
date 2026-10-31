//! Home-shelf cards. A package contributes tiles; the host owns the shelf layout.

use aster_acl::Block;
use aster_use_core::UseResult;
use serde::{Deserialize, Serialize};

use super::{
    bounded_text, manifest_error, optional_i32_attribute, optional_string_attribute,
    require_known_attributes, string_attribute, valid_segment,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginHomeShelf {
    Write,
    Create,
    Process,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginHomeAction {
    Writing,
    Applet,
    File,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginHomeCard {
    pub id: String,
    pub shelf: PluginHomeShelf,
    pub title: String,
    pub description: String,
    pub order: i32,
    pub action: PluginHomeAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writing_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
}

impl PluginHomeShelf {
    fn parse(value: &str) -> UseResult<Self> {
        match value {
            "write" => Ok(Self::Write),
            "create" => Ok(Self::Create),
            "process" => Ok(Self::Process),
            _ => Err(manifest_error(
                "AIX home shelf must be write, create, or process.",
            )),
        }
    }
}

impl PluginHomeAction {
    fn parse(value: &str) -> UseResult<Self> {
        match value {
            "writing" => Ok(Self::Writing),
            "applet" => Ok(Self::Applet),
            "file" => Ok(Self::File),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(manifest_error(
                "AIX home action must be writing, applet, file, or unavailable.",
            )),
        }
    }
}

pub(super) fn parse_home(block: &Block) -> UseResult<PluginHomeCard> {
    let [id] = block.labels.as_slice() else {
        return Err(manifest_error("AIX home card requires one local ID."));
    };
    if !valid_segment(id) || !block.blocks.is_empty() {
        return Err(manifest_error(
            "AIX home card IDs must be valid and unique inside the package.",
        ));
    }
    require_known_attributes(
        block,
        &[
            "shelf",
            "title",
            "description",
            "order",
            "action",
            "seed",
            "tag",
            "writing_type",
            "applet",
            "file_kind",
            "format",
            "cover",
        ],
    )?;
    let action = PluginHomeAction::parse(&string_attribute(block, "action")?)?;
    let writing_type = optional_string_attribute(block, "writing_type")?;
    let applet = optional_string_attribute(block, "applet")?;
    let file_kind = optional_string_attribute(block, "file_kind")?;
    let format = optional_string_attribute(block, "format")?;
    let cover = optional_string_attribute(block, "cover")?;
    match action {
        PluginHomeAction::Writing => {
            if !matches!(
                writing_type.as_deref(),
                Some("notice" | "report" | "request" | "minutes")
            ) {
                return Err(manifest_error(
                    "AIX writing card requires writing_type notice, report, request, or minutes.",
                ));
            }
        }
        PluginHomeAction::Applet => {
            if !matches!(applet.as_deref(), Some("plan" | "pdf-tools")) {
                return Err(manifest_error(
                    "AIX applet card requires applet plan or pdf-tools.",
                ));
            }
        }
        PluginHomeAction::File => {
            if !matches!(file_kind.as_deref(), Some("pdf-to-word" | "word-to-pdf"))
                || !matches!(format.as_deref(), Some("pdf" | "word"))
            {
                return Err(manifest_error(
                    "AIX file card requires file_kind pdf-to-word or word-to-pdf and format pdf or word.",
                ));
            }
        }
        PluginHomeAction::Unavailable => {
            if !matches!(cover.as_deref(), Some("ppt" | "site" | "video" | "image")) {
                return Err(manifest_error(
                    "AIX unavailable card requires cover ppt, site, video, or image.",
                ));
            }
        }
    }
    Ok(PluginHomeCard {
        id: id.clone(),
        shelf: PluginHomeShelf::parse(&string_attribute(block, "shelf")?)?,
        title: bounded_text(string_attribute(block, "title")?, "home title", 64)?,
        description: bounded_text(
            optional_string_attribute(block, "description")?.unwrap_or_default(),
            "home description",
            240,
        )?,
        order: optional_i32_attribute(block, "order")?.unwrap_or(100),
        action,
        seed: optional_string_attribute(block, "seed")?,
        tag: optional_string_attribute(block, "tag")?,
        writing_type,
        applet,
        file_kind,
        format,
        cover,
    })
}
