//! Versioned single-file transport for an already validated local data set.
//! This is only a codec: no disk access, current-service capture, or import activation.
use super::{ImportFiles, SetValidation};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Deserializer, Serialize};

const FORMAT: &str = "march7-local-backup";
const VERSION: u8 = 2;
pub const MAX_BACKUP_BYTES: usize = 2 * 1024 * 1024;
const MAX_CREATED_AT_UTC_MS: i64 = 253_402_300_799_999; // 9999-12-31T23:59:59.999Z

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupDocument {
    format: String,
    schema_version: u8,
    created_at_utc_ms: i64,
    files: BackupFiles,
    sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupFiles {
    #[serde(deserialize_with = "required_nullable")]
    desktop_json_base64: Option<String>,
    characters_json_base64: String,
    reminders_json_base64: String,
    #[serde(
        default,
        deserialize_with = "present_string",
        skip_serializing_if = "Option::is_none"
    )]
    focus_json_base64: Option<String>,
}

fn present_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

fn required_nullable<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::deserialize(deserializer)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub created_at_utc_ms: i64,
    pub selected_character_id: String,
    pub has_desktop_placement: bool,
    pub reminders: ReminderPreview,
    pub focus: FocusPreview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusPreview {
    pub status: &'static str,
    pub defaulted_from_v1: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderPreview {
    pub items: [ReminderItemPreview; 3],
    pub active_hours: ActiveHoursPreview,
    pub snooze_minutes: u16,
    pub pending_count: u8,
    pub paused: bool,
    pub quiet_until_utc_ms: Option<i64>,
    pub snooze_pending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderItemPreview {
    pub id: &'static str,
    pub enabled: bool,
    pub interval_minutes: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActiveHoursPreview {
    AllDay,
    Daily { start: u16, end: u16 },
}

pub struct DecodedBackup {
    pub files: ImportFiles,
    pub preview: Preview,
}

/// Encode only a complete v2 candidate. Caller owns snapshot consistency.
pub fn encode(files: ImportFiles, created_at_utc_ms: i64) -> Result<Vec<u8>, &'static str> {
    validate_time(created_at_utc_ms)?;
    files.validate(SetValidation::Candidate)?;
    let document = BackupDocument {
        format: FORMAT.into(),
        schema_version: VERSION,
        created_at_utc_ms,
        files: BackupFiles {
            desktop_json_base64: files.desktop.as_deref().map(|bytes| STANDARD.encode(bytes)),
            characters_json_base64: STANDARD.encode(&files.characters),
            reminders_json_base64: STANDARD.encode(&files.reminders),
            focus_json_base64: Some(STANDARD.encode(&files.focus)),
        },
        sha256: files.digest(),
    };
    let bytes = serde_json::to_vec(&document).map_err(|_| "backupInvalid")?;
    if bytes.len() > MAX_BACKUP_BYTES {
        return Err("backupTooLarge");
    }
    Ok(bytes)
}

/// Decode one bounded package into a frozen candidate and a content-free preview.
pub fn decode(bytes: &[u8]) -> Result<DecodedBackup, &'static str> {
    if bytes.len() > MAX_BACKUP_BYTES {
        return Err("backupTooLarge");
    }
    let document: BackupDocument = serde_json::from_slice(bytes).map_err(|_| "backupInvalid")?;
    if document.format != FORMAT {
        return Err("backupInvalid");
    }
    if !matches!(document.schema_version, 1 | VERSION) {
        return Err("backupUnsupportedVersion");
    }
    validate_time(document.created_at_utc_ms)?;
    if document.sha256.len() != 64
        || !document
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("backupInvalid");
    }
    let files = ImportFiles {
        desktop: document
            .files
            .desktop_json_base64
            .as_deref()
            .map(decode_canonical)
            .transpose()?,
        characters: decode_canonical(&document.files.characters_json_base64)?,
        reminders: decode_canonical(&document.files.reminders_json_base64)?,
        focus: match (
            document.schema_version,
            document.files.focus_json_base64.as_deref(),
        ) {
            (1, None) => super::default_focus()?,
            (2, Some(value)) => decode_canonical(value)?,
            _ => return Err("backupInvalid"),
        },
    };
    files.validate_for(document.schema_version, SetValidation::Candidate)?;
    if files.digest_for(document.schema_version) != document.sha256 {
        return Err("backupChecksumMismatch");
    }
    let character_id =
        crate::characters::imported_selected_id(&files.characters).map_err(|_| "dataSetInvalid")?;
    let has_desktop_placement = files
        .desktop
        .as_deref()
        .map(crate::desktop::imported_has_placement)
        .transpose()
        .map_err(|_| "dataSetInvalid")?
        .unwrap_or(false);
    let data = crate::reminders::decode_imported_reminders(&files.reminders)
        .map_err(|_| "dataSetInvalid")?;
    let items = data.settings.items.map(|item| ReminderItemPreview {
        id: match item.id {
            crate::reminders::ImportId::Water => "water",
            crate::reminders::ImportId::Move => "move",
            crate::reminders::ImportId::Eyes => "eyes",
        },
        enabled: item.enabled,
        interval_minutes: item.interval_minutes,
    });
    let active_hours = match data.settings.active_hours {
        crate::reminders::ImportActiveHours::AllDay {} => ActiveHoursPreview::AllDay,
        crate::reminders::ImportActiveHours::Daily { start, end } => {
            ActiveHoursPreview::Daily { start, end }
        }
    };
    let reminders = ReminderPreview {
        items,
        active_hours,
        snooze_minutes: data.settings.snooze_minutes,
        pending_count: data.progress.iter().filter(|p| p.pending).count() as u8,
        paused: data.paused,
        quiet_until_utc_ms: data.quiet.map(|quiet| quiet.until),
        snooze_pending: data.snooze_pending,
    };
    let focus_data = super::decode_focus(&files.focus)?;
    use crate::focus::model::Session;
    let focus = FocusPreview {
        status: match focus_data.session {
            Session::Idle {} => "idle",
            Session::Running { .. } => "running",
            Session::Paused { .. } => "paused",
            Session::Interrupted { .. } => "interrupted",
            Session::Finished { .. } => "finished",
        },
        defaulted_from_v1: document.schema_version == 1,
    };
    Ok(DecodedBackup {
        files,
        preview: Preview {
            created_at_utc_ms: document.created_at_utc_ms,
            selected_character_id: character_id,
            has_desktop_placement,
            reminders,
            focus,
        },
    })
}

fn validate_time(created_at_utc_ms: i64) -> Result<(), &'static str> {
    if !(0..=MAX_CREATED_AT_UTC_MS).contains(&created_at_utc_ms) {
        return Err("backupInvalidTime");
    }
    Ok(())
}

fn decode_canonical(text: &str) -> Result<Vec<u8>, &'static str> {
    let bytes = STANDARD.decode(text).map_err(|_| "backupInvalid")?;
    if STANDARD.encode(&bytes) != text {
        return Err("backupInvalid");
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_directory::ImportFiles;
    use base64::engine::general_purpose::STANDARD;

    fn candidate() -> ImportFiles {
        ImportFiles {
            desktop: None,
            focus: serde_json::to_vec(&crate::focus::model::Data::default()).unwrap(),
            characters: br#"{ "version":1, "selectedCharacterId":"march-7th" }"#.to_vec(),
            reminders: br#"{"version":1,"settings":{"items":[{"id":"water","enabled":false,"intervalMinutes":60},{"id":"move","enabled":false,"intervalMinutes":60},{"id":"eyes","enabled":false,"intervalMinutes":30}],"activeHours":{"kind":"daily","start":540,"end":1320},"snoozeMinutes":10},"progress":[{"id":"water","nextDueAt":null,"pending":false,"autoHandled":false},{"id":"move","nextDueAt":null,"pending":false,"autoHandled":false},{"id":"eyes","nextDueAt":null,"pending":false,"autoHandled":false}],"paused":false,"quiet":null,"snoozePending":false}"#.to_vec(),
        }
    }

    fn mutate(mut edit: impl FnMut(&mut serde_json::Value)) -> Vec<u8> {
        let mut value: serde_json::Value =
            serde_json::from_slice(&encode(candidate(), 0).unwrap()).unwrap();
        edit(&mut value);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn round_trip_preserves_exact_store_bytes_and_previews_only_summary() {
        let files = candidate();
        let source_characters = files.characters.clone();
        let source_reminders = files.reminders.clone();
        let encoded = encode(files, 1_700_000_000_000).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(document["format"], "march7-local-backup");
        assert_eq!(document["schemaVersion"], 2);
        assert_eq!(
            document["files"]["desktopJsonBase64"],
            serde_json::Value::Null
        );
        assert_eq!(document["sha256"].as_str().unwrap().len(), 64);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.files.desktop, None);
        assert_eq!(decoded.files.characters, source_characters);
        assert_eq!(decoded.files.reminders, source_reminders);
        assert_eq!(decoded.preview.created_at_utc_ms, 1_700_000_000_000);
        assert_eq!(decoded.preview.selected_character_id, "march-7th");
        assert!(!decoded.preview.has_desktop_placement);
        assert_eq!(decoded.preview.reminders.pending_count, 0);
        assert!(!decoded.preview.reminders.paused);
        assert_eq!(decoded.preview.reminders.items[0].id, "water");
        assert!(!decoded.preview.reminders.items[0].enabled);
        assert_eq!(decoded.preview.reminders.items[0].interval_minutes, 60);
        assert_eq!(decoded.preview.reminders.snooze_minutes, 10);
        assert_eq!(decoded.preview.reminders.quiet_until_utc_ms, None);
    }

    #[test]
    fn saved_desktop_bytes_and_pending_reminders_survive_round_trip() {
        let mut files = candidate();
        files.desktop = Some(
            br#"{ "version":1,"placement":{"monitor_name":null,"x":-42.5,"y":10.0}}"#.to_vec(),
        );
        files.reminders = String::from_utf8(files.reminders)
            .unwrap()
            .replace("\"enabled\":false", "\"enabled\":true")
            .replace(
                "\"nextDueAt\":null,\"pending\":false",
                "\"nextDueAt\":null,\"pending\":true",
            )
            .into_bytes();
        // A pending reminder is valid only when that item is enabled.
        let desktop = files.desktop.clone();
        let reminders = files.reminders.clone();
        let decoded = decode(&encode(files, 0).unwrap()).unwrap();
        assert_eq!(decoded.files.desktop, desktop);
        assert_eq!(decoded.files.reminders, reminders);
        assert!(decoded.preview.has_desktop_placement);
        assert_eq!(decoded.preview.reminders.pending_count, 3);
    }

    #[test]
    fn rejects_unknown_duplicate_missing_and_future_backup_fields() {
        for bad in [
            mutate(|v| v["unexpected"] = true.into()),
            mutate(|v| v["files"]["unexpected"] = true.into()),
            mutate(|v| v["schemaVersion"] = 3.into()),
            mutate(|v| v["format"] = "different".into()),
            mutate(|v| {
                v.as_object_mut().unwrap().remove("files");
            }),
            mutate(|v| {
                v["files"]
                    .as_object_mut()
                    .unwrap()
                    .remove("desktopJsonBase64");
            }),
        ] {
            assert!(decode(&bad).is_err());
        }
        let original = String::from_utf8(encode(candidate(), 0).unwrap()).unwrap();
        let duplicate_top = original.replacen(
            "\"format\":",
            "\"format\":\"march7-local-backup\",\"format\":",
            1,
        );
        assert!(decode(duplicate_top.as_bytes()).is_err());
        let duplicate_file = original.replacen(
            "\"desktopJsonBase64\":null",
            "\"desktopJsonBase64\":null,\"desktopJsonBase64\":null",
            1,
        );
        assert!(decode(duplicate_file.as_bytes()).is_err());
    }

    #[test]
    fn rejects_bad_time_checksum_and_base64() {
        for bad in [
            mutate(|v| v["createdAtUtcMs"] = (-1).into()),
            mutate(|v| v["createdAtUtcMs"] = (253_402_300_800_000_i64).into()),
            mutate(|v| v["sha256"] = "A".repeat(64).into()),
            mutate(|v| v["sha256"] = "0".repeat(64).into()),
            mutate(|v| v["files"]["charactersJsonBase64"] = "@@@".into()),
            mutate(|v| v["files"]["charactersJsonBase64"] = "YQ=".into()),
            mutate(|v| v["files"]["charactersJsonBase64"] = serde_json::Value::Null),
        ] {
            assert!(decode(&bad).is_err());
        }
        for bad_base64 in ["YQ=", "Zh==", " YQ=="] {
            let bad = mutate(|v| v["files"]["charactersJsonBase64"] = bad_base64.into());
            assert_eq!(decode(&bad).err(), Some("backupInvalid"));
        }
    }

    #[test]
    fn enforces_package_and_decoded_set_limits_before_use() {
        let mut too_large = encode(candidate(), 0).unwrap();
        too_large.extend(vec![b' '; MAX_BACKUP_BYTES + 1]);
        assert_eq!(decode(&too_large).err(), Some("backupTooLarge"));
        let mut files = candidate();
        files.characters = vec![b'x'; super::super::MAX_V2_SET_BYTES];
        let over_set = mutate(|v| {
            v["files"]["charactersJsonBase64"] = STANDARD.encode(&files.characters).into();
            v["sha256"] = files.digest().into();
        });
        assert_eq!(decode(&over_set).err(), Some("dataSetTooLarge"));
    }

    #[test]
    fn checksum_cannot_make_unknown_role_or_invalid_reminder_acceptable() {
        let mut unknown_role = candidate();
        unknown_role.characters =
            br#"{"version":1,"selectedCharacterId":"retired-character"}"#.to_vec();
        let role_package = mutate(|v| {
            v["files"]["charactersJsonBase64"] = STANDARD.encode(&unknown_role.characters).into();
            v["sha256"] = unknown_role.digest().into();
        });
        assert_eq!(decode(&role_package).err(), Some("dataSetInvalid"));
        let mut bad_reminder = candidate();
        bad_reminder.reminders = String::from_utf8(bad_reminder.reminders)
            .unwrap()
            .replacen("\"intervalMinutes\":60", "\"intervalMinutes\":0", 1)
            .into_bytes();
        let reminder_package = mutate(|v| {
            v["files"]["remindersJsonBase64"] = STANDARD.encode(&bad_reminder.reminders).into();
            v["sha256"] = bad_reminder.digest().into();
        });
        assert_eq!(decode(&reminder_package).err(), Some("dataSetInvalid"));
        assert_eq!(encode(unknown_role, 0).err(), Some("dataSetInvalid"));
        assert_eq!(encode(candidate(), -1).err(), Some("backupInvalidTime"));
    }

    #[test]
    fn rejects_invalid_desktop_and_bad_reminder_progress_with_correct_digest() {
        let mut bad_desktop = candidate();
        bad_desktop.desktop =
            Some(br#"{"version":1,"placement":{"x":1e999,"y":0,"monitor_name":null}}"#.to_vec());
        let desktop_package = mutate(|v| {
            v["files"]["desktopJsonBase64"] = STANDARD
                .encode(bad_desktop.desktop.as_ref().unwrap())
                .into();
            v["sha256"] = bad_desktop.digest().into();
        });
        assert_eq!(decode(&desktop_package).err(), Some("dataSetInvalid"));
        let mut bad_progress = candidate();
        bad_progress.reminders = String::from_utf8(bad_progress.reminders)
            .unwrap()
            .replacen(
                "\"pending\":false,\"autoHandled\":false",
                "\"pending\":false,\"autoHandled\":true",
                1,
            )
            .into_bytes();
        let progress_package = mutate(|v| {
            v["files"]["remindersJsonBase64"] = STANDARD.encode(&bad_progress.reminders).into();
            v["sha256"] = bad_progress.digest().into();
        });
        assert_eq!(decode(&progress_package).err(), Some("dataSetInvalid"));
    }
    #[test]
    fn v1_golden_checksum_is_accepted_and_default_focus_is_explicit_in_preview() {
        use sha2::{Digest, Sha256};
        let files = candidate();
        let mut digest = Sha256::new();
        for (name, bytes) in [
            ("desktop-state.json", super::super::ABSENT_DESKTOP),
            ("character-preferences.json", files.characters.as_slice()),
            ("reminders.json", files.reminders.as_slice()),
        ] {
            digest.update(name.as_bytes());
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        let old = serde_json::json!({"format":"march7-local-backup","schemaVersion":1,"createdAtUtcMs":0,
            "files":{"desktopJsonBase64":null,"charactersJsonBase64":STANDARD.encode(&files.characters),"remindersJsonBase64":STANDARD.encode(&files.reminders)},
            "sha256":format!("{:x}",digest.finalize())});
        let decoded = decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(decoded.files.focus, super::super::default_focus().unwrap());
        assert!(decoded.preview.focus.defaulted_from_v1);
        assert_eq!(decoded.preview.focus.status, "idle");
        let reencoded = encode(decoded.files, 0).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&reencoded).unwrap();
        assert_eq!(document["schemaVersion"], 2);
        assert_ne!(old["sha256"], document["sha256"]);
        let mut with_focus = old.clone();
        with_focus["files"]["focusJsonBase64"] = STANDARD.encode(&files.focus).into();
        assert_eq!(
            decode(&serde_json::to_vec(&with_focus).unwrap()).err(),
            Some("backupInvalid")
        );
    }

    #[test]
    fn v2_focus_roundtrip_checksum_shape_and_limits_are_enforced() {
        let mut files = candidate();
        files.focus=br#"{ "version":1,"session":{"status":"paused","duration_ms":60000,"remaining_ms":12000}}"#.to_vec();
        let bytes = encode(files.clone(), 0).unwrap();
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.files.focus, files.focus);
        assert_eq!(decoded.preview.focus.status, "paused");
        assert!(!decoded.preview.focus.defaulted_from_v1);
        assert_eq!(
            serde_json::to_value(&decoded.preview.focus).unwrap(),
            serde_json::json!({"status":"paused","defaultedFromV1":false})
        );
        let mut changed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        changed["files"]["focusJsonBase64"] = STANDARD
            .encode(super::super::default_focus().unwrap())
            .into();
        assert_eq!(
            decode(&serde_json::to_vec(&changed).unwrap()).err(),
            Some("backupChecksumMismatch")
        );
        for bad in [
            mutate(|v| {
                v["files"]
                    .as_object_mut()
                    .unwrap()
                    .remove("focusJsonBase64");
            }),
            mutate(|v| v["files"]["focusJsonBase64"] = serde_json::Value::Null),
            mutate(|v| v["files"]["focusJsonBase64"] = "bad".into()),
        ] {
            assert_eq!(decode(&bad).err(), Some("backupInvalid"));
        }
        for bad in [
            br#"{"version":2,"session":{"status":"idle"}}"#.as_slice(),
            br#"{"version":1,"session":{"status":"idle","task":"secret"}}"#.as_slice(),
            br#"{"version":1,"session":{"status":"paused","duration_ms":60000,"remaining_ms":0}}"#
                .as_slice(),
        ] {
            files.focus = bad.to_vec();
            assert_eq!(encode(files.clone(), 0).err(), Some("dataSetInvalid"));
        }
        files.focus = vec![b' '; super::super::MAX_SET_BYTES];
        assert_eq!(encode(files, 0).err(), Some("dataSetTooLarge"));
    }
    #[test]
    fn legacy_limit_and_reserved_focus_budget_are_both_bounded() {
        let mut files = candidate();
        let total =
            super::super::ABSENT_DESKTOP.len() + files.characters.len() + files.reminders.len();
        files
            .reminders
            .extend(vec![b' '; super::super::MAX_SET_BYTES - total]);
        let v1 = |files: &ImportFiles| {
            serde_json::to_vec(&serde_json::json!({
            "format":"march7-local-backup","schemaVersion":1,"createdAtUtcMs":0,
            "files":{"desktopJsonBase64":null,"charactersJsonBase64":STANDARD.encode(&files.characters),"remindersJsonBase64":STANDARD.encode(&files.reminders)},
            "sha256":files.digest_for(1)
        })).unwrap()
        };
        let decoded = decode(&v1(&files)).unwrap();
        assert_eq!(decoded.files.reminders, files.reminders);
        assert_eq!(decoded.files.focus, super::super::default_focus().unwrap());
        files.focus=br#"{"version":1,"session":{"status":"paused","duration_ms":60000,"remaining_ms":12000}}"#.to_vec();
        files.focus.resize(super::super::MAX_FOCUS_BYTES, b' ');
        let restored = decode(&encode(files.clone(), 0).unwrap()).unwrap();
        assert_eq!(restored.files.focus, files.focus);
        files.focus.push(b' ');
        assert_eq!(encode(files.clone(), 0).err(), Some("dataSetTooLarge"));
        files.focus = super::super::default_focus().unwrap();
        files.reminders.push(b' ');
        assert_eq!(decode(&v1(&files)).err(), Some("dataSetTooLarge"));
    }
}
