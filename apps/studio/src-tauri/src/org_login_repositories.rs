//! Team repositories on the member plane (`/v1/api/org/repositories`). The
//! upstream admits every route against the addressed repository; this list
//! only keeps the proxy to the shapes those routes accept.

/// Same shape as a repository namespace or name upstream.
fn part_ok(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.ends_with('-')
}

fn commit_ok(oid: &str) -> bool {
    oid.len() == 40 && oid.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn number_ok(number: &str) -> bool {
    !number.is_empty() && number.len() <= 9 && number.bytes().all(|byte| byte.is_ascii_digit())
}

/// `rest` follows `/v1/api/org/repositories`.
pub fn allowed_repository_path(method: &str, rest: &str) -> bool {
    if rest.is_empty() {
        return method == "GET" || method == "POST";
    }
    let Some(rest) = rest.strip_prefix('/') else {
        return false;
    };
    let segments: Vec<&str> = rest.split('/').collect();
    let [namespace, name, tail @ ..] = segments.as_slice() else {
        return false;
    };
    if !part_ok(namespace) || !part_ok(name) {
        return false;
    }
    match (method, tail) {
        ("GET", [])
        | ("GET", ["branches" | "tags" | "tree" | "blob" | "commits" | "compare"])
        | ("GET" | "POST", ["change-requests"])
        | ("GET" | "PUT", ["protections"]) => true,
        ("GET", ["commits", oid]) | ("GET", ["commits", oid, "checks"]) => commit_ok(oid),
        ("GET", ["change-requests", number])
        | (
            "POST",
            [
                "change-requests",
                number,
                "comments" | "reviews" | "merge" | "close" | "reopen",
            ],
        ) => number_ok(number),
        _ => false,
    }
}

/// Same shape as a pipeline repository id (a storage key) upstream.
fn repo_id_ok(value: &str) -> bool {
    (1..=120).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        && !value.starts_with('.')
        && !value.contains("..")
}

/// `rest` follows `/v1/api/org/pipelines/`: runs are read only for members.
pub fn allowed_pipeline_path(method: &str, rest: &str) -> bool {
    if method != "GET" {
        return false;
    }
    match rest.split('/').collect::<Vec<_>>().as_slice() {
        ["repos", repo_id] => repo_id_ok(repo_id),
        ["runs", run_id] => super::uuid_id_ok(run_id),
        _ => false,
    }
}

/// `rest` follows `/v1/api/org/repository-namespaces/`: one team id, read only.
pub fn allowed_namespace_path(method: &str, rest: &str) -> bool {
    method == "GET" && super::uuid_id_ok(rest)
}

#[cfg(test)]
#[path = "org_login_repositories_tests.rs"]
mod tests;
