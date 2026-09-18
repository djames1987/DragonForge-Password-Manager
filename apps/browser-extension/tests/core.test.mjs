import test from "node:test";
import assert from "node:assert/strict";
import {
  PROTOCOL_VERSION,
  credentialRequest,
  normalizedHost,
  sameSite,
  searchRequest,
  statusRequest,
} from "../src/core.js";

test("protocol builders stay versioned and narrowly scoped", () => {
  assert.deepEqual(statusRequest(), { version: PROTOCOL_VERSION, action: "status" });
  assert.deepEqual(searchRequest("https://example.com/login", " alice "), {
    version: PROTOCOL_VERSION,
    action: "search",
    pageUrl: "https://example.com/login",
    query: "alice",
  });
  assert.deepEqual(credentialRequest("https://example.com", "item-1"), {
    version: PROTOCOL_VERSION,
    action: "credential",
    pageUrl: "https://example.com",
    itemId: "item-1",
  });
});

test("site comparison accepts www variant but rejects different sites", () => {
  assert.equal(normalizedHost("https://www.example.com/login"), "example.com");
  assert.equal(sameSite("https://example.com", "https://www.example.com/account"), true);
  assert.equal(sameSite("https://example.com", "https://evil.example.net"), false);
  assert.equal(sameSite("chrome://extensions", "https://example.com"), false);
});
