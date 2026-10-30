import { readFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createProcessor } from '@mdx-js/mdx';
import remarkGfm from 'remark-gfm';

const contentRoot = fileURLToPath(new URL('../content/docs/', import.meta.url));
const destination = path.resolve(process.argv[2] || 'target/docs-stc');
await mkdir(destination, { recursive: true });
const catalog = JSON.parse(await readFile(path.join(contentRoot, 'catalog.json'), 'utf8'));
const lines = [];
const mapping = [];
const text = (node) => node.type === 'inlineCode' ? `\`${node.value}\`` : node.value ?? (node.children ?? []).map(text).join('');
for (const project of catalog.projects) {
  const version = project.versions.find((item) => item.id === '9.1.6');
  if (!version) continue;
  for (const page of version.pages.filter((item) => item.file.startsWith('internals/'))) {
    const filename = `${project.id}/${version.id}/${page.file}.mdx`;
    const tree = createProcessor({ remarkPlugins: [remarkGfm] }).parse(await readFile(path.join(contentRoot, filename), 'utf8'));
    const append = (value, sourceLine) => { mapping.push({ outputLine: lines.length + 1, file: filename, sourceLine }); lines.push(value, ''); };
    const visit = (node) => {
      if (node.type === 'mdxjsEsm') {
        for (const statement of node.data.estree.body) {
          const declaration = statement.declaration?.declarations?.find((item) => item.id.name === 'meta');
          for (const property of declaration?.init?.properties ?? []) if (['title','description','label'].includes(property.key.name)) append(property.value.value, node.position.start.line);
        }
      } else if (['paragraph','heading','tableCell'].includes(node.type)) append(text(node), node.position.start.line);
      else if (node.type !== 'code') for (const child of node.children ?? []) visit(child);
    };
    visit(tree);
  }
}
const atlas = JSON.parse(await readFile(path.join(contentRoot, 'atlas.json'), 'utf8'));
for (const entry of atlas.diagrams) {
  const filename = `${entry.product}/${entry.version}/_atlas/${entry.name}.json`;
  const definition = JSON.parse(await readFile(path.join(contentRoot, filename), 'utf8'));
  const values = [definition.title, definition.description, ...definition.steps.flatMap((step) => [step.title, step.body])].filter(Boolean);
  for (const value of values) { mapping.push({ outputLine: lines.length + 1, file: filename }); lines.push(value, ''); }
}
for (const [project, names] of [['code',['system-prompts','context','execution','long-running','recovery','completion']], ['acp',['runtime']]]) {
  for (const name of names) {
    const filename = `${project}/9.1.6/_walks/${name}.json`;
    const definition = JSON.parse(await readFile(path.join(contentRoot, filename), 'utf8'));
    for (const value of [definition.title, definition.intro, ...definition.steps.flatMap((step) => [step.title, step.body, step.takeaway])]) {
      mapping.push({ outputLine: lines.length + 1, file: filename }); lines.push(value, '');
    }
  }
}
await writeFile(path.join(destination, 'prose.md'), lines.join('\n') + '\n');
await writeFile(path.join(destination, 'source-map.json'), JSON.stringify(mapping, null, 2) + '\n');
process.stdout.write(`Extracted ${mapping.length} prose units; code, JSX attributes and module syntax remain excluded.\n`);
