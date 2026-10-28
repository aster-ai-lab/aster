import catalog from '../content/docs/catalog.json' with { type: 'json' };

if (Object.keys(catalog).length !== 1 || !Array.isArray(catalog.projects)) throw new Error('Documentation catalog must contain only a projects array');
export const projects = catalog.projects;

export function validateProjects(entries) {
  const ids = new Set();
  const routes = new Set(['']);
  const addRoute = (route) => {
    if (routes.has(route)) throw new Error(`Duplicate documentation route: ${route}`);
    routes.add(route);
  };
  for (const project of entries) {
    if (Object.keys(project).some((key) => !['id', 'title', 'label', 'description', 'defaultVersion', 'versions'].includes(key))) throw new Error('Unknown documentation project field');
    if (typeof project.id !== 'string' || !/^[a-z][a-z0-9-]*$/.test(project.id) || ids.has(project.id)) throw new Error(`Invalid or duplicate project ID: ${project.id}`);
    ids.add(project.id);
    for (const key of ['title', 'label', 'description']) {
      if (typeof project[key] !== 'string' || !project[key].trim()) throw new Error(`Missing project ${key}: ${project.id}`);
    }
    if (!Array.isArray(project.versions) || !project.versions.length) throw new Error(`Project needs versions: ${project.id}`);
    const versions = new Set();
    for (const version of project.versions) {
      if (Object.keys(version).some((key) => !['id', 'label', 'sourceRef', 'pages', 'components'].includes(key))) throw new Error('Unknown documentation version field');
      if (version.components !== undefined && (typeof version.components !== 'string' || !/^[a-z_][a-z0-9_-]*$/.test(version.components))) throw new Error(`Invalid version components: ${version.id}`);
      if (typeof version.id !== 'string' || version.id === 'index.html' || !/^[a-z0-9]+(?:[.-][a-z0-9]+)*$/.test(version.id) || versions.has(version.id)) throw new Error(`Invalid or duplicate version: ${project.id}/${version.id}`);
      versions.add(version.id);
      if (typeof version.label !== 'string' || !version.label.trim()) throw new Error(`Missing version label: ${project.id}/${version.id}`);
      if (typeof version.sourceRef !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(version.sourceRef) || version.sourceRef.split('/').some((part) => !part || part === '.' || part === '..')) throw new Error(`Invalid version source ref: ${version.id}`);
      if (!Array.isArray(version.pages) || !version.pages.length || version.pages[0].slug !== '') throw new Error(`Version needs a landing page: ${project.id}/${version.id}`);
      const slugs = new Set();
      const files = new Set();
      for (const page of version.pages) {
        if (Object.keys(page).some((key) => !['slug', 'file', 'label', 'title', 'description', 'section'].includes(key))) throw new Error('Unknown documentation page field');
        if (page.section !== undefined && (typeof page.section !== 'string' || !page.section.trim())) throw new Error(`Invalid page section: ${page.file}`);
        if (typeof page.slug !== 'string' || typeof page.file !== 'string' || !/^(?:[a-z0-9-]+\/)*$/.test(page.slug) || !/^[a-z0-9_-]+(?:\/[a-z0-9_-]+)*$/.test(page.file) || slugs.has(page.slug)) throw new Error(`Invalid or duplicate document route: ${project.id}/${version.id}/${page.slug}`);
        slugs.add(page.slug);
        if (files.has(page.file)) throw new Error(`Duplicate document file: ${project.id}/${version.id}/${page.file}`);
        files.add(page.file);
        for (const key of ['label', 'title', 'description']) {
          if (typeof page[key] !== 'string' || !page[key].trim()) throw new Error(`Missing page ${key}: ${page.file}`);
        }
        addRoute(`${project.id}/${version.id}/${page.slug}`);
      }
    }
    if (!versions.has(project.defaultVersion)) throw new Error(`Unknown default version: ${project.id}/${project.defaultVersion}`);
    for (const page of project.versions.find((version) => version.id === project.defaultVersion).pages) addRoute(`${project.id}/${page.slug}`);
  }
}

export function documentPages(entries) {
  validateProjects(entries);
  const result = [{ route: '', canonicalRoute: '', file: 'index', sourceFile: 'index' }];
  for (const project of entries) {
    for (const version of project.versions) {
      for (const page of version.pages) {
        const route = `${project.id}/${version.id}/${page.slug}`;
        result.push({ ...page, project, version, route, canonicalRoute: route, sourceFile: `${project.id}/${version.id}/${page.file}` });
      }
    }
    const version = project.versions.find((entry) => entry.id === project.defaultVersion);
    for (const page of version.pages) result.push({ ...page, project, version, route: `${project.id}/${page.slug}`, canonicalRoute: `${project.id}/${version.id}/${page.slug}`, sourceFile: `${project.id}/${version.id}/${page.file}`, alias: true });
  }
  return result;
}

validateProjects(projects);
