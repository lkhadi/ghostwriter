<script setup>
import { ref, onMounted, onBeforeUnmount } from "vue";

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

// Maps for prettier display
const keyMap = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Meta: "Cmd",
  Control: "Ctrl",
  Alt: "Option",
  Shift: "Shift",
};

const formatKey = (e) => {
  const keys = [];
  if (e.metaKey) keys.push("Command");
  if (e.ctrlKey) keys.push("Control");
  if (e.altKey) keys.push("Alt"); // "Option" in Tauri usually maps to Alt string or Option
  if (e.shiftKey) keys.push("Shift");

  // If the key is not a modifier itself, add it
  if (
    !["Meta", "Control", "Alt", "Shift"].includes(e.key)
  ) {
    let key = e.key.toUpperCase();
    if (key === " ") key = "Space";
    keys.push(key);
  }

  // Deduplicate
  return [...new Set(keys)].join("+");
};

const handleKeyDown = (e) => {
  if (!isRecording.value) return;

  e.preventDefault();
  e.stopPropagation();

  // If user presses Escape, cancel recording
  if (e.key === "Escape") {
    isRecording.value = false;
    displayHotkey.value = currentHotkey.value; // Revert
    return;
  }

  // Build the string
  const hotkeyString = formatKey(e);
  displayHotkey.value = hotkeyString;

  // We only "commit" if a non-modifier key is pressed OR if we want to allow single keys
  // For better UX, we usually wait for a non-modifier.
  // HOWEVER, user asked for "Right Command" specifically. 
  // If they want to bind JUST a modifier, we might need to allow it.
  
  // Logic: If keys array has a non-modifier, stop recording.
  const hasNonModifier = !["Meta", "Control", "Alt", "Shift"].includes(e.key);
  
  if (hasNonModifier) {
      currentHotkey.value = hotkeyString;
      emit("update:hotkey", hotkeyString);
      isRecording.value = false;
  }
};

const startRecording = () => {
  isRecording.value = true;
  displayHotkey.value = "Press keys...";
};

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
  font-family: 'Inter', sans-serif;
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
