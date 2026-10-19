//! Host-owned retrieval and tool-presentation declarations.

use super::{CodeConfig, WorkspaceRetrievalConfig};
use crate::error::{CodeError, Result};
use crate::tools::{ToolPresentationModeV1, ToolPresentationProfileV1};
use a3s_acl::{Block, Document, Lexer, Token, Value};
use std::collections::HashSet;

fn invalid(name: &str, reason: &str) -> CodeError {
    CodeError::Config(format!("{name}: {reason}"))
}

pub(super) fn apply_layer(source: &str, doc: &Document, config: &mut CodeConfig) -> Result<()> {
    for name in ["workspace_retrieval", "tool_presentation"] {
        let mut declarations = doc.blocks.iter().filter(|block| block.name == name);
        let Some(block) = declarations.next() else {
            continue;
        };
        if declarations.next().is_some() {
            return Err(invalid(name, "duplicate declaration in one ACL layer"));
        }
        if !block.labels.is_empty() {
            return Err(invalid(name, "requires an unlabeled block"));
        }
        match name {
            "workspace_retrieval" => config.workspace_retrieval = Some(parse_retrieval(block)?),
            _ => {
                if !block.blocks.is_empty() || block.attributes.len() != 1 {
                    return Err(invalid(name, "requires only mode = a literal string"));
                }
                let Some(Value::String(mode)) = block.attributes.get("mode") else {
                    return Err(invalid(name, "requires mode = a literal string"));
                };
                let mode = ToolPresentationModeV1::parse(mode)
                    .map_err(|error| invalid(name, &error.to_string()))?;
                config.tool_presentation_profile = Some(ToolPresentationProfileV1::for_mode(mode));
            }
        }
        validate_source_fields(source, name)?;
    }
    Ok(())
}

fn parse_retrieval(block: &Block) -> Result<WorkspaceRetrievalConfig> {
    let name = "workspace_retrieval";
    if block.attributes.keys().any(|key| key != "enabled") {
        return Err(invalid(name, "attribute is not in the closed set enabled"));
    }
    let enabled = match block.attributes.get("enabled") {
        Some(Value::Bool(enabled)) => *enabled,
        None => return Err(invalid(name, "requires enabled = a bool")),
        _ => return Err(invalid(name, "enabled must be a bool")),
    };
    for child in &block.blocks {
        if child.name != "local_cpu" {
            return Err(invalid(name, "block is not in the closed set local_cpu"));
        }
        if !child.labels.is_empty() || !child.attributes.is_empty() || !child.blocks.is_empty() {
            return Err(invalid(name, "local_cpu requires an empty unlabeled block"));
        }
    }
    if block.blocks.len() > 1 {
        return Err(invalid(name, "duplicate local_cpu declaration"));
    }
    Ok(WorkspaceRetrievalConfig {
        enabled,
        local_cpu: !block.blocks.is_empty(),
    })
}

fn validate_source_fields(source: &str, name: &str) -> Result<()> {
    // The ACL AST erases duplicate fields, literal types, and some malformed delimiters.
    let mut tokens = Lexer::new(source)
        .tokenize()
        .into_iter()
        .map(|token| token.token)
        .filter(|token| !matches!(token, Token::Comment | Token::Newline | Token::Comma));
    let mut depth = 0usize;
    while let Some(token) = tokens.next() {
        match token {
            Token::Ident(value) if depth == 0 && value == name => {
                if tokens.next() != Some(Token::LeftBrace) {
                    return Err(invalid(name, "requires an unlabeled block"));
                }
                let mut fields = HashSet::new();
                loop {
                    let field = match tokens.next() {
                        Some(Token::RightBrace) => break,
                        Some(Token::Ident(field)) => field,
                        _ => {
                            return Err(invalid(
                                name,
                                "requires literal fields and a closing brace",
                            ))
                        }
                    };
                    if !fields.insert(field.clone()) {
                        return Err(invalid(name, "duplicate field in one declaration"));
                    }
                    match (name, field.as_str()) {
                        ("workspace_retrieval", "local_cpu") => {
                            if tokens.next() != Some(Token::LeftBrace)
                                || tokens.next() != Some(Token::RightBrace)
                            {
                                return Err(invalid(
                                    name,
                                    "local_cpu requires an empty unlabeled block",
                                ));
                            }
                        }
                        ("workspace_retrieval", "enabled") | ("tool_presentation", "mode") => {
                            if tokens.next() != Some(Token::Equal) {
                                return Err(invalid(name, "requires field = literal assignments"));
                            }
                            let literal = tokens.next();
                            let valid = if field == "enabled" {
                                matches!(literal, Some(Token::True | Token::False))
                            } else {
                                matches!(literal, Some(Token::String(_)))
                            };
                            if !valid {
                                return Err(invalid(
                                    name,
                                    "requires the declared literal field type",
                                ));
                            }
                        }
                        _ => return Err(invalid(name, "field is outside the declared closed set")),
                    }
                }
            }
            Token::LeftBrace | Token::LeftBracket | Token::LeftParen => depth += 1,
            Token::RightBrace | Token::RightBracket | Token::RightParen => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
