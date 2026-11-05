//! Model-addressed Gemini requests. URL metadata stays separate from body bytes.

use super::openai_request::OpenAiRequestError;
use http::Uri;

/// Normalize Google's SDK credentials before ordinary principal verification.
pub(crate) fn normalize_credentials(
    headers: &mut http::HeaderMap,
    uri: &mut Uri,
) -> Result<(), super::InferenceAccessError> {
    use super::InferenceAccessError::Unauthorized;
    let mut google_keys = headers.get_all("x-goog-api-key").iter();
    let google_key = google_keys.next();
    if google_keys.next().is_some() {
        return Err(Unauthorized);
    }
    let existing =
        if headers.contains_key(http::header::AUTHORIZATION) || headers.contains_key("x-api-key") {
            Some(super::principal::principal_token(headers)?)
        } else {
            None
        };
    let query =
        url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()).collect::<Vec<_>>();
    let mut keys = query
        .iter()
        .filter(|(name, _)| name == "key")
        .map(|(_, value)| value.as_ref());
    let query_key = keys.next();
    if keys.next().is_some() {
        return Err(Unauthorized);
    }
    let google_key = google_key
        .map(|key| key.to_str().map_err(|_| Unauthorized))
        .transpose()?;
    if let (Some(header), Some(query)) = (google_key, query_key) {
        if header != query {
            return Err(Unauthorized);
        }
    }
    let mut normalized = headers.clone();
    if let Some(key) = google_key.or(query_key) {
        if existing.is_some_and(|existing| existing != key) {
            return Err(Unauthorized);
        }
        normalized.insert("x-api-key", key.parse().map_err(|_| Unauthorized)?);
    }
    let token = super::principal::principal_token(&normalized)?;
    let bearer = format!("Bearer {token}")
        .parse()
        .map_err(|_| Unauthorized)?;
    if query_key.is_some() {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(query.iter().filter(|(name, _)| name != "key"))
            .finish();
        let path = if query.is_empty() {
            uri.path().to_owned()
        } else {
            format!("{}?{query}", uri.path())
        };
        let mut parts = uri.clone().into_parts();
        parts.path_and_query = Some(path.parse().map_err(|_| Unauthorized)?);
        *uri = Uri::from_parts(parts).map_err(|_| Unauthorized)?;
    }
    headers.remove("x-goog-api-key");
    headers.remove("x-api-key");
    headers.insert(http::header::AUTHORIZATION, bearer);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeminiMethod {
    Generate,
    Stream,
    Count,
}

impl GeminiMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Generate => "generateContent",
            Self::Stream => "streamGenerateContent",
            Self::Count => "countTokens",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeminiModelCall {
    model: String,
    version: &'static str,
    method: GeminiMethod,
}

impl GeminiModelCall {
    pub(crate) fn parse(path: &str) -> Result<Self, OpenAiRequestError> {
        let (version, call) = if let Some(call) = path.strip_prefix("/v1beta/models/") {
            ("v1beta", call)
        } else if let Some(call) = path.strip_prefix("/v1/models/") {
            ("v1", call)
        } else {
            return Err(OpenAiRequestError::InvalidModel);
        };
        let (model, method) = call
            .rsplit_once(':')
            .ok_or(OpenAiRequestError::InvalidModel)?;
        let method = match method {
            "generateContent" => GeminiMethod::Generate,
            "streamGenerateContent" => GeminiMethod::Stream,
            "countTokens" => GeminiMethod::Count,
            _ => return Err(OpenAiRequestError::InvalidModel),
        };
        let model = decode_model(model)?;
        if !super::openai_request::valid_model_alias(&model)
            || model.contains(['\\', '?', '#'])
            || model.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(OpenAiRequestError::InvalidModel);
        }
        Ok(Self {
            model,
            version,
            method,
        })
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn streaming(&self) -> bool {
        self.method == GeminiMethod::Stream
    }

    pub(crate) fn counting(&self) -> bool {
        self.method == GeminiMethod::Count
    }

    pub(crate) fn upstream_uri(&self, model: &str, original: &Uri) -> Result<Uri, http::Error> {
        // URL's path API escapes API identifiers without changing their UTF-8 or slash identity.
        let mut url = url::Url::parse("http://localhost/").expect("static HTTP URL");
        url.path_segments_mut()
            .expect("static HTTP URL has path segments")
            .extend([
                self.version,
                "models",
                &format!("{model}:{}", self.method.as_str()),
            ]);
        let query = original
            .query()
            .map(|query| url::form_urlencoded::parse(query.as_bytes()))
            .into_iter()
            .flatten()
            .filter(|(name, _)| name != "key" && name != "alt")
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        if self.streaming() || !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            pairs.extend_pairs(query);
            if self.streaming() {
                pairs.append_pair("alt", "sse");
            }
        }
        Uri::builder()
            .path_and_query(&url[url::Position::BeforePath..])
            .build()
    }
}

fn decode_model(raw: &str) -> Result<String, OpenAiRequestError> {
    let mut decoded = Vec::with_capacity(raw.len());
    let mut bytes = raw.bytes();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let hi = bytes.next().and_then(|byte| char::from(byte).to_digit(16));
            let lo = bytes.next().and_then(|byte| char::from(byte).to_digit(16));
            let (Some(hi), Some(lo)) = (hi, lo) else {
                return Err(OpenAiRequestError::InvalidModel);
            };
            u8::try_from(hi * 16 + lo).map_err(|_| OpenAiRequestError::InvalidModel)?
        } else {
            byte
        });
    }
    String::from_utf8(decoded).map_err(|_| OpenAiRequestError::InvalidModel)
}

pub(crate) fn models_body(
    models: &[String],
    uri: &Uri,
) -> Result<bytes::Bytes, OpenAiRequestError> {
    use sha2::{Digest, Sha256};
    let mut page_size = None;
    let mut page_token = None;
    let mut alt = false;
    for (name, value) in url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes()) {
        match name.as_ref() {
            "pageSize" if page_size.is_none() => {
                let size = value
                    .parse::<usize>()
                    .map_err(|_| OpenAiRequestError::InvalidQuery)?;
                page_size = Some(if size == 0 { 50 } else { size.min(1000) });
            }
            "pageToken" if page_token.is_none() => page_token = Some(value.into_owned()),
            "alt" if !alt && value == "json" => alt = true,
            _ => return Err(OpenAiRequestError::InvalidQuery),
        }
    }
    let size = page_size.unwrap_or(50);
    let mut digest = Sha256::new();
    digest.update((size as u64).to_le_bytes());
    for model in models {
        digest.update(model.as_bytes());
        digest.update([0]);
    }
    let fingerprint = format!("{:x}", digest.finalize());
    let offset = match page_token {
        None => 0,
        Some(token) => {
            let (offset, expected) = token
                .split_once('.')
                .ok_or(OpenAiRequestError::InvalidQuery)?;
            if expected != fingerprint {
                return Err(OpenAiRequestError::InvalidQuery);
            }
            offset
                .parse::<usize>()
                .map_err(|_| OpenAiRequestError::InvalidQuery)?
        }
    };
    if offset > models.len() {
        return Err(OpenAiRequestError::InvalidQuery);
    }
    let end = offset.saturating_add(size).min(models.len());
    let mut document = serde_json::json!({"models": models[offset..end].iter().map(|model| serde_json::json!({
        "name": format!("models/{model}"),
        "baseModelId": model,
        "displayName": model,
        "supportedGenerationMethods": ["generateContent", "streamGenerateContent", "countTokens"]
    })).collect::<Vec<_>>()});
    if end < models.len() {
        document["nextPageToken"] = serde_json::json!(format!("{end}.{fingerprint}"));
    }
    serde_json::to_vec(&document)
        .map(bytes::Bytes::from)
        .map_err(|_| OpenAiRequestError::InvalidJson)
}

#[cfg(test)]
#[path = "gemini_request_tests.rs"]
mod tests;
