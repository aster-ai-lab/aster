use super::*;

#[tokio::test]
async fn embedded_reload_requires_constructor_authority_and_keeps_normal_reload_rules() {
    let mut config = GatewayConfig::default();
    config.entrypoints.clear();
    config.routers.clear();
    config.services.clear();
    let ordinary = Gateway::new(config.clone()).unwrap();
    ordinary.start().await.unwrap();
    assert!(matches!(
        ordinary.reload_embedded(config.clone()).await,
        Err(GatewayError::Config(_))
    ));
    ordinary.reload(config.clone()).await.unwrap();
    ordinary.shutdown().await;

    let embedded = Gateway::new_embedded(config.clone(), HashMap::new()).unwrap();
    embedded.start().await.unwrap();
    embedded.reload_embedded(config.clone()).await.unwrap();
    assert!(embedded.is_running());
    let mut invalid = config;
    invalid.mode = crate::config::OperatingMode::CloudManaged;
    assert!(embedded.reload_embedded(invalid).await.is_err());
    assert!(embedded.is_running());
    embedded.shutdown().await;
}

#[tokio::test]
async fn embedded_host_authority_does_not_allow_manual_managed_reload() {
    let mut config = GatewayConfig::default();
    config.entrypoints.clear();
    config.routers.clear();
    config.services.clear();
    config.mode = crate::config::OperatingMode::CloudManaged;
    config.managed.gateway_id = Some(uuid::Uuid::new_v4());
    for embedded in [false, true] {
        let gateway = if embedded {
            Gateway::new_embedded(config.clone(), HashMap::new()).unwrap()
        } else {
            Gateway::new(config.clone()).unwrap()
        };
        gateway.start().await.unwrap();
        assert!(matches!(
            gateway.reload(config.clone()).await,
            Err(GatewayError::Config(_))
        ));
        if embedded {
            gateway.reload_embedded(config.clone()).await.unwrap();
        } else {
            assert!(matches!(
                gateway.reload_embedded(config.clone()).await,
                Err(GatewayError::Config(_))
            ));
        }
        assert!(gateway.is_running());
        gateway.shutdown().await;
    }
}
