// Test-only passive observation. No event dispatch, cancellation, document edits or editor writes.
(() => {
  let count = 0
  const limit = 1200
  const channel = window.webkit?.messageHandlers?.asterImeReadonlyTrace20261020
  if (!channel) return
  const post = (record) => {
    if (count >= limit) return
    channel.postMessage(JSON.stringify({ sequence: ++count, ...record }))
  }
  const inspect = (event, phase) => {
    const target = event.target
    const host = target?.closest?.('[contenteditable],textarea,input')
    let composing = null
    try {
      const editor = host?.editor
      if (editor) composing = editor.view.composing
    } catch {
      composing = 'unavailable'
    }
    post({
      type: event.type,
      phase,
      timeStamp: event.timeStamp,
      key: event.key ?? null,
      code: event.code ?? null,
      keyCode: event.keyCode ?? null,
      isComposing: event.isComposing ?? null,
      viewComposing: composing,
      defaultPrevented: event.defaultPrevented,
      repeat: event.repeat ?? null,
      shiftKey: event.shiftKey ?? null,
      ctrlKey: event.ctrlKey ?? null,
      altKey: event.altKey ?? null,
      metaKey: event.metaKey ?? null,
      inputType: event.inputType ?? null,
      targetTag: target?.tagName ?? null,
      targetRole: target?.getAttribute?.('role') ?? null,
      targetClass: (target?.getAttribute?.('class') ?? '').slice(0, 120),
      hostTag: host?.tagName ?? null,
      activeTag: document.activeElement?.tagName ?? null
    })
  }
  for (const type of ['keydown', 'keypress', 'keyup', 'compositionstart', 'compositionupdate',
    'compositionend', 'beforeinput', 'input', 'focusin', 'focusout']) {
    window.addEventListener(type, (event) => inspect(event, 'window-capture'), { capture: true, passive: true })
    window.addEventListener(type, (event) => inspect(event, 'window-bubble'), { passive: true })
  }
  post({ type: 'observer-ready', limit, pathname: location.pathname })
})()
