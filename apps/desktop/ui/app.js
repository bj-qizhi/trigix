const connection = document.querySelector("#connection");
const automation = document.querySelector("#automation");
const revision = document.querySelector("#revision");
const stopButton = document.querySelector("#stop");
const notice = document.querySelector("#notice");
const statusPanel = document.querySelector(".status-panel");

let currentRevision = 0;

function readable(value) {
  return value.replaceAll("_", " ").replace(/^./, (letter) => letter.toUpperCase());
}

function render(snapshot) {
  currentRevision = snapshot.revision;
  connection.textContent = readable(snapshot.connection);
  automation.textContent = readable(snapshot.automation);
  revision.textContent = String(snapshot.revision);
  stopButton.disabled = !snapshot.can_request_stop;
  statusPanel.setAttribute("aria-busy", "false");
  notice.textContent = snapshot.can_request_stop
    ? "An automation action is active."
    : "No interruptible automation action is active.";
}

async function refresh() {
  try {
    render(await window.__TAURI__.core.invoke("shell_status"));
  } catch (_error) {
    stopButton.disabled = true;
    statusPanel.setAttribute("aria-busy", "false");
    notice.textContent = "Local runtime state is unavailable.";
  }
}

stopButton.addEventListener("click", async () => {
  stopButton.disabled = true;
  notice.textContent = "Requesting immediate stop…";
  const requestId = `stop-${crypto.randomUUID()}`;
  try {
    await window.__TAURI__.core.invoke("request_automation_stop", {
      request: { request_id: requestId, observed_revision: currentRevision },
    });
    await refresh();
  } catch (_error) {
    notice.textContent = "Stop request was rejected. Refreshing runtime state.";
    await refresh();
  }
});

refresh();
window.setInterval(refresh, 2000);
