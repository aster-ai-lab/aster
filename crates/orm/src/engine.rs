//! Closed catalog of data sources. A name parses or the error lists the set.
//! `profiles` is the subset whose runtime driver can open a connection.

use std::fmt;

use crate::compiler::{Dialect, MysqlDialect, PostgresDialect, SqliteDialect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineFamily {
    Postgres,
    Mysql,
    Sqlite,
    Oracle,
    SqlServer,
    ClickHouse,
    DuckDb,
    Trino,
    BigQuery,
    Snowflake,
    Document,
    KeyValue,
    Graph,
    WideColumn,
    Search,
    Timeseries,
    /// Protocol is not measured yet. Capabilities stay empty.
    Other,
}

impl EngineFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
            Self::Sqlite => "sqlite",
            Self::Oracle => "oracle",
            Self::SqlServer => "sqlserver",
            Self::ClickHouse => "clickhouse",
            Self::DuckDb => "duckdb",
            Self::Trino => "trino",
            Self::BigQuery => "bigquery",
            Self::Snowflake => "snowflake",
            Self::Document => "document",
            Self::KeyValue => "key-value",
            Self::Graph => "graph",
            Self::WideColumn => "wide-column",
            Self::Search => "search",
            Self::Timeseries => "timeseries",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineTransport {
    File,
    Url,
    Host,
}

impl EngineTransport {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Url => "url",
            Self::Host => "host",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineCapabilities {
    pub returning: bool,
    pub on_conflict: bool,
    pub update_from: bool,
    pub row_lock: bool,
    pub table_lock: bool,
    pub information_schema: bool,
}

impl EngineCapabilities {
    const fn empty() -> Self {
        Self {
            returning: false,
            on_conflict: false,
            update_from: false,
            row_lock: false,
            table_lock: false,
            information_schema: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DialectKind {
    Postgres,
    Sqlite,
    Mysql,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineProfile {
    pub engine: Engine,
    pub family: EngineFamily,
    pub transport: EngineTransport,
    pub capabilities: EngineCapabilities,
    pub runtime: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Sqlite,
    Postgres,
    Mysql,
    Mariadb,
    Tidb,
    OceanbaseMysql,
    Goldendb,
    Doris,
    Selectdb,
    Starrocks,
    Manticore,
    Cockroach,
    Questdb,
    Redshift,
    Opengauss,
    Gaussdb,
    Kingbase,
    Vastbase,
    Highgo,
    Libsql,
    Turso,
    Rqlite,
    Duckdb,
    Clickhouse,
    Oracle,
    OceanbaseOracle,
    Dameng,
    Yashandb,
    Sundb,
    SqlServer,
    Db2,
    Informix,
    Snowflake,
    BigQuery,
    Trino,
    Hive,
    Databricks,
    Kylin,
    H2,
    Firebird,
    Exasol,
    Vertica,
    Databend,
    Tdengine,
    Gbase8a,
    Gbase8s,
    Xugudb,
    Iris,
    Redis,
    Etcd,
    Mongodb,
    Elasticsearch,
    Neo4j,
    Cassandra,
    Influxdb,
    Iotdb,
    Access,
    Teradata,
    Saphana,
}

struct Spec {
    engine: Engine,
    name: &'static str,
    family: EngineFamily,
    transport: EngineTransport,
    dialect: DialectKind,
    runtime: bool,
}

const SPECS: &[Spec] = &[
    spec(
        Engine::Sqlite,
        "sqlite",
        EngineFamily::Sqlite,
        EngineTransport::File,
        DialectKind::Sqlite,
        true,
    ),
    spec(
        Engine::Postgres,
        "postgres",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::Postgres,
        true,
    ),
    spec(
        Engine::Mysql,
        "mysql",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::Mysql,
        false,
    ),
    spec(
        Engine::Mariadb,
        "mariadb",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Tidb,
        "tidb",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::OceanbaseMysql,
        "oceanbase-mysql",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Goldendb,
        "goldendb",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Doris,
        "doris",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Selectdb,
        "selectdb",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Starrocks,
        "starrocks",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Manticore,
        "manticore",
        EngineFamily::Mysql,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Cockroach,
        "cockroach",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Questdb,
        "questdb",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Redshift,
        "redshift",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Opengauss,
        "opengauss",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Gaussdb,
        "gaussdb",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Kingbase,
        "kingbase",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Vastbase,
        "vastbase",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Highgo,
        "highgo",
        EngineFamily::Postgres,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Libsql,
        "libsql",
        EngineFamily::Sqlite,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Turso,
        "turso",
        EngineFamily::Sqlite,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Rqlite,
        "rqlite",
        EngineFamily::Sqlite,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Duckdb,
        "duckdb",
        EngineFamily::DuckDb,
        EngineTransport::File,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Clickhouse,
        "clickhouse",
        EngineFamily::ClickHouse,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Oracle,
        "oracle",
        EngineFamily::Oracle,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::OceanbaseOracle,
        "oceanbase-oracle",
        EngineFamily::Oracle,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Dameng,
        "dameng",
        EngineFamily::Oracle,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Yashandb,
        "yashandb",
        EngineFamily::Oracle,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Sundb,
        "sundb",
        EngineFamily::Oracle,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::SqlServer,
        "sqlserver",
        EngineFamily::SqlServer,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Db2,
        "db2",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Informix,
        "informix",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Snowflake,
        "snowflake",
        EngineFamily::Snowflake,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::BigQuery,
        "bigquery",
        EngineFamily::BigQuery,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Trino,
        "trino",
        EngineFamily::Trino,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Hive,
        "hive",
        EngineFamily::Trino,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Databricks,
        "databricks",
        EngineFamily::Trino,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Kylin,
        "kylin",
        EngineFamily::Trino,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::H2,
        "h2",
        EngineFamily::Other,
        EngineTransport::File,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Firebird,
        "firebird",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Exasol,
        "exasol",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Vertica,
        "vertica",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Databend,
        "databend",
        EngineFamily::Other,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Tdengine,
        "tdengine",
        EngineFamily::Timeseries,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Gbase8a,
        "gbase8a",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Gbase8s,
        "gbase8s",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Xugudb,
        "xugudb",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Iris,
        "iris",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Redis,
        "redis",
        EngineFamily::KeyValue,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Etcd,
        "etcd",
        EngineFamily::KeyValue,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Mongodb,
        "mongodb",
        EngineFamily::Document,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Elasticsearch,
        "elasticsearch",
        EngineFamily::Search,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Neo4j,
        "neo4j",
        EngineFamily::Graph,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Cassandra,
        "cassandra",
        EngineFamily::WideColumn,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Influxdb,
        "influxdb",
        EngineFamily::Timeseries,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Iotdb,
        "iotdb",
        EngineFamily::Timeseries,
        EngineTransport::Url,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Access,
        "access",
        EngineFamily::Other,
        EngineTransport::File,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Teradata,
        "teradata",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
    spec(
        Engine::Saphana,
        "saphana",
        EngineFamily::Other,
        EngineTransport::Host,
        DialectKind::None,
        false,
    ),
];

const fn spec(
    engine: Engine,
    name: &'static str,
    family: EngineFamily,
    transport: EngineTransport,
    dialect: DialectKind,
    runtime: bool,
) -> Spec {
    Spec {
        engine,
        name,
        family,
        transport,
        dialect,
        runtime,
    }
}

fn capabilities_of(dialect: DialectKind) -> EngineCapabilities {
    match dialect {
        DialectKind::Postgres => from_dialect(&PostgresDialect),
        DialectKind::Sqlite => from_dialect(&SqliteDialect),
        DialectKind::Mysql => from_dialect(&MysqlDialect),
        DialectKind::None => EngineCapabilities::empty(),
    }
}

fn from_dialect(dialect: &impl Dialect) -> EngineCapabilities {
    EngineCapabilities {
        returning: dialect.supports_returning(),
        on_conflict: dialect.supports_on_conflict(),
        update_from: dialect.supports_update_from(),
        row_lock: dialect.supports_select_row_locking(),
        table_lock: dialect.supports_table_locking(),
        information_schema: dialect.supports_information_schema(),
    }
}

fn find(engine: Engine) -> &'static Spec {
    // Infallible: SPECS lists every Engine variant exactly once. A missing row
    // fails tests/engine.rs, so this call never observes an incomplete catalog.
    SPECS
        .iter()
        .find(|spec| spec.engine == engine)
        .expect("engine catalog row")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineParseError;

impl EngineParseError {
    pub fn allowed(self) -> impl Iterator<Item = &'static str> {
        SPECS.iter().map(|spec| spec.name)
    }
}

impl fmt::Display for EngineParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown engine, expected one of:")?;
        for spec in SPECS {
            write!(formatter, " {}", spec.name)?;
        }
        Ok(())
    }
}

impl std::error::Error for EngineParseError {}

impl Engine {
    pub fn parse(name: &str) -> Result<Self, EngineParseError> {
        let name = name.trim();
        SPECS
            .iter()
            .find(|spec| spec.name == name)
            .map(|spec| spec.engine)
            .ok_or(EngineParseError)
    }

    pub fn as_str(self) -> &'static str {
        find(self).name
    }

    pub fn profile(self) -> EngineProfile {
        let spec = find(self);
        EngineProfile {
            engine: self,
            family: spec.family,
            transport: spec.transport,
            capabilities: capabilities_of(spec.dialect),
            runtime: spec.runtime,
        }
    }

    /// Runtimes that can open a connection, in catalog order.
    pub fn profiles() -> Vec<EngineProfile> {
        SPECS
            .iter()
            .filter(|spec| spec.runtime)
            .map(|spec| spec.engine.profile())
            .collect()
    }

    pub fn missing_driver_message(self) -> String {
        if self == Self::Mysql {
            return "a3s-orm has no MySQL driver".to_owned();
        }
        format!("a3s-orm has no {} driver", self.as_str())
    }
}
