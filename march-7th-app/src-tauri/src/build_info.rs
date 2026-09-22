use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    schema_version: u8,
    app_version: &'static str,
    target: &'static str,
    source_commit: Option<&'static str>,
    source_state: &'static str,
}

const COMMIT: &str = env!("MARCH_SOURCE_COMMIT");
pub const BUILD_INFO: BuildInfo = BuildInfo {
    schema_version: 1,
    app_version: env!("CARGO_PKG_VERSION"),
    target: env!("MARCH_BUILD_TARGET"),
    source_commit: if COMMIT.is_empty() {
        None
    } else {
        Some(COMMIT)
    },
    source_state: env!("MARCH_SOURCE_STATE"),
};

#[tauri::command]
pub fn get_build_info() -> BuildInfo {
    BUILD_INFO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_and_cli_value_have_one_exact_contract() {
        let value = serde_json::to_value(get_build_info()).unwrap();
        assert_eq!(get_build_info(), BUILD_INFO);
        assert_eq!(value.as_object().unwrap().len(), 5);
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["appVersion"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["target"], env!("MARCH_BUILD_TARGET"));
        assert!(matches!(
            value["sourceState"].as_str(),
            Some("clean" | "modified" | "unknown")
        ));
        match BUILD_INFO.source_commit {
            Some(commit) => {
                assert_eq!(commit.len(), 40);
                assert!(commit
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
                assert_ne!(BUILD_INFO.source_state, "unknown");
            }
            None => assert_eq!(BUILD_INFO.source_state, "unknown"),
        }
    }
}
