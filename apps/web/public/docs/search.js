(() => {
  const dialog = document.querySelector('[data-docs-search-dialog]');
  const trigger = document.querySelector('[data-docs-search-open]');
  if (!dialog || !trigger) return;

  const input = dialog.querySelector('[data-docs-search-input]');
  const list = dialog.querySelector('[data-docs-search-results]');
  const status = dialog.querySelector('[data-docs-search-status]');
  const closeButton = dialog.querySelector('[data-docs-search-close]');
  const projectButtons = [...dialog.querySelectorAll('[data-docs-search-project]')];
  const versionButtons = [...dialog.querySelectorAll('[data-docs-search-version]')];
  const docsRoot = dialog.dataset.docsRoot;
  const indexUrl = dialog.dataset.indexUrl;
  let documents;
  let loadPromise;
  let results = [];
  let project = '';
  let version = '';
  let previousFocus;

  const normalize = (value) => String(value).normalize('NFKC').toLocaleLowerCase();
  const snippet = (document, tokens) => {
    const text = document.text;
    const normalized = normalize(text);
    const positions = tokens.map((token) => normalized.indexOf(token)).filter((position) => position >= 0);
    const start = positions.length ? Math.max(0, Math.min(...positions) - 64) : 0;
    return `${start > 0 ? '…' : ''}${text.slice(start, start + 180).trim()}${start + 180 < text.length ? '…' : ''}`;
  };
  const search = (query) => {
    const tokens = normalize(query).split(/\s+/).filter(Boolean);
    if (!tokens.length) return [];
    return documents.map((document) => {
      const title = normalize(document.title);
      const description = normalize(document.description);
      const body = normalize(document.text);
      let score = 0;
      for (const token of tokens) {
        if (title === token) score += 100;
        else if (title.includes(token)) score += 50;
        else if (description.includes(token)) score += 20;
        else if (body.includes(token)) score += 6;
        else return null;
      }
      return { document, score, tokens };
    }).filter(Boolean).sort((left, right) => right.score - left.score || left.document.title.localeCompare(right.document.title, 'zh-CN'));
  };

  const render = () => {
    const query = input.value.trim();
    list.replaceChildren();
    results = [];
    if (!query) {
      status.textContent = documents ? '输入关键词，搜索标题、正文和代码。' : '正在加载文档索引…';
      return;
    }
    if (!documents) {
      status.textContent = loadPromise ? '正在加载文档索引…' : '文档索引尚未加载。';
      return;
    }
    const matches = search(query).filter(({ document }) => (!project || document.project === project) && (!version || document.version === version));
    results = matches.slice(0, 12);
    status.textContent = matches.length ? `找到 ${matches.length} 篇文档${matches.length > results.length ? `，显示前 ${results.length} 篇` : ''}。` : '没有匹配的文档，请更换关键词或筛选条件。';
    for (const [index, match] of results.entries()) {
      const item = document.createElement('li');
      const link = document.createElement('a');
      const title = document.createElement('span');
      const meta = document.createElement('span');
      const excerpt = document.createElement('span');
      link.href = docsRoot + match.document.route;
      link.id = `docs-search-result-${index}`;
      link.className = 'docs-search-result';
      title.className = 'docs-search-result-title';
      title.textContent = match.document.title;
      meta.className = 'docs-search-result-meta';
      meta.textContent = [match.document.projectTitle, match.document.versionLabel, match.document.section].filter(Boolean).join(' · ');
      excerpt.className = 'docs-search-result-excerpt';
      excerpt.textContent = match.document.description || snippet(match.document, match.tokens);
      link.append(title, meta, excerpt);
      item.append(link);
      list.append(item);
    }
  };

  const loadIndex = () => {
    if (documents || loadPromise) return loadPromise;
    loadPromise = fetch(indexUrl).then((response) => {
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      return response.json();
    }).then((data) => {
      if (data?.format !== 1 || !Array.isArray(data.documents)) throw new Error('invalid index');
      documents = data.documents;
      render();
      return documents;
    }).catch(() => {
      loadPromise = undefined;
      status.textContent = '文档索引加载失败，请关闭后重试。';
    });
    return loadPromise;
  };

  const updateFilterButtons = () => {
    for (const button of projectButtons) button.setAttribute('aria-pressed', String(button.dataset.docsSearchProject === project));
    for (const button of versionButtons) button.setAttribute('aria-pressed', String(button.dataset.docsSearchVersion === version));
  };
  const closeDialog = () => {
    if (!dialog.open) return;
    dialog.close();
  };
  const openDialog = () => {
    if (dialog.open) {
      input.focus();
      return;
    }
    previousFocus = document.activeElement;
    project = dialog.dataset.currentProject;
    version = dialog.dataset.currentVersion;
    updateFilterButtons();
    render();
    dialog.showModal();
    document.documentElement.classList.add('docs-search-open');
    input.focus();
    void loadIndex();
  };

  trigger.addEventListener('click', openDialog);
  closeButton.addEventListener('click', closeDialog);
  dialog.addEventListener('cancel', (event) => {
    event.preventDefault();
    closeDialog();
  });
  dialog.addEventListener('close', () => {
    document.documentElement.classList.remove('docs-search-open');
    previousFocus?.focus?.();
  });
  dialog.addEventListener('click', (event) => {
    if (event.target === dialog) closeDialog();
  });
  input.addEventListener('input', render);
  input.addEventListener('keydown', (event) => {
    if (event.key === 'ArrowDown' && results.length) {
      event.preventDefault();
      list.querySelector('a')?.focus();
    }
    if (event.key === 'Enter' && results.length) {
      event.preventDefault();
      window.location.href = list.querySelector('a').href;
    }
  });
  list.addEventListener('keydown', (event) => {
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    const links = [...list.querySelectorAll('a')];
    const current = links.indexOf(document.activeElement);
    if (current < 0) return;
    event.preventDefault();
    if (event.key === 'ArrowUp' && current === 0) {
      input.focus();
      return;
    }
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? links.length - 1 : Math.max(0, Math.min(links.length - 1, current + (event.key === 'ArrowDown' ? 1 : -1)));
    links[next]?.focus();
  });
  for (const button of projectButtons) button.addEventListener('click', () => {
    project = button.dataset.docsSearchProject;
    updateFilterButtons();
    render();
    input.focus();
  });
  for (const button of versionButtons) button.addEventListener('click', () => {
    version = button.dataset.docsSearchVersion;
    updateFilterButtons();
    render();
    input.focus();
  });
  document.addEventListener('keydown', (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === 'k') {
      event.preventDefault();
      openDialog();
    }
  });
  trigger.querySelector('kbd').textContent = /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘ K' : 'Ctrl K';
})();
