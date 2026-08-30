const connection = document.querySelector("#connection");
const automation = document.querySelector("#automation");
const automationHost = document.querySelector("#automation-host");
const revision = document.querySelector("#revision");
const stopButton = document.querySelector("#stop");
const notice = document.querySelector("#notice");
const statusPanel = document.querySelector(".status-panel");
const pairingForm = document.querySelector("#pairing-form");
const pairingTitle = document.querySelector("#pairing-title");
const platformUrl = document.querySelector("#platform-url");
const deviceName = document.querySelector("#device-name");
const pairingPhase = document.querySelector("#pairing-phase");
const pairingWaiting = document.querySelector("#pairing-waiting");
const pairingComplete = document.querySelector("#pairing-complete");
const pairingCode = document.querySelector("#pairing-code");
const pairingExpiry = document.querySelector("#pairing-expiry");
const pairedDevice = document.querySelector("#paired-device");
const claimPairing = document.querySelector("#claim-pairing");
const forgetPairing = document.querySelector("#forget-pairing");

let currentRevision = 0;

function readable(value) {
  return value.replaceAll("_", " ").replace(/^./, (letter) => letter.toUpperCase());
}

function render(snapshot) {
  currentRevision = snapshot.revision;
  connection.textContent = readable(snapshot.connection);
  automation.textContent = readable(snapshot.automation);
  automationHost.textContent = readable(snapshot.automation_host);
  revision.textContent = String(snapshot.revision);
  stopButton.disabled = !snapshot.can_request_stop;
  statusPanel.setAttribute("aria-busy", "false");
  notice.textContent = snapshot.can_request_stop
    ? "An automation action is active."
    : "No interruptible automation action is active.";
}

function renderPairing(snapshot) {
  pairingPhase.textContent = readable(snapshot.phase);
  pairingTitle.textContent = {
    unpaired: "Pair this computer",
    waiting_for_approval: "Approve this computer",
    paired: "This computer is paired",
    unavailable: "Pairing unavailable",
  }[snapshot.phase] ?? "Device pairing";
  pairingForm.hidden = snapshot.phase !== "unpaired";
  pairingWaiting.hidden = snapshot.phase !== "waiting_for_approval";
  pairingComplete.hidden = snapshot.phase !== "paired";

  if (snapshot.phase === "waiting_for_approval") {
    pairingCode.textContent = snapshot.pairing_code;
    const expiry = new Date(snapshot.expires_at_unix_seconds * 1000);
    pairingExpiry.textContent = `Approve this code before ${expiry.toLocaleTimeString()}.`;
  }
  if (snapshot.phase === "paired") {
    pairedDevice.textContent = snapshot.device_id;
  }
  if (snapshot.phase === "unavailable") {
    pairingForm.hidden = true;
    pairingWaiting.hidden = true;
    pairingComplete.hidden = true;
    notice.textContent = "Windows secure storage is unavailable; pairing is disabled.";
  }
}

async function refresh() {
  try {
    const [shell, pairing] = await Promise.all([
      window.__TAURI__.core.invoke("shell_status"),
      window.__TAURI__.core.invoke("pairing_status"),
    ]);
    render(shell);
    renderPairing(pairing);
  } catch (_error) {
    stopButton.disabled = true;
    statusPanel.setAttribute("aria-busy", "false");
    notice.textContent = "Local runtime state is unavailable.";
  }
}

pairingForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const submit = document.querySelector("#start-pairing");
  submit.disabled = true;
  notice.textContent = "Creating a short-lived pairing code…";
  try {
    const snapshot = await window.__TAURI__.core.invoke("start_device_pairing", {
      input: {
        platform_url: platformUrl.value,
        display_name: deviceName.value,
      },
    });
    renderPairing(snapshot);
    notice.textContent = "Pairing code created. Approval is required in the Tenant administration page.";
  } catch (_error) {
    notice.textContent = "Pairing could not start. Verify the HTTPS origin and Platform availability.";
  } finally {
    submit.disabled = false;
  }
});

claimPairing.addEventListener("click", async () => {
  claimPairing.disabled = true;
  notice.textContent = "Claiming the approved Device credential…";
  try {
    renderPairing(await window.__TAURI__.core.invoke("complete_device_pairing"));
    notice.textContent = "Pairing completed. Establishing the authenticated Device connection.";
  } catch (_error) {
    notice.textContent = "Approval is not available yet, or the pairing code expired.";
  } finally {
    claimPairing.disabled = false;
  }
});

forgetPairing.addEventListener("click", async () => {
  forgetPairing.disabled = true;
  notice.textContent = "Removing the local Device credential…";
  try {
    renderPairing(await window.__TAURI__.core.invoke("forget_device_pairing"));
    notice.textContent = "Local pairing removed. An administrator must revoke stale server records separately.";
  } catch (_error) {
    notice.textContent = "The local Device credential could not be removed securely.";
  } finally {
    forgetPairing.disabled = false;
  }
});

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
