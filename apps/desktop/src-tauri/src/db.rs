use std::{collections::HashMap, path::Path};

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::models::{
    HistoryMetrics, HistoryRecord, HistoryWordMetric, NewHistoryRecord, Settings, SpeechModel,
};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  hotkey TEXT NOT NULL,
  auto_paste INTEGER NOT NULL,
  auto_backtrack INTEGER NOT NULL DEFAULT 0,
  editor_file_mentions_enabled INTEGER NOT NULL DEFAULT 0,
  force_editor_composer INTEGER NOT NULL DEFAULT 0,
  spoken_press_enter_command INTEGER NOT NULL DEFAULT 0,
  pause_background_audio INTEGER NOT NULL DEFAULT 1,
  warm_model_on_launch INTEGER NOT NULL DEFAULT 1,
  speech_model TEXT NOT NULL DEFAULT 'base_en',
  recording_cue_volume INTEGER NOT NULL DEFAULT 75,
  restore_clipboard INTEGER NOT NULL,
  interaction_mode TEXT NOT NULL DEFAULT 'hold',
  preferred_mode TEXT NOT NULL,
  typing_style TEXT NOT NULL DEFAULT 'formal',
  custom_dictionary TEXT NOT NULL DEFAULT '',
  save_history INTEGER NOT NULL,
  hide_dock_icon INTEGER NOT NULL DEFAULT 0,
  show_hud INTEGER NOT NULL DEFAULT 1,
  show_editor_debug_hud INTEGER NOT NULL DEFAULT 0,
  theme TEXT NOT NULL DEFAULT 'system'
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
            "SELECT hotkey, auto_paste, auto_backtrack, editor_file_mentions_enabled, force_editor_composer, spoken_press_enter_command, pause_background_audio, warm_model_on_launch, speech_model, recording_cue_volume, restore_clipboard, interaction_mode, preferred_mode, typing_style, custom_dictionary, save_history, hide_dock_icon, show_hud, show_editor_debug_hud, theme
             FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(Settings {
                    hotkey: row.get(0)?,
                    auto_paste: row.get::<_, i64>(1)? != 0,
                    auto_backtrack: row.get::<_, i64>(2)? != 0,
                    editor_file_mentions_enabled: row.get::<_, i64>(3)? != 0,
                    force_editor_composer: row.get::<_, i64>(4)? != 0,
                    spoken_press_enter_command: row.get::<_, i64>(5)? != 0,
                    pause_background_audio: row.get::<_, i64>(6)? != 0,
                    warm_model_on_launch: row.get::<_, i64>(7)? != 0,
                    speech_model: SpeechModel::from_db_value(&row.get::<_, String>(8)?),
                    recording_cue_volume: row.get(9)?,
                    restore_clipboard: row.get::<_, i64>(10)? != 0,
                    interaction_mode: row.get(11)?,
                    preferred_mode: row.get(12)?,
                    typing_style: row.get(13)?,
                    custom_dictionary: row.get(14)?,
                    save_history: row.get::<_, i64>(15)? != 0,
                    hide_dock_icon: row.get::<_, i64>(16)? != 0,
                    show_hud: row.get::<_, i64>(17)? != 0,
                    show_editor_debug_hud: row.get::<_, i64>(18)? != 0,
                    theme: row.get(19)?,
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

pub fn history_metrics(path: &Path, top_word_limit: u32) -> Result<HistoryMetrics, String> {
    let connection = open(path)?;

    let (total_sessions, total_speaking_time_ms): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(audio_duration_ms), 0) FROM history",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;

    let mut statement = connection
        .prepare("SELECT clean_text, status FROM history")
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?;

    let mut word_counts = HashMap::<String, i64>::new();

    for row in rows {
        let (clean_text, status) = row.map_err(|error| error.to_string())?;
        if !matches!(status.as_str(), "inserted" | "fallback") {
            continue;
        }

        for word in tokenize_words(&clean_text) {
            *word_counts.entry(word).or_insert(0) += 1;
        }
    }

    let total_words_spoken: i64 = word_counts.values().sum();
    let average_words_per_session = if total_sessions > 0 {
        total_words_spoken as f64 / total_sessions as f64
    } else {
        0.0
    };

    // 40 words per minute as a conservative baseline for average typing speed.
    let estimated_typing_time_saved_ms = total_words_spoken * 1_500;

    let top_words = build_top_words(&word_counts, top_word_limit, |_| true);
    let top_meaningful_words =
        build_top_words(&word_counts, top_word_limit, |word| !is_common_word(word));

    Ok(HistoryMetrics {
        total_sessions,
        total_words_spoken,
        total_speaking_time_ms,
        estimated_typing_time_saved_ms,
        average_words_per_session,
        top_words,
        top_meaningful_words,
    })
}

fn build_top_words<F>(
    word_counts: &HashMap<String, i64>,
    top_word_limit: u32,
    include_word: F,
) -> Vec<HistoryWordMetric>
where
    F: Fn(&str) -> bool,
{
    let mut top_words: Vec<HistoryWordMetric> = word_counts
        .iter()
        .filter_map(|(word, count)| {
            include_word(word).then(|| HistoryWordMetric {
                word: word.clone(),
                count: *count,
            })
        })
        .collect();
    top_words.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.word.cmp(&right.word))
    });
    top_words.truncate(top_word_limit as usize);
    top_words
}

fn tokenize_words(input: &str) -> Vec<String> {
    input
        .split(|character: char| !character.is_alphanumeric() && character != '\'')
        .map(|piece| piece.trim_matches('\''))
        .filter(|piece| !piece.is_empty())
        .map(|piece| piece.to_lowercase())
        .collect()
}

fn is_common_word(word: &str) -> bool {
    matches!(
        word,
        "a" | "about"
            | "all"
            | "am"
            | "an"
            | "and"
            | "any"
            | "are"
            | "as"
            | "at"
            | "be"
            | "been"
            | "but"
            | "by"
            | "can"
            | "could"
            | "did"
            | "do"
            | "does"
            | "for"
            | "from"
            | "had"
            | "has"
            | "have"
            | "he"
            | "her"
            | "here"
            | "him"
            | "his"
            | "how"
            | "i"
            | "i'm"
            | "if"
            | "in"
            | "into"
            | "is"
            | "it"
            | "it's"
            | "just"
            | "me"
            | "more"
            | "my"
            | "not"
            | "of"
            | "on"
            | "or"
            | "our"
            | "out"
            | "please"
            | "she"
            | "so"
            | "some"
            | "than"
            | "that"
            | "the"
            | "their"
            | "them"
            | "then"
            | "there"
            | "these"
            | "they"
            | "this"
            | "those"
            | "to"
            | "up"
            | "us"
            | "very"
            | "was"
            | "we"
            | "were"
            | "what"
            | "when"
            | "where"
            | "which"
            | "who"
            | "will"
            | "with"
            | "would"
            | "you"
            | "your"
    )
}
fn save_settings_connection(connection: &Connection, settings: &Settings) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO settings (
                id,
                hotkey,
                auto_paste,
                auto_backtrack,
                editor_file_mentions_enabled,
                force_editor_composer,
                spoken_press_enter_command,
                pause_background_audio,
                warm_model_on_launch,
                speech_model,
                recording_cue_volume,
                restore_clipboard,
                interaction_mode,
                preferred_mode,
                typing_style,
                custom_dictionary,
                save_history,
                hide_dock_icon,
                show_hud,
                show_editor_debug_hud,
                theme
             )
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
             ON CONFLICT(id) DO UPDATE SET
                hotkey = excluded.hotkey,
                auto_paste = excluded.auto_paste,
                auto_backtrack = excluded.auto_backtrack,
                editor_file_mentions_enabled = excluded.editor_file_mentions_enabled,
                force_editor_composer = excluded.force_editor_composer,
                spoken_press_enter_command = excluded.spoken_press_enter_command,
                pause_background_audio = excluded.pause_background_audio,
                warm_model_on_launch = excluded.warm_model_on_launch,
                speech_model = excluded.speech_model,
                recording_cue_volume = excluded.recording_cue_volume,
                restore_clipboard = excluded.restore_clipboard,
                interaction_mode = excluded.interaction_mode,
                preferred_mode = excluded.preferred_mode,
                typing_style = excluded.typing_style,
                custom_dictionary = excluded.custom_dictionary,
                save_history = excluded.save_history,
                hide_dock_icon = excluded.hide_dock_icon,
                show_hud = excluded.show_hud,
                show_editor_debug_hud = excluded.show_editor_debug_hud,
                theme = excluded.theme",
            params![
                settings.hotkey,
                bool_to_integer(settings.auto_paste),
                bool_to_integer(settings.auto_backtrack),
                bool_to_integer(settings.editor_file_mentions_enabled),
                bool_to_integer(settings.force_editor_composer),
                bool_to_integer(settings.spoken_press_enter_command),
                bool_to_integer(settings.pause_background_audio),
                bool_to_integer(settings.warm_model_on_launch),
                settings.speech_model.as_db_value(),
                settings.recording_cue_volume,
                bool_to_integer(settings.restore_clipboard),
                settings.interaction_mode,
                settings.preferred_mode,
                settings.typing_style,
                settings.custom_dictionary,
                bool_to_integer(settings.save_history),
                bool_to_integer(settings.hide_dock_icon),
                bool_to_integer(settings.show_hud),
                bool_to_integer(settings.show_editor_debug_hud),
                settings.theme,
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
        "auto_backtrack",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "editor_file_mentions_enabled",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "force_editor_composer",
        "INTEGER NOT NULL DEFAULT 0",
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
        "speech_model",
        "TEXT NOT NULL DEFAULT 'base_en'",
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
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "show_editor_debug_hud",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "typing_style",
        "TEXT NOT NULL DEFAULT 'formal'",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "custom_dictionary",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        connection,
        "settings",
        "theme",
        "TEXT NOT NULL DEFAULT 'system'",
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
              auto_backtrack INTEGER NOT NULL DEFAULT 0,
              editor_file_mentions_enabled INTEGER NOT NULL DEFAULT 0,
              force_editor_composer INTEGER NOT NULL DEFAULT 0,
              spoken_press_enter_command INTEGER NOT NULL DEFAULT 0,
              pause_background_audio INTEGER NOT NULL DEFAULT 1,
              warm_model_on_launch INTEGER NOT NULL DEFAULT 1,
              speech_model TEXT NOT NULL DEFAULT 'base_en',
              recording_cue_volume INTEGER NOT NULL DEFAULT 75,
              restore_clipboard INTEGER NOT NULL,
              interaction_mode TEXT NOT NULL DEFAULT 'hold',
              preferred_mode TEXT NOT NULL,
              typing_style TEXT NOT NULL DEFAULT 'formal',
              custom_dictionary TEXT NOT NULL DEFAULT '',
              save_history INTEGER NOT NULL,
              hide_dock_icon INTEGER NOT NULL DEFAULT 0,
              show_hud INTEGER NOT NULL DEFAULT 1,
              show_editor_debug_hud INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO settings_next (
              id,
              hotkey,
              auto_paste,
              auto_backtrack,
              editor_file_mentions_enabled,
              force_editor_composer,
              spoken_press_enter_command,
              pause_background_audio,
              warm_model_on_launch,
              speech_model,
              recording_cue_volume,
              restore_clipboard,
              interaction_mode,
              preferred_mode,
              typing_style,
              custom_dictionary,
              save_history,
              hide_dock_icon,
              show_hud,
              show_editor_debug_hud
            )
            SELECT
              id,
              hotkey,
              auto_paste,
              0,
              0,
              0,
              0,
              COALESCE(pause_background_audio, 1),
              1,
              'base_en',
              COALESCE(recording_cue_volume, 75),
              restore_clipboard,
              COALESCE(interaction_mode, 'hold'),
              preferred_mode,
              'formal',
              '',
              save_history,
              0,
              1,
              0
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
    let normalized_dictionary = normalize_custom_dictionary(&settings.custom_dictionary);
    let normalized_mode = normalize_preferred_mode(&settings.preferred_mode);
    let normalized_style = normalize_typing_style(&settings.typing_style);
    let dictionary_changed = normalized_dictionary != settings.custom_dictionary;
    let mode_changed = normalized_mode != settings.preferred_mode;
    let style_changed = normalized_style != settings.typing_style;

    if !dictionary_changed && !mode_changed && !style_changed {
        return false;
    }

    settings.custom_dictionary = normalized_dictionary;
    settings.preferred_mode = normalized_mode;
    settings.typing_style = normalized_style;
    true
}

fn normalize_custom_dictionary(dictionary: &str) -> String {
    dictionary
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_preferred_mode(mode: &str) -> String {
    match mode {
        "notes" => "bullet_list".to_string(),
        "default" | "chat" | "bullet_list" | "auto" => mode.to_string(),
        _ => "default".to_string(),
    }
}

fn normalize_typing_style(style: &str) -> String {
    match style {
        "very_casual" | "formal" | "excited" => style.to_string(),
        _ => "formal".to_string(),
    }
}

fn normalize_history_mode(mode: &str) -> String {
    match mode {
        "notes" => "bullet_list".to_string(),
        "default" | "chat" | "bullet_list" | "auto" => mode.to_string(),
        _ => "default".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::*;

    fn temp_db_path() -> std::path::PathBuf {
        env::temp_dir().join(format!("kiflo-test-{}.sqlite3", Uuid::new_v4()))
    }

    #[test]
    fn initializes_and_round_trips_settings() {
        let path = temp_db_path();
        initialize(&path).unwrap();

        let mut settings = get_settings(&path).unwrap();
        assert!(settings.warm_model_on_launch);
        assert_eq!(settings.speech_model, SpeechModel::BaseEn);
        assert!(!settings.editor_file_mentions_enabled);
        assert!(!settings.force_editor_composer);
        settings.hotkey = "Option+Shift+Space".to_string();
        settings.interaction_mode = "toggle".to_string();
        settings.auto_backtrack = true;
        settings.editor_file_mentions_enabled = true;
        settings.force_editor_composer = true;
        settings.spoken_press_enter_command = true;
        settings.pause_background_audio = false;
        settings.warm_model_on_launch = false;
        settings.speech_model = SpeechModel::SmallEn;
        settings.recording_cue_volume = 0;
        settings.typing_style = "excited".to_string();
        settings.custom_dictionary = "Rebecca => Rebekah  \nKatherine => Kathryn\t".to_string();
        settings.save_history = false;
        settings.hide_dock_icon = true;
        settings.show_hud = false;

        update_settings(&path, &settings).unwrap();

        let loaded = get_settings(&path).unwrap();
        assert_eq!(loaded.hotkey, "Option+Shift+Space");
        assert_eq!(loaded.interaction_mode, "toggle");
        assert!(loaded.auto_backtrack);
        assert!(loaded.editor_file_mentions_enabled);
        assert!(loaded.force_editor_composer);
        assert!(loaded.spoken_press_enter_command);
        assert!(!loaded.pause_background_audio);
        assert!(!loaded.warm_model_on_launch);
        assert_eq!(loaded.speech_model, SpeechModel::SmallEn);
        assert_eq!(loaded.recording_cue_volume, 0);
        assert_eq!(loaded.typing_style, "excited");
        assert_eq!(
            loaded.custom_dictionary,
            "Rebecca => Rebekah\nKatherine => Kathryn"
        );
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
        assert_eq!(migrated.speech_model, SpeechModel::BaseEn);
        assert_eq!(migrated.typing_style, "formal");
        assert!(migrated.custom_dictionary.is_empty());
        assert!(!migrated.auto_backtrack);
        assert!(!migrated.editor_file_mentions_enabled);
        assert!(!migrated.force_editor_composer);
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

    #[test]
    fn normalizes_invalid_typing_style_to_formal() {
        let path = temp_db_path();
        initialize(&path).unwrap();

        let connection = open(&path).unwrap();
        connection
            .execute("UPDATE settings SET typing_style = 'loud' WHERE id = 1", [])
            .unwrap();
        drop(connection);

        let settings = get_settings(&path).unwrap();
        assert_eq!(settings.typing_style, "formal");

        let persisted_style: String = open(&path)
            .unwrap()
            .query_row(
                "SELECT typing_style FROM settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted_style, "formal");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn adds_auto_backtrack_column_for_existing_settings_tables() {
        let path = temp_db_path();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE settings (
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
                  custom_dictionary TEXT NOT NULL DEFAULT '',
                  save_history INTEGER NOT NULL,
                  hide_dock_icon INTEGER NOT NULL DEFAULT 0,
                  show_hud INTEGER NOT NULL DEFAULT 1
                );
                INSERT INTO settings (
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
                  custom_dictionary,
                  save_history,
                  hide_dock_icon,
                  show_hud
                ) VALUES (
                  1,
                  'Option+Space',
                  1,
                  0,
                  1,
                  1,
                  75,
                  1,
                  'hold',
                  'default',
                  '',
                  1,
                  0,
                  1
                );
                "#,
            )
            .unwrap();
        drop(connection);

        initialize(&path).unwrap();

        let settings = get_settings(&path).unwrap();
        assert!(!settings.auto_backtrack);
        assert!(!settings.editor_file_mentions_enabled);
        assert!(!settings.force_editor_composer);
        assert_eq!(settings.speech_model, SpeechModel::BaseEn);
        assert!(column_exists(&open(&path).unwrap(), "settings", "auto_backtrack").unwrap());
        assert!(column_exists(
            &open(&path).unwrap(),
            "settings",
            "editor_file_mentions_enabled"
        )
        .unwrap());
        assert!(column_exists(&open(&path).unwrap(), "settings", "force_editor_composer").unwrap());
        assert!(column_exists(&open(&path).unwrap(), "settings", "speech_model").unwrap());
        assert_eq!(settings.typing_style, "formal");
        assert!(column_exists(&open(&path).unwrap(), "settings", "typing_style").unwrap());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn computes_history_metrics() {
        let path = temp_db_path();
        initialize(&path).unwrap();

        create_history_record(
            &path,
            &NewHistoryRecord {
                app_name: Some("TextEdit".to_string()),
                mode: "default".to_string(),
                audio_duration_ms: 1_000,
                raw_text: "Hello hello world".to_string(),
                clean_text: "Hello hello world".to_string(),
                status: "inserted".to_string(),
            },
        )
        .unwrap();
        create_history_record(
            &path,
            &NewHistoryRecord {
                app_name: Some("Slack".to_string()),
                mode: "chat".to_string(),
                audio_duration_ms: 500,
                raw_text: "World of words".to_string(),
                clean_text: "World of words".to_string(),
                status: "inserted".to_string(),
            },
        )
        .unwrap();

        let metrics = history_metrics(&path, 3).unwrap();
        assert_eq!(metrics.total_sessions, 2);
        assert_eq!(metrics.total_words_spoken, 6);
        assert_eq!(metrics.total_speaking_time_ms, 1_500);
        assert_eq!(metrics.estimated_typing_time_saved_ms, 9_000);
        assert_eq!(metrics.average_words_per_session, 3.0);
        assert_eq!(metrics.top_words.len(), 3);
        assert_eq!(metrics.top_words[0].word, "hello");
        assert_eq!(metrics.top_words[0].count, 2);
        assert_eq!(metrics.top_words[1].word, "world");
        assert_eq!(metrics.top_words[1].count, 2);
        assert_eq!(metrics.top_meaningful_words.len(), 3);
        assert_eq!(metrics.top_meaningful_words[0].word, "hello");
        assert_eq!(metrics.top_meaningful_words[1].word, "world");
        assert_eq!(metrics.top_meaningful_words[2].word, "words");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn filters_common_words_from_meaningful_word_metrics() {
        let path = temp_db_path();
        initialize(&path).unwrap();

        create_history_record(
            &path,
            &NewHistoryRecord {
                app_name: Some("Mail".to_string()),
                mode: "default".to_string(),
                audio_duration_ms: 800,
                raw_text: "the update is ready".to_string(),
                clean_text: "The update is ready.".to_string(),
                status: "inserted".to_string(),
            },
        )
        .unwrap();
        create_history_record(
            &path,
            &NewHistoryRecord {
                app_name: Some("Slack".to_string()),
                mode: "chat".to_string(),
                audio_duration_ms: 700,
                raw_text: "i sent the update".to_string(),
                clean_text: "I sent the update.".to_string(),
                status: "inserted".to_string(),
            },
        )
        .unwrap();

        let metrics = history_metrics(&path, 4).unwrap();
        assert_eq!(metrics.top_words[0].word, "the");
        assert_eq!(metrics.top_words[1].word, "update");
        assert_eq!(metrics.top_meaningful_words[0].word, "update");
        assert!(metrics
            .top_meaningful_words
            .iter()
            .all(|item| item.word != "the"));
        assert!(metrics
            .top_meaningful_words
            .iter()
            .all(|item| item.word != "i"));

        let _ = fs::remove_file(path);
    }
}
