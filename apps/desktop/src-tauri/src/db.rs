use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::models::{HistoryRecord, NewHistoryRecord, Settings};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  hotkey TEXT NOT NULL,
  auto_paste INTEGER NOT NULL,
  spoken_press_enter_command INTEGER NOT NULL DEFAULT 0,
  pause_background_audio INTEGER NOT NULL DEFAULT 1,
  warm_model_on_launch INTEGER NOT NULL DEFAULT 1,
  recording_cue_volume INTEGER NOT NULL DEFAULT 75,
  restore_clipboard INTEGER NOT NULL,
  interaction_mode TEXT NOT NULL DEFAULT 'hold',
  preferred_mode TEXT NOT NULL,
  save_history INTEGER NOT NULL,
  hide_dock_icon INTEGER NOT NULL DEFAULT 0,
  show_hud INTEGER NOT NULL DEFAULT 1
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
    migrate_settings_table(&connection)?;

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
    let mut settings = connection
        .query_row(
            "SELECT hotkey, auto_paste, spoken_press_enter_command, pause_background_audio, warm_model_on_launch, recording_cue_volume, restore_clipboard, interaction_mode, preferred_mode, save_history, hide_dock_icon, show_hud
             FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(Settings {
                    hotkey: row.get(0)?,
                    auto_paste: row.get::<_, i64>(1)? != 0,
                    spoken_press_enter_command: row.get::<_, i64>(2)? != 0,
                    pause_background_audio: row.get::<_, i64>(3)? != 0,
                    warm_model_on_launch: row.get::<_, i64>(4)? != 0,
                    recording_cue_volume: row.get(5)?,
                    restore_clipboard: row.get::<_, i64>(6)? != 0,
                    interaction_mode: row.get(7)?,
                    preferred_mode: row.get(8)?,
                    save_history: row.get::<_, i64>(9)? != 0,
                    hide_dock_icon: row.get::<_, i64>(10)? != 0,
                    show_hud: row.get::<_, i64>(11)? != 0,
                })
            },
        )
        .map_err(|error| error.to_string())?;

    if normalize_settings(&mut settings) {
        save_settings_connection(&connection, &settings)?;
    }

    Ok(settings)
}

pub fn update_settings(path: &Path, settings: &Settings) -> Result<Settings, String> {
    let connection = open(path)?;
    let mut normalized = settings.clone();
    normalize_settings(&mut normalized);
    save_settings_connection(&connection, &normalized)?;
    Ok(normalized)
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
            let mode: String = row.get(3)?;
            Ok(HistoryRecord {
                id: row.get(0)?,
                created_at: row.get(1)?,
                app_name: row.get(2)?,
                mode: normalize_history_mode(&mode),
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
        mode: normalize_history_mode(&record.mode),
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
                auto_paste,
                spoken_press_enter_command,
                pause_background_audio,
                warm_model_on_launch,
                recording_cue_volume,
                restore_clipboard,
                interaction_mode,
                preferred_mode,
                save_history,
                hide_dock_icon,
                show_hud
             )
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                hotkey = excluded.hotkey,
                auto_paste = excluded.auto_paste,
                spoken_press_enter_command = excluded.spoken_press_enter_command,
                pause_background_audio = excluded.pause_background_audio,
                warm_model_on_launch = excluded.warm_model_on_launch,
                recording_cue_volume = excluded.recording_cue_volume,
                restore_clipboard = excluded.restore_clipboard,
                interaction_mode = excluded.interaction_mode,
                preferred_mode = excluded.preferred_mode,
                save_history = excluded.save_history,
                hide_dock_icon = excluded.hide_dock_icon,
                show_hud = excluded.show_hud",
            params![
                settings.hotkey,
                bool_to_integer(settings.auto_paste),
                bool_to_integer(settings.spoken_press_enter_command),
                bool_to_integer(settings.pause_background_audio),
                bool_to_integer(settings.warm_model_on_launch),
                settings.recording_cue_volume,
                bool_to_integer(settings.restore_clipboard),
                settings.interaction_mode,
                settings.preferred_mode,
                bool_to_integer(settings.save_history),
                bool_to_integer(settings.hide_dock_icon),
                bool_to_integer(settings.show_hud),
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
        "spoken_press_enter_command",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "pause_background_audio",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "warm_model_on_launch",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "recording_cue_volume",
        "INTEGER NOT NULL DEFAULT 75",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "hide_dock_icon",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "show_hud",
        "INTEGER NOT NULL DEFAULT 1",
    )
}

fn migrate_settings_table(connection: &Connection) -> Result<(), String> {
    if !column_exists(connection, "settings", "api_base_url")? {
        return Ok(());
    }

    connection
        .execute_batch(
            r#"
            BEGIN IMMEDIATE;
            CREATE TABLE settings_next (
              id INTEGER PRIMARY KEY CHECK (id = 1),
              hotkey TEXT NOT NULL,
              auto_paste INTEGER NOT NULL,
              spoken_press_enter_command INTEGER NOT NULL DEFAULT 0,
              pause_background_audio INTEGER NOT NULL DEFAULT 1,
              warm_model_on_launch INTEGER NOT NULL DEFAULT 1,
              recording_cue_volume INTEGER NOT NULL DEFAULT 75,
              restore_clipboard INTEGER NOT NULL,
              interaction_mode TEXT NOT NULL DEFAULT 'hold',
              preferred_mode TEXT NOT NULL,
              save_history INTEGER NOT NULL,
              hide_dock_icon INTEGER NOT NULL DEFAULT 0,
              show_hud INTEGER NOT NULL DEFAULT 1
            );
            INSERT INTO settings_next (
              id,
              hotkey,
              auto_paste,
              spoken_press_enter_command,
              pause_background_audio,
              warm_model_on_launch,
              recording_cue_volume,
              restore_clipboard,
              interaction_mode,
              preferred_mode,
              save_history,
              hide_dock_icon,
              show_hud
            )
            SELECT
              id,
              hotkey,
              auto_paste,
              0,
              COALESCE(pause_background_audio, 1),
              1,
              COALESCE(recording_cue_volume, 75),
              restore_clipboard,
              COALESCE(interaction_mode, 'hold'),
              preferred_mode,
              save_history,
              0,
              1
            FROM settings;
            DROP TABLE settings;
            ALTER TABLE settings_next RENAME TO settings;
            COMMIT;
            "#,
        )
        .map_err(|error| error.to_string())
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

fn column_exists(connection: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let pragma = format!("PRAGMA table_info({table})");
    let mut statement = connection
        .prepare(&pragma)
        .map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;

    columns
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
        .map(|existing| existing.into_iter().any(|name| name == column))
}

fn bool_to_integer(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn normalize_settings(settings: &mut Settings) -> bool {
    let normalized_mode = normalize_preferred_mode(&settings.preferred_mode);
    if normalized_mode == settings.preferred_mode {
        return false;
    }

    settings.preferred_mode = normalized_mode;
    true
}

fn normalize_preferred_mode(mode: &str) -> String {
    match mode {
        "notes" => "bullet_list".to_string(),
        "default" | "chat" | "bullet_list" => mode.to_string(),
        _ => "default".to_string(),
    }
}

fn normalize_history_mode(mode: &str) -> String {
    match mode {
        "notes" => "bullet_list".to_string(),
        "default" | "chat" | "bullet_list" => mode.to_string(),
        _ => "default".to_string(),
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
        assert!(settings.warm_model_on_launch);
        settings.hotkey = "Option+Shift+Space".to_string();
        settings.interaction_mode = "toggle".to_string();
        settings.spoken_press_enter_command = true;
        settings.pause_background_audio = false;
        settings.warm_model_on_launch = false;
        settings.recording_cue_volume = 0;
        settings.save_history = false;
        settings.hide_dock_icon = true;
        settings.show_hud = false;

        update_settings(&path, &settings).unwrap();

        let loaded = get_settings(&path).unwrap();
        assert_eq!(loaded.hotkey, "Option+Shift+Space");
        assert_eq!(loaded.interaction_mode, "toggle");
        assert!(loaded.spoken_press_enter_command);
        assert!(!loaded.pause_background_audio);
        assert!(!loaded.warm_model_on_launch);
        assert_eq!(loaded.recording_cue_volume, 0);
        assert!(!loaded.save_history);
        assert!(loaded.hide_dock_icon);
        assert!(!loaded.show_hud);
        assert!(!column_exists(&open(&path).unwrap(), "settings", "api_base_url").unwrap());

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

    #[test]
    fn migrates_legacy_settings_without_api_url() {
        let path = temp_db_path();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE settings (
                  id INTEGER PRIMARY KEY CHECK (id = 1),
                  hotkey TEXT NOT NULL,
                  api_base_url TEXT NOT NULL,
                  auto_paste INTEGER NOT NULL,
                  spoken_press_enter_command INTEGER NOT NULL DEFAULT 0,
                  pause_background_audio INTEGER NOT NULL DEFAULT 1,
                  recording_cue_volume INTEGER NOT NULL DEFAULT 75,
                  restore_clipboard INTEGER NOT NULL,
                  interaction_mode TEXT NOT NULL DEFAULT 'hold',
                  preferred_mode TEXT NOT NULL,
                  save_history INTEGER NOT NULL
                );
                INSERT INTO settings (
                  id,
                  hotkey,
                  api_base_url,
                  auto_paste,
                  spoken_press_enter_command,
                  pause_background_audio,
                  recording_cue_volume,
                  restore_clipboard,
                  interaction_mode,
                  preferred_mode,
                  save_history
                ) VALUES (
                  1,
                  'Option+Space',
                  'http://127.0.0.1:8000',
                  1,
                  0,
                  1,
                  75,
                  1,
                  'hold',
                  'default',
                  1
                );
                "#,
            )
            .unwrap();
        drop(connection);

        initialize(&path).unwrap();

        let migrated = get_settings(&path).unwrap();
        assert_eq!(migrated.hotkey, "Option+Space");
        assert!(!migrated.spoken_press_enter_command);
        assert!(migrated.warm_model_on_launch);
        assert!(migrated.show_hud);
        assert!(
            column_exists(&open(&path).unwrap(), "settings", "api_base_url")
                .map(|exists| !exists)
                .unwrap()
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn normalizes_removed_email_preferred_mode_to_default() {
        let path = temp_db_path();
        initialize(&path).unwrap();

        let connection = open(&path).unwrap();
        connection
            .execute(
                "UPDATE settings SET preferred_mode = 'email' WHERE id = 1",
                [],
            )
            .unwrap();
        drop(connection);

        let settings = get_settings(&path).unwrap();
        assert_eq!(settings.preferred_mode, "default");

        let persisted_mode: String = open(&path)
            .unwrap()
            .query_row(
                "SELECT preferred_mode FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted_mode, "default");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn normalizes_removed_notes_preferred_mode_to_bullet_list() {
        let path = temp_db_path();
        initialize(&path).unwrap();

        let connection = open(&path).unwrap();
        connection
            .execute(
                "UPDATE settings SET preferred_mode = 'notes' WHERE id = 1",
                [],
            )
            .unwrap();
        drop(connection);

        let settings = get_settings(&path).unwrap();
        assert_eq!(settings.preferred_mode, "bullet_list");

        let persisted_mode: String = open(&path)
            .unwrap()
            .query_row(
                "SELECT preferred_mode FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted_mode, "bullet_list");

        let _ = fs::remove_file(path);
    }
}
