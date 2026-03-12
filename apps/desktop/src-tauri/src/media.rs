use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(target_os = "macos")]
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[cfg(target_os = "macos")]
use serde::Deserialize;

#[derive(Debug, Default)]
pub struct MediaPlaybackInterruption {
    resume_required: bool,
}

impl MediaPlaybackInterruption {
    pub fn begin() -> Self {
        let backend = SystemMediaPlaybackBackend;
        match begin_interruption(&backend) {
            Ok(interruption) => interruption,
            Err(error) => {
                eprintln!("media playback pause failed: {error}");
                Self::default()
            }
        }
    }

    pub fn resume(&mut self) {
        let backend = SystemMediaPlaybackBackend;
        if let Err(error) = self.resume_with(&backend) {
            eprintln!("media playback resume failed: {error}");
        }
    }

    fn resume_with<B: MediaPlaybackBackend>(&mut self, backend: &B) -> Result<(), String> {
        if !self.resume_required {
            return Ok(());
        }

        backend.resume_playback()?;
        self.resume_required = false;
        Ok(())
    }
}

trait MediaPlaybackBackend {
    fn is_media_playing(&self) -> Result<bool, String>;
    fn pause_playback(&self) -> Result<(), String>;
    fn resume_playback(&self) -> Result<(), String>;
}

fn begin_interruption<B: MediaPlaybackBackend>(
    backend: &B,
) -> Result<MediaPlaybackInterruption, String> {
    if !backend.is_media_playing()? {
        return Ok(MediaPlaybackInterruption::default());
    }

    backend.pause_playback()?;
    Ok(MediaPlaybackInterruption {
        resume_required: true,
    })
}

struct SystemMediaPlaybackBackend;

impl MediaPlaybackBackend for SystemMediaPlaybackBackend {
    fn is_media_playing(&self) -> Result<bool, String> {
        is_media_playing()
    }

    fn pause_playback(&self) -> Result<(), String> {
        pause_system_media()
    }

    fn resume_playback(&self) -> Result<(), String> {
        resume_system_media()
    }
}

#[cfg(target_os = "macos")]
fn is_media_playing() -> Result<bool, String> {
    query_now_playing_state().map(|state| state.is_playing)
}

#[cfg(target_os = "macos")]
#[link(name = "MediaRemote", kind = "framework")]
unsafe extern "C" {
    fn MRMediaRemoteSendCommand(command: i32, user_info: *const std::ffi::c_void) -> bool;
}

#[cfg(target_os = "macos")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NowPlayingState {
    is_playing: bool,
}

#[cfg(not(target_os = "macos"))]
fn is_media_playing() -> Result<bool, String> {
    Ok(false)
}

#[cfg(target_os = "macos")]
fn query_now_playing_state() -> Result<NowPlayingState, String> {
    const NOW_PLAYING_SCRIPT: &str = r#"
function run() {
  const MediaRemote = $.NSBundle.bundleWithPath("/System/Library/PrivateFrameworks/MediaRemote.framework/");
  MediaRemote.load;

  const MRNowPlayingRequest = $.NSClassFromString("MRNowPlayingRequest");
  if (!MRNowPlayingRequest) {
    return JSON.stringify({ isPlaying: false });
  }

  return JSON.stringify({
    isPlaying: Boolean(MRNowPlayingRequest.localIsPlaying)
  });
}
"#;

    let mut child = Command::new("osascript")
        .args(["-l", "JavaScript"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!("Unable to launch osascript for media state detection: {error}")
        })?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| "osascript stdin was unavailable.".to_string())?
        .write_all(NOW_PLAYING_SCRIPT.as_bytes())
        .map_err(|error| format!("Unable to send media state script to osascript: {error}"))?;

    let output = child
        .wait_with_output()
        .map_err(|error| format!("Unable to read osascript output: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "osascript returned a non-zero exit code while detecting media playback: {}",
            output.status
        ));
    }

    serde_json::from_slice::<NowPlayingState>(&output.stdout)
        .map_err(|error| format!("Unable to parse media playback state: {error}"))
}

#[cfg(target_os = "macos")]
fn pause_system_media() -> Result<(), String> {
    send_media_command(MediaRemoteCommand::Pause)
}

#[cfg(target_os = "macos")]
fn resume_system_media() -> Result<(), String> {
    send_media_command(MediaRemoteCommand::Play)
}

#[cfg(target_os = "macos")]
fn send_media_command(command: MediaRemoteCommand) -> Result<(), String> {
    let sent = unsafe { MRMediaRemoteSendCommand(command as i32, std::ptr::null()) };

    if sent {
        Ok(())
    } else {
        toggle_media_playback()
    }
}

#[cfg(not(target_os = "macos"))]
fn pause_system_media() -> Result<(), String> {
    toggle_media_playback()
}

#[cfg(not(target_os = "macos"))]
fn resume_system_media() -> Result<(), String> {
    toggle_media_playback()
}

#[cfg(target_os = "macos")]
#[repr(i32)]
enum MediaRemoteCommand {
    Play = 0,
    Pause = 1,
}

fn toggle_media_playback() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    enigo
        .key(Key::MediaPlayPause, Direction::Click)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
    };

    use super::*;

    struct FakeMediaPlaybackBackend {
        media_playing: Result<bool, String>,
        pause_results: RefCell<VecDeque<Result<(), String>>>,
        resume_results: RefCell<VecDeque<Result<(), String>>>,
        pause_count: Cell<usize>,
        resume_count: Cell<usize>,
    }

    impl FakeMediaPlaybackBackend {
        fn active() -> Self {
            Self {
                media_playing: Ok(true),
                pause_results: RefCell::new(VecDeque::from([Ok(())])),
                resume_results: RefCell::new(VecDeque::from([Ok(())])),
                pause_count: Cell::new(0),
                resume_count: Cell::new(0),
            }
        }

        fn idle() -> Self {
            Self {
                media_playing: Ok(false),
                pause_results: RefCell::new(VecDeque::new()),
                resume_results: RefCell::new(VecDeque::new()),
                pause_count: Cell::new(0),
                resume_count: Cell::new(0),
            }
        }

        fn with_results(
            pause_results: Vec<Result<(), String>>,
            resume_results: Vec<Result<(), String>>,
        ) -> Self {
            Self {
                media_playing: Ok(true),
                pause_results: RefCell::new(pause_results.into()),
                resume_results: RefCell::new(resume_results.into()),
                pause_count: Cell::new(0),
                resume_count: Cell::new(0),
            }
        }
    }

    impl MediaPlaybackBackend for FakeMediaPlaybackBackend {
        fn is_media_playing(&self) -> Result<bool, String> {
            self.media_playing.clone()
        }

        fn pause_playback(&self) -> Result<(), String> {
            self.pause_count.set(self.pause_count.get() + 1);
            self.pause_results
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(()))
        }

        fn resume_playback(&self) -> Result<(), String> {
            self.resume_count.set(self.resume_count.get() + 1);
            self.resume_results
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(()))
        }
    }

    #[test]
    fn begins_interruption_only_when_media_is_playing() {
        let idle_backend = FakeMediaPlaybackBackend::idle();
        let idle_interruption = begin_interruption(&idle_backend).unwrap();
        assert!(!idle_interruption.resume_required);
        assert_eq!(idle_backend.pause_count.get(), 0);
        assert_eq!(idle_backend.resume_count.get(), 0);

        let active_backend = FakeMediaPlaybackBackend::active();
        let active_interruption = begin_interruption(&active_backend).unwrap();
        assert!(active_interruption.resume_required);
        assert_eq!(active_backend.pause_count.get(), 1);
        assert_eq!(active_backend.resume_count.get(), 0);
    }

    #[test]
    fn resume_only_plays_once_after_a_successful_pause() {
        let backend = FakeMediaPlaybackBackend::with_results(vec![Ok(())], vec![Ok(())]);
        let mut interruption = begin_interruption(&backend).unwrap();

        interruption.resume_with(&backend).unwrap();
        interruption.resume_with(&backend).unwrap();

        assert_eq!(backend.pause_count.get(), 1);
        assert_eq!(backend.resume_count.get(), 1);
        assert!(!interruption.resume_required);
    }

    #[test]
    fn resume_keeps_the_resume_flag_when_resume_fails() {
        let backend = FakeMediaPlaybackBackend::with_results(
            vec![Ok(())],
            vec![Err("resume failed".to_string())],
        );
        let mut interruption = begin_interruption(&backend).unwrap();

        let error = interruption.resume_with(&backend).unwrap_err();

        assert_eq!(error, "resume failed");
        assert_eq!(backend.pause_count.get(), 1);
        assert_eq!(backend.resume_count.get(), 1);
        assert!(interruption.resume_required);
    }
}
