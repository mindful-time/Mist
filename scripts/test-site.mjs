import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';

const html = readFileSync(new URL('../site/index.html', import.meta.url), 'utf8');
const script = readFileSync(new URL('../site/site.js', import.meta.url), 'utf8');

// Supply the browser's DOM and HTTP boundaries, running the actual page script
// against its actual data attributes. No rendering/state functions are replaced.
class Element {
  constructor(tag, attributes = {}) {
    this.tagName = tag;
    this.attributes = attributes;
    this.dataset = Object.fromEntries(Object.entries(attributes).filter(([key]) => key.startsWith('data-'))
      .map(([key, value]) => [key.slice(5).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase()), value]));
    this.children = [];
    this.hidden = 'hidden' in attributes;
    this.listeners = {};
    this.href = attributes.href ?? '';
    this.classList = { add() {} };
    this.text = '';
  }
  set textContent(value) { this.text = value; this.children = []; }
  get textContent() { return this.text + this.children.map(child => typeof child === 'string' ? child : child.textContent).join(''); }
  append(...children) { this.children.push(...children); }
  replaceChildren(...children) { this.text = ''; this.children = children; }
  setAttribute(name, value) { this.attributes[name] = value; }
  removeAttribute(name) { delete this.attributes[name]; }
  addEventListener(name, listener) { this.listeners[name] = listener; }
}

test('without JavaScript, the page offers release status rather than unverified installation steps', () => {
  for (const attribute of [
    'data-platform-picker', 'data-next-steps', 'data-setup-expectation', 'data-download-step',
  ]) {
    const tag = html.match(new RegExp(`<[^>]*\\b${attribute}\\b[^>]*>`))?.[0];
    assert.ok(tag, `${attribute} exists`);
    assert.match(tag, /\shidden(?:\s|>)/, `${attribute} starts hidden`);
  }
  assert.match(html, /<noscript><style>\.checking-state\s*\{\s*display:\s*none\s*!important;/);
  assert.match(html, /<strong>Check releases on GitHub\.<\/strong>/);
});

function browser(responses, platform = 'Linux x86_64') {
  const nodes = [...html.matchAll(/<(\w+)\b([^>]*\bdata-[^>]*)>/g)].map(([, tag, source]) => {
    const attributes = Object.fromEntries([...source.matchAll(/([\w-]+)(?:="([^"]*)")?/g)]
      .map(([, key, value]) => [key, value ?? '']));
    return new Element(tag, attributes);
  });
  const select = selector => nodes.filter(node => selector.slice(1, -1) in node.attributes);
  const requests = [];
  const document = {
    body: nodes.find(node => node.tagName === 'body'),
    querySelector: selector => select(selector)[0] ?? null,
    querySelectorAll: select,
    createElement: tag => new Element(tag),
  };
  const done = runInNewContext(script, {
    document, navigator: { platform, userAgent: platform, maxTouchPoints: 0 }, AbortSignal,
    fetch: async url => {
      requests.push(url);
      const response = responses.shift();
      if (!response) throw new Error('Unexpected HTTP request');
      if (response instanceof Error) throw response;
      return { status: response.status, ok: response.status === 200, json: async () => response.body };
    },
  });
  return {
    done, requests,
    node: attribute => document.querySelector(`[${attribute}]`),
    choose: platform => nodes.find(node => node.dataset.platform === platform).listeners.click(),
  };
}

test('a site without a public installer does not offer a download', async () => {
  const page = browser([{ status: 404 }, { status: 200, body: [] }]);
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'Downloads coming soon');
  assert.equal(page.node('data-platform-picker').hidden, true);
  assert.equal(page.node('data-next-steps').hidden, true);
});

const preview = {
  tag_name: 'v0.1.0-linux-preview.1', draft: false, prerelease: true,
  published_at: '2026-10-05T00:00:00Z',
  assets: ['Mist-linux-x86_64.deb', 'Mist-linux-x86_64.AppImage'].map(name => ({
    name, state: 'uploaded', size: 1024,
    browser_download_url: `https://github.com/mindful-time/Mist/releases/download/v0.1.0-linux-preview.1/${name}`,
  })),
};

test('Linux users can download a published preview before the full release', async () => {
  const page = browser([{ status: 404 }, { status: 200, body: [preview] }]);
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'Download for Linux');
  assert.match(page.node('data-package-area').textContent, /Download package/);
  assert.match(page.node('data-package-area').textContent, /Download AppImage/);
  assert.equal(page.node('data-preview-notice').hidden, false);
  assert.equal(page.node('data-next-steps').hidden, false);
});

test('the Linux preview does not pretend a Mac download exists', async () => {
  const page = browser([{ status: 404 }, { status: 200, body: [preview] }], 'MacIntel');
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'Mac download coming soon');
  assert.match(page.node('data-package-area').textContent, /Choose an available platform above/);
  page.choose('linux');
  assert.equal(page.node('data-hero-download').textContent, 'Download for Linux');
});

test('an uploading or external-host asset is not offered as an installer', async () => {
  for (const asset of [
    { ...preview.assets[0], state: 'new' },
    { ...preview.assets[0], size: 0 },
    { ...preview.assets[0], browser_download_url: 'https://example.invalid/Mist-linux-x86_64.deb' },
  ]) {
    const page = browser([{ status: 404 }, { status: 200, body: [{ ...preview, assets: [asset] }] }]);
    await page.done;
    assert.equal(page.node('data-hero-download').textContent, 'Downloads coming soon');
  }
});

test('loading and API failures do not promise a download', async () => {
  const page = browser([new Error('Network unavailable')]);
  assert.equal(page.node('data-hero-download').textContent, 'Checking downloads…');
  assert.equal(page.node('data-package-area').attributes['aria-busy'], 'true');
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'View download status');
  assert.equal(page.node('data-package-area').attributes['aria-busy'], 'false');
  assert.match(page.node('data-package-area').textContent, /could not check downloads/);
});

test('Windows users see coming soon and can switch to the available Linux preview', async () => {
  const page = browser([{ status: 404 }, { status: 200, body: [preview] }], 'Win32');
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'Windows download coming soon');
  assert.equal(page.node('data-next-steps').hidden, true);
  page.choose('linux');
  assert.equal(page.node('data-next-steps').hidden, false);
});

test('drafts, unpublished builds, and unrelated prereleases are never offered', async () => {
  for (const release of [
    { ...preview, draft: true },
    { ...preview, published_at: null },
    { ...preview, tag_name: 'v0.1.0-alpha.1' },
  ]) {
    const page = browser([{ status: 404 }, { status: 200, body: [release] }]);
    await page.done;
    assert.equal(page.node('data-hero-download').textContent, 'Downloads coming soon');
  }
});

test('a stable installer takes precedence over previews and downloads directly on Windows', async () => {
  const url = 'https://github.com/mindful-time/Mist/releases/download/v0.1.0/Mist-windows-x86_64-setup.exe';
  const page = browser([{ status: 200, body: {
    tag_name: 'v0.1.0', draft: false, prerelease: false, published_at: '2026-10-06T00:00:00Z',
    assets: [{ name: 'Mist-windows-x86_64-setup.exe', browser_download_url: url, state: 'uploaded', size: 1024 }],
  } }], 'Win32');
  await page.done;
  assert.equal(page.node('data-hero-download').href, url);
  assert.equal(page.node('data-preview-notice').hidden, true);
  assert.equal(page.requests.length, 1);
});

test('an incomplete release listing fails closed', async () => {
  const page = browser([{ status: 404 }, { status: 200, body: { message: 'Invalid listing' } }]);
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'View download status');
});

function platformPreview(platform, names, date = '2026-10-05T01:00:00Z', number = 1) {
  const tag = `v0.1.0-${platform}-preview.${number}`;
  return {
    ...preview, tag_name: tag, published_at: date,
    assets: names.map(name => ({ name, state: 'uploaded', size: 1024,
      browser_download_url: `https://github.com/mindful-time/Mist/releases/download/${tag}/${name}` })),
  };
}

test('independently published Mac and Linux previews appear without waiting for Intel or Windows', async () => {
  const apple = platformPreview('macos-aarch64', ['Mist-macos-aarch64.dmg']);
  const page = browser([{ status: 404 }, { status: 200, body: [apple, preview] }], 'MacIntel');
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'Download for Mac');
  assert.match(page.node('data-package-area').textContent, /Download DMG/);
  assert.match(page.node('data-package-area').textContent, /Intel Mac.*Not available yet/);
  assert.equal(page.node('data-download-heading').textContent, 'Try the Mist preview');
  assert.ok(!page.node('data-download-copy').textContent.includes('Linux is available first'));
  page.choose('linux');
  assert.match(page.node('data-package-area').textContent, /Download AppImage/);
  page.choose('windows');
  assert.equal(page.node('data-hero-download').textContent, 'Windows download coming soon');
});

test('Windows can publish first and its preview downloads directly', async () => {
  const windows = platformPreview('windows-x86_64', ['Mist-windows-x86_64-setup.exe']);
  const page = browser([{ status: 404 }, { status: 200, body: [windows] }], 'Win32');
  await page.done;
  assert.equal(page.node('data-hero-download').href, windows.assets[0].browser_download_url);
  assert.equal(page.node('data-preview-notice').hidden, false);
});

test('the newest published preview for each platform wins even when releases arrive out of order', async () => {
  const intel = platformPreview('macos-x86_64', ['Mist-macos-x86_64.dmg']);
  const old = platformPreview('windows-x86_64', ['Mist-windows-x86_64-setup.exe']);
  const newer = platformPreview('windows-x86_64', ['Mist-windows-x86_64-setup.exe'], '2026-10-05T02:00:00Z', 2);
  const page = browser([{ status: 404 }, { status: 200, body: [old, intel, newer, preview] }], 'Win32');
  await page.done;
  assert.equal(page.node('data-hero-download').href, newer.assets[0].browser_download_url);
  page.choose('mac');
  assert.match(page.node('data-package-area').textContent, /Intel Mac.*Download DMG/);
});

test('a platform preview cannot offer another platform installer or use an invalid publication date', async () => {
  const misplaced = platformPreview('linux', ['Mist-windows-x86_64-setup.exe']);
  const invalidDate = platformPreview('windows-x86_64', ['Mist-windows-x86_64-setup.exe'], 'invalid');
  const page = browser([{ status: 404 }, { status: 200, body: [misplaced, invalidDate] }], 'Win32');
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'Downloads coming soon');
});

test('preview discovery follows pagination to keep a less frequently released platform available', async () => {
  const windows = platformPreview('windows-x86_64', ['Mist-windows-x86_64-setup.exe']);
  const page = browser([{ status: 404 }, { status: 200, body: Array(100).fill(preview) },
    { status: 200, body: [windows] }], 'Win32');
  await page.done;
  assert.equal(page.node('data-hero-download').href, windows.assets[0].browser_download_url);
  assert.equal(page.requests.length, 3);
  assert.match(page.requests[2], /[?&]page=2(?:&|$)/);
});

test('failed pagination does not present an incomplete release listing as authoritative', async () => {
  const page = browser([{ status: 404 }, { status: 200, body: Array(100).fill(preview) }, { status: 403 }]);
  await page.done;
  assert.equal(page.node('data-hero-download').textContent, 'View download status');
});
