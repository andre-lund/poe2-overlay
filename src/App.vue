<script setup lang="ts">
// T5 overlay UI. Consumes the two-phase pricing contract (ADR-0004):
//   price-check-loading → item name (string); show the card while pricing runs
//   price-check-result  → PriceResult; render listings + the editable filters
// The user can toggle base-property / stat filters and pick a league, then Requery
// (an explicit button — auto-requery on every keystroke would risk the GGG IP limit;
// changing the league requeries directly since it is a single deliberate action).
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface Listing {
  display: string;
  exaltVal: number;
  age: string;
}
interface ParsedStat {
  id: string;
  text: string;
  tier: number | null;
  value: number | null;
  min: string;
  max: string;
  active: boolean;
}
interface BaseProp {
  id: string;
  text: string;
  value: string;
  active: boolean;
}
interface PriceResult {
  status: "success" | "empty" | "rateLimited" | "error";
  item: string;
  message: string | null;
  listings: Listing[];
  parsedStats: ParsedStat[];
  baseProperties: BaseProp[];
  league: string;
  leagues: string[];
}
type DangerLevel = "safe" | "caution" | "dangerous" | "deadly";
interface DangerFlag {
  severity: DangerLevel;
  label: string;
  matched: string;
  why: string;
}
interface DangerReport {
  item: string;
  level: DangerLevel;
  flags: DangerFlag[];
}
interface Pattern {
  label: string;
  regex: string;
  note: string;
}
interface Category {
  name: string;
  patterns: Pattern[];
}
interface Cheatsheet {
  categories: Category[];
  charLimit: number;
}

const itemName = ref("");
const loading = ref(false); // initial price check in flight
const busy = ref(false); // requery in flight
const result = ref<PriceResult | null>(null);
const danger = ref<DangerReport | null>(null); // set for waystones (T7), instead of a price
const cheatsheet = ref<Cheatsheet | null>(null); // set in regex mode (T8), not item-driven
const copiedRegex = ref(""); // the pattern just copied, for the "Copied" flash
const stats = ref<ParsedStat[]>([]);
const baseProps = ref<BaseProp[]>([]);
const leagues = ref<string[]>([]);
const selectedLeague = ref("");
// Monotonic token: a fresh price-check bumps it so a slow in-flight requery for the
// previous item can't overwrite the newly-checked one when it finally resolves.
const reqGen = ref(0);
const unlisten: UnlistenFn[] = [];

const hasFilters = computed(() => stats.value.length > 0 || baseProps.value.length > 0);

function applyResult(r: PriceResult) {
  danger.value = null; // a price result replaces any prior waystone danger panel
  cheatsheet.value = null;
  result.value = r;
  itemName.value = r.item;
  // Echoed filters carry the toggle state forward, so editing persists across requeries.
  stats.value = r.parsedStats.map((s) => ({ ...s }));
  baseProps.value = r.baseProperties.map((b) => ({ ...b }));
  leagues.value = r.leagues;
  selectedLeague.value = r.league;
  loading.value = false;
  busy.value = false;
}

async function requery() {
  if (busy.value || !result.value) return;
  busy.value = true;
  const myGen = reqGen.value;
  try {
    const r = await invoke<PriceResult>("requery", {
      league: selectedLeague.value,
      parsedStats: stats.value,
      baseProperties: baseProps.value,
    });
    if (myGen !== reqGen.value) return; // a newer price-check landed — drop the stale result
    applyResult(r);
  } catch (e) {
    if (myGen === reqGen.value) busy.value = false;
    console.error("requery failed", e);
  }
}

async function copyPattern(p: Pattern) {
  try {
    await invoke("copy_to_clipboard", { text: p.regex });
    copiedRegex.value = p.regex;
    setTimeout(() => {
      if (copiedRegex.value === p.regex) copiedRegex.value = "";
    }, 1500);
  } catch (e) {
    console.error("clipboard write failed", e);
  }
}

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
      reqGen.value++; // invalidate any in-flight requery for the previous item
      itemName.value = e.payload;
      result.value = null;
      danger.value = null;
      cheatsheet.value = null;
      stats.value = [];
      baseProps.value = [];
      loading.value = true;
      busy.value = false; // a new check abandons any in-flight requery
    }),
  );
  unlisten.push(
    await listen<PriceResult>("price-check-result", (e) => applyResult(e.payload)),
  );
  unlisten.push(
    await listen<DangerReport>("price-check-danger", (e) => {
      reqGen.value++; // a waystone check also invalidates any in-flight requery
      danger.value = e.payload;
      itemName.value = e.payload.item;
      result.value = null;
      cheatsheet.value = null;
      loading.value = false;
      busy.value = false; // no price result follows a danger check — reset the requery flag here
      stats.value = [];
      baseProps.value = [];
    }),
  );
  unlisten.push(
    await listen("show-regex", async () => {
      reqGen.value++; // opening the cheat-sheet abandons any in-flight requery
      busy.value = false;
      // Fetch first, then swap panels atomically — clearing the price/danger state
      // before this await would fall the template through to the stale price card for
      // the IPC round-trip; holding the prior panel until the sheet is ready avoids any
      // flash. The price-check/danger listeners clear `cheatsheet` when they fire.
      const sheet = await invoke<Cheatsheet>("get_cheatsheet");
      loading.value = false;
      result.value = null;
      danger.value = null;
      itemName.value = "";
      stats.value = [];
      baseProps.value = [];
      copiedRegex.value = "";
      cheatsheet.value = sheet;
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
      <button class="close" title="Hide (Esc / Ctrl+Alt+X)" @click="hide">✕</button>

      <template v-if="cheatsheet">
        <header class="head">
          <div class="name">Regex cheat-sheet</div>
        </header>
        <div v-for="cat in cheatsheet.categories" :key="cat.name" class="rcat">
          <div class="rcat-name">{{ cat.name }}</div>
          <button
            v-for="p in cat.patterns"
            :key="p.regex"
            class="rrow"
            :title="`Copy: ${p.regex}`"
            @click="copyPattern(p)"
          >
            <span class="rlabel">{{ p.label }}</span>
            <code class="rregex">{{ p.regex }}</code>
            <span v-if="p.note" class="rnote">{{ p.note }}</span>
            <span class="rcopied" :class="{ show: copiedRegex === p.regex }">✓ Copied</span>
          </button>
        </div>
        <div class="rfooter">
          Click a pattern → paste (Ctrl+V) into the in-game Ctrl-F box · max
          {{ cheatsheet.charLimit }} chars
        </div>
      </template>

      <div v-else-if="!itemName" class="hint">
        Hover an item in PoE2 and press Ctrl+Alt+D…
      </div>

      <template v-else-if="danger">
        <header class="head">
          <div class="name">{{ itemName }}</div>
          <span class="level" :class="danger.level">{{ danger.level }}</span>
        </header>
        <ul v-if="danger.flags.length" class="flags">
          <li v-for="(f, i) in danger.flags" :key="i" class="flag" :class="f.severity">
            <span class="dot"></span>
            <div class="fbody">
              <div class="flabel">{{ f.label }}</div>
              <div class="fwhy">{{ f.why }}</div>
              <div v-if="f.matched" class="fmod">{{ f.matched }}</div>
            </div>
          </li>
        </ul>
        <div v-else class="status safe">No dangerous mods — safe to run.</div>
      </template>

      <template v-else>
        <header class="head">
          <div class="name">{{ itemName }}</div>
          <select
            v-if="leagues.length"
            v-model="selectedLeague"
            class="league"
            :disabled="busy"
            @change="requery"
          >
            <option v-for="lg in leagues" :key="lg" :value="lg">{{ lg }}</option>
          </select>
        </header>

        <section v-if="hasFilters" class="filters">
          <label v-for="bp in baseProps" :key="bp.id" class="row">
            <input v-model="bp.active" type="checkbox" :disabled="busy" />
            <span class="ftext">{{ bp.text }}</span>
          </label>

          <div v-for="(st, i) in stats" :key="st.id + i" class="row stat">
            <input v-model="st.active" type="checkbox" :disabled="busy" />
            <span class="ftext">{{ st.text }}</span>
            <input v-model="st.min" class="num" placeholder="min" :disabled="busy || !st.active" />
            <input v-model="st.max" class="num" placeholder="max" :disabled="busy || !st.active" />
          </div>

          <button class="requery" :disabled="busy" @click="requery">
            {{ busy ? "Searching…" : "Requery" }}
          </button>
        </section>

        <div v-if="loading" class="status">Searching market…</div>

        <template v-else-if="result">
          <ul v-if="result.listings.length" class="listings" :class="{ stale: busy }">
            <li v-for="(l, i) in result.listings" :key="i" class="listing">
              <span class="price">{{ l.display }}</span>
              <span v-if="l.age" class="age">{{ l.age }}</span>
            </li>
          </ul>
          <div v-else class="status" :class="{ err: result.status === 'error' }">
            {{ result.message || "No listings" }}
          </div>
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
/* The backdrop is click-through; only the card itself captures input. The layer-shell
   surface is a bounded centred rectangle (ADR-0003), so clicks outside it reach the
   game regardless. */
.overlay-root {
  position: fixed;
  inset: 0;
  pointer-events: none;
}

/* Constant-size card filling the surface with internal scroll. A constant painted
   region avoids the WebKitGTK transparent-repaint ghost stacking from T3 (ADR-0003). */
.card {
  position: absolute;
  inset: 6px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px;
  overflow-y: auto;
  border-radius: 10px;
  background: rgba(10, 12, 20, 0.92);
  border: 1px solid rgba(120, 180, 255, 0.55);
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.5);
  color: #cfe3ff;
  font: 600 13px/1.4 Inter, system-ui, sans-serif;
  pointer-events: auto;
}

.close {
  position: absolute;
  top: 8px;
  right: 10px;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: rgba(120, 180, 255, 0.15);
  color: #cfe3ff;
  font-size: 14px;
  line-height: 24px;
  cursor: pointer;
}

.close:hover {
  background: rgba(120, 180, 255, 0.32);
}

.hint {
  padding-right: 28px;
  color: #aebfd6;
  font-weight: 400;
}

.head {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-right: 28px;
}

.name {
  font-size: 14px;
  color: #e8c98a;
}

.league {
  align-self: flex-start;
  max-width: 100%;
  padding: 3px 6px;
  border-radius: 6px;
  border: 1px solid rgba(120, 180, 255, 0.35);
  background: rgba(20, 24, 36, 0.95);
  color: #cfe3ff;
  font: inherit;
}

.filters {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 0;
  border-top: 1px solid rgba(120, 180, 255, 0.14);
  border-bottom: 1px solid rgba(120, 180, 255, 0.14);
}

.row {
  display: flex;
  align-items: center;
  gap: 7px;
  font-weight: 400;
}

.row input[type="checkbox"] {
  flex: none;
  accent-color: #78b4ff;
}

.ftext {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #c2d2ea;
}

.stat .num {
  flex: none;
  width: 42px;
  padding: 2px 4px;
  border-radius: 5px;
  border: 1px solid rgba(120, 180, 255, 0.3);
  background: rgba(20, 24, 36, 0.95);
  color: #cfe3ff;
  font: inherit;
  text-align: center;
}

.stat .num:disabled {
  opacity: 0.4;
}

.requery {
  align-self: flex-start;
  margin-top: 4px;
  padding: 4px 12px;
  border: none;
  border-radius: 6px;
  background: rgba(120, 180, 255, 0.22);
  color: #eaf2ff;
  font: 600 12px/1.4 Inter, system-ui, sans-serif;
  cursor: pointer;
}

.requery:hover:not(:disabled) {
  background: rgba(120, 180, 255, 0.38);
}

.requery:disabled {
  opacity: 0.6;
  cursor: default;
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

/* Dim listings while a requery is in flight so the shown prices read as stale. */
.listings.stale {
  opacity: 0.45;
}

.listing {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding: 4px 0;
  border-bottom: 1px solid rgba(120, 180, 255, 0.12);
}

.price {
  font: 600 13px/1.4 "JetBrains Mono", ui-monospace, monospace;
  color: #cfe3ff;
}

.age {
  font-size: 11px;
  color: #7e8aa0;
}

.status.safe {
  color: #8fe3a0;
}

/* --- waystone danger panel (T7) --- */
.level {
  align-self: flex-start;
  padding: 2px 9px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: #0a0c14;
}
.level.safe {
  background: #8fe3a0;
}
.level.caution {
  background: #e8c98a;
}
.level.dangerous {
  background: #ff9d5c;
}
.level.deadly {
  background: #ff6b6b;
}

.flags {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.flag {
  display: flex;
  gap: 8px;
  padding: 6px 0;
  border-bottom: 1px solid rgba(120, 180, 255, 0.12);
}

.flag .dot {
  flex: none;
  width: 8px;
  height: 8px;
  margin-top: 5px;
  border-radius: 50%;
}
.flag.caution .dot {
  background: #e8c98a;
}
.flag.dangerous .dot {
  background: #ff9d5c;
}
.flag.deadly .dot {
  background: #ff6b6b;
}

.fbody {
  min-width: 0;
}

.flabel {
  font-weight: 600;
  color: #eaf2ff;
}

.fwhy {
  font-weight: 400;
  font-size: 12px;
  color: #aebfd6;
}

.fmod {
  margin-top: 2px;
  font: 400 11px/1.4 "JetBrains Mono", ui-monospace, monospace;
  color: #7e8aa0;
}

/* --- regex cheat-sheet (T8) --- */
.rcat {
  margin-bottom: 8px;
}

.rcat-name {
  margin: 6px 0 3px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: #7e8aa0;
}

.rrow {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px 10px;
  width: 100%;
  padding: 5px 8px;
  border: none;
  border-radius: 6px;
  background: rgba(120, 180, 255, 0.06);
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
  margin-bottom: 3px;
}

.rrow:hover {
  background: rgba(120, 180, 255, 0.16);
}

.rlabel {
  font-weight: 600;
  color: #cfe3ff;
}

.rregex {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font: 400 12px/1.4 "JetBrains Mono", ui-monospace, monospace;
  color: #e8c98a;
}

.rnote {
  flex-basis: 100%;
  font-size: 11px;
  font-weight: 400;
  color: #7e8aa0;
}

.rcopied {
  position: absolute;
  top: 5px;
  right: 8px;
  font-size: 11px;
  font-weight: 600;
  color: #8fe3a0;
  opacity: 0;
  transition: opacity 0.1s;
}

.rcopied.show {
  opacity: 1;
}

.rfooter {
  margin-top: 6px;
  font-size: 11px;
  font-weight: 400;
  color: #7e8aa0;
}
</style>
