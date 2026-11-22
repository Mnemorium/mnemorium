function addToggle(sidebarSelector, className, label, symbol, bodyClass) {
  const sidebar = document.querySelector(sidebarSelector);
  const inner = sidebar?.querySelector(".md-sidebar__inner");
  if (!inner || inner.querySelector(`.${className}`)) {
    return;
  }

  const bar = document.createElement("div");
  bar.className = "sidebar-toggle-bar";

  const button = document.createElement("button");
  button.className = `sidebar-toggle ${className}`;
  button.type = "button";
  button.title = label;
  button.setAttribute("aria-label", label);
  button.textContent = symbol;
  button.addEventListener("click", () => {
    document.body.classList.toggle(bodyClass);
  });

  bar.appendChild(button);
  inner.insertBefore(bar, inner.firstChild);
}

function initializeSidebarToggles() {
  addToggle(".md-sidebar--primary", "sidebar-toggle-left", "Toggle navigation", "☰", "hide-left-sidebar");
  addToggle(
    ".md-sidebar--secondary",
    "sidebar-toggle-right",
    "Toggle table of contents",
    "☷",
    "hide-right-sidebar",
  );
}

/* Supports normal and instant navigation: `document$` emits after every page swap. */
if (typeof document$ !== "undefined") {
  document$.subscribe(initializeSidebarToggles);
} else {
  document.addEventListener("DOMContentLoaded", initializeSidebarToggles);
}
