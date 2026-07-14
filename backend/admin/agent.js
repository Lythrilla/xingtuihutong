function renderAgent(settings, tools) {
  const tab = state.agent.tab ?? "settings";
  content.innerHTML = `
    <div class="agent-tabs">
      <button class="agent-tab ${tab === "settings" ? "active" : ""}" data-agent-tab="settings">基础设置</button>
      <button class="agent-tab ${tab === "tools" ? "active" : ""}" data-agent-tab="tools">工具管理</button>
      <button class="agent-tab ${tab === "sessions" ? "active" : ""}" data-agent-tab="sessions">会话监控</button>
      <button class="agent-tab ${tab === "test" ? "active" : ""}" data-agent-tab="test">快速测试</button>
    </div>
    <div class="agent-body">
      ${tab === "settings" ? renderAgentSettings(settings) : ""}
      ${tab === "tools" ? renderAgentTools(tools) : ""}
      ${tab === "sessions" ? renderAgentSessions(state.agent.sessions) : ""}
      ${tab === "test" ? renderAgentTest() : ""}
    </div>`;
}

function renderAgentSettings(settings) {
  return `
    <div class="agent-layout">
      <section class="agent-panel">
        <div class="panel-head"><div><h2>基础设置</h2><span class="muted">配置 OpenAI 兼容接口、模型参数与默认话术</span></div></div>
        <form class="agent-form" data-agent-settings>
          <div class="form-grid">
            <label class="checkbox-label full"><input name="enabled" type="checkbox" ${settings.enabled ? "checked" : ""}>启用 Agent 服务</label>
            <label>引擎标识<input name="engine" value="${escapeAttribute(settings.engine)}" required></label>
            <label>模型名称<input name="model" value="${escapeAttribute(settings.model)}" required></label>
            <label class="full">Chat Completions 接口地址
              <input name="apiUrl" type="url" value="${escapeAttribute(settings.apiUrl)}" placeholder="https://api.example.com/v1/chat/completions">
              <span class="muted">留空时使用后端环境变量 AGENT_MODEL_API_URL</span>
            </label>
            <label class="full">API Key
              <input name="apiKey" type="password" value="" autocomplete="new-password" placeholder="${settings.apiKeyConfigured ? "已配置，留空保持不变" : "输入兼容接口的 API Key"}">
              <span class="muted">${settings.apiKeyConfigured ? "已保存 API Key，不会在页面中回显" : "留空时使用后端环境变量 AGENT_MODEL_API_KEY"}</span>
            </label>
            ${settings.apiKeyConfigured ? '<label class="checkbox-label full"><input name="clearApiKey" type="checkbox">清除后台已保存的 API Key</label>' : ""}
            <label>最大 Token 数<input name="maxTokens" type="number" value="${settings.maxTokens}" min="1" required></label>
            <label>温度 (0-2)<input name="temperature" type="number" value="${settings.temperature}" step="0.1" min="0" max="2" required></label>
            <label>最大工具调用数<input name="maxToolCalls" type="number" value="${settings.maxToolCalls}" min="1" required></label>
            <label>最大历史消息数<input name="maxHistory" type="number" value="${settings.maxHistory}" min="1" required></label>
            <label>建议语数量<input name="suggestionCount" type="number" value="${settings.suggestionCount}" min="1" required></label>
            <label class="full">欢迎语（可用 {organization} 占位用户名称）
              <textarea name="welcomeMessage" required>${escapeHtml(settings.welcomeMessage)}</textarea>
            </label>
            <label class="full">系统 Prompt
              <textarea name="systemPrompt" required>${escapeHtml(settings.systemPrompt)}</textarea>
            </label>
            <label class="full">未命中回退话术
              <textarea name="fallbackReply" required>${escapeHtml(settings.fallbackReply)}</textarea>
            </label>
            <label class="full">默认建议语（每行一条）
              <textarea name="defaultSuggestions" rows="4">${escapeHtml((settings.defaultSuggestions ?? []).join("\n"))}</textarea>
            </label>
            <label class="full">后续建议语（每行一条）
              <textarea name="followUpSuggestions" rows="4">${escapeHtml((settings.followUpSuggestions ?? []).join("\n"))}</textarea>
            </label>
          </div>
          <div class="modal-actions">
            <button class="primary-button" type="submit">保存基础设置</button>
          </div>
        </form>
      </section>
    </div>`;
}

function renderAgentTools(tools) {
  return `
    <section class="agent-panel">
      <div class="panel-head">
        <div><h2>工具列表</h2><span class="muted">启用、禁用与编辑工具触发关键词</span></div>
        <button class="primary-button" data-create-agent-tool type="button">新增工具</button>
      </div>
      <div class="agent-tools" data-agent-tools>
        ${tools.map((tool) => renderAgentTool(tool, tool.name === state.agent.toolEditing)).join("")}
        ${state.agent.toolEditing === "__new__" ? renderAgentToolNew() : ""}
      </div>
    </section>`;
}

function renderAgentTool(tool, editing = false) {
  const keywords = (tool.keywords ?? []).join(", ");
  const blockedKeywords = (tool.blockedKeywords ?? []).join(", ");
  const requiredTools = (tool.requiredTools ?? []).join(", ");
  const keywordGroups = (tool.keywordGroups ?? [])
    .map((group) => group.join(", "))
    .join("\n");
  if (!editing) {
    return `
      <article class="agent-tool-card" data-tool-name="${escapeAttribute(tool.name)}">
        <div class="agent-tool-header">
          <div class="agent-tool-title">
            <strong>${escapeHtml(tool.label)}</strong>
            <code>${escapeHtml(tool.name)}</code>
            <span class="state ${tool.mode === "write" ? "inactive" : ""}">${tool.mode === "write" ? "write" : "read"}</span>
          </div>
          <div class="agent-tool-actions">
            <label class="agent-toggle">
              <input type="checkbox" data-tool-toggle ${tool.enabled ? "checked" : ""}>
              <span>${tool.enabled ? "已启用" : "已停用"}</span>
            </label>
            <button class="text-button" data-tool-edit="${escapeAttribute(tool.name)}" type="button">编辑</button>
          </div>
        </div>
        <p class="muted">${escapeHtml(tool.description)}</p>
        <div class="agent-tool-meta">
          <span>关键词：${escapeHtml(keywords) || "—"}</span>
          <span>屏蔽词：${escapeHtml(blockedKeywords) || "—"}</span>
          <span>依赖工具：${escapeHtml(requiredTools) || "—"}</span>
          <span>排序：${tool.sortOrder}</span>
        </div>
      </article>`;
  }
  return `
    <article class="agent-tool-card editing" data-tool-name="${escapeAttribute(tool.name)}">
      <form class="agent-tool-form" data-tool-form>
        <input type="hidden" name="name" value="${escapeAttribute(tool.name)}">
        <div class="form-grid">
          <label>名称（只读）<input value="${escapeAttribute(tool.name)}" disabled></label>
          <label>显示名称<input name="label" value="${escapeAttribute(tool.label)}" required></label>
          <label>模式<select name="mode"><option value="read" ${tool.mode === "read" ? "selected" : ""}>read</option><option value="write" ${tool.mode === "write" ? "selected" : ""}>write</option></select></label>
          <label>排序<input name="sortOrder" type="number" value="${tool.sortOrder}" required></label>
          <label class="checkbox-label full"><input name="enabled" type="checkbox" ${tool.enabled ? "checked" : ""}>启用</label>
          <label class="full">说明<textarea name="description" required>${escapeHtml(tool.description)}</textarea></label>
          <label class="full">触发关键词（逗号分隔）<input name="keywords" value="${escapeAttribute(keywords)}"></label>
          <label class="full">屏蔽关键词（逗号分隔）<input name="blockedKeywords" value="${escapeAttribute(blockedKeywords)}"></label>
          <label class="full">关键词组合（每行一组，组内逗号分隔表示 AND）
            <textarea name="keywordGroups" rows="4">${escapeHtml(keywordGroups)}</textarea>
          </label>
          <label class="full">依赖工具（逗号分隔）<input name="requiredTools" value="${escapeAttribute(requiredTools)}"></label>
        </div>
        <div class="modal-actions">
          <button class="secondary-button" type="button" data-tool-cancel="${escapeAttribute(tool.name)}">取消</button>
          <button class="primary-button" type="submit">保存工具</button>
        </div>
      </form>
    </article>`;
}

function renderAgentToolNew() {
  return `
    <article class="agent-tool-card editing" data-tool-name="">
      <form class="agent-tool-form" data-tool-form>
        <div class="form-grid">
          <label>名称<input name="name" value="" required></label>
          <label>显示名称<input name="label" value="" required></label>
          <label>模式<select name="mode"><option value="read">read</option><option value="write">write</option></select></label>
          <label>排序<input name="sortOrder" type="number" value="0" required></label>
          <label class="checkbox-label full"><input name="enabled" type="checkbox" checked>启用</label>
          <label class="full">说明<textarea name="description" required></textarea></label>
          <label class="full">触发关键词（逗号分隔）<input name="keywords"></label>
          <label class="full">屏蔽关键词（逗号分隔）<input name="blockedKeywords"></label>
          <label class="full">关键词组合（每行一组，组内逗号分隔表示 AND）
            <textarea name="keywordGroups" rows="4"></textarea>
          </label>
          <label class="full">依赖工具（逗号分隔）<input name="requiredTools"></label>
        </div>
        <div class="modal-actions">
          <button class="secondary-button" type="button" data-tool-cancel-new>取消</button>
          <button class="primary-button" type="submit">创建工具</button>
        </div>
      </form>
    </article>`;
}

function renderAgentSessions(sessions) {
  const detail = state.agent.sessionDetail;
  return `
    <section class="agent-panel">
      <div class="panel-head"><div><h2>会话列表</h2><span class="muted">查看 Agent 会话、消息与工具调用</span></div></div>
      <div class="agent-session-layout">
        <div class="agent-session-list">
          ${sessions.length === 0 ? '<div class="empty compact">暂无会话</div>' : sessions.map((session) => `
            <div class="agent-session-item ${detail?.id === session.id ? "active" : ""}" data-session-id="${escapeAttribute(session.id)}">
              <strong>${escapeHtml(session.title)}</strong>
              <span class="muted">${escapeHtml(session.userOrganization || session.userId)} · ${formatDate(session.updatedAt)}</span>
              <span class="state ${session.status === "active" ? "" : "inactive"}">${session.status}</span>
            </div>
          `).join("")}
        </div>
        <div class="agent-session-detail">
          ${detail ? renderSessionDetail(detail) : '<div class="empty compact">选择左侧会话查看详情</div>'}
        </div>
      </div>
    </section>`;
}

function renderSessionDetail(detail) {
  const messages = detail.messages ?? [];
  const toolCalls = detail.toolCalls ?? [];
  return `
    <div class="agent-session-detail-head">
      <div>
        <h3>${escapeHtml(detail.title)}</h3>
        <span class="muted">${escapeHtml(detail.userOrganization || detail.userId)} · ${formatDate(detail.createdAt)}</span>
      </div>
      <span class="state ${detail.status === "active" ? "" : "inactive"}">${detail.status}</span>
    </div>
    <div class="agent-session-messages">
      ${messages.length === 0 ? '<div class="empty compact">暂无消息</div>' : messages.map((msg) => `
        <div class="agent-message ${msg.role}">
          <span class="agent-message-role">${msg.role === "user" ? "用户" : "助手"}</span>
          <p>${escapeHtml(msg.content)}</p>
          <span class="muted">${formatDate(msg.createdAt)}</span>
        </div>
      `).join("")}
    </div>
    ${toolCalls.length ? `
      <div class="agent-session-toolcalls">
        <h4>工具调用</h4>
        ${toolCalls.map((call) => `
          <div class="agent-toolcall-row">
            <span class="state ${call.status === "completed" ? "" : "inactive"}">${call.status}</span>
            <strong>${escapeHtml(call.label)}</strong>
            <code>${escapeHtml(call.toolName)}</code>
            <span class="muted">${formatDate(call.createdAt)}</span>
          </div>
        `).join("")}
      </div>
    ` : ""}`;
}

function renderAgentTest() {
  const users = state.agent.users ?? [];
  const result = state.agent.testResult;
  return `
    <section class="agent-panel">
      <div class="panel-head"><div><h2>快速测试</h2><span class="muted">模拟用户输入并观察 Agent 执行结果</span></div></div>
      <form class="agent-form" data-agent-test>
        <div class="form-grid">
          <label class="full">选择测试用户
            <select name="userId" required>
              <option value="">请选择</option>
              ${users.map((user) => `<option value="${escapeAttribute(user.id)}">${escapeHtml(user.organization || user.id)}</option>`).join("")}
            </select>
          </label>
          <label class="full">测试输入
            <textarea name="prompt" rows="3" required></textarea>
          </label>
        </div>
        <div class="modal-actions">
          <button class="primary-button" type="submit">发送测试</button>
        </div>
      </form>
      ${result ? `
        <div class="agent-test-result">
          <h4>Agent 回复</h4>
          <p>${escapeHtml(result.message?.content)}</p>
          ${result.toolCalls?.length ? `
            <h4>调用工具</h4>
            <ul>
              ${result.toolCalls.map((call) => `<li>${escapeHtml(call.label)} <span class="muted">(${escapeHtml(call.name)} · ${escapeHtml(call.status)})</span></li>`).join("")}
            </ul>
          ` : ""}
          ${result.suggestions?.length ? `
            <h4>建议语</h4>
            <div class="agent-test-suggestions">
              ${result.suggestions.map((item) => `<span class="tag">${escapeHtml(item)}</span>`).join("")}
            </div>
          ` : ""}
        </div>
      ` : ""}
    </section>`;
}

async function saveAgentSettings(form) {
  const values = Object.fromEntries(new FormData(form));
  const body = {
    enabled: values.enabled === "on",
    engine: values.engine,
    model: values.model,
    apiUrl: values.apiUrl,
    apiKey: values.apiKey,
    clearApiKey: values.clearApiKey === "on",
    welcomeMessage: values.welcomeMessage,
    systemPrompt: values.systemPrompt,
    maxTokens: Number(values.maxTokens),
    temperature: Number(values.temperature),
    maxToolCalls: Number(values.maxToolCalls),
    maxHistory: Number(values.maxHistory),
    fallbackReply: values.fallbackReply,
    suggestionCount: Number(values.suggestionCount),
    defaultSuggestions: values.defaultSuggestions.split("\n").map((item) => item.trim()).filter(Boolean),
    followUpSuggestions: values.followUpSuggestions.split("\n").map((item) => item.trim()).filter(Boolean),
  };
  await api("/api/admin/agent/settings", { method: "PUT", body });
  toast("Agent 设置已保存");
  await loadView();
}

async function saveAgentTool(form) {
  const values = Object.fromEntries(new FormData(form));
  const body = {
    name: values.name,
    label: values.label,
    mode: values.mode,
    sortOrder: Number(values.sortOrder),
    enabled: values.enabled === "on",
    description: values.description,
    keywords: commaList(values.keywords),
    blockedKeywords: commaList(values.blockedKeywords),
    keywordGroups: parseKeywordGroups(values.keywordGroups),
    requiredTools: commaList(values.requiredTools),
  };
  await api(`/api/admin/agent/tools/${encodeURIComponent(values.name)}`, { method: "PUT", body });
  state.agent.toolEditing = null;
  toast("工具已保存");
  await loadView();
}

async function createAgentTool(form) {
  const values = Object.fromEntries(new FormData(form));
  const body = {
    name: values.name,
    label: values.label,
    mode: values.mode,
    sortOrder: Number(values.sortOrder),
    enabled: values.enabled === "on",
    description: values.description,
    keywords: commaList(values.keywords),
    blockedKeywords: commaList(values.blockedKeywords),
    keywordGroups: parseKeywordGroups(values.keywordGroups),
    requiredTools: commaList(values.requiredTools),
  };
  await api("/api/admin/agent/tools", { method: "PUT", body: [body] });
  state.agent.toolEditing = null;
  toast("工具已创建");
  await loadView();
}

async function toggleAgentTool(name, enabled) {
  const tool = state.agent.tools.find((item) => item.name === name);
  if (!tool) return;
  const body = { ...tool, enabled };
  await api(`/api/admin/agent/tools/${encodeURIComponent(name)}`, { method: "PUT", body });
  toast(enabled ? "工具已启用" : "工具已停用");
  await loadView();
}

async function loadAgentSessions() {
  state.agent.sessions = await api("/api/admin/agent/sessions");
}

async function loadSessionDetail(sessionId) {
  const [detail, messages, toolCalls] = await Promise.all([
    api(`/api/admin/agent/sessions/${encodeURIComponent(sessionId)}`),
    api(`/api/admin/agent/sessions/${encodeURIComponent(sessionId)}/messages`),
    api(`/api/admin/agent/sessions/${encodeURIComponent(sessionId)}/tool_calls`),
  ]);
  state.agent.sessionDetail = { ...detail, messages, toolCalls };
}

async function loadAgentUsers() {
  state.agent.users = await api("/api/admin/users");
}

async function runAgentTest(form) {
  const values = Object.fromEntries(new FormData(form));
  const body = { userId: values.userId, prompt: values.prompt };
  const result = await api("/api/admin/agent/test", { method: "POST", body });
  state.agent.testResult = result;
  renderAgent(state.agent.settings, state.agent.tools);
}

function commaList(value) {
  return value
    .split(/[,，]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function parseKeywordGroups(value) {
  return value
    .split("\n")
    .map((line) =>
      line
        .split(/[,，]/)
        .map((item) => item.trim())
        .filter(Boolean),
    )
    .filter((group) => group.length > 0);
}

document.addEventListener("submit", async (event) => {
    if (event.target.matches("[data-agent-settings]")) {
      event.preventDefault();
      await saveAgentSettings(event.target);
      return;
    }
    if (event.target.matches("[data-tool-form]")) {
      event.preventDefault();
      const name = new FormData(event.target).get("name");
      if (name) {
        await saveAgentTool(event.target);
      } else {
        await createAgentTool(event.target);
      }
      return;
    }
    if (event.target.matches("[data-agent-test]")) {
      event.preventDefault();
      await runAgentTest(event.target);
      return;
    }
  });

  document.addEventListener("click", async (event) => {
    const edit = event.target.closest("[data-tool-edit]");
    const cancel = event.target.closest("[data-tool-cancel]");
    const cancelNew = event.target.closest("[data-tool-cancel-new]");
    const create = event.target.closest("[data-create-agent-tool]");
    const tab = event.target.closest("[data-agent-tab]");
    const session = event.target.closest("[data-session-id]");
    if (tab) {
      state.agent.tab = tab.dataset.agentTab;
      state.agent.toolEditing = null;
      state.agent.sessionDetail = null;
      state.agent.testResult = null;
      if (state.agent.tab === "sessions" && !state.agent.sessions.length) await loadAgentSessions();
      if (state.agent.tab === "test" && !state.agent.users.length) await loadAgentUsers();
      renderAgent(state.agent.settings, state.agent.tools);
      return;
    }
    if (session) {
      await loadSessionDetail(session.dataset.sessionId);
      renderAgent(state.agent.settings, state.agent.tools);
      return;
    }
    if (edit) {
      state.agent.toolEditing = edit.dataset.toolEdit;
      renderAgent(state.agent.settings, state.agent.tools);
      return;
    }
    if (cancel) {
      state.agent.toolEditing = null;
      renderAgent(state.agent.settings, state.agent.tools);
      return;
    }
    if (cancelNew) {
      state.agent.toolEditing = null;
      renderAgent(state.agent.settings, state.agent.tools);
      return;
    }
    if (create) {
      state.agent.toolEditing = "__new__";
      renderAgent(state.agent.settings, state.agent.tools);
      return;
    }
  });

document.addEventListener("change", (event) => {
    const toggle = event.target.closest("[data-tool-toggle]");
    if (toggle) {
      const card = event.target.closest("[data-tool-name]");
      if (card) toggleAgentTool(card.dataset.toolName, toggle.checked);
    }
  });
