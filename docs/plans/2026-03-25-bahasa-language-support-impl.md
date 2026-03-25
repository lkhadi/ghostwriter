# Bahasa Language Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add multilingual speech recognition support with dynamic language switching between English and Bahasa Indonesia.

**Architecture:** Add `language` field to AppConfig, modify `transcriber.transcribe()` to accept language parameter, add get/set language commands, update UI with language dropdown, and replace English-only model with multilingual model.

**Tech Stack:** Tauri 2, Rust, Whisper, Vue 3

---

## Task 1: Update config.rs - Add language field

**Files:**
- Modify: `src-tauri/src/config.rs`

**Step 1: Modify AppConfig struct**

Add `language` field to the `AppConfig` struct:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub hotkey: String,
    pub auto_mute_enabled: bool,
    pub language: String,  // NEW: "en" or "id"
}
```

**Step 2: Update Default implementation**

```rust
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: "Cmd+Option+Space".to_string(),
            auto_mute_enabled: true,
            language: "en".to_string(),  // NEW
        }
    }
}
```

**Step 3: Update OldConfig migration**

```rust
struct OldConfig {
    hotkey: String,
    // No language field in old config
}
```

And in the migration block, also preserve language:

```rust
let new_config = AppConfig {
    hotkey: old_config.hotkey,
    auto_mute_enabled: true,
    language: "en".to_string(), // Default for migrated users
};
```

**Step 4: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "feat: add language field to AppConfig"
```

---

## Task 2: Update transcriber.rs - Add language parameter

**Files:**
- Modify: `src-tauri/src/transcriber.rs:23`

**Step 1: Change method signature**

Change line 23 from:
```rust
pub fn transcribe(&self, audio_data: &[f32]) -> Result<String, Box<dyn Error + Send + Sync>> {
```

To:
```rust
pub fn transcribe(&self, audio_data: &[f32], language: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
```

**Step 2: Update set_language call**

Change line 32 from:
```rust
params.set_language(Some("en"));
```

To:
```rust
params.set_language(Some(language));
```

**Step 3: Commit**

```bash
git add src-tauri/src/transcriber.rs
git commit -m "feat: add language parameter to transcribe method"
```

---

## Task 3: Update lib.rs - Add get/set language commands

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: Add get_language command** (after line 124)

```rust
#[tauri::command]
fn get_language(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;
    let config = store.get("config").ok_or("Config not found")?;
    let config: AppConfig = serde_json::from_value(config).map_err(|e| e.to_string())?;
    Ok(config.language)
}
```

**Step 2: Add set_language command** (after get_language)

```rust
#[tauri::command]
fn set_language(app: tauri::AppHandle, language: String) -> Result<(), String> {
    let store = app.store("config.json").map_err(|e| e.to_string())?;

    let existing_config: Option<AppConfig> = store
        .get("config")
        .and_then(|v| serde_json::from_value(v).ok());

    let config = AppConfig {
        hotkey: existing_config.map(|c| c.hotkey).unwrap_or_else(|| "Cmd+Option+Space".to_string()),
        auto_mute_enabled: existing_config.map(|c| c.auto_mute_enabled).unwrap_or(true),
        language: language.clone(),
    };
    store.set("config".to_string(), json!(config));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
```

**Step 3: Register new commands** (line 467-478)

Add `get_language` and `set_language` to the invoke_handler:

```rust
.invoke_handler(tauri::generate_handler![
    check_permissions,
    start_recording,
    stop_recording,
    save_test_audio,
    transcribe_test_audio,
    inject_test_text,
    get_hotkey,
    save_hotkey,
    get_auto_mute_enabled,
    set_auto_mute_enabled,
    get_language,      // NEW
    set_language        // NEW
])
```

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add get_language and set_language commands"
```

---

## Task 4: Update logic_helper.rs - Pass language to transcribe

**Files:**
- Modify: `src-tauri/src/logic_helper.rs`

**Step 1: Update stop_and_transcribe_logic function**

Add this import at the top (after line 4):
```rust
use crate::config::AppConfig;
```

Change the transcription block (lines 28-49) to:

```rust
// Transcribe
let text = {
    match state.transcriber.lock() {
        Ok(transcriber_guard) => {
            if let Some(transcriber) = transcriber_guard.as_ref() {
                // Get language from config
                let language = {
                    let store = app.store("config.json").ok();
                    store
                        .and_then(|s| s.get("config"))
                        .and_then(|c| serde_json::from_value::<AppConfig>(c).ok())
                        .map(|c| c.language)
                        .unwrap_or_else(|| "en".to_string())
                };

                match transcriber.transcribe(&audio, &language) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Transcription error: {}", e);
                        return;
                    }
                }
            } else {
                eprintln!("Transcriber not initialized");
                return;
            }
        }
        Err(e) => {
            eprintln!("Transcriber lock poisoned: {}", e);
            return;
        }
    }
};
```

**Step 2: Commit**

```bash
git add src-tauri/src/logic_helper.rs
git commit -m "feat: pass language parameter to transcribe"
```

---

## Task 5: Update lib.rs - Fix transcribe_test_audio to use language

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: Update transcribe_test_audio command** (lines 171-204)

Change to:

```rust
#[tauri::command]
fn transcribe_test_audio(app: tauri::AppHandle) -> Result<String, String> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("models")
        .join("ggml-base.en.bin");  // Model filename stays the same

    if !resource_path.exists() {
        return Err(format!("Model not found at {:?}", resource_path));
    }

    let transcriber =
        Transcriber::new(resource_path.to_str().unwrap()).map_err(|e| e.to_string())?;

    // Read test_audio.wav from Desktop
    let desktop_path = dirs::desktop_dir().ok_or("Could not find desktop")?;
    let file_path = desktop_path.join("test_audio.wav");

    if !file_path.exists() {
        return Err(
            "test_audio.wav not found on Desktop. Please record and save first.".to_string(),
        );
    }

    let mut reader = hound::WavReader::open(&file_path).map_err(|e| e.to_string())?;
    let samples: Vec<f32> = reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect();

    // Get language from config
    let language = {
        let store = app.store("config.json").ok();
        store
            .and_then(|s| s.get("config"))
            .and_then(|c| serde_json::from_value::<AppConfig>(c).ok())
            .map(|c| c.language)
            .unwrap_or_else(|| "en".to_string())
    };

    let text = transcriber
        .transcribe(&samples, &language)
        .map_err(|e| e.to_string())?;
    Ok(text)
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: use language from config in transcribe_test_audio"
```

---

## Task 6: Update App.vue - Add language selector UI

**Files:**
- Modify: `src/App.vue`

**Step 1: Add reactive state** (after line 13)

```js
const selectedLanguage = ref("en");
```

**Step 2: Add loadLanguage function** (after loadAutoMute, around line 60)

```js
async function loadLanguage() {
  try {
    selectedLanguage.value = await invoke("get_language");
    log("Loaded language: " + selectedLanguage.value);
  } catch (e) {
    log("Error loading language: " + e);
  }
}
```

**Step 3: Add saveLanguage function** (after loadLanguage)

```js
async function saveLanguage() {
  try {
    await invoke("set_language", { lang: selectedLanguage.value });
    log("Saved language: " + selectedLanguage.value);
  } catch (e) {
    log("Error saving language: " + e);
  }
}
```

**Step 4: Call loadLanguage in onMounted** (after line 31)

Change:
```js
if (currentLabel.value === 'main') {
    loadHotkey();
    loadAutoMute();
}
```

To:
```js
if (currentLabel.value === 'main') {
    loadHotkey();
    loadAutoMute();
    loadLanguage();
}
```

**Step 5: Add language selector in template** (after hotkey section, around line 139)

```vue
<div class="section">
  <label>Recognition Language</label>
  <select v-model="selectedLanguage" @change="saveLanguage" class="language-select">
    <option value="en">English</option>
    <option value="id">Bahasa Indonesia</option>
  </select>
  <p class="hint">Language for speech recognition</p>
</div>
```

**Step 6: Add CSS for select** (in the `<style scoped>` section)

```css
.language-select {
  width: 100%;
  padding: 10px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  color: #fff;
  font-size: 14px;
  cursor: pointer;
  outline: none;
}

.language-select:focus {
  border-color: rgba(255, 255, 255, 0.3);
}

.language-select option {
  background: #1a1a1a;
  color: #fff;
}
```

**Step 7: Commit**

```bash
git add src/App.vue
git commit -m "feat: add language selector dropdown in settings UI"
```

---

## Task 7: Download multilingual model

**Files:**
- Modify: `src-tauri/models/ggml-base.en.bin`

**Step 1: Download multilingual model**

Run:
```bash
cd /Users/lkhadi/docker/htdocs/systemTest/ghostwriter/src-tauri/models
mv ggml-base.en.bin ggml-base.en.bin.bak
curl -L -o ggml-base.bin "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
```

Note: If the exact URL doesn't work, search for the whisper.cpp model at:
https://huggingface.co/ggerganov/whisper.cpp

**Step 2: Verify model works**

The model filename in code still references `ggml-base.en.bin` in some places - but we won't rename it since the multilingual model is still named `ggml-base.bin`. Actually, wait - let me check the code to see what filename it expects.

Looking at lib.rs line 177 and 425, the model path is hardcoded as `ggml-base.en.bin`. We need to either:
- Rename the new model to `ggml-base.en.bin` (misleading name)
- Or update the code to use `ggml-base.bin`

**Decision:** Rename new model to `ggml-base.en.bin` for backward compatibility - this way existing users don't need to change config paths and the multilingual model will work transparently.

Actually, looking again - the multilingual base model is `ggml-base.bin` on huggingface. The English-only model is `ggml-base.en.bin`. So:
- Download `ggml-base.bin` (multilingual)
- Rename to `ggml-base.en.bin` to match existing code paths

```bash
mv ggml-base.bin ggml-base.en.bin
rm ggml-base.en.bin.bak
```

**Step 3: Commit**

```bash
git add src-tauri/models/
git commit -m "feat: replace English-only model with multilingual model"
```

---

## Task 8: Build and test

**Step 1: Build the app**

```bash
cd /Users/lkhadi/docker/htdocs/systemTest/ghostwriter
npm run tauri build 2>&1 | head -100
```

Expected: Successful build with no errors

**Step 2: Test language switching**

1. Open GhostWriter settings
2. Verify language dropdown appears
3. Select "Bahasa Indonesia"
4. Close and reopen app
5. Verify "Bahasa Indonesia" is still selected
6. Test recording to verify transcription works

**Step 3: Commit**

```bash
git add -A
git commit -m "feat: complete Bahasa language support implementation

- Add language field to AppConfig with get/set commands
- Pass language parameter to Whisper transcriber
- Add language selector UI in Vue settings
- Replace English-only model with multilingual model"
```

---

## Summary of Changes

| File | Change Type |
|------|-------------|
| `src-tauri/src/config.rs` | Add language field |
| `src-tauri/src/transcriber.rs` | Add language param to transcribe() |
| `src-tauri/src/lib.rs` | Add get/set language commands |
| `src-tauri/src/logic_helper.rs` | Pass language to transcribe |
| `src/App.vue` | Add language selector UI |
| `src-tauri/models/` | Replace with multilingual model |

## Estimated Time

8 tasks × 5-10 min each = ~1 hour implementation + testing
