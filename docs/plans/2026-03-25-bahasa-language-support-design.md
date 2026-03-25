# Design: Bahasa Indonesia Language Support for GhostWriter

## Overview

Add multilingual speech recognition support to GhostWriter, enabling dynamic language switching between English and Bahasa Indonesia from the settings UI.

## Context

GhostWriter currently uses `ggml-base.en.bin` (English-only base model) for Whisper transcription. The language is hardcoded to English (`params.set_language(Some("en"))` in `transcriber.rs`). Users need the ability to transcribe Bahasa Indonesia without losing English support.

## Goals

1. Enable Whisper transcription in Bahasa Indonesia
2. Allow users to switch between English and Bahasa dynamically from settings
3. Maintain backward compatibility with existing English users

## Architecture

### Components to Modify

| File | Change |
|------|--------|
| `src-tauri/src/config.rs` | Add `language` field with default "en" |
| `src-tauri/src/transcriber.rs` | Add language parameter to `transcribe()` method |
| `src-tauri/src/lib.rs` | Add `set_language` and `get_language` Tauri commands |
| `src/App.vue` | Add language selector dropdown in settings UI |
| `src-tauri/models/` | Download multilingual model `ggml-base.bin` |

### Data Flow

```
User selects "Bahasa" in dropdown
    ↓
invoke("set_language", { lang: "id" })
    ↓
Save to config store (config.rs)
    ↓
On next recording: invoke("start_recording")
    ↓
Transcriber reads language from config
    ↓
params.set_language(Some("id"))
    ↓
Whisper transcribes in Bahasa
```

## Detailed Changes

### 1. Model Download

Replace `ggml-base.en.bin` with `ggml-base.bin` (multilingual base model):
- Size: ~75MB (same as current)
- Supports 30+ languages including English (en) and Indonesian (id)
- Source: Whisper model repository

**Action:** Delete old model, download multilingual model to same location

### 2. config.rs Changes

```rust
#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub hotkey: String,
    pub auto_mute_enabled: bool,
    pub language: String,  // NEW: "en" or "id"
}
```

### 3. transcriber.rs Changes

Change method signature to accept language parameter:

```rust
pub fn transcribe(&self, audio_data: &[f32], language: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    // ...
    params.set_language(Some(language));
    // ...
}
```

Update all call sites in lib.rs to pass the language parameter.

### 4. lib.rs Changes

Add new commands:

```rust
#[tauri::command]
fn get_language(app: AppHandle) -> String {
    config::get_config(app).language
}

#[tauri::command]
fn set_language(app: AppHandle, language: String) -> Result<(), String> {
    let mut cfg = config::get_config(app);
    cfg.language = language;
    config::save_config(app, &cfg)
}
```

Update `start_recording` to read language from config and pass to transcriber.

### 5. App.vue Changes

Add language selector after the hotkey section:

```vue
<div class="section">
  <label>Recognition Language</label>
  <select v-model="selectedLanguage" @change="saveLanguage">
    <option value="en">English</option>
    <option value="id">Bahasa Indonesia</option>
  </select>
  <p class="hint">Language for speech recognition</p>
</div>
```

Add reactive state and handlers:

```js
const selectedLanguage = ref("en");

async function loadLanguage() {
  selectedLanguage.value = await invoke("get_language");
}

async function saveLanguage() {
  await invoke("set_language", { lang: selectedLanguage.value });
}
```

Call `loadLanguage()` in `onMounted()` when `currentLabel === 'main'`.

## Success Criteria

- [ ] User can select Bahasa Indonesia from dropdown
- [ ] Selection persists across app restarts
- [ ] Recording in Bahasa mode transcribes Indonesian speech
- [ ] English mode continues to work as before
- [ ] No breaking changes to existing functionality

## Out of Scope

- UI localization (translating app labels to Bahasa)
- Additional languages beyond English and Bahasa
- Model fine-tuning or custom models

## Risk & Mitigations

| Risk | Mitigation |
|------|------------|
| Multilingual model less accurate than English-only | Acceptable for v1; user can revert if needed |
| Bahasa model quality | ggml-base is baseline; medium model upgrade path exists |

## Timeline

Estimated: 1-2 hours for implementation + testing
