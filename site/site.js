const repository = document.body.dataset.repository;
const packageArea = document.querySelector("[data-package-area]");
const releaseStatus = document.querySelector("[data-release-status]");
const platformButtons = [...document.querySelectorAll("[data-platform]")];
const instructions = document.querySelector("[data-instructions]");

const releasesUrl = `https://github.com/${repository}/releases`;

const platforms = {
  mac: {
    packages: [
      {
        asset: "Mist-macos-aarch64.dmg",
        name: "Apple silicon Mac",
        detail: "M-series chip",
        action: "Download DMG",
        primary: true,
      },
      {
        asset: "Mist-macos-x86_64.dmg",
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
    packages: [
      {
        asset: "Mist-windows-x86_64-setup.exe",
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
    packages: [
      {
        asset: "Mist-linux-x86_64.deb",
        name: "Ubuntu, Debian, or Mint",
        detail: "Debian package (.deb)",
        action: "Download package",
        primary: true,
      },
      {
        asset: "Mist-linux-x86_64.AppImage",
        name: "Other Linux desktops",
        detail: "Portable AppImage for x86_64",
        action: "Download AppImage",
      },
    ],
    instructions: [
      "Open the DEB with your software installer, or make the AppImage executable and open it.",
      "Open Mist, approve the shortcut prompt if shown, and download the voices.",
      "Select text, then press Ctrl + Space to listen.",
    ],
  },
};

let selectedPlatform = detectPlatform();
let releaseState = { kind: "checking", assets: new Map(), release: null };

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

function unavailableNotice(title, copy, linkLabel = "Check GitHub Releases") {
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
  if (available) {
    option.href = url;
    option.addEventListener("click", () => {
      releaseStatus.textContent = "After the download begins, follow Step 2 below.";
    });
  }

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

  const list = document.createElement("div");
  list.className = "package-list";
  for (const packageInfo of platforms[selectedPlatform].packages) {
    const asset = releaseState.assets.get(packageInfo.asset);
    list.append(packageOption(packageInfo, asset?.browser_download_url, Boolean(asset)));
  }
  packageArea.append(list);
}

for (const button of platformButtons) {
  button.addEventListener("click", () => {
    selectedPlatform = button.dataset.platform;
    renderPackages();
  });
}

async function loadLatestRelease() {
  renderPackages();
  try {
    const response = await fetch(`https://api.github.com/repos/${repository}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    });

    if (response.status === 404) {
      releaseState = { kind: "none", assets: new Map(), release: null };
      releaseStatus.textContent = "First public release in preparation.";
      renderPackages();
      return;
    }
    if (!response.ok) throw new Error(`GitHub returned ${response.status}`);

    const release = await response.json();
    releaseState = {
      kind: "ready",
      assets: new Map(release.assets.map((asset) => [asset.name, asset])),
      release,
    };
    releaseStatus.textContent = `${release.name || release.tag_name} is available.`;
    releaseStatus.classList.add("is-ready");
  } catch {
    releaseState = { kind: "error", assets: new Map(), release: null };
    releaseStatus.textContent = "Automatic download check unavailable.";
  }
  renderPackages();
}

loadLatestRelease();
