import { invoke } from "@tauri-apps/api/core";

/** 与 src-tauri/src/registry.rs 的 MenuEntry 对应 */
interface MenuEntry {
  name: string;
  kind: "verb" | "shellex" | "custom" | "packaged";
  mount: string;
  reg_path: string;
  command: string;
  source: string;
  icon: string | null;
  clsid: string;
  enabled: boolean;
}

interface SnapshotMeta {
  id: string;
  when: number;
  kind: string;
  action: string;
  keys: number;
  protected: boolean;
  reg_file: string | null;
}

interface RestoreReport {
  written: number;
  notes: string[];
  undo_id: string | null;
}

interface SettingsInfo {
  version: string;
  data_dir: string;
  data_dir_portable: boolean;
  auto_keep: number;
  policy_menu_disabled: boolean;
  policy_sources: string[];
}

/** 后端不可达时的兜底样例（如单独 `pnpm dev` 在纯浏览器预览前端） */
const MOCK: MenuEntry[] = [
  { name: "7-Zip 压缩 / 解压", kind: "shellex", mount: "*", reg_path: "HKCR\\*\\shellex\\ContextMenuHandlers\\7-Zip", command: "C:\\Program Files\\7-Zip\\7-zip.dll", source: "7-Zip", icon: null, clsid: "{23170F69-40C1-278A-1000-000100020000}", enabled: true },
  { name: "用 Code 打开", kind: "verb", mount: "*", reg_path: "HKCR\\*\\shell\\VSCode", command: '"C:\\Program Files\\Microsoft VS Code\\Code.exe" "%1"', source: "Code", icon: null, clsid: "", enabled: true },
  { name: "在终端中打开", kind: "verb", mount: "Directory", reg_path: "HKCR\\Directory\\shell\\OpenTerminal", command: 'wt.exe -d "%V"', source: "wt", icon: null, clsid: "", enabled: true },
  { name: "用 Code 打开文件夹", kind: "custom", mount: "Directory", reg_path: "HKCU\\Software\\Classes\\Directory\\shell\\RCM_OpenWithCode", command: '"C:\\Program Files\\Microsoft VS Code\\Code.exe" "%V"', source: "Code", icon: null, clsid: "", enabled: true },
];

const KIND_LABEL: Record<MenuEntry["kind"], string> = {
  verb: "静态动词",
  shellex: "shellex 处理器",
  custom: "自定义",
  packaged: "打包 (MSIX)",
};
const KIND_ICON: Record<MenuEntry["kind"], string> = {
  verb: "📄",
  shellex: "🧩",
  custom: "⭐",
  packaged: "📦",
};
/** D5 分层标注：条目出现在哪一层菜单 */
function layerOf(e: MenuEntry): string {
  if (e.kind === "shellex") return "仅经典菜单";
  if (e.kind === "packaged") return "仅新版菜单";
  return "新版 + 经典菜单";
}
const SNAP_ICON: Record<string, string> = {
  disable: "⏸",
  enable: "▶",
  delete: "🗑",
  create: "✨",
  update: "✏",
  classic: "🔁",
  manual: "📷",
  "pre-restore": "⏱",
};
const SNAP_KIND_CN: Record<string, string> = {
  disable: "禁用",
  enable: "启用",
  delete: "删除",
  create: "新增",
  update: "更新",
  classic: "经典菜单",
  manual: "手动快照",
  "pre-restore": "还原前留档",
};

/** 用户场景（导航两大分类）：桌面/文件夹背景 → 桌面右键，其余 → 文件右键 */
type Scene = "file" | "desktop";
function sceneOf(mount: string): Scene {
  const s = mount.toLowerCase();
  return s.includes("background") || s.includes("desktop") ? "desktop" : "file";
}
const SCENE_LABEL: Record<Scene, string> = { file: "文件右键", desktop: "桌面右键" };
const SCENE_ICON: Record<Scene, string> = { file: "📄", desktop: "🖥️" };
const SCENE_TITLE: Record<Scene, string> = { file: "文件右键菜单", desktop: "桌面右键菜单" };

// ===== 状态 =====
let entries: MenuEntry[] = [];
let live = false;
let filter: Scene | "全部" = "全部";
let q = "";
let expanded: string | null = null;
let animateRows = false;
let pendingRestart = false;
let confirmDelKey: string | null = null;
/** 视图：条目管理 / 快照历史 / 设置 */
let view: "entries" | "history" | "settings" = "entries";
let snapshots: SnapshotMeta[] = [];
let confirmRestoreId: string | null = null;
let lastRestore: { written: number; notes: string[]; undoId: string | null } | null = null;
let settings: SettingsInfo | null = null;
let confirmClear: "" | "auto" | "all" = "";
/** 自定义条目模态 */
let modalMode: "create" | "edit" = "create";
let modalScene: Scene = "file";
let editEntry: MenuEntry | null = null;
/** 经典菜单开关（以注册表键为准） */
let classicOn = false;

// ===== 元素（静态骨架，查询一次；杜绝全量重绘） =====
const navEl = document.querySelector<HTMLElement>("#nav-items")!;
const listEl = document.querySelector<HTMLElement>("#list")!;
const panelEl = document.querySelector<HTMLElement>(".panel")!;
const categoryTitleEl = document.querySelector<HTMLElement>("#category-title")!;
const statusEl = document.querySelector<HTMLElement>("#status")!;
const statusDotEl = document.querySelector<HTMLElement>("#status-dot")!;
const footerCountEl = document.querySelector<HTMLElement>("#footer-count")!;
const searchInput = document.querySelector<HTMLInputElement>("#q")!;
const rescanBtn = document.querySelector<HTMLButtonElement>("#rescan")!;
const pendingEl = document.querySelector<HTMLButtonElement>("#pending")!;
const toastsEl = document.querySelector<HTMLElement>("#toasts")!;
const addNewBtn = document.querySelector<HTMLButtonElement>("#add-new")!;
const snapManualBtn = document.querySelector<HTMLButtonElement>("#snap-manual")!;
const modalEl = document.querySelector<HTMLElement>("#modal")!;
const modalTitleEl = document.querySelector<HTMLElement>("#modal-title")!;
const fNameInput = document.querySelector<HTMLInputElement>("#f-name")!;
const fCmdInput = document.querySelector<HTMLInputElement>("#f-cmd")!;
const fIconInput = document.querySelector<HTMLInputElement>("#f-icon")!;
const fIconPick = document.querySelector<HTMLButtonElement>("#f-icon-pick")!;
const fPreviewEl = document.querySelector<HTMLElement>("#f-preview")!;
const fSceneEl = document.querySelector<HTMLElement>("#f-scene")!;
const modalCancelBtn = document.querySelector<HTMLButtonElement>("#modal-cancel")!;
const modalSaveBtn = document.querySelector<HTMLButtonElement>("#modal-save")!;
const classicToggleEl = document.querySelector<HTMLInputElement>("#classic-toggle")!;
let policyHit = false;

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/"/g, "&quot;");
}

/** 分类 toast（T9：成功/警告/错误，不打断操作流） */
function toast(msg: string, kind: "ok" | "warn" | "err" = "ok"): void {
  const el = document.createElement("div");
  el.className = `toast ${kind}`;
  el.textContent = msg;
  toastsEl.append(el);
  window.setTimeout(() => {
    el.style.opacity = "0";
    window.setTimeout(() => el.remove(), 300);
  }, 3600);
}

function renderPending(): void {
  pendingEl.hidden = !pendingRestart;
}

async function load(): Promise<void> {
  try {
    entries = await invoke<MenuEntry[]>("list_menu_entries");
    live = true;
  } catch {
    entries = MOCK;
    live = false;
  }
  try {
    classicOn = await invoke<boolean>("classic_menu_state");
  } catch {
    classicOn = false;
  }
  classicToggleEl.checked = classicOn;
  try {
    settings = await invoke<SettingsInfo>("get_settings");
    policyHit = settings.policy_menu_disabled;
  } catch {
    policyHit = false;
  }
  expanded = null;
  confirmDelKey = null;
  animateRows = true;
  await loadSnapshots();
  renderStatus();
  renderNav();
  renderView();
}

async function loadSnapshots(): Promise<void> {
  try {
    snapshots = await invoke<SnapshotMeta[]>("list_snapshots");
  } catch {
    snapshots = [];
  }
}

function renderStatus(): void {
  if (policyHit) {
    statusEl.textContent = "⚠ 系统策略已禁用右键菜单（详见设置页）";
    statusDotEl.classList.remove("live");
    return;
  }
  statusEl.textContent = live ? "系统状态: 已连接注册表" : "系统状态: 示例数据（后端未达）";
  statusDotEl.classList.toggle("live", live);
}

/** 底部计数：筛选/搜索时显示「可见 / 总数」，未筛选时显示「共 N」 */
function renderFooterCount(n: number, unit = "个条目"): void {
  const total = entries.length;
  footerCountEl.textContent = n === total ? `共 ${total} ${unit}` : `${n} / ${total} ${unit}`;
}

function visible(): MenuEntry[] {
  return entries.filter(
    (e) =>
      (filter === "全部" || sceneOf(e.mount) === filter) &&
      (!q || e.name.toLowerCase().includes(q.toLowerCase())),
  );
}

function renderNav(): void {
  const countOf = (sc: Scene) => entries.filter((e) => sceneOf(e.mount) === sc).length;
  const item = (attrs: string, icon: string, label: string, cnt: number | string, active: boolean) => `
    <div class="nav-item ${active ? "active" : ""}" ${attrs} role="button" tabindex="0">
      <span aria-hidden="true">${icon}</span><span>${label}</span><span class="cnt">${cnt}</span>
    </div>`;
  navEl.innerHTML =
    item(`data-v="全部"`, "🌐", "全部", entries.length, view === "entries" && filter === "全部") +
    item(`data-v="file"`, SCENE_ICON.file, SCENE_LABEL.file, countOf("file"), view === "entries" && filter === "file") +
    item(`data-v="desktop"`, SCENE_ICON.desktop, SCENE_LABEL.desktop, countOf("desktop"), view === "entries" && filter === "desktop") +
    `<div class="nav-h">维护</div>` +
    item(`data-view="history"`, "🕘", "快照历史", snapshots.length, view === "history") +
    item(`data-view="settings"`, "⚙", "设置", "", view === "settings");
}

function setView(v: "entries" | "history" | "settings"): void {
  view = v;
  confirmClear = "";
  document.body.dataset.view = v;
  renderNav();
  renderView();
}

function renderView(): void {
  snapManualBtn.hidden = view !== "history";
  addNewBtn.hidden = view !== "entries";
  searchInput.closest<HTMLElement>(".search-box")!.hidden = view !== "entries";
  if (view === "history") {
    renderHistory();
  } else if (view === "settings") {
    renderSettings();
  } else {
    renderList();
  }
}

/* ================= 条目视图 ================= */

function renderList(): void {
  const list = visible();
  categoryTitleEl.textContent = filter === "全部" ? "全部条目" : SCENE_TITLE[filter];
  renderFooterCount(list.length);

  listEl.innerHTML = list
    .map((e, i) => {
      const stagger = animateRows ? ` style="animation-delay:${Math.min(i, 12) * 20}ms"` : "";
      const open = expanded === e.reg_path;
      const sc = sceneOf(e.mount);
      const cmd = e.command ? esc(e.command) : "（无独立命令，由系统实现）";
      return `
      <div class="menu-card clickable ${open ? "open" : ""}" data-key="${esc(e.reg_path)}" role="button" aria-expanded="${open}"${stagger}>
        <div class="card-main">
          <div class="item-info">
            <div class="item-icon">${e.icon
              ? `<img class="item-img" src="${e.icon}" alt="" draggable="false" />`
              : `<span aria-hidden="true">${KIND_ICON[e.kind]}</span>`}</div>
            <div class="item-details">
              <div class="item-name"><span class="nm">${esc(e.name)}</span><span class="badge ${e.kind === "verb" || e.kind === "custom" ? "acc" : ""}">${KIND_LABEL[e.kind]}</span></div>
              <div class="chips">
                <span class="chip sc-${sc}">${SCENE_ICON[sc]} ${SCENE_LABEL[sc]}</span>
                <span class="chip" title="来源应用（自动推导）">来源 · ${esc(e.source)}</span>
                <span class="chip" title="出现在哪一层菜单">📍 ${layerOf(e)}</span>
              </div>
            </div>
          </div>
          <div class="item-side">${renderSide(e)}</div>
        </div>
        <div class="card-more">
          <div class="card-more-inner">
            <div class="card-more-content mono">
              <div>注册表位置&nbsp;&nbsp;${esc(e.reg_path)}</div>
              <div style="margin-top:6px">命令行&nbsp;&nbsp;${cmd}</div>
            </div>
          </div>
        </div>
      </div>`;
    })
    .join("");
  if (!list.length) listEl.innerHTML = `<div class="empty">没有匹配的条目</div>`;
}

/** 条目右侧操作区：弹簧开关（真实操作）+ 两段式删除 + 展开 chevron */
function renderSide(e: MenuEntry): string {
  if (confirmDelKey === e.reg_path) {
    return `
      <span class="confirm-hint">删除？已自动备份</span>
      <button class="btn tiny danger" data-confirm-del="${esc(e.reg_path)}">确认</button>
      <button class="btn tiny" data-cancel-del="${esc(e.reg_path)}">取消</button>`;
  }
  const editBtn = e.kind === "custom"
    ? `<button class="btn-del" data-edit="${esc(e.reg_path)}" title="编辑（自定义条目）">✎</button>`
    : "";
  const delBtn = e.kind === "packaged"
    ? ""
    : `<button class="btn-del" data-del="${esc(e.reg_path)}" title="删除（先自动导出 .reg 备份）">🗑</button>`;
  return `
    <label class="switch" title="${e.enabled ? "禁用（可逆，写值不删键）" : "启用（移除禁用标记）"}">
      <input type="checkbox" data-toggle data-key="${esc(e.reg_path)}" ${e.enabled ? "checked" : ""} />
      <span class="slider"></span>
    </label>
    ${editBtn}
    ${delBtn}
    <span class="chevron" aria-hidden="true">›</span>`;
}

function toggleCard(cardEl: HTMLElement): void {
  const key = cardEl.dataset.key ?? "";
  expanded = expanded === key ? null : key;
  listEl.querySelectorAll<HTMLElement>(".menu-card").forEach((c) => {
    const on = c.dataset.key === expanded;
    c.classList.toggle("open", on);
    c.setAttribute("aria-expanded", String(on));
  });
}

function refreshSide(key: string): void {
  const entry = entries.find((x) => x.reg_path === key);
  const side = listEl.querySelector<HTMLElement>(`.menu-card[data-key="${CSS.escape(key)}"] .item-side`);
  if (entry && side) side.innerHTML = renderSide(entry);
}

/* ================= 快照历史视图 ================= */

function fmtTime(when: number): string {
  const d = new Date(when);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${d.getMonth() + 1}月${d.getDate()}日 ${hh}:${mm}`;
}

function renderHistory(): void {
  categoryTitleEl.textContent = "快照历史";
  footerCountEl.textContent = `共 ${snapshots.length} 个快照`;

  const banner = lastRestore
    ? `<div class="restore-banner">
        <div>
          <b>已写回 ${lastRestore.written} 个键</b>
          ${lastRestore.notes.length ? `<div class="restore-notes">${lastRestore.notes.map((n) => esc(n)).join("<br>")}</div>` : ""}
        </div>
        <div style="display:flex;gap:6px;flex:none">
          ${lastRestore.undoId ? `<button class="btn tiny" data-undo="${esc(lastRestore.undoId)}">撤销本次还原</button>` : ""}
          <button class="btn tiny" data-dismiss-restore>知道了</button>
        </div>
      </div>`
    : "";

  const cards = snapshots
    .map((s, i) => {
      const stagger = animateRows ? ` style="animation-delay:${Math.min(i, 12) * 20}ms"` : "";
      const confirming = confirmRestoreId === s.id;
      return `
      <div class="menu-card snap-card" data-snap="${esc(s.id)}"${stagger}>
        <div class="card-main">
          <div class="item-info">
            <div class="item-icon" aria-hidden="true">${SNAP_ICON[s.kind] ?? "🗒"}</div>
            <div class="item-details">
              <div class="item-name"><span class="nm">${esc(s.action)}</span><span class="badge">${SNAP_KIND_CN[s.kind] ?? s.kind}</span></div>
              <div class="chips">
                <span class="chip">${fmtTime(s.when)}</span>
                <span class="chip">${s.keys} 个键</span>
                ${s.protected ? `<span class="chip">永不自动清理</span>` : ""}
                ${s.reg_file ? `<span class="chip">附 .reg 备份</span>` : ""}
              </div>
            </div>
          </div>
          <div class="item-side">
            ${confirming
              ? `<span class="confirm-hint">确认还原？</span>
                 <button class="btn tiny danger" data-confirm-restore="${esc(s.id)}">确认</button>
                 <button class="btn tiny" data-cancel-restore>取消</button>`
              : `<button class="btn tiny" data-restore="${esc(s.id)}">还原至此</button>`}
          </div>
        </div>
      </div>`;
    })
    .join("");

  listEl.innerHTML =
    banner +
    (cards || `<div class="empty">还没有快照——禁用、删除等操作会自动留档</div>`);
}

/* ================= 设置视图（T15） ================= */

function renderSettings(): void {
  categoryTitleEl.textContent = "设置";
  footerCountEl.textContent = "";
  const s = settings;
  if (!s) {
    listEl.innerHTML = `<div class="empty">设置加载失败（后端未达）</div>`;
    return;
  }
  listEl.innerHTML = `
    <div class="menu-card set-card">
      <h3>🕘 快照保留</h3>
      <p>自动记录保留最近 <input id="set-keep" class="keep-input" type="number" min="5" max="500" value="${s.auto_keep}" /> 条；<b>删除类快照与手动快照点永不自动清理</b>。</p>
      <div class="row-actions">
        <button class="btn tiny" data-set-keep>保存</button>
        <button class="btn tiny danger ${confirmClear === "auto" ? "confirming" : ""}" data-clear="auto">${confirmClear === "auto" ? "确认清空？" : "清空自动记录"}</button>
        <button class="btn tiny danger ${confirmClear === "all" ? "confirming" : ""}" data-clear="all">${confirmClear === "all" ? "确认全部清空？" : "清空全部（含手动点与备份）"}</button>
      </div>
    </div>
    <div class="menu-card set-card">
      <h3>📁 数据目录</h3>
      <p class="mono">${esc(s.data_dir)}</p>
      <div class="row-actions">
        <span class="badge ${s.data_dir_portable ? "acc" : ""}">${s.data_dir_portable ? "便携模式（数据随程序走）" : "回退模式（%APPDATA%）"}</span>
        <button class="btn tiny" data-open-dir>打开目录</button>
      </div>
    </div>
    <div class="menu-card set-card">
      <h3>🛡 系统策略</h3>
      ${s.policy_menu_disabled
        ? `<p style="color:var(--danger)">⚠ 检测到组策略禁用了右键菜单：${esc(s.policy_sources.join("、"))}<br>请先在组策略中恢复，本程序管理的条目才会显示在系统菜单里。</p>`
        : `<p>✓ 未检测到限制右键菜单的策略。</p>`}
    </div>
    <div class="menu-card set-card">
      <h3>💬 关于</h3>
      <p>右键菜单管家 v${esc(s.version)} · Tauri v2 + 原生 TypeScript<br>
      规划与全部决策记录见仓库 <span class="mono">wayfinder/map.md</span>。</p>
    </div>`;
}

async function doRestore(id: string): Promise<void> {
  try {
    const rep = await invoke<RestoreReport>("restore_snapshot", { id });
    confirmRestoreId = null;
    lastRestore = { written: rep.written, notes: rep.notes, undoId: rep.undo_id };
    pendingRestart = true;
    renderPending();
    await loadSnapshots();
    if (view === "history") renderHistory(); else renderList();
    toast(`已写回 ${rep.written} 个键 · 重启资源管理器后生效`, "ok");
  } catch (err) {
    confirmRestoreId = null;
    renderHistory();
    toast(`还原失败：${String(err)}`, "err");
  }
}

// ===== 事件（委托，只挂一次）=====

navEl.addEventListener("click", (ev) => {
  const it = (ev.target as HTMLElement).closest<HTMLElement>(".nav-item");
  if (!it) return;
  if (it.dataset.view === "history" || it.dataset.view === "settings") {
    setView(it.dataset.view as "history" | "settings");
    return;
  }
  filter = (it.dataset.v as Scene | "全部") ?? "全部";
  setView("entries");
});
navEl.addEventListener("keydown", (ev) => {
  if (ev.key !== "Enter" && ev.key !== " ") return;
  const it = (ev.target as HTMLElement).closest<HTMLElement>(".nav-item");
  if (it) it.click();
});

// 条目视图事件
listEl.addEventListener("pointerdown", (ev) => {
  if (view !== "entries") return;
  const t = ev.target as HTMLElement;
  if (t.closest(".switch, [data-del], [data-edit], [data-confirm-del], [data-cancel-del]")) return;
  const card = t.closest<HTMLElement>(".menu-card.clickable");
  if (card) toggleCard(card);
});

listEl.addEventListener("change", async (ev) => {
  if (view !== "entries") return;
  const input = ev.target as HTMLInputElement;
  if (!input.matches("[data-toggle]")) return;
  const key = input.dataset.key ?? "";
  const entry = entries.find((x) => x.reg_path === key);
  if (!entry) return;
  const on = input.checked;
  try {
    await invoke("set_entry_enabled", {
      regPath: entry.reg_path,
      kind: entry.kind,
      clsid: entry.clsid,
      enabled: on,
      name: entry.name,
    });
    entry.enabled = on;
    pendingRestart = true;
    renderPending();
    toast(`已${on ? "启用" : "禁用"}「${entry.name}」· 一般立即生效；若菜单未变化，点击左侧徽章重启资源管理器`, "ok");
  } catch (err) {
    input.checked = !on; // 回滚开关
    toast(`操作失败：${String(err)}`, "err");
  }
});

listEl.addEventListener("click", async (ev) => {
  const t = ev.target as HTMLElement;

  // 设置视图
  if (view === "settings") {
    if (t.closest("[data-set-keep]")) {
      const input = document.querySelector<HTMLInputElement>("#set-keep");
      const n = Math.max(5, Math.min(500, Number(input?.value ?? 60) || 60));
      try {
        await invoke("set_auto_keep", { n });
        if (settings) settings.auto_keep = n;
        toast(`保留策略已保存：自动记录保留最近 ${n} 条`, "ok");
      } catch (err) {
        toast(`保存失败：${String(err)}`, "err");
      }
      return;
    }
    const clr = t.closest<HTMLElement>("[data-clear]");
    if (clr) {
      const mode = (clr.dataset.clear as "auto" | "all") ?? "auto";
      if (confirmClear !== mode) {
        confirmClear = mode;
        renderSettings();
        return;
      }
      try {
        const removed = await invoke<number>("clear_snapshots", { includeProtected: mode === "all" });
        confirmClear = "";
        await loadSnapshots();
        renderSettings();
        toast(`已清空 ${removed} 条快照记录`, "ok");
      } catch (err) {
        confirmClear = "";
        renderSettings();
        toast(`清空失败：${String(err)}`, "err");
      }
      return;
    }
    if (t.closest("[data-open-dir]")) {
      try {
        await invoke("open_data_dir");
      } catch (err) {
        toast(`打开目录失败：${String(err)}`, "err");
      }
      return;
    }
    return;
  }

  // 快照历史视图
  if (view === "history") {
    const restore = t.closest<HTMLElement>("[data-restore]");
    if (restore) {
      confirmRestoreId = restore.dataset.restore ?? null;
      renderHistory();
      return;
    }
    const confirmR = t.closest<HTMLElement>("[data-confirm-restore]");
    if (confirmR) {
      await doRestore(confirmR.dataset.confirmRestore!);
      return;
    }
    if (t.closest("[data-cancel-restore]")) {
      confirmRestoreId = null;
      renderHistory();
      return;
    }
    const undo = t.closest<HTMLElement>("[data-undo]");
    if (undo) {
      const id = undo.dataset.undo!;
      lastRestore = null;
      await doRestore(id);
      toast("已撤销本次还原", "ok");
      return;
    }
    if (t.closest("[data-dismiss-restore]")) {
      lastRestore = null;
      renderHistory();
      return;
    }
    return;
  }

  // 条目视图：编辑 + 两段式删除确认
  const editBtn = t.closest<HTMLElement>("[data-edit]");
  if (editBtn) {
    const entry = entries.find((x) => x.reg_path === editBtn.dataset.edit);
    if (entry) openModal("edit", entry);
    return;
  }
  const del = t.closest<HTMLElement>("[data-del]");
  if (del) {
    const key = del.dataset.del ?? "";
    if (confirmDelKey === key) {
      const entry = entries.find((x) => x.reg_path === key);
      try {
        await invoke("delete_entry", { regPath: key, kind: entry?.kind ?? "verb", name: entry?.name ?? key });
        entries = entries.filter((x) => x.reg_path !== key);
        if (expanded === key) expanded = null;
        confirmDelKey = null;
        pendingRestart = true;
        renderPending();
        renderList();
        toast(`已删除「${entry?.name ?? key}」· 已导出 .reg 备份，可在快照历史还原；一般立即生效`, "ok");
      } catch (err) {
        confirmDelKey = null;
        refreshSide(key);
        toast(`删除失败：${String(err)}`, "err");
      }
    } else {
      confirmDelKey = key;
      refreshSide(key);
    }
    return;
  }
  const cancel = t.closest<HTMLElement>("[data-cancel-del]");
  if (cancel) {
    confirmDelKey = null;
    refreshSide(cancel.dataset.cancelDel ?? "");
  }
});

searchInput.addEventListener("input", () => {
  q = searchInput.value;
  renderList(); // 输入框是静态元素：不重建、不失焦
});

rescanBtn.addEventListener("click", () => void load());

snapManualBtn.addEventListener("click", async () => {
  try {
    await invoke("create_manual_snapshot");
    await loadSnapshots();
    renderHistory();
    toast("已创建手动快照点（永不自动清理）", "ok");
  } catch (err) {
    toast(`手动快照失败：${String(err)}`, "err");
  }
});

pendingEl.addEventListener("click", async () => {
  try {
    await invoke("restart_explorer");
    pendingRestart = false;
    renderPending();
    toast("资源管理器已重启，改动已生效", "ok");
  } catch (err) {
    toast(`重启失败：${String(err)}`, "err");
  }
});

// 经典菜单开关（T13：以键为准、写前留档、待生效徽章）
classicToggleEl.addEventListener("change", async () => {
  const on = classicToggleEl.checked;
  try {
    await invoke("set_classic_menu", { on });
    classicOn = on;
    pendingRestart = true;
    renderPending();
    toast(
      on
        ? "已切换为经典完整菜单 · 重启资源管理器后生效"
        : "已恢复 Win11 新版菜单 · 重启资源管理器后生效",
      "ok",
    );
  } catch (err) {
    classicToggleEl.checked = !on; // 回滚开关
    toast(`切换失败：${String(err)}`, "err");
  }
});

/* ===== 自定义条目模态（新增 / 编辑） ===== */

function openModal(mode: "create" | "edit", entry?: MenuEntry): void {
  modalMode = mode;
  editEntry = entry ?? null;
  modalTitleEl.textContent = mode === "create" ? "新增菜单项" : "编辑菜单项";
  fNameInput.value = entry?.name ?? "";
  fCmdInput.value = entry?.command ?? "";
  fIconInput.value = ""; // 编辑时留空 = 保持现有图标
  fPreviewEl.innerHTML = entry?.icon ? `<img class="item-img" src="${entry.icon}" alt="" />` : "";
  modalScene = entry ? sceneOf(entry.mount) : "file";
  fSceneEl.querySelectorAll<HTMLElement>(".seg-item").forEach((s) => {
    s.classList.toggle("sel", s.dataset.v === modalScene);
  });
  modalEl.hidden = false;
  fNameInput.focus();
}

function closeModal(): void {
  modalEl.hidden = true;
  editEntry = null;
}

async function updateIconPreview(): Promise<void> {
  const p = fIconInput.value.trim();
  if (!p) {
    fPreviewEl.innerHTML = "";
    return;
  }
  fPreviewEl.textContent = "…";
  try {
    const icon = await invoke<string | null>("extract_icon_preview", { path: p });
    fPreviewEl.innerHTML = icon ? `<img class="item-img" src="${icon}" alt="" />` : "❔";
  } catch {
    fPreviewEl.textContent = "❔";
  }
}

async function saveModal(): Promise<void> {
  const name = fNameInput.value.trim();
  const command = fCmdInput.value.trim();
  const icon = fIconInput.value.trim();
  if (!name || !command) {
    toast("名称与命令行为必填项", "warn");
    return;
  }
  try {
    if (modalMode === "create") {
      await invoke("create_custom_entry", { name, command, icon, scene: modalScene });
    } else if (editEntry) {
      await invoke("update_custom_entry", {
        regPath: editEntry.reg_path,
        name,
        command,
        icon,
        scene: modalScene,
      });
    }
    closeModal();
    pendingRestart = true;
    renderPending();
    await load();
    toast(`已${modalMode === "create" ? "新增" : "更新"}「${name}」· 一般立即生效；若菜单未变化，点击左侧徽章重启资源管理器`, "ok");
  } catch (err) {
    toast(`保存失败：${String(err)}`, "err");
  }
}

addNewBtn.addEventListener("click", () => openModal("create"));
modalCancelBtn.addEventListener("click", closeModal);
modalSaveBtn.addEventListener("click", () => void saveModal());
modalEl.addEventListener("click", (ev) => {
  if (ev.target === modalEl) closeModal(); // 点遮罩关闭
});
fIconPick.addEventListener("click", async () => {
  try {
    const p = await invoke<string | null>("pick_icon_file");
    if (p) {
      fIconInput.value = p;
      await updateIconPreview();
    }
  } catch (err) {
    toast(`选择文件失败：${String(err)}`, "err");
  }
});
fIconInput.addEventListener("change", () => void updateIconPreview());
fSceneEl.addEventListener("click", (ev) => {
  const it = (ev.target as HTMLElement).closest<HTMLElement>(".seg-item");
  if (!it) return;
  modalScene = (it.dataset.v as Scene) ?? "file";
  fSceneEl.querySelectorAll<HTMLElement>(".seg-item").forEach((s) => {
    s.classList.toggle("sel", s === it);
  });
});
document.addEventListener("keydown", (ev) => {
  if (ev.key === "Escape" && !modalEl.hidden) closeModal();
});

// 滚动条浮现：滚动中打 .scrolling，停止约 0.7s 后淡出
let scrollIdleTimer: number | undefined;
panelEl.addEventListener("scroll", () => {
  panelEl.classList.add("scrolling");
  window.clearTimeout(scrollIdleTimer);
  scrollIdleTimer = window.setTimeout(() => panelEl.classList.remove("scrolling"), 700);
}, { passive: true });

void load();
