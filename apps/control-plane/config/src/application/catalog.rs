//! Built-in bootstrap + runtime config catalogs for console settings.

use serde::Serialize;
use serde_json::{json, Value};

use crate::bootstrap::{discovery_group, BootstrapGroup, BootstrapItem};
use crate::config::ConfigServerConfig;

use super::schema::{EnumOption, FieldSchema, IntegerUnit, Pattern, ValueSchema};

#[derive(Debug, Clone, Serialize)]
pub struct CatalogGroupDto {
    pub id: String,
    pub label: String,
    pub items: Vec<CatalogItemDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogItemDto {
    pub key: String,
    pub label: String,
    pub description: String,
    pub value: Value,
    pub sensitive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
    /// Whether the value is present in the config center (runtime only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured: Option<bool>,
    /// Shape of `value` (runtime only); the console renders its form from it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<ValueSchema>,
}

#[derive(Debug, Clone)]
pub struct BuiltinDef {
    pub namespace: &'static str,
    pub key: &'static str,
    pub group: &'static str,
    pub group_label: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub default: Value,
    pub sensitive: bool,
    pub schema: ValueSchema,
}

/// Console page that edits a builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Runtime,
    Login,
}

impl Surface {
    pub fn parse(raw: Option<&str>) -> Option<Self> {
        match raw.map(str::trim) {
            None | Some("") | Some("runtime") => Some(Self::Runtime),
            Some("login") => Some(Self::Login),
            Some(_) => None,
        }
    }
}

pub fn builtins_on(surface: Surface) -> Vec<BuiltinDef> {
    match surface {
        Surface::Runtime => runtime_builtins(),
        Surface::Login => login_builtins(),
    }
}

pub fn all_builtins() -> Vec<BuiltinDef> {
    let mut defs = runtime_builtins();
    defs.extend(login_builtins());
    defs
}

/// The builtin defined for `namespace` / `key`, if any.
pub fn builtin(namespace: &str, key: &str) -> Option<BuiltinDef> {
    all_builtins()
        .into_iter()
        .find(|def| def.namespace == namespace && def.key == key)
}

/// OAuth sign-in providers, edited on the console's 授权登录 page. Each provider is one
/// value at `auth.<provider>`; every one shares `enabled`, `callback_domain` and
/// `account_policy` so the console can list them uniformly.
pub fn login_builtins() -> Vec<BuiltinDef> {
    vec![
        wechat_login(),
        feishu_login(),
        dingtalk_login(),
        wecom_login(),
    ]
}

fn enabled_field(label: &'static str) -> FieldSchema {
    FieldSchema::new("enabled", label, ValueSchema::Boolean)
}

fn callback_domain_field(console: &'static str) -> FieldSchema {
    FieldSchema::new(
        "callback_domain",
        "授权回调域",
        ValueSchema::Text {
            max_len: 253,
            multiline: false,
            placeholder: Some("console.example.com"),
            pattern: Some(Pattern::new(
                "^(?:[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?\\.)+[A-Za-z]{2,63}$",
                "请填写域名本身，不含 http(s)://、端口与路径",
            )),
        },
    )
    .describe(console)
    .required_when("enabled")
}

fn account_policy_field(bind_hint: &'static str) -> FieldSchema {
    FieldSchema::new(
        "account_policy",
        "账号策略",
        ValueSchema::Enum {
            options: vec![
                EnumOption {
                    value: "bind_only",
                    label: "仅已绑定账号",
                },
                EnumOption {
                    value: "auto_create",
                    label: "首次登录自动注册",
                },
            ],
        },
    )
    .describe(bind_hint)
}

fn plain_text(max_len: usize, placeholder: &'static str, pattern: Option<Pattern>) -> ValueSchema {
    ValueSchema::Text {
        max_len,
        multiline: false,
        placeholder: Some(placeholder),
        pattern,
    }
}

fn login_def(
    key: &'static str,
    group: &'static str,
    label: &'static str,
    description: &'static str,
    default: Value,
    fields: Vec<FieldSchema>,
) -> BuiltinDef {
    BuiltinDef {
        namespace: "default",
        key,
        group,
        group_label: label,
        label,
        description,
        default,
        sensitive: true,
        schema: ValueSchema::Object { fields },
    }
}

fn wechat_login() -> BuiltinDef {
    login_def(
        "auth.wechat",
        "wechat",
        "微信",
        "微信开放平台网站应用（snsapi_login），用户用微信扫码登录控制台与前台。",
        json!({
            "enabled": false,
            "app_id": "",
            "app_secret": "",
            "callback_domain": "",
            "display_mode": "embedded",
            "qr_style": "black",
            "account_policy": "bind_only",
            "use_unionid": true
        }),
        vec![
            enabled_field("启用微信登录"),
            FieldSchema::new(
                "app_id",
                "AppID",
                plain_text(
                    18,
                    "wx0123456789abcdef",
                    Some(Pattern::new(
                        "^wx[0-9a-f]{16}$",
                        "AppID 应为 wx 开头、后接 16 位小写十六进制字符",
                    )),
                ),
            )
            .describe("微信开放平台 › 管理中心 › 网站应用 › 应用详情中的 AppID。")
            .required_when("enabled"),
            FieldSchema::new(
                "app_secret",
                "AppSecret",
                ValueSchema::Secret {
                    max_len: 32,
                    pattern: Some(Pattern::new(
                        "^[0-9A-Za-z]{32}$",
                        "AppSecret 应为 32 位字母或数字",
                    )),
                },
            )
            .describe("仅用于服务端换取 access_token，不会下发到浏览器。")
            .required_when("enabled"),
            callback_domain_field("须与开放平台网站应用的“授权回调域”完全一致。"),
            FieldSchema::new(
                "display_mode",
                "展示方式",
                ValueSchema::Enum {
                    options: vec![
                        EnumOption {
                            value: "embedded",
                            label: "内嵌二维码",
                        },
                        EnumOption {
                            value: "redirect",
                            label: "跳转授权页",
                        },
                    ],
                },
            )
            .describe("内嵌：登录页直接显示二维码；跳转：打开微信授权页扫码后返回。"),
            FieldSchema::new(
                "qr_style",
                "二维码样式",
                ValueSchema::Enum {
                    options: vec![
                        EnumOption {
                            value: "black",
                            label: "深色",
                        },
                        EnumOption {
                            value: "white",
                            label: "浅色",
                        },
                    ],
                },
            )
            .describe("仅内嵌模式生效；浅色适合深色背景的登录页。"),
            account_policy_field(
                "仅已绑定：用户需先在个人设置中绑定微信；自动注册：未绑定的微信首次扫码即创建账号。",
            ),
            FieldSchema::new("use_unionid", "按 UnionID 识别用户", ValueSchema::Boolean)
                .describe("同一开放平台主体下的公众号、小程序与网站共享身份；需应用已绑定到开放平台账号。"),
        ],
    )
}

fn feishu_login() -> BuiltinDef {
    login_def(
        "auth.feishu",
        "feishu",
        "飞书",
        "飞书开放平台企业自建应用的网页登录，成员用飞书账号登录控制台与前台。",
        json!({
            "enabled": false,
            "app_id": "",
            "app_secret": "",
            "callback_domain": "",
            "account_policy": "bind_only",
            "tenant_key": ""
        }),
        vec![
            enabled_field("启用飞书登录"),
            FieldSchema::new(
                "app_id",
                "App ID",
                plain_text(
                    40,
                    "cli_a1b2c3d4e5f6a7b8",
                    Some(Pattern::new(
                        "^cli_[0-9a-z]{16}$",
                        "App ID 应为 cli_ 开头、后接 16 位小写字母或数字",
                    )),
                ),
            )
            .describe("飞书开放平台 › 开发者后台 › 凭证与基础信息中的 App ID。")
            .required_when("enabled"),
            FieldSchema::new(
                "app_secret",
                "App Secret",
                ValueSchema::Secret {
                    max_len: 64,
                    pattern: Some(Pattern::new(
                        "^[0-9A-Za-z]{32}$",
                        "App Secret 应为 32 位字母或数字",
                    )),
                },
            )
            .describe("仅用于服务端换取 user_access_token，不会下发到浏览器。")
            .required_when("enabled"),
            callback_domain_field("须与应用“安全设置 › 重定向 URL”中登记的域名一致。"),
            account_policy_field(
                "仅已绑定：成员需先在个人设置中绑定飞书；自动注册：未绑定的飞书账号首次登录即创建账号。",
            ),
            FieldSchema::new(
                "tenant_key",
                "限定企业",
                plain_text(64, "留空表示不限", None),
            )
            .describe("填写飞书企业的 tenant_key 后，只允许该企业的成员登录。"),
        ],
    )
}

fn dingtalk_login() -> BuiltinDef {
    login_def(
        "auth.dingtalk",
        "dingtalk",
        "钉钉",
        "钉钉开放平台应用的扫码登录（OAuth 2.0），成员用钉钉账号登录控制台与前台。",
        json!({
            "enabled": false,
            "client_id": "",
            "client_secret": "",
            "callback_domain": "",
            "account_policy": "bind_only",
            "corp_id": ""
        }),
        vec![
            enabled_field("启用钉钉登录"),
            FieldSchema::new(
                "client_id",
                "Client ID",
                plain_text(
                    64,
                    "dingxxxxxxxxxxxxxxxx",
                    Some(Pattern::new(
                        "^[0-9A-Za-z_-]{8,64}$",
                        "Client ID 只含字母、数字、下划线或连字符",
                    )),
                ),
            )
            .describe("钉钉开放平台 › 应用开发 › 凭证与基础信息中的 Client ID（原 AppKey）。")
            .required_when("enabled"),
            FieldSchema::new(
                "client_secret",
                "Client Secret",
                ValueSchema::Secret {
                    max_len: 128,
                    pattern: Some(Pattern::new(
                        "^[0-9A-Za-z_-]{16,128}$",
                        "Client Secret 只含字母、数字、下划线或连字符",
                    )),
                },
            )
            .describe("仅用于服务端换取用户 token，不会下发到浏览器。")
            .required_when("enabled"),
            callback_domain_field("须与应用“登录与分享 › 回调域名”中登记的域名一致。"),
            account_policy_field(
                "仅已绑定：成员需先在个人设置中绑定钉钉；自动注册：未绑定的钉钉账号首次登录即创建账号。",
            ),
            FieldSchema::new(
                "corp_id",
                "限定企业",
                plain_text(64, "留空表示不限", None),
            )
            .describe("填写企业 CorpId 后，只允许该企业的成员登录。"),
        ],
    )
}

fn wecom_login() -> BuiltinDef {
    login_def(
        "auth.wecom",
        "wecom",
        "企业微信",
        "企业微信自建应用的网页登录，成员用企业微信扫码登录控制台与前台。",
        json!({
            "enabled": false,
            "corp_id": "",
            "agent_id": "",
            "secret": "",
            "callback_domain": "",
            "account_policy": "bind_only"
        }),
        vec![
            enabled_field("启用企业微信登录"),
            FieldSchema::new(
                "corp_id",
                "企业 ID",
                plain_text(
                    18,
                    "ww0123456789abcdef",
                    Some(Pattern::new(
                        "^ww[0-9a-f]{16}$",
                        "企业 ID 应为 ww 开头、后接 16 位小写十六进制字符",
                    )),
                ),
            )
            .describe("企业微信管理后台 › 我的企业 › 企业信息中的企业 ID。")
            .required_when("enabled"),
            FieldSchema::new(
                "agent_id",
                "AgentId",
                plain_text(
                    10,
                    "1000002",
                    Some(Pattern::new("^[0-9]{1,10}$", "AgentId 应为数字")),
                ),
            )
            .describe("应用管理 › 自建应用详情中的 AgentId。")
            .required_when("enabled"),
            FieldSchema::new(
                "secret",
                "Secret",
                ValueSchema::Secret {
                    max_len: 64,
                    pattern: Some(Pattern::new(
                        "^[0-9A-Za-z_-]{43}$",
                        "Secret 应为 43 位字母、数字、下划线或连字符",
                    )),
                },
            )
            .describe("仅用于服务端获取 access_token，不会下发到浏览器。")
            .required_when("enabled"),
            callback_domain_field("须与应用“企业微信授权登录 › 授权回调域”完全一致。"),
            account_policy_field(
                "仅已绑定：成员需先在个人设置中绑定企业微信；自动注册：本企业成员首次登录即创建账号。",
            ),
        ],
    )
}

pub fn runtime_builtins() -> Vec<BuiltinDef> {
    vec![
        BuiltinDef {
            namespace: "default",
            key: "feature.maintenance",
            group: "feature",
            group_label: "功能开关",
            label: "维护模式",
            description: "开启后控制台与公开 API 进入只读/维护提示。",
            default: json!({ "enabled": false, "message": "" }),
            sensitive: false,
            schema: ValueSchema::Object {
                fields: vec![
                    FieldSchema::new("enabled", "开启维护模式", ValueSchema::Boolean),
                    FieldSchema::new(
                        "message",
                        "维护提示",
                        ValueSchema::Text {
                            max_len: 280,
                            multiline: true,
                            placeholder: Some("例如：系统升级中，预计 30 分钟后恢复。"),
                            pattern: None,
                        },
                    )
                    .describe("展示给控制台与 API 调用方；留空使用默认文案。")
                    .visible_when("enabled"),
                ],
            },
        },
        BuiltinDef {
            namespace: "default",
            key: "feature.registration",
            group: "feature",
            group_label: "功能开关",
            label: "开放注册",
            description: "是否允许前台 Member 自助注册。",
            default: json!({ "enabled": true }),
            sensitive: false,
            schema: ValueSchema::Object {
                fields: vec![FieldSchema::new(
                    "enabled",
                    "允许自助注册",
                    ValueSchema::Boolean,
                )],
            },
        },
        BuiltinDef {
            namespace: "default",
            key: "feature.demo",
            group: "feature",
            group_label: "功能开关",
            label: "演示特性",
            description: "演示用开关；生产环境可关闭。",
            default: json!({ "enabled": true, "quota": 3 }),
            sensitive: false,
            schema: ValueSchema::Object {
                fields: vec![
                    FieldSchema::new("enabled", "启用演示特性", ValueSchema::Boolean),
                    FieldSchema::new(
                        "quota",
                        "演示配额",
                        ValueSchema::integer(0, 1000, IntegerUnit::Items),
                    )
                    .describe("每个组织可使用的演示次数。")
                    .visible_when("enabled"),
                ],
            },
        },
        BuiltinDef {
            namespace: "default",
            key: "limits.request_body_mb",
            group: "limits",
            group_label: "配额与限制",
            label: "请求体上限",
            description: "单次请求 body 最大体积（MB）。",
            default: json!(16),
            sensitive: false,
            schema: ValueSchema::integer(1, 1024, IntegerUnit::Megabytes),
        },
        BuiltinDef {
            namespace: "default",
            key: "limits.list_page_size",
            group: "limits",
            group_label: "配额与限制",
            label: "列表默认分页",
            description: "列表接口默认 page size。",
            default: json!(20),
            sensitive: false,
            schema: ValueSchema::integer(1, 500, IntegerUnit::Items),
        },
        BuiltinDef {
            namespace: "default",
            key: "limits.upload_mb",
            group: "limits",
            group_label: "配额与限制",
            label: "上传大小上限",
            description: "对象存储单文件上传上限（MB）。",
            default: json!(64),
            sensitive: false,
            schema: ValueSchema::integer(1, 10240, IntegerUnit::Megabytes),
        },
        BuiltinDef {
            namespace: "default",
            key: "session.member_ttl_hours",
            group: "session",
            group_label: "会话",
            label: "Member 会话时长",
            description: "前台 Member token 有效小时数。",
            default: json!(720),
            sensitive: false,
            schema: ValueSchema::integer(1, 8760, IntegerUnit::Hours),
        },
        BuiltinDef {
            namespace: "default",
            key: "session.user_ttl_hours",
            group: "session",
            group_label: "会话",
            label: "User 会话时长",
            description: "后台 User token 有效小时数。",
            default: json!(168),
            sensitive: false,
            schema: ValueSchema::integer(1, 8760, IntegerUnit::Hours),
        },
        BuiltinDef {
            namespace: "default",
            key: "notify.email_enabled",
            group: "notify",
            group_label: "通知",
            label: "邮件通知",
            description: "是否启用事务/告警邮件投递。",
            default: json!({ "enabled": false }),
            sensitive: false,
            schema: ValueSchema::Object {
                fields: vec![FieldSchema::new(
                    "enabled",
                    "启用邮件通知",
                    ValueSchema::Boolean,
                )],
            },
        },
        BuiltinDef {
            namespace: "default",
            key: "model_hub.huggingface.token",
            group: "model_hub",
            group_label: "模型 Hub",
            label: "Hugging Face Token",
            description: "可选；用于私有模型与更高速率限制。",
            default: json!(""),
            sensitive: true,
            schema: ValueSchema::Secret {
                max_len: 512,
                pattern: None,
            },
        },
        BuiltinDef {
            namespace: "default",
            key: "model_hub.huggingface.endpoint",
            group: "model_hub",
            group_label: "模型 Hub",
            label: "Hugging Face Endpoint",
            description:
                "HF API/下载基址；国内默认 https://hf-mirror.com，可改回 https://huggingface.co。",
            default: json!("https://hf-mirror.com"),
            sensitive: false,
            schema: ValueSchema::url("https://huggingface.co"),
        },
        BuiltinDef {
            namespace: "default",
            key: "model_hub.modelscope.token",
            group: "model_hub",
            group_label: "模型 Hub",
            label: "ModelScope Token",
            description: "可选；用于私有模型与更高速率限制。",
            default: json!(""),
            sensitive: true,
            schema: ValueSchema::Secret {
                max_len: 512,
                pattern: None,
            },
        },
        BuiltinDef {
            namespace: "default",
            key: "model_hub.default",
            group: "model_hub",
            group_label: "模型 Hub",
            label: "默认 Hub",
            description: "控制台默认选中的模型来源：huggingface 或 modelscope。",
            default: json!("huggingface"),
            sensitive: false,
            schema: ValueSchema::Enum {
                options: vec![
                    EnumOption {
                        value: "huggingface",
                        label: "Hugging Face",
                    },
                    EnumOption {
                        value: "modelscope",
                        label: "ModelScope",
                    },
                ],
            },
        },
    ]
}

pub fn bootstrap_catalog(config: &ConfigServerConfig) -> Vec<BootstrapGroup> {
    let service_name = std::env::var("ASTER_SERVICE_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "aster-config".into());
    vec![
        BootstrapGroup::new(
            "http",
            "HTTP",
            vec![
                BootstrapItem::env(
                    &["ASTER_CONFIG_LISTEN"],
                    "监听",
                    "完整监听地址；设置后覆盖主机与端口",
                    config.listen(),
                ),
                BootstrapItem::env(
                    &["ASTER_CONFIG_HOST", "HOST"],
                    "主机",
                    "HTTP 绑定主机",
                    config.host.to_string(),
                ),
                BootstrapItem::env(
                    &["ASTER_CONFIG_PORT", "PORT"],
                    "端口",
                    "HTTP 监听端口",
                    config.port,
                ),
            ],
        ),
        BootstrapGroup::new(
            "database",
            "PostgreSQL",
            vec![
                BootstrapItem::env(
                    &["ASTER_CONFIG_DATABASE_URL", "DATABASE_URL"],
                    "连接串",
                    "配置项的权威数据源",
                    config.database_url.clone(),
                )
                .url()
                .required(),
                BootstrapItem::env(
                    &["ASTER_CONFIG_DATABASE_POOL_SIZE", "DATABASE_POOL_SIZE"],
                    "连接池",
                    "数据库连接池大小",
                    config.database_pool_size,
                ),
            ],
        ),
        BootstrapGroup::new(
            "iggy",
            "Iggy",
            vec![
                BootstrapItem::env(
                    &["ASTER_CONFIG_IGGY_ADDRESS", "IGGY_ADDRESS"],
                    "地址",
                    "配置变更广播：stream aster_config，topic config_changed",
                    config.iggy_address.clone(),
                ),
                BootstrapItem::env(
                    &["ASTER_CONFIG_IGGY_USERNAME", "IGGY_ROOT_USERNAME"],
                    "用户名",
                    "Iggy 登录用户",
                    config.iggy_username.clone(),
                ),
                BootstrapItem::env(
                    &["ASTER_CONFIG_IGGY_PASSWORD", "IGGY_ROOT_PASSWORD"],
                    "密码",
                    "Iggy 登录密码；未设置时使用开发默认值",
                    config.iggy_password.clone(),
                )
                .secret(),
            ],
        ),
        discovery_group(&service_name, config.port),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_default_matches_its_schema() {
        for def in all_builtins() {
            assert_eq!(def.schema.check(&def.default), Ok(()), "{}", def.key);
        }
    }

    #[test]
    fn every_login_provider_shares_the_uniform_fields() {
        let defs = login_builtins();
        assert_eq!(
            defs.iter().map(|def| def.key).collect::<Vec<_>>(),
            ["auth.wechat", "auth.feishu", "auth.dingtalk", "auth.wecom"]
        );
        for def in defs {
            let ValueSchema::Object { fields } = &def.schema else {
                panic!("{} is not an object", def.key);
            };
            for name in ["enabled", "callback_domain", "account_policy"] {
                assert!(
                    fields.iter().any(|field| field.name == name),
                    "{} lacks {name}",
                    def.key
                );
            }
            assert!(def.sensitive, "{}", def.key);
        }
    }

    #[test]
    fn builtins_are_found_by_namespace_and_key() {
        assert!(builtin("default", "limits.upload_mb").is_some());
        assert!(builtin("default", "auth.wechat").is_some());
        assert!(builtin("default", "auth.feishu").is_some());
        assert!(builtin("other", "limits.upload_mb").is_none());
        assert!(builtin("default", "custom.key").is_none());
        // The system page owns default/console.brand. It stays out of the runtime catalog.
        assert!(builtin("default", "console.brand").is_none());
        assert!(runtime_builtins().iter().all(|def| def.group != "console"));
        assert!(builtins_on(Surface::Runtime)
            .iter()
            .all(|def| def.key != "auth.wechat"));
        assert_eq!(Surface::parse(Some("login")), Some(Surface::Login));
        assert_eq!(Surface::parse(None), Some(Surface::Runtime));
        assert_eq!(Surface::parse(Some("other")), None);
    }
}
