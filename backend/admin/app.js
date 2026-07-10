const state = {
  view: location.hash.slice(1) || "overview",
  records: [],
  editingId: null,
  query: "",
};

const titles = {
  overview: "运营总览",
  partners: "合作方管理",
  songs: "曲库管理",
  plans: "推广方案",
  users: "用户管理",
  conversations: "合作会话",
  settlements: "结算记录",
};

const editableViews = new Set(["partners", "songs", "plans"]);
const content = document.querySelector("#content");
const loginLayer = document.querySelector("#loginLayer");
const modalLayer = document.querySelector("#modalLayer");
const confirmLayer = document.querySelector("#confirmLayer");
const createButton = document.querySelector("#createButton");
const serverStatus = document.querySelector("#serverStatus");
const validViews = new Set(Object.keys(titles));
let searchTimer;

if (!validViews.has(state.view)) state.view = "overview";

document.querySelector("#nav").addEventListener("click", (event) => {
  const button = event.target.closest("[data-view]");
  if (!button) return;
  switchView(button.dataset.view);
});

document.querySelector("#loginForm").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = new FormData(event.currentTarget);
  const error = document.querySelector("#loginError");
  const button = document.querySelector("#loginButton");
  error.textContent = "";
  setButtonBusy(button, true, "正在登录");
  try {
    await api("/api/admin/auth/login", {
      method: "POST",
      body: {
        username: form.get("username"),
        password: form.get("password"),
      },
    });
    loginLayer.classList.add("hidden");
    event.currentTarget.reset();
    await loadView();
  } catch (requestError) {
    error.textContent = requestError.message;
  } finally {
    setButtonBusy(button, false);
  }
});

document.querySelector("#logoutButton").addEventListener("click", async () => {
  const button = document.querySelector("#logoutButton");
  setButtonBusy(button, true, "正在退出");
  await api("/api/admin/auth/logout", { method: "POST" }).catch(() => {});
  loginLayer.classList.remove("hidden");
  setButtonBusy(button, false);
});

createButton.addEventListener("click", () => openModal());
document.querySelector("#closeModal").addEventListener("click", closeModal);
document.querySelector("#cancelModal").addEventListener("click", closeModal);
document.querySelector("#entityForm").addEventListener("submit", saveEntity);

content.addEventListener("click", async (event) => {
  const edit = event.target.closest("[data-edit]");
  const remove = event.target.closest("[data-delete]");
  const retry = event.target.closest("[data-retry]");
  const create = event.target.closest("[data-create]");
  if (retry) {
    await loadView();
    return;
  }
  if (create) {
    openModal();
    return;
  }
  if (edit) {
    const record = state.records.find((item) => item.id === edit.dataset.edit);
    openModal(record);
  }
  if (remove) {
    const accepted = await confirmAction(
      "确认停用这条记录？",
      "停用后将不再对小程序用户展示，但历史业务数据会保留。",
    );
    if (!accepted) return;
    setButtonBusy(remove, true, "处理中");
    try {
      await api(`/api/admin/${state.view}/${remove.dataset.delete}`, { method: "DELETE" });
      toast("记录已停用");
      await loadView();
    } catch (error) {
      toast(error.message);
      setButtonBusy(remove, false);
    }
  }
});

content.addEventListener("input", (event) => {
  const input = event.target.closest("[data-search]");
  if (!input) return;
  state.query = input.value;
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    renderTable(state.view, state.records);
    const nextInput = content.querySelector("[data-search]");
    nextInput?.focus();
    nextInput?.setSelectionRange(state.query.length, state.query.length);
  }, 180);
});

modalLayer.addEventListener("click", (event) => {
  if (event.target === modalLayer) closeModal();
});

confirmLayer.addEventListener("click", (event) => {
  if (event.target === confirmLayer) document.querySelector("#confirmCancel").click();
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  if (!confirmLayer.classList.contains("hidden")) {
    document.querySelector("#confirmCancel").click();
  } else if (!modalLayer.classList.contains("hidden")) {
    closeModal();
  }
});

window.addEventListener("hashchange", () => {
  const view = location.hash.slice(1);
  if (validViews.has(view) && view !== state.view) switchView(view, false);
});

async function switchView(view, updateHash = true) {
  state.view = view;
  state.query = "";
  if (updateHash) history.replaceState(null, "", `#${view}`);
  document.querySelectorAll(".nav-item").forEach((item) => {
    item.classList.toggle("active", item.dataset.view === view);
  });
  document.querySelector("#pageTitle").textContent = titles[view];
  createButton.classList.toggle("hidden", !editableViews.has(view));
  await loadView();
}

async function loadView() {
  content.innerHTML = loadingState();
  setServerStatus("loading");
  try {
    if (state.view === "overview") {
      renderOverview(await api("/api/admin/overview"));
      loginLayer.classList.add("hidden");
      setServerStatus("online");
      return;
    }
    const records = await api(`/api/admin/${state.view}`);
    state.records = records;
    renderTable(state.view, records);
    loginLayer.classList.add("hidden");
    setServerStatus("online");
  } catch (error) {
    if (error.status === 401) {
      loginLayer.classList.remove("hidden");
      content.innerHTML = "";
      setServerStatus("offline");
      return;
    }
    setServerStatus("offline");
    content.innerHTML = errorState(error.message);
  }
}

function renderOverview(data) {
  const metrics = [
    ["用户", data.users],
    ["活跃合作方", data.activePartners],
    ["在架歌曲", data.activeSongs],
    ["有效方案", data.activePlans],
    ["合作会话", data.conversations],
    ["待处理结算", data.pendingSettlements],
  ];
  content.innerHTML = `
    <div class="metric-grid">
      ${metrics
        .map(
          ([label, value]) => `
            <article class="metric-card">
              <span>${label}</span>
              <strong>${value}</strong>
            </article>`,
        )
        .join("")}
    </div>
    <section class="table-panel">
      <div class="table-head"><h2>最近注册用户</h2></div>
      ${table(
        ["用户", "身份", "认证", "注册时间"],
        data.recentUsers.map((user) => [
          identity(user.avatar, user.organization),
          roleLabel(user.role),
          badge(user.verified ? "已认证" : "未认证", user.verified),
          formatDate(user.createdAt),
        ]),
      )}
    </section>`;
}

function renderTable(view, records) {
  const definitions = {
    partners: {
      headers: ["合作方", "类型", "匹配度", "标签", "状态", "操作"],
      row: (item) => [
        identity(item.avatar, item.name, item.description),
        roleLabel(item.partnerType),
        `${item.matchScore}%`,
        tags(item.tags),
        badge(item.active ? "启用" : "停用", item.active),
        actions(item.id),
      ],
    },
    songs: {
      headers: ["歌曲", "音乐人", "封面主题", "状态", "操作"],
      row: (item) => [
        `<span class="cell-title">${escapeHtml(item.name)}</span>`,
        escapeHtml(item.artist),
        escapeHtml(item.coverClass),
        badge(item.active ? "启用" : "停用", item.active),
        actions(item.id),
      ],
    },
    plans: {
      headers: ["方案", "类型", "预算", "匹配度", "标签", "状态", "操作"],
      row: (item) => [
        titleCell(item.title, item.description),
        escapeHtml(item.planType),
        money(item.budgetAmount),
        `${item.score}%`,
        tags(item.tags),
        badge(item.active ? "启用" : "停用", item.active),
        actions(item.id),
      ],
    },
    users: {
      headers: ["用户", "身份", "认证", "注册时间"],
      row: (item) => [
        identity(item.avatar, item.organization),
        roleLabel(item.role),
        badge(item.verified ? "已认证" : "未认证", item.verified),
        formatDate(item.createdAt),
      ],
    },
    conversations: {
      headers: ["用户", "合作方", "最后消息", "未读", "更新时间"],
      row: (item) => [
        escapeHtml(item.userName),
        escapeHtml(item.partnerName),
        escapeHtml(item.lastMessage),
        item.unreadCount,
        formatDate(item.updatedAt),
      ],
    },
    settlements: {
      headers: ["用户", "事项", "金额", "状态", "时间"],
      row: (item) => [
        escapeHtml(item.userName),
        escapeHtml(item.title),
        money(item.amount),
        badge(settlementLabel(item.status), item.status === "completed"),
        formatDate(item.createdAt),
      ],
    },
  };
  const definition = definitions[view];
  const filtered = filterRecords(records, state.query);
  content.innerHTML = `
    <section class="table-panel">
      <div class="table-head">
        <div>
          <h2>${titles[view]}</h2>
          <span class="muted">共 ${records.length} 条${state.query ? `，筛选出 ${filtered.length} 条` : ""}</span>
        </div>
        <label class="table-search">
          <span aria-hidden="true">⌕</span>
          <input data-search value="${escapeAttribute(state.query)}" placeholder="搜索当前列表" aria-label="搜索当前列表" />
        </label>
      </div>
      ${table(
        definition.headers,
        filtered.map(definition.row),
        state.query
          ? "没有符合条件的记录"
          : editableViews.has(view)
            ? `暂无数据<button class="empty-action" data-create>新增第一条记录</button>`
            : "暂无数据",
      )}
    </section>`;
}

function table(headers, rows, emptyMessage = "暂无数据") {
  if (!rows.length) return `<div class="empty">${emptyMessage}</div>`;
  return `
    <div class="table-wrap">
      <table>
        <thead><tr>${headers.map((item) => `<th>${item}</th>`).join("")}</tr></thead>
        <tbody>
          ${rows
            .map(
              (row) => `<tr>${row.map((cell) => `<td>${cell ?? ""}</td>`).join("")}</tr>`,
            )
            .join("")}
        </tbody>
      </table>
    </div>`;
}

function filterRecords(records, query) {
  const keyword = query.trim().toLocaleLowerCase();
  if (!keyword) return records;
  return records.filter((record) =>
    Object.values(record).some((value) => {
      const text = Array.isArray(value) ? value.join(" ") : String(value ?? "");
      return text.toLocaleLowerCase().includes(keyword);
    }),
  );
}

function openModal(record = null) {
  state.editingId = record?.id ?? null;
  document.querySelector("#modalTitle").textContent =
    `${record ? "编辑" : "新增"}${titles[state.view].replace("管理", "")}`;
  document.querySelector("#formFields").innerHTML = formFields(state.view, record);
  document.querySelector("#formError").textContent = "";
  modalLayer.classList.remove("hidden");
  document.body.classList.add("modal-open");
  requestAnimationFrame(() => modalLayer.querySelector("input, select, textarea")?.focus());
}

function closeModal() {
  modalLayer.classList.add("hidden");
  document.body.classList.remove("modal-open");
  state.editingId = null;
}

function formFields(view, item = {}) {
  if (view === "partners") {
    return [
      selectField("partnerType", "类型", item.partnerType, [
        ["provider", "服务方"],
        ["client", "被服务方"],
      ]),
      inputField("name", "名称", item.name, true),
      inputField("avatar", "头像文字", item.avatar, true),
      selectField("avatarClass", "头像配色", item.avatarClass, colorOptions()),
      inputField("identity", "身份说明", item.identity, true),
      numberField("matchScore", "匹配度", item.matchScore ?? 90, 0, 100),
      textareaField("description", "简介", item.description, "full"),
      inputField("tags", "标签（逗号分隔）", (item.tags ?? []).join(","), true, "full"),
      inputField("resultText", "业务结果", item.resultText, true),
      checkboxField("active", "启用", item.active ?? true),
    ].join("");
  }
  if (view === "songs") {
    return [
      inputField("name", "歌曲名称", item.name, true),
      inputField("artist", "音乐人", item.artist, true),
      selectField("coverClass", "封面主题", item.coverClass, colorOptions()),
      checkboxField("active", "启用", item.active ?? true),
    ].join("");
  }
  return [
    inputField("title", "方案名称", item.title, true),
    inputField("planType", "方案类型", item.planType, true),
    selectField("iconClass", "图标", item.iconClass, [
      ["video", "短视频"],
      ["campus", "校园"],
      ["briefcase", "品牌"],
      ["audio", "音频"],
    ]),
    selectField("colorClass", "配色", item.colorClass, colorOptions()),
    textareaField("description", "方案说明", item.description, "full"),
    inputField("tags", "标签（逗号分隔）", (item.tags ?? []).join(","), true, "full"),
    numberField("budgetAmount", "预算（分）", item.budgetAmount ?? 0, 0),
    numberField("score", "匹配度", item.score ?? 90, 0, 100),
    checkboxField("active", "启用", item.active ?? true),
  ].join("");
}

async function saveEntity(event) {
  event.preventDefault();
  const button = document.querySelector("#saveButton");
  const values = Object.fromEntries(new FormData(event.currentTarget));
  const body = normalizeForm(state.view, values);
  const path = state.editingId
    ? `/api/admin/${state.view}/${state.editingId}`
    : `/api/admin/${state.view}`;
  try {
    document.querySelector("#formError").textContent = "";
    setButtonBusy(button, true, "保存中");
    await api(path, {
      method: state.editingId ? "PUT" : "POST",
      body,
    });
    closeModal();
    toast("已保存");
    await loadView();
  } catch (error) {
    document.querySelector("#formError").textContent = error.message;
  } finally {
    setButtonBusy(button, false);
  }
}

function normalizeForm(view, values) {
  const active = values.active === "on";
  if (view === "partners") {
    return {
      ...values,
      active,
      matchScore: Number(values.matchScore),
      tags: splitTags(values.tags),
    };
  }
  if (view === "songs") return { ...values, active };
  return {
    ...values,
    active,
    budgetAmount: Number(values.budgetAmount),
    score: Number(values.score),
    tags: splitTags(values.tags),
  };
}

function inputField(name, label, value = "", required = false, className = "") {
  return `<label class="${className}">${label}<input name="${name}" value="${escapeAttribute(value)}" ${required ? "required" : ""}></label>`;
}

function numberField(name, label, value, min, max = "") {
  return `<label>${label}<input name="${name}" type="number" value="${value}" min="${min}" ${max !== "" ? `max="${max}"` : ""} required></label>`;
}

function textareaField(name, label, value = "", className = "") {
  return `<label class="${className}">${label}<textarea name="${name}" required>${escapeHtml(value ?? "")}</textarea></label>`;
}

function selectField(name, label, value, options) {
  return `<label>${label}<select name="${name}">${options
    .map(
      ([key, text]) =>
        `<option value="${key}" ${value === key ? "selected" : ""}>${text}</option>`,
    )
    .join("")}</select></label>`;
}

function checkboxField(name, label, checked) {
  return `<label class="checkbox-label"><input name="${name}" type="checkbox" ${checked ? "checked" : ""}>${label}</label>`;
}

function colorOptions() {
  return [
    ["aqua", "青绿"],
    ["blue", "蓝色"],
    ["violet", "紫色"],
    ["gold", "金色"],
    ["sunset", "日落"],
    ["ocean", "海洋"],
  ];
}

async function api(path, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 12000);
  let response;
  try {
    response = await fetch(path, {
      method: options.method ?? "GET",
      headers: options.body ? { "Content-Type": "application/json" } : undefined,
      body: options.body ? JSON.stringify(options.body) : undefined,
      credentials: "same-origin",
      signal: controller.signal,
    });
  } catch (error) {
    if (error.name === "AbortError") throw new Error("请求超时，请稍后重试");
    throw new Error("无法连接服务，请检查网络后重试");
  } finally {
    clearTimeout(timeout);
  }
  if (!response.ok) {
    const payload = await response.json().catch(() => ({}));
    const error = new Error(payload.error ?? `请求失败 (${response.status})`);
    error.status = response.status;
    throw error;
  }
  if (response.status === 204) return null;
  return response.json();
}

function loadingState() {
  return `
    <div class="loading" aria-label="正在读取数据">
      <span class="loading-spinner"></span>
      <strong>正在读取数据</strong>
      <span>请稍候</span>
    </div>`;
}

function errorState(message) {
  return `
    <div class="empty error-state">
      <strong>暂时无法加载</strong>
      <span>${escapeHtml(message)}</span>
      <button class="secondary-button" data-retry>重新加载</button>
    </div>`;
}

function setServerStatus(status) {
  const labels = {
    loading: "正在连接",
    online: "服务正常",
    offline: "连接异常",
  };
  serverStatus.textContent = labels[status];
  serverStatus.className = `status ${status}`;
}

function setButtonBusy(button, busy, label = "") {
  if (!button) return;
  if (busy) {
    button.dataset.originalText = button.textContent;
    button.textContent = label;
    button.disabled = true;
  } else {
    button.textContent = button.dataset.originalText || button.textContent;
    button.disabled = false;
    delete button.dataset.originalText;
  }
}

function confirmAction(title, message) {
  document.querySelector("#confirmTitle").textContent = title;
  document.querySelector("#confirmMessage").textContent = message;
  confirmLayer.classList.remove("hidden");
  document.body.classList.add("modal-open");
  const cancel = document.querySelector("#confirmCancel");
  const accept = document.querySelector("#confirmAccept");
  accept.focus();
  return new Promise((resolve) => {
    const finish = (accepted) => {
      confirmLayer.classList.add("hidden");
      document.body.classList.remove("modal-open");
      cancel.onclick = null;
      accept.onclick = null;
      resolve(accepted);
    };
    cancel.onclick = () => finish(false);
    accept.onclick = () => finish(true);
  });
}

function identity(avatar, title, subtitle = "") {
  return `<div class="identity-cell"><span class="avatar">${escapeHtml(avatar)}</span><span>${titleCell(title, subtitle)}</span></div>`;
}

function titleCell(title, subtitle = "") {
  return `<div class="cell-title">${escapeHtml(title)}</div>${subtitle ? `<div class="cell-subtitle">${escapeHtml(subtitle)}</div>` : ""}`;
}

function actions(id) {
  return `<div class="row-actions"><button class="text-button" data-edit="${id}">编辑</button><button class="text-button danger" data-delete="${id}">停用</button></div>`;
}

function badge(text, active) {
  return `<span class="state ${active ? "" : "inactive"}">${escapeHtml(text)}</span>`;
}

function tags(items) {
  return (items ?? []).map((item) => `<span class="tag">${escapeHtml(item)}</span>`).join("");
}

function roleLabel(role) {
  return role === "provider" ? "服务方" : "被服务方";
}

function settlementLabel(status) {
  return { completed: "已完成", pending: "待处理", rejected: "已拒绝" }[status] ?? status;
}

function money(cents) {
  return new Intl.NumberFormat("zh-CN", {
    style: "currency",
    currency: "CNY",
  }).format(cents / 100);
}

function formatDate(value) {
  if (!value) return "—";
  return value.replace("T", " ").slice(0, 16);
}

function splitTags(value) {
  return value
    .split(/[,，]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function escapeAttribute(value) {
  return escapeHtml(value);
}

let toastTimer;
function toast(message) {
  const element = document.querySelector("#toast");
  element.textContent = message;
  element.classList.add("visible");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => element.classList.remove("visible"), 1800);
}

switchView(state.view, false);
