//! Versioned single-file transport for an already validated local data set.
//! This is only a codec: no disk access, current-service capture, or import activation.
use super::{ImportFiles, SetValidation};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Deserializer, Serialize};

const FORMAT: &str = "march7-local-backup";
const VERSION: u8 = 1;
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

/// Encode only a complete v1 candidate. Caller owns snapshot consistency.
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
    if document.schema_version != VERSION {
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
    };
    files.validate(SetValidation::Candidate)?;
    if files.digest() != document.sha256 {
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
    Ok(DecodedBackup {
        files,
        preview: Preview {
            created_at_utc_ms: document.created_at_utc_ms,
            selected_character_id: character_id,
            has_desktop_placement,
            reminders,
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
        assert_eq!(document["schemaVersion"], 1);
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
            mutate(|v| v["schemaVersion"] = 2.into()),
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
        files.characters = vec![b'x'; 1024 * 1024];
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
}
