export const PROTOCOL_VERSION = 1;
export const NATIVE_HOST_NAME = "com.dragonforge.passwordmanager";

export function statusRequest() {
  return { version: PROTOCOL_VERSION, action: "status" };
}

export function searchRequest(pageUrl, query = "") {
  return {
    version: PROTOCOL_VERSION,
    action: "search",
    pageUrl: String(pageUrl),
    query: String(query || "").trim(),
  };
}

export function credentialRequest(pageUrl, itemId) {
  return {
    version: PROTOCOL_VERSION,
    action: "credential",
    pageUrl: String(pageUrl),
    itemId: String(itemId),
  };
}

export function normalizedHost(value) {
  try {
    const url = new URL(value);
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    return url.hostname.toLowerCase().replace(/^www\./, "").replace(/\.$/, "");
  } catch {
    return null;
  }
}

export function sameSite(left, right) {
  const leftHost = normalizedHost(left);
  const rightHost = normalizedHost(right);
  return Boolean(leftHost && rightHost && leftHost === rightHost);
}

export function browserError(response, fallback = "DragonForge browser integration failed.") {
  if (response?.error?.message) return response.error.message;
  return fallback;
}
