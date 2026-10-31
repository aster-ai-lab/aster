import { TreeView } from '@aster-ai-lab/ui/components/composites/tree-view'
import { Button } from '@aster-ai-lab/ui/components/primitives/button'
import { Input } from '@aster-ai-lab/ui/components/primitives/input'
import { Box, ChevronDown, ChevronRight, Circle } from 'lucide-react'
import { useMemo, useState } from 'react'

const adapter = {
  getId: (node) => node.id,
  getChildren: (node) => node.children,
  canHaveChildren: (node) => node.children.length > 0
}

export function SceneTree({ entities, selected, onSelect, t }) {
  const [query, setQuery] = useState('')
  const data = useMemo(() => {
    const visible = new Set()
    for (const [id, entity] of Object.entries(entities)) {
      if (!entity.name.toLocaleLowerCase().includes(query.toLocaleLowerCase())) continue
      for (let current = id; current && !visible.has(current); current = entities[current].parent) visible.add(current)
    }
    const nodes = new Map([...visible].sort().map((id) => [id, { id, ...entities[id], children: [] }]))
    const roots = []
    for (const node of nodes.values()) {
      if (node.parent) nodes.get(node.parent)?.children.push(node)
      else roots.push(node)
    }
    return { roots, expanded: new Set(nodes.keys()) }
  }, [entities, query])
  return (
    <aside className="twin-tree">
      <header>
        <h2>{t('scene_tree')}</h2>
        <span>{Object.keys(entities).length}</span>
      </header>
      <div className="twin-tree-search">
        <Input
          size="sm"
          aria-label={t('search_objects')}
          placeholder={t('search_objects')}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>
      <nav aria-label={t('scene_tree')} className="twin-tree-scroll">
        <TreeView
          key={query}
          data={data.roots}
          adapter={adapter}
          defaultExpandedIds={data.expanded}
          selectedId={selected}
          onSelectedChange={onSelect}
          emptyState={<p className="twin-empty">{t(Object.keys(entities).length ? 'no_matches' : 'empty_scene')}</p>}
          renderRow={({ node, depth, isSelected, isExpanded, selectNode, toggleExpanded }) => (
            <div className="twin-tree-row" style={{ paddingInlineStart: Math.min(depth, 12) * 16 }}>
              {node.children.length > 0 ? (
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t(isExpanded ? 'collapse_object' : 'expand_object', {
                    name: node.name
                  })}
                  aria-expanded={isExpanded}
                  onClick={toggleExpanded}>
                  {isExpanded ? <ChevronDown /> : <ChevronRight />}
                </Button>
              ) : (
                <span className="twin-tree-spacer" />
              )}
              <Button
                variant="ghost"
                className="twin-tree-select"
                aria-current={isSelected ? 'true' : undefined}
                onClick={selectNode}>
                {node.body?.collider.kind === 'sphere' ? <Circle /> : <Box />}
                <span>{node.name}</span>
              </Button>
            </div>
          )}
        />
      </nav>
    </aside>
  )
}
