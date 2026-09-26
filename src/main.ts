import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Profile = { id: string; label: string; customName: string | null };
type RemovedAccount = { profile: Profile; email: string | null };
type ProfileList = { profiles: Profile[]; notice: string | null };
type WindowUsage = { durationMinutes: number; remainingPercent: number; resetsAt: number | null };
type AccountStatus = {
  profile: string;
  email: string | null;
  plan: string | null;
  signedIn: boolean;
  windows: WindowUsage[];
  usageError: string | null;
  error: string | null;
  desktopIdentityVerified: boolean;
};
type ShellStatus = {
  version: string;
  launchAtLogin: boolean;
  launchAtLoginAvailable: boolean;
  startupError: string | null;
};
type ConnectResult = { profile: Profile; launched: boolean; message: string };

const accounts = document.querySelector<HTMLElement>("#accounts")!;
const updated = document.querySelector<HTMLElement>("#updated")!;
const message = document.querySelector<HTMLElement>("#message")!;
const refreshButton = document.querySelector<HTMLButtonElement>("#refresh")!;
const addButton = document.querySelector<HTMLButtonElement>("#add-account")!;
const launchToggle = document.querySelector<HTMLInputElement>("#launch-at-login")!;
const panelHeading = document.querySelector<HTMLElement>("#panel-heading")!;
const settingsButton = document.querySelector<HTMLButtonElement>("#settings-button")!;
const backButton = document.querySelector<HTMLButtonElement>("#back-button")!;
const removedCard = document.querySelector<HTMLElement>("#removed-card")!;
const removedAccounts = document.querySelector<HTMLElement>("#removed-accounts")!;
const appVersion = document.querySelector<HTMLElement>("#app-version")!;
const updateStatus = document.querySelector<HTMLElement>("#update-status")!;
const checkUpdateButton = document.querySelector<HTMLButtonElement>("#check-update")!;
const viewUpdateButton = document.querySelector<HTMLButtonElement>("#view-update")!;
let generation = 0;
let latestProfiles: Profile[] = [];
let statuses = new Map<string, AccountStatus>();
let panelOpen = false;
let editingProfile: string | null = null;
let editingDraft = "";
let removingProfile: string | null = null;
let lastUpdateCheck = 0;
let pendingProfileNotice: string | null = null;

function versionParts(value: string): number[] | null {
  const match = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(value.trim());
  return match ? match.slice(1).map(Number) : null;
}

function isNewerVersion(latest: string, installed: string): boolean {
  const a = versionParts(latest);
  const b = versionParts(installed);
  if (!a || !b) return false;
  for (let index = 0; index < 3; index++) {
    if (a[index] !== b[index]) return a[index] > b[index];
  }
  return false;
}

async function checkForUpdates(force = false) {
  if (checkUpdateButton.disabled || (!force && Date.now() - lastUpdateCheck < 6 * 60 * 60 * 1000)) return;
  checkUpdateButton.disabled = true;
  updateStatus.textContent = "Checking for updates…";
  const timeout = new AbortController();
  const timer = window.setTimeout(() => timeout.abort(), 8_000);
  try {
    const [shell, response] = await Promise.all([
      invoke<ShellStatus>("get_shell_status"),
      fetch("https://api.github.com/repos/Ved-Joshi/codex-switcher/releases/latest", {
        headers: { Accept: "application/vnd.github+json" },
        cache: "no-store",
        signal: timeout.signal,
      }),
    ]);
    if (response.status === 404) {
      updateStatus.textContent = "No releases published yet.";
      viewUpdateButton.hidden = true;
      settingsButton.dataset.update = "";
      lastUpdateCheck = Date.now();
      return;
    }
    if (!response.ok) throw new Error(`GitHub returned ${response.status}`);
    const release = await response.json() as { tag_name?: string };
    if (!release.tag_name || !versionParts(release.tag_name)) throw new Error("The release has no valid version tag");
    const available = isNewerVersion(release.tag_name, shell.version);
    updateStatus.textContent = available ? `${release.tag_name} is available.` : "You have the latest release.";
    viewUpdateButton.hidden = !available;
    settingsButton.dataset.update = available ? "available" : "";
    settingsButton.title = available ? "Settings, update available" : "Settings";
    settingsButton.setAttribute("aria-label", settingsButton.title);
    lastUpdateCheck = Date.now();
  } catch (error) {
    updateStatus.textContent = `Could not check for updates: ${String(error)}`;
  } finally {
    window.clearTimeout(timer);
    checkUpdateButton.disabled = false;
  }
}

function notify(value: string, error = false) {
  message.textContent = value;
  message.dataset.kind = error ? "error" : "note";
  message.hidden = false;
}

function make(tag: string, className: string, value?: string): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  if (value !== undefined) element.textContent = value;
  return element;
}

function usageWindow(status: AccountStatus | undefined, minutes: number, title: string): HTMLElement {
  const tile = make("div", "usage-tile");
  const heading = make("span", "usage-title", title);
  const value = make("strong", "usage-value");
  const summary = make("div", "usage-summary");
  summary.append(heading, value);
  const meter = make("div", "meter");
  const fill = make("span", "meter-fill");
  meter.append(fill);
  const window = status?.windows.find((entry) => entry.durationMinutes === minutes);
  if (window) {
    value.textContent = `${window.remainingPercent}% left`;
    fill.style.width = `${Math.max(0, Math.min(100, window.remainingPercent))}%`;
    meter.setAttribute("role", "progressbar");
    meter.setAttribute("aria-label", `${title} remaining`);
    meter.setAttribute("aria-valuenow", String(window.remainingPercent));
    meter.setAttribute("aria-valuemin", "0");
    meter.setAttribute("aria-valuemax", "100");
  } else {
    value.textContent = status ? "—" : "···";
    tile.classList.add("unavailable");
  }
  tile.append(summary, meter);
  if (window?.resetsAt) {
    const reset = new Date(window.resetsAt * 1000);
    if (Number.isFinite(reset.getTime())) {
      const formatted = new Intl.DateTimeFormat(undefined, {
        weekday: minutes >= 10080 ? "short" : undefined,
        hour: "numeric", minute: "2-digit",
      }).format(reset);
      tile.append(make("span", "usage-reset", `Resets ${formatted}`));
    }
  }
  return tile;
}

function duplicateEmails(): Set<string> {
  const counts = new Map<string, number>();
  for (const status of statuses.values()) {
    if (status.email) {
      const key = status.email.toLowerCase();
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
  }
  return new Set([...counts].filter(([, count]) => count > 1).map(([email]) => email));
}

function displayName(profile: Profile, status?: AccountStatus): string {
  return profile.customName || status?.email || profile.label;
}

function planLabel(plan: string): string {
  const name = plan.trim().replace(/\s+plan$/i, "");
  const known = { plus: "Plus", pro: "Pro", max: "Max" } as const;
  return known[name.toLowerCase() as keyof typeof known]
    ?? name.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

async function saveName(profile: Profile) {
  const name = editingDraft.trim();
  try {
    await invoke("set_profile_name", { profile: profile.id, name });
    profile.customName = name || null;
    editingProfile = null;
    render();
  } catch (error) {
    notify(`Could not save account name: ${String(error)}`, true);
  }
}

function render() {
  const activeEdit = document.activeElement?.classList.contains("rename-input") ?? false;
  const caret = activeEdit ? (document.activeElement as HTMLInputElement).selectionStart : null;
  const listScrollTop = accounts.scrollTop;
  accounts.replaceChildren();
  const duplicates = duplicateEmails();
  const visibleProfiles = latestProfiles.filter((profile) => statuses.has(profile.id));
  if (visibleProfiles.length === 0) {
    accounts.append(make("p", "empty-state", updated.textContent === "Refreshing…"
      ? "Checking connected accounts…" : "No connected accounts. Connect one here or restore one in Settings."));
    return;
  }
  for (const profile of visibleProfiles) {
    const status = statuses.get(profile.id);
    const card = make("article", "account-card");
    card.dataset.profile = profile.id;
    const header = make("div", "account-head");
    const details = make("div", "identity-text");
    const nameRow = make("div", "name-row");
    if (editingProfile === profile.id) {
      const input = make("input", "rename-input") as HTMLInputElement;
      input.type = "text";
      input.maxLength = 48;
      input.value = editingDraft;
      input.placeholder = status?.email ?? "Account name";
      input.setAttribute("aria-label", `Name for ${profile.label}`);
      input.addEventListener("input", () => { editingDraft = input.value; });
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter") void saveName(profile);
        if (event.key === "Escape") { event.stopPropagation(); editingProfile = null; render(); }
      });
      const save = make("button", "rename-action", "Save") as HTMLButtonElement;
      save.type = "button";
      save.addEventListener("click", () => void saveName(profile));
      const cancel = make("button", "rename-cancel", "Cancel") as HTMLButtonElement;
      cancel.type = "button";
      cancel.addEventListener("click", () => { editingProfile = null; render(); });
      nameRow.append(input, save, cancel);
    } else {
      nameRow.append(make("strong", "account-name", displayName(profile, status)));
      const rename = make("button", "rename-button", "✎") as HTMLButtonElement;
      rename.type = "button";
      rename.setAttribute("aria-label", `Edit name for ${displayName(profile, status)}`);
      rename.title = "Edit account name";
      rename.addEventListener("click", () => {
        editingProfile = profile.id;
        editingDraft = profile.customName ?? "";
        render();
        accounts.querySelector<HTMLInputElement>(".rename-input")?.focus();
      });
      nameRow.append(rename);
    }
    details.append(nameRow);
    const subtitle = [profile.customName ? status?.email : null, status?.plan ? planLabel(status.plan) : null]
      .filter(Boolean).join(" · ");
    if (subtitle) details.append(make("span", "account-email", subtitle));
    const open = make("button", "open-button", status?.signedIn ? "Open" : "Sign in") as HTMLButtonElement;
    open.type = "button";
    open.setAttribute("aria-label", `${status?.signedIn ? "Open" : "Sign in to"} ${displayName(profile, status)}`);
    open.addEventListener("click", async () => {
      open.disabled = true;
      try {
        const result = await invoke<string>("open_account", { profile: profile.id });
        notify(`${displayName(profile, status)} ${result.toLowerCase()}. Check the account shown in Codex before starting work.`);
        void invoke("hide_panel");
      } catch (error) {
        notify(`Could not open ${displayName(profile, status)}: ${String(error)}`, true);
      } finally {
        open.disabled = false;
      }
    });
    const actions = make("div", "account-actions");
    const remove = make("button", "remove-button", "Remove") as HTMLButtonElement;
    remove.type = "button";
    remove.setAttribute("aria-label", `Remove ${displayName(profile, status)} from switcher`);
    remove.addEventListener("click", () => {
      removingProfile = profile.id;
      render();
      accounts.querySelector<HTMLButtonElement>(".remove-confirm .confirm-remove")?.focus();
    });
    actions.append(open, remove);
    header.append(details, actions);
    card.append(header);
    if (status?.email && duplicates.has(status.email.toLowerCase())) {
      card.append(make("p", "card-alert", "This sign-in also appears in another profile."));
    }
    const usage = make("div", "usage-grid");
    usage.append(usageWindow(status, 300, "5 hour"), usageWindow(status, 10080, "Weekly"));
    card.append(usage);
    if (status && (!status.signedIn || status.usageError || status.error)) {
      card.append(make("p", "card-note", status.error ?? status.usageError ?? "Sign in through Codex to see usage."));
    }
    if (removingProfile === profile.id) {
      const confirm = make("div", "remove-confirm");
      confirm.setAttribute("role", "group");
      confirm.setAttribute("aria-label", `Confirm removal of ${displayName(profile, status)}`);
      confirm.append(make("p", "", "Remove this account from the switcher? Its Codex data stays on this Mac."));
      const confirmActions = make("div", "remove-confirm-actions");
      const cancel = make("button", "", "Cancel") as HTMLButtonElement;
      cancel.type = "button";
      cancel.addEventListener("click", () => {
        removingProfile = null;
        render();
        accounts.querySelector<HTMLElement>(`.account-card[data-profile="${profile.id}"] .remove-button`)?.focus();
      });
      const confirmRemove = make("button", "confirm-remove", "Remove account") as HTMLButtonElement;
      confirmRemove.type = "button";
      confirmRemove.addEventListener("click", async () => {
        confirmRemove.disabled = true;
        try {
          await invoke("remove_account", { profile: profile.id });
          removingProfile = null;
          await refresh();
          notify(`${displayName(profile, status)} was removed from the switcher. You can restore it in Settings.`);
        } catch (error) {
          notify(`Could not remove account: ${String(error)}`, true);
          confirmRemove.disabled = false;
        }
      });
      confirmActions.append(cancel, confirmRemove);
      confirm.append(confirmActions);
      card.append(confirm);
    }
    accounts.append(card);
  }
  if (activeEdit) {
    const input = accounts.querySelector<HTMLInputElement>(".rename-input");
    input?.focus();
    if (caret !== null) input?.setSelectionRange(caret, caret);
  }
  accounts.scrollTop = listScrollTop;
}

async function refresh() {
  const current = ++generation;
  refreshButton.disabled = true;
  refreshButton.classList.add("spinning");
  updated.textContent = "Refreshing…";
  try {
    const result = await invoke<ProfileList>("list_profiles");
    if (current !== generation) return;
    const profiles = result.profiles;
    if (result.notice) {
      if (panelOpen) notify(result.notice);
      else pendingProfileNotice = result.notice;
    }
    latestProfiles = profiles;
    const ids = new Set(profiles.map((profile) => profile.id));
    if (removingProfile && !ids.has(removingProfile)) removingProfile = null;
    statuses = new Map([...statuses].filter(([id]) => ids.has(id)));
    render();
    let index = 0;
    const workers = Array.from({ length: Math.min(3, profiles.length) }, async () => {
      while (index < profiles.length && current === generation) {
        const profile = profiles[index++];
        try {
          const status = await invoke<AccountStatus>("get_account_status", { profile: profile.id });
          if (current === generation) {
            const previous = statuses.get(profile.id);
            if (status.error && previous?.email) {
              status.email = previous.email;
              status.plan = previous.plan;
              status.signedIn = true;
              status.usageError = status.error;
            }
            statuses.set(profile.id, status);
          }
        } catch (error) {
          if (current === generation) notify(`Could not read ${profile.label}: ${String(error)}`, true);
        }
        if (current === generation) render();
      }
    });
    await Promise.all(workers);
    if (current === generation) {
      updated.textContent = `Updated ${new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" }).format(new Date())}`;
      render();
    }
  } catch (error) {
    if (current === generation) {
      updated.textContent = "Unavailable";
      notify(`Could not load accounts: ${String(error)}`, true);
    }
  } finally {
    if (current === generation) {
      refreshButton.disabled = false;
      refreshButton.classList.remove("spinning");
    }
  }
}

async function refreshSettings() {
  try {
    const [status, removed] = await Promise.all([
      invoke<ShellStatus>("get_shell_status"),
      invoke<RemovedAccount[]>("list_removed_profiles"),
    ]);
    launchToggle.checked = status.launchAtLogin;
    launchToggle.disabled = !status.launchAtLoginAvailable;
    appVersion.textContent = `version ${status.version}`;
    renderRemovedAccounts(removed);
    if (status.startupError) notify(status.startupError, true);
  } catch (error) {
    notify(`Could not read settings: ${String(error)}`, true);
  }
}

function renderRemovedAccounts(removed: RemovedAccount[]) {
  removedAccounts.replaceChildren();
  for (const { profile, email } of removed) {
    const row = make("div", "managed-row");
    const accountName = email || profile.customName || profile.label;
    row.append(make("span", "managed-name", accountName));
    const button = make("button", "manage-button", "Restore") as HTMLButtonElement;
    button.type = "button";
    const alreadyConnected = !!email && [...statuses.values()].some((status) =>
      status.email?.trim().toLowerCase() === email.trim().toLowerCase(),
    );
    button.textContent = alreadyConnected ? "Already connected" : "Restore";
    button.disabled = alreadyConnected;
    button.setAttribute("aria-label", alreadyConnected ? `${accountName} is already connected` : `Restore ${accountName}`);
    button.addEventListener("click", async () => {
      button.disabled = true;
      try {
        await invoke("restore_account", { profile: profile.id });
        await refresh();
        await refreshSettings();
      } catch (error) {
        notify(`Could not restore account: ${String(error)}`, true);
        button.disabled = false;
      }
    });
    row.append(button);
    removedAccounts.append(row);
  }
  removedCard.hidden = removed.length === 0;
}

function showSettings(show: boolean) {
  if (show && removingProfile) {
    removingProfile = null;
    render();
  }
  document.querySelector<HTMLElement>("#accounts-view")!.hidden = show;
  document.querySelector<HTMLElement>("#settings-view")!.hidden = !show;
  panelHeading.textContent = show ? "Settings" : "Accounts";
  updated.hidden = show;
  refreshButton.hidden = show;
  settingsButton.hidden = show;
  backButton.hidden = !show;
  if (show) void refreshSettings();
  if (show) void checkForUpdates();
}

const panel = document.querySelector<HTMLElement>(".panel")!;
let lastPanelHeight = 0;
new ResizeObserver(() => {
  const height = Math.ceil(document.body.getBoundingClientRect().height);
  if (height !== lastPanelHeight) {
    lastPanelHeight = height;
    void invoke("resize_panel", { height });
  }
}).observe(panel);

refreshButton.addEventListener("click", () => void refresh());
addButton.addEventListener("click", async () => {
  addButton.disabled = true;
  try {
    const result = await invoke<ConnectResult>("connect_account");
    notify(result.message, !result.launched);
    await refresh();
  } catch (error) {
    notify(`Could not connect another account: ${String(error)}`, true);
  } finally {
    addButton.disabled = false;
  }
});
document.querySelector("#settings-button")!.addEventListener("click", () => showSettings(true));
document.querySelector("#back-button")!.addEventListener("click", () => showSettings(false));
document.querySelector("#quit")!.addEventListener("click", () => void invoke("quit_switcher"));
checkUpdateButton.addEventListener("click", () => void checkForUpdates(true));
viewUpdateButton.addEventListener("click", async () => {
  try { await invoke("open_releases_page"); }
  catch (error) { notify(`Could not open update: ${String(error)}`, true); }
});
launchToggle.addEventListener("change", async () => {
  launchToggle.disabled = true;
  try {
    await invoke("set_launch_at_login", { enabled: launchToggle.checked });
  } catch (error) {
    notify(`Could not change login setting: ${String(error)}`, true);
  }
  await refreshSettings();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") void invoke("hide_panel");
});
void listen("panel-opened", () => {
  panelOpen = true;
  removingProfile = null;
  render();
  showSettings(false);
  message.hidden = true;
  if (pendingProfileNotice) {
    notify(pendingProfileNotice);
    pendingProfileNotice = null;
  }
  void refresh();
  void checkForUpdates();
});
void listen("panel-closed", () => { panelOpen = false; });
window.setInterval(() => {
  if (panelOpen && !refreshButton.disabled) void refresh();
}, 30_000);
void refresh();
