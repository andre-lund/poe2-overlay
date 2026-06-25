<script setup lang="ts">
// T3 proof: the overlay is hidden until the Insert hotkey fires a price check,
// which synthesizes Ctrl+C into PoE2, reads the clipboard, and emits the item
// text here. T4 parses + prices it; T5 builds the real listings UI.
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const itemText = ref("");
let unlisten: UnlistenFn | undefined;

function hide() {
  invoke("hide_overlay");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") hide();
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  unlisten = await listen<string>("price-check-item", (e) => {
    itemText.value = e.payload;
  });
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  unlisten?.();
});
</script>

<template>
  <div class="overlay-root">
    <div class="card">
      <button class="close" title="Hide (Esc)" @click="hide">✕</button>
      <div class="badge">PoE2 Overlay — copied item (T3)</div>
      <pre class="item">{{ itemText || "Hover an item in PoE2 and press Ctrl+Alt+D…" }}</pre>
    </div>
  </div>
</template>

<style>
:root,
html,
body,
#app {
  margin: 0;
  height: 100%;
  background: transparent !important;
}
</style>

<style scoped>
.overlay-root {
  position: fixed;
  inset: 0;
  pointer-events: none;
}

.card {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 400px;
  /* Fixed (not max-) height: a content-sized card shrinks for shorter items, and
     WebKitGTK leaves the previously-painted transparent region uncleared until a later
     repaint — so old cards linger stacked behind the new one. A constant-size card
     overpaints the same region every time. */
  height: 380px;
  overflow: auto;
  padding: 16px 20px;
  border-radius: 10px;
  background: rgba(10, 12, 20, 0.88);
  border: 1px solid rgba(120, 180, 255, 0.55);
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.5);
  color: #cfe3ff;
  font: 600 14px/1.4 Inter, system-ui, sans-serif;
  pointer-events: auto;
}

.close {
  position: absolute;
  top: 8px;
  right: 10px;
  width: 26px;
  height: 26px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: rgba(120, 180, 255, 0.15);
  color: #cfe3ff;
  font-size: 15px;
  line-height: 26px;
  cursor: pointer;
}

.close:hover {
  background: rgba(120, 180, 255, 0.32);
}

.badge {
  padding-right: 30px;
  margin-bottom: 8px;
}

.item {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font: 400 12px/1.45 "JetBrains Mono", ui-monospace, monospace;
  color: #aebfd6;
}
</style>
