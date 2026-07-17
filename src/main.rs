#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::SystemTime;

use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveTime};
use lmu2motec::converter::{ConvertOptions, ExportMode, convert_file_with_progress};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

slint::include_modules!();

fn main() -> Result<()> {
    let ui = MainWindow::new()?;
    let files = Rc::new(VecModel::<TelemetryFile>::default());
    ui.set_files(ModelRc::from(files.clone()));

    let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let settings_path = settings_path(&current);
    let settings = load_settings(&settings_path);
    let source_folder = settings
        .as_ref()
        .map(|settings| settings.source_folder.clone())
        .filter(|folder| folder.is_dir())
        .unwrap_or_else(|| current.clone());
    let output_folder = settings
        .as_ref()
        .map(|settings| settings.output_folder.clone())
        .filter(|folder| !folder.as_os_str().is_empty())
        .unwrap_or_else(|| current.join("output"));
    ui.set_source_folder(source_folder.display().to_string().into());
    ui.set_output_folder(output_folder.display().to_string().into());
    ui.set_export_mode(
        settings
            .as_ref()
            .map_or(0, |settings| settings.export_mode.clamp(0, 2)),
    );
    ui.set_requested_lap(
        settings
            .as_ref()
            .map_or(1, |settings| settings.requested_lap.max(1)),
    );

    wire_browse_source(&ui, files.clone(), settings_path.clone());
    wire_browse_output(&ui, settings_path.clone());
    wire_open_output(&ui);
    wire_save_settings(&ui, settings_path.clone());
    wire_refresh(&ui, files.clone(), settings_path.clone());
    wire_selection(&ui, files.clone());
    wire_conversion(&ui, files.clone());

    refresh_files(&ui, &files, &source_folder);
    ui.run()?;
    let settings = AppSettings {
        source_folder: PathBuf::from(ui.get_source_folder().as_str()),
        output_folder: PathBuf::from(ui.get_output_folder().as_str()),
        export_mode: ui.get_export_mode(),
        requested_lap: ui.get_requested_lap(),
    };
    save_settings(&settings_path, &settings)?;
    Ok(())
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct AppSettings {
    source_folder: PathBuf,
    output_folder: PathBuf,
    #[serde(default)]
    export_mode: i32,
    #[serde(default = "default_requested_lap")]
    requested_lap: i32,
}

fn default_requested_lap() -> i32 {
    1
}

fn settings_path(current: &Path) -> PathBuf {
    current.join("lmu2motec_config.toml")
}

fn load_settings(path: &Path) -> Option<AppSettings> {
    if let Ok(contents) = std::fs::read_to_string(path)
        && let Ok(settings) = toml::from_str(&contents)
    {
        return Some(settings);
    }

    let legacy_path = path.with_file_name("settings.txt");
    let settings = load_legacy_settings(&legacy_path)?;
    let _ = save_settings(path, &settings);
    Some(settings)
}

fn load_legacy_settings(path: &Path) -> Option<AppSettings> {
    let contents = std::fs::read_to_string(path).ok()?;
    let mut lines = contents.lines();
    let source_folder = PathBuf::from(lines.next()?);
    let output_folder = PathBuf::from(lines.next().unwrap_or_default());
    Some(AppSettings {
        source_folder,
        output_folder,
        export_mode: 0,
        requested_lap: 1,
    })
}

fn save_settings(path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(settings).context("failed to serialize settings")?;
    std::fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn save_current_settings(ui: &MainWindow, path: &Path) {
    let settings = AppSettings {
        source_folder: PathBuf::from(ui.get_source_folder().as_str()),
        output_folder: PathBuf::from(ui.get_output_folder().as_str()),
        export_mode: ui.get_export_mode(),
        requested_lap: ui.get_requested_lap(),
    };
    if let Err(error) = save_settings(path, &settings) {
        ui.set_status_text(format!("Unable to save settings: {error:#}").into());
    }
}

fn wire_browse_source(ui: &MainWindow, files: Rc<VecModel<TelemetryFile>>, settings_path: PathBuf) {
    let weak = ui.as_weak();
    ui.on_browse_source(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let current = PathBuf::from(ui.get_source_folder().as_str());
        let mut dialog = FileDialog::new().set_title("LMU telemetry folder");
        if current.is_dir() {
            dialog = dialog.set_directory(current);
        }
        if let Some(folder) = dialog.pick_folder() {
            ui.set_source_folder(folder.display().to_string().into());
            if ui.get_output_folder().is_empty() {
                ui.set_output_folder(folder.join("output").display().to_string().into());
            }
            refresh_files(&ui, &files, &folder);
            save_current_settings(&ui, &settings_path);
        }
    });
}

fn wire_browse_output(ui: &MainWindow, settings_path: PathBuf) {
    let weak = ui.as_weak();
    ui.on_browse_output(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let current = PathBuf::from(ui.get_output_folder().as_str());
        let mut dialog = FileDialog::new().set_title("MoTeC output folder");
        if current.is_dir() {
            dialog = dialog.set_directory(current);
        }
        if let Some(folder) = dialog.pick_folder() {
            ui.set_output_folder(folder.display().to_string().into());
            save_current_settings(&ui, &settings_path);
        }
    });
}

fn wire_open_output(ui: &MainWindow) {
    let weak = ui.as_weak();
    ui.on_open_output_folder(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let folder = PathBuf::from(ui.get_output_folder().as_str());
        if folder.as_os_str().is_empty() {
            ui.set_status_text("Choose an output folder.".into());
            return;
        }

        let result = resolve_folder(&folder).and_then(|folder| open_folder(&folder));
        if let Err(error) = result {
            ui.set_status_text(format!("Unable to open output folder: {error:#}").into());
        }
    });
}

fn wire_save_settings(ui: &MainWindow, settings_path: PathBuf) {
    let weak = ui.as_weak();
    ui.on_save_settings(move || {
        if let Some(ui) = weak.upgrade() {
            save_current_settings(&ui, &settings_path);
        }
    });
}

fn resolve_folder(folder: &Path) -> Result<PathBuf> {
    let absolute = if folder.is_absolute() {
        folder.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to determine the launch directory")?
            .join(folder)
    };
    std::fs::create_dir_all(&absolute)
        .with_context(|| format!("failed to create {}", absolute.display()))?;
    let canonical = absolute
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", absolute.display()))?;
    Ok(platform_folder_path(canonical))
}

#[cfg(target_os = "windows")]
fn platform_folder_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\UNC\")
        .map(|path| PathBuf::from(format!(r"\\{path}")))
        .or_else(|| text.strip_prefix(r"\\?\").map(PathBuf::from))
        .unwrap_or(path)
}

#[cfg(not(target_os = "windows"))]
fn platform_folder_path(path: PathBuf) -> PathBuf {
    path
}

fn open_folder(folder: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");

    command
        .arg(folder)
        .spawn()
        .with_context(|| format!("failed to open {}", folder.display()))?;
    Ok(())
}

fn wire_refresh(ui: &MainWindow, files: Rc<VecModel<TelemetryFile>>, settings_path: PathBuf) {
    let weak = ui.as_weak();
    ui.on_refresh_files(move || {
        if let Some(ui) = weak.upgrade() {
            let folder = PathBuf::from(ui.get_source_folder().as_str());
            refresh_files(&ui, &files, &folder);
            save_current_settings(&ui, &settings_path);
        }
    });
}

fn wire_selection(ui: &MainWindow, files: Rc<VecModel<TelemetryFile>>) {
    let weak = ui.as_weak();
    let toggle_files = files.clone();
    ui.on_toggle_file(move |index, selected| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        if let Some(mut row) = toggle_files.row_data(index) {
            row.selected = selected;
            toggle_files.set_row_data(index, row);
        }
        if let Some(ui) = weak.upgrade() {
            update_selected_count(&ui, &toggle_files);
        }
    });

    let weak = ui.as_weak();
    ui.on_select_all(move |selected| {
        let updated_files: Vec<TelemetryFile> = (0..files.row_count())
            .filter_map(|index| {
                files.row_data(index).map(|mut row| {
                    row.selected = selected;
                    row
                })
            })
            .collect();
        files.set_vec(updated_files);
        if let Some(ui) = weak.upgrade() {
            update_selected_count(&ui, &files);
        }
    });
}

fn wire_conversion(ui: &MainWindow, files: Rc<VecModel<TelemetryFile>>) {
    let cancellation_requested = Arc::new(AtomicBool::new(false));

    let weak_stop = ui.as_weak();
    let stop_request = cancellation_requested.clone();
    ui.on_stop_conversion(move || {
        stop_request.store(true, Ordering::Relaxed);
        if let Some(ui) = weak_stop.upgrade() {
            ui.set_status_text("Stop requested… The current LD file will be completed.".into());
        }
    });

    let weak = ui.as_weak();
    ui.on_convert_selected(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let output_root = PathBuf::from(ui.get_output_folder().as_str());
        if output_root.as_os_str().is_empty() {
            ui.set_status_text("Choose an output folder.".into());
            return;
        }

        let selected: Vec<(usize, PathBuf)> = (0..files.row_count())
            .filter_map(|index| {
                files.row_data(index).and_then(|row| {
                    row.selected
                        .then(|| (index, PathBuf::from(row.path.as_str())))
                })
            })
            .collect();
        if selected.is_empty() {
            ui.set_status_text("Select at least one file.".into());
            return;
        }

        let export_mode = match ui.get_export_mode() {
            1 => ExportMode::Combined,
            2 => match u16::try_from(ui.get_requested_lap()) {
                Ok(number) if number > 0 => ExportMode::SingleLap(number),
                _ => {
                    ui.set_status_text("Choose a valid lap number.".into());
                    return;
                }
            },
            _ => ExportMode::PerLap,
        };

        ui.set_busy(true);
        ui.set_progress_value(0);
        ui.set_progress_max(i32::try_from(selected.len()).unwrap_or(i32::MAX));
        ui.set_status_text("Conversion in progress…".into());
        cancellation_requested.store(false, Ordering::Relaxed);

        let weak_worker = ui.as_weak();
        let worker_cancellation = cancellation_requested.clone();
        std::thread::spawn(move || {
            let total = selected.len();
            let mut processed = 0usize;
            let mut failed = 0usize;
            for (position, (model_index, input)) in selected.into_iter().enumerate() {
                if worker_cancellation.load(Ordering::Relaxed) {
                    break;
                }

                set_file_status(&weak_worker, model_index, "Converting…");
                let output = output_root.join(file_stem(&input));
                let input_name = file_name(&input);
                let weak_file_progress = weak_worker.clone();
                let result = convert_file_with_progress(
                    &input,
                    &output,
                    ConvertOptions {
                        export_mode,
                        ..ConvertOptions::default()
                    },
                    move |destination| {
                        let status = short_output_path(destination);
                        let weak_file_progress = weak_file_progress.clone();
                        let _ = weak_file_progress.upgrade_in_event_loop(move |ui| {
                            ui.set_status_text(format!("Creating {status}…").into());
                        });
                    },
                    || worker_cancellation.load(Ordering::Relaxed),
                );
                let status = match &result {
                    Ok(summary) if summary.cancelled => {
                        format!("Stopped · {} LD file(s)", summary.generated_count)
                    }
                    Ok(summary) => format!(
                        "Completed · {} lap(s) in {} LD file(s)",
                        summary.exported_lap_count, summary.generated_count
                    ),
                    Err(error) => format!("Error · {error:#}"),
                };
                set_file_status(&weak_worker, model_index, &status);
                let conversion_cancelled = result.as_ref().is_ok_and(|summary| summary.cancelled);
                if result.is_err() {
                    failed += 1;
                }
                if !conversion_cancelled {
                    processed = position + 1;
                }

                let weak_progress = weak_worker.clone();
                let progress = i32::try_from(processed).unwrap_or(i32::MAX);
                let result_text = result
                    .map(|summary| {
                        format!(
                            "{input_name} converted: {} LD file(s)",
                            summary.generated_count
                        )
                    })
                    .unwrap_or_else(|error| format!("Failed to convert {input_name}: {error:#}"));
                let _ = weak_progress.upgrade_in_event_loop(move |ui| {
                    ui.set_progress_value(progress);
                    ui.set_status_text(result_text.into());
                });

                if conversion_cancelled {
                    break;
                }
            }

            let cancelled = worker_cancellation.load(Ordering::Relaxed);
            let weak_done = weak_worker.clone();
            let _ = weak_done.upgrade_in_event_loop(move |ui| {
                ui.set_busy(false);
                if cancelled {
                    ui.set_status_text(
                        format!("Conversion stopped after {processed} of {total} file(s).").into(),
                    );
                } else if failed > 0 {
                    ui.set_status_text(
                        format!("Conversion processed {processed} file(s) with {failed} error(s).")
                            .into(),
                    );
                } else {
                    ui.set_status_text(format!("Conversion completed for {total} file(s).").into());
                }
            });
        });
    });
}

fn refresh_files(ui: &MainWindow, model: &Rc<VecModel<TelemetryFile>>, folder: &Path) {
    let previously_selected: HashSet<String> = (0..model.row_count())
        .filter_map(|index| model.row_data(index))
        .filter(|row| row.selected)
        .map(|row| row.path.to_string())
        .collect();

    let result = scan_duckdb_files(folder);
    model.set_vec(Vec::new());
    match result {
        Ok(paths) => {
            for path in paths {
                let path_text = path.display().to_string();
                model.push(TelemetryFile {
                    name: display_file_name(&path).into(),
                    path: path_text.clone().into(),
                    selected: previously_selected.contains(&path_text),
                    status: "Ready".into(),
                    completed: false,
                    failed: false,
                });
            }
            ui.set_status_text(format!("{} DuckDB file(s) found.", model.row_count()).into());
        }
        Err(error) => {
            ui.set_status_text(format!("Unable to read folder: {error:#}").into());
        }
    }
    update_selected_count(ui, model);
}

fn scan_duckdb_files(folder: &Path) -> Result<Vec<PathBuf>> {
    if !folder.is_dir() {
        anyhow::bail!("the path is not a folder");
    }
    let mut paths = std::fs::read_dir(folder)
        .with_context(|| format!("failed to read {}", folder.display()))?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("duckdb"))
        })
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| {
        file_creation_time(right)
            .cmp(&file_creation_time(left))
            .then_with(|| {
                left.file_name()
                    .map(|name| name.to_string_lossy().to_lowercase())
                    .cmp(
                        &right
                            .file_name()
                            .map(|name| name.to_string_lossy().to_lowercase()),
                    )
            })
    });
    Ok(paths)
}

fn file_creation_time(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.created().ok().or_else(|| metadata.modified().ok()))
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn update_selected_count(ui: &MainWindow, model: &VecModel<TelemetryFile>) {
    let count = (0..model.row_count())
        .filter_map(|index| model.row_data(index))
        .filter(|row| row.selected)
        .count();
    ui.set_selected_count(i32::try_from(count).unwrap_or(i32::MAX));
}

fn set_file_status(weak: &slint::Weak<MainWindow>, index: usize, status: &str) {
    let weak = weak.clone();
    let status = SharedString::from(status);
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let model = ui.get_files();
        if let Some(mut row) = model.row_data(index) {
            row.status = status;
            row.completed = row.status.starts_with("Completed");
            row.failed = row.status.starts_with("Error");
            model.set_row_data(index, row);
        }
    });
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("telemetry")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn display_file_name(path: &Path) -> String {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name(path));
    let bytes = name.as_bytes();

    for index in 0..bytes.len() {
        if let Some(timestamp) = bytes
            .get(index..index + 20)
            .and_then(|candidate| std::str::from_utf8(candidate).ok())
            .and_then(parse_filename_timestamp)
        {
            return format!("{}{}{}", &name[..index], timestamp, &name[index + 20..]);
        }
    }

    name
}

fn parse_filename_timestamp(value: &str) -> Option<String> {
    let date = NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()?;
    if value.get(10..11)? != "T" || value.get(19..20)? != "Z" {
        return None;
    }
    let time = NaiveTime::parse_from_str(value.get(11..19)?, "%H_%M_%S").ok()?;
    Some(format!(
        "{} {}",
        date.format("%Y-%m-%d"),
        time.format("%H:%M:%S")
    ))
}

fn short_output_path(path: &Path) -> String {
    let file = file_name(path);
    path.parent()
        .and_then(Path::file_name)
        .map(|folder| format!("{} / {file}", folder.to_string_lossy()))
        .unwrap_or(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_preserves_folders() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lmu2motec_config.toml");
        let expected = AppSettings {
            source_folder: PathBuf::from(r"C:\Telemetry LMU"),
            output_folder: PathBuf::from(r"D:\Exports MoTeC"),
            export_mode: 2,
            requested_lap: 7,
        };

        save_settings(&path, &expected).unwrap();
        let loaded = load_settings(&path).unwrap();

        assert_eq!(loaded, expected);
        let contents = std::fs::read_to_string(path).unwrap();
        assert!(contents.contains("source_folder"));
        assert!(contents.contains("output_folder"));
    }

    #[test]
    fn legacy_settings_are_migrated_to_toml() {
        let directory = tempfile::tempdir().unwrap();
        let toml_path = directory.path().join("lmu2motec_config.toml");
        let legacy_path = directory.path().join("settings.txt");
        std::fs::write(legacy_path, "C:\\Telemetry LMU\nD:\\Exports MoTeC\n").unwrap();

        let loaded = load_settings(&toml_path).unwrap();

        assert_eq!(loaded.source_folder, PathBuf::from(r"C:\Telemetry LMU"));
        assert_eq!(loaded.output_folder, PathBuf::from(r"D:\Exports MoTeC"));
        assert_eq!(loaded.export_mode, 0);
        assert_eq!(loaded.requested_lap, 1);
        assert!(toml_path.is_file());
    }

    #[test]
    fn older_toml_settings_receive_export_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lmu2motec_config.toml");
        std::fs::write(
            &path,
            "source_folder = 'C:\\Telemetry'\noutput_folder = 'C:\\Exports'\n",
        )
        .unwrap();

        let loaded = load_settings(&path).unwrap();

        assert_eq!(loaded.export_mode, 0);
        assert_eq!(loaded.requested_lap, 1);
    }

    #[test]
    fn settings_are_stored_in_the_launch_directory() {
        let current = Path::new("launch-directory");

        assert_eq!(
            settings_path(current),
            current.join("lmu2motec_config.toml")
        );
    }

    #[test]
    fn output_status_uses_only_folder_and_file_names() {
        let path = Path::new("Exports")
            .join("session_123")
            .join("Le_Mans_lap_4.ld");

        assert_eq!(short_output_path(&path), "session_123 / Le_Mans_lap_4.ld");
    }

    #[test]
    fn display_file_name_formats_lmu_timestamp() {
        let path = Path::new("telemetry_2026-04-03T20_06_03Z.duckdb");

        assert_eq!(display_file_name(path), "telemetry_2026-04-03 20:06:03");
    }

    #[test]
    fn display_file_name_formats_iso_date() {
        let path = Path::new("telemetry_2026-04-03.duckdb");

        assert_eq!(display_file_name(path), "telemetry_2026-04-03");
    }

    #[test]
    fn output_folder_is_resolved_to_an_absolute_path() {
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().join("nested").join("output");

        let resolved = resolve_folder(&folder).unwrap();

        assert!(resolved.is_absolute());
        assert!(resolved.is_dir());
        assert_eq!(
            resolved,
            platform_folder_path(folder.canonicalize().unwrap())
        );
    }
}
