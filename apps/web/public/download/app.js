"use strict";

document.body.classList.add("enhanced");

const header = document.querySelector(".site-header");
const mobileNav = document.querySelector("#mobile-nav");
const menuToggle = document.querySelector(".menu-toggle");
const backToTop = document.querySelector(".back-to-top");
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const openers = new WeakMap();
const closeTimers = new WeakMap();

function closeMenu() {
  mobileNav.hidden = true;
  menuToggle.setAttribute("aria-expanded", "false");
  menuToggle.setAttribute("aria-label", "打开导航");
}
menuToggle.addEventListener("click", () => {
  const open = mobileNav.hidden;
  mobileNav.hidden = !open;
  menuToggle.setAttribute("aria-expanded", String(open));
  menuToggle.setAttribute("aria-label", open ? "关闭导航" : "打开导航");
});
mobileNav.addEventListener("click", (event) => {
  if (event.target.closest("a")) closeMenu();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !mobileNav.hidden) {
    closeMenu();
    menuToggle.focus();
  }
});
window.matchMedia("(min-width: 768px)").addEventListener("change", (event) => {
  if (event.matches) closeMenu();
});

const progressBar = document.querySelector(".scroll-progress > span");
const heroScreen = document.querySelector(".hero-screen");
const spyLinks = [...document.querySelectorAll(".desktop-nav a[href^='#']")];
const spySections = spyLinks
  .map((link) => document.querySelector(link.getAttribute("href")))
  .filter(Boolean);

function updateScrollSpy(y) {
  if (!spySections.length) return;
  const marker = y + 170;
  let current = "";
  for (const section of spySections) {
    if (section.offsetTop <= marker) current = section.id;
  }
  for (const link of spyLinks) {
    link.classList.toggle("is-active", link.getAttribute("href") === `#${current}`);
  }
}
function updateScrollControls() {
  const y = window.scrollY;
  header.classList.toggle("scrolled", y > 40);
  backToTop.hidden = y < 600;
  if (progressBar) {
    const total = document.documentElement.scrollHeight - window.innerHeight;
    const ratio = total > 0 ? Math.min(1, Math.max(0, y / total)) : 0;
    progressBar.style.transform = `scaleX(${ratio})`;
  }
  if (heroScreen) heroScreen.style.setProperty("--hero-shift", `${Math.min(y * 0.06, 54)}px`);
  updateScrollSpy(y);
}
window.addEventListener("scroll", updateScrollControls, { passive: true });
updateScrollControls();
backToTop.addEventListener("click", () => {
  window.scrollTo({ top: 0, behavior: reducedMotion.matches ? "instant" : "smooth" });
  document.querySelector(".brand").focus({ preventScroll: true });
});

function openDialog(dialog, opener) {
  window.clearTimeout(closeTimers.get(dialog));
  dialog.classList.remove("closing");
  openers.set(dialog, opener);
  closeMenu();
  dialog.showModal();
}
function closeDialog(dialog) {
  if (!dialog.open || dialog.classList.contains("closing")) return;
  if (reducedMotion.matches) {
    dialog.close();
    return;
  }
  dialog.classList.add("closing");
  closeTimers.set(
    dialog,
    window.setTimeout(() => {
      dialog.close();
      dialog.classList.remove("closing");
    }, 200),
  );
}
document.querySelectorAll("dialog").forEach((dialog) => {
  dialog.querySelector("[data-close-dialog]").addEventListener("click", () => closeDialog(dialog));
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    closeDialog(dialog);
  });
  dialog.addEventListener("click", (event) => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (
      event.clientX < bounds.left ||
      event.clientX > bounds.right ||
      event.clientY < bounds.top ||
      event.clientY > bounds.bottom
    )
      closeDialog(dialog);
  });
  dialog.addEventListener("close", () => openers.get(dialog)?.focus({ preventScroll: true }));
});
document.querySelectorAll("[data-download]").forEach((opener) =>
  opener.addEventListener("click", (event) => {
    event.preventDefault();
    openDialog(document.querySelector("#download-dialog"), opener);
  }),
);

const screenshotLabels = {
  workspace: "Aster 工作区",
  office: "Aster Office",
  review: "Aster 自动审查",
  lineage: "Aster 产物追溯",
};
document.querySelectorAll("[data-screenshot], [data-enlarge]").forEach((opener) =>
  opener.addEventListener("click", (event) => {
    event.preventDefault();
    const key = opener.dataset.screenshot || opener.dataset.enlarge;
    if (!Object.hasOwn(screenshotLabels, key)) throw new Error("Unknown Aster screenshot");
    const image = document.querySelector("#screenshot-full");
    image.src = `./screenshots/${key}.png`;
    image.alt = `${screenshotLabels[key]}真实界面，演示数据`;
    document.querySelector("#screenshot-original").href = image.src;
    document.querySelector("#screenshot-title").textContent = `${screenshotLabels[key]}真实界面`;
    openDialog(document.querySelector("#screenshot-dialog"), opener);
  }),
);

document.querySelectorAll('[role="tablist"]').forEach((tablist) => {
  const tabs = [...tablist.querySelectorAll('[role="tab"]')];
  function selectTab(tab) {
    tabs.forEach((item) => {
      const selected = item === tab;
      item.setAttribute("aria-selected", String(selected));
      item.tabIndex = selected ? 0 : -1;
      document.getElementById(item.getAttribute("aria-controls")).hidden = !selected;
    });
  }
  selectTab(tabs.find((tab) => tab.getAttribute("aria-selected") === "true") || tabs[0]);
  tabs.forEach((tab, index) => {
    tab.addEventListener("click", () => selectTab(tab));
    tab.addEventListener("keydown", (event) => {
      let target;
      if (event.key === "ArrowLeft") target = tabs[(index + tabs.length - 1) % tabs.length];
      if (event.key === "ArrowRight") target = tabs[(index + 1) % tabs.length];
      if (event.key === "Home") target = tabs[0];
      if (event.key === "End") target = tabs[tabs.length - 1];
      if (target) {
        event.preventDefault();
        selectTab(target);
        target.focus();
      }
    });
  });
});

// Copy the CLI install command so the terminal flow stays one action.
document.querySelectorAll("[data-copy]").forEach((button) => {
  button.addEventListener("click", async () => {
    const source = document.querySelector(button.getAttribute("data-copy"));
    const label = button.querySelector("[data-copy-label]");
    if (!source) return;
    const text = source.textContent.replace(/\s+/g, " ").trim();
    let copied = false;
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
        copied = true;
      }
    } catch {
      copied = false;
    }
    if (!copied) {
      try {
        const range = document.createRange();
        range.selectNodeContents(source);
        const selection = window.getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
        copied = document.execCommand("copy");
        selection.removeAllRanges();
      } catch {
        copied = false;
      }
    }
    if (!label) return;
    const previous = label.textContent;
    label.textContent = copied ? "已复制" : "复制失败";
    button.classList.toggle("is-copied", copied);
    window.setTimeout(() => {
      label.textContent = previous;
      button.classList.remove("is-copied");
    }, 1800);
  });
});

/* ---------- Ambient motion ---------- */

// The wordmark lands letter by letter so the first screen reads as a reveal.
const heroTitle = document.querySelector("#hero-title");
if (heroTitle && !reducedMotion.matches) {
  const word = heroTitle.textContent.trim();
  heroTitle.setAttribute("aria-label", word);
  heroTitle.textContent = "";
  for (const [index, letter] of [...word].entries()) {
    const span = document.createElement("span");
    span.className = "hero-char";
    span.setAttribute("aria-hidden", "true");
    span.style.setProperty("--i", index);
    span.textContent = letter;
    heroTitle.appendChild(span);
  }
}

// A soft spotlight follows the pointer across the reading area.
const cursorGlow = document.querySelector(".cursor-glow");
const cursorCore = document.querySelector(".cursor-core");
if (cursorGlow && !reducedMotion.matches) {
  let glowX = window.innerWidth / 2;
  let glowY = window.innerHeight / 2;
  let targetGlowX = glowX;
  let targetGlowY = glowY;
  let glowFrame = 0;
  function stepGlow() {
    glowFrame = 0;
    glowX += (targetGlowX - glowX) * 0.12;
    glowY += (targetGlowY - glowY) * 0.12;
    cursorGlow.style.transform = `translate3d(${glowX}px, ${glowY}px, 0)`;
    if (cursorCore) cursorCore.style.transform = `translate3d(${targetGlowX}px, ${targetGlowY}px, 0)`;
    if (Math.abs(targetGlowX - glowX) > 0.6 || Math.abs(targetGlowY - glowY) > 0.6) {
      glowFrame = window.requestAnimationFrame(stepGlow);
    }
  }
  document.addEventListener(
    "pointermove",
    (event) => {
      if (event.pointerType === "touch") return;
      targetGlowX = event.clientX;
      targetGlowY = event.clientY;
      document.body.classList.add("has-pointer");
      if (!glowFrame) glowFrame = window.requestAnimationFrame(stepGlow);
    },
    { passive: true },
  );
}

// The CLI card replays a short session so the terminal flow reads at a glance.
const demoText = document.querySelector("#cli-demo-text");
if (demoText && !reducedMotion.matches) {
  const script = demoText.textContent;
  const caret = document.createElement("span");
  caret.className = "cli-caret";
  let origin = performance.now();
  function paint() {
    const elapsed = performance.now() - origin;
    const position = Math.min(script.length, Math.floor(elapsed / 22));
    demoText.textContent = script.slice(0, position);
    demoText.appendChild(caret);
    if (position >= script.length) {
      window.setTimeout(() => {
        origin = performance.now();
        paint();
      }, 3400);
      return;
    }
    window.setTimeout(paint, 40);
  }
  demoText.textContent = "";
  window.setTimeout(() => {
    origin = performance.now();
    paint();
  }, 600);
}

// Cards lean towards the pointer and carry a highlight with them.
for (const card of document.querySelectorAll(".scene-card, .practice-step, .platform-card")) {
  card.addEventListener("pointermove", (event) => {
    if (event.pointerType === "touch") return;
    const bounds = card.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const px = (event.clientX - bounds.left) / bounds.width;
    const py = (event.clientY - bounds.top) / bounds.height;
    card.style.setProperty("--mx", `${px * 100}%`);
    card.style.setProperty("--my", `${py * 100}%`);
    card.style.setProperty("--rx", `${(0.5 - py) * 4.5}deg`);
    card.style.setProperty("--ry", `${(px - 0.5) * 5.5}deg`);
  });
  card.addEventListener("pointerleave", () => {
    card.style.setProperty("--rx", "0deg");
    card.style.setProperty("--ry", "0deg");
  });
}

// A ripple confirms every press on a control.
for (const button of document.querySelectorAll(".pill-button, .quiet-button, .copy-button")) {
  button.addEventListener("pointerdown", (event) => {
    if (event.pointerType === "touch") return;
    const bounds = button.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const size = Math.max(bounds.width, bounds.height) * 2;
    const ripple = document.createElement("span");
    ripple.className = "ripple";
    ripple.setAttribute("aria-hidden", "true");
    ripple.style.width = `${size}px`;
    ripple.style.height = `${size}px`;
    ripple.style.left = `${event.clientX - bounds.left - size / 2}px`;
    ripple.style.top = `${event.clientY - bounds.top - size / 2}px`;
    button.appendChild(ripple);
    window.setTimeout(() => ripple.remove(), 640);
  });
}

// In-page links glide to their section instead of jumping.
for (const link of document.querySelectorAll('a[href^="#"]')) {
  if (link.classList.contains("skip-link")) continue;
  link.addEventListener("click", (event) => {
    const hash = link.getAttribute("href");
    if (!hash || hash === "#") return;
    const target = document.querySelector(hash);
    if (!target || typeof target.scrollIntoView !== "function") return;
    event.preventDefault();
    target.scrollIntoView({
      behavior: reducedMotion.matches ? "auto" : "smooth",
      block: "start",
    });
    try {
      window.history.replaceState(null, "", hash);
    } catch {
      // Keep the current address when the history API refuses the update.
    }
  });
}

// Cards ease in once they reach the viewport; the fallback keeps them visible.
const revealTargets = [...document.querySelectorAll("[data-reveal]")];
if (revealTargets.length) {
  const revealAll = () => revealTargets.forEach((node) => node.classList.add("is-visible"));
  if (!("IntersectionObserver" in window) || reducedMotion.matches) {
    revealAll();
  } else {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          entry.target.classList.add("is-visible");
          observer.unobserve(entry.target);
        }
      },
      { rootMargin: "0px 0px -6% 0px", threshold: 0.06 },
    );
    revealTargets.forEach((node) => observer.observe(node));
    window.setTimeout(revealAll, 2600);
  }
}
