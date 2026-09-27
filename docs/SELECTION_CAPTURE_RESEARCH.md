# System-wide selected-text capture research

Date: 2026-09-27

## Decision

The product cannot truthfully guarantee **“select any text anywhere, press
Ctrl+Space, and it speaks”** while also promising that it never changes the
normal clipboard.

The strongest defensible claim is:

> Select text in a supported app and press your configured shortcut. Mist reads
> selections exposed by the operating system's accessibility or primary-selection
> APIs without changing your clipboard. When an app or desktop does not expose
> the selection, use **Copy & Speak** or paste text into Mist.

The limitation is structural, not just an implementation gap:

- Accessibility clients can only read what the target application's
  accessibility provider exposes. Apple requires `AXSelectedText` for editable
  text elements, not for every selectable visual element; Microsoft likewise
  makes the target control's provider responsible for implementing TextPattern;
  and Linux AT-SPI only works for objects implementing its Text interface
  ([Apple](https://developer.apple.com/documentation/applicationservices/kaxselectedtextattribute),
  [Microsoft](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-providersoverview),
  [AT-SPI](https://docs.gtk.org/atspi2/iface.Text.html)).
- Custom-painted controls, canvas-rendered documents, image-only PDFs, games,
  protected fields, and inaccessible web widgets may have visible/selectable
  pixels but no programmatic text selection. No accessibility API can recover
  data the provider never publishes.
- A normal background Wayland client is intentionally not sent another client's
  selection. The standard primary-selection protocol sends a non-null offer only
  to the keyboard-focused client
  ([official protocol source](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/unstable/primary-selection/primary-selection-unstable-v1.xml)).
- macOS forbids an assistive app that controls other apps from using App Sandbox,
  so this feature is incompatible with a conventional Mac App Store sandboxed
  build
  ([Apple App Sandbox documentation](https://developer.apple.com/documentation/security/protecting-user-data-with-app-sandbox)).
- On Windows, a normal-integrity client cannot inspect an elevated target. A
  signed, securely installed `uiAccess=true` assistive application is required to
  cross that boundary
  ([Microsoft UI Automation security](https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/ui-automation-security-overview),
  [UIAccess secure-location policy](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/security-policy-settings/user-account-control-only-elevate-uiaccess-applications-that-are-installed-in-secure-locations)).

The shortcut and selection capture are independent capabilities. A registered
shortcut proves only that Mist received a key gesture; it says nothing about
whether the focused app exposes selected text. Conversely, a perfect selection
adapter is irrelevant if the desktop consumed the key first.

## Feasibility at a glance

| Platform | Best clipboard-neutral path | What is genuinely achievable | Hard boundary |
|---|---|---|---|
| macOS | Accessibility (`AXSelectedText`), plus an explicit AppKit Service | Good coverage for standard AppKit controls, accessible browsers, Office views, terminals, and Electron content after its AX tree is active | Accessibility permission, target-provider quality, protected/custom content, and no sandboxed assistive client |
| Windows | UI Automation `TextPattern.GetSelection`; optional Chromium/Firefox IA2 fallback | Good coverage for standard Win32/WinUI/WPF controls, Office document providers, WebView2, accessible browsers, and Windows Terminal | Target must implement a text provider; elevated targets require UIAccess; Chromium accessibility may need activation |
| Linux X11 | X11 `PRIMARY`, then AT-SPI | Broad coverage for conventional X11 text selection, including browsers and terminals, without touching `CLIPBOARD` | The source app must own and convert `PRIMARY`; grabs can conflict; sandbox/X server access can be denied |
| Linux Wayland | AT-SPI; compositor-specific privileged data-control only when available | Best effort on accessible apps; reliable background primary-selection reads only on compositors exposing a privileged data-control protocol | Standard primary selection is focus-scoped; no general “read selected text” portal exists; compositor and sandbox policy decide availability |

## Shortcut reality

`Ctrl+Space` must be a configurable preference, never an unconditional promise.

- macOS already uses Control-Space for switching to the previous input source
  when multiple input sources are configured
  ([Apple's shortcut list](https://support.apple.com/en-my/102650)). Mist must
  detect registration failure/conflict, explain it, and offer alternatives such
  as Control-Option-S, Control-Shift-S, or a user-recorded gesture.
- Windows `RegisterHotKey` normally fails if the combination is already
  registered by another hotkey
  ([Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)).
  Registration success must be checked and shown in the UI.
- X11 `XGrabKey` reports `BadAccess` when another client owns the same key
  combination
  ([Xlib specification](https://www.x.org/releases/X11R7.6/doc/libX11/specs/libX11/libX11.html)).
- On Wayland, the GlobalShortcuts portal accepts a *preferred* trigger, may show
  configuration UI, may bind only a subset (including none), and returns a
  user-readable `trigger_description`. The app must display the gesture actually
  granted
  ([XDG GlobalShortcuts portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)).

If the OS opens its input-source/voice UI instead of Mist reacting, diagnose
shortcut registration first. Do not treat that symptom as a text-selection bug.

## macOS

### What the public API supports

After the user grants Accessibility permission, create the system-wide element
with `AXUIElementCreateSystemWide`, which Apple explicitly describes as useful
for finding the focused object regardless of the active application
([Apple](https://developer.apple.com/documentation/applicationservices/1462095-axuielementcreatesystemwide)).
Read `AXFocusedUIElement`, then inspect the focused element and its ancestors.
`AXUIElementCopyAttributeValue` can return `kAXErrorAttributeUnsupported`,
`kAXErrorNoValue`, `kAXErrorCannotComplete`, or `kAXErrorNotImplemented`; these
are normal capability failures, not reasons to synthesize Copy
([Apple](https://developer.apple.com/documentation/applicationservices/1462085-axuielementcopyattributevalue)).

The read order should be:

1. `AXSelectedText` on the system-wide focused element.
2. `AXSelectedText` on each parent up to the focused window, `AXWebArea`, and
   application root. Web/document selections often belong to an ancestor rather
   than the leaf with keyboard focus.
3. `AXSelectedTextRanges` for non-contiguous selections, then
   `AXSelectedTextRange` for a single range
   ([Apple single range](https://developer.apple.com/documentation/applicationservices/kaxselectedtextrangeattribute),
   [Apple multiple ranges](https://developer.apple.com/documentation/applicationservices/kaxselectedtextrangesattribute)).
4. Only when the same element exposes a string `AXValue`, slice that value by the
   selected range. Foundation string indices and ranges are UTF-16 code units, so
   Rust must validate the bounds in UTF-16 rather than byte or Unicode-scalar
   offsets
   ([Apple `NSString`](https://developer.apple.com/documentation/foundation/nsstring)).
5. Perform a small, role-aware search inside the focused window for a text area,
   document, or web area if the focused chain did not expose the selection.
   Bound it by time and node count. A blind whole-application traversal can pick
   up an unrelated stale selection and can be very expensive.

Permission should be checked with `AXIsProcessTrustedWithOptions`; the prompt is
asynchronous, and the function's return value still reflects the current trust
state
([Apple](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)).
The installed, signed bundle identity and path need to remain stable so the TCC
grant remains understandable to the user.

`NSWorkspace.frontmostApplication` must be resolved on the macOS UI thread
before capture is dispatched. Calling it from Mist's background selection
worker reaches HIToolbox input-source state and macOS terminates the process
with a dispatch-queue assertion. The worker receives only the captured PID and
uses `AXUIElementCreateApplication(pid)` from there.

### Chromium, Electron, and helper processes

Chromium's renderer processes send accessibility data to the browser process,
where per-frame trees are cached and dynamically composed into one virtual tree
for the native accessibility API
([Chromium architecture](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/accessibility/overview.md),
 [content accessibility data flow](https://chromium.googlesource.com/chromium/src/+/HEAD/content/browser/accessibility/README.md)).
Therefore:

- Query the system-wide focused element and the top-level application AX tree.
  Do **not** assume that enumerating every renderer/helper PID is the correct way
  to reach web content. Renderer PIDs are implementation details, churn between
  navigations, and are not the native accessibility root Chromium promises.
- If an Electron target has no web accessibility tree, set
  `AXManualAccessibility=true` on the **Electron application** AX element, then
  retry even when the first focused-element lookup was inconclusive, after the
  tree has had time to appear. Electron documents this exact
  third-party activation mechanism
  ([Electron accessibility guide](https://www.electronjs.org/docs/latest/tutorial/accessibility)).
- Treat failure to set that attribute as “unsupported by this target/version,”
  not as permission to inspect arbitrary child processes.
- Activation has a performance cost. Electron says accessibility support is off
  by default and warns that rendering its tree can significantly affect app
  performance
  ([Electron `app.accessibilitySupportEnabled`](https://github.com/electron/electron/blob/main/docs/api/app.md)).

Chromium accessibility is generally off by default and enabled on demand.
Chromium's official inspection instructions note that testing may require a
screen reader or `--force-renderer-accessibility`; Mist cannot require users to
relaunch arbitrary browsers with that flag
([Chromium overview](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/accessibility/overview.md),
 [inspection tools](https://chromium.googlesource.com/chromium/src/tools/+/HEAD/accessibility/inspect/README.md)).

### AppKit Service fallback

A macOS Service is the most deterministic clipboard-neutral fallback when the
front app supports Services. The requesting app puts its current selection on a
service pasteboard and invokes the provider through the responder chain; Mist
reads that request pasteboard, not `NSPasteboard.general`
([Apple Services flow](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/SysServices/Articles/using.html),
 [service declaration](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/SysServices/Articles/properties.html)).

This is not a silent global read. The user must invoke **Services → Speak
Selection with Mist** (or a Services shortcut), and the source application must
participate in the Services responder chain and advertise a compatible text
type. It is an excellent explicit fallback for TextEdit, many native editors,
and cooperating browsers, but it cannot make a custom canvas or nonparticipating
app expose text.

### macOS distribution and privacy boundaries

Apple states that using accessibility APIs in assistive apps is incompatible
with App Sandbox and that a sandboxed app cannot control another app
([current sandbox guidance](https://developer.apple.com/documentation/security/protecting-user-data-with-app-sandbox),
 [accessibility guidance](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Accessibility/cocoaAXIntro/cocoaAXintro.html)).
Plan on signed and notarized direct distribution rather than a normal Mac App
Store sandbox.

Reject secure fields before reading or speaking. macOS exposes the
`AXSecureTextField` subrole for fields intended to contain sensitive data
([Apple](https://developer.apple.com/documentation/applicationservices/kaxsecuretextfieldsubrole)).

## Windows

### UI Automation capture

Create `CUIAutomation`, call `GetFocusedElement`, and query the focused element
for `IUIAutomationTextPattern`. If it does not support TextPattern, walk **up**
the UIA parent chain to the nearest Edit/Document/WebView provider before giving
up; Microsoft documents `GetFocusedElement` as the way to obtain the focused
control and notes that clients may examine its descendants/related tree
([Microsoft](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-obtainingelements)).

For each provider:

1. Check `SupportedTextSelection`.
2. Call `IUIAutomationTextPattern::GetSelection`.
3. For each returned `IUIAutomationTextRange`, call `GetText(-1)` and concatenate
   the provider-ordered ranges with a sensible separator.
4. Reject a zero-length/degenerate range: Microsoft specifies that a caret with
   no selection is returned as an empty range
   ([TextPattern `GetSelection`](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationtextpattern),
   [TextPattern rules](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingtextandtextrange)).
5. Put strict timeouts around cross-process calls and discard stale elements
   when focus changes during capture.

`TextPattern2` does **not** provide a stronger selection API. It inherits
TextPattern and adds `GetCaretRange`, which returns a zero-length range at the
caret, plus annotation support. It can help diagnostics around caret ownership,
but it does not recover selected text missing from `GetSelection`
([Microsoft `IUIAutomationTextPattern2`](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomationtextpattern2)).

Standard Win32, Windows Forms, and WPF controls get Microsoft providers, while
custom controls must implement their own. Without a provider, a custom control
is largely opaque apart from basic window information
([Microsoft provider overview](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-providersoverview)).
This is the primary reason “any Windows text” is impossible.

### Chromium, Edge, Electron, and WebView2

Chromium supports MSAA/IAccessible2 and UI Automation, but its own documentation
still calls the native UIA provider under development; Chromium's inspection
tool defaults to IA2 on Windows
([Chromium UIA](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/accessibility/browser/uiautomation.md),
 [Chromium inspect tool](https://chromium.googlesource.com/chromium/src/+/HEAD/tools/accessibility/inspect/README.md)).

Recommended strategy:

- Keep UIA TextPattern as the first Windows path because it is the OS-standard
  API and works across native controls and Windows Terminal.
- Test Edge, Chrome, Electron/Codex, Teams, and each Outlook generation with the
  exact shipped versions. Accessibility is enabled on demand and the first query
  may expose a smaller or delayed tree.
- If Chromium/Firefox coverage is a release requirement, add an explicit
  IAccessible2 fallback. Chromium ships IA2 and implements text selection APIs;
  its source tests selection across text fields and multi-node documents
  ([Chromium IA2 source](https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/iaccessible2/ia2_api_all.idl),
  [Chromium selection tests](https://chromium.googlesource.com/chromium/src/+/HEAD/content/browser/accessibility/accessibility_win_browsertest.cc)).
  This improves browser coverage but still cannot bypass a missing accessibility
  tree or protected content.
- WebView2 normally appears as a child of its parent HWND in the accessibility
  tree. Composition hosts must correctly connect the WebView2 automation
  provider
  ([Microsoft WebView2 accessibility](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/overview-features-apis)).
  A broken custom host can therefore make otherwise accessible web content
  invisible to Mist.

Do not infer support from the product name. Word, classic Outlook, new Outlook,
Teams, embedded Office panes, and add-ins can use different native and web
providers in different regions/release channels. Microsoft confirms that some
Microsoft 365 features and Office add-ins use WebView2, not that every surface
has one uniform provider
([Microsoft 365 and WebView2](https://learn.microsoft.com/en-us/deployoffice/webview2-install)).

### Office and terminal expectations

- Word-like document controls *should* implement TextPattern because Microsoft
  says Edit and Document controls should support it, but the actual Office
  provider remains the authority
  ([Microsoft Text/TextRange patterns](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingtextandtextrange)).
- Excel cell selection is object/cell selection, not necessarily a text range.
  Editing text inside a cell and selecting a grid cell are different user
  intents; do not speak a selected cell by pretending it is selected text.
- Windows Terminal's official source implements `UIA_TextPatternId` and
  `GetSelection`, returning a range for the active terminal selection
  ([Microsoft Terminal source](https://github.com/microsoft/terminal/blob/main/src/types/ScreenInfoUiaProviderBase.cpp)).
  Conhost, third-party terminals, and embedded terminals still require separate
  tests.

Reject `UIA_IsPasswordPropertyId=true`; Microsoft identifies it as protected
content and warns against echoing it
([Microsoft](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-automation-element-propids)).

### Integrity and packaging

Same-user, same-integrity UIA is the normal supported case. For elevated apps,
either report “Mist cannot read selections from administrator apps” or ship a
proper accessibility product with `uiAccess=true`, a trusted signature, and an
administrator-writable install location such as Program Files. Do not run the
whole app as administrator merely to increase capture coverage
([Microsoft application manifests](https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests),
 [UIAccess policy](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/security-policy-settings/user-account-control-only-elevate-uiaccess-applications-that-are-installed-in-secure-locations)).

## Linux X11

### PRIMARY is not the normal clipboard

X11 selections are global, dynamically typed data owned by a client. The X
server does not store the content; a requestor calls `XConvertSelection`, the
current owner converts the requested target, and the server relays the result.
`PRIMARY` and `CLIPBOARD` are distinct selection atoms
([Xlib selection model](https://www.x.org/releases/X11R7.6/doc/libX11/specs/libX11/libX11.html),
 [ICCCM](https://www.x.org/releases/current/doc/xorg-docs/icccm/icccm.pdf)).

Reading `PRIMARY` therefore does not modify the user's normal `CLIPBOARD`.
It works well in conventional X11 editors, browsers, and terminals that claim
`PRIMARY` when the user highlights text. It still fails when:

- the app does not claim `PRIMARY` for its selection;
- the owner exits or replaces the selection before conversion completes;
- the owner does not offer a compatible text target;
- the request times out; or
- an X11 sandbox/security policy blocks the connection.

Negotiate text targets instead of assuming one encoding: request `TARGETS`,
prefer UTF-8 text, and retain `COMPOUND_TEXT`/`STRING` compatibility as needed.
Use bounded asynchronous reads because the owning app supplies the bytes.

### AT-SPI fallback

AT-SPI is independent of X11 selection ownership and can recover selections
from accessible controls that do not publish `PRIMARY`. For a focused object
implementing `Atspi.Text`, use `GetNSelections`, `GetSelection`, and `GetText`.
For a complex document selection spanning accessible objects, newer AT-SPI also
defines `Document.GetTextSelections`
([AT-SPI Text D-Bus API](https://gnome.pages.gitlab.gnome.org/at-spi2-core/devel-docs/doc-org.a11y.atspi.Text.html),
 [AT-SPI document selections](https://gnome.pages.gitlab.gnome.org/at-spi2-core/devel-docs/doc-org.a11y.atspi.Document.html)).

Chromium exposes ATK on Linux, but its accessibility engine is enabled on demand
and official test instructions may require Orca or
`--force-renderer-accessibility`
([Chromium](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/accessibility/overview.md)).
Mist should attempt AT-SPI, never silently require the browser flag, and report
an inactive/missing tree distinctly from “no selection.”

Reject `ATSPI_ROLE_PASSWORD_TEXT`, which AT-SPI defines for text that is not
shown visibly to the user
([AT-SPI roles](https://gnome.pages.gitlab.gnome.org/at-spi2-core/libatspi/enum.Role.html)).

## Linux Wayland

### Standard primary selection cannot support the background promise

The standard `primary-selection-unstable-v1` protocol models X11's select-to-copy
behavior, but only the keyboard-focused client receives a non-null offer. A
background tray/pet process is normally not that client
([official Wayland protocol](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/unstable/primary-selection/primary-selection-unstable-v1.xml)).

Stealing keyboard focus merely to receive the offer would disrupt the user's
workflow, can cause the source app to clear its selection, and gives Mist no
portable right to restore focus. It is not an acceptable implementation of a
background shortcut.

The standardized staging `ext-data-control-v1` protocol explicitly allows a
**privileged client** to act as a clipboard manager and receive normal and
primary selection offers
([official protocol source](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-data-control/ext-data-control-v1.xml)).
`wlr-data-control` provides a similar compositor-specific route. These are valid
optional capabilities, not a cross-desktop guarantee:

- a compositor may not implement or advertise them;
- a compositor/security layer may reserve them for trusted clipboard managers;
- primary-selection support and required protocol version can differ; and
- a sandbox may not receive the global even when an unsandboxed build does.

The Wayland GlobalShortcuts portal solves only shortcut delivery. It does not
return selected text. Likewise, the newer Clipboard portal is not a general
background-selection API: it extends RemoteDesktop or InputCapture sessions and
may restrict clipboard access to an active session
([GlobalShortcuts](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html),
 [Clipboard portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Clipboard.html)).
Opening a remote-desktop/input-capture permission session just to read a
selection is disproportionate and still addresses `CLIPBOARD`, not arbitrary
highlighted text.

### Wayland recommendation

Use this order:

1. GlobalShortcuts portal for the gesture; show the actual granted gesture.
2. AT-SPI selection capture, which runs over the accessibility bus and is not
   tied to Wayland focus-scoped data offers.
3. `ext-data-control-v1`, then supported `wlr-data-control` versions, when the
   compositor advertises them and policy permits primary-selection access.
4. User-initiated **Copy**, followed by **Speak copied text**; reading the
   existing clipboard is allowed, but Mist must not synthesize Copy.
5. Manual text input or an app-specific integration.

Flatpak confines host services and D-Bus by default; accessibility-bus and
Wayland/X11 access must be tested in the actual package permissions. Flatpak's
command reference distinguishes `--a11y-bus`, `--socket=wayland`, and X11 socket
access, and its general documentation emphasizes that portals mediate only the
interfaces they define
([Flatpak command reference](https://docs.flatpak.org/en/latest/flatpak-command-reference.html),
 [sandbox model](https://docs.flatpak.org/en/latest/basic-concepts.html)).

## Recommended per-OS fallback hierarchy

| Rank | macOS | Windows | Linux X11 | Linux Wayland |
|---|---|---|---|---|
| 1 | AX focused element + ancestors: `AXSelectedText` | UIA focused element + ancestors: TextPattern selection | `PRIMARY` conversion | AT-SPI Text/Document selection |
| 2 | AX selected ranges + same-element `AXValue` | Additional relevant UIA document ancestor/provider | AT-SPI Text/Document selection | `ext-data-control`, then supported wlr-data-control primary selection |
| 3 | Activate Electron AX tree and retry targeted search | IA2 fallback for Chromium/Firefox when justified | App-specific integration | App-specific integration |
| 4 | Explicit **Services → Speak Selection with Mist** | User manually copies; Mist reads existing clipboard | User manually copies; Mist reads existing clipboard | User manually copies; Mist reads existing clipboard |
| 5 | Manual text input | Manual text input | Manual text input | Manual text input |

At every rank, stop and report protected content rather than proceeding to a
weaker mechanism.

Mist's optional automatic macOS Copy fallback posts the fixed physical
Command-C chord with CoreGraphics. It deliberately does not use a keyboard
layout helper from the background capture worker: those helpers can enter
main-thread-only HIToolbox state. Clipboard change-token and fingerprint checks
still guard cleanup as described below.

### Why “save clipboard, press Copy, restore clipboard” is not acceptable

That technique still changes clipboard ownership, sequence/change counters,
notifications, clipboard history, and potentially clipboard-manager state. It
also cannot faithfully snapshot every delayed-rendered or application-private
format and has races with simultaneous user/app writes. It violates the stated
“does not alter the normal clipboard” requirement even if plain text is restored
afterward.

The safe fallback is explicit: the **user** chooses Copy, then Mist only reads
the already-current clipboard. Label it **Speak copied text**, not **Speak
selection**.

## Expected behavior by application class

These are engineering expectations, not support claims. The test matrix below
must establish the actual support envelope for every release.

| Application/content | Likely route | Important limitation |
|---|---|---|
| TextEdit, Notepad, GTK/Qt editors | Native AX/UIA/AT-SPI; X11 also PRIMARY | Read-only custom views may omit the text-selection interface even when editable controls work |
| Safari, Edge, Chrome, Firefox page text | Browser accessibility document; X11 PRIMARY | Accessibility engine may activate lazily; selection can span many DOM/accessibility nodes; canvas/custom rendering may expose nothing |
| Browser text inputs/contenteditable | Native text provider | Empty selection is a caret, not text; password fields must be rejected |
| Electron apps, Codex, VS Code, Slack-like clients | Chromium accessibility tree | Activation may be lazy; macOS can try `AXManualAccessibility`; do not chase renderer PIDs |
| Custom WebView/WebView2 host | Host-wired accessibility provider | Composition/custom hosting can omit or misplace the provider; test the host, not just the engine |
| Word/LibreOffice document | Document text provider; X11 PRIMARY | Complex/multi-object selections and protected documents vary by provider/version |
| Excel/spreadsheet grid | Usually object/cell selection, sometimes text while editing | A selected cell is not automatically a selected text range; define separate product behavior before supporting it |
| Teams and Outlook | Release-specific native/web provider | Message list selection, message body text selection, and composer selection are different controls; old/new/client/web versions must be separate cases |
| Apple Mail and other email clients | Native/web document provider | Message body, subject field, and message-list item have different semantics |
| Terminal, Windows Terminal, GNOME Terminal, Konsole, iTerm2 | Terminal AX/UIA/AT-SPI provider; X11 PRIMARY | Some terminals expose only the visible buffer; rectangular selections and remote TUIs need exact-text tests |
| PDF with real text | Viewer/browser accessibility provider or X11 PRIMARY | Scanned/image-only PDFs have no text selection for these APIs; OCR would be a separate feature and permission model |
| Canvas, game, remote-desktop stream, image | Usually none | Visible pixels are not selected text. Screen capture/OCR is not an equivalent selection API |

## Release test matrix

Record OS build, application version/channel, rendering mode (native/X11/
XWayland/Wayland), package type, route used, latency, exact returned Unicode,
shortcut result, and failure code. “Works once” is not a support result.

### macOS

Test on the oldest supported macOS and current release, on Intel if supported and
Apple Silicon:

- TextEdit: plain and rich text, editable and read-only.
- Safari, Chrome, Edge, Firefox: static paragraph across inline elements; input;
  textarea; contenteditable; shadow DOM; iframe; canvas; browser PDF.
- Word and Outlook: document/message body, compose field, subject field, and
  non-text list selection.
- Teams, Codex, VS Code: static content, editor, embedded terminal, and webview.
- Apple Mail, Notes, Preview PDF.
- Terminal and iTerm2: wrapped lines, multi-line and rectangular selection,
  emoji, combining marks, RTL text.
- Secure text field: assert no text is returned or spoken.
- Accessibility denied, granted while app is running, revoked while running.
- Signed installed build versus ad-hoc development build.
- Control-Space with one and multiple keyboard input sources; verify conflict
  detection and alternate-shortcut setup.
- Service invocation from each app that advertises Services.

For diagnostics, use Apple's Accessibility Inspector and Chromium's
`ax_dump_tree` against the target. Validate that Mist never returns a selected
string from an unfocused window after focus changes.

### Windows

Test Windows 10 (if supported) and current Windows 11, x64 and ARM64 where
applicable:

- Notepad, WordPad replacement/current native text editor, and a standard Win32
  edit control.
- Edge, Chrome, Firefox: the same web corpus as macOS, with browser accessibility
  initially inactive and after activation.
- WebView2 sample in windowed and composition hosting.
- Word, Excel (cell vs in-cell text), classic Outlook, new Outlook, and Office
  WebView2/add-in panes.
- Teams, Codex, VS Code, and their embedded editors/webviews.
- Windows Terminal, conhost, PowerShell ISE if supported, and one third-party
  terminal.
- Acrobat/browser PDF and image-only PDF.
- Standard app versus target run as Administrator; assert a clear integrity
  boundary error when Mist lacks UIAccess.
- Password controls; assert `UIA_IsPassword` prevents speech.
- Another process registering the preferred hotkey; assert fallback UX.
- Multiple disjoint ranges, empty caret range, very long selection, rapid focus
  changes, app exit mid-read, and provider timeout.

Use Microsoft Inspect or Accessibility Insights to confirm which element owns
TextPattern, and Chromium's tool with both `--api=uia` and `--api=ia2` to explain
browser differences.

### Linux X11

Test GNOME and KDE X11 sessions where still supported:

- GTK editor, Qt editor, LibreOffice Writer/Calc, Firefox, Chromium, an Electron
  app, GNOME Terminal, Konsole, and VS Code terminal.
- Verify PRIMARY owner, offered targets, UTF-8/legacy target negotiation, and
  owner exit during transfer.
- Verify `CLIPBOARD` owner/content/sequence stays unchanged after capture.
- App that highlights visually but never claims PRIMARY; verify AT-SPI fallback.
- Chromium with accessibility inactive/active.
- Conflicting XGrabKey owner and layouts where Space/keycode/modifier mappings
  differ.
- Native X11 app and XWayland app in a Wayland session as separate cases.
- Flatpak and unsandboxed packages as separate cases.

### Linux Wayland

Test at minimum GNOME/Mutter, KDE/KWin, and one wlroots compositor (for example
Sway), because protocol sets and portal backends differ:

- Portal availability/version, user denial, empty binding, reassigned trigger,
  persisted binding, and session restart.
- AT-SPI selection in GTK, Qt, Firefox, Chromium/Electron, LibreOffice, and a
  terminal.
- Native Wayland and XWayland variants of the same app.
- Presence/absence and version of `ext-data-control` and `wlr-data-control`;
  primary-selection presence; permission/policy failure.
- Demonstrate that standard primary-selection alone does not deliver a non-null
  offer while Mist remains unfocused.
- Flatpak versus native package, including accessibility-bus access.
- GNOME/KDE desktop lock screen and password controls; no capture or speech.
- Read existing clipboard only after explicit user Copy; confirm Mist never
  injects Ctrl+C.

### Cross-platform content corpus

Every app route should run these assertions:

- ASCII, non-Latin scripts, emoji outside the BMP, combining sequences, RTL,
  line breaks, tabs, and non-breaking spaces.
- Single word, paragraph, selection across formatting/nodes, and multiple ranges.
- No selection/caret only returns “Select text first,” never surrounding text.
- Selection changes between hotkey and API response: cancel stale result.
- Sensitive/protected element: reject.
- Large selection: enforce a documented size limit before synthesis.
- Clipboard data and ownership unchanged by selection capture.
- No selected text in logs, crash reports, analytics, window titles, or error
  messages.

## Privacy and security requirements

Accessibility and data-control permissions are powerful enough to read text from
other applications. Treat them as a sensitive-data capability:

- Capture only in direct response to the user shortcut or explicit Service/menu
  action. Do not poll selection changes or index background application text.
- Explain the permission in plain language before the OS prompt, including that
  Mist can read the currently selected text in other apps.
- Keep text in memory only as long as synthesis/playback requires; clear queued
  or cancelled text promptly.
- Perform synthesis locally as designed. Never include selected text in
  telemetry, diagnostics, panic messages, filenames, or update requests.
- Reject secure/password roles on every platform even if a buggy provider leaks a
  value.
- Impose size and timeout limits to prevent an accessibility provider from
  causing unbounded allocation or blocking the UI thread.
- Display which route is active in diagnostics (AX, Service, UIA, IA2, PRIMARY,
  AT-SPI, Wayland data-control, copied text) without displaying the captured
  content.
- Keep “read current clipboard” an explicit fallback. Never monitor clipboard
  history continuously.

Microsoft explicitly warns that data exposed through UIA is effectively public
to other code and providers must not expose sensitive text
([TextPattern security](https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/ui-automation-textpattern-overview)).
The product should assume the same sensitivity on macOS Accessibility, X11, and
AT-SPI.

## Product language to use

### Recommended

Short marketing/UI text:

> Select text in a supported app, then press your configured Speak shortcut.
> Mist reads it locally without changing your clipboard.

Capability explanation:

> Works with apps that expose their text selection through macOS Accessibility,
> Windows UI Automation, X11 PRIMARY, Linux accessibility, or supported Wayland
> selection protocols. Some protected, custom-drawn, sandboxed, or image-only
> content cannot be read. Copy & Speak is always available as a fallback.

Wayland-specific onboarding:

> Your desktop controls global shortcuts and selection access. Mist will show the
> shortcut your desktop grants. If this desktop does not expose selected text,
> copy it and choose Speak copied text.

### Avoid

- “Select **any** text anywhere.”
- “Works in every app.”
- “Ctrl+Space always speaks the selection.”
- “No clipboard needed” without qualifying Wayland/custom-provider failures.
- “Accessibility permission guarantees access.” Permission authorizes the
  query; it does not force the target app to expose a selection.
- “Wayland support” based solely on GlobalShortcuts registration. Shortcut
  delivery and selection access are different capabilities.

## Engineering consequences for Mist

No production code was changed as part of this research. The next implementation
work should be scoped as separate, testable changes:

### Gap table against the current adapters

This table reflects the repository as inspected on 2026-09-27. It is a code
assessment, not a claim that every listed path has passed the release matrix.

| Area | What Mist already does | Gap against the recommended design | Priority |
|---|---|---|---|
| Shared shortcut/UI | Registers fixed `Ctrl+Space`, surfaces registration failure, uses supported-app wording, and on Wayland displays the portal-granted trigger | The shortcut is not configurable. On macOS the fixed default can collide with Apple's input-source shortcut | Release blocker for broad distribution |
| macOS focus and selection | Starts from the system-wide `AXFocusedUIElement`, walks its ancestors, reads `AXSelectedText`, and falls back to `AXSelectedTextRange` plus UTF-16-correct `AXValue` slicing | Support `AXSelectedTextRanges` and `AXStringForRange`; make focused-window fallback role-aware; distinguish every caret/provider failure | High |
| macOS Chromium/Electron | Sets `AXManualAccessibility` only on the top-level application, retries, and bounds focused-window traversal to 512 nodes/16 levels | Make the remaining search role-aware and enforce an adapter-level time budget | High |
| macOS privacy/fallback | Checks Accessibility trust, rejects `AXSecureTextField`, and routes the AppKit Service through the visible queue | Validate the Service in each supported host; document and enforce the direct-distribution/no-App-Sandbox requirement | High for packaging |
| Windows selection | Calls UIA `GetFocusedElement`, walks parents for TextPattern, reads multiple non-empty ranges, and rejects `UIA_IsPassword` | Add per-provider timeout and stale-focus checks; validate the hierarchy on Windows hardware | High |
| Windows browser/elevation | Uses the OS-standard UIA path | Measure Chromium/Electron coverage and add IA2 only if the matrix justifies it; return an explicit integrity-boundary error for elevated targets, or meet the signing/install requirements for UIAccess | High for diagnostics; IA2 conditional |
| X11 selection | Reads `PRIMARY`, tries `UTF8_STRING` then `STRING`, applies a 500 ms timeout, and leaves `CLIPBOARD` unchanged | Negotiate `TARGETS` and legacy `TEXT`/`COMPOUND_TEXT`; add AT-SPI for apps that expose accessibility selection but not PRIMARY | Medium/High |
| Wayland selection | Reads PRIMARY using data-control-capable `wl-clipboard-rs` and reports when the compositor does not expose it | Add AT-SPI as the first semantic path; capability-detect `ext-data-control` and `wlr-data-control` separately; clearly label data-control coverage as compositor-dependent | High |
| Wayland shortcut | Uses the GlobalShortcuts portal and displays its returned trigger description | Keep shortcut capability separate from selection capability in diagnostics and onboarding; handle portal denial/no binding as a supported degraded state | High |
| Clipboard fallback | Explicit **Speak copied text** is available; optional automatic Copy runs only after direct capture fails; fallback items carry a clipboard fingerprint/change token and enter the same queue | Validate synthetic Copy on Windows/Linux desktops; document that change-token checks narrow races but the supported OS APIs do not provide one portable atomic compare-and-clear operation; Linux also cannot distinguish a same-text recopy without a stronger compositor/X11 token | High for cross-platform validation |
| Errors/telemetry | Has generic registration, permission, timeout, and capture errors; validates a global selection-size limit | Add typed failure reasons and privacy-safe route diagnostics; reject protected roles on every OS; confirm raw text, titles, and document names never reach logs/analytics | Release blocker for supportability/privacy |

The current implementation is therefore a credible **best-effort prototype for
supported apps**, not evidence for “any text anywhere.” The product claim should
be narrowed before expanding the adapters, because even a complete implementation
of every gap above remains bounded by target providers and Wayland compositor
policy.

Remaining work after the queue/clipboard follow-up implementation:

1. Make the shortcut configurable on all platforms, verify registration, and
   show the actual/alternate gesture. Control-Space is a poor macOS default.
2. macOS: add multiple/parameterized selected-range support and make the
   bounded focused-window search role-aware and deadline-aware.
3. Windows: validate focused-element/ancestor TextPattern capture on Windows,
   add provider-call timeouts, and decide whether IA2 browser coverage justifies
   its maintenance cost.
4. Linux: add an AT-SPI adapter. Use X11 PRIMARY first on X11 and AT-SPI first on
   Wayland.
5. Wayland: capability-detect GlobalShortcuts, AT-SPI, ext-data-control, and
   wlr-data-control independently. Never infer selection support from shortcut
   success.
6. Add typed failure reasons—permission denied, shortcut conflict, no selection,
   protected content, provider unsupported/inactive, provider timeout, integrity
   boundary, compositor protocol missing, portal denied—so the UI can offer the
   correct recovery instead of a generic error.
7. Measure and publish the actual supported-application matrix on each target
   OS; keep the supported-app wording until those results exist.
