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
  total: number | null;
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
interface SheetEntry {
  name: string;
  display: string;
  exaltVal: number;
}
interface SheetGroup {
  name: string;
  categories: string[];
}
interface PriceSheet {
  league: string;
  category: string;
  groups: SheetGroup[];
  entries: SheetEntry[];
}

const itemName = ref("");
const loading = ref(false); // initial price check in flight
const busy = ref(false); // requery in flight
const result = ref<PriceResult | null>(null);
const danger = ref<DangerReport | null>(null); // set for waystones (T7), instead of a price
const cheatsheet = ref<Cheatsheet | null>(null); // set in regex mode (T8), not item-driven
const priceSheet = ref<PriceSheet | null>(null); // set in price-sheet mode (T9), not item-driven
const sheetFilter = ref(""); // price sheet name filter
const sheetBusy = ref(false); // category switch in flight
const catsOpen = ref(true); // category tabs expanded (collapsible to save card space)
const copiedRegex = ref(""); // the pattern just copied, for the "Copied" flash
const stats = ref<ParsedStat[]>([]);
const baseProps = ref<BaseProp[]>([]);
const leagues = ref<string[]>([]);
const selectedLeague = ref("");
// Monotonic token: a fresh price-check bumps it so a slow in-flight requery for the
// previous item can't overwrite the newly-checked one when it finally resolves. The
// panel-open listeners and the sheet category switch guard on it too — any async fetch
// that resolves after a newer trigger must drop its result, not resurrect a stale panel.
const reqGen = ref(0);
// Rate-limit countdown, seconds remaining. Display + affordance gating only — the
// backend lockout stays the source of truth (a premature search just returns another
// rateLimited result, restarting the countdown from the server's number).
const rateWait = ref(0);
let rateTimer: number | undefined;
const unlisten: UnlistenFn[] = [];

function stopCountdown() {
  if (rateTimer !== undefined) {
    clearInterval(rateTimer);
    rateTimer = undefined;
  }
  rateWait.value = 0;
}

function startCountdown(secs: number) {
  stopCountdown();
  rateWait.value = secs;
  rateTimer = window.setInterval(() => {
    if (rateWait.value <= 1) {
      stopCountdown(); // hits 0 → the status flips to "cleared" and the button re-enables
    } else {
      rateWait.value--;
    }
  }, 1000);
}

const hasFilters = computed(() => stats.value.length > 0 || baseProps.value.length > 0);

// Rarity class for the item-name header, PoE2 tooltip colors (rare yellow, unique
// orange, …). Derived from the echoed base properties; bulk results carry none and
// fall back to the default gold.
const rarityClass = computed(() => {
  const r = baseProps.value.find((b) => b.id === "rarity")?.value.toLowerCase() ?? "";
  const map: Record<string, string> = {
    normal: "r-normal",
    magic: "r-magic",
    rare: "r-rare",
    unique: "r-unique",
    currency: "r-currency",
  };
  return map[r] ?? "";
});

// PoE2 tooltips stack a rare/unique name over its base type on two lines; the backend
// sends "Name (Base Type)", so split it back apart for the header. Waystones split the
// same way ("Dread Core (Waystone Tier 15)"), matching the game.
const nameParts = computed(() => {
  const m = itemName.value.match(/^(.*) \((.+)\)$/);
  return m ? [m[1], m[2]] : [itemName.value];
});

// Price spread over the fetched (cheapest-first) listings plus the search's total match
// count, so a wall of identical floor prices reads as "the floor of a big pool" instead
// of the item's value.
const spread = computed(() => {
  const r = result.value;
  if (!r || r.listings.length < 2) return "";
  const lo = r.listings[0].display;
  const hi = r.listings[r.listings.length - 1].display;
  const range = lo === hi ? lo : `${lo} – ${hi}`;
  return r.total && r.total > r.listings.length
    ? `${range} · cheapest ${r.listings.length} of ${r.total} matches`
    : `${range} · ${r.listings.length} matches`;
});

// Sheet-name matching ignores case and apostrophes: poe.ninja ids lose the apostrophe
// ("Craiceanns"), so a user typing the in-game "Craiceann's" must still hit.
const filteredEntries = computed(() => {
  const sheet = priceSheet.value;
  if (!sheet) return [];
  const q = sheetFilter.value.toLowerCase().replace(/'/g, "").trim();
  if (!q) return sheet.entries;
  return sheet.entries.filter((e) => e.name.toLowerCase().replace(/'/g, "").includes(q));
});

// The group whose category chips are shown: the one containing the active category.
const activeGroup = computed(() => {
  const sheet = priceSheet.value;
  if (!sheet) return null;
  return sheet.groups.find((g) => g.categories.includes(sheet.category)) ?? sheet.groups[0];
});

// Clicking a group tab jumps to that group's first category.
function switchGroup(g: SheetGroup) {
  if (g.name === activeGroup.value?.name) return;
  switchCategory(g.categories[0]);
}

// Switch the sheet to another poe.ninja category (one round-trip). The current
// entries stay visible, dimmed, until the new sheet lands — same anti-flash idea
// as the panel-open listeners.
async function switchCategory(category: string) {
  const sheet = priceSheet.value;
  if (sheetBusy.value || !sheet || category === sheet.category) return;
  sheetBusy.value = true;
  const myGen = reqGen.value;
  try {
    const next = await invoke<PriceSheet>("get_price_sheet", { category });
    // A price check / panel switch landed while this fetch ran — its listener already
    // cleared the sheet; assigning now would resurrect it over the new panel.
    if (myGen !== reqGen.value) return;
    sheetFilter.value = "";
    priceSheet.value = next;
  } catch (e) {
    console.error("price sheet fetch failed", e);
  } finally {
    sheetBusy.value = false;
  }
}

function applyResult(r: PriceResult) {
  danger.value = null; // a price result replaces any prior waystone danger panel
  cheatsheet.value = null;
  priceSheet.value = null;
  result.value = r;
  itemName.value = r.item;
  // Tick down the backend's "wait Ns" so the card isn't a stale static number.
  if (r.status === "rateLimited") {
    const secs = Number(r.message?.match(/(\d+)s/)?.[1] ?? 0);
    if (secs > 0) startCountdown(secs);
    else stopCountdown();
  } else {
    stopCountdown();
  }
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

// Human label for a base-property filter so each checkbox says what it matches, not
// just a bare value (the id is the stable key from the backend's build_base_properties).
function propKind(id: string): string {
  switch (id) {
    case "class":
      return "Class";
    case "rarity":
      return "Rarity";
    case "base":
      return "Base type";
    case "name":
      return "Name";
    case "ilvl":
      return "Item level";
    case "gemLevel":
      return "Gem level";
    case "quality":
      return "Quality";
    case "sockets":
    case "gem_sockets":
      return "Sockets";
    default:
      return "Filter";
  }
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") hide();
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  unlisten.push(
    await listen<string>("price-check-loading", (e) => {
      reqGen.value++; // invalidate any in-flight requery for the previous item
      stopCountdown(); // a fresh check owns the card; the result restarts it if limited
      itemName.value = e.payload;
      result.value = null;
      danger.value = null;
      cheatsheet.value = null;
      priceSheet.value = null;
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
      priceSheet.value = null;
      loading.value = false;
      busy.value = false; // no price result follows a danger check — reset the requery flag here
      stats.value = [];
      baseProps.value = [];
    }),
  );
  unlisten.push(
    await listen("show-regex", async () => {
      reqGen.value++; // opening the cheat-sheet abandons any in-flight requery
      const myGen = reqGen.value;
      busy.value = false;
      // Fetch first, then swap panels atomically — clearing the price/danger state
      // before this await would fall the template through to the stale price card for
      // the IPC round-trip; holding the prior panel until the sheet is ready avoids any
      // flash. The price-check/danger listeners clear `cheatsheet` when they fire.
      const sheet = await invoke<Cheatsheet>("get_cheatsheet");
      // A price check / other panel superseded this open while the fetch ran — showing
      // the sheet now would bury the newer panel under a stale one.
      if (myGen !== reqGen.value) return;
      loading.value = false;
      result.value = null;
      danger.value = null;
      itemName.value = "";
      stats.value = [];
      baseProps.value = [];
      copiedRegex.value = "";
      priceSheet.value = null;
      cheatsheet.value = sheet;
    }),
  );
  unlisten.push(
    await listen("show-runes", async () => {
      reqGen.value++; // opening the price sheet abandons any in-flight requery
      const myGen = reqGen.value;
      busy.value = false;
      // Same anti-flash pattern as show-regex: fetch first (one poe.ninja round-trip),
      // then swap panels atomically so the prior card holds until the sheet is ready.
      // Reopening always lands on the default category (Runes) — the backend maps ""
      // to it — so the hotkey's behavior is predictable regardless of the last tab.
      const sheet = await invoke<PriceSheet>("get_price_sheet", { category: "" });
      if (myGen !== reqGen.value) return; // superseded while fetching — drop, don't resurrect
      loading.value = false;
      result.value = null;
      danger.value = null;
      itemName.value = "";
      stats.value = [];
      baseProps.value = [];
      cheatsheet.value = null;
      sheetFilter.value = "";
      sheetBusy.value = false;
      priceSheet.value = sheet;
    }),
  );
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  stopCountdown();
  unlisten.forEach((u) => u());
});
</script>

<template>
  <div class="overlay-root">
    <div class="card">
      <button class="close" title="Hide (Ctrl+Alt+X)" @click="hide">✕</button>

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

      <template v-else-if="priceSheet">
        <header class="head">
          <div class="name">{{ priceSheet.category }}</div>
          <div class="sheet-league">
            {{ priceSheet.league }} · poe.ninja ·
            <button class="sheet-toggle" @click="catsOpen = !catsOpen">
              {{ catsOpen ? "▾ categories" : "▸ categories" }}
            </button>
          </div>
        </header>
        <template v-if="catsOpen">
          <div class="sheet-groups">
            <button
              v-for="g in priceSheet.groups"
              :key="g.name"
              class="sheet-group"
              :class="{ active: g.name === activeGroup?.name }"
              :disabled="sheetBusy"
              @click="switchGroup(g)"
            >
              {{ g.name }}
            </button>
          </div>
          <div class="sheet-cats">
            <button
              v-for="c in activeGroup?.categories ?? []"
              :key="c"
              class="sheet-cat"
              :class="{ active: c === priceSheet.category }"
              :disabled="sheetBusy"
              @click="switchCategory(c)"
            >
              {{ c }}
            </button>
          </div>
        </template>
        <input
          v-model="sheetFilter"
          class="sheet-filter"
          type="text"
          placeholder="Filter by name… (e.g. craiceann)"
        />
        <ul v-if="filteredEntries.length" class="listings" :class="{ stale: sheetBusy }">
          <li v-for="e in filteredEntries" :key="e.name" class="listing">
            <span class="sheet-name">{{ e.name }}</span>
            <span class="price">{{ e.display }}</span>
          </li>
        </ul>
        <div v-else-if="priceSheet.entries.length" class="status">Nothing matches the filter.</div>
        <div v-else class="status err">
          poe.ninja unreachable — pick another category or press Ctrl+Alt+F to retry.
        </div>
      </template>

      <template v-else-if="!itemName">
        <header class="head">
          <div class="name">PoE2 Overlay</div>
        </header>
        <div class="hint">
          <div class="hint-row"><kbd>Ctrl+Alt+D</kbd><span>price-check the hovered item</span></div>
          <div class="hint-row"><kbd>Ctrl+Alt+F</kbd><span>open the price sheet</span></div>
          <div class="hint-row"><kbd>Ctrl+Alt+X</kbd><span>hide the overlay</span></div>
        </div>
      </template>

      <template v-else-if="danger">
        <header class="head">
          <div class="name">
            <div>{{ nameParts[0] }}</div>
            <div v-if="nameParts[1]" class="name-base">{{ nameParts[1] }}</div>
          </div>
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
          <div class="name" :class="rarityClass">
            <div>{{ nameParts[0] || "Unrecognized item" }}</div>
            <div v-if="nameParts[1]" class="name-base">{{ nameParts[1] }}</div>
          </div>
          <label v-if="leagues.length" class="league-field">
            <span class="field-label">League</span>
            <div class="select-wrap">
              <select v-model="selectedLeague" class="league" :disabled="busy || rateWait > 0" @change="requery">
                <option v-for="lg in leagues" :key="lg" :value="lg">{{ lg }}</option>
              </select>
            </div>
          </label>
        </header>

        <section v-if="hasFilters" class="filters">
          <div class="filters-head">
            <span class="filters-title">Search filters</span>
            <span class="filters-hint">Tick the item properties to match in the trade search</span>
          </div>

          <label v-for="bp in baseProps" :key="bp.id" class="row">
            <input v-model="bp.active" type="checkbox" :disabled="busy" />
            <span class="fkind">{{ propKind(bp.id) }}</span>
            <span class="ftext">{{ bp.text }}</span>
          </label>

          <div v-for="(st, i) in stats" :key="st.id + i" class="row stat">
            <input v-model="st.active" type="checkbox" :disabled="busy" />
            <span class="ftext">{{ st.text }}</span>
            <input v-model="st.min" class="num" placeholder="min" :disabled="busy || !st.active" />
            <input v-model="st.max" class="num" placeholder="max" :disabled="busy || !st.active" />
          </div>

          <button class="requery" :disabled="busy || rateWait > 0" @click="requery">
            {{ busy ? "Searching…" : rateWait > 0 ? `Wait ${rateWait}s…` : "Search again" }}
          </button>
        </section>

        <div v-if="loading" class="status searching">Searching market…</div>

        <template v-else-if="result">
          <div v-if="spread" class="spread" :class="{ stale: busy }">{{ spread }}</div>
          <ul v-if="result.listings.length" class="listings" :class="{ stale: busy }">
            <li v-for="(l, i) in result.listings" :key="i" class="listing">
              <span class="price">{{ l.display }}</span>
              <span v-if="l.age" class="age">{{ l.age }}</span>
            </li>
          </ul>
          <div v-else class="status" :class="{ err: result.status === 'error' }">
            <template v-if="result.status === 'rateLimited'">
              {{
                rateWait > 0
                  ? `Rate limit — try again in ${rateWait}s`
                  : "Rate limit cleared — you can search again."
              }}
            </template>
            <template v-else>{{ result.message || "No listings" }}</template>
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
/* PoE2 in-game look: the panel borrows the item-tooltip vocabulary the player already
   parses hundreds of times per session — black glass, bronze/gold chrome, Fontin serif
   (the game's own typeface; falls back to system serifs when not installed), rarity
   colors on the item name. See PRODUCT.md / DESIGN.md. */

/* The backdrop is click-through; only the card itself captures input. The layer-shell
   surface is a bounded centred rectangle (ADR-0003), so clicks outside it reach the
   game regardless. */
.overlay-root {
  position: fixed;
  inset: 0;
  pointer-events: none;

  /* Theme tokens (PoE2 tooltip palette; sRGB canon from the game UI, not composed). */
  --bg: #0c0a07;
  --bg-raised: #171207;
  --edge: #574a2c;
  --edge-dim: #3d331e;
  --edge-hi: #8a7444;
  --ink: #d6cbb2;
  --ink-dim: #9a8d70;
  --gold: #c8aa6d;
  --gold-bright: #e8d5a0;
  --serif: Fontin, "Palatino Linotype", Palatino, Georgia, serif;
  --smallcaps: "Fontin SmallCaps", Fontin, Palatino, Georgia, serif;
}

/* Constant-size card filling the surface with internal scroll. A constant painted
   region avoids the WebKitGTK transparent-repaint ghost stacking from T3 (ADR-0003).
   Solid near-black: an 8%-transparent panel let the bright game scene bleed through
   and wash out the text (T3 lesson) — readability over glass. */
.card {
  position: absolute;
  inset: 6px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px;
  overflow-y: auto;
  border-radius: 3px;
  background: linear-gradient(180deg, #141008 0%, var(--bg) 90px);
  border: 1px solid var(--edge);
  /* Bronze double-edge: a black outer line + a faint gilt inner bevel, the tooltip
     frame idiom — plus the drop shadow lifting the panel off the game scene. */
  box-shadow:
    0 0 0 1px #000,
    inset 0 0 0 1px rgba(232, 213, 160, 0.09),
    0 10px 32px rgba(0, 0, 0, 0.8);
  color: var(--ink);
  font: 400 15px/1.45 var(--serif);
  pointer-events: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--edge-dim) transparent;
}

.card::-webkit-scrollbar {
  width: 8px;
}
.card::-webkit-scrollbar-thumb {
  background: var(--edge-dim);
  border-radius: 4px;
}
.card::-webkit-scrollbar-thumb:hover {
  background: var(--edge);
}
.card::-webkit-scrollbar-track {
  background: transparent;
}

.close {
  position: absolute;
  top: 9px;
  right: 11px;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 1px solid var(--edge);
  border-radius: 3px;
  background: var(--bg-raised);
  color: var(--gold);
  font-size: 14px;
  line-height: 24px;
  cursor: pointer;
}

.close:hover {
  border-color: var(--edge-hi);
  color: var(--gold-bright);
}

/* Empty state: teach the three hotkeys instead of a bare one-liner. */
.hint {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.hint-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  color: var(--ink-dim);
  font-size: 13.5px;
}

.hint-row kbd {
  flex: none;
  padding: 1px 7px 2px;
  border: 1px solid var(--edge);
  border-radius: 3px;
  background: var(--bg-raised);
  color: var(--ink);
  font: 400 12px/1.5 var(--serif);
  white-space: nowrap;
}

/* Tooltip header plate: the full-bleed darker band PoE2 draws the item name on,
   closed by a gilt separator with a center ornament. Bleeds into the card padding
   (negative margins) so it reads as part of the frame, not a floating heading. */
.head {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: -14px -16px 0;
  padding: 12px 34px 0;
  border-radius: 2px 2px 0 0;
  background: linear-gradient(180deg, #221a10, #14100a 88%);
}

.head::after {
  content: "";
  height: 8px;
  margin: 3px -18px 0;
  padding-bottom: 9px;
  background:
    radial-gradient(circle, var(--gold) 1.5px, rgba(200, 170, 109, 0) 2.5px) center top 4px /
      9px 9px no-repeat,
    linear-gradient(90deg, transparent, rgba(200, 170, 109, 0.5), transparent) center top 8px /
      100% 1px no-repeat;
}

.name {
  font: 400 19px/1.25 var(--smallcaps);
  letter-spacing: 0.02em;
  text-align: center;
  text-wrap: balance;
  color: var(--gold-bright);
}

/* Second header line: the base type under a rare/unique name, PoE2's two-line stack. */
.name-base {
  margin-top: 1px;
  font-size: 15px;
  opacity: 0.9;
}

/* PoE2 rarity colors on the item name (from the echoed rarity base-property). */
.name.r-normal {
  color: #c8c8c8;
}
.name.r-magic {
  color: #8888ff;
}
.name.r-rare {
  color: #ffff77;
}
.name.r-unique {
  color: #af6025;
}
.name.r-currency {
  color: #aa9e82;
}

/* Labeled league control — a bare <select> read as plain text; the caption makes it
   obviously a picker. */
.league-field {
  display: flex;
  flex-direction: column;
  gap: 3px;
  align-self: center;
  max-width: 100%;
}

.field-label {
  font: 400 11px/1.4 var(--smallcaps);
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--ink-dim);
}

/* WebKitGTK renders a native <select>'s value text in the GTK theme colour (dark,
   near-invisible on this panel) and ignores `color` — `appearance: none` is what makes
   it honour our CSS. The caret is drawn on the wrapper (a rotated border square) so it
   never depends on the native widget's own arrow. */
.select-wrap {
  position: relative;
  max-width: 100%;
}

.select-wrap::after {
  content: "";
  position: absolute;
  top: 50%;
  right: 13px;
  width: 7px;
  height: 7px;
  margin-top: -6px;
  border-right: 2px solid var(--gold);
  border-bottom: 2px solid var(--gold);
  transform: rotate(45deg);
  pointer-events: none;
}

.league {
  appearance: none;
  -webkit-appearance: none;
  max-width: 100%;
  padding: 6px 32px 7px 11px;
  border-radius: 3px;
  border: 1px solid var(--edge);
  background: var(--bg-raised);
  color: var(--ink);
  font: 400 14px/1.4 var(--serif);
}

.league:disabled {
  opacity: 0.7;
}

.league option {
  background: var(--bg-raised);
  color: var(--ink);
}

.filters {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 0;
  border-top: 1px solid rgba(200, 170, 109, 0.14);
  border-bottom: 1px solid rgba(200, 170, 109, 0.14);
}

/* Heading so the checkboxes' purpose is self-evident. */
.filters-head {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-bottom: 3px;
}

.filters-title {
  font: 400 12.5px/1.4 var(--smallcaps);
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--gold);
}

.filters-hint {
  font-size: 12.5px;
  color: var(--ink-dim);
}

.row {
  display: flex;
  align-items: center;
  gap: 9px;
}

.row input[type="checkbox"] {
  flex: none;
  width: 16px;
  height: 16px;
  accent-color: var(--edge-hi);
}

/* Field name on a base-property row ("Class", "Base type", …) so a ticked box is
   self-explanatory; fixed width keeps the values aligned in a column. */
.fkind {
  flex: none;
  width: 78px;
  font: 400 11px/1.4 var(--smallcaps);
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--ink-dim);
}

.ftext {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ink);
}

/* Stat lines in the game's magic-mod blue (lightened for 13-15px contrast on the
   near-black panel) — base properties stay parchment, mirroring the real tooltip. */
.stat .ftext {
  color: #a8a8f8;
}

.stat .num {
  flex: none;
  width: 54px;
  padding: 3px 5px;
  border-radius: 3px;
  border: 1px solid var(--edge-dim);
  background: var(--bg-raised);
  color: var(--ink);
  font: 400 13px/1.4 var(--serif);
  text-align: center;
}

.stat .num:focus {
  outline: none;
  border-color: var(--edge-hi);
}

.stat .num::placeholder {
  color: var(--ink-dim);
}

.stat .num:disabled {
  opacity: 0.4;
}

/* Bronze action button — the PoE panel-button idiom, not a flat accent fill. */
.requery {
  align-self: flex-start;
  margin-top: 6px;
  padding: 6px 18px 7px;
  border: 1px solid var(--edge);
  border-radius: 3px;
  background: linear-gradient(180deg, #3a2f1a, #241c0f);
  box-shadow: inset 0 1px 0 rgba(232, 213, 160, 0.14);
  color: var(--gold-bright);
  font: 400 14px/1.4 var(--smallcaps);
  letter-spacing: 0.04em;
  cursor: pointer;
  transition: border-color 0.15s, color 0.15s, box-shadow 0.15s;
}

.requery:hover:not(:disabled) {
  border-color: var(--edge-hi);
  color: #f4e7c3;
  box-shadow:
    inset 0 1px 0 rgba(232, 213, 160, 0.14),
    0 0 8px rgba(200, 170, 109, 0.25);
}

.requery:disabled {
  opacity: 0.55;
  cursor: default;
}

.status {
  color: var(--ink-dim);
}

.status.err {
  color: #ff8f7d;
}

/* Loading pulse: motion conveys the in-flight state, nothing else. */
.status.searching {
  animation: pulse 1.2s ease-in-out infinite alternate;
}

@keyframes pulse {
  from {
    opacity: 1;
  }
  to {
    opacity: 0.45;
  }
}

@media (prefers-reduced-motion: reduce) {
  .status.searching {
    animation: none;
  }
}

/* --- category price sheet (T9) --- */
.sheet-league {
  text-align: center;
  font-size: 12px;
  color: var(--ink-dim);
}

.sheet-toggle {
  padding: 0;
  border: none;
  background: none;
  color: var(--gold);
  font: 400 12px/1.4 var(--serif);
  cursor: pointer;
}

.sheet-toggle:hover {
  color: var(--gold-bright);
}

.sheet-groups {
  display: flex;
  gap: 3px;
  border-bottom: 1px solid rgba(200, 170, 109, 0.14);
}

.sheet-group {
  padding: 4px 12px 6px;
  border: none;
  border-bottom: 2px solid transparent;
  background: none;
  color: var(--ink-dim);
  font: 400 13px/1.4 var(--smallcaps);
  letter-spacing: 0.05em;
  text-transform: uppercase;
  cursor: pointer;
}

.sheet-group:hover:not(:disabled) {
  color: var(--ink);
}

.sheet-group.active {
  color: var(--gold-bright);
  border-bottom-color: var(--gold);
}

.sheet-group:disabled {
  opacity: 0.55;
  cursor: default;
}

.sheet-cats {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.sheet-cat {
  padding: 3px 10px 4px;
  border: 1px solid var(--edge-dim);
  border-radius: 3px;
  background: var(--bg);
  color: var(--ink);
  font: 400 12.5px/1.4 var(--serif);
  cursor: pointer;
}

.sheet-cat:hover:not(:disabled) {
  border-color: var(--edge);
  color: var(--gold-bright);
}

.sheet-cat.active {
  background: linear-gradient(180deg, #3a2f1a, #241c0f);
  border-color: var(--edge-hi);
  color: var(--gold-bright);
}

.sheet-cat:disabled {
  opacity: 0.55;
  cursor: default;
}

.sheet-filter {
  padding: 6px 11px 7px;
  border-radius: 3px;
  border: 1px solid var(--edge-dim);
  background: var(--bg-raised);
  color: var(--ink);
  font: 400 14px/1.4 var(--serif);
}

.sheet-filter:focus {
  outline: none;
  border-color: var(--edge-hi);
}

.sheet-filter::placeholder {
  color: var(--ink-dim);
}

.sheet-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ink);
}

.spread {
  font-size: 12.5px;
  color: var(--gold);
}

.spread.stale {
  opacity: 0.45;
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
  padding: 5px 0;
  border-bottom: 1px solid rgba(200, 170, 109, 0.12);
}

.price {
  font: 700 14px/1.4 var(--serif);
  color: var(--gold-bright);
}

/* The verdict the user came for: the cheapest listing reads first, slightly louder. */
.listing:first-child .price {
  font-size: 16px;
  color: #f4e7c3;
}

.age {
  font-size: 12px;
  color: var(--ink-dim);
}

.status.safe {
  color: #8fe3a0;
}

/* --- waystone danger panel (T7) --- */
/* Severity as colored text on a dark plate (the game's map-mod idiom), not a filled
   candy pill; the label text carries the meaning, color reinforces it. */
.level {
  align-self: center;
  padding: 2px 12px 3px;
  border: 1px solid;
  border-radius: 3px;
  background: rgba(0, 0, 0, 0.3);
  font: 400 12px/1.5 var(--smallcaps);
  letter-spacing: 0.1em;
  text-transform: uppercase;
}
.level.safe {
  color: #8fe3a0;
  border-color: rgba(143, 227, 160, 0.5);
}
.level.caution {
  color: #e8c98a;
  border-color: rgba(232, 201, 138, 0.5);
}
.level.dangerous {
  color: #ff9d5c;
  border-color: rgba(255, 157, 92, 0.5);
}
.level.deadly {
  color: #ff6b6b;
  border-color: rgba(255, 107, 107, 0.55);
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
  border-bottom: 1px solid rgba(200, 170, 109, 0.12);
}

.flag .dot {
  flex: none;
  width: 8px;
  height: 8px;
  margin-top: 6px;
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
  font-weight: 700;
  color: var(--ink);
}

.fwhy {
  font-size: 12.5px;
  color: var(--ink-dim);
}

.fmod {
  margin-top: 2px;
  font-style: italic;
  font-size: 12px;
  color: #8888ff;
}

/* --- regex cheat-sheet (T8) --- */
.rcat {
  margin-bottom: 8px;
}

.rcat-name {
  margin: 6px 0 3px;
  font: 400 11.5px/1.4 var(--smallcaps);
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--ink-dim);
}

.rrow {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px 10px;
  width: 100%;
  padding: 5px 8px;
  border: 1px solid transparent;
  border-radius: 3px;
  background: rgba(200, 170, 109, 0.05);
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
  margin-bottom: 3px;
}

.rrow:hover {
  border-color: var(--edge-dim);
  background: rgba(200, 170, 109, 0.1);
}

.rlabel {
  color: var(--ink);
}

.rregex {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font: 400 12px/1.4 "JetBrains Mono", ui-monospace, monospace;
  color: var(--gold);
}

.rnote {
  flex-basis: 100%;
  font-size: 11.5px;
  color: var(--ink-dim);
}

.rcopied {
  position: absolute;
  top: 5px;
  right: 8px;
  font-size: 11.5px;
  color: #8fe3a0;
  opacity: 0;
  transition: opacity 0.1s;
}

.rcopied.show {
  opacity: 1;
}

.rfooter {
  margin-top: 6px;
  font-size: 11.5px;
  color: var(--ink-dim);
}
</style>
