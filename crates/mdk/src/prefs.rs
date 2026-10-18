//! Preferences and the project list, kept in one `config.json` in the OS config folder.
//! Nothing is ever stored inside a project folder.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewChoice {
    #[default]
    Board,
    List,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorTab {
    #[default]
    Markdown,
    Preview,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedProject {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
}

/// A named set of filters, as kept in the config file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedView {
    pub name: String,
    pub query: String,
    /// Priority names: `critical`, `high`, `medium`, `low`.
    pub priorities: Vec<String>,
    pub assignee: String,
    /// Empty for every label.
    pub label: String,
    /// `all`, `overdue`, `today`, `week` or `none`.
    pub due: String,
}

impl SavedView {
    pub fn new(name: &str, filters: &crate::board::Filters) -> SavedView {
        SavedView {
            name: name.trim().to_string(),
            query: filters.query.clone(),
            priorities: filters
                .priorities
                .iter()
                .map(|p| p.as_str().to_string())
                .collect(),
            assignee: filters.assignee.clone(),
            label: filters.label.clone().unwrap_or_default(),
            due: filters.due.as_str().to_string(),
        }
    }

    pub fn filters(&self) -> crate::board::Filters {
        crate::board::Filters {
            query: self.query.clone(),
            priorities: self
                .priorities
                .iter()
                .filter_map(|p| mdk_format::Priority::parse(p))
                .collect(),
            assignee: self.assignee.clone(),
            label: Some(self.label.clone()).filter(|l| !l.is_empty()),
            due: crate::board::DueFilter::parse(&self.due),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub maximized: bool,
}

pub const SIDEBAR_WIDTH: f32 = 240.;
pub const EDITOR_WIDTH: f32 = 672.;

/// `"all"` in the file means no project or work folder is selected.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub version: u32,
    /// In sidebar order.
    pub projects: Vec<SavedProject>,
    pub selected_project: String,
    pub selected_workspace: String,
    pub auto_refresh: bool,
    pub vim_keys: bool,
    pub theme: ThemeChoice,
    pub view: ViewChoice,
    pub editor_tab: EditorTab,
    pub editor_expanded: bool,
    /// Dragged widths, in pixels.
    pub sidebar_width: f32,
    pub editor_width: f32,
    /// Saved filter views per project id, or `"all"` for All projects.
    pub saved_views: BTreeMap<String, Vec<SavedView>>,
    pub window: Option<WindowState>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            version: 1,
            projects: Vec::new(),
            selected_project: "all".into(),
            selected_workspace: "all".into(),
            auto_refresh: true,
            vim_keys: false,
            theme: ThemeChoice::Dark,
            view: ViewChoice::Board,
            editor_tab: EditorTab::Markdown,
            editor_expanded: false,
            sidebar_width: SIDEBAR_WIDTH,
            editor_width: EDITOR_WIDTH,
            saved_views: BTreeMap::new(),
            window: None,
        }
    }
}

/// The folder for `config.json`: `MDK_CONFIG_DIR`, or the OS config folder.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MDK_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    directories::ProjectDirs::from("", "", "mdk")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".mdk"))
}

impl Prefs {
    /// Read the config. A missing file gives the defaults. A file that will not parse is
    /// set aside as `config.broken-<time>.json` and the defaults are used; the second value
    /// is then the message to show.
    pub fn load(dir: &Path) -> (Prefs, Option<String>) {
        let path = dir.join("config.json");
        let Ok(text) = fs::read_to_string(&path) else {
            return (Prefs::default(), None);
        };
        match serde_json::from_str(&text) {
            Ok(prefs) => (prefs, None),
            Err(error) => {
                let stamp = mdk_format::now_iso().replace(':', "-");
                let aside = dir.join(format!("config.broken-{stamp}.json"));
                let _ = fs::rename(&path, &aside);
                let message = format!("Settings could not be read ({error}). Starting fresh.");
                (Prefs::default(), Some(message))
            }
        }
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let text = serde_json::to_string_pretty(self).expect("prefs serialise") + "\n";
        mdk_store::atomic_write(&dir.join("config.json"), text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::{Prefs, SavedProject, ThemeChoice, ViewChoice, WindowState};
    use std::fs;

    #[test]
    fn prefs_survive_a_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = Prefs {
            projects: vec![
                SavedProject {
                    id: "a1b2c3d4".into(),
                    name: "tasks".into(),
                    root: "/code/app/tasks".into(),
                },
                SavedProject {
                    id: "z9y8x7w6".into(),
                    name: "clinicos".into(),
                    root: "/code/clinicos".into(),
                },
            ],
            selected_project: "z9y8x7w6".into(),
            selected_workspace: "backend".into(),
            auto_refresh: false,
            vim_keys: true,
            theme: ThemeChoice::System,
            view: ViewChoice::List,
            window: Some(WindowState {
                x: 10.,
                y: 20.,
                w: 1200.,
                h: 800.,
                maximized: true,
            }),
            ..Prefs::default()
        };
        prefs.save(dir.path()).unwrap();
        assert_eq!(Prefs::load(dir.path()), (prefs, None));
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["config.json"]);
    }

    #[test]
    fn a_missing_config_gives_defaults_and_old_files_gain_new_fields() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Prefs::load(dir.path()), (Prefs::default(), None));
        fs::write(
            dir.path().join("config.json"),
            r#"{"version":1,"vim_keys":true}"#,
        )
        .unwrap();
        let (prefs, message) = Prefs::load(dir.path());
        assert!(prefs.vim_keys && prefs.auto_refresh && message.is_none());
        assert_eq!(prefs.theme, ThemeChoice::Dark);
    }

    #[test]
    fn a_broken_config_is_set_aside_not_lost() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.json"), "{ not json").unwrap();
        let (prefs, message) = Prefs::load(dir.path());
        assert_eq!(prefs, Prefs::default());
        assert!(message.unwrap().starts_with("Settings could not be read"));
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.len(), 1);
        assert!(names[0].starts_with("config.broken-"), "{names:?}");
        assert_eq!(
            fs::read_to_string(dir.path().join(&names[0])).unwrap(),
            "{ not json"
        );
    }
}
