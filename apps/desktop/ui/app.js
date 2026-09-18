(() => {
  "use strict";

  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) {
    document.body.textContent = "DragonForge requires the Tauri desktop runtime.";
    return;
  }

  const state = {
    status: null,
    items: [],
    selectedId: null,
    selectedItem: null,
    filter: "all",
    search: "",
    passwordVisible: false,
    editorKind: "login",
  };

  const $ = (id) => document.getElementById(id);
  const welcomeView = $("welcome-view");
  const vaultView = $("vault-view");

  function toast(message, type = "success") {
    const node = document.createElement("div");
    node.className = "toast " + type;
    node.textContent = message;
    $("toast-region").appendChild(node);
    window.setTimeout(() => node.remove(), 4200);
  }

  function readableError(error) {
    const text = String(error ?? "Unknown error");
    if (/authentication|decrypt|crypto/i.test(text)) {
      return "Unable to unlock or decrypt the vault. Check your master password and Account Secret.";
    }
    return text;
  }

  async function call(command, args = {}) {
    try {
      return await invoke(command, args);
    } catch (error) {
      throw new Error(readableError(error));
    }
  }

  function setBusy(button, busy, busyText = "Working…") {
    if (!button) return;
    if (busy) {
      button.dataset.label = button.textContent;
      button.textContent = busyText;
      button.disabled = true;
    } else {
      button.textContent = button.dataset.label || button.textContent;
      button.disabled = false;
    }
  }

  function showAuthPanel(id) {
    $("create-panel").classList.toggle("hidden", id !== "create-panel");
    $("unlock-panel").classList.toggle("hidden", id !== "unlock-panel");
  }

  function showVault() {
    welcomeView.classList.add("hidden");
    vaultView.classList.remove("hidden");
  }

  function showWelcome() {
    vaultView.classList.add("hidden");
    welcomeView.classList.remove("hidden");
    showAuthPanel(null);
    clearUnlockInputs();
    state.items = [];
    state.selectedId = null;
    state.selectedItem = null;
  }

  function clearUnlockInputs() {
    $("unlock-password").value = "";
    $("unlock-secret").value = "";
    $("create-password").value = "";
    $("create-password-confirm").value = "";
  }

  async function initialize() {
    state.status = await call("app_status");
    if (state.status.unlocked) {
      showVault();
      await refreshItems();
    } else {
      showWelcome();
    }
  }

  async function refreshItems() {
    const query = state.search.trim() || null;
    const all = await call("list_items", { query });
    state.items = all;
    renderList();
    $("all-count").textContent = String(all.length);
    $("vault-location").textContent = state.status?.vaultPath || "";

    if (state.selectedId && !all.some((item) => item.id === state.selectedId)) {
      state.selectedId = null;
      state.selectedItem = null;
      renderDetail();
    } else if (state.selectedId) {
      await selectItem(state.selectedId, false);
    }
  }

  function filteredItems() {
    return state.items.filter((item) => {
      if (state.filter === "favorites") return item.favorite;
      if (state.filter === "login") return item.kind === "login";
      if (state.filter === "secure_note") return item.kind === "secure_note";
      return true;
    });
  }

  function renderList() {
    const list = $("item-list");
    list.replaceChildren();
    const items = filteredItems();
    $("empty-list").classList.toggle("hidden", items.length !== 0);

    const titles = {
      all: "All items",
      favorites: "Favorites",
      login: "Logins",
      secure_note: "Secure notes",
    };
    $("list-title").textContent = titles[state.filter];

    for (const item of items) {
      const card = document.createElement("div");
      card.className = "item-card" + (item.id === state.selectedId ? " selected" : "");
      card.setAttribute("role", "listitem");
      card.tabIndex = 0;

      const avatar = document.createElement("div");
      avatar.className = "item-avatar";
      avatar.textContent = initials(item.name);

      const text = document.createElement("div");
      const title = document.createElement("div");
      title.className = "item-card-title";
      title.textContent = item.name;
      const subtitle = document.createElement("div");
      subtitle.className = "item-card-subtitle";
      subtitle.textContent = item.kind === "login"
        ? (item.tags.length ? item.tags.join(" · ") : "Login")
        : (item.tags.length ? item.tags.join(" · ") : "Secure note");
      text.append(title, subtitle);

      const favorite = document.createElement("div");
      favorite.className = "item-favorite";
      favorite.textContent = item.favorite ? "★" : "";

      card.append(avatar, text, favorite);
      card.addEventListener("click", () => selectItem(item.id));
      card.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") selectItem(item.id);
      });
      list.appendChild(card);
    }
  }

  function initials(name) {
    const parts = String(name).trim().split(/\s+/).filter(Boolean);
    if (!parts.length) return "?";
    if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }

  async function selectItem(id, rerender = true) {
    state.selectedId = id;
    state.selectedItem = await call("get_item", { id });
    state.passwordVisible = false;
    if (rerender) renderList();
    renderDetail();
  }

  function renderDetail() {
    const item = state.selectedItem;
    $("detail-empty").classList.toggle("hidden", Boolean(item));
    $("detail-content").classList.toggle("hidden", !item);
    if (!item) return;

    $("detail-avatar").textContent = initials(item.name);
    $("detail-kind").textContent = item.kind === "login" ? "Login" : "Secure note";
    $("detail-name").textContent = item.name;
    $("favorite-action").textContent = item.favorite ? "★" : "☆";
    $("favorite-action").title = item.favorite ? "Remove from favorites" : "Add to favorites";

    const tags = $("detail-tags");
    tags.replaceChildren();
    for (const value of item.tags) {
      const tag = document.createElement("span");
      tag.className = "tag";
      tag.textContent = value;
      tags.appendChild(tag);
    }

    const isLogin = item.kind === "login";
    $("login-detail-fields").classList.toggle("hidden", !isLogin);
    $("detail-username").textContent = item.username || "—";
    $("detail-url").textContent = item.url || "—";
    $("detail-password").textContent = state.passwordVisible ? item.password : "••••••••••••••••";
    $("reveal-password").textContent = state.passwordVisible ? "Hide" : "Reveal";
    $("detail-notes").textContent = item.notes || "No notes";
    $("detail-created").textContent = "Created " + formatDate(item.createdAt);
    $("detail-updated").textContent = "Updated " + formatDate(item.updatedAt);
  }

  function formatDate(epoch) {
    if (!epoch) return "—";
    return new Date(epoch * 1000).toLocaleString();
  }

  function openItemModal(kind = "login", item = null) {
    state.editorKind = item?.kind || kind;
    $("item-id").value = item?.id || "";
    $("item-name").value = item?.name || "";
    $("item-username").value = item?.username || "";
    $("item-password").value = item?.password || "";
    $("item-url").value = item?.url || "";
    $("item-tags").value = item?.tags?.join(", ") || "";
    $("item-notes").value = item?.notes || "";
    $("item-favorite").checked = Boolean(item?.favorite);
    updateEditorKind();
    $("item-modal-title").textContent = item ? "Edit item" : (state.editorKind === "login" ? "New login" : "New secure note");
    $("item-modal").classList.remove("hidden");
    window.setTimeout(() => $("item-name").focus(), 30);
  }

  function updateEditorKind() {
    document.querySelectorAll(".segment").forEach((button) => {
      button.classList.toggle("active", button.dataset.kind === state.editorKind);
    });
    $("login-fields").classList.toggle("hidden", state.editorKind !== "login");
  }

  function closeModal(id) {
    $(id).classList.add("hidden");
    if (id === "item-modal") {
      $("item-password").value = "";
      $("item-notes").value = "";
    }
    if (id === "settings-modal") {
      $("settings-password").value = "";
      $("settings-password-confirm").value = "";
      $("settings-secret").value = "";
    }
  }

  async function saveEditor(event) {
    event.preventDefault();
    const button = $("save-item");
    setBusy(button, true, "Encrypting…");
    try {
      const tags = $("item-tags").value.split(",").map((tag) => tag.trim()).filter(Boolean);
      const draft = {
        id: $("item-id").value || null,
        name: $("item-name").value,
        kind: state.editorKind,
        favorite: $("item-favorite").checked,
        tags,
        username: $("item-username").value,
        password: $("item-password").value,
        url: $("item-url").value,
        notes: $("item-notes").value,
      };
      const id = await call("save_item", { draft });
      closeModal("item-modal");
      await refreshItems();
      await selectItem(id);
      toast("Item encrypted and saved.");
    } catch (error) {
      toast(error.message, "error");
    } finally {
      setBusy(button, false);
    }
  }

  async function deleteSelected() {
    const item = state.selectedItem;
    if (!item) return;
    if (!window.confirm("Delete “" + item.name + "” from this vault? This cannot be undone.")) return;
    try {
      await call("delete_item", { id: item.id });
      state.selectedId = null;
      state.selectedItem = null;
      renderDetail();
      await refreshItems();
      toast("Item deleted.");
    } catch (error) {
      toast(error.message, "error");
    }
  }

  async function toggleFavorite() {
    const item = state.selectedItem;
    if (!item) return;
    try {
      const draft = {
        id: item.id,
        name: item.name,
        kind: item.kind,
        favorite: !item.favorite,
        tags: item.tags,
        username: item.username,
        password: item.password,
        url: item.url,
        notes: item.notes,
      };
      await call("save_item", { draft });
      await refreshItems();
      await selectItem(item.id);
    } catch (error) {
      toast(error.message, "error");
    }
  }

  async function copyValue(value, label) {
    try {
      await navigator.clipboard.writeText(value || "");
      toast(label + " copied. Remember that other applications may be able to read your clipboard.");
    } catch {
      toast("Clipboard access was not available.", "error");
    }
  }

  async function createVault() {
    const path = $("create-path").value;
    const password = $("create-password").value;
    const confirm = $("create-password-confirm").value;
    if (!path) return toast("Choose a vault file location.", "error");
    if (!password) return toast("Enter a master password.", "error");
    if (password !== confirm) return toast("Master passwords do not match.", "error");

    const button = $("create-submit");
    setBusy(button, true, "Creating vault…");
    try {
      const response = await call("create_vault", { path, masterPassword: password });
      state.status = response.status;
      $("create-password").value = "";
      $("create-password-confirm").value = "";
      $("recovery-secret").textContent = response.accountSecretHex;
      $("recovery-saved").checked = false;
      $("recovery-continue").disabled = true;
      $("recovery-modal").classList.remove("hidden");
    } catch (error) {
      toast(error.message, "error");
    } finally {
      setBusy(button, false);
    }
  }

  async function unlockVault() {
    const path = $("unlock-path").value;
    const password = $("unlock-password").value;
    const secret = $("unlock-secret").value;
    if (!path || !password || !secret) return toast("Vault file, master password, and Account Secret are required.", "error");

    const button = $("unlock-submit");
    setBusy(button, true, "Unlocking…");
    try {
      state.status = await call("unlock_vault", {
        path,
        masterPassword: password,
        accountSecretHex: secret,
      });
      clearUnlockInputs();
      showVault();
      await refreshItems();
      toast("Vault unlocked.");
    } catch (error) {
      toast(error.message, "error");
    } finally {
      setBusy(button, false);
    }
  }

  async function completeRecovery() {
    $("recovery-secret").textContent = "";
    closeModal("recovery-modal");
    showVault();
    await refreshItems();
    toast("Vault created. Keep your Account Secret safe.");
  }

  async function lockVault() {
    try {
      state.status = await call("lock_vault");
      state.selectedItem = null;
      state.selectedId = null;
      showWelcome();
      toast("Vault locked.");
    } catch (error) {
      toast(error.message, "error");
    }
  }

  async function verifyVault() {
    try {
      toast("Verifying every encrypted record…");
      await call("verify_vault");
      toast("Vault integrity check passed.");
    } catch (error) {
      toast(error.message, "error");
    }
  }

  async function exportBackup() {
    try {
      const destination = await call("pick_backup_destination");
      if (!destination) return;
      await call("export_backup", { destination });
      toast("Encrypted backup created.");
    } catch (error) {
      toast(error.message, "error");
    }
  }

  async function changeMasterPassword() {
    const password = $("settings-password").value;
    const confirm = $("settings-password-confirm").value;
    const secret = $("settings-secret").value;
    if (!password || !secret) return toast("New password and Account Secret are required.", "error");
    if (password !== confirm) return toast("New master passwords do not match.", "error");

    const button = $("change-password");
    setBusy(button, true, "Updating…");
    try {
      await call("change_master_password", {
        newMasterPassword: password,
        accountSecretHex: secret,
      });
      closeModal("settings-modal");
      toast("Master password changed successfully.");
    } catch (error) {
      toast(error.message, "error");
    } finally {
      setBusy(button, false);
    }
  }

  function bindEvents() {
    $("show-create").addEventListener("click", () => showAuthPanel("create-panel"));
    $("show-unlock").addEventListener("click", () => showAuthPanel("unlock-panel"));
    document.querySelectorAll(".auth-close").forEach((button) => button.addEventListener("click", () => showAuthPanel(null)));

    $("create-browse").addEventListener("click", async () => {
      const path = await call("pick_new_vault");
      if (path) $("create-path").value = path;
    });
    $("unlock-browse").addEventListener("click", async () => {
      const path = await call("pick_existing_vault");
      if (path) $("unlock-path").value = path;
    });
    $("create-submit").addEventListener("click", createVault);
    $("unlock-submit").addEventListener("click", unlockVault);

    $("recovery-saved").addEventListener("change", (event) => {
      $("recovery-continue").disabled = !event.target.checked;
    });
    $("copy-recovery").addEventListener("click", () => copyValue($("recovery-secret").textContent, "Account Secret"));
    $("recovery-continue").addEventListener("click", completeRecovery);

    $("new-login").addEventListener("click", () => openItemModal("login"));
    $("new-note").addEventListener("click", () => openItemModal("secure_note"));
    $("quick-add").addEventListener("click", () => openItemModal("login"));
    $("empty-add").addEventListener("click", () => openItemModal("login"));
    $("item-form").addEventListener("submit", saveEditor);
    document.querySelectorAll(".segment").forEach((button) => button.addEventListener("click", () => {
      state.editorKind = button.dataset.kind;
      updateEditorKind();
    }));
    document.querySelectorAll(".modal-close").forEach((button) => button.addEventListener("click", () => closeModal(button.dataset.modal)));

    $("generate-password").addEventListener("click", async () => {
      try {
        $("item-password").value = await call("generate_password", { length: 24 });
        $("item-password").type = "text";
        window.setTimeout(() => { $("item-password").type = "password"; }, 5000);
        toast("Strong password generated.");
      } catch (error) {
        toast(error.message, "error");
      }
    });

    document.querySelectorAll(".nav-item[data-filter]").forEach((button) => button.addEventListener("click", () => {
      state.filter = button.dataset.filter;
      document.querySelectorAll(".nav-item[data-filter]").forEach((node) => node.classList.toggle("active", node === button));
      renderList();
    }));

    let searchTimer;
    $("search-input").addEventListener("input", (event) => {
      window.clearTimeout(searchTimer);
      state.search = event.target.value;
      searchTimer = window.setTimeout(() => refreshItems().catch((error) => toast(error.message, "error")), 180);
    });

    $("edit-action").addEventListener("click", () => state.selectedItem && openItemModal(state.selectedItem.kind, state.selectedItem));
    $("delete-action").addEventListener("click", deleteSelected);
    $("favorite-action").addEventListener("click", toggleFavorite);
    $("reveal-password").addEventListener("click", () => {
      state.passwordVisible = !state.passwordVisible;
      renderDetail();
    });

    document.querySelectorAll("[data-copy]").forEach((button) => button.addEventListener("click", () => {
      const item = state.selectedItem;
      if (!item) return;
      const key = button.dataset.copy;
      copyValue(item[key], key.charAt(0).toUpperCase() + key.slice(1));
    }));

    $("lock-action").addEventListener("click", lockVault);
    $("verify-action").addEventListener("click", verifyVault);
    $("backup-action").addEventListener("click", exportBackup);
    $("settings-action").addEventListener("click", () => $("settings-modal").classList.remove("hidden"));
    $("settings-verify").addEventListener("click", verifyVault);
    $("change-password").addEventListener("click", changeMasterPassword);

    document.addEventListener("keydown", (event) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k" && !vaultView.classList.contains("hidden")) {
        event.preventDefault();
        $("search-input").focus();
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "l" && !vaultView.classList.contains("hidden")) {
        event.preventDefault();
        lockVault();
      }
      if (event.key === "Escape") {
        document.querySelectorAll(".modal-backdrop:not(.hidden)").forEach((modal) => {
          if (modal.id !== "recovery-modal") closeModal(modal.id);
        });
      }
    });
  }

  bindEvents();
  initialize().catch((error) => {
    toast(error.message, "error");
    showWelcome();
  });
})();
