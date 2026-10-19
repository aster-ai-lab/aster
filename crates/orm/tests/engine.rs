use a3s_orm::{Engine, EngineFamily, EngineTransport};

#[test]
fn the_catalog_parses_only_its_closed_set() {
    assert_eq!(Engine::parse("sqlite").unwrap(), Engine::Sqlite);
    assert_eq!(Engine::parse(" postgres ").unwrap(), Engine::Postgres);
    assert_eq!(Engine::parse("mysql").unwrap().as_str(), "mysql");
    assert_eq!(Engine::parse("redis").unwrap().as_str(), "redis");
    let error = Engine::parse("not-an-engine").unwrap_err();
    let text = error.to_string();
    assert!(text.contains("sqlite"));
    assert!(text.contains("postgres"));
    assert!(text.contains("mysql"));
    assert!(!text.contains("not-an-engine"));
    let allowed: Vec<&str> = error.allowed().collect();
    assert_eq!(allowed.len(), 59);
    assert!(allowed.contains(&"sqlite"));
    assert!(allowed.contains(&"saphana"));
}

#[test]
fn only_sqlite_and_postgres_can_open_a_connection() {
    let profiles = Engine::profiles();
    let names: Vec<&str> = profiles
        .iter()
        .map(|profile| profile.engine.as_str())
        .collect();
    assert_eq!(names, ["sqlite", "postgres"]);
    assert!(profiles.iter().all(|profile| profile.runtime));
    assert!(!Engine::Mysql.profile().runtime);
    assert!(!Engine::Redis.profile().runtime);
    assert!(!Engine::Cockroach.profile().runtime);
    assert_eq!(
        Engine::Mysql.missing_driver_message(),
        "a3s-orm has no MySQL driver"
    );
    assert_eq!(
        Engine::Redis.missing_driver_message(),
        "a3s-orm has no redis driver"
    );
    assert_eq!(
        Engine::Saphana.missing_driver_message(),
        "a3s-orm has no saphana driver"
    );
}

#[test]
fn capabilities_come_from_the_dialect_that_already_compiles() {
    let postgres = Engine::Postgres.profile();
    assert_eq!(postgres.family, EngineFamily::Postgres);
    assert_eq!(postgres.transport, EngineTransport::Url);
    assert!(postgres.capabilities.returning);
    assert!(postgres.capabilities.information_schema);
    assert!(postgres.capabilities.row_lock);

    let sqlite = Engine::Sqlite.profile();
    assert_eq!(sqlite.transport, EngineTransport::File);
    assert!(sqlite.capabilities.returning);
    assert!(!sqlite.capabilities.information_schema);
    assert!(!sqlite.capabilities.row_lock);

    let mysql = Engine::Mysql.profile();
    assert_eq!(mysql.family, EngineFamily::Mysql);
    assert!(!mysql.capabilities.returning);
    assert!(!mysql.capabilities.on_conflict);
    assert!(mysql.capabilities.information_schema);

    let cockroach = Engine::Cockroach.profile();
    assert_eq!(cockroach.family, EngineFamily::Postgres);
    assert!(!cockroach.capabilities.returning);
    assert!(!cockroach.runtime);

    let mongo = Engine::Mongodb.profile();
    assert_eq!(mongo.family, EngineFamily::Document);
    assert!(!mongo.capabilities.information_schema);
}

#[test]
fn every_variant_has_one_row_and_names_do_not_repeat() {
    let engines = [
        Engine::Sqlite,
        Engine::Postgres,
        Engine::Mysql,
        Engine::Mariadb,
        Engine::Tidb,
        Engine::OceanbaseMysql,
        Engine::Goldendb,
        Engine::Doris,
        Engine::Selectdb,
        Engine::Starrocks,
        Engine::Manticore,
        Engine::Cockroach,
        Engine::Questdb,
        Engine::Redshift,
        Engine::Opengauss,
        Engine::Gaussdb,
        Engine::Kingbase,
        Engine::Vastbase,
        Engine::Highgo,
        Engine::Libsql,
        Engine::Turso,
        Engine::Rqlite,
        Engine::Duckdb,
        Engine::Clickhouse,
        Engine::Oracle,
        Engine::OceanbaseOracle,
        Engine::Dameng,
        Engine::Yashandb,
        Engine::Sundb,
        Engine::SqlServer,
        Engine::Db2,
        Engine::Informix,
        Engine::Snowflake,
        Engine::BigQuery,
        Engine::Trino,
        Engine::Hive,
        Engine::Databricks,
        Engine::Kylin,
        Engine::H2,
        Engine::Firebird,
        Engine::Exasol,
        Engine::Vertica,
        Engine::Databend,
        Engine::Tdengine,
        Engine::Gbase8a,
        Engine::Gbase8s,
        Engine::Xugudb,
        Engine::Iris,
        Engine::Redis,
        Engine::Etcd,
        Engine::Mongodb,
        Engine::Elasticsearch,
        Engine::Neo4j,
        Engine::Cassandra,
        Engine::Influxdb,
        Engine::Iotdb,
        Engine::Access,
        Engine::Teradata,
        Engine::Saphana,
    ];
    assert_eq!(engines.len(), 59);
    let mut names = Vec::new();
    for engine in engines {
        let name = engine.as_str();
        assert_eq!(Engine::parse(name).unwrap(), engine);
        assert!(!names.contains(&name), "{name}");
        names.push(name);
    }
}
