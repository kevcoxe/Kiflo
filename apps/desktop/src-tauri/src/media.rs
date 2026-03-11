use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(target_os = "macos")]
use std::{mem, ptr::NonNull};

#[cfg(target_os = "macos")]
use objc2_core_audio::{
    kAudioDevicePropertyDeviceIsRunningSomewhere, kAudioHardwareNoError,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, AudioObjectGetPropertyData,
    AudioObjectID, AudioObjectPropertyAddress,
};

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

        backend.toggle_play_pause()?;
        self.resume_required = false;
        Ok(())
    }
}

trait MediaPlaybackBackend {
    fn is_output_active(&self) -> Result<bool, String>;
    fn toggle_play_pause(&self) -> Result<(), String>;
}

fn begin_interruption<B: MediaPlaybackBackend>(
    backend: &B,
) -> Result<MediaPlaybackInterruption, String> {
    if !backend.is_output_active()? {
        return Ok(MediaPlaybackInterruption::default());
    }

    backend.toggle_play_pause()?;
    Ok(MediaPlaybackInterruption {
        resume_required: true,
    })
}

struct SystemMediaPlaybackBackend;

impl MediaPlaybackBackend for SystemMediaPlaybackBackend {
    fn is_output_active(&self) -> Result<bool, String> {
        is_output_audio_active()
    }

    fn toggle_play_pause(&self) -> Result<(), String> {
        toggle_media_playback()
    }
}

#[cfg(target_os = "macos")]
fn is_output_audio_active() -> Result<bool, String> {
    let default_output_device = read_audio_property::<AudioObjectID>(
        kAudioObjectSystemObject as AudioObjectID,
        AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        },
    )?;

    if default_output_device == 0 {
        return Ok(false);
    }

    let is_running = read_audio_property::<u32>(
        default_output_device,
        AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyDeviceIsRunningSomewhere,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain,
        },
    )?;

    Ok(is_running != 0)
}

#[cfg(target_os = "macos")]
fn read_audio_property<T: Copy>(
    object_id: AudioObjectID,
    address: AudioObjectPropertyAddress,
) -> Result<T, String> {
    let mut value = mem::MaybeUninit::<T>::uninit();
    let mut data_size = mem::size_of::<T>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            object_id,
            NonNull::from(&address),
            0,
            std::ptr::null(),
            NonNull::from(&mut data_size),
            NonNull::new(value.as_mut_ptr().cast())
                .ok_or_else(|| "Audio property pointer was null.".to_string())?,
        )
    };

    if status != kAudioHardwareNoError {
        return Err(format!(
            "CoreAudio property lookup failed with status {status}."
        ));
    }

    if data_size != mem::size_of::<T>() as u32 {
        return Err(format!(
            "CoreAudio property size mismatch. Expected {} bytes, received {} bytes.",
            mem::size_of::<T>(),
            data_size,
        ));
    }

    Ok(unsafe { value.assume_init() })
}

#[cfg(not(target_os = "macos"))]
fn is_output_audio_active() -> Result<bool, String> {
    Ok(false)
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
        output_active: Result<bool, String>,
        toggle_results: RefCell<VecDeque<Result<(), String>>>,
        toggle_count: Cell<usize>,
    }

    impl FakeMediaPlaybackBackend {
        fn active() -> Self {
            Self {
                output_active: Ok(true),
                toggle_results: RefCell::new(VecDeque::from([Ok(())])),
                toggle_count: Cell::new(0),
            }
        }

        fn idle() -> Self {
            Self {
                output_active: Ok(false),
                toggle_results: RefCell::new(VecDeque::new()),
                toggle_count: Cell::new(0),
            }
        }

        fn with_toggle_results(toggle_results: Vec<Result<(), String>>) -> Self {
            Self {
                output_active: Ok(true),
                toggle_results: RefCell::new(toggle_results.into()),
                toggle_count: Cell::new(0),
            }
        }
    }

    impl MediaPlaybackBackend for FakeMediaPlaybackBackend {
        fn is_output_active(&self) -> Result<bool, String> {
            self.output_active.clone()
        }

        fn toggle_play_pause(&self) -> Result<(), String> {
            self.toggle_count.set(self.toggle_count.get() + 1);
            self.toggle_results
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(()))
        }
    }

    #[test]
    fn begins_interruption_only_when_output_is_active() {
        let idle_backend = FakeMediaPlaybackBackend::idle();
        let idle_interruption = begin_interruption(&idle_backend).unwrap();
        assert!(!idle_interruption.resume_required);
        assert_eq!(idle_backend.toggle_count.get(), 0);

        let active_backend = FakeMediaPlaybackBackend::active();
        let active_interruption = begin_interruption(&active_backend).unwrap();
        assert!(active_interruption.resume_required);
        assert_eq!(active_backend.toggle_count.get(), 1);
    }

    #[test]
    fn resume_only_toggles_once_after_a_successful_pause() {
        let backend = FakeMediaPlaybackBackend::with_toggle_results(vec![Ok(()), Ok(())]);
        let mut interruption = begin_interruption(&backend).unwrap();

        interruption.resume_with(&backend).unwrap();
        interruption.resume_with(&backend).unwrap();

        assert_eq!(backend.toggle_count.get(), 2);
        assert!(!interruption.resume_required);
    }

    #[test]
    fn resume_keeps_the_resume_flag_when_the_toggle_fails() {
        let backend = FakeMediaPlaybackBackend::with_toggle_results(vec![
            Ok(()),
            Err("resume failed".to_string()),
        ]);
        let mut interruption = begin_interruption(&backend).unwrap();

        let error = interruption.resume_with(&backend).unwrap_err();

        assert_eq!(error, "resume failed");
        assert_eq!(backend.toggle_count.get(), 2);
        assert!(interruption.resume_required);
    }
}
