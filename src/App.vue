<script setup lang="ts">
// T4: the price-check flow now emits a two-phase contract (ADR-0004):
//   price-check-loading  → item name (string); show the card while pricing runs
//   price-check-result   → PriceResult; render the cheapest listings or a status
// The rich per-stat toggle / league-selector / requery UI is T5; this is the minimal
// verifiable render of the pricing core.
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface Listing {
  display: string;
  exaltVal: number;
  age: string;
}
interface PriceResult {
  status: "success" | "empty" | "rateLimited" | "error";
  item: string;
  message: string | null;
  listings: Listing[];
  league: string;
  leagues: string[];
}

const loading = ref(false);
const itemName = ref("");
const result = ref<PriceResult | null>(null);
const unlisten: UnlistenFn[] = [];

function hide() {
  invoke("hide_overlay");
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") hide();
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  unlisten.push(
    await listen<string>("price-check-loading", (e) => {
      itemName.value = e.payload;
      result.value = null;
      loading.value = true;
    }),
  );
  unlisten.push(
    await listen<PriceResult>("price-check-result", (e) => {
      result.value = e.payload;
      itemName.value = e.payload.item;
      loading.value = false;
    }),
  );
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  unlisten.forEach((u) => u());
});
</script>

<template>
  <div class="overlay-root">
    <div class="card">
      <button class="close" title="Hide (Esc)" @click="hide">✕</button>

      <div v-if="!itemName" class="hint">
        Hover an item in PoE2 and press Ctrl+Alt+D…
      </div>

      <template v-else>
        <div class="name">{{ itemName }}</div>

        <div v-if="loading" class="status">Searching market…</div>

        <template v-else-if="result">
          <ul v-if="result.listings.length" class="listings">
            <li v-for="(l, i) in result.listings" :key="i" class="listing">
              <span class="price">{{ l.display }}</span>
              <span v-if="l.age" class="age">{{ l.age }}</span>
            </li>
          </ul>
          <div v-else class="status" :class="{ err: result.status === 'error' }">
            {{ result.message || "No listings" }}
          </div>
          <div class="league">{{ result.league }}</div>
        </template>
      </template>
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
     overpaints the same region every time (T3 finding, ADR-0003). */
  height: 380px;
  overflow: auto;
  padding: 16px 20px;
  border-radius: 10px;
  background: rgba(10, 12, 20, 0.9);
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

.hint {
  padding-right: 30px;
  color: #aebfd6;
}

.name {
  padding-right: 30px;
  margin-bottom: 10px;
  font-size: 15px;
  color: #e8c98a;
}

.status {
  color: #aebfd6;
  font-weight: 400;
}

.status.err {
  color: #ff9d9d;
}

.listings {
  list-style: none;
  margin: 0;
  padding: 0;
}

.listing {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding: 4px 0;
  border-bottom: 1px solid rgba(120, 180, 255, 0.12);
}

.price {
  font: 600 14px/1.4 "JetBrains Mono", ui-monospace, monospace;
  color: #cfe3ff;
}

.age {
  font-size: 11px;
  color: #7e8aa0;
}

.league {
  margin-top: 10px;
  font-size: 11px;
  font-weight: 400;
  color: #7e8aa0;
}
</style>
