(() => {
  const canvas = document.getElementById('aster-canvas');
  const hero = canvas?.closest('.hero');
  if (!canvas || !hero) return;
  let context;
  try { context = canvas.getContext('2d'); } catch { return; }
  if (!context || typeof context.ellipse !== 'function') return;

  const motion = window.matchMedia?.('(prefers-reduced-motion: reduce)') ?? { matches: true };
  const stars = [
    [.08, .18, 2, 1], [.17, .38, 3, 0], [.24, .11, 1.5, 2], [.31, .48, 2, 1],
    [.42, .07, 1.5, 0], [.58, .47, 1.5, 2], [.69, .12, 2.5, 0], [.77, .36, 2, 1],
    [.88, .20, 3, 2], [.94, .45, 1.5, 0], [.12, .58, 1.5, 1], [.86, .59, 2, 2],
    [.05, .72, 1.4, 1], [.15, .8, 2.2, 0], [.28, .66, 1.2, 2], [.36, .84, 1.6, 1],
    [.49, .7, 2.4, 0], [.63, .78, 1.3, 2], [.74, .64, 1.8, 1], [.91, .76, 2.6, 0],
    [.2, .28, 1.2, 0], [.52, .16, 1.4, 1], [.81, .08, 1.3, 2], [.96, .3, 1.2, 0]
  ];
  let width = 0, height = 0, ratio = 0, frame = 0, lastPaint = null, phase = 0;
  let inView = false, pageActive = true, pointerX = 0, pointerY = 0, targetX = 0, targetY = 0;
  let glowX = .5, glowY = .62, glowTargetX = .5, glowTargetY = .62, shield = null;
  const hasIntersection = typeof IntersectionObserver === 'function';

  function strokeRibbon(time, y, amplitude, frequency, color, line, speed, shift) {
    context.beginPath();
    const steps = 32;
    for (let i = 0; i <= steps; i++) {
      const t = i / steps;
      const wave = Math.sin(t * frequency + time * speed + shift) * amplitude
        + Math.sin(t * frequency * 1.8 - time * speed * .6 + shift * 1.7) * amplitude * .3;
      const x = width * (-.02 + t * 1.04);
      const py = y + wave;
      if (i) context.lineTo(x, py);
      else context.moveTo(x, py);
    }
    context.strokeStyle = color;
    context.lineWidth = line;
    context.lineCap = 'round';
    context.stroke();
  }

  function strokeColumn(time, x, color, line, speed, shift) {
    context.beginPath();
    const steps = 28;
    for (let i = 0; i <= steps; i++) {
      const t = i / steps;
      const px = x + Math.sin(t * 6.2 + time * speed + shift) * 26
        + Math.sin(t * 2.4 - time * speed * .7) * 14;
      const py = height * t;
      if (i) context.lineTo(px, py);
      else context.moveTo(px, py);
    }
    context.strokeStyle = color;
    context.lineWidth = line;
    context.lineCap = 'round';
    context.stroke();
  }

  function measureShield() {
    shield = null;
    if (typeof hero.querySelector !== 'function') return;
    const title = hero.querySelector('#hero-title');
    const text = hero.querySelector('.hero-copy p');
    const button = hero.querySelector('.hero-download');
    if (!title || !text || !button) return;
    const canvasBox = canvas.getBoundingClientRect();
    const titleBox = title.getBoundingClientRect();
    const textBox = text.getBoundingClientRect();
    const buttonBox = button.getBoundingClientRect();
    if (!titleBox.width || !textBox.width) return;
    context.save();
    const style = window.getComputedStyle(title);
    context.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    const titleWidth = context.measureText(title.textContent.trim()).width;
    context.restore();
    const titleCenter = titleBox.left + titleBox.width / 2;
    const left = Math.min(titleCenter - titleWidth / 2, textBox.left, buttonBox.left) - 42;
    const right = Math.max(titleCenter + titleWidth / 2, textBox.right, buttonBox.right) + 42;
    const top = Math.min(titleBox.top, textBox.top, buttonBox.top) - 22;
    const bottom = Math.max(titleBox.bottom, textBox.bottom, buttonBox.bottom) + 18;
    shield = {
      x: left - canvasBox.left,
      y: top - canvasBox.top,
      w: right - left,
      h: bottom - top
    };
  }

  function shieldCopy() {
    if (!shield) measureShield();
    if (!shield) return;
    const { x, y, w, h } = shield;
    context.save();
    context.globalCompositeOperation = 'destination-out';
    for (const [grow, alpha] of [[28, .4], [14, .75], [0, 1]]) {
      context.fillStyle = `rgba(0,0,0,${alpha})`;
      context.fillRect(x - grow, y - grow, w + grow * 2, h + grow * 2);
    }
    context.restore();
  }

  function drawConstellation(points) {
    const limit = Math.min(width, height) * 0.17;
    let drawn = 0;
    context.save();
    context.lineWidth = 1;
    context.lineCap = 'round';
    for (let i = 0; i < points.length && drawn < 44; i++) {
      for (let j = i + 1; j < points.length && drawn < 44; j++) {
        const dx = points[i].x - points[j].x;
        const dy = points[i].y - points[j].y;
        const distance = Math.hypot(dx, dy);
        if (distance > limit) continue;
        context.strokeStyle = `rgba(196,226,255,${(1 - distance / limit) * 0.17})`;
        context.beginPath();
        context.moveTo(points[i].x, points[i].y);
        context.lineTo(points[j].x, points[j].y);
        context.stroke();
        drawn++;
      }
    }
    context.restore();
  }

  function drawShootingStar(time) {
    const local = (time + 3.2) % 7.5;
    if (local > 0.85) return;
    const progress = local / 0.85;
    const x = width * (0.82 - progress * 0.56);
    const y = height * (0.06 + progress * 0.44);
    const length = 160 + progress * 110;
    const angle = Math.atan2(height * 0.44, width * 0.56);
    const tailX = x - Math.cos(angle) * length;
    const tailY = y - Math.sin(angle) * length;
    const trail = context.createLinearGradient(tailX, tailY, x, y);
    trail.addColorStop(0, 'rgba(255,255,255,0)');
    trail.addColorStop(0.7, 'rgba(214,238,255,0.5)');
    trail.addColorStop(1, 'rgba(255,255,255,0.92)');
    context.save();
    context.strokeStyle = trail;
    context.lineWidth = 2.2;
    context.lineCap = 'round';
    context.beginPath();
    context.moveTo(tailX, tailY);
    context.lineTo(x, y);
    context.stroke();
    context.fillStyle = 'rgba(255,255,255,0.95)';
    context.beginPath();
    context.arc(x, y, 2.2, 0, Math.PI * 2);
    context.fill();
    context.restore();
  }

  function draw(time) {
    if (!width || !height) return;
    context.clearRect(0, 0, width, height);
    const sway = Math.min(46, height * .05);
    strokeRibbon(time, 48, sway * .28, 3.6, 'rgba(255,255,255,.62)', 9, 1.1, .2);
    strokeRibbon(time, height * .5, sway * .8, 5.2, 'rgba(186,236,255,.5)', 20, .7, 1.1);
    strokeRibbon(time, height * .64, sway, 4.1, 'rgba(198,176,255,.46)', 34, .55, 2.2);
    strokeColumn(time, width * .07, 'rgba(186,236,255,.42)', 18, 1.05, .4);
    strokeColumn(time, width * .93, 'rgba(206,186,255,.38)', 18, .85, 1.6);

    const pool = Math.min(width, height) * .34;
    const gx = width * glowX;
    const gy = height * glowY;
    const light = context.createRadialGradient(gx, gy, 0, gx, gy, pool);
    light.addColorStop(0, 'rgba(255,255,255,.55)');
    light.addColorStop(.42, 'rgba(126,214,255,.28)');
    light.addColorStop(1, 'rgba(126,214,255,0)');
    context.fillStyle = light;
    context.beginPath();
    context.arc(gx, gy, pool, 0, Math.PI * 2);
    context.fill();

    const radius = Math.min(width * .43, 720);
    context.save();
    context.translate(width * .5 + pointerX, Math.min(height * .32, 330) + pointerY);
    context.rotate(-.18 + Math.sin(time * .12) * .015);
    for (const [scale, tilt, alpha] of [[1, 0, .55], [.78, .21, .35]]) {
      context.save();
      context.rotate(tilt);
      const silver = context.createLinearGradient(-radius, 0, radius, 0);
      silver.addColorStop(0, 'rgba(186,236,255,0)');
      silver.addColorStop(.22, `rgba(255,255,255,${alpha})`);
      silver.addColorStop(.62, `rgba(176,164,255,${alpha * .7})`);
      silver.addColorStop(1, 'rgba(186,236,255,0)');
      context.strokeStyle = silver;
      context.lineWidth = scale === 1 ? 2.2 : 1.4;
      context.beginPath();
      context.ellipse(0, 0, radius * scale, radius * .24 * scale, 0, 0, Math.PI * 2);
      context.stroke();
      context.restore();
    }
    const comet = time * 1.35;
    for (let i = 4; i >= 0; i--) {
      const angle = comet - i * .12;
      context.beginPath();
      context.fillStyle = `rgba(255,255,255,${.9 - i * .16})`;
      context.arc(Math.cos(angle) * radius, Math.sin(angle) * radius * .24, Math.max(.6, 2.6 - i * .4), 0, Math.PI * 2);
      context.fill();
    }
    context.restore();

    const points = [];
    stars.forEach(([x, y, size, tone], index) => {
      const px = width * x + pointerX * (index % 2 ? .6 : 1);
      const py = height * y + pointerY * .6 + Math.sin(time * .22 + index) * 3;
      points.push({ x: px, y: py });
    });
    drawConstellation(points);
    drawShootingStar(time);

    stars.forEach(([x, y, size, tone], index) => {
      const shimmer = .35 + (Math.sin(time * .8 + index * 1.3) * .5 + .5) * .5;
      const px = width * x + pointerX * (index % 2 ? .6 : 1);
      const py = height * y + pointerY * .6 + Math.sin(time * .22 + index) * 3;
      context.fillStyle = tone === 0
        ? `rgba(255,255,255,${shimmer})`
        : tone === 1
          ? `rgba(170,230,255,${shimmer})`
          : `rgba(206,186,255,${shimmer})`;
      context.beginPath();
      if (size > 2) {
        context.moveTo(px, py - size * 2);
        context.lineTo(px + size * .5, py - size * .5);
        context.lineTo(px + size * 2, py);
        context.lineTo(px + size * .5, py + size * .5);
        context.lineTo(px, py + size * 2);
        context.lineTo(px - size * .5, py + size * .5);
        context.lineTo(px - size * 2, py);
        context.lineTo(px - size * .5, py - size * .5);
        context.closePath();
      } else context.arc(px, py, size, 0, Math.PI * 2);
      context.fill();
    });
    shieldCopy();
  }

  function active() { return pageActive && !document.hidden && inView; }
  function animated() { return active() && hasIntersection && !motion.matches && width > 0 && height > 0; }
  function stop() {
    if (frame) window.cancelAnimationFrame(frame);
    frame = 0;
    lastPaint = null;
  }
  function tick(now) {
    frame = 0;
    if (!animated()) return;
    const elapsed = lastPaint === null ? 34 : now - lastPaint;
    if (elapsed >= 1000 / 30) {
      phase += Math.min(elapsed, 64) / 1000;
      pointerX += (targetX - pointerX) * .08;
      pointerY += (targetY - pointerY) * .08;
      glowX += (glowTargetX - glowX) * .08;
      glowY += (glowTargetY - glowY) * .08;
      draw(phase);
      lastPaint = now;
    }
    frame = window.requestAnimationFrame(tick);
  }
  function sync() {
    if (!animated()) {
      stop();
      if (active()) {
        if (motion.matches) {
          pointerX = pointerY = targetX = targetY = 0;
          glowX = glowTargetX = .5;
          glowY = glowTargetY = .62;
        }
        draw(motion.matches ? 0 : phase);
      }
    } else if (!frame) frame = window.requestAnimationFrame(tick);
  }
  function resize() {
    const bounds = canvas.getBoundingClientRect();
    const nextWidth = Math.max(0, Math.round(bounds.width));
    const nextHeight = Math.max(0, Math.round(bounds.height));
    const nextRatio = Math.min(2, window.devicePixelRatio || 1);
    if (nextWidth === width && nextHeight === height && nextRatio === ratio) return;
    width = nextWidth; height = nextHeight; ratio = nextRatio;
    shield = null;
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    if (animated()) draw(phase);
    sync();
  }
  function pointer(event) {
    if (motion.matches || !active() || event.pointerType === 'touch') return;
    const bounds = hero.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const nx = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const ny = Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height));
    targetX = (nx - .5) * 12;
    targetY = (ny - .5) * 8;
    glowTargetX = nx;
    glowTargetY = ny;
  }

  hero.addEventListener('pointermove', pointer, { passive: true });
  hero.addEventListener('pointerleave', () => {
    targetX = targetY = 0;
    glowTargetX = .5;
    glowTargetY = .62;
  }, { passive: true });
  document.addEventListener('visibilitychange', sync);
  window.addEventListener('resize', resize, { passive: true });
  window.addEventListener('pagehide', () => { pageActive = false; stop(); });
  window.addEventListener('pageshow', () => { pageActive = true; resize(); sync(); });
  if (motion.addEventListener) motion.addEventListener('change', sync);
  else motion.addListener?.(sync);
  if (hasIntersection) new IntersectionObserver(([entry]) => { inView = entry.isIntersecting; sync(); }).observe(hero);
  else inView = true;
  if (typeof ResizeObserver === 'function') new ResizeObserver(resize).observe(canvas.parentElement);
  resize();
})();
