<script setup lang="ts">
// T2 probe: a top-right card on the full-screen layer-shell OVERLAY surface, to
// verify the surface composites over fullscreen Proton PoE2. The surface is modal
// while shown (covers the screen); dismiss with the ✕ or Esc (both invoke the
// hide_overlay command). Real pricing UI + hotkey-gated show land in T5/T3.
import { onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

function hide() {
  invoke("hide_overlay");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") hide();
}

onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="overlay-root">
    <div class="card">
      <button class="close" title="Hide (Esc)" @click="hide">✕</button>
      <div class="badge">PoE2 Overlay — layer-shell OVERLAY surface</div>
      <p class="hint">Composites over fullscreen PoE2 → T2 works.</p>
      <p class="hint">Click ✕ or press Esc to dismiss.</p>
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
/* Full-screen transparent canvas; the card is the only visible/interactive bit. */
.overlay-root {
  position: fixed;
  inset: 0;
  pointer-events: none;
}

.card {
  position: fixed;
  top: 24px;
  right: 24px;
  padding: 16px 20px;
  border-radius: 10px;
  background: rgba(10, 12, 20, 0.85);
  border: 1px solid rgba(120, 180, 255, 0.55);
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.5);
  color: #cfe3ff;
  font: 600 15px/1.4 Inter, system-ui, sans-serif;
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
}

.hint {
  margin: 6px 0 0;
  font-weight: 400;
  font-size: 12px;
  color: #93a7c4;
}
</style>
