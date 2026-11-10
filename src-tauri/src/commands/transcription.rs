use crate::actions::FinishGuard;
use crate::audio_toolkit::constants::WHISPER_SAMPLE_RATE;
use crate::language_detection::LanguageStatus;
use crate::managers::model::ModelManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings, ModelUnloadTimeout};
use crate::TranscriptionCoordinator;
use serde::Serialize;
use specta::Type;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

/// Largest WAV accepted for transcription: about 52 minutes of 16 kHz mono PCM.
const MAX_WAV_BYTES: u64 = 100 * 1024 * 1024;

/// Rejects anything but 16 kHz mono 16-bit PCM, the only format the
/// transcription pipeline is fed.
fn check_wav_spec(spec: hound::WavSpec) -> Result<(), String> {
    if spec.sample_rate != WHISPER_SAMPLE_RATE
        || spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err(format!(
            "Expected a 16 kHz mono 16-bit PCM WAV file, got {} Hz / {} channel(s) / {}-bit {:?}",
            spec.sample_rate, spec.channels, spec.bits_per_sample, spec.sample_format
        ));
    }
    Ok(())
}

/// Reads a WAV file chosen by the user, after checking its size and format.
/// Opens the file once: spec validation and sample reading share the same handle.
fn load_wav(path: &Path) -> Result<Vec<f32>, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("Cannot open file: {e}"))?
        .len();
    if size > MAX_WAV_BYTES {
        return Err(format!(
            "File is too large (limit {} MB)",
            MAX_WAV_BYTES / (1024 * 1024)
        ));
    }

    let reader =
        hound::WavReader::open(path).map_err(|e| format!("Not a valid WAV file: {e}"))?;
    check_wav_spec(reader.spec())?;

    let samples: Vec<f32> = reader
        .into_samples::<i16>()
        .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read WAV samples: {e}"))?;

    if samples.is_empty() {
        return Err("The WAV file contains no audio".to_string());
    }
    Ok(samples)
}

#[derive(Serialize, Type)]
pub struct ModelLoadStatus {
    is_loaded: bool,
    current_model: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn set_model_unload_timeout(app: AppHandle, timeout: ModelUnloadTimeout) {
    let mut settings = get_settings(&app);
    settings.model_unload_timeout = timeout;
    write_settings(&app, settings);
}

/// The language transcription on `model_id` will use, for the picker.
#[tauri::command]
#[specta::specta]
pub fn get_language_status(
    app: AppHandle,
    model_manager: State<Arc<ModelManager>>,
    model_id: String,
) -> LanguageStatus {
    let settings = get_settings(&app);
    crate::language_detection::status_for(
        &settings,
        model_manager.get_model_info(&model_id).as_ref(),
        model_manager.language_detector_path().is_some(),
    )
}

#[tauri::command]
#[specta::specta]
pub fn get_model_load_status(
    transcription_manager: State<Arc<TranscriptionManager>>,
) -> Result<ModelLoadStatus, String> {
    Ok(ModelLoadStatus {
        is_loaded: transcription_manager.is_model_loaded(),
        current_model: transcription_manager.get_current_model(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn unload_model_manually(
    transcription_manager: State<Arc<TranscriptionManager>>,
) -> Result<(), String> {
    transcription_manager.ensure_model_unlocked()?;
    transcription_manager.request_unload();
    Ok(())
}


/// Transcribes a WAV file with the selected model, through the same batch
/// pipeline as recordings. Nothing is pasted or saved to history.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_wav_file(
    app: AppHandle,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    path: String,
) -> Result<String, String> {
    let tm = Arc::clone(&transcription_manager);
    tauri::async_runtime::spawn_blocking(move || {
        // Reserve the pipeline first so the coordinator transition is visible
        // to shortcut handlers before any significant work begins. Validation
        // errors (file not found, wrong format) release the reservation
        // immediately via the FinishGuard drop.
        let coordinator = app
            .try_state::<TranscriptionCoordinator>()
            .ok_or_else(|| "Transcription is not ready yet".to_string())?;
        if !coordinator.try_begin_file_processing() {
            return Err(
                "Handy is busy with another transcription. Try again shortly.".to_string(),
            );
        }
        // FinishGuard releases the pipeline (calls notify_processing_finished)
        // on any exit path, including early returns below.
        let _guard = FinishGuard(app.clone(), Arc::clone(&tm));

        let samples = load_wav(Path::new(&path))?;
        tm.initiate_model_load();
        tm.transcribe(samples).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Transcription task panicked: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wav(path: &Path, sample_rate: u32, channels: u16, samples: usize) {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..samples {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn load_wav_accepts_16khz_mono() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ok.wav");
        write_wav(&path, 16_000, 1, 160);

        assert_eq!(load_wav(&path).unwrap().len(), 160);
    }

    #[test]
    fn load_wav_rejects_other_formats() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stereo.wav");
        write_wav(&path, 44_100, 2, 160);

        assert!(load_wav(&path).unwrap_err().contains("16 kHz mono"));
    }

    #[test]
    fn load_wav_rejects_empty_and_non_wav_files() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.wav");
        write_wav(&empty, 16_000, 1, 0);
        let text = dir.path().join("text.wav");
        std::fs::write(&text, b"not audio").unwrap();

        assert!(load_wav(&empty).unwrap_err().contains("no audio"));
        assert!(load_wav(&text).unwrap_err().contains("Not a valid WAV"));
    }
}
