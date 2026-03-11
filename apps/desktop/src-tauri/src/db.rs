use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::models::{HistoryRecord, NewHistoryRecord, Settings};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  hotkey TEXT NOT NULL,
  api_base_url TEXT NOT NULL,
  auto_paste INTEGER NOT NULL,
  pause_background_audio INTEGER NOT NULL DEFAULT 1,
  restore_clipboard INTEGER NOT NULL,
  interaction_mode TEXT NOT NULL DEFAULT 'hold',
  preferred_mode TEXT NOT NULL,
  save_history INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS history (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  app_name TEXT,
  mode TEXT NOT NULL,
  audio_duration_ms INTEGER NOT NULL,
  raw_text TEXT NOT NULL,
  clean_text TEXT NOT NULL,
  status TEXT NOT NULL
);
"#;

pub fn initialize(path: &Path) -> Result<(), String> {
    let connection = open(path)?;
    connection
        .execute_batch(SCHEMA)
        .map_err(|error| error.to_string())?;
    ensure_settings_columns(&connection)?;

    let row_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;

    if row_count == 0 {
        save_settings_connection(&connection, &Settings::default())?;
    }

    Ok(())
}

pub fn get_settings(path: &Path) -> Result<Settings, String> {
    let connection = open(path)?;
    connection
        .query_row(
            "SELECT hotkey, api_base_url, auto_paste, pause_background_audio, restore_clipboard, interaction_mode, preferred_mode, save_history
             FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(Settings {
                    hotkey: row.get(0)?,
                    api_base_url: row.get(1)?,
                    auto_paste: row.get::<_, i64>(2)? != 0,
                    pause_background_audio: row.get::<_, i64>(3)? != 0,
                    restore_clipboard: row.get::<_, i64>(4)? != 0,
                    interaction_mode: row.get(5)?,
                    preferred_mode: row.get(6)?,
                    save_history: row.get::<_, i64>(7)? != 0,
                })
            },
        )
        .map_err(|error| error.to_string())
}

pub fn update_settings(path: &Path, settings: &Settings) -> Result<Settings, String> {
    let connection = open(path)?;
    save_settings_connection(&connection, settings)?;
    Ok(settings.clone())
}

pub fn list_history(path: &Path, limit: u32) -> Result<Vec<HistoryRecord>, String> {
    let connection = open(path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, created_at, app_name, mode, audio_duration_ms, raw_text, clean_text, status
             FROM history
             ORDER BY datetime(created_at) DESC
             LIMIT ?1",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map(params![limit], |row| {
            Ok(HistoryRecord {
                id: row.get(0)?,
                created_at: row.get(1)?,
                app_name: row.get(2)?,
                mode: row.get(3)?,
                audio_duration_ms: row.get(4)?,
                raw_text: row.get(5)?,
                clean_text: row.get(6)?,
                status: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn create_history_record(
    path: &Path,
    record: &NewHistoryRecord,
) -> Result<HistoryRecord, String> {
    let connection = open(path)?;
    let history_record = HistoryRecord {
        id: Uuid::new_v4().to_string(),
        created_at: Utc::now().to_rfc3339(),
        app_name: record.app_name.clone(),
        mode: record.mode.clone(),
        audio_duration_ms: record.audio_duration_ms,
        raw_text: record.raw_text.clone(),
        clean_text: record.clean_text.clone(),
        status: record.status.clone(),
    };

    connection
        .execute(
            "INSERT INTO history (id, created_at, app_name, mode, audio_duration_ms, raw_text, clean_text, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                history_record.id,
                history_record.created_at,
                history_record.app_name,
                history_record.mode,
                history_record.audio_duration_ms,
                history_record.raw_text,
                history_record.clean_text,
                history_record.status,
            ],
        )
        .map_err(|error| error.to_string())?;

    Ok(history_record)
}

fn save_settings_connection(connection: &Connection, settings: &Settings) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO settings (
                id,
                hotkey,
                api_base_url,
                auto_paste,
                pause_background_audio,
                restore_clipboard,
                interaction_mode,
                preferred_mode,
                save_history
             )
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                hotkey = excluded.hotkey,
                api_base_url = excluded.api_base_url,
                auto_paste = excluded.auto_paste,
                pause_background_audio = excluded.pause_background_audio,
                restore_clipboard = excluded.restore_clipboard,
                interaction_mode = excluded.interaction_mode,
                preferred_mode = excluded.preferred_mode,
                save_history = excluded.save_history",
            params![
                settings.hotkey,
                settings.api_base_url,
                bool_to_integer(settings.auto_paste),
                bool_to_integer(settings.pause_background_audio),
                bool_to_integer(settings.restore_clipboard),
                settings.interaction_mode,
                settings.preferred_mode,
                bool_to_integer(settings.save_history),
            ],
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn open(path: &Path) -> Result<Connection, String> {
    Connection::open(path).map_err(|error| error.to_string())
}

fn ensure_settings_columns(connection: &Connection) -> Result<(), String> {
    add_column_if_missing(
        connection,
        "settings",
        "interaction_mode",
        "TEXT NOT NULL DEFAULT 'hold'",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "pause_background_audio",
        "INTEGER NOT NULL DEFAULT 1",
    )
}

fn add_column_if_missing(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut statement = connection
        .prepare(&pragma)
        .map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;
    let found = columns
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .into_iter()
        .any(|existing| existing == column);

    if found {
        return Ok(());
    }

    let alter = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    connection
        .execute(&alter, [])
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn bool_to_integer(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::*;

    fn temp_db_path() -> std::path::PathBuf {
        env::temp_dir().join(format!("tator-test-{}.sqlite3", Uuid::new_v4()))
    }

    #[test]
    fn initializes_and_round_trips_settings() {
        let path = temp_db_path();
        initialize(&path).unwrap();

        let mut settings = get_settings(&path).unwrap();
        settings.hotkey = "Option+Shift+Space".to_string();
        settings.interaction_mode = "toggle".to_string();
        settings.pause_background_audio = false;
        settings.save_history = false;

        update_settings(&path, &settings).unwrap();

        let loaded = get_settings(&path).unwrap();
        assert_eq!(loaded.hotkey, "Option+Shift+Space");
        assert_eq!(loaded.interaction_mode, "toggle");
        assert!(!loaded.pause_background_audio);
        assert!(!loaded.save_history);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn inserts_history_records() {
        let path = temp_db_path();
        initialize(&path).unwrap();

        create_history_record(
            &path,
            &NewHistoryRecord {
                app_name: Some("TextEdit".to_string()),
                mode: "default".to_string(),
                audio_duration_ms: 500,
                raw_text: "hello".to_string(),
                clean_text: "Hello.".to_string(),
                status: "inserted".to_string(),
            },
        )
        .unwrap();

        let history = list_history(&path, 5).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].app_name.as_deref(), Some("TextEdit"));

        let _ = fs::remove_file(path);
    }
}
