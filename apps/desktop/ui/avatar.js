const preferenceKey = "trigix.desktop.avatar.preferences.v1";
const presentationKey = "trigix.desktop.avatar.presentation.v1";
const allowedStates = new Set(["idle", "listening", "thinking", "speaking", "interrupted", "error", "stopped"]);
const allowedMotion = new Set(["full", "reduced", "none"]);

const copy = {
  en: {
    label: "Trigix desktop avatar",
    controls: "Avatar window controls",
    top: "Keep on top",
    hide: "Hide avatar",
    drag: "Drag to move",
    idle: "Ready",
    listening: "Listening",
    thinking: "Thinking",
    speaking: "Speaking",
    interrupted: "Interrupted",
    error: "Avatar unavailable",
    stopped: "Stopped",
  },
  zh: {
    label: "Trigix 桌面虚拟人物",
    controls: "虚拟人物窗口控制",
    top: "保持最上层",
    hide: "隐藏虚拟人物",
    drag: "拖动可移动",
    idle: "随时待命",
    listening: "正在聆听",
    thinking: "正在思考",
    speaking: "正在说话",
    interrupted: "已被打断",
    error: "虚拟人物暂不可用",
    stopped: "已停止",
  },
};

const elements = {
  companion: document.querySelector("#companion"),
  controls: document.querySelector(".companion-controls"),
  dragSurface: document.querySelector("#drag-surface"),
  top: document.querySelector("#toggle-top"),
  hide: document.querySelector("#hide-avatar"),
  caption: document.querySelector("#avatar-caption"),
  dragHint: document.querySelector(".drag-hint"),
};

function readJson(key) {
  try {
    const value = JSON.parse(window.localStorage.getItem(key) ?? "null");
    return value && typeof value === "object" ? value : {};
  } catch (_error) {
    return {};
  }
}

function readPreferences() {
  const stored = readJson(preferenceKey);
  return {
    enabled: typeof stored.enabled === "boolean" ? stored.enabled : true,
    alwaysOnTop: typeof stored.alwaysOnTop === "boolean" ? stored.alwaysOnTop : true,
    highContrast: stored.highContrast === true,
    motion: allowedMotion.has(stored.motion) ? stored.motion : "full",
    captions: stored.captions !== false,
    x: Number.isInteger(stored.x) ? stored.x : null,
    y: Number.isInteger(stored.y) ? stored.y : null,
  };
}

function persistPreferences(preferences) {
  const existing = readJson(preferenceKey);
  window.localStorage.setItem(preferenceKey, JSON.stringify({ ...existing, ...preferences }));
}

function locale() {
  const stored = window.localStorage.getItem("trigix.desktop.locale");
  return stored === "zh" || (stored !== "en" && navigator.language.toLowerCase().startsWith("zh")) ? "zh" : "en";
}

function applyLanguage() {
  const language = locale();
  const messages = copy[language];
  document.documentElement.lang = language;
  elements.companion.setAttribute("aria-label", messages.label);
  elements.controls.setAttribute("aria-label", messages.controls);
  elements.top.title = messages.top;
  elements.top.querySelector(".visually-hidden").textContent = messages.top;
  elements.hide.title = messages.hide;
  elements.hide.querySelector(".visually-hidden").textContent = messages.hide;
  elements.dragHint.textContent = messages.drag;
}

async function configureWindow(preferences = readPreferences()) {
  elements.top.setAttribute("aria-pressed", String(preferences.alwaysOnTop));
  try {
    await window.__TAURI__.core.invoke("configure_avatar_window", {
      input: {
        visible: preferences.enabled,
        always_on_top: preferences.alwaysOnTop,
        x: preferences.x,
        y: preferences.y,
      },
    });
  } catch (_error) {
    // The standalone HTML preview intentionally remains usable outside Tauri.
  }
}

function renderPresentation() {
  const preferences = readPreferences();
  const presentation = readJson(presentationKey);
  const state = allowedStates.has(presentation.state) ? presentation.state : "idle";
  const mouthLevel = Math.max(0, Math.min(100, Number(presentation.mouthLevel) || 0));
  const language = locale();
  document.body.dataset.state = state;
  document.body.dataset.motion = preferences.motion;
  document.body.dataset.highContrast = String(preferences.highContrast);
  document.body.style.setProperty("--mouth-level", String(1 + mouthLevel / 25));
  elements.caption.hidden = !preferences.captions;
  elements.caption.textContent = copy[language][state];
  elements.top.setAttribute("aria-pressed", String(preferences.alwaysOnTop));
}

let gestureTimer = null;
function scheduleGesture() {
  if (gestureTimer !== null) window.clearTimeout(gestureTimer);
  const preferences = readPreferences();
  if (preferences.motion !== "full") {
    document.body.dataset.gesture = "";
    return;
  }
  gestureTimer = window.setTimeout(() => {
    if (document.body.dataset.state === "idle") {
      const gestures = ["wave", "look", "stretch"];
      document.body.dataset.gesture = gestures[Math.floor(Math.random() * gestures.length)];
      window.setTimeout(() => { document.body.dataset.gesture = ""; }, 2_100);
    }
    scheduleGesture();
  }, 6_000 + Math.floor(Math.random() * 6_000));
}

elements.dragSurface.addEventListener("pointerdown", async (event) => {
  if (event.button !== 0) return;
  event.preventDefault();
  try {
    const position = await window.__TAURI__.core.invoke("start_avatar_window_drag");
    if (Number.isInteger(position?.x) && Number.isInteger(position?.y)) {
      persistPreferences({ x: position.x, y: position.y });
    }
  } catch (_error) {
    // Native dragging is unavailable in a regular browser preview.
  }
});

elements.top.addEventListener("click", async () => {
  const preferences = readPreferences();
  preferences.alwaysOnTop = !preferences.alwaysOnTop;
  persistPreferences({ alwaysOnTop: preferences.alwaysOnTop });
  await configureWindow(preferences);
});

elements.hide.addEventListener("click", async () => {
  const preferences = readPreferences();
  preferences.enabled = false;
  persistPreferences({ enabled: false });
  await configureWindow(preferences);
});

window.addEventListener("storage", (event) => {
  if (event.key === preferenceKey) void configureWindow();
  if (event.key === preferenceKey || event.key === presentationKey || event.key === "trigix.desktop.locale") {
    applyLanguage();
    renderPresentation();
  }
});

window.addEventListener("contextmenu", (event) => event.preventDefault());
window.addEventListener("pagehide", () => {
  if (gestureTimer !== null) window.clearTimeout(gestureTimer);
});

applyLanguage();
renderPresentation();
void configureWindow();
scheduleGesture();
window.setInterval(renderPresentation, 100);
