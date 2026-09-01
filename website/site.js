const root = document.documentElement;
const themeButton = document.querySelector("#theme-toggle");
const storedTheme = window.localStorage.getItem("trigix.website.theme");
const preferredTheme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";

function applyTheme(theme) {
  root.dataset.theme = theme;
  themeButton.textContent = theme === "dark" ? "Light" : "Dark";
  themeButton.setAttribute("aria-label", `Switch to ${theme === "dark" ? "light" : "dark"} theme`);
}

applyTheme(storedTheme === "dark" || storedTheme === "light" ? storedTheme : preferredTheme);
themeButton.addEventListener("click", () => {
  const theme = root.dataset.theme === "dark" ? "light" : "dark";
  window.localStorage.setItem("trigix.website.theme", theme);
  applyTheme(theme);
});

const releasePanel = document.querySelector("#release-panel");
const releaseTitle = document.querySelector("#release-title");
const releaseDetail = document.querySelector("#release-detail");
const releaseActions = document.querySelector("#release-actions");
const releasesUrl = "https://github.com/bj-qizhi/trigix/releases";

function assetLink(asset, label, primary = false) {
  const link = document.createElement("a");
  link.className = `button ${primary ? "primary" : "quiet"}`;
  link.href = asset.browser_download_url;
  link.textContent = label;
  return link;
}

async function loadDesktopRelease() {
  try {
    const response = await fetch("https://api.github.com/repos/bj-qizhi/trigix/releases?per_page=20", {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!response.ok) throw new Error("release feed unavailable");
    const releases = await response.json();
    const release = releases.find((item) => !item.draft && !item.prerelease && /^desktop-v\d+\.\d+\.\d+$/.test(item.tag_name));
    if (!release) {
      releaseTitle.textContent = "Official Desktop GA has not been published";
      releaseDetail.textContent = "Official signed installers will appear here after production qualification. Independent distributors may publish clearly identified builds under their own responsibility.";
      return;
    }

    const installers = release.assets.filter((asset) => /\.(dmg|exe|msi)$/i.test(asset.name));
    const checksums = release.assets.filter((asset) => /\.sha256$/i.test(asset.name));
    releaseTitle.textContent = `Trigix Desktop ${release.tag_name.replace("desktop-v", "")}`;
    releaseDetail.textContent = `Published ${new Date(release.published_at).toLocaleDateString(undefined, { dateStyle: "long" })}. Verify the checksum and operating-system signature before installation.`;
    releaseActions.replaceChildren();
    installers.forEach((asset) => releaseActions.append(assetLink(asset, asset.name, true)));
    checksums.forEach((asset) => releaseActions.append(assetLink(asset, `${asset.name} checksum`)));
    releaseActions.append(assetLink({ browser_download_url: release.html_url }, "Release notes"));
  } catch (_error) {
    releaseTitle.textContent = "Release status is temporarily unavailable";
    releaseDetail.textContent = "Use the GitHub Releases page to confirm published installers and checksums.";
  } finally {
    releasePanel.setAttribute("aria-busy", "false");
  }
}

void loadDesktopRelease();
