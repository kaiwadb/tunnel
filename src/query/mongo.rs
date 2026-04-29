use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use mongodb::{Client, Collection, bson::Document};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde_json::{Value, to_value};
use tracing::info;

use crate::error::AgentError;
use crate::params::{ConnectionParams, MongoHost};

static CLIENTS: LazyLock<Mutex<HashMap<String, Client>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// RFC 3986 userinfo-safe set: encode anything that would be ambiguous in
/// userinfo (`:`, `@`, `/`, `?`, `#`, `[`, `]`, etc).
const USERINFO: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b':')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

pub async fn execute(
    conn: &ConnectionParams,
    collection: String,
    pipeline: Vec<Document>,
) -> Result<Value, AgentError> {
    let ConnectionParams::Mongo {
        srv,
        hosts,
        username,
        password,
        database,
        auth_source,
        replica_set,
        tls,
    } = conn
    else {
        return Err(AgentError::Connection(
            "mongo executor received non-mongo connection params".into(),
        ));
    };

    let uri = build_mongo_uri(
        *srv,
        hosts,
        username.as_deref(),
        password.as_deref(),
        database,
        auth_source.as_deref(),
        replica_set.as_deref(),
        *tls,
    )?;

    let cache_key = uri.clone();
    let client = {
        let clients = CLIENTS.lock().unwrap();
        clients.get(&cache_key).cloned()
    };

    let client = match client {
        Some(client) => client,
        None => {
            info!(database = %database, "creating new mongodb client");
            let client = Client::with_uri_str(&uri).await?;
            CLIENTS
                .lock()
                .unwrap()
                .insert(cache_key, client.clone());
            client
        }
    };

    let db = client.database(database);
    let collection: Collection<Document> = db.collection(&collection);

    let mut cursor = collection.aggregate(pipeline).await?;
    let mut batch = Vec::new();

    while cursor.advance().await? {
        let doc = cursor.deserialize_current()?;
        batch.push(doc);
    }

    Ok(to_value(&batch)?)
}

fn build_mongo_uri(
    srv: bool,
    hosts: &[MongoHost],
    username: Option<&str>,
    password: Option<&str>,
    database: &str,
    auth_source: Option<&str>,
    replica_set: Option<&str>,
    tls: Option<bool>,
) -> Result<String, AgentError> {
    if hosts.is_empty() {
        return Err(AgentError::Connection(
            "mongo connection requires at least one host".into(),
        ));
    }
    if srv && hosts.len() != 1 {
        return Err(AgentError::Connection(
            "mongo SRV connection requires exactly one host".into(),
        ));
    }

    let scheme = if srv { "mongodb+srv" } else { "mongodb" };

    let userinfo = match (username, password) {
        (Some(u), Some(p)) => format!(
            "{}:{}@",
            utf8_percent_encode(u, USERINFO),
            utf8_percent_encode(p, USERINFO)
        ),
        (Some(u), None) => format!("{}@", utf8_percent_encode(u, USERINFO)),
        _ => String::new(),
    };

    let host_list = hosts
        .iter()
        .map(|h| match (srv, h.port) {
            (true, _) => h.host.clone(),
            (false, Some(p)) => format!("{}:{}", h.host, p),
            (false, None) => h.host.clone(),
        })
        .collect::<Vec<_>>()
        .join(",");

    let mut params: Vec<String> = Vec::new();
    if let Some(src) = auth_source {
        params.push(format!("authSource={src}"));
    }
    if let Some(rs) = replica_set {
        params.push(format!("replicaSet={rs}"));
    }
    if let Some(t) = tls {
        params.push(format!("tls={t}"));
    }

    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };

    Ok(format!(
        "{scheme}://{userinfo}{host_list}/{database}{query}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_uri_basic() {
        let hosts = vec![MongoHost { host: "h".into(), port: Some(27017) }];
        let uri = build_mongo_uri(false, &hosts, Some("u"), Some("p"), "d", None, None, None)
            .unwrap();
        assert_eq!(uri, "mongodb://u:p@h:27017/d");
    }

    #[test]
    fn build_uri_encodes_password_specials() {
        let hosts = vec![MongoHost { host: "h".into(), port: None }];
        // `$` is a sub-delim per RFC 3986 userinfo; not encoded.
        let uri = build_mongo_uri(false, &hosts, Some("u"), Some("p#$%@:/"), "d", None, None, None)
            .unwrap();
        assert!(uri.contains("p%23$%25%40%3A%2F"), "got {uri}");
        assert!(uri.contains("@h/d"));
    }

    #[test]
    fn build_uri_srv_no_port() {
        let hosts = vec![MongoHost { host: "cluster.mongodb.net".into(), port: None }];
        let uri = build_mongo_uri(
            true,
            &hosts,
            Some("u"),
            Some("p"),
            "app",
            Some("admin"),
            None,
            Some(true),
        )
        .unwrap();
        assert_eq!(uri, "mongodb+srv://u:p@cluster.mongodb.net/app?authSource=admin&tls=true");
    }

    #[test]
    fn build_uri_multi_host_replica_set() {
        let hosts = vec![
            MongoHost { host: "h1".into(), port: Some(27017) },
            MongoHost { host: "h2".into(), port: Some(27018) },
        ];
        let uri = build_mongo_uri(
            false,
            &hosts,
            None,
            None,
            "app",
            None,
            Some("rs0"),
            None,
        )
        .unwrap();
        assert_eq!(uri, "mongodb://h1:27017,h2:27018/app?replicaSet=rs0");
    }

    #[test]
    fn build_uri_srv_rejects_multi_host() {
        let hosts = vec![
            MongoHost { host: "h1".into(), port: None },
            MongoHost { host: "h2".into(), port: None },
        ];
        assert!(build_mongo_uri(true, &hosts, None, None, "d", None, None, None).is_err());
    }

    #[test]
    fn build_uri_rejects_empty_hosts() {
        let hosts: Vec<MongoHost> = vec![];
        assert!(build_mongo_uri(false, &hosts, None, None, "d", None, None, None).is_err());
    }
}
