use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::persistence::{atomic_write, data_dir};
use crate::types::{
    PolishMode, StylePack, StylePackCatalogSnapshot, StylePackDraft, StylePackExample,
    StylePackKind, UserPreferences,
};

const STYLE_PACKS_FILE: &str = "style-packs.json";
const CATALOG_VERSION: u32 = 1;
pub const BUILTIN_RAW_ID: &str = "builtin.raw";
pub const BUILTIN_LIGHT_ID: &str = "builtin.light";
pub const BUILTIN_STRUCTURED_ID: &str = "builtin.structured";
pub const BUILTIN_FORMAL_ID: &str = "builtin.formal";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogFile {
    version: u32,
    active_style_id: String,
    enabled_style_ids: Vec<String>,
    custom_packs: Vec<StylePack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PortableStylePack {
    format_version: u32,
    name: String,
    description: String,
    base_mode: PolishMode,
    dictation_prompt: String,
    selection_prompt: String,
    examples: Vec<StylePackExample>,
}

pub struct StylePackStore {
    path: PathBuf,
    state: Mutex<CatalogFile>,
    cleanup_dir: Option<PathBuf>,
}

impl StylePackStore {
    pub fn new(legacy: &UserPreferences) -> Result<Self> {
        Self::open(data_dir()?.join(STYLE_PACKS_FILE), legacy)
    }

    pub fn new_for_coordinator(legacy: &UserPreferences) -> Result<Self> {
        #[cfg(test)]
        {
            let dir = test_scratch_dir("coordinator");
            let mut store = Self::open(dir.join(STYLE_PACKS_FILE), legacy)?;
            store.cleanup_dir = Some(dir);
            return Ok(store);
        }
        #[cfg(not(test))]
        {
            Self::new(legacy)
        }
    }

    #[cfg(test)]
    fn with_dir_for_tests(dir: PathBuf, legacy: &UserPreferences) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Self::open(dir.join(STYLE_PACKS_FILE), legacy)
    }

    fn open(path: PathBuf, legacy: &UserPreferences) -> Result<Self> {
        let state = if path.exists() {
            let decoded: CatalogFile = serde_json::from_slice(&std::fs::read(&path)?)?;
            validate_catalog(&decoded)?;
            decoded
        } else {
            let migrated = migrate_legacy(legacy);
            write_catalog(&path, &migrated)?;
            migrated
        };
        Ok(Self {
            path,
            state: Mutex::new(state),
            cleanup_dir: None,
        })
    }

    pub fn snapshot(&self) -> StylePackCatalogSnapshot {
        let state = self.state.lock();
        let mut packs = builtin_packs();
        packs.extend(state.custom_packs.clone());
        StylePackCatalogSnapshot {
            packs,
            active_style_id: state.active_style_id.clone(),
            enabled_style_ids: state.enabled_style_ids.clone(),
        }
    }

    pub fn active(&self) -> StylePack {
        let snapshot = self.snapshot();
        snapshot
            .packs
            .into_iter()
            .find(|pack| pack.id == snapshot.active_style_id)
            .unwrap_or_else(builtin_light)
    }

    pub fn builtin_id_for_mode(mode: PolishMode) -> &'static str {
        mode_id(mode)
    }

    pub fn create(&self, draft: StylePackDraft) -> Result<StylePack> {
        validate_draft(&draft)?;
        let now = Utc::now().to_rfc3339();
        let pack = StylePack {
            id: format!("custom.{}", Uuid::new_v4()),
            name: draft.name.trim().to_string(),
            description: draft.description.trim().to_string(),
            kind: StylePackKind::Custom,
            base_mode: draft.base_mode,
            dictation_prompt: draft.dictation_prompt.trim().to_string(),
            selection_prompt: draft.selection_prompt.trim().to_string(),
            examples: draft.examples,
            created_at: now.clone(),
            updated_at: now,
        };
        self.mutate(|catalog| {
            ensure_unique_name(catalog, &pack.name, None)?;
            catalog.custom_packs.push(pack.clone());
            catalog.enabled_style_ids.push(pack.id.clone());
            Ok(())
        })?;
        Ok(pack)
    }

    pub fn update(&self, id: &str, draft: StylePackDraft) -> Result<StylePack> {
        validate_draft(&draft)?;
        let mut updated = None;
        self.mutate(|catalog| {
            ensure_unique_name(catalog, draft.name.trim(), Some(id))?;
            let pack = catalog
                .custom_packs
                .iter_mut()
                .find(|pack| pack.id == id)
                .ok_or_else(|| anyhow!("stylePackReadonlyOrNotFound"))?;
            pack.name = draft.name.trim().to_string();
            pack.description = draft.description.trim().to_string();
            pack.base_mode = draft.base_mode;
            pack.dictation_prompt = draft.dictation_prompt.trim().to_string();
            pack.selection_prompt = draft.selection_prompt.trim().to_string();
            pack.examples = draft.examples.clone();
            pack.updated_at = Utc::now().to_rfc3339();
            updated = Some(pack.clone());
            Ok(())
        })?;
        updated.ok_or_else(|| anyhow!("stylePackReadonlyOrNotFound"))
    }

    pub fn duplicate(&self, id: &str) -> Result<StylePack> {
        let source = self
            .snapshot()
            .packs
            .into_iter()
            .find(|pack| pack.id == id)
            .ok_or_else(|| anyhow!("stylePackNotFound"))?;
        let catalog = self.state.lock().clone();
        let mut suffix = 1;
        let name = loop {
            let candidate = if suffix == 1 {
                format!("{} Copy", source.name)
            } else {
                format!("{} Copy {suffix}", source.name)
            };
            if !name_exists(&catalog, &candidate, None) {
                break candidate;
            }
            suffix += 1;
        };
        self.create(StylePackDraft {
            name,
            description: source.description,
            base_mode: source.base_mode,
            dictation_prompt: source.dictation_prompt,
            selection_prompt: source.selection_prompt,
            examples: source.examples,
        })
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        if is_builtin_id(id) {
            return Err(anyhow!("stylePackBuiltinReadonly"));
        }
        self.mutate(|catalog| {
            if !catalog.custom_packs.iter().any(|pack| pack.id == id) {
                return Err(anyhow!("stylePackNotFound"));
            }
            if enabled {
                if !catalog.enabled_style_ids.iter().any(|item| item == id) {
                    catalog.enabled_style_ids.push(id.to_string());
                }
            } else {
                catalog.enabled_style_ids.retain(|item| item != id);
                if catalog.active_style_id == id {
                    fallback_to_light(catalog);
                }
            }
            Ok(())
        })
    }

    pub fn activate(&self, id: &str) -> Result<()> {
        self.mutate(|catalog| {
            if !all_ids(catalog).iter().any(|item| item == id) {
                return Err(anyhow!("stylePackNotFound"));
            }
            if !catalog.enabled_style_ids.iter().any(|item| item == id) {
                return Err(anyhow!("stylePackDisabled"));
            }
            catalog.active_style_id = id.to_string();
            Ok(())
        })
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        if is_builtin_id(id) {
            return Err(anyhow!("stylePackBuiltinReadonly"));
        }
        self.mutate(|catalog| {
            let before = catalog.custom_packs.len();
            catalog.custom_packs.retain(|pack| pack.id != id);
            if before == catalog.custom_packs.len() {
                return Err(anyhow!("stylePackNotFound"));
            }
            catalog.enabled_style_ids.retain(|item| item != id);
            if catalog.active_style_id == id {
                fallback_to_light(catalog);
            }
            Ok(())
        })
    }

    pub fn cycle_previous(&self) -> Result<Option<String>> {
        let mut activated = None;
        self.mutate(|catalog| {
            let order = ordered_enabled_ids(catalog);
            if order.len() <= 1 {
                return Ok(());
            }
            let current = order
                .iter()
                .position(|id| id == &catalog.active_style_id)
                .unwrap_or(0);
            let previous = if current == 0 {
                order.len() - 1
            } else {
                current - 1
            };
            catalog.active_style_id = order[previous].clone();
            activated = Some(catalog.active_style_id.clone());
            Ok(())
        })?;
        Ok(activated)
    }

    pub fn export(&self, id: &str) -> Result<String> {
        let pack = self
            .snapshot()
            .packs
            .into_iter()
            .find(|pack| pack.id == id)
            .ok_or_else(|| anyhow!("stylePackNotFound"))?;
        serde_json::to_string_pretty(&PortableStylePack {
            format_version: CATALOG_VERSION,
            name: pack.name,
            description: pack.description,
            base_mode: pack.base_mode,
            dictation_prompt: pack.dictation_prompt,
            selection_prompt: pack.selection_prompt,
            examples: pack.examples,
        })
        .context("stylePackExportFailed")
    }

    pub fn import(&self, json: &str) -> Result<StylePack> {
        let portable: PortableStylePack =
            serde_json::from_str(json).map_err(|_| anyhow!("stylePackImportInvalid"))?;
        if portable.format_version != CATALOG_VERSION {
            return Err(anyhow!("stylePackImportVersion"));
        }
        self.create(StylePackDraft {
            name: portable.name,
            description: portable.description,
            base_mode: portable.base_mode,
            dictation_prompt: portable.dictation_prompt,
            selection_prompt: portable.selection_prompt,
            examples: portable.examples,
        })
    }

    fn mutate(&self, mutate: impl FnOnce(&mut CatalogFile) -> Result<()>) -> Result<()> {
        let mut state = self.state.lock();
        let mut candidate = state.clone();
        mutate(&mut candidate)?;
        normalize_catalog(&mut candidate);
        validate_catalog(&candidate)?;
        write_catalog(&self.path, &candidate)?;
        *state = candidate;
        Ok(())
    }
}

#[cfg(test)]
fn test_scratch_dir(scope: &str) -> PathBuf {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a repository parent");
    let dir = repository_root
        .join(".runtime")
        .join(".cache")
        .join("style-pack-tests")
        .join(format!("{scope}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create repository-local style-pack test scratch");
    let owner = serde_json::json!({
        "owner": "whisper-input style-pack tests",
        "sourceProject": repository_root.display().to_string(),
        "createdAt": Utc::now().to_rfc3339(),
        "ttlHours": 1,
        "cleanupCommand": format!(
            "Remove-Item -LiteralPath '{}' -Recurse -Force",
            dir.display()
        ),
    });
    std::fs::write(
        dir.join(".vibe-owner.json"),
        serde_json::to_vec_pretty(&owner).expect("serialize scratch ownership"),
    )
    .expect("write scratch ownership");
    dir
}

impl Drop for StylePackStore {
    fn drop(&mut self) {
        if let Some(dir) = self.cleanup_dir.take() {
            std::fs::remove_dir_all(dir).ok();
        }
    }
}

fn mode_id(mode: PolishMode) -> &'static str {
    match mode {
        PolishMode::Raw => BUILTIN_RAW_ID,
        PolishMode::Light => BUILTIN_LIGHT_ID,
        PolishMode::Structured => BUILTIN_STRUCTURED_ID,
        PolishMode::Formal => BUILTIN_FORMAL_ID,
    }
}

fn migrate_legacy(prefs: &UserPreferences) -> CatalogFile {
    let mut enabled_style_ids: Vec<String> = prefs
        .enabled_modes
        .iter()
        .map(|mode| mode_id(*mode).to_string())
        .collect();
    let active_style_id = mode_id(prefs.default_mode).to_string();
    if !enabled_style_ids.contains(&active_style_id) {
        enabled_style_ids.push(active_style_id.clone());
    }
    if enabled_style_ids.is_empty() {
        enabled_style_ids.push(BUILTIN_LIGHT_ID.into());
    }
    CatalogFile {
        version: CATALOG_VERSION,
        active_style_id,
        enabled_style_ids,
        custom_packs: Vec::new(),
    }
}

fn builtin_packs() -> Vec<StylePack> {
    vec![
        builtin(BUILTIN_RAW_ID, "原始转写", PolishMode::Raw),
        builtin(BUILTIN_LIGHT_ID, "轻度润色", PolishMode::Light),
        builtin(BUILTIN_STRUCTURED_ID, "清晰结构", PolishMode::Structured),
        builtin(BUILTIN_FORMAL_ID, "正式表达", PolishMode::Formal),
    ]
}

fn builtin_light() -> StylePack {
    builtin(BUILTIN_LIGHT_ID, "轻度润色", PolishMode::Light)
}

fn builtin(id: &str, name: &str, base_mode: PolishMode) -> StylePack {
    StylePack {
        id: id.into(),
        name: name.into(),
        description: String::new(),
        kind: StylePackKind::Builtin,
        base_mode,
        dictation_prompt: String::new(),
        selection_prompt: String::new(),
        examples: Vec::new(),
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn is_builtin_id(id: &str) -> bool {
    [
        BUILTIN_RAW_ID,
        BUILTIN_LIGHT_ID,
        BUILTIN_STRUCTURED_ID,
        BUILTIN_FORMAL_ID,
    ]
    .contains(&id)
}

fn all_ids(catalog: &CatalogFile) -> Vec<String> {
    builtin_packs()
        .into_iter()
        .map(|pack| pack.id)
        .chain(catalog.custom_packs.iter().map(|pack| pack.id.clone()))
        .collect()
}

fn ordered_enabled_ids(catalog: &CatalogFile) -> Vec<String> {
    let mut custom = catalog.custom_packs.clone();
    custom.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    builtin_packs()
        .into_iter()
        .chain(custom)
        .map(|pack| pack.id)
        .filter(|id| catalog.enabled_style_ids.contains(id))
        .collect()
}

fn fallback_to_light(catalog: &mut CatalogFile) {
    if !catalog
        .enabled_style_ids
        .iter()
        .any(|id| id == BUILTIN_LIGHT_ID)
    {
        catalog.enabled_style_ids.push(BUILTIN_LIGHT_ID.into());
    }
    catalog.active_style_id = BUILTIN_LIGHT_ID.into();
}

fn normalize_catalog(catalog: &mut CatalogFile) {
    catalog.enabled_style_ids.sort();
    catalog.enabled_style_ids.dedup();
}

fn validate_catalog(catalog: &CatalogFile) -> Result<()> {
    if catalog.version != CATALOG_VERSION {
        return Err(anyhow!("stylePackCatalogVersion"));
    }
    let ids = all_ids(catalog);
    if !ids.contains(&catalog.active_style_id)
        || !catalog.enabled_style_ids.contains(&catalog.active_style_id)
        || catalog.enabled_style_ids.iter().any(|id| !ids.contains(id))
    {
        return Err(anyhow!("stylePackCatalogInvalid"));
    }
    let mut custom_ids = HashSet::new();
    let mut names: HashSet<String> = builtin_packs()
        .into_iter()
        .map(|pack| pack.name.to_lowercase())
        .collect();
    for pack in &catalog.custom_packs {
        if pack.kind != StylePackKind::Custom || is_builtin_id(&pack.id) {
            return Err(anyhow!("stylePackCatalogInvalid"));
        }
        if !custom_ids.insert(pack.id.clone()) || !names.insert(pack.name.trim().to_lowercase()) {
            return Err(anyhow!("stylePackCatalogInvalid"));
        }
        validate_draft(&StylePackDraft {
            name: pack.name.clone(),
            description: pack.description.clone(),
            base_mode: pack.base_mode,
            dictation_prompt: pack.dictation_prompt.clone(),
            selection_prompt: pack.selection_prompt.clone(),
            examples: pack.examples.clone(),
        })?;
    }
    Ok(())
}

fn validate_draft(draft: &StylePackDraft) -> Result<()> {
    if draft.name.trim().is_empty() || draft.name.chars().count() > 80 {
        return Err(anyhow!("stylePackNameInvalid"));
    }
    if draft.description.chars().count() > 500
        || draft.dictation_prompt.chars().count() > 4000
        || draft.selection_prompt.chars().count() > 4000
        || draft.examples.len() > 20
    {
        return Err(anyhow!("stylePackContentInvalid"));
    }
    Ok(())
}

fn name_exists(catalog: &CatalogFile, name: &str, except_id: Option<&str>) -> bool {
    builtin_packs()
        .into_iter()
        .chain(catalog.custom_packs.clone())
        .any(|pack| {
            Some(pack.id.as_str()) != except_id && pack.name.eq_ignore_ascii_case(name.trim())
        })
}

fn ensure_unique_name(catalog: &CatalogFile, name: &str, except_id: Option<&str>) -> Result<()> {
    if name_exists(catalog, name, except_id) {
        Err(anyhow!("stylePackNameConflict"))
    } else {
        Ok(())
    }
}

fn write_catalog(path: &Path, catalog: &CatalogFile) -> Result<()> {
    atomic_write(path, &serde_json::to_vec_pretty(catalog)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_migration_is_idempotent_and_keeps_builtins_computed() {
        let dir = test_scratch_dir("migration");
        let mut prefs = UserPreferences::default();
        prefs.default_mode = PolishMode::Formal;
        prefs.enabled_modes = vec![PolishMode::Light, PolishMode::Formal];
        let first = StylePackStore::with_dir_for_tests(dir.clone(), &prefs).unwrap();
        let first_snapshot = first.snapshot();
        drop(first);
        let second = StylePackStore::with_dir_for_tests(dir.clone(), &prefs).unwrap();
        assert_eq!(first_snapshot, second.snapshot());
        assert_eq!(first_snapshot.active_style_id, BUILTIN_FORMAL_ID);
        assert!(second.state.lock().custom_packs.is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn deleting_active_custom_pack_falls_back_to_builtin_light() {
        let dir = test_scratch_dir("lifecycle");
        let store =
            StylePackStore::with_dir_for_tests(dir.clone(), &UserPreferences::default()).unwrap();
        let pack = store
            .create(StylePackDraft {
                name: "Work update".into(),
                description: String::new(),
                base_mode: PolishMode::Light,
                dictation_prompt: "Keep it concise".into(),
                selection_prompt: "Improve clarity".into(),
                examples: Vec::new(),
            })
            .unwrap();
        store.activate(&pack.id).unwrap();
        store.delete(&pack.id).unwrap();
        let snapshot = store.snapshot();
        assert_eq!(snapshot.active_style_id, BUILTIN_LIGHT_ID);
        assert!(snapshot
            .enabled_style_ids
            .iter()
            .any(|id| id == BUILTIN_LIGHT_ID));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn catalog_validation_rejects_duplicate_custom_ids_and_names() {
        let now = Utc::now().to_rfc3339();
        let pack = StylePack {
            id: "custom.same".into(),
            name: "Same".into(),
            description: String::new(),
            kind: StylePackKind::Custom,
            base_mode: PolishMode::Light,
            dictation_prompt: String::new(),
            selection_prompt: String::new(),
            examples: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        };
        let catalog = CatalogFile {
            version: CATALOG_VERSION,
            active_style_id: BUILTIN_LIGHT_ID.into(),
            enabled_style_ids: vec![BUILTIN_LIGHT_ID.into()],
            custom_packs: vec![pack.clone(), pack],
        };

        assert!(validate_catalog(&catalog).is_err());
    }

    #[test]
    fn invalid_import_keeps_catalog_bytes_unchanged() {
        let dir = test_scratch_dir("import");
        let store =
            StylePackStore::with_dir_for_tests(dir.clone(), &UserPreferences::default()).unwrap();
        let before = std::fs::read(&store.path).unwrap();

        assert_eq!(store.import(r#"{"formatVersion":1,"name":"Bad","description":"","baseMode":"light","dictationPrompt":"","selectionPrompt":"","examples":[],"unknown":true}"#).unwrap_err().to_string(), "stylePackImportInvalid");
        assert_eq!(std::fs::read(&store.path).unwrap(), before);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn duplicate_and_cycle_use_independent_ids_and_stable_order() {
        let dir = test_scratch_dir("cycle");
        let store =
            StylePackStore::with_dir_for_tests(dir.clone(), &UserPreferences::default()).unwrap();
        let first = store
            .create(StylePackDraft {
                name: "First custom".into(),
                description: String::new(),
                base_mode: PolishMode::Light,
                dictation_prompt: "First".into(),
                selection_prompt: String::new(),
                examples: Vec::new(),
            })
            .unwrap();
        let copy = store.duplicate(&first.id).unwrap();
        assert_ne!(first.id, copy.id);
        store.activate(&first.id).unwrap();
        assert_eq!(
            store.cycle_previous().unwrap(),
            Some(BUILTIN_FORMAL_ID.into())
        );
        store.activate(&copy.id).unwrap();
        assert_eq!(store.cycle_previous().unwrap(), Some(first.id));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn builtin_lifecycle_is_readonly_but_duplicate_is_allowed() {
        let dir = test_scratch_dir("builtin");
        let store =
            StylePackStore::with_dir_for_tests(dir.clone(), &UserPreferences::default()).unwrap();
        assert_eq!(
            store
                .set_enabled(BUILTIN_LIGHT_ID, false)
                .unwrap_err()
                .to_string(),
            "stylePackBuiltinReadonly"
        );
        assert_eq!(
            store.delete(BUILTIN_LIGHT_ID).unwrap_err().to_string(),
            "stylePackBuiltinReadonly"
        );
        let copy = store.duplicate(BUILTIN_LIGHT_ID).unwrap();
        assert_eq!(copy.kind, StylePackKind::Custom);
        std::fs::remove_dir_all(dir).ok();
    }
}
