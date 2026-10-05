const repository = document.body.dataset.repository;
const packageArea = document.querySelector("[data-package-area]");
const platformButtons = [...document.querySelectorAll("[data-platform]")];
const instructions = document.querySelector("[data-instructions]");
const heroDownload = document.querySelector("[data-hero-download]");
const platformPicker = document.querySelector("[data-platform-picker]");
const nextSteps = document.querySelector("[data-next-steps]");
const setupExpectation = document.querySelector("[data-setup-expectation]");
const previewNotice = document.querySelector("[data-preview-notice]");
const downloadHeading = document.querySelector("[data-download-heading]");
const downloadCopy = document.querySelector("[data-download-copy]");
const downloadStep = document.querySelector("[data-download-step]");
const panelHeading = document.querySelector("[data-panel-heading]");

const releasesUrl = `https://github.com/${repository}/releases`;

const platforms = {
  mac: {
    label: "Mac",
    packages: [
      {
        asset: "Mist-macos-aarch64.dmg",
        previewChannel: "macos-aarch64",
        name: "Apple silicon Mac",
        detail: "M-series chip",
        action: "Download DMG",
        primary: true,
      },
      {
        asset: "Mist-macos-x86_64.dmg",
        previewChannel: "macos-x86_64",
        name: "Intel Mac",
        detail: "Macs with an Intel processor",
        action: "Download DMG",
      },
    ],
    instructions: [
      "Open the DMG and drag Mist into Applications.",
      "Open Mist and allow it in System Settings → Privacy & Security → Accessibility.",
      "Download the voices, select text, then press Ctrl + Space.",
    ],
  },
  windows: {
    label: "Windows",
    packages: [
      {
        asset: "Mist-windows-x86_64-setup.exe",
        previewChannel: "windows-x86_64",
        name: "Windows 64-bit installer",
        detail: "For most Windows 10 and Windows 11 PCs",
        action: "Download installer",
        primary: true,
      },
    ],
    instructions: [
      "Open the downloaded setup file and finish installation.",
      "Open Mist and download the voices.",
      "Select text, then press Ctrl + Space to listen.",
    ],
  },
  linux: {
    label: "Linux",
    packages: [
      {
        asset: "Mist-linux-x86_64.deb",
        previewChannel: "linux",
        name: "Ubuntu 24.04+ / Mint 22+ (x86_64)",
        detail: "Debian package (.deb) · AVX2 processor required",
        action: "Download package",
        primary: true,
      },
      {
        asset: "Mist-linux-x86_64.AppImage",
        previewChannel: "linux",
        name: "Compatible Linux desktops (x86_64)",
        detail: "AppImage · glibc 2.39+, GLIBCXX 3.4.32, AVX2",
        action: "Download AppImage",
      },
    ],
    instructions: [
      "Open the DEB with your software installer. For AppImage, make it executable and open it; if FUSE 2 is unavailable, launch it with APPIMAGE_EXTRACT_AND_RUN=1.",
      "Open Mist, approve the shortcut prompt if shown, and download the voices.",
      "Select text, then press Ctrl + Space to listen.",
    ],
  },
};

let selectedPlatform = detectPlatform();
let releaseState = { kind: "checking", assets: new Map() };

function detectPlatform() {
  const userAgent = navigator.userAgent.toLowerCase();
  const value = (
    navigator.userAgentData?.platform ||
    navigator.platform ||
    navigator.userAgent
  ).toLowerCase();

  if (
    /android|iphone|ipad|ipod|mobile/.test(userAgent) ||
    value.includes("android") ||
    value.includes("cros") ||
    (value.includes("mac") && navigator.maxTouchPoints > 1)
  ) return null;

  if (value.includes("mac")) return "mac";
  if (value.includes("win")) return "windows";
  if (value.includes("linux")) return "linux";
  return null;
}

function renderInstructions() {
  instructions.replaceChildren();
  const platformInstructions = selectedPlatform
    ? platforms[selectedPlatform].instructions
    : [
        "Choose Mac, Windows, or Linux above.",
        "Open the installer and download the speech model and voices.",
        "Select text, then press Ctrl + Space to listen.",
      ];
  for (const [index, instruction] of platformInstructions.entries()) {
    const item = document.createElement("li");
    const number = document.createElement("span");
    const copy = document.createElement("p");
    number.textContent = String(index + 1);
    number.setAttribute("aria-hidden", "true");
    copy.textContent = instruction;
    item.append(number, copy);
    instructions.append(item);
  }
}

function unavailableNotice(title, copy, linkLabel) {
  const notice = document.createElement("div");
  notice.className = "release-notice";
  const heading = document.createElement("strong");
  const detail = document.createElement("span");
  const link = document.createElement("a");
  heading.textContent = title;
  detail.textContent = copy;
  link.href = releasesUrl;
  link.textContent = linkLabel;
  notice.append(heading, detail, link);
  return notice;
}

function packageOption(packageInfo, url, available) {
  const option = document.createElement(available ? "a" : "div");
  option.className = "package-option";
  if (packageInfo.primary && available) option.classList.add("package-option-primary");
  if (!available) option.classList.add("is-disabled");
  if (available) option.href = url;

  const label = document.createElement("span");
  const name = document.createElement("span");
  const detail = document.createElement("span");
  const action = document.createElement("span");
  name.className = "package-name";
  detail.className = "package-detail";
  action.className = "package-action";
  name.textContent = packageInfo.name;
  detail.textContent = packageInfo.detail;
  action.textContent = available ? packageInfo.action : "Not available yet";
  label.append(name, detail);
  option.append(label, action);
  return option;
}

function renderPackages() {
  for (const button of platformButtons) {
    button.setAttribute("aria-pressed", String(button.dataset.platform === selectedPlatform));
  }
  renderInstructions();
  updateHeroDownload();
  const ready = releaseState.kind === "ready";
  const selectedAvailable = ready && selectedPlatform && platforms[selectedPlatform].packages.some((packageInfo) =>
    releaseState.assets.has(packageInfo.asset));
  platformPicker.hidden = !ready;
  nextSteps.hidden = !selectedAvailable;
  setupExpectation.hidden = !selectedAvailable;
  downloadStep.hidden = !selectedAvailable;
  previewNotice.hidden = !ready || !releaseState.prerelease;
  panelHeading.textContent = ready ? "Choose your platform" : "Release status";
  downloadHeading.textContent = ready
    ? (releaseState.prerelease ? "Try the Mist preview" : "Download Mist")
    : (releaseState.kind === "none" ? "Downloads coming soon" : "Download availability");
  downloadCopy.textContent = ready
    ? (releaseState.prerelease
      ? "Platform previews arrive independently. Choose an available installer for your computer."
      : "Choose your computer, then pick the installer that matches it.")
    : "Only published installers appear here. You can follow progress on GitHub.";
  packageArea.replaceChildren();

  if (releaseState.kind === "checking") {
    const checking = document.createElement("div");
    const spinner = document.createElement("span");
    checking.className = "checking-state";
    spinner.className = "spinner";
    spinner.setAttribute("aria-hidden", "true");
    checking.append(spinner, "Checking available downloads…");
    packageArea.append(checking);
    packageArea.setAttribute("aria-busy", "true");
    return;
  }

  packageArea.setAttribute("aria-busy", "false");
  if (releaseState.kind === "none") {
    packageArea.append(unavailableNotice(
      "Mist is not available to download yet.",
      "The first public release is still being tested. No installer has been published.",
      "Follow release progress",
    ));
    return;
  }

  if (releaseState.kind === "error") {
    packageArea.append(unavailableNotice(
      "We could not check downloads right now.",
      "Open GitHub Releases to see the available installers.",
      "View downloads on GitHub",
    ));
    return;
  }

  if (!selectedPlatform) {
    packageArea.append(unavailableNotice(
      "Mist runs on desktop computers.",
      "Open this page on macOS, Windows, or x64 Linux, or choose a platform above.",
      "View all releases",
    ));
    return;
  }

  if (!platforms[selectedPlatform].packages.some((packageInfo) => releaseState.assets.has(packageInfo.asset))) {
    packageArea.append(unavailableNotice(
      `${platforms[selectedPlatform].label} download coming soon.`,
      releaseState.prerelease
        ? "Choose an available platform above to try a preview. Other installers are still being prepared."
        : "This release has no installer for this platform. Choose another platform above or check back later.",
      "View available releases",
    ));
    return;
  }

  const list = document.createElement("div");
  list.className = "package-list";
  for (const packageInfo of platforms[selectedPlatform].packages) {
    const asset = releaseState.assets.get(packageInfo.asset);
    list.append(packageOption(packageInfo, asset?.browser_download_url, Boolean(asset)));
  }
  packageArea.append(list);
}

function updateHeroDownload() {
  heroDownload.href = "#download";
  heroDownload.removeAttribute("download");
  if (releaseState.kind === "none") {
    heroDownload.textContent = "Downloads coming soon";
    return;
  }
  if (releaseState.kind === "checking") {
    heroDownload.textContent = "Checking downloads…";
    return;
  }
  if (releaseState.kind === "error") {
    heroDownload.textContent = "View download status";
    return;
  }
  heroDownload.textContent = selectedPlatform
    ? `Download for ${platforms[selectedPlatform].label}`
    : "Choose your download";

  if (releaseState.kind !== "ready" || !selectedPlatform) return;
  const availablePackages = platforms[selectedPlatform].packages.filter((packageInfo) =>
    releaseState.assets.has(packageInfo.asset)
  );
  if (availablePackages.length === 0) {
    heroDownload.textContent = `${platforms[selectedPlatform].label} download coming soon`;
    return;
  }
  if (platforms[selectedPlatform].packages.length !== 1 || availablePackages.length !== 1) return;

  const packageInfo = availablePackages[0];
  heroDownload.href = releaseState.assets.get(packageInfo.asset).browser_download_url;
  heroDownload.textContent = `Download for ${platforms[selectedPlatform].label}`;
}

for (const button of platformButtons) {
  button.addEventListener("click", () => {
    selectedPlatform = button.dataset.platform;
    renderPackages();
  });
}

function publishedAssets(release, channel = null) {
  const assets = new Map();
  if (!release || release.draft || typeof release.tag_name !== "string" ||
      !release.published_at || !Number.isFinite(Date.parse(release.published_at)) ||
      !Array.isArray(release.assets)) return assets;
  const supportedNames = new Set(Object.values(platforms).flatMap((platform) =>
    platform.packages.filter((packageInfo) => !channel || packageInfo.previewChannel === channel)
      .map((packageInfo) => packageInfo.asset)));
  for (const asset of release.assets) {
    if (!asset || !supportedNames.has(asset.name) || asset.state !== "uploaded" ||
        !Number.isSafeInteger(asset.size) || asset.size <= 0) continue;
    const expectedUrl = `https://github.com/${repository}/releases/download/${encodeURIComponent(release.tag_name)}/${encodeURIComponent(asset.name)}`;
    if (asset.browser_download_url === expectedUrl) assets.set(asset.name, asset);
  }
  return assets;
}

async function loadLatestRelease() {
  renderPackages();
  try {
    const response = await fetch(`https://api.github.com/repos/${repository}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
      signal: AbortSignal.timeout(15_000),
    });

    if (response.status === 404) {
      // GitHub's "latest" endpoint excludes previews. Only use this explicitly
      // named platform preview channels when there is no stable desktop release.
      const releases = [];
      for (let page = 1; ; page += 1) {
        const previewsResponse = await fetch(`https://api.github.com/repos/${repository}/releases?per_page=100&page=${page}`, {
          headers: { Accept: "application/vnd.github+json" },
          signal: AbortSignal.timeout(15_000),
        });
        if (!previewsResponse.ok) throw new Error(`GitHub returned ${previewsResponse.status}`);
        const batch = await previewsResponse.json();
        if (!Array.isArray(batch) || batch.length > 100) throw new Error("Invalid release listing");
        releases.push(...batch);
        if (batch.length < 100) break;
      }
      const previews = releases.filter((release) => release?.prerelease && !release.draft &&
        Number.isFinite(Date.parse(release.published_at)) &&
        /^v\d+\.\d+\.\d+-(linux|macos-aarch64|macos-x86_64|windows-x86_64)-preview\.[1-9]\d*$/.test(release.tag_name))
        .sort((left, right) => Date.parse(right.published_at) - Date.parse(left.published_at));
      const assets = new Map();
      const selectedChannels = new Set();
      for (const preview of previews) {
        const channel = preview.tag_name.match(/-(linux|macos-aarch64|macos-x86_64|windows-x86_64)-preview\./)[1];
        const available = publishedAssets(preview, channel);
        if (selectedChannels.has(channel) || available.size === 0) continue;
        selectedChannels.add(channel);
        for (const [name, asset] of available) assets.set(name, asset);
      }
      releaseState = assets.size > 0
        ? { kind: "ready", assets, prerelease: true }
        : { kind: "none", assets };
    } else {
      if (!response.ok) throw new Error(`GitHub returned ${response.status}`);
      const release = await response.json();
      const assets = publishedAssets(release);
      releaseState = assets.size > 0
        ? { kind: "ready", assets, prerelease: false }
        : { kind: "none", assets };
    }
  } catch {
    releaseState = { kind: "error", assets: new Map() };
  }
  renderPackages();
}

loadLatestRelease();
