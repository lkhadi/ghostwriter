<script setup>
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import HotkeyRecorder from "./components/HotkeyRecorder.vue";
import Hud from "./components/Hud.vue";

const currentLabel = ref("");
const isRecording = ref(false);
const statusMsg = ref("Ready");
const hotkey = ref("");
const showDebug = ref(false);
const logs = ref([]);
const autoMuteEnabled = ref(true);
const selectedLanguage = ref("en");

// Add a log message
const log = (msg) => {
  logs.value.unshift(`[${new Date().toLocaleTimeString()}] ${msg}`);
  if (logs.value.length > 20) logs.value.pop();
  statusMsg.value = msg;
};

onMounted(async () => {
    // Determine which window this is
    const win = getCurrentWebviewWindow();
    currentLabel.value = win.label;

    if (currentLabel.value === 'main') {
        loadHotkey();
        loadAutoMute();
        loadLanguage();
    }
});

async function loadHotkey() {
  try {
    hotkey.value = await invoke("get_hotkey");
    log("Loaded hotkey: " + hotkey.value);
  } catch (e) {
    log("Error loading hotkey: " + e);
  }
}

async function saveHotkey(newKey) {
  try {
    await invoke("save_hotkey", { hotkey: newKey });
    hotkey.value = newKey;
    log("Saved hotkey: " + newKey);
  } catch (e) {
    log("Error saving hotkey: " + e);
  }
}

async function loadAutoMute() {
  try {
    autoMuteEnabled.value = await invoke("get_auto_mute_enabled");
    log("Auto-mute enabled: " + autoMuteEnabled.value);
  } catch (e) {
    log("Error loading auto-mute setting: " + e);
  }
}

async function saveAutoMute(enabled) {
  try {
    await invoke("set_auto_mute_enabled", { enabled });
    autoMuteEnabled.value = enabled;
    log("Auto-mute " + (enabled ? "enabled" : "disabled"));
  } catch (e) {
    log("Error saving auto-mute setting: " + e);
  }
}

async function loadLanguage() {
  try {
    selectedLanguage.value = await invoke("get_language");
    log("Loaded language: " + selectedLanguage.value);
  } catch (e) {
    log("Error loading language: " + e);
  }
}

async function saveLanguage() {
  try {
    await invoke("set_language", { lang: selectedLanguage.value });
    log("Saved language: " + selectedLanguage.value);
  } catch (e) {
    log("Error saving language: " + e);
  }
}

async function startRecording() {
  try {
     log(await invoke("start_recording"));
     isRecording.value = true;
  } catch (e) {
     log("Error: " + e);
  }
}

async function stopRecording() {
  try {
     log(await invoke("stop_recording"));
     isRecording.value = false;
  } catch (e) {
     log("Error: " + e);
  }
}

async function saveTestAudio() {
  try {
     log(await invoke("save_test_audio"));
  } catch (e) {
     log("Error: " + e);
  }
}

async function transcribeTestAudio() {
   try {
     log("Transcribing...");
     const text = await invoke("transcribe_test_audio");
     log("Transcription: " + text);
   } catch (e) {
     log("Error: " + e);
   }
}

async function injectTestText() {
  try {
     await invoke("inject_test_text", { text: "Hello from GhostWriter!" });
     log("Injected text");
  } catch (e) {
     log("Error: " + e);
  }
}

async function checkPermissions() {
  log(await invoke("check_permissions"));
}

</script>

<template>
  <!-- HUD WINDOW -->
  <Hud v-if="currentLabel === 'hud'" />

  <!-- MAIN WINDOW -->
  <div v-else class="app-container">
    <div class="glass-card">
      <div class="header">
        <h1>GhostWriter</h1>
        <div class="status-indicator" :class="{ active: isRecording }"></div>
      </div>

      <div class="section">
        <label>Global Hotkey</label>
        <HotkeyRecorder :initial-hotkey="hotkey" @update:hotkey="saveHotkey" />
        <p class="hint">Press hotkey to Start/Stop recording</p>
      </div>

      <div class="section">
        <label>Recognition Language</label>
        <select v-model="selectedLanguage" @change="saveLanguage" class="language-select">
          <option value="en">English</option>
          <option value="id">Bahasa Indonesia</option>
        </select>
        <p class="hint">Language for speech recognition</p>
      </div>

      <div class="section">
        <label class="checkbox-label">
          <input
            type="checkbox"
            v-model="autoMuteEnabled"
            @change="saveAutoMute(autoMuteEnabled)"
          />
          <span>Auto-mute system audio while recording</span>
        </label>
        <p class="hint">Mutes music/videos when you start dictating</p>
      </div>

      <div class="section logs">
        <label>Activity Log</label>
        <div class="log-window">
          <div v-for="(l, i) in logs" :key="i" class="log-entry">{{ l }}</div>
          <div v-if="logs.length === 0" class="log-entry placeholder">Waiting for action...</div>
        </div>
      </div>
      
      <div class="footer">
        <button class="text-btn" @click="showDebug = !showDebug">
            {{ showDebug ? 'Hide Debug Tools' : 'Show Debug Tools' }}
        </button>
      </div>

      <div v-if="showDebug" class="debug-panel">
        <button @click="checkPermissions">Check Perms</button>
        <button @click="startRecording">Start</button>
        <button @click="stopRecording">Stop</button>
        <button @click="saveTestAudio">Save WAV</button>
        <button @click="transcribeTestAudio">Transcribe</button>
        <button @click="injectTestText">Inject Text</button>
      </div>
    </div>
  </div>
</template>

<style>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap');

:root {
  font-family: 'Inter', sans-serif;
  /* background: #000; REMOVED */
  color: #fff;
  overflow: hidden;
}

body {
  margin: 0;
  padding: 0;
  width: 100vw;
  height: 100vh;
  background: transparent; /* Changed from gradient */
}
</style>

<style scoped>
.app-container {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  width: 100%;
  background: radial-gradient(circle at 50% -20%, #2a2a2a, #000000); /* Moved here */
}

.glass-card {
  width: 90%;
  max-width: 400px;
  background: rgba(255, 255, 255, 0.03);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 20px;
  padding: 30px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

h1 {
  font-size: 24px;
  font-weight: 600;
  margin: 0;
  background: linear-gradient(90deg, #fff, #888);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  letter-spacing: -0.5px;
}

.status-indicator {
  width: 12px;
  height: 12px;
  background: #333;
  border-radius: 50%;
  box-shadow: 0 0 0 2px rgba(255,255,255,0.1);
  transition: all 0.3s ease;
}

.status-indicator.active {
  background: #ff3b30;
  box-shadow: 0 0 10px #ff3b30;
  animation: breathe 2s infinite;
}

@keyframes breathe {
  0% { opacity: 0.8; box-shadow: 0 0 10px #ff3b30; }
  50% { opacity: 1; box-shadow: 0 0 20px #ff3b30; }
  100% { opacity: 0.8; box-shadow: 0 0 10px #ff3b30; }
}

.section label {
  display: block;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: #666;
  margin-bottom: 8px;
  font-weight: 600;
}

.checkbox-label {
  display: flex !important;
  align-items: center;
  gap: 10px;
  cursor: pointer;
}

.checkbox-label input[type="checkbox"] {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: #007aff;
}

.checkbox-label span {
  font-size: 14px;
  text-transform: none;
  letter-spacing: normal;
  color: #fff;
  font-weight: 400;
}

.hint {
  font-size: 12px;
  color: #444;
  margin-top: 8px;
  text-align: center;
}

.log-window {
  height: 120px;
  background: rgba(0,0,0,0.3);
  border-radius: 8px;
  padding: 10px;
  overflow-y: auto;
  font-family: 'Monaco', 'Consolas', monospace;
  font-size: 11px;
  border: 1px solid rgba(255,255,255,0.05);
}

.log-entry {
  color: #aaa;
  margin-bottom: 4px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.log-entry:first-child {
  color: #fff;
}

.placeholder {
  color: #444;
  font-style: italic;
}

.footer {
  text-align: center;
}

.text-btn {
  background: none;
  border: none;
  color: #666;
  font-size: 12px;
  cursor: pointer;
  transition: color 0.2s;
}

.text-btn:hover {
  color: #fff;
}

.debug-panel {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  padding-top: 10px;
  border-top: 1px solid rgba(255,255,255,0.1);
}

.debug-panel button {
  background: rgba(255,255,255,0.1);
  border: none;
  color: #eee;
  padding: 8px;
  border-radius: 6px;
  font-size: 11px;
  cursor: pointer;
  transition: background 0.2s;
}

.debug-panel button:hover {
  background: rgba(255,255,255,0.2);
}

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
</style>
