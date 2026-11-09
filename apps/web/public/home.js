(() => {
  "use strict";

  const header = document.querySelector(".site-header");
  const progress = document.querySelector(".scroll-progress span");
  const backToTop = document.querySelector(".back-to-top");
  const menuButton = document.querySelector(".menu-toggle");
  const mobileNav = document.querySelector(".mobile-nav");
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  const downloadTriggers = Array.from(document.querySelectorAll("[data-download-trigger]"));

  function initHeroTyping() {
    const title = document.querySelector("[data-typing-title]");
    if (!title || reduceMotion.matches) return;
    const original = title.innerHTML;
    const textNodes = [];
    const walker = document.createTreeWalker(title, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      if (walker.currentNode.nodeValue.trim()) textNodes.push(walker.currentNode);
    }
    const characters = [];
    textNodes.forEach((node) => {
      const fragment = document.createDocumentFragment();
      Array.from(node.nodeValue).forEach((value) => {
        const character = document.createElement("span");
        character.className = "typing-char";
        character.setAttribute("aria-hidden", "true");
        character.textContent = value;
        fragment.appendChild(character);
        characters.push(character);
      });
      node.replaceWith(fragment);
    });
    title.classList.add("is-typing");
    const cursor = document.createElement("span");
    cursor.className = "typing-cursor";
    cursor.setAttribute("aria-hidden", "true");
    title.appendChild(cursor);
    let index = 0;
    let timer = 0;
    let currentCharacter = null;

    function positionCursor() {
      if (!currentCharacter) return;
      const titleRect = title.getBoundingClientRect();
      const characterRect = currentCharacter.getBoundingClientRect();
      const cursorRect = cursor.getBoundingClientRect();
      cursor.style.left = `${Math.max(0, characterRect.right - titleRect.left - 1)}px`;
      cursor.style.top = `${Math.max(0, characterRect.top - titleRect.top + (characterRect.height - cursorRect.height) / 2)}px`;
    }

    function isVisible(character) {
      for (let element = character; element && element !== title; element = element.parentElement) {
        if (window.getComputedStyle(element).display === "none") return false;
      }
      return true;
    }

    function revealNext() {
      const visibleCharacters = characters.filter(isVisible);
      if (index >= visibleCharacters.length) {
        cursor.classList.add("is-complete");
        timer = window.setTimeout(() => cursor.classList.remove("is-active"), 1200);
        return;
      }
      currentCharacter = visibleCharacters[index];
      currentCharacter.classList.add("is-visible");
      positionCursor();
      cursor.classList.add("is-active");
      const value = currentCharacter.textContent;
      const delay = value === "，" ? 170 : /[\u3400-\u9fff]/.test(value) ? 105 : 78;
      index += 1;
      timer = window.setTimeout(revealNext, delay);
    }

    function restoreOriginal() {
      window.clearTimeout(timer);
      window.removeEventListener("resize", positionCursor);
      title.classList.remove("is-typing");
      title.innerHTML = original;
    }

    window.addEventListener("resize", positionCursor);
    reduceMotion.addEventListener("change", (event) => {
      if (event.matches) restoreOriginal();
    });
    timer = window.setTimeout(revealNext, 220);
  }

  const pinnedDownloads = new WeakSet();
  const hoverCloseTimers = new WeakMap();
  const hoverCapable = window.matchMedia("(hover: hover) and (pointer: fine)");

  function clearHoverClose(trigger) {
    const timer = hoverCloseTimers.get(trigger);
    if (timer) window.clearTimeout(timer);
    hoverCloseTimers.delete(trigger);
  }

  function closeDownloadPopover(trigger) {
    clearHoverClose(trigger);
    pinnedDownloads.delete(trigger);
    trigger.setAttribute("aria-expanded", "false");
    trigger.closest(".download-dropdown").classList.remove("is-open");
    const popover = document.getElementById(trigger.getAttribute("aria-controls"));
    if (popover) popover.hidden = true;
  }

  function closeDownloadPopovers(except = null) {
    downloadTriggers.forEach((trigger) => {
      if (trigger !== except) closeDownloadPopover(trigger);
    });
  }

  function setDownloadPopover(trigger, open) {
    if (open) closeDownloadPopovers(trigger);
    clearHoverClose(trigger);
    trigger.setAttribute("aria-expanded", String(open));
    trigger.closest(".download-dropdown").classList.toggle("is-open", open);
    const popover = document.getElementById(trigger.getAttribute("aria-controls"));
    if (popover) popover.hidden = !open;
    if (open) closeMenu();
    else pinnedDownloads.delete(trigger);
  }

  function toggleDownloadPopover(trigger) {
    if (pinnedDownloads.has(trigger)) {
      pinnedDownloads.delete(trigger);
      setDownloadPopover(trigger, false);
    } else {
      pinnedDownloads.add(trigger);
      setDownloadPopover(trigger, true);
    }
  }

  downloadTriggers.forEach((trigger) => {
    const dropdown = trigger.closest(".download-dropdown");
    const popover = document.getElementById(trigger.getAttribute("aria-controls"));
    trigger.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      toggleDownloadPopover(trigger);
    });
    trigger.addEventListener("keydown", (event) => {
      if (event.key === " ") {
        event.preventDefault();
        toggleDownloadPopover(trigger);
      }
      if (event.key === "ArrowDown" && popover) {
        event.preventDefault();
        pinnedDownloads.add(trigger);
        setDownloadPopover(trigger, true);
        popover.querySelector("a")?.focus();
      }
    });
    if (popover) {
      popover.querySelectorAll("a").forEach((link) => {
        link.addEventListener("click", () => setDownloadPopover(trigger, false));
      });
    }
    if (hoverCapable.matches) {
      dropdown.addEventListener("mouseenter", () => {
        clearHoverClose(trigger);
        if (!pinnedDownloads.has(trigger)) setDownloadPopover(trigger, true);
      });
      dropdown.addEventListener("mouseleave", () => {
        if (pinnedDownloads.has(trigger)) return;
        clearHoverClose(trigger);
        const timer = window.setTimeout(() => setDownloadPopover(trigger, false), 180);
        hoverCloseTimers.set(trigger, timer);
      });
    }
  });

  document.addEventListener("click", (event) => {
    if (!event.target.closest(".download-dropdown")) closeDownloadPopovers();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    const openTrigger = downloadTriggers.find((trigger) => trigger.getAttribute("aria-expanded") === "true");
    if (openTrigger) {
      event.preventDefault();
      closeDownloadPopovers();
      openTrigger.focus();
    }
  });
  window.addEventListener("resize", () => closeDownloadPopovers());

  function closeMenu() {
    if (!menuButton || !mobileNav) return;
    menuButton.setAttribute("aria-expanded", "false");
    menuButton.setAttribute("aria-label", "打开导航");
    mobileNav.classList.remove("is-open");
    document.body.classList.remove("menu-open");
  }

  if (menuButton && mobileNav) {
    menuButton.addEventListener("click", () => {
      const open = menuButton.getAttribute("aria-expanded") !== "true";
      if (open) closeDownloadPopovers();
      menuButton.setAttribute("aria-expanded", String(open));
      menuButton.setAttribute("aria-label", open ? "关闭导航" : "打开导航");
      mobileNav.classList.toggle("is-open", open);
      document.body.classList.toggle("menu-open", open);
    });
    mobileNav.querySelectorAll("a").forEach((link) => link.addEventListener("click", closeMenu));
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && menuButton.getAttribute("aria-expanded") === "true") {
        closeMenu();
        menuButton.focus();
      }
    });
    window.addEventListener("resize", () => {
      if (window.innerWidth > 980) closeMenu();
    });
  }

  const tabs = Array.from(document.querySelectorAll('[role="tab"][aria-controls]'));
  const tabList = document.querySelector(".scenario-tabs");
  const tabIndicator = document.querySelector(".scenario-tab-indicator");
  const replaySteps = {
    research: ["拆解研究问题", "检索并核对资料", "整理结论与引用", "生成可核验报告"],
    document: ["读取项目资料", "编辑文档内容", "对照源文件检查", "保存并继续审阅"],
    data: ["读取数据与字段", "运行受控脚本", "核对产物快照", "交付分析结果"],
    development: ["理解需求与项目", "修改代码文件", "运行测试与审查", "提交验证结果"]
  };
  const replayControllers = new Map();

  function createScenarioReplay(panel, steps) {
    const root = document.createElement("div");
    root.className = "scenario-replay";
    const header = document.createElement("div");
    header.className = "replay-header";
    const label = document.createElement("span");
    label.className = "replay-label";
    label.textContent = "演示轨迹";
    const toggle = document.createElement("button");
    toggle.className = "replay-toggle";
    toggle.type = "button";
    const count = document.createElement("span");
    count.className = "replay-count";
    const track = document.createElement("div");
    track.className = "replay-track";
    const progress = document.createElement("span");
    progress.className = "replay-progress";
    track.appendChild(progress);
    const list = document.createElement("ol");
    list.className = "replay-steps";
    const event = document.createElement("p");
    event.className = "replay-event";
    header.append(label, toggle, count);
    root.append(header, track, list, event);

    let activeIndex = 0;
    let playing = false;
    let timer = 0;
    const buttons = steps.map((step, index) => {
      const item = document.createElement("li");
      const button = document.createElement("button");
      button.type = "button";
      button.className = "replay-step";
      button.textContent = String(index + 1).padStart(2, "0");
      button.setAttribute("aria-label", `查看步骤 ${index + 1}：${step}`);
      button.addEventListener("click", () => {
        setPlaying(false);
        setActive(index);
      });
      item.appendChild(button);
      list.appendChild(item);
      return button;
    });

    function render() {
      buttons.forEach((button, index) => {
        const current = index === activeIndex;
        button.classList.toggle("is-current", current);
        button.classList.toggle("is-complete", index < activeIndex);
        if (current) button.setAttribute("aria-current", "step");
        else button.removeAttribute("aria-current");
      });
      count.textContent = `${activeIndex + 1} / ${steps.length}`;
      event.textContent = steps[activeIndex];
      progress.style.transform = `scaleX(${activeIndex / (steps.length - 1)})`;
      toggle.textContent = playing ? "暂停" : reduceMotion.matches ? "下一步" : activeIndex === steps.length - 1 ? "重播" : "播放";
      toggle.setAttribute("aria-pressed", String(playing));
      toggle.setAttribute(
        "aria-label",
        playing ? "暂停演示轨迹" : reduceMotion.matches ? "查看下一步" : activeIndex === steps.length - 1 ? "重新播放演示轨迹" : "播放演示轨迹"
      );
    }

    function setActive(index) {
      activeIndex = Math.max(0, Math.min(steps.length - 1, index));
      render();
    }

    function schedule() {
      if (!playing) return;
      timer = window.setTimeout(() => {
        if (!playing) return;
        if (activeIndex < steps.length - 1) {
          setActive(activeIndex + 1);
          schedule();
        } else {
          setPlaying(false);
        }
      }, 1400);
    }

    function setPlaying(value) {
      if (reduceMotion.matches && value) return;
      window.clearTimeout(timer);
      playing = value;
      render();
      if (playing) schedule();
    }

    toggle.addEventListener("click", () => {
      if (reduceMotion.matches) {
        setActive((activeIndex + 1) % steps.length);
        return;
      }
      if (!playing && activeIndex === steps.length - 1) setActive(0);
      setPlaying(!playing);
    });

    panel.querySelector(".scenario-visual figure").appendChild(root);
    render();
    return { pause: () => setPlaying(false) };
  }

  document.querySelectorAll(".scenario-panel").forEach((panel) => {
    const steps = replaySteps[panel.id.replace("panel-", "")];
    if (steps) replayControllers.set(panel.id, createScenarioReplay(panel, steps));
  });

  function pauseScenarioReplays() {
    replayControllers.forEach((controller) => controller.pause());
  }

  document.addEventListener("visibilitychange", () => {
    if (document.hidden) pauseScenarioReplays();
  });
  reduceMotion.addEventListener("change", (event) => {
    if (event.matches) pauseScenarioReplays();
  });

  function syncTabIndicator(tab) {
    if (!tabList || !tabIndicator) return;
    const listRect = tabList.getBoundingClientRect();
    const tabRect = tab.getBoundingClientRect();
    tabIndicator.style.width = `${tabRect.width}px`;
    tabIndicator.style.transform = `translateX(${tabRect.left - listRect.left}px)`;
    tabIndicator.dataset.activeTab = tab.id;
  }

  function activateTab(tab, moveFocus = false) {
    pauseScenarioReplays();
    const previousIndex = tabs.findIndex((item) => item.getAttribute("aria-selected") === "true");
    const nextIndex = tabs.indexOf(tab);
    const direction = previousIndex >= 0 && nextIndex < previousIndex ? "-1" : "1";
    tabs.forEach((item) => {
      const selected = item === tab;
      item.setAttribute("aria-selected", String(selected));
      item.tabIndex = selected ? 0 : -1;
      const panel = document.getElementById(item.getAttribute("aria-controls"));
      if (panel) {
        panel.hidden = !selected;
        panel.classList.toggle("is-active", selected);
        if (selected) panel.style.setProperty("--panel-direction", direction);
      }
    });
    syncTabIndicator(tab);
    if (moveFocus) tab.focus();
  }

  tabs.forEach((tab, index) => {
    tab.addEventListener("click", () => activateTab(tab));
    tab.addEventListener("keydown", (event) => {
      let nextIndex = null;
      if (event.key === "ArrowRight") nextIndex = (index + 1) % tabs.length;
      if (event.key === "ArrowLeft") nextIndex = (index - 1 + tabs.length) % tabs.length;
      if (event.key === "Home") nextIndex = 0;
      if (event.key === "End") nextIndex = tabs.length - 1;
      if (nextIndex !== null) {
        event.preventDefault();
        activateTab(tabs[nextIndex], true);
      }
    });
  });

  const selectedTab = tabs.find((tab) => tab.getAttribute("aria-selected") === "true");
  if (selectedTab) activateTab(selectedTab);
  if (tabList) tabList.classList.add("is-enhanced");
  window.addEventListener("resize", () => {
    const selected = tabs.find((tab) => tab.getAttribute("aria-selected") === "true");
    if (selected) syncTabIndicator(selected);
  });

  const navLinks = Array.from(document.querySelectorAll("[data-nav]"));
  const navSections = navLinks
    .map((link) => ({ link, section: document.getElementById(link.dataset.nav) }))
    .filter((item) => item.section);
  let scrollFrame = 0;

  function updateScrollState() {
    scrollFrame = 0;
    const scrollTop = window.scrollY || document.documentElement.scrollTop;
    const available = document.documentElement.scrollHeight - window.innerHeight;
    const ratio = available > 0 ? Math.min(1, Math.max(0, scrollTop / available)) : 0;
    if (progress) progress.style.transform = `scaleX(${ratio})`;
    if (header) header.classList.toggle("is-scrolled", scrollTop > 8);
    if (backToTop) backToTop.classList.toggle("is-visible", scrollTop > 760);

    let activeId = "top";
    navSections.forEach(({ link, section }) => {
      if (link.dataset.nav !== "top" && scrollTop >= section.offsetTop - window.innerHeight * 0.38) {
        activeId = section.id;
      }
    });
    navSections.forEach(({ link }) => {
      const active = link.dataset.nav === activeId;
      link.classList.toggle("is-active", active);
      if (active) link.setAttribute("aria-current", "location");
      else link.removeAttribute("aria-current");
    });
  }

  function requestScrollUpdate() {
    if (!scrollFrame) scrollFrame = window.requestAnimationFrame(updateScrollState);
  }

  window.addEventListener("scroll", requestScrollUpdate, { passive: true });
  window.addEventListener("resize", requestScrollUpdate);
  updateScrollState();

  if (backToTop) {
    backToTop.addEventListener("click", () => {
      window.scrollTo({ top: 0, behavior: reduceMotion.matches ? "auto" : "smooth" });
    });
  }

  const userAgent = navigator.userAgent;
  let currentPlatform = "";
  if (/Windows/i.test(userAgent)) currentPlatform = "windows";
  else if (/(Macintosh|Mac OS X)/i.test(userAgent) && !/(iPhone|iPad)/i.test(userAgent)) currentPlatform = "macos";
  else if (/(Linux|X11)/i.test(userAgent) && !/Android/i.test(userAgent)) currentPlatform = "linux";
  if (currentPlatform) {
    const currentCard = document.querySelector(`[data-platform="${currentPlatform}"]`);
    if (currentCard) {
      currentCard.classList.add("is-current");
      const badge = document.createElement("span");
      badge.className = "current-badge";
      badge.textContent = "当前设备";
      currentCard.appendChild(badge);
    }
    document.querySelectorAll(`[data-download-platform="${currentPlatform}"]`).forEach((option) => {
      const badge = document.createElement("span");
      badge.className = "platform-current";
      badge.textContent = "当前设备";
      option.insertBefore(badge, option.querySelector(".option-arrow"));
    });
  }

  const copyButton = document.querySelector("[data-copy-command]");
  const command = document.getElementById("quickstart-command");
  let copyTimer = 0;

  async function copyText(text) {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return;
    }
    const field = document.createElement("textarea");
    field.value = text;
    field.setAttribute("readonly", "");
    field.className = "copy-helper";
    document.body.appendChild(field);
    field.select();
    const copied = document.execCommand("copy");
    field.remove();
    if (!copied) throw new Error("copy unavailable");
  }

  if (copyButton && command) {
    copyButton.addEventListener("click", async () => {
      const label = copyButton.querySelector("span");
      window.clearTimeout(copyTimer);
      try {
        await copyText(command.textContent.trim());
        label.textContent = "已复制";
        copyButton.setAttribute("aria-label", "Aster 启动命令已复制");
      } catch (_error) {
        label.textContent = "请手动复制";
        copyButton.setAttribute("aria-label", "自动复制不可用，请手动复制命令");
      }
      copyTimer = window.setTimeout(() => {
        label.textContent = "复制命令";
        copyButton.setAttribute("aria-label", "复制 Aster 启动命令");
      }, 2200);
    });
  }

  const year = document.getElementById("current-year");
  if (year) year.textContent = String(new Date().getFullYear());
  initHeroTyping();
})();
