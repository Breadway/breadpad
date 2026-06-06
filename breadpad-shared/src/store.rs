use crate::calendar::{caldav_uid, CalDavClient};
use crate::config::CalendarConfig;
use crate::types::Note;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Store {
    notes_path: PathBuf,
    archive_path: PathBuf,
    calendar: Option<CalendarConfig>,
}

impl Store {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_local_dir()
            .context("no XDG data dir")?
            .join("breadpad");
        Self::from_dir(&data_dir)
    }

    pub fn from_dir(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Store {
            notes_path: dir.join("notes.jsonl"),
            archive_path: dir.join("archive.jsonl"),
            calendar: None,
        })
    }

    pub fn with_calendar(mut self, cfg: CalendarConfig) -> Self {
        self.calendar = Some(cfg);
        self
    }

    pub fn with_calendar_if_enabled(self, cfg: &crate::config::Config) -> Self {
        if cfg.calendar.enabled {
            self.with_calendar(cfg.calendar.clone())
        } else {
            self
        }
    }

    pub fn load_all(&self) -> Result<Vec<Note>> {
        self.load_from(&self.notes_path)
    }

    pub fn load_archive(&self) -> Result<Vec<Note>> {
        self.load_from(&self.archive_path)
    }

    fn load_from(&self, path: &Path) -> Result<Vec<Note>> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut notes = Vec::new();
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<Note>(trimmed) {
                Ok(note) => notes.push(note),
                Err(e) => tracing::warn!("skipping malformed note at line {}: {}", i + 1, e),
            }
        }
        Ok(notes)
    }

    pub fn save_note(&self, note: &Note) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.notes_path)?;
        let line = serde_json::to_string(note)?;
        writeln!(file, "{}", line)?;

        if let Some(cal_cfg) = &self.calendar {
            if cal_cfg.enabled && (note.time.is_some() || note.rrule.is_some()) {
                spawn_caldav_push(note.clone(), cal_cfg.clone());
            }
        }

        Ok(())
    }

    pub fn update_note(&self, updated: &Note) -> Result<()> {
        self.rewrite_notes(|note| {
            if note.id == updated.id { updated.clone() } else { note }
        })?;
        if let Some(cal_cfg) = &self.calendar {
            if cal_cfg.enabled && (updated.time.is_some() || updated.rrule.is_some()) {
                spawn_caldav_push(updated.clone(), cal_cfg.clone());
            }
        }
        Ok(())
    }

    pub fn delete_note(&self, id: &str) -> Result<()> {
        let all = self.load_all()?;
        let (to_delete, keep): (Vec<Note>, Vec<Note>) = all.into_iter().partition(|n| n.id == id);
        self.write_all(&self.notes_path, &keep)?;

        if let Some(cal_cfg) = &self.calendar {
            if cal_cfg.enabled {
                if let Some(note) = to_delete.into_iter().next() {
                    spawn_caldav_delete(caldav_uid(&note), cal_cfg.clone());
                }
            }
        }

        Ok(())
    }

    fn rewrite_notes<F>(&self, mut f: F) -> Result<()>
    where
        F: FnMut(Note) -> Note,
    {
        let notes: Vec<Note> = self.load_all()?.into_iter().map(|n| f(n)).collect();
        self.write_all(&self.notes_path, &notes)
    }

    fn write_all(&self, path: &Path, notes: &[Note]) -> Result<()> {
        let tmp_path = path.with_extension("tmp");
        {
            let mut file = fs::File::create(&tmp_path)?;
            for note in notes {
                let line = serde_json::to_string(note)?;
                writeln!(file, "{}", line)?;
            }
            file.flush()?;
        }
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn rotate_archive(&self, archive_after_days: i64) -> Result<usize> {
        let cutoff = Utc::now() - Duration::days(archive_after_days);
        let notes = self.load_all()?;
        let (to_archive, keep): (Vec<Note>, Vec<Note>) = notes
            .into_iter()
            .partition(|n| n.done && n.completed.map_or(false, |c| c < cutoff));

        if to_archive.is_empty() {
            return Ok(0);
        }

        let count = to_archive.len();
        let mut archive_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.archive_path)?;
        for note in &to_archive {
            writeln!(archive_file, "{}", serde_json::to_string(note)?)?;
        }

        self.write_all(&self.notes_path, &keep)?;
        Ok(count)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Note>> {
        Ok(self.load_all()?.into_iter().find(|n| n.id == id))
    }
}

fn spawn_caldav_push(note: Note, cfg: CalendarConfig) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::warn!("CalDAV: failed to create runtime: {}", e);
                return;
            }
        };
        rt.block_on(async {
            let client = CalDavClient::new(cfg);
            match client.push_event(&note).await {
                Ok(uid) => tracing::info!("CalDAV: pushed note {} as {}", note.id, uid),
                Err(e) => tracing::warn!("CalDAV: push failed for note {}: {}", note.id, e),
            }
        });
    });
}

fn spawn_caldav_delete(uid: String, cfg: CalendarConfig) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::warn!("CalDAV: failed to create runtime: {}", e);
                return;
            }
        };
        rt.block_on(async {
            let client = CalDavClient::new(cfg);
            match client.delete_event(&uid).await {
                Ok(()) => tracing::info!("CalDAV: deleted event {}", uid),
                Err(e) => tracing::warn!("CalDAV: delete failed for {}: {}", uid, e),
            }
        });
    });
}
