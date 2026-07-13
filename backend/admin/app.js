const state = {
  view: location.hash.slice(1) || "overview",
  records: [],
  editingId: null,
  query: "",
};

const titles = {
  overview: "运营总览",
  analytics: "数据驾驶舱",
  partners: "公开主页管理",
  songs: "曲库管理",
  plans: "推广方案",
  users: "入驻审核",
  conversations: "合作会话",
  settlements: "结算记录",
  agent: "Agent 设置",
};

const editableViews = new Set(["partners", "songs", "plans"]);
state.agent = { settings: null, tools: [], toolEditing: null, tab: "settings", sessions: [], sessionDetail: null, users: [] };
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
  const settlementAction = event.target.closest("[data-settlement-action]");
  const reviewAction = event.target.closest("[data-review-action]");
  if (retry) {
    await loadView();
    return;
  }
  if (create) {
    openModal();
    return;
  }
  if (reviewAction) {
    const status = reviewAction.dataset.reviewAction;
    const approved = status === "approved";
    const accepted = await confirmAction(
      approved ? "确认通过这份入驻申请？" : "确认退回这份入驻申请？",
      approved
        ? "通过后会公开该用户主页，并加入对应角色的合作广场。"
        : "退回后不会公开展示，用户可补充资料后重新提交。",
    );
    if (!accepted) return;
    setButtonBusy(reviewAction, true, "处理中");
    try {
      await api(`/api/admin/users/${reviewAction.dataset.id}/review`, {
        method: "PUT",
        body: { status },
      });
      toast(approved ? "入驻审核已通过" : "入驻申请已退回");
      await loadView();
    } catch (error) {
      toast(error.message);
      setButtonBusy(reviewAction, false);
    }
    return;
  }
  if (settlementAction) {
    const status = settlementAction.dataset.settlementAction;
    const actionLabel = status === "completed" ? "批准提现" : "拒绝提现";
    const accepted = await confirmAction(
      `确认${actionLabel}？`,
      status === "completed"
        ? "批准后将标记为已完成，提现进入打款完成状态。"
        : "拒绝后将退回提现金额至用户钱包。",
    );
    if (!accepted) return;
    setButtonBusy(settlementAction, true, "处理中");
    try {
      await api(`/api/admin/settlements/${settlementAction.dataset.id}`, {
        method: "PUT",
        body: { status },
      });
      toast(`${actionLabel}成功`);
      await loadView();
    } catch (error) {
      toast(error.message);
      setButtonBusy(settlementAction, false);
    }
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
    if (state.view === "analytics") {
      renderAnalytics(await api("/api/admin/analytics"));
      loginLayer.classList.add("hidden");
      setServerStatus("online");
      return;
    }
    if (state.view === "agent") {
      const [settings, tools] = await Promise.all([
        api("/api/admin/agent/settings"),
        api("/api/admin/agent/tools"),
      ]);
      state.agent.settings = settings;
      state.agent.tools = tools;
      renderAgent(settings, tools);
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
        ["用户", "身份", "入驻状态", "注册时间"],
        data.recentUsers.map((user) => [
          identity(user.avatar, user.organization),
          roleLabel(user.role),
          badge(onboardingLabel(user.onboardingStatus), user.onboardingStatus === "approved"),
          formatDate(user.createdAt),
        ]),
      )}
    </section>`;
}

function renderAnalytics(data) {
  const latest = data.trend[data.trend.length - 1] ?? {};
  content.innerHTML = `
    <section class="analytics-hero">
      <div>
        <p class="eyebrow">REAL-TIME BUSINESS INTELLIGENCE</p>
        <h2>增长、转化与 Agent 执行全景</h2>
        <p>所有指标直接聚合业务数据库，工具调用与执行轨迹同步进入运营视图。</p>
      </div>
      <div class="live-signal"><i></i><span>实时数据</span><strong>${formatDate(new Date().toISOString())}</strong></div>
    </section>
    <div class="metric-grid analytics-metrics">
      ${data.metrics
        .map(
          (item, index) => `
          <article class="metric-card tone-${index % 3}">
            <span>${escapeHtml(item.label)}</span>
            <strong>${escapeHtml(item.displayValue)}</strong>
            <small>${item.change ? `${item.change > 0 ? "+" : ""}${item.change}% 较上周期` : "累计实时值"}</small>
          </article>`,
        )
        .join("")}
    </div>
    <div class="analytics-grid">
      <section class="panel trend-panel">
        <div class="panel-head">
          <div><p class="eyebrow">14 DAYS</p><h2>核心业务趋势</h2></div>
          <div class="chart-legend"><span class="violet"></span>匹配 <span class="cyan"></span>会话</div>
        </div>
        ${trendChart(data.trend)}
        <div class="trend-footer">
          <span>今日新增用户 <strong>${latest.users ?? 0}</strong></span>
          <span>今日匹配 <strong>${latest.matches ?? 0}</strong></span>
          <span>今日会话 <strong>${latest.connections ?? 0}</strong></span>
          <span>今日交易额 <strong>${money(latest.revenue ?? 0)}</strong></span>
        </div>
      </section>
      <section class="panel funnel-panel">
        <div class="panel-head"><div><p class="eyebrow">CONVERSION</p><h2>业务转化漏斗</h2></div></div>
        ${distributionBars(data.funnel, "conversion", "%")}
      </section>
      <section class="panel">
        <div class="panel-head"><div><p class="eyebrow">PARTNER MIX</p><h2>合作方构成</h2></div></div>
        ${distributionBars(data.partnerMix, "percent", "%")}
      </section>
      <section class="panel">
        <div class="panel-head"><div><p class="eyebrow">AGENT TOOL CALLS</p><h2>工具调用分布</h2></div></div>
        ${data.toolUsage.length ? distributionBars(data.toolUsage, "percent", "%") : '<div class="empty compact">等待首个 Agent 工具调用</div>'}
      </section>
    </div>
    <section class="table-panel agent-runs">
      <div class="table-head">
        <div><h2>最近 Agent 运行</h2><span class="muted">监控会话、工具调用量与运行状态</span></div>
        <span class="agent-runtime">STARCONNECT RUNTIME</span>
      </div>
      ${table(
        ["用户", "会话目标", "工具调用", "状态", "最近运行"],
        data.recentRuns.map((run) => [
          escapeHtml(run.userName),
          escapeHtml(run.title),
          `<strong>${run.toolCalls}</strong> calls`,
          badge(run.status === "active" ? "可继续" : run.status, run.status === "active"),
          formatDate(run.updatedAt),
        ]),
        "暂无 Agent 运行记录",
      )}
    </section>`;
}

function trendChart(points) {
  const maximum = Math.max(1, ...points.flatMap((item) => [item.matches, item.connections]));
  const width = 720;
  const height = 210;
  const x = (index) => (points.length <= 1 ? 0 : (index / (points.length - 1)) * width);
  const y = (value) => height - (value / maximum) * (height - 20);
  const line = (key) => points.map((item, index) => `${x(index)},${y(item[key])}`).join(" ");
  return `
    <div class="trend-chart">
      <svg viewBox="0 0 ${width} ${height}" role="img" aria-label="十四天业务趋势">
        <defs>
          <linearGradient id="violetArea" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stop-color="#756bd7" stop-opacity=".26"/>
            <stop offset="1" stop-color="#756bd7" stop-opacity="0"/>
          </linearGradient>
        </defs>
        <polyline class="chart-area" points="0,${height} ${line("matches")} ${width},${height}" />
        <polyline class="chart-line violet" points="${line("matches")}" />
        <polyline class="chart-line cyan" points="${line("connections")}" />
      </svg>
      <div class="chart-labels">${points
        .filter((_, index) => index % 2 === 0 || index === points.length - 1)
        .map((item) => `<span>${escapeHtml(item.label)}</span>`)
        .join("")}</div>
    </div>`;
}

function distributionBars(items, percentKey, suffix) {
  const maximum = Math.max(1, ...items.map((item) => item.value));
  return `<div class="distribution-list">${items
    .map((item) => {
      const percent = item[percentKey] ?? Math.round((item.value / maximum) * 100);
      return `
        <div class="distribution-row">
          <div><span>${escapeHtml(item.label)}</span><strong>${item.value} · ${percent}${suffix}</strong></div>
          <div class="distribution-track"><i style="width:${Math.max(3, Math.min(100, percent))}%"></i></div>
        </div>`;
    })
    .join("")}</div>`;
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
      headers: ["申请主体", "身份", "联系信息", "申请内容", "入驻状态", "注册时间", "操作"],
      row: (item) => [
        identity(item.avatar, item.organization, item.workTitle || item.cooperationBudget),
        roleLabel(item.role),
        item.contactName
          ? titleCell(item.contactName, item.contactMethod)
          : '<span class="muted">尚未填写</span>',
        item.applicationDescription
          ? `<div class="cell-subtitle">${escapeHtml(item.applicationDescription)}</div>`
          : '<span class="muted">等待提交</span>',
        badge(onboardingLabel(item.onboardingStatus), item.onboardingStatus === "approved"),
        formatDate(item.createdAt),
        onboardingActions(item),
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
      headers: ["用户", "事项", "金额", "状态", "时间", "操作"],
      row: (item) => [
        escapeHtml(item.userName),
        escapeHtml(item.title),
        money(item.amount),
        badge(settlementLabel(item.status), item.status === "completed"),
        formatDate(item.createdAt),
        settlementActions(item),
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
        ["provider", "推广服务方"],
        ["client", "音乐创作者"],
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

function settlementActions(item) {
  if (item.status !== "pending" || item.amount >= 0) return "—";
  return `<div class="row-actions"><button class="text-button" data-id="${item.id}" data-settlement-action="completed">批准</button><button class="text-button danger" data-id="${item.id}" data-settlement-action="rejected">拒绝</button></div>`;
}

function onboardingActions(item) {
  if (item.onboardingStatus !== "pending") {
    return item.reviewNote
      ? `<span class="cell-subtitle">${escapeHtml(item.reviewNote)}</span>`
      : "—";
  }
  return `<div class="row-actions"><button class="text-button" data-id="${item.id}" data-review-action="approved">通过</button><button class="text-button danger" data-id="${item.id}" data-review-action="rejected">退回</button></div>`;
}

function badge(text, active) {
  return `<span class="state ${active ? "" : "inactive"}">${escapeHtml(text)}</span>`;
}

function tags(items) {
  return (items ?? []).map((item) => `<span class="tag">${escapeHtml(item)}</span>`).join("");
}

function roleLabel(role) {
  return role === "provider" ? "推广服务方" : "音乐创作者";
}

function onboardingLabel(status) {
  return {
    draft: "待填写",
    pending: "待审核",
    approved: "已通过",
    rejected: "需补充",
  }[status] ?? status;
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

// Agent UI and handlers are in /admin/agent.js

switchView(state.view, false);
