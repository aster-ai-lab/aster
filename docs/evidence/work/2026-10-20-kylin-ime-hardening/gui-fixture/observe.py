import json, sys
from pathlib import Path
import gi, pyatspi

gi.require_version('Gdk', '3.0')
from gi.repository import Gdk
out = Path(sys.argv[1])
out.parent.mkdir(parents=True, exist_ok=True)
Gdk.init([])
root = Gdk.get_default_root_window()
image = Gdk.pixbuf_get_from_window(root, 0, 0, root.get_width(), root.get_height())
image.savev(str(out.with_suffix('.png')), 'png', [], [])
items = []
seen = 0

def walk(node, path=(), depth=0):
    global seen
    seen += 1
    if depth > 20 or seen > 1200:
        return
    try:
        state = node.getState()
        role = node.getRoleName()
        item = {'path': list(path), 'role': role, 'name': (node.name or '')[:240]}
        if state.contains(pyatspi.STATE_EDITABLE):
            item['editable'] = True
        if state.contains(pyatspi.STATE_FOCUSED):
            item['focused'] = True
        if state.contains(pyatspi.STATE_SHOWING) or depth < 3:
            try:
                rect = node.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
                item['bounds'] = [rect.x, rect.y, rect.width, rect.height]
            except (NotImplementedError, AttributeError):
                pass
            if node.getRole() != pyatspi.ROLE_PASSWORD_TEXT:
                try:
                    text = node.queryText().getText(0, -1)
                    if text:
                        item['text'] = text[:500]
                except (NotImplementedError, AttributeError):
                    pass
            items.append(item)
        for index, child in enumerate(node):
            walk(child, (*path, index), depth + 1)
    except Exception as error:
        items.append({'path': list(path), 'error_type': type(error).__name__})

walk(pyatspi.Registry.getDesktop(0))
out.with_suffix('.json').write_text(json.dumps({'nodes_seen': seen, 'items': items}, ensure_ascii=False, indent=2)+'\n')
print(json.dumps({'nodes_seen': seen, 'items_captured': len(items), 'screenshot': str(out.with_suffix('.png'))}))
