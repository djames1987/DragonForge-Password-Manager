const $ = (id) => document.getElementById(id);

const views = ["offline-view", "locked-view", "unsupported-view", "vault-view"];
let currentPageUrl = "";
let searchTimer = null;

function showView(id) {
  for (const view of views) $(view).classList.toggle("hidden", view !== id);
}

function send(message) {
  return new Promise((resolve) => {
    chrome.runtime.sendMessage(message, (response) => {
      const runtimeError = chrome.runtime.lastError;
      if (runtimeError) {
        resolve({ ok: false, error: "DragonForge extension background service is unavailable." });
        return;
      }
      resolve(response || { ok: false, error: "DragonForge returned no response." });
    });
  });
}

async function currentTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  return tab;
}

function siteLabel(url) {
  try {
    return new URL(url).hostname;
  } catch {
    return "Password Manager";
  }
}

function initials(name) {
  const parts = String(name || "").trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return "?";
  return (parts.length === 1 ? parts[0].slice(0, 2) : parts[0][0] + parts[1][0]).toUpperCase();
}

async function initialize() {
  $("status-dot").className = "status-dot";
  const tab = await currentTab();
  currentPageUrl = tab?.url || "";
  $("site-label").textContent = siteLabel(currentPageUrl);

  if (!/^https?:/i.test(currentPageUrl)) {
    showView("unsupported-view");
    return;
  }

  const status = await send({ type: "status" });
  if (!status.ok) {
    $("offline-message").textContent =
      status.error || "Start DragonForge Password Manager, then reopen this extension.";
    showView("offline-view");
    return;
  }
  if (!status.status?.unlocked) {
    $("status-dot").classList.add("locked");
    showView("locked-view");
    return;
  }

  $("status-dot").classList.add("connected");
  showView("vault-view");
  await refresh();
}

async function refresh() {
  const response = await send({ type: "search", query: $("search-input").value });
  if (!response.ok) {
    if (/locked/i.test(response.error || "")) {
      $("status-dot").className = "status-dot locked";
      showView("locked-view");
    } else {
      $("status-dot").className = "status-dot";
      $("offline-message").textContent =
        response.error || "DragonForge desktop connection is unavailable.";
      showView("offline-view");
    }
    return;
  }

  currentPageUrl = response.pageUrl;
  render(response.items || []);
}

function render(items) {
  const results = $("results");
  results.replaceChildren();
  $("result-count").textContent = String(items.length);
  $("empty-results").classList.toggle("hidden", items.length !== 0);

  for (const item of items) {
    const row = document.createElement("div");
    row.className = "result";

    const avatar = document.createElement("div");
    avatar.className = "avatar";
    avatar.textContent = initials(item.name);

    const copy = document.createElement("div");
    copy.className = "result-copy";
    const title = document.createElement("div");
    title.className = "result-title";
    title.textContent = item.name;
    if (item.favorite) {
      const star = document.createElement("span");
      star.className = "favorite";
      star.textContent = "★";
      title.appendChild(star);
    }
    const username = document.createElement("div");
    username.className = "result-user";
    username.textContent = item.username || "No username";
    copy.append(title, username);

    const fill = document.createElement("button");
    fill.className = "fill";
    fill.textContent = "Fill";
    fill.addEventListener("click", async () => {
      fill.disabled = true;
      fill.textContent = "Filling…";
      const response = await send({
        type: "fill",
        itemId: item.id,
        pageUrl: currentPageUrl,
      });
      if (response.ok) {
        fill.textContent = "Filled";
        setTimeout(() => window.close(), 350);
      } else {
        fill.disabled = false;
        fill.textContent = "Retry";
        title.title = response.error || "Unable to fill this login.";
      }
    });

    row.append(avatar, copy, fill);
    results.appendChild(row);
  }
}

$("retry-button").addEventListener("click", initialize);
$("locked-retry").addEventListener("click", initialize);
$("search-input").addEventListener("input", () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(refresh, 160);
});

initialize();
