function initializeSidebarToggles() {
  if (!document.querySelector(".sidebar-toggle-left")) {
    const leftButton = document.createElement("button");
    leftButton.className = "sidebar-toggle sidebar-toggle-left";
    leftButton.type = "button";
    leftButton.title = "Toggle navigation";
    leftButton.setAttribute("aria-label", "Toggle navigation");
    leftButton.textContent = "☰";

    leftButton.addEventListener("click", () => {
      document.body.classList.toggle("hide-left-sidebar");
    });

    document.body.appendChild(leftButton);
  }

  if (!document.querySelector(".sidebar-toggle-right")) {
    const rightButton = document.createElement("button");
    rightButton.className = "sidebar-toggle sidebar-toggle-right";
    rightButton.type = "button";
    rightButton.title = "Toggle table of contents";
    rightButton.setAttribute("aria-label", "Toggle table of contents");
    rightButton.textContent = "☷";

    rightButton.addEventListener("click", () => {
      document.body.classList.toggle("hide-right-sidebar");
    });

    document.body.appendChild(rightButton);
  }
}

/* Supports normal and instant navigation: `document$` emits after every page swap. */
if (typeof document$ !== "undefined") {
  document$.subscribe(initializeSidebarToggles);
} else {
  document.addEventListener("DOMContentLoaded", initializeSidebarToggles);
}
