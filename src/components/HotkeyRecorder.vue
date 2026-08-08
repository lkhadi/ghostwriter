<script setup>
import { ref, watch } from "vue";

const props = defineProps({
  initialHotkey: {
    type: String,
    default: "",
  },
});

const emit = defineEmits(["update:hotkey"]);

const isRecording = ref(false);
const currentHotkey = ref(props.initialHotkey);
const displayHotkey = ref(props.initialHotkey);

// The parent loads the hotkey asynchronously in onMounted, after setup() has
// already snapshotted the prop — without this the box stays blank on launch.
watch(
  () => props.initialHotkey,
  (value) => {
    currentHotkey.value = value;
    if (!isRecording.value) displayHotkey.value = value;
  }
);

const MODIFIER_CODES = [
  "MetaLeft",
  "MetaRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
];

// e.code is the physical key ("KeyA", "Space", "Digit1", "ArrowUp"), so it is
// unaffected by Option producing dead keys on macOS — e.key would turn
// Option+A into "å" and the backend would reject the shortcut.
// Modifiers must precede the key: global-hotkey rejects "Command+KeyA+Shift".
const formatKey = (e) => {
  const keys = [];
  if (e.metaKey) keys.push("Command");
  if (e.ctrlKey) keys.push("Control");
  if (e.altKey) keys.push("Alt");
  if (e.shiftKey) keys.push("Shift");

  if (!MODIFIER_CODES.includes(e.code)) keys.push(e.code);

  return keys.join("+");
};

const handleKeyDown = (e) => {
  if (!isRecording.value) return;

  e.preventDefault();
  e.stopPropagation();

  // If user presses Escape, cancel recording
  if (e.code === "Escape") {
    isRecording.value = false;
    displayHotkey.value = currentHotkey.value; // Revert
    return;
  }

  // Build the string
  const hotkeyString = formatKey(e);
  displayHotkey.value = hotkeyString;

  // Commit once a non-modifier key lands. A shortcut needs exactly one main
  // key, so modifier-only combinations can never be committed.
  if (!MODIFIER_CODES.includes(e.code)) {
    currentHotkey.value = hotkeyString;
    emit("update:hotkey", hotkeyString);
    isRecording.value = false;
  }
};

const startRecording = () => {
  isRecording.value = true;
  displayHotkey.value = "Press keys...";
};

// Called by the parent when the backend rejects a shortcut, so the box stops
// showing a combination that was never registered.
const revertDisplay = () => {
  isRecording.value = false;
  currentHotkey.value = props.initialHotkey;
  displayHotkey.value = props.initialHotkey;
};

defineExpose({ revertDisplay });

const inputRef = ref(null);

// Focus trap logic if needed, but simple focus is enough usually
</script>

<template>
  <div class="hotkey-recorder">
    <div 
      class="recorder-box"
      :class="{ recording: isRecording }"
      @click="startRecording"
      tabindex="0"
      @keydown="handleKeyDown"
      ref="inputRef"
    >
      <span v-if="displayHotkey">{{ displayHotkey }}</span>
      <span v-else class="placeholder">Click to record hotkey</span>
    </div>
  </div>
</template>

<style scoped>
.hotkey-recorder {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
}

.recorder-box {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 12px 20px;
  width: 100%;
  text-align: center;
  color: #fff;
  font-family: inherit;
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s ease;
  user-select: none;
  outline: none;
  box-shadow: 0 4px 6px rgba(0,0,0,0.1);
}

.recorder-box:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.2);
}

.recorder-box.recording {
  background: rgba(255, 59, 48, 0.1);
  border-color: #ff3b30;
  color: #ff3b30;
  animation: pulse 2s infinite;
}

.placeholder {
  color: rgba(255, 255, 255, 0.4);
}

@keyframes pulse {
  0% { box-shadow: 0 0 0 0 rgba(255, 59, 48, 0.4); }
  70% { box-shadow: 0 0 0 10px rgba(255, 59, 48, 0); }
  100% { box-shadow: 0 0 0 0 rgba(255, 59, 48, 0); }
}
</style>
