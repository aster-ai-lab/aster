"""Generate an Aster Test suite for the shipped note editor, without editor mocks."""
import argparse
import json
from pathlib import Path
from urllib.parse import urlencode, urlsplit

MODES = {'visual': '可视化编辑', 'source': '源码编辑', 'split': '分屏编辑'}
BUTTONS = {
    'bold': '加粗', 'italic': '斜体', 'strike': '删除线', 'code': '行内代码',
    'bullet-list': '项目列表', 'ordered-list': '编号列表', 'task-list': '任务列表',
    'blockquote': '引用', 'code-block': '代码块', 'undo': '撤销', 'redo': '重做',
    'link-insert': '添加链接', 'link-edit': '编辑链接', 'link-remove': '移除链接',
    'image-insert': '插入图片', 'horizontal-rule': '分隔线', 'table': '表格',
}
EXPECTED = {
    'bold': '**Alpha**', 'italic': '*Alpha*', 'strike': '~~Alpha~~', 'code': '`Alpha`',
    'bullet-list': '- Alpha', 'ordered-list': '1. Alpha', 'task-list': '- [ ] Alpha',
    'blockquote': '> Alpha', 'code-block': '```\nAlpha\n```', 'paragraph': 'Alpha',
    'align-left': 'Alpha', 'undo': 'Alpha', 'redo': '**Alpha**', 'save': '**Alpha**',
    'link-insert': '[Alpha](https://example.test/docs)',
    'link-edit': '[Beta](https://example.test/docs)', 'link-remove': 'Alpha',
    'image-insert': '![Diagram](/image.svg)',
    **{f'heading-{n}': '#' * n + ' Alpha' for n in range(1, 7)},
    **{f'align-{align}': f'<p style="text-align: {align}">Alpha</p>' for align in ['center', 'right', 'justify']},
}
SHORTCUTS = {
    'bold': ['Meta+B', 'Control+B'], 'italic': ['Meta+I', 'Control+I'],
    'strike': ['Meta+Shift+S', 'Control+Shift+S', 'Meta+Shift+X', 'Control+Shift+X'],
    'code': ['Meta+E', 'Control+E'],
    'bullet-list': ['Meta+Shift+8', 'Control+Shift+8'],
    'ordered-list': ['Meta+Shift+7', 'Control+Shift+7'],
    'task-list': ['Meta+Shift+9', 'Control+Shift+9'],
    'blockquote': ['Meta+Shift+B', 'Control+Shift+B'],
    'code-block': ['Meta+Alt+C', 'Control+Alt+C'],
    'link-insert': ['Meta+K', 'Control+K'],
    'undo': ['Meta+Z', 'Control+Z'],
    'redo': ['Meta+Shift+Z', 'Control+Shift+Z', 'Meta+Y', 'Control+Y'],
    'save': ['Meta+S', 'Control+S'],
    **{name: [f'Meta+Alt+{n}', f'Control+Alt+{n}'] for n, name in enumerate(['paragraph', *[f'heading-{i}' for i in range(1, 7)]])},
}
CONTROLS = [name for name in EXPECTED if name != 'save'] + ['horizontal-rule', 'table']
assert len(CONTROLS) == 28
assert sum(map(len, SHORTCUTS.values())) == 44


def quote(value):
    return json.dumps(value, ensure_ascii=False)


def role(kind, name):
    return f'role({quote(kind)}, {quote(name)})'


def render_scenario(name, steps):
    return '\n'.join([
        f'    scenario {quote(name)} {{',
        '        surface = "web"', '        timeout_ms = 60000', *steps, '    }',
    ])


def dialog_scenarios(base):
    cases = []
    for mode in MODES:
        for image in [False, True]:
            title = '插入图片' if image else '添加链接'
            address = '图片地址' if image else '链接地址'
            address_selector = f'input[aria-label="{address}"]'
            address_target = f'css({quote(address_selector)})'
            invalid_selector = f'[aria-invalid="true"][aria-label="{address}"]'
            for dismissal in ['cancel', 'close', 'escape']:
                actions = [
                    ('navigate', f'url = {quote(base + "/?markdown=Alpha")}'),
                    ('wait', f'visible = {role("textbox", "Markdown 编辑区")}'),
                ]
                if mode != 'visual':
                    actions += [('click', f'target = {role("button", MODES[mode])}')]
                actions += [
                    ('click', f'target = {role("button", title)}'),
                    ('wait', f'visible = {role("dialog", title)}'),
                    ('expect', f'disabled = {role("button", "插入" if image else "添加")}'),
                    ('fill', f'target = {address_target} value = "javascript:alert(1)"'),
                    ('expect', f'target = css({quote(invalid_selector)}) visible_count = 1'),
                    ('click', f'target = {role("textbox", address)}'),
                    ('press', 'key = "Enter"'),
                    ('expect', f'target = {role("textbox", "序列化 Markdown")} value = "Alpha"'),
                    ('fill', f'target = {address_target} value = {quote("/image.svg" if image else "https://example.test/docs")}'),
                    ('expect', f'enabled = {role("button", "插入" if image else "添加")}'),
                    ('press', 'key = "Meta+S"'),
                    ('expect', f'target = {role("textbox", "保存内容")} value = ""'),
                ]
                if dismissal == 'escape':
                    actions.append(('press', 'key = "Escape"'))
                else:
                    actions.append(('click', f'target = {role("button", "取消" if dismissal == "cancel" else "关闭")}'))
                actions += [
                    ('wait', f'hidden = {role("dialog", title)}'),
                    ('expect', f'hidden = {role("dialog", title)}'),
                    ('expect', f'target = {role("textbox", "序列化 Markdown")} value = "Alpha"'),
                    ('expect', f'target = {role("textbox", "保存内容")} value = ""'),
                    ('expect', f'focused = {role("textbox", "Markdown 编辑区" if mode == "visual" else "Markdown 源码")}'),
                ]
                name = f'{mode}-{"image" if image else "link"}-{dismissal}'
                steps = [f'        {kind} "step-{n}" {{ {body} }}' for n, (kind, body) in enumerate(actions, 1)]
                cases.append((name, render_scenario(name, steps)))
    return cases


def scenario(base, mode, command, shortcut):
    name = f'{mode}-{command}-' + (shortcut.lower().replace('+', '-') if shortcut else 'button')
    steps = []

    def step(kind, body):
        steps.append(f'        {kind} "step-{len(steps) + 1}" {{ {body} }}')

    def click(kind, label):
        step('click', f'target = {role(kind, label)}')

    def press(key):
        step('press', f'key = {quote(key)}')

    def expect_value(label, value):
        step('expect', f'target = {role("textbox", label)} value = {quote(value)}')

    initial = '# Alpha' if command == 'paragraph' else (
        '<p style="text-align: center">Alpha</p>' if command == 'align-left' else (
            '[Alpha](https://example.test/old)' if command in ['link-edit', 'link-remove'] else (
                '' if command in ['image-insert', 'horizontal-rule', 'table'] else 'Alpha'
            )
        )
    )
    step('navigate', f'url = {quote(base + "/?" + urlencode({"markdown": initial}))}')
    step('wait', f'visible = {role("textbox", "Markdown 编辑区")}')
    if mode != 'visual':
        click('button', MODES[mode])
    field = 'Markdown 编辑区' if mode == 'visual' else 'Markdown 源码'
    step('wait', f'visible = {role("textbox", field)}')
    expect_value('序列化 Markdown', initial)
    click('textbox', field)
    if mode == 'visual':
        press('Meta+A')
    else:
        # CDP's Meta+A does not invoke native textarea Select All in this driver.
        for _ in initial:
            press('ArrowLeft')
        for _ in initial:
            press('Shift+ArrowRight')
    if command in ['undo', 'redo', 'save']:
        click('button', '加粗')
        expect_value('序列化 Markdown', '**Alpha**')
        if command == 'redo':
            click('button', '撤销')
            expect_value('序列化 Markdown', 'Alpha')
        if shortcut:
            click('textbox', field)
    if shortcut:
        press(shortcut)
    elif command == 'paragraph' or command.startswith('heading-'):
        click('combobox', '段落样式')
        click('option', '正文' if command == 'paragraph' else f'标题 {command[-1]}')
    elif command.startswith('align-'):
        click('radio', {'align-left': '左对齐', 'align-center': '居中', 'align-right': '右对齐', 'align-justify': '两端对齐'}[command])
    else:
        click('button', BUTTONS[command])
    if command in ['link-insert', 'link-edit', 'image-insert']:
        if command == 'image-insert':
            step('fill', 'target = label("替代文字（可选）") value = "Diagram"')
            step('fill', 'target = label("图片地址") value = "/image.svg"')
            step('wait', 'visible = css(".ds-dialog footer button[type=submit]:not(:disabled)")')
            click('button', '插入')
        else:
            step('fill', f'target = label("显示文字") value = {quote("Beta" if command == "link-edit" else "Alpha")}')
            step('fill', 'target = label("链接地址") value = "https://example.test/docs"')
            step('wait', 'visible = css(".ds-dialog footer button[type=submit]:not(:disabled)")')
            click('button', '保存' if command == 'link-edit' else '添加')
        title = {'image-insert': '插入图片', 'link-edit': '编辑链接', 'link-insert': '添加链接'}[command]
        step('wait', f'hidden = {role("dialog", title)}')
    if command in ['horizontal-rule', 'table']:
        if mode != 'visual':
            click('button', MODES['visual'])
        if command == 'table':
            for selector, count in [('table', 1), ('table tr', 3), ('table th', 3), ('table td', 6)]:
                step('expect', f'target = css({quote(selector)}) visible_count = {count}')
        else:
            step('expect', 'target = css("hr") visible_count = 1')
    else:
        expected = EXPECTED[command]
        if mode == 'visual' and command in ['paragraph', 'bullet-list', 'ordered-list', 'task-list', 'blockquote', 'code-block', *[f'heading-{n}' for n in range(1, 7)]]:
            expected += '\n\n'
        expect_value('序列化 Markdown', expected)
    if command == 'save':
        expect_value('保存内容', EXPECTED[command])
    return name, render_scenario(name, steps)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--base', default='http://127.0.0.1:58128')
    parser.add_argument('--only', help='Exact scenario identifier for an isolated reproduction')
    args = parser.parse_args()
    base = urlsplit(args.base)
    assert base.scheme == 'http' and base.hostname == '127.0.0.1' and base.port and base.path in ['', '/']
    cases = [scenario(args.base.rstrip('/'), mode, command, None) for mode in MODES for command in CONTROLS]
    cases += [scenario(args.base.rstrip('/'), mode, command, shortcut) for mode in MODES for command, shortcuts in SHORTCUTS.items() for shortcut in shortcuts]
    assert len(cases) == 216 and len({name for name, _ in cases}) == 216
    cases += dialog_scenarios(args.base.rstrip('/'))
    assert len(cases) == 234 and len({name for name, _ in cases}) == 234
    if args.only:
        cases = [case for case in cases if case[0] == args.only]
        assert len(cases) == 1, 'unknown scenario'
    args.output.write_text('suite "markdown-editor" {\n    version = 1\n' + '\n'.join(body for _, body in cases) + '\n}\n')
    print(json.dumps({'scenarios': len(cases), 'output': str(args.output)}))
