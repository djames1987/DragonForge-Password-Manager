import {
  NATIVE_HOST_NAME,
  browserError,
  credentialRequest,
  sameSite,
  searchRequest,
  statusRequest,
} from "./core.js";

function sendNative(request) {
  return new Promise((resolve, reject) => {
    chrome.runtime.sendNativeMessage(NATIVE_HOST_NAME, request, (response) => {
      const runtimeError = chrome.runtime.lastError;
      if (runtimeError) {
        reject(new Error("DragonForge desktop connection is unavailable."));
        return;
      }
      if (!response) {
        reject(new Error("DragonForge desktop returned no response."));
        return;
      }
      resolve(response);
    });
  });
}

async function activeWebTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id || !tab.url || !/^https?:/i.test(tab.url)) {
    throw new Error("Open an HTTP or HTTPS website before using DragonForge.");
  }
  return tab;
}

async function status() {
  const response = await sendNative(statusRequest());
  if (!response.ok) throw new Error(browserError(response));
  return response.status;
}

async function search(query) {
  const tab = await activeWebTab();
  const response = await sendNative(searchRequest(tab.url, query));
  if (!response.ok) throw new Error(browserError(response));
  return {
    pageUrl: tab.url,
    items: Array.isArray(response.items) ? response.items : [],
  };
}

async function fill(itemId, pageUrl) {
  const tab = await activeWebTab();
  if (!sameSite(tab.url, pageUrl)) {
    throw new Error("The active tab changed sites. Reopen DragonForge and try again.");
  }

  const response = await sendNative(credentialRequest(tab.url, itemId));
  if (!response.ok) throw new Error(browserError(response));
  const credential = response.credential;
  if (!credential || typeof credential.password !== "string") {
    throw new Error("DragonForge returned an invalid credential response.");
  }

  const results = await chrome.scripting.executeScript({
    target: { tabId: tab.id },
    func: fillLoginFields,
    args: [credential.username || "", credential.password],
  });
  const outcome = results?.[0]?.result;
  if (!outcome?.passwordFilled) {
    throw new Error("No visible password field was found on this page.");
  }

  return outcome;
}

function fillLoginFields(username, password) {
  const visible = (element) => {
    const style = window.getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return !element.disabled
      && !element.readOnly
      && style.display !== "none"
      && style.visibility !== "hidden"
      && rect.width > 0
      && rect.height > 0;
  };

  const passwordFields = [...document.querySelectorAll('input[type="password"]')].filter(visible);
  const passwordField = passwordFields[0];
  if (!passwordField) return { usernameFilled: false, passwordFilled: false };

  const form = passwordField.form || passwordField.closest("form");
  const scope = form || document;
  const usernameCandidates = [
    ...scope.querySelectorAll(
      'input[autocomplete="username"], input[type="email"], input[type="text"], input:not([type])'
    ),
  ].filter((input) => input !== passwordField && visible(input));

  const usernameField = usernameCandidates.find((input) => {
    if (!form) return true;
    const relation = input.compareDocumentPosition(passwordField);
    return Boolean(relation & Node.DOCUMENT_POSITION_FOLLOWING);
  }) || usernameCandidates[0];

  const setValue = (input, value) => {
    const descriptor = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
    if (descriptor?.set) descriptor.set.call(input, value);
    else input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  };

  if (usernameField && username) setValue(usernameField, username);
  setValue(passwordField, password);

  return {
    usernameFilled: Boolean(usernameField && username),
    passwordFilled: true,
  };
}

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  const handle = async () => {
    switch (message?.type) {
      case "status":
        return { ok: true, status: await status() };
      case "search":
        return { ok: true, ...(await search(message.query || "")) };
      case "fill":
        return { ok: true, result: await fill(message.itemId, message.pageUrl) };
      default:
        throw new Error("Unsupported DragonForge extension request.");
    }
  };

  handle()
    .then(sendResponse)
    .catch((error) => sendResponse({ ok: false, error: String(error.message || error) }));
  return true;
});
