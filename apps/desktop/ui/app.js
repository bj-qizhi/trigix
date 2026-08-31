const messages = {
  en: {
    skip_to_content: "Skip to main content",
    language: "Language",
    local_device_control: "LOCAL DEVICE CONTROL",
    lede: "Governed automation stays visible and interruptible.",
    runtime_status: "Runtime status",
    connection: "Connection",
    automation: "Automation",
    automation_host: "Automation host",
    state_revision: "State revision",
    checking: "Checking",
    device_trust: "DEVICE TRUST",
    pair_this_computer: "Pair this computer",
    approve_this_computer: "Approve this computer",
    computer_is_paired: "This computer is paired",
    pairing_unavailable: "Pairing unavailable",
    device_pairing: "Device pairing",
    pairing_help: "Enter the secure Platform origin and a recognizable Device name.",
    platform_origin: "Platform HTTPS origin",
    platform_origin_help: "HTTPS is required. Paths, queries, and fragments are not accepted.",
    device_name: "Device name",
    default_device_name: "My Windows PC",
    create_pairing_code: "Create pairing code",
    pairing_code: "PAIRING CODE",
    approve_code_admin: "Approve this code in your Tenant administration page.",
    approve_before: "Approve this code before {time}.",
    approved_code: "I approved the code",
    local_device_id: "LOCAL DEVICE ID",
    credential_protected: "The Device credential is protected by Windows Credential Manager.",
    forget_pairing: "Forget local pairing",
    voice_conversation: "VOICE CONVERSATION",
    local_microphone: "Local microphone",
    voice_description: "Microphone access starts only after you choose Start. Audio remains local in this foundation and cannot authorize automation.",
    start_voice: "Start microphone",
    stop_voice: "Stop microphone",
    state_microphone_off: "Microphone off",
    state_requesting_permission: "Requesting permission",
    state_listening: "Microphone active",
    state_microphone_stopped: "Microphone stopped",
    state_permission_denied: "Permission denied",
    state_microphone_unavailable: "Microphone unavailable",
    requesting_microphone: "Requesting microphone permission…",
    microphone_active: "Microphone is active. Choose Stop at any time.",
    microphone_stopped: "Microphone access stopped and local tracks were released.",
    microphone_hidden_stop: "Microphone access stopped because the window was hidden.",
    microphone_permission_denied: "Microphone permission was denied. Use Start to request access again.",
    microphone_unavailable: "No usable microphone is available.",
    safety_control: "SAFETY CONTROL",
    immediate_stop: "Immediate stop",
    stop_description: "Requests cancellation through the local runtime. It cannot approve or start an action.",
    stop_automation: "Stop automation",
    retry: "Retry",
    cancel: "Cancel",
    forget_confirm_title: "Forget local pairing?",
    forget_confirm_description: "This removes the Device credential from this computer. An administrator must revoke the server record separately.",
    confirm_forget: "Forget pairing",
    loading_runtime: "Loading local runtime state…",
    action_active: "An automation action is active.",
    no_action_active: "No interruptible automation action is active.",
    secure_storage_unavailable: "Windows secure storage is unavailable; pairing is disabled.",
    runtime_unavailable: "Local runtime state is unavailable.",
    creating_pairing: "Creating a short-lived pairing code…",
    pairing_created: "Pairing code created. Approval is required in the Tenant administration page.",
    pairing_start_error: "Pairing could not start. Verify the HTTPS origin and Platform availability.",
    claiming_credential: "Claiming the approved Device credential…",
    pairing_completed: "Pairing completed. Establishing the authenticated Device connection.",
    pairing_claim_error: "Approval is not available yet, or the pairing code expired.",
    removing_credential: "Removing the local Device credential…",
    pairing_removed: "Local pairing removed. An administrator must revoke stale server records separately.",
    pairing_forget_error: "The local Device credential could not be removed securely.",
    requesting_stop: "Requesting immediate stop…",
    stop_requested: "Stop request accepted. Refreshing runtime state.",
    stop_error: "Stop request was rejected. Check the active action and try again.",
    state_offline: "Offline",
    state_connecting: "Connecting",
    state_online: "Online",
    state_degraded: "Degraded",
    state_idle: "Idle",
    state_running: "Running",
    state_awaiting_approval: "Awaiting approval",
    state_stopping: "Stopping",
    state_ready: "Ready",
    state_unavailable: "Unavailable",
    state_unpaired: "Unpaired",
    state_waiting_for_approval: "Waiting for approval",
    state_paired: "Paired",
  },
  zh: {
    skip_to_content: "跳到主要内容",
    language: "语言",
    local_device_control: "本机设备控制",
    lede: "受管控的自动化始终可见，并可随时中止。",
    runtime_status: "运行时状态",
    connection: "连接",
    automation: "自动化",
    automation_host: "自动化主机",
    state_revision: "状态版本",
    checking: "检查中",
    device_trust: "设备信任",
    pair_this_computer: "配对此电脑",
    approve_this_computer: "批准此电脑",
    computer_is_paired: "此电脑已配对",
    pairing_unavailable: "配对不可用",
    device_pairing: "设备配对",
    pairing_help: "请输入安全的平台地址和易于识别的设备名称。",
    platform_origin: "平台 HTTPS 地址",
    platform_origin_help: "必须使用 HTTPS，且不能包含路径、查询参数或片段。",
    device_name: "设备名称",
    default_device_name: "我的 Windows 电脑",
    create_pairing_code: "创建配对码",
    pairing_code: "配对码",
    approve_code_admin: "请在租户管理页面批准此配对码。",
    approve_before: "请在 {time} 前批准此配对码。",
    approved_code: "我已批准配对码",
    local_device_id: "本机设备 ID",
    credential_protected: "设备凭据由 Windows 凭据管理器保护。",
    forget_pairing: "忘记本机配对",
    voice_conversation: "语音对话",
    local_microphone: "本机麦克风",
    voice_description: "仅在你选择“启动”后申请麦克风权限。此基础版本的音频保留在本机，且不能授权自动化。",
    start_voice: "启动麦克风",
    stop_voice: "停止麦克风",
    state_microphone_off: "麦克风已关闭",
    state_requesting_permission: "正在请求权限",
    state_listening: "麦克风使用中",
    state_microphone_stopped: "麦克风已停止",
    state_permission_denied: "权限被拒绝",
    state_microphone_unavailable: "麦克风不可用",
    requesting_microphone: "正在请求麦克风权限…",
    microphone_active: "麦克风正在使用中，可随时选择“停止”。",
    microphone_stopped: "麦克风访问已停止，本机媒体轨道已释放。",
    microphone_hidden_stop: "窗口已隐藏，麦克风访问已停止。",
    microphone_permission_denied: "麦克风权限被拒绝，可再次选择“启动”重新申请。",
    microphone_unavailable: "没有可用的麦克风。",
    safety_control: "安全控制",
    immediate_stop: "立即停止",
    stop_description: "通过本机运行时请求取消；此操作不能批准或启动自动化。",
    stop_automation: "停止自动化",
    retry: "重试",
    cancel: "取消",
    forget_confirm_title: "忘记本机配对？",
    forget_confirm_description: "这会删除此电脑上的设备凭据。管理员仍需另行撤销服务端记录。",
    confirm_forget: "忘记配对",
    loading_runtime: "正在加载本机运行时状态…",
    action_active: "当前有自动化操作正在运行。",
    no_action_active: "当前没有可中止的自动化操作。",
    secure_storage_unavailable: "Windows 安全存储不可用，设备配对已禁用。",
    runtime_unavailable: "无法读取本机运行时状态。",
    creating_pairing: "正在创建短期配对码…",
    pairing_created: "配对码已创建，请在租户管理页面批准。",
    pairing_start_error: "无法开始配对，请检查 HTTPS 地址和平台可用性。",
    claiming_credential: "正在领取已批准的设备凭据…",
    pairing_completed: "配对完成，正在建立已认证的设备连接。",
    pairing_claim_error: "尚未获得批准，或配对码已过期。",
    removing_credential: "正在删除本机设备凭据…",
    pairing_removed: "本机配对已删除，管理员仍需撤销过期的服务端记录。",
    pairing_forget_error: "无法安全删除本机设备凭据。",
    requesting_stop: "正在请求立即停止…",
    stop_requested: "停止请求已接受，正在刷新运行时状态。",
    stop_error: "停止请求被拒绝，请检查活动操作后重试。",
    state_offline: "离线",
    state_connecting: "连接中",
    state_online: "在线",
    state_degraded: "异常",
    state_idle: "空闲",
    state_running: "运行中",
    state_awaiting_approval: "等待批准",
    state_stopping: "停止中",
    state_ready: "就绪",
    state_unavailable: "不可用",
    state_unpaired: "未配对",
    state_waiting_for_approval: "等待批准",
    state_paired: "已配对",
  },
};

const elements = {
  connection: document.querySelector("#connection"),
  automation: document.querySelector("#automation"),
  automationHost: document.querySelector("#automation-host"),
  revision: document.querySelector("#revision"),
  stopButton: document.querySelector("#stop"),
  notice: document.querySelector("#notice"),
  statusPanel: document.querySelector(".status-panel"),
  pairingPanel: document.querySelector(".pairing-panel"),
  pairingForm: document.querySelector("#pairing-form"),
  pairingTitle: document.querySelector("#pairing-title"),
  platformUrl: document.querySelector("#platform-url"),
  deviceName: document.querySelector("#device-name"),
  startPairing: document.querySelector("#start-pairing"),
  pairingPhase: document.querySelector("#pairing-phase"),
  pairingWaiting: document.querySelector("#pairing-waiting"),
  pairingComplete: document.querySelector("#pairing-complete"),
  pairingCode: document.querySelector("#pairing-code"),
  pairingExpiry: document.querySelector("#pairing-expiry"),
  pairedDevice: document.querySelector("#paired-device"),
  claimPairing: document.querySelector("#claim-pairing"),
  forgetPairing: document.querySelector("#forget-pairing"),
  errorRegion: document.querySelector("#error-region"),
  errorMessage: document.querySelector("#error-message"),
  retry: document.querySelector("#retry"),
  forgetConfirm: document.querySelector("#forget-confirm"),
  voiceStatus: document.querySelector("#voice-status"),
  voiceStatusLabel: document.querySelector("#voice-status-label"),
  startVoice: document.querySelector("#start-voice"),
  stopVoice: document.querySelector("#stop-voice"),
  localeEn: document.querySelector("#locale-en"),
  localeZh: document.querySelector("#locale-zh"),
};

const storedLocale = window.localStorage.getItem("trigix.desktop.locale");
let locale = storedLocale === "en" || storedLocale === "zh"
  ? storedLocale
  : (navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en");
let currentRevision = 0;
let shellSnapshot = null;
let pairingSnapshot = null;
let lastCanRequestStop = null;
let activeOperation = null;
let refreshing = false;
let retryAction = null;
let noticeState = { key: "loading_runtime", values: {} };
let errorState = null;
let voiceStream = null;
let voiceState = "idle";
let voiceRequestGeneration = 0;
let voiceRequestPending = false;

function translate(key, values = {}) {
  const template = messages[locale][key] ?? messages.en[key] ?? key;
  return Object.entries(values).reduce(
    (value, [name, replacement]) => value.replaceAll(`{${name}}`, String(replacement)),
    template,
  );
}

function stateLabel(value) {
  return translate(`state_${value}`);
}

function setNotice(key, values = {}) {
  noticeState = { key, values };
  elements.notice.textContent = translate(key, values);
}

function clearError() {
  errorState = null;
  retryAction = null;
  elements.errorRegion.hidden = true;
  elements.errorMessage.textContent = "";
}

function showError(key, retry, focus = true) {
  errorState = { key, retry };
  retryAction = retry;
  elements.errorMessage.textContent = translate(key);
  elements.retry.hidden = typeof retry !== "function";
  elements.errorRegion.hidden = false;
  if (focus) {
    window.requestAnimationFrame(() => {
      if (!elements.retry.hidden) elements.retry.focus();
      else {
        elements.errorRegion.setAttribute("tabindex", "-1");
        elements.errorRegion.focus();
      }
    });
  }
}

function applyLocale(nextLocale, persist = true) {
  const previousDefault = translate("default_device_name");
  locale = nextLocale;
  if (persist) window.localStorage.setItem("trigix.desktop.locale", locale);
  document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
  document.querySelectorAll("[data-i18n]").forEach((element) => {
    element.textContent = translate(element.dataset.i18n);
  });
  document.querySelectorAll("[data-i18n-aria]").forEach((element) => {
    element.setAttribute("aria-label", translate(element.dataset.i18nAria));
  });
  elements.localeEn.setAttribute("aria-pressed", String(locale === "en"));
  elements.localeZh.setAttribute("aria-pressed", String(locale === "zh"));
  if (!elements.deviceName.value || elements.deviceName.value === previousDefault) {
    elements.deviceName.value = translate("default_device_name");
  }
  if (shellSnapshot) renderShell(shellSnapshot);
  if (pairingSnapshot) renderPairing(pairingSnapshot, false);
  renderVoiceState(voiceState);
  setNotice(noticeState.key, noticeState.values);
  if (errorState) showError(errorState.key, errorState.retry, false);
}

function renderShell(snapshot) {
  shellSnapshot = snapshot;
  currentRevision = snapshot.revision;
  elements.connection.textContent = stateLabel(snapshot.connection);
  elements.automation.textContent = stateLabel(snapshot.automation);
  elements.automationHost.textContent = stateLabel(snapshot.automation_host);
  elements.revision.textContent = String(snapshot.revision);
  elements.stopButton.disabled = activeOperation === "stop" || !snapshot.can_request_stop;
  elements.statusPanel.setAttribute("aria-busy", "false");
  if (activeOperation === null && lastCanRequestStop !== snapshot.can_request_stop) {
    setNotice(snapshot.can_request_stop ? "action_active" : "no_action_active");
  }
  lastCanRequestStop = snapshot.can_request_stop;
}

function pairingTitleKey(phase) {
  return {
    unpaired: "pair_this_computer",
    waiting_for_approval: "approve_this_computer",
    paired: "computer_is_paired",
    unavailable: "pairing_unavailable",
  }[phase] ?? "device_pairing";
}

function renderPairing(snapshot, focusPhase) {
  const phaseChanged = pairingSnapshot?.phase !== snapshot.phase;
  pairingSnapshot = snapshot;
  elements.pairingPhase.textContent = stateLabel(snapshot.phase);
  elements.pairingTitle.textContent = translate(pairingTitleKey(snapshot.phase));
  elements.pairingForm.hidden = snapshot.phase !== "unpaired";
  elements.pairingWaiting.hidden = snapshot.phase !== "waiting_for_approval";
  elements.pairingComplete.hidden = snapshot.phase !== "paired";

  if (snapshot.phase === "waiting_for_approval") {
    elements.pairingCode.textContent = snapshot.pairing_code;
    const expiry = new Date(snapshot.expires_at_unix_seconds * 1000);
    elements.pairingExpiry.textContent = translate("approve_before", {
      time: expiry.toLocaleTimeString(locale === "zh" ? "zh-CN" : "en"),
    });
  }
  if (snapshot.phase === "paired") elements.pairedDevice.textContent = snapshot.device_id;
  if (snapshot.phase === "unavailable") {
    elements.pairingForm.hidden = true;
    elements.pairingWaiting.hidden = true;
    elements.pairingComplete.hidden = true;
    if (activeOperation === null) setNotice("secure_storage_unavailable");
  }
  if (focusPhase && phaseChanged) {
    window.requestAnimationFrame(() => elements.pairingTitle.focus());
  }
}

function renderVoiceState(state) {
  voiceState = state;
  const statusKey = {
    idle: "state_microphone_off",
    requesting_permission: "state_requesting_permission",
    listening: "state_listening",
    stopped: "state_microphone_stopped",
    permission_denied: "state_permission_denied",
    unavailable: "state_microphone_unavailable",
  }[state] ?? "state_microphone_unavailable";
  const active = state === "requesting_permission" || state === "listening";
  elements.voiceStatus.dataset.state = state;
  elements.voiceStatusLabel.textContent = translate(statusKey);
  elements.startVoice.disabled = active;
  elements.stopVoice.disabled = !active;
}

function stopVoiceSession(state = "stopped", noticeKey = "microphone_stopped") {
  voiceRequestGeneration += 1;
  voiceRequestPending = false;
  const stream = voiceStream;
  voiceStream = null;
  if (stream) stream.getTracks().forEach((track) => track.stop());
  renderVoiceState(state);
  if (noticeKey) setNotice(noticeKey);
}

function setOperation(name, busy) {
  activeOperation = busy ? name : null;
  elements.pairingPanel.setAttribute("aria-busy", String(busy));
  elements.startPairing.disabled = busy;
  elements.claimPairing.disabled = busy;
  elements.forgetPairing.disabled = busy;
  elements.stopButton.disabled = busy || !shellSnapshot?.can_request_stop;
}

async function refresh(force = false) {
  if (refreshing || document.hidden || (activeOperation && !force)) return;
  refreshing = true;
  try {
    const [shell, pairing] = await Promise.all([
      window.__TAURI__.core.invoke("shell_status"),
      window.__TAURI__.core.invoke("pairing_status"),
    ]);
    renderShell(shell);
    renderPairing(pairing, false);
    if (retryAction === refresh) clearError();
  } catch (_error) {
    elements.stopButton.disabled = true;
    elements.statusPanel.setAttribute("aria-busy", "false");
    showError("runtime_unavailable", refresh, errorState?.key !== "runtime_unavailable");
  } finally {
    refreshing = false;
  }
}

elements.pairingForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (activeOperation) return;
  clearError();
  setOperation("pair", true);
  setNotice("creating_pairing");
  try {
    const snapshot = await window.__TAURI__.core.invoke("start_device_pairing", {
      input: {
        platform_url: elements.platformUrl.value,
        display_name: elements.deviceName.value,
      },
    });
    renderPairing(snapshot, true);
    setNotice("pairing_created");
  } catch (_error) {
    showError("pairing_start_error", () => elements.pairingForm.requestSubmit());
  } finally {
    setOperation("pair", false);
  }
});

elements.claimPairing.addEventListener("click", async () => {
  if (activeOperation) return;
  clearError();
  setOperation("claim", true);
  setNotice("claiming_credential");
  try {
    const snapshot = await window.__TAURI__.core.invoke("complete_device_pairing");
    renderPairing(snapshot, true);
    setNotice("pairing_completed");
  } catch (_error) {
    showError("pairing_claim_error", () => elements.claimPairing.click());
  } finally {
    setOperation("claim", false);
  }
});

elements.forgetPairing.addEventListener("click", () => {
  if (!activeOperation) elements.forgetConfirm.showModal();
});

elements.forgetConfirm.addEventListener("close", async () => {
  if (elements.forgetConfirm.returnValue !== "confirm" || activeOperation) return;
  clearError();
  setOperation("forget", true);
  setNotice("removing_credential");
  try {
    const snapshot = await window.__TAURI__.core.invoke("forget_device_pairing");
    renderPairing(snapshot, true);
    setNotice("pairing_removed");
  } catch (_error) {
    showError("pairing_forget_error", () => elements.forgetPairing.click());
  } finally {
    setOperation("forget", false);
  }
});

elements.stopButton.addEventListener("click", async () => {
  if (activeOperation || !shellSnapshot?.can_request_stop) return;
  clearError();
  setOperation("stop", true);
  setNotice("requesting_stop");
  const requestId = `stop-${crypto.randomUUID()}`;
  try {
    await window.__TAURI__.core.invoke("request_automation_stop", {
      request: { request_id: requestId, observed_revision: currentRevision },
    });
    setNotice("stop_requested");
    await refresh(true);
  } catch (_error) {
    showError("stop_error", refresh);
  } finally {
    setOperation("stop", false);
  }
});

elements.startVoice.addEventListener("click", async () => {
  if (voiceStream || voiceRequestPending) return;
  clearError();
  voiceRequestGeneration += 1;
  const requestGeneration = voiceRequestGeneration;
  voiceRequestPending = true;
  renderVoiceState("requesting_permission");
  setNotice("requesting_microphone");
  try {
    if (!navigator.mediaDevices?.getUserMedia) throw new Error("microphone unavailable");
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
      video: false,
    });
    if (requestGeneration !== voiceRequestGeneration || document.hidden) {
      stream.getTracks().forEach((track) => track.stop());
      return;
    }
    const audioTracks = stream
      .getAudioTracks()
      .filter((track) => track.readyState === "live");
    if (audioTracks.length === 0) {
      stream.getTracks().forEach((track) => track.stop());
      throw new Error("microphone unavailable");
    }
    voiceStream = stream;
    voiceRequestPending = false;
    audioTracks.forEach((track) => {
      track.addEventListener("ended", () => {
        if (voiceStream === stream) stopVoiceSession("unavailable", "microphone_unavailable");
      }, { once: true });
    });
    renderVoiceState("listening");
    setNotice("microphone_active");
  } catch (error) {
    if (requestGeneration !== voiceRequestGeneration) return;
    voiceRequestPending = false;
    const denied = error?.name === "NotAllowedError" || error?.name === "SecurityError";
    renderVoiceState(denied ? "permission_denied" : "unavailable");
    setNotice(denied ? "microphone_permission_denied" : "microphone_unavailable");
  }
});

elements.stopVoice.addEventListener("click", () => stopVoiceSession());

elements.retry.addEventListener("click", async () => {
  const action = retryAction;
  clearError();
  if (action) await action();
});

elements.localeEn.addEventListener("click", () => applyLocale("en"));
elements.localeZh.addEventListener("click", () => applyLocale("zh"));

document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    if (voiceStream || voiceRequestPending) {
      stopVoiceSession("stopped", "microphone_hidden_stop");
    }
  } else {
    void refresh();
  }
});

window.addEventListener("pagehide", () => stopVoiceSession("stopped", null));
window.addEventListener("beforeunload", () => stopVoiceSession("stopped", null));

applyLocale(locale, false);
void refresh();
window.setInterval(() => {
  if (!document.hidden) void refresh();
}, 2000);
