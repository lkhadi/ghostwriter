
<script setup>
</script>

<template>
  <div class="hud-container">
    <div class="hud-content">
      <div class="recording-indicator"></div>
      <span class="status-text">Recording...</span>
    </div>
  </div>
</template>

<style scoped>
.hud-container {
  width: 100vw;
  height: 100vh;
  display: flex;
  justify-content: center;
  align-items: center;
  background: transparent;
  user-select: none;
  cursor: default;
}

.hud-content {
  display: flex;
  align-items: center;
  gap: 12px;
  background: rgba(20, 20, 20, 0.85);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  padding: 12px 20px;
  border-radius: 999px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}

.recording-indicator {
  width: 12px;
  height: 12px;
  background-color: #ff4b4b;
  border-radius: 50%;
  animation: pulse 1.5s infinite;
}

.status-text {
  color: white;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 14px;
  font-weight: 500;
  letter-spacing: 0.5px;
}

@keyframes pulse {
  0% { transform: scale(0.95); opacity: 0.8; box-shadow: 0 0 0 0 rgba(255, 75, 75, 0.7); }
  70% { transform: scale(1); opacity: 1; box-shadow: 0 0 0 6px rgba(255, 75, 75, 0); }
  100% { transform: scale(0.95); opacity: 0.8; box-shadow: 0 0 0 0 rgba(255, 75, 75, 0); }
}
</style>
