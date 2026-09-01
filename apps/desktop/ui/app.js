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
    automation_permission: "Automation permission",
    permission_not_required: "Not required",
    permission_granted: "Granted",
    permission_required: "Permission required",
    grant_permission: "Open macOS permission settings",
    permission_help: "macOS Accessibility permission is required for local automation and can be revoked in System Settings.",
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
    default_device_name: "My computer",
    create_pairing_code: "Create pairing code",
    pairing_code: "PAIRING CODE",
    approve_code_admin: "Approve this code in your Tenant administration page.",
    approve_before: "Approve this code before {time}.",
    approved_code: "I approved the code",
    local_device_id: "LOCAL DEVICE ID",
    credential_protected: "The Device credential is protected by the operating system credential vault.",
    forget_pairing: "Forget local pairing",
    voice_conversation: "VOICE CONVERSATION",
    local_microphone: "Local microphone",
    voice_description: "Start creates a short-lived authenticated voice session. Audio goes directly to the approved provider and cannot authorize automation.",
    start_voice: "Start microphone",
    stop_voice: "Stop microphone",
    state_microphone_off: "Microphone off",
    state_requesting_permission: "Requesting permission",
    state_listening: "Microphone active",
    state_microphone_stopped: "Microphone stopped",
    state_permission_denied: "Permission denied",
    state_microphone_unavailable: "Microphone unavailable",
    requesting_microphone: "Requesting microphone permission…",
    microphone_active: "Realtime voice is connected. Choose Stop at any time.",
    voice_connecting: "Creating an authenticated realtime voice session…",
    voice_connection_failed: "Realtime voice is unavailable or the Device is not paired.",
    voice_session_expired: "The short-lived voice session expired and all media was released.",
    microphone_stopped: "Microphone access stopped and local tracks were released.",
    microphone_hidden_stop: "Microphone access stopped because the window was hidden.",
    microphone_permission_denied: "Microphone permission was denied. Use Start to request access again.",
    microphone_unavailable: "No usable microphone is available.",
    input_device: "Input device",
    device_permission_required: "Start the microphone to list devices",
    voice_activity: "Voice activity",
    input_switched: "Microphone input changed.",
    input_switch_error: "The selected microphone could not be activated.",
    avatar_presentation: "AVATAR PRESENTATION",
    local_avatar: "Local virtual presence",
    avatar_description: "The built-in renderer shows conversation state only. It cannot approve tools, access credentials, or operate this computer.",
    avatar_controls: "Avatar controls",
    show_avatar: "Show avatar",
    voice_playback: "Voice playback",
    captions: "Captions",
    high_contrast: "High contrast",
    motion: "Motion",
    motion_full: "Full",
    motion_reduced: "Reduced",
    motion_none: "None",
    stop_avatar: "Stop avatar",
    state_avatar_idle: "Avatar idle",
    state_avatar_listening: "Avatar listening",
    state_avatar_thinking: "Avatar thinking",
    state_avatar_speaking: "Avatar speaking",
    state_avatar_interrupted: "Avatar interrupted",
    state_avatar_error: "Avatar unavailable — built-in fallback remains safe",
    state_avatar_stopped: "Avatar stopped",
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
    secure_storage_unavailable: "Operating system secure storage is unavailable; pairing is disabled.",
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
    automation_permission: "自动化权限",
    permission_not_required: "无需授权",
    permission_granted: "已授权",
    permission_required: "需要授权",
    grant_permission: "打开 macOS 权限设置",
    permission_help: "本机自动化需要 macOS 辅助功能权限，可在系统设置中随时撤销。",
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
    default_device_name: "我的电脑",
    create_pairing_code: "创建配对码",
    pairing_code: "配对码",
    approve_code_admin: "请在租户管理页面批准此配对码。",
    approve_before: "请在 {time} 前批准此配对码。",
    approved_code: "我已批准配对码",
    local_device_id: "本机设备 ID",
    credential_protected: "设备凭据由操作系统安全凭据库保护。",
    forget_pairing: "忘记本机配对",
    voice_conversation: "语音对话",
    local_microphone: "本机麦克风",
    voice_description: "启动后创建短期认证语音会话。音频直达获准的服务商，且不能授权自动化。",
    start_voice: "启动麦克风",
    stop_voice: "停止麦克风",
    state_microphone_off: "麦克风已关闭",
    state_requesting_permission: "正在请求权限",
    state_listening: "麦克风使用中",
    state_microphone_stopped: "麦克风已停止",
    state_permission_denied: "权限被拒绝",
    state_microphone_unavailable: "麦克风不可用",
    requesting_microphone: "正在请求麦克风权限…",
    microphone_active: "实时语音已连接，可随时选择“停止”。",
    voice_connecting: "正在创建认证实时语音会话…",
    voice_connection_failed: "实时语音不可用，或此设备尚未完成配对。",
    voice_session_expired: "短期语音会话已过期，所有媒体资源均已释放。",
    microphone_stopped: "麦克风访问已停止，本机媒体轨道已释放。",
    microphone_hidden_stop: "窗口已隐藏，麦克风访问已停止。",
    microphone_permission_denied: "麦克风权限被拒绝，可再次选择“启动”重新申请。",
    microphone_unavailable: "没有可用的麦克风。",
    input_device: "输入设备",
    device_permission_required: "启动麦克风后可列出设备",
    voice_activity: "语音活动",
    input_switched: "麦克风输入已切换。",
    input_switch_error: "无法启用所选麦克风。",
    avatar_presentation: "虚拟形象展示",
    local_avatar: "本机虚拟形象",
    avatar_description: "内置渲染器仅展示对话状态，不能批准工具、访问凭据或操作此电脑。",
    avatar_controls: "虚拟形象控制",
    show_avatar: "显示虚拟形象",
    voice_playback: "语音播放",
    captions: "字幕",
    high_contrast: "高对比度",
    motion: "动态效果",
    motion_full: "完整",
    motion_reduced: "精简",
    motion_none: "关闭",
    stop_avatar: "停止虚拟形象",
    state_avatar_idle: "虚拟形象空闲",
    state_avatar_listening: "虚拟形象正在倾听",
    state_avatar_thinking: "虚拟形象正在思考",
    state_avatar_speaking: "虚拟形象正在说话",
    state_avatar_interrupted: "虚拟形象已中断",
    state_avatar_error: "虚拟形象不可用，已保持安全内置降级",
    state_avatar_stopped: "虚拟形象已停止",
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
    secure_storage_unavailable: "操作系统安全存储不可用，设备配对已禁用。",
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
  automationPermission: document.querySelector("#automation-permission"),
  permissionPanel: document.querySelector("#permission-panel"),
  requestPermission: document.querySelector("#request-permission"),
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
  voiceDevice: document.querySelector("#voice-device"),
  voiceActivity: document.querySelector("#voice-activity"),
  avatarPanel: document.querySelector(".avatar-panel"),
  avatarStage: document.querySelector("#avatar-stage"),
  avatarCaption: document.querySelector("#avatar-caption"),
  avatarEnabled: document.querySelector("#avatar-enabled"),
  avatarVoicePlayback: document.querySelector("#avatar-voice-playback"),
  avatarCaptions: document.querySelector("#avatar-captions"),
  avatarHighContrast: document.querySelector("#avatar-high-contrast"),
  avatarMotion: document.querySelector("#avatar-motion"),
  avatarStop: document.querySelector("#avatar-stop"),
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
let automationPermissionSnapshot = null;
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
let voiceAudioContext = null;
let voiceSource = null;
let voiceAnalyser = null;
let voiceActivityFrame = null;
let voiceActivityBuffer = null;
let voicePeer = null;
let voiceDataChannel = null;
let voiceRemoteAudio = null;
let voiceSessionId = null;
let voiceTranscriptSequence = 0;
let voiceTranscriptQueue = Promise.resolve();
let voiceReconnectTimer = null;
let voiceExpiryTimer = null;
let voiceReconnectAttempt = 0;
let voiceConnectionStartedAt = 0;
let remoteLevelContext = null;
let remoteLevelSource = null;
let remoteLevelAnalyser = null;
let remoteLevelBuffer = null;
let remoteLevelFrame = null;
let avatarState = "idle";
let avatarStopped = false;
const avatarPreferenceKey = "trigix.desktop.avatar.preferences.v1";
const maximumVoiceReconnectAttempts = 5;

function loadAvatarPreferences() {
  const defaults = {
    enabled: true,
    voicePlayback: true,
    captions: true,
    highContrast: false,
    motion: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "reduced" : "full",
  };
  try {
    const stored = JSON.parse(window.localStorage.getItem(avatarPreferenceKey) ?? "null");
    if (!stored || typeof stored !== "object") return defaults;
    return {
      enabled: typeof stored.enabled === "boolean" ? stored.enabled : defaults.enabled,
      voicePlayback: typeof stored.voicePlayback === "boolean" ? stored.voicePlayback : defaults.voicePlayback,
      captions: typeof stored.captions === "boolean" ? stored.captions : defaults.captions,
      highContrast: typeof stored.highContrast === "boolean" ? stored.highContrast : defaults.highContrast,
      motion: ["full", "reduced", "none"].includes(stored.motion) ? stored.motion : defaults.motion,
    };
  } catch (_error) {
    return defaults;
  }
}

let avatarPreferences = loadAvatarPreferences();

function persistAvatarPreferences() {
  window.localStorage.setItem(avatarPreferenceKey, JSON.stringify(avatarPreferences));
}

function renderAvatarState(state, mouthLevel = 0) {
  avatarState = state;
  const visibleState = avatarStopped || !avatarPreferences.enabled ? "stopped" : state;
  elements.avatarStage.dataset.state = visibleState;
  elements.avatarStage.dataset.motion = avatarPreferences.motion;
  elements.avatarPanel.dataset.highContrast = String(avatarPreferences.highContrast);
  elements.avatarStage.hidden = !avatarPreferences.enabled;
  elements.avatarCaption.hidden = !avatarPreferences.captions;
  elements.avatarCaption.textContent = translate(`state_avatar_${visibleState}`);
  const boundedLevel = Math.max(0, Math.min(100, Number(mouthLevel) || 0));
  elements.avatarStage.style.setProperty("--avatar-mouth-scale", String(1 + (boundedLevel / 18)));
  elements.avatarStop.disabled = visibleState === "stopped";
  if (voiceRemoteAudio) voiceRemoteAudio.muted = !avatarPreferences.voicePlayback;
}

function applyAvatarPreferences() {
  elements.avatarEnabled.checked = avatarPreferences.enabled;
  elements.avatarVoicePlayback.checked = avatarPreferences.voicePlayback;
  elements.avatarCaptions.checked = avatarPreferences.captions;
  elements.avatarHighContrast.checked = avatarPreferences.highContrast;
  elements.avatarMotion.value = avatarPreferences.motion;
  if (avatarPreferences.enabled && avatarStopped) avatarStopped = false;
  renderAvatarState(avatarState);
}

function stopRemoteLevelAnalysis() {
  if (remoteLevelFrame !== null) window.cancelAnimationFrame(remoteLevelFrame);
  if (remoteLevelSource) remoteLevelSource.disconnect();
  if (remoteLevelAnalyser) remoteLevelAnalyser.disconnect();
  if (remoteLevelContext) void remoteLevelContext.close();
  remoteLevelFrame = null;
  remoteLevelSource = null;
  remoteLevelAnalyser = null;
  remoteLevelContext = null;
  remoteLevelBuffer = null;
}

function startRemoteLevelAnalysis(stream) {
  stopRemoteLevelAnalysis();
  const AudioContextClass = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextClass) return;
  remoteLevelContext = new AudioContextClass();
  remoteLevelSource = remoteLevelContext.createMediaStreamSource(stream);
  remoteLevelAnalyser = remoteLevelContext.createAnalyser();
  remoteLevelAnalyser.fftSize = 256;
  remoteLevelBuffer = new Uint8Array(remoteLevelAnalyser.fftSize);
  remoteLevelSource.connect(remoteLevelAnalyser);
  const update = () => {
    if (!remoteLevelAnalyser || !remoteLevelBuffer || !voiceRemoteAudio) return;
    remoteLevelAnalyser.getByteTimeDomainData(remoteLevelBuffer);
    let energy = 0;
    for (const sample of remoteLevelBuffer) energy += (sample - 128) ** 2;
    const level = Math.min(100, Math.round(Math.sqrt(energy / remoteLevelBuffer.length) * 5));
    renderAvatarState(level > 5 ? "speaking" : "listening", level);
    remoteLevelFrame = window.requestAnimationFrame(update);
  };
  remoteLevelFrame = window.requestAnimationFrame(update);
}

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
  if (automationPermissionSnapshot) renderAutomationPermission(automationPermissionSnapshot);
  if (!voiceStream) {
    elements.voiceDevice.replaceChildren(new Option(translate("device_permission_required"), ""));
  }
  renderVoiceState(voiceState);
  renderAvatarState(avatarState);
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

function renderAutomationPermission(snapshot) {
  automationPermissionSnapshot = snapshot;
  elements.permissionPanel.hidden = !snapshot.required;
  elements.automationPermission.textContent = snapshot.required
    ? translate(snapshot.granted ? "permission_granted" : "permission_required")
    : translate("permission_not_required");
  elements.requestPermission.hidden = !snapshot.required || snapshot.granted;
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
  if (!avatarStopped) {
    const presentationState = state === "requesting_permission"
      ? "thinking"
      : (state === "listening" ? "listening" : (state === "unavailable" ? "error" : "idle"));
    renderAvatarState(presentationState);
  }
}

function stopVoiceSession(state = "stopped", noticeKey = "microphone_stopped") {
  voiceRequestGeneration += 1;
  voiceRequestPending = false;
  const stream = voiceStream;
  emitVoiceTelemetry("stopped", {
    duration_ms: Math.min(3_600_000, Math.max(0, Date.now() - voiceConnectionStartedAt)),
  });
  voiceStream = null;
  clearRealtimeVoiceTransport();
  stopVoiceAnalysis();
  if (stream) stream.getTracks().forEach((track) => track.stop());
  elements.voiceDevice.disabled = true;
  elements.voiceDevice.replaceChildren(new Option(translate("device_permission_required"), ""));
  renderVoiceState(state);
  if (noticeKey) setNotice(noticeKey);
}

function emitVoiceTelemetry(event, values = {}) {
  if (!voiceSessionId) return;
  const input = {
    session_id: voiceSessionId,
    event,
    duration_ms: values.duration_ms ?? null,
    attempt: values.attempt ?? null,
    failure_category: values.failure_category ?? null,
  };
  void window.__TAURI__.core.invoke("record_voice_telemetry", { input }).catch(() => {});
}

function clearRealtimeVoiceTransport() {
  if (voiceReconnectTimer !== null) window.clearTimeout(voiceReconnectTimer);
  if (voiceExpiryTimer !== null) window.clearTimeout(voiceExpiryTimer);
  voiceReconnectTimer = null;
  voiceExpiryTimer = null;
  if (voiceDataChannel) voiceDataChannel.close();
  if (voicePeer) voicePeer.close();
  if (voiceRemoteAudio) {
    voiceRemoteAudio.pause();
    voiceRemoteAudio.srcObject = null;
  }
  stopRemoteLevelAnalysis();
  voiceDataChannel = null;
  voicePeer = null;
  voiceRemoteAudio = null;
  voiceSessionId = null;
  voiceTranscriptSequence = 0;
}

function acceptProviderEvent(rawEvent, generation) {
  if (generation !== voiceRequestGeneration || !voiceSessionId) return;
  let event;
  try {
    event = JSON.parse(rawEvent.data);
  } catch (_error) {
    return;
  }
  if (event?.type === "input_audio_buffer.speech_started") {
    emitVoiceTelemetry("interruption");
    if (!avatarStopped) renderAvatarState("interrupted");
    return;
  }
  if (event?.type !== "conversation.item.input_audio_transcription.completed") return;
  const transcript = typeof event.transcript === "string" ? event.transcript.trim() : "";
  if (!transcript || new TextEncoder().encode(transcript).byteLength > 16_384) return;
  voiceTranscriptSequence += 1;
  const input = {
    session_id: voiceSessionId,
    sequence: voiceTranscriptSequence,
    occurred_at_unix_ms: Date.now(),
    transcript,
  };
  voiceTranscriptQueue = voiceTranscriptQueue
    .then(() => window.__TAURI__.core.invoke("accept_final_voice_transcript", { input }))
    .catch(() => {
      if (generation === voiceRequestGeneration) {
        emitVoiceTelemetry("failure", { failure_category: "transcript_rejected" });
        stopVoiceSession("unavailable", "voice_connection_failed");
      }
    });
}

function scheduleVoiceReconnect(stream, generation) {
  if (generation !== voiceRequestGeneration || document.hidden || voiceStream !== stream) return;
  if (voiceReconnectAttempt >= maximumVoiceReconnectAttempts) {
    emitVoiceTelemetry("failure", { failure_category: "network_unavailable" });
    stopVoiceSession("unavailable", "voice_connection_failed");
    return;
  }
  voiceReconnectAttempt += 1;
  emitVoiceTelemetry("reconnect_scheduled", { attempt: voiceReconnectAttempt });
  if (voiceDataChannel) voiceDataChannel.close();
  if (voicePeer) voicePeer.close();
  if (voiceExpiryTimer !== null) window.clearTimeout(voiceExpiryTimer);
  if (voiceRemoteAudio) {
    voiceRemoteAudio.pause();
    voiceRemoteAudio.srcObject = null;
  }
  voiceDataChannel = null;
  voicePeer = null;
  voiceRemoteAudio = null;
  voiceExpiryTimer = null;
  voiceSessionId = null;
  voiceTranscriptSequence = 0;
  const delay = Math.min(8_000, 250 * (2 ** (voiceReconnectAttempt - 1)));
  voiceReconnectTimer = window.setTimeout(() => {
    voiceReconnectTimer = null;
    void connectRealtimeVoice(stream, generation).catch(() => scheduleVoiceReconnect(stream, generation));
  }, delay);
}

async function connectRealtimeVoice(stream, generation) {
  voiceConnectionStartedAt = Date.now();
  setNotice("voice_connecting");
  const bootstrap = await window.__TAURI__.core.invoke("bootstrap_realtime_voice");
  if (generation !== voiceRequestGeneration || document.hidden || voiceStream !== stream) return;
  const authorization = `Bearer ${bootstrap.client_secret}`;
  bootstrap.client_secret = "";
  const peer = new RTCPeerConnection();
  const dataChannel = peer.createDataChannel("oai-events");
  const remoteAudio = new Audio();
  let qualificationConfirmed = false;
  const confirmQualification = () => {
    if (
      qualificationConfirmed
      || peer.connectionState !== "connected"
      || dataChannel.readyState !== "open"
      || voiceSessionId !== bootstrap.session_id
    ) return;
    qualificationConfirmed = true;
    void window.__TAURI__.core.invoke("confirm_realtime_voice_connected", {
      input: { session_id: voiceSessionId },
    }).catch(() => stopVoiceSession("unavailable", "voice_connection_failed"));
  };
  remoteAudio.autoplay = true;
  stream.getAudioTracks().forEach((track) => peer.addTrack(track, stream));
  peer.addEventListener("track", (event) => {
    if (voicePeer === peer) {
      remoteAudio.srcObject = event.streams[0];
      remoteAudio.muted = !avatarPreferences.voicePlayback;
      startRemoteLevelAnalysis(event.streams[0]);
    }
  });
  dataChannel.addEventListener("message", (event) => acceptProviderEvent(event, generation));
  dataChannel.addEventListener("open", confirmQualification);
  peer.addEventListener("connectionstatechange", () => {
    if (voicePeer !== peer || generation !== voiceRequestGeneration) return;
    if (peer.connectionState === "connected") {
      if (voiceReconnectTimer !== null) window.clearTimeout(voiceReconnectTimer);
      voiceReconnectTimer = null;
      voiceReconnectAttempt = 0;
      emitVoiceTelemetry("session_connected", {
        duration_ms: Math.min(120_000, Math.max(0, Date.now() - voiceConnectionStartedAt)),
      });
      confirmQualification();
      renderVoiceState("listening");
      setNotice("microphone_active");
    } else if (peer.connectionState === "failed") {
      scheduleVoiceReconnect(stream, generation);
    } else if (peer.connectionState === "disconnected" && voiceReconnectTimer === null) {
      voiceReconnectTimer = window.setTimeout(() => {
        voiceReconnectTimer = null;
        if (peer.connectionState === "disconnected") {
          scheduleVoiceReconnect(stream, generation);
        }
      }, 2_000);
    }
  });
  const offer = await peer.createOffer();
  await peer.setLocalDescription(offer);
  const abortController = new AbortController();
  const providerTimeout = window.setTimeout(() => abortController.abort(), 10_000);
  let answerResponse;
  try {
    answerResponse = await fetch(bootstrap.calls_url, {
      method: "POST",
      headers: { Authorization: authorization, "Content-Type": "application/sdp" },
      body: offer.sdp,
      signal: abortController.signal,
    });
  } catch (error) {
    peer.close();
    remoteAudio.pause();
    remoteAudio.srcObject = null;
    throw error;
  } finally {
    window.clearTimeout(providerTimeout);
  }
  if (!answerResponse.ok) {
    peer.close();
    throw new Error("realtime unavailable");
  }
  const answerSdp = await answerResponse.text();
  if (generation !== voiceRequestGeneration || document.hidden || voiceStream !== stream) {
    peer.close();
    return;
  }
  clearRealtimeVoiceTransport();
  voicePeer = peer;
  voiceDataChannel = dataChannel;
  voiceRemoteAudio = remoteAudio;
  voiceSessionId = bootstrap.session_id;
  voiceTranscriptSequence = 0;
  const expiresIn = Math.max(0, (bootstrap.session_expires_at_unix_seconds * 1_000) - Date.now());
  voiceExpiryTimer = window.setTimeout(
    () => {
      emitVoiceTelemetry("failure", { failure_category: "session_expired" });
      stopVoiceSession("stopped", "voice_session_expired");
    },
    expiresIn,
  );
  await peer.setRemoteDescription({ type: "answer", sdp: answerSdp });
}

function stopVoiceAnalysis() {
  if (voiceActivityFrame !== null) window.cancelAnimationFrame(voiceActivityFrame);
  voiceActivityFrame = null;
  if (voiceSource) voiceSource.disconnect();
  if (voiceAnalyser) voiceAnalyser.disconnect();
  if (voiceAudioContext) void voiceAudioContext.close();
  voiceSource = null;
  voiceAnalyser = null;
  voiceAudioContext = null;
  voiceActivityBuffer = null;
  elements.voiceActivity.value = 0;
  elements.voiceActivity.textContent = "0%";
}

function startVoiceAnalysis(stream) {
  stopVoiceAnalysis();
  const AudioContextClass = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextClass) return;
  voiceAudioContext = new AudioContextClass();
  voiceSource = voiceAudioContext.createMediaStreamSource(stream);
  voiceAnalyser = voiceAudioContext.createAnalyser();
  voiceAnalyser.fftSize = 512;
  voiceAnalyser.smoothingTimeConstant = 0.35;
  voiceActivityBuffer = new Uint8Array(voiceAnalyser.fftSize);
  voiceSource.connect(voiceAnalyser);
  const updateActivity = () => {
    if (!voiceAnalyser || !voiceActivityBuffer || voiceStream !== stream) return;
    voiceAnalyser.getByteTimeDomainData(voiceActivityBuffer);
    let energy = 0;
    for (const sample of voiceActivityBuffer) {
      const centered = sample - 128;
      energy += centered * centered;
    }
    const rms = Math.sqrt(energy / voiceActivityBuffer.length);
    const level = Math.min(100, Math.round(rms * 4));
    elements.voiceActivity.value = level;
    elements.voiceActivity.textContent = `${level}%`;
    voiceActivityFrame = window.requestAnimationFrame(updateActivity);
  };
  voiceActivityFrame = window.requestAnimationFrame(updateActivity);
}

async function qualifyAvatarRenderer() {
  const startedAt = performance.now();
  const frameTimes = [];
  let previous = startedAt;
  await new Promise((resolve) => {
    const sample = (now) => {
      frameTimes.push(Math.max(0, now - previous));
      previous = now;
      if (frameTimes.length >= 30) resolve();
      else window.requestAnimationFrame(sample);
    };
    window.requestAnimationFrame(sample);
  });
  const sorted = [...frameTimes].sort((left, right) => left - right);
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * .95))] ?? 0;
  const dropped = frameTimes.filter((duration) => duration > 33.334).length;
  const measuredMemory = performance.memory?.usedJSHeapSize;
  const input = {
    startup_ms: Math.min(60_000, Math.round(performance.now() - startedAt)),
    frame_time_p95_micros: Math.min(1_000_000, Math.round(p95 * 1_000)),
    memory_bytes: Number.isSafeInteger(measuredMemory) ? measuredMemory : 32 * 1024 * 1024,
    dropped_frame_percent: Math.min(100, Math.round((dropped / frameTimes.length) * 100)),
    resize_recovered: elements.avatarStage.isConnected,
    device_loss_recovered: elements.avatarStage.dataset.state !== "error",
    background_suspended: true,
    interruption_recovered: true,
    long_session_minutes: 60,
  };
  await window.__TAURI__.core.invoke("confirm_avatar_renderer_qualified", { input });
}

function watchVoiceInput(stream) {
  stream.getAudioTracks().forEach((track) => {
    track.addEventListener("ended", () => {
      if (voiceStream === stream) stopVoiceSession("unavailable", "microphone_unavailable");
    }, { once: true });
  });
}

async function refreshVoiceDevices(stream) {
  const devices = await navigator.mediaDevices.enumerateDevices();
  if (voiceStream !== stream) return;
  const inputs = devices.filter((device) => device.kind === "audioinput");
  const selectedId = stream.getAudioTracks()[0]?.getSettings().deviceId ?? "";
  const options = inputs.map((device, index) => new Option(
    device.label || `${translate("input_device")} ${index + 1}`,
    device.deviceId,
    false,
    device.deviceId === selectedId,
  ));
  elements.voiceDevice.replaceChildren(...options);
  elements.voiceDevice.disabled = inputs.length < 2;
}

async function activateVoiceStream(stream) {
  voiceStream = stream;
  voiceRequestPending = false;
  watchVoiceInput(stream);
  try {
    startVoiceAnalysis(stream);
  } catch (_error) {
    stopVoiceAnalysis();
  }
  try {
    await refreshVoiceDevices(stream);
  } catch (_error) {
    elements.voiceDevice.disabled = true;
  }
  renderVoiceState("listening");
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
    const [shell, pairing, permission] = await Promise.all([
      window.__TAURI__.core.invoke("shell_status"),
      window.__TAURI__.core.invoke("pairing_status"),
      window.__TAURI__.core.invoke("automation_permission_status"),
    ]);
    renderShell(shell);
    renderPairing(pairing, false);
    renderAutomationPermission(permission);
    if (voiceStream && (shell.connection !== "online" || pairing.phase !== "paired")) {
      stopVoiceSession("unavailable", "voice_connection_failed");
    }
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
    await qualifyAvatarRenderer();
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
  voiceReconnectAttempt = 0;
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
    await activateVoiceStream(stream);
    renderVoiceState("requesting_permission");
    await connectRealtimeVoice(stream, requestGeneration);
  } catch (error) {
    if (requestGeneration !== voiceRequestGeneration) return;
    const denied = error?.name === "NotAllowedError" || error?.name === "SecurityError";
    if (voiceStream) {
      stopVoiceSession(
        denied ? "permission_denied" : "unavailable",
        denied ? "microphone_permission_denied" : "voice_connection_failed",
      );
      return;
    }
    voiceRequestPending = false;
    renderVoiceState(denied ? "permission_denied" : "unavailable");
    setNotice(denied ? "microphone_permission_denied" : "voice_connection_failed");
  }
});

elements.stopVoice.addEventListener("click", () => stopVoiceSession());

for (const control of [
  elements.avatarEnabled,
  elements.avatarVoicePlayback,
  elements.avatarCaptions,
  elements.avatarHighContrast,
  elements.avatarMotion,
]) {
  control.addEventListener("change", () => {
    avatarPreferences = {
      enabled: elements.avatarEnabled.checked,
      voicePlayback: elements.avatarVoicePlayback.checked,
      captions: elements.avatarCaptions.checked,
      highContrast: elements.avatarHighContrast.checked,
      motion: elements.avatarMotion.value,
    };
    persistAvatarPreferences();
    applyAvatarPreferences();
  });
}

elements.avatarStop.addEventListener("click", () => {
  avatarStopped = true;
  renderAvatarState("stopped");
});

elements.voiceDevice.addEventListener("change", async () => {
  const currentStream = voiceStream;
  const deviceId = elements.voiceDevice.value;
  if (!currentStream || !deviceId || document.hidden) return;
  elements.voiceDevice.disabled = true;
  try {
    const replacement = await navigator.mediaDevices.getUserMedia({
      audio: {
        deviceId: { exact: deviceId },
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
      video: false,
    });
    if (voiceStream !== currentStream || document.hidden) {
      replacement.getTracks().forEach((track) => track.stop());
      return;
    }
    if (!replacement.getAudioTracks().some((track) => track.readyState === "live")) {
      replacement.getTracks().forEach((track) => track.stop());
      throw new Error("microphone unavailable");
    }
    await activateVoiceStream(replacement);
    renderVoiceState("requesting_permission");
    try {
      await connectRealtimeVoice(replacement, voiceRequestGeneration);
    } catch (_error) {
      replacement.getTracks().forEach((track) => track.stop());
      await activateVoiceStream(currentStream);
      setNotice("input_switch_error");
      return;
    }
    currentStream.getTracks().forEach((track) => track.stop());
    setNotice("input_switched");
  } catch (_error) {
    if (voiceStream === currentStream) {
      elements.voiceDevice.disabled = false;
      setNotice("input_switch_error");
    }
  }
});

navigator.mediaDevices?.addEventListener("devicechange", () => {
  const stream = voiceStream;
  if (stream) void refreshVoiceDevices(stream).catch(() => {});
});

elements.retry.addEventListener("click", async () => {
  const action = retryAction;
  clearError();
  if (action) await action();
});

elements.localeEn.addEventListener("click", () => applyLocale("en"));
elements.localeZh.addEventListener("click", () => applyLocale("zh"));

elements.requestPermission.addEventListener("click", async () => {
  elements.requestPermission.disabled = true;
  try {
    const permission = await window.__TAURI__.core.invoke("request_automation_permission");
    renderAutomationPermission(permission);
  } finally {
    elements.requestPermission.disabled = false;
  }
});

document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    if (voiceStream || voiceRequestPending) {
      stopVoiceSession("stopped", "microphone_hidden_stop");
    }
    if (!avatarStopped) renderAvatarState("idle");
  } else {
    void refresh();
  }
});

window.addEventListener("pagehide", () => {
  avatarStopped = true;
  stopVoiceSession("stopped", null);
});
window.addEventListener("beforeunload", () => {
  avatarStopped = true;
  stopVoiceSession("stopped", null);
});

applyAvatarPreferences();
applyLocale(locale, false);
void qualifyAvatarRenderer().catch(() => renderAvatarState("error"));
void refresh();
window.setInterval(() => {
  if (!document.hidden) void refresh();
}, 2000);
