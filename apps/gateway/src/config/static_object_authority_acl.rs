use super::{bool_attr, config_error, required_string_attr, string_attr, u64_attr};
use crate::config::{StaticObjectAuthorityConfig, StaticObjectS3Config};
use crate::error::Result;
use aster_acl::Block;

pub(super) fn parse(block: &Block) -> Result<StaticObjectAuthorityConfig> {
    if !block.labels.is_empty()
        || !block.attributes.is_empty()
        || block.blocks.len() != 1
        || block.blocks[0].name != "s3"
    {
        return Err(config_error(
            "managed.static_objects requires exactly one unlabeled s3 { ... } provider block",
        ));
    }
    let s3 = &block.blocks[0];
    if !s3.labels.is_empty() || !s3.blocks.is_empty() {
        return Err(config_error(
            "managed.static_objects.s3 does not accept labels or nested blocks",
        ));
    }
    for field in s3.attributes.keys() {
        if ![
            "endpoint",
            "region",
            "bucket",
            "prefix",
            "access_key_id_env",
            "secret_access_key_env",
            "session_token_env",
            "ca_file",
            "allow_http",
            "virtual_hosted_style",
            "request_timeout_ms",
            "connect_timeout_ms",
        ]
        .contains(&field.as_str())
        {
            return Err(config_error(format!(
                "Unknown managed.static_objects.s3 field '{field}'"
            )));
        }
    }
    let config = StaticObjectAuthorityConfig {
        s3: StaticObjectS3Config {
            endpoint: required_string_attr(s3, &["endpoint"])?,
            region: required_string_attr(s3, &["region"])?,
            bucket: required_string_attr(s3, &["bucket"])?,
            prefix: required_string_attr(s3, &["prefix"])?,
            access_key_id_env: required_string_attr(s3, &["access_key_id_env"])?,
            secret_access_key_env: required_string_attr(s3, &["secret_access_key_env"])?,
            session_token_env: string_attr(s3, &["session_token_env"])?,
            ca_file: string_attr(s3, &["ca_file"])?.map(std::path::PathBuf::from),
            allow_http: bool_attr(s3, &["allow_http"])?.unwrap_or(false),
            virtual_hosted_style: bool_attr(s3, &["virtual_hosted_style"])?.unwrap_or(false),
            request_timeout_ms: u64_attr(s3, &["request_timeout_ms"])?.unwrap_or(10_000),
            connect_timeout_ms: u64_attr(s3, &["connect_timeout_ms"])?.unwrap_or(3_000),
        },
    };
    config.validate()?;
    Ok(config)
}
