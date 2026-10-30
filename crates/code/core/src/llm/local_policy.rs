//! ACL routing configuration referencing the host's existing model catalog.

use std::collections::{BTreeSet, HashMap};

use a3s_acl::{
    AttributeSchema, Block, BlockSchema, Cardinality, Document, Schema, Value, ValueSchema,
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};

const MAX_POLICY_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelReference {
    pub provider_id: String,
    pub model_id: String,
}

impl ModelReference {
    pub fn validate(&self) -> Result<()> {
        validate_id(&self.provider_id)?;
        validate_id(&self.model_id)?;
        ensure!(
            !self.provider_id.contains("::"),
            "provider id contains the model identity separator"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ModelTarget {
    Model {
        provider_id: String,
        model_id: String,
    },
    Route {
        route_id: String,
    },
}

impl ModelTarget {
    pub fn parse(
        kind: &str,
        provider_id: Option<&str>,
        model_id: Option<&str>,
        route_id: Option<&str>,
    ) -> Result<Self> {
        let target = match kind {
            "model" => {
                ensure!(route_id.is_none(), "model binding contains route_id");
                Self::Model {
                    provider_id: provider_id
                        .context("model binding is missing provider_id")?
                        .to_owned(),
                    model_id: model_id
                        .context("model binding is missing model_id")?
                        .to_owned(),
                }
            }
            "route" => {
                ensure!(
                    provider_id.is_none() && model_id.is_none(),
                    "route binding contains model identity"
                );
                Self::Route {
                    route_id: route_id
                        .context("route binding is missing route_id")?
                        .to_owned(),
                }
            }
            value => bail!("unknown model target kind: {value}"),
        };
        target.validate()?;
        Ok(target)
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Model {
                provider_id,
                model_id,
            } => ModelReference {
                provider_id: provider_id.clone(),
                model_id: model_id.clone(),
            }
            .validate(),
            Self::Route { route_id } => validate_id(route_id),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Model { .. } => "model",
            Self::Route { .. } => "route",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelUse {
    Default,
    Quick,
    Translate,
}

impl ModelUse {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "default" => Ok(Self::Default),
            "quick" => Ok(Self::Quick),
            "translate" => Ok(Self::Translate),
            _ => bail!("unknown model use: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Quick => "quick",
            Self::Translate => "translate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    Fixed,
    Ordered,
}

impl RoutingStrategy {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "fixed" => Ok(Self::Fixed),
            "ordered" => Ok(Self::Ordered),
            _ => bail!("unknown local routing strategy: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Ordered => "ordered",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutePolicy {
    pub id: String,
    pub name: String,
    pub strategy: RoutingStrategy,
    pub candidates: Vec<ModelReference>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalModelPolicy {
    pub routes: Vec<RoutePolicy>,
}

impl LocalModelPolicy {
    pub fn from_acl(text: &str) -> Result<Self> {
        ensure!(
            text.len() <= MAX_POLICY_BYTES,
            "local model policy exceeds byte limit"
        );
        let document = a3s_acl::parse(text).context("invalid local model policy ACL")?;
        let report = a3s_acl::validate_document(&document, &policy_schema());
        ensure!(
            report.is_empty(),
            "invalid local model policy fields: {report:?}"
        );
        let root = document
            .blocks
            .first()
            .context("missing local_models block")?;
        let mut policy = Self::default();
        for block in &root.blocks {
            let id = block.labels.first().context("missing policy block label")?;
            policy.routes.push(RoutePolicy {
                id: id.clone(),
                name: string_attribute(block, "name")?.to_owned(),
                strategy: RoutingStrategy::parse(string_attribute(block, "strategy")?)?,
                candidates: block
                    .blocks
                    .iter()
                    .map(model_reference)
                    .collect::<Result<_>>()?,
            });
        }
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        for route in &self.routes {
            validate_id(&route.id)?;
            validate_id(&route.name)?;
            ensure!(
                ids.insert(&route.id),
                "duplicate local route id: {}",
                route.id
            );
            ensure!(
                !route.candidates.is_empty(),
                "local route has no candidates: {}",
                route.id
            );
            ensure!(
                route.strategy != RoutingStrategy::Fixed || route.candidates.len() == 1,
                "fixed local route requires exactly one candidate: {}",
                route.id
            );
            let mut candidates = BTreeSet::new();
            for candidate in &route.candidates {
                candidate.validate()?;
                ensure!(
                    candidates.insert(candidate),
                    "duplicate local route candidate: {}",
                    route.id
                );
            }
        }
        Ok(())
    }

    pub fn to_acl(&self) -> Result<String> {
        self.validate()?;
        let mut children = Vec::new();
        for route in &self.routes {
            let mut block = policy_block(
                "route",
                Some(&route.id),
                &[("name", &route.name), ("strategy", route.strategy.as_str())],
            );
            block.blocks = route
                .candidates
                .iter()
                .map(|model| {
                    policy_block(
                        "candidate",
                        None,
                        &[
                            ("provider_id", &model.provider_id),
                            ("model_id", &model.model_id),
                        ],
                    )
                })
                .collect();
            children.push(block);
        }
        let mut root = policy_block("local_models", None, &[]);
        root.blocks = children;
        let text = a3s_acl::generate(&Document { blocks: vec![root] });
        ensure!(
            text.len() <= MAX_POLICY_BYTES,
            "local model policy exceeds byte limit"
        );
        Ok(text)
    }

    /// The host resolves every returned reference against its admitted catalog.
    pub fn candidates(&self, target: &ModelTarget) -> Result<Vec<ModelReference>> {
        target.validate()?;
        match target {
            ModelTarget::Model {
                provider_id,
                model_id,
            } => Ok(vec![ModelReference {
                provider_id: provider_id.clone(),
                model_id: model_id.clone(),
            }]),
            ModelTarget::Route { route_id } => self
                .routes
                .iter()
                .find(|route| &route.id == route_id)
                .map(|route| route.candidates.clone())
                .with_context(|| format!("model target references missing route: {route_id}")),
        }
    }
}

fn policy_schema() -> Schema {
    let required = || AttributeSchema::required(ValueSchema::string());
    let candidate = Schema::new()
        .attribute("provider_id", required())
        .attribute("model_id", required());
    let route = Schema::new()
        .attribute("name", required())
        .attribute("strategy", required())
        .block(
            "candidate",
            BlockSchema::new(candidate).occurrences(Cardinality::at_least(1)),
        );
    Schema::new().block(
        "local_models",
        BlockSchema::new(Schema::new().block(
            "route",
            BlockSchema::new(route).labels(Cardinality::exactly(1)),
        ))
        .occurrences(Cardinality::exactly(1)),
    )
}

fn string_attribute<'a>(block: &'a Block, name: &str) -> Result<&'a str> {
    block
        .attributes
        .get(name)
        .and_then(Value::as_str)
        .with_context(|| format!("missing string attribute {}.{name}", block.name))
}

fn model_reference(block: &Block) -> Result<ModelReference> {
    Ok(ModelReference {
        provider_id: string_attribute(block, "provider_id")?.to_owned(),
        model_id: string_attribute(block, "model_id")?.to_owned(),
    })
}

fn policy_block(name: &str, label: Option<&str>, fields: &[(&str, &str)]) -> Block {
    Block {
        name: name.to_owned(),
        labels: label.into_iter().map(str::to_owned).collect(),
        attributes: fields
            .iter()
            .map(|(key, value)| (key.to_string(), Value::String(value.to_string())))
            .collect::<HashMap<_, _>>(),
        blocks: Vec::new(),
    }
}

fn validate_id(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= 1024,
        "local model identity must contain 1 to 1024 bytes"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
