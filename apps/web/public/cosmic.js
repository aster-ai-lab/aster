(() => {
  let canvas = document.getElementById("download-cosmos");
  const section = canvas?.closest(".download");
  if (!canvas || !section) return;
  const motion = window.matchMedia?.("(prefers-reduced-motion: reduce)") ?? { matches: true };
  const hasIntersection = typeof IntersectionObserver === "function";
  let width = 0;
  let height = 0;
  let ratio = 0;
  let frame = 0;
  let lastPaint = null;
  let elapsed = 0;
  let inView = false;
  let pageActive = true;
  let pointerX = 0;
  let pointerY = 0;
  let targetX = 0;
  let targetY = 0;
  let renderer = null;
  let seed = 20261026;

  function random() {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 4294967296;
  }

  function createWebGLRenderer() {
    const gl = canvas.getContext("webgl", { alpha: false, antialias: false, powerPreference: "low-power" });
    if (!gl || typeof gl.createShader !== "function") return null;
    const starVertex = `
      attribute vec2 aPosition;
      attribute float aSize;
      attribute vec4 aColor;
      varying vec4 vColor;
      void main() {
        gl_Position = vec4(aPosition, 0.0, 1.0);
        gl_PointSize = aSize;
        vColor = aColor;
      }
    `;
    const starFragment = `
      precision mediump float;
      varying vec4 vColor;
      void main() {
        vec2 point = gl_PointCoord - 0.5;
        float distanceToCenter = length(point);
        float glow = smoothstep(0.5, 0.04, distanceToCenter);
        float core = smoothstep(0.17, 0.0, distanceToCenter);
        gl_FragColor = vec4(vColor.rgb, vColor.a * (glow * 0.72 + core * 0.5));
      }
    `;
    const quadVertex = `
      attribute vec2 aPosition;
      void main() { gl_Position = vec4(aPosition, 0.0, 1.0); }
    `;
    const nebulaFragment = `
      precision mediump float;
      uniform vec2 uResolution;
      uniform float uTime;
      uniform vec2 uPointer;
      float hash(vec2 p) {
        return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
      }
      float noise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        vec2 u = f * f * (3.0 - 2.0 * f);
        return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
          mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
      }
      float fbm(vec2 p) {
        float value = 0.0;
        float amplitude = 0.5;
        for (int i = 0; i < 4; i++) {
          value += amplitude * noise(p);
          p = p * 2.03 + 17.1;
          amplitude *= 0.5;
        }
        return value;
      }
      void main() {
        vec2 uv = gl_FragCoord.xy / uResolution.xy;
        vec2 p = uv * 3.2 + uPointer * 0.045 + vec2(uTime * 0.007, -uTime * 0.004);
        float cloud = fbm(p + fbm(p * 1.55));
        float mist = smoothstep(0.32, 0.82, cloud);
        vec3 color = vec3(0.012, 0.018, 0.045);
        color = mix(color, vec3(0.08, 0.16, 0.38), mist * 0.58);
        float violet = smoothstep(0.46, 0.84, fbm(p * 1.36 + 4.2));
        color = mix(color, vec3(0.18, 0.07, 0.32), violet * 0.34);
        float vignette = 1.0 - smoothstep(0.35, 0.92, length(uv - 0.5));
        gl_FragColor = vec4(color * vignette, 1.0);
      }
    `;

    function shader(type, source) {
      const item = gl.createShader(type);
      gl.shaderSource(item, source);
      gl.compileShader(item);
      if (!gl.getShaderParameter(item, gl.COMPILE_STATUS) || !item) return null;
      return item;
    }

    function program(vertexSource, fragmentSource) {
      const result = gl.createProgram();
      gl.attachShader(result, shader(gl.VERTEX_SHADER, vertexSource));
      gl.attachShader(result, shader(gl.FRAGMENT_SHADER, fragmentSource));
      gl.linkProgram(result);
      if (!gl.getProgramParameter(result, gl.LINK_STATUS)) return null;
      return result;
    }

    const starProgram = program(starVertex, starFragment);
    const nebulaProgram = program(quadVertex, nebulaFragment);
    if (!starProgram || !nebulaProgram) return null;
    const starBuffer = gl.createBuffer();
    const quadBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, quadBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
    const uniforms = {
      resolution: gl.getUniformLocation(nebulaProgram, "uResolution"),
      time: gl.getUniformLocation(nebulaProgram, "uTime"),
      pointer: gl.getUniformLocation(nebulaProgram, "uPointer")
    };
    let stars = [];
    let galaxy = [];
    let meteor = null;
    let nextMeteorAt = 3.4;

    function buildStars() {
      const fieldCount = Math.max(240, Math.min(420, Math.round((width * height) / 4200)));
      const galaxyCount = Math.max(320, Math.min(680, Math.round((width * height) / 3800)));
      stars = Array.from({ length: fieldCount }, () => ({
        x: (random() - 0.5) * 65,
        y: (random() - 0.5) * 42,
        z: -12 - random() * 112,
        radius: 0.45 + random() * 1.2,
        phase: random() * Math.PI * 2,
        tone: random()
      }));
      galaxy = Array.from({ length: galaxyCount }, (_, index) => ({
        arm: index % 3,
        radius: 2 + Math.pow(random(), 0.65) * 22,
        angle: random() * Math.PI * 2,
        phase: random() * Math.PI * 2,
        y: (random() - 0.5) * 1.8,
        z: (random() - 0.5) * 4,
        tone: random()
      }));
    }

    function project(x, y, z, time) {
      const depth = Math.max(1.4, -z);
      const focal = height * 1.08;
      const cameraX = Math.sin(time * 0.08) * 1.2;
      const cameraY = Math.cos(time * 0.06) * 0.8;
      const px = width * 0.5 + ((x + cameraX) / depth) * focal + pointerX * (18 / depth);
      const py = height * 0.5 + ((y + cameraY) / depth) * focal + pointerY * (18 / depth);
      return [(px / width) * 2 - 1, 1 - (py / height) * 2, depth];
    }

    function vertex(vertices, projected, size, color, alpha) {
      if (projected[0] < -1.2 || projected[0] > 1.2 || projected[1] < -1.2 || projected[1] > 1.2) return;
      const fog = Math.max(0.08, 1 - projected[2] / 145);
      vertices.push(projected[0], projected[1], size, color[0], color[1], color[2], alpha * fog);
    }

    function colorFor(tone) {
      if (tone > 0.84) return [0.77, 0.62, 1.0];
      if (tone > 0.55) return [0.48, 0.72, 1.0];
      return [0.94, 0.96, 1.0];
    }

    function draw(time) {
      if (!width || !height) return;
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.disable(gl.BLEND);
      gl.useProgram(nebulaProgram);
      gl.uniform2f(uniforms.resolution, canvas.width, canvas.height);
      gl.uniform1f(uniforms.time, time);
      gl.uniform2f(uniforms.pointer, pointerX, pointerY);
      gl.bindBuffer(gl.ARRAY_BUFFER, quadBuffer);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      gl.enableVertexAttribArray(0);
      gl.drawArrays(gl.TRIANGLES, 0, 6);

      const vertices = [];
      const focal = height * 1.08;
      stars.forEach((star) => {
        let z = star.z + time * 1.35;
        while (z > -2) z -= 112;
        const shimmer = 0.32 + (Math.sin(time * 0.85 + star.phase) * 0.5 + 0.5) * 0.68;
        const size = Math.min(18, Math.max(1, (star.radius * focal * ratio) / Math.max(1.4, -z)));
        vertex(vertices, project(star.x, star.y, z, time), size, colorFor(star.tone), shimmer);
      });
      galaxy.forEach((star) => {
        const angle = star.arm * (Math.PI * 2 / 3) + star.radius * 0.48 + star.angle + time * 0.045;
        const x = Math.cos(angle) * star.radius + 14;
        const z = Math.sin(angle) * star.radius - 62 + star.z;
        const y = star.y + Math.sin(angle * 1.7) * 0.7 - 4;
        const shimmer = 0.34 + (Math.sin(time * 0.72 + star.phase) * 0.5 + 0.5) * 0.66;
        const size = Math.min(14, Math.max(1, (0.9 * focal * ratio) / Math.max(8, -z)));
        vertex(vertices, project(x, y, z, time), size, colorFor(star.tone), Math.min(1, shimmer * 1.25));
      });

      if (time >= nextMeteorAt && !meteor) {
        meteor = {
          start: nextMeteorAt,
          x: (random() - 0.5) * 24,
          y: 8 + random() * 8,
          z: -18 - random() * 20,
          vx: -16 - random() * 12,
          vy: -5 - random() * 4
        };
      }
      if (meteor) {
        const progress = (time - meteor.start) / 1.3;
        if (progress >= 1) {
          meteor = null;
          nextMeteorAt = time + 5 + random() * 5;
        } else {
          for (let index = 0; index < 34; index++) {
            const t = progress - index * 0.018;
            if (t < 0) continue;
            const x = meteor.x + meteor.vx * t;
            const y = meteor.y + meteor.vy * t;
            const depth = Math.max(1.4, -meteor.z);
            const size = Math.min(12, Math.max(1, ((1.4 - index * 0.025) * focal * ratio) / depth));
            vertex(vertices, project(x, y, meteor.z, time), size, [0.72, 0.86, 1], (1 - index / 34) * Math.sin(progress * Math.PI));
          }
        }
      }

      gl.useProgram(starProgram);
      gl.bindBuffer(gl.ARRAY_BUFFER, starBuffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.DYNAMIC_DRAW);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 28, 0);
      gl.vertexAttribPointer(1, 1, gl.FLOAT, false, 28, 8);
      gl.vertexAttribPointer(2, 4, gl.FLOAT, false, 28, 12);
      gl.enableVertexAttribArray(0);
      gl.enableVertexAttribArray(1);
      gl.enableVertexAttribArray(2);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE);
      gl.drawArrays(gl.POINTS, 0, vertices.length / 7);
    }

    buildStars();
    canvas.dataset.renderer = "webgl";
    return { draw: draw, rebuild: buildStars };
  }

  function createCanvasRenderer() {
    let context;
    try {
      context = canvas.getContext("2d");
    } catch {
      return null;
    }
    if (!context) return null;
    let stars = [];
    let meteor = null;
    let nextMeteorAt = 4.2;

    function buildStars() {
      const count = Math.max(70, Math.min(170, Math.round((width * height) / 8500)));
      stars = Array.from({ length: count }, (_, index) => ({
        x: random(), y: random(), radius: 0.35 + random() * 1.35,
        depth: 0.2 + random() * 0.8, phase: random() * Math.PI * 2, blue: index % 5 === 0
      }));
    }

    function drawBackground() {
      context.fillStyle = "#070910";
      context.fillRect(0, 0, width, height);
      const radius = Math.max(width, height) * 0.82;
      const blue = context.createRadialGradient(width * 0.18, height * 0.22, 0, width * 0.18, height * 0.22, radius);
      blue.addColorStop(0, "rgba(54,82,154,0.24)");
      blue.addColorStop(0.46, "rgba(34,52,105,0.12)");
      blue.addColorStop(1, "rgba(7,9,16,0)");
      context.fillStyle = blue;
      context.fillRect(0, 0, width, height);
      const violet = context.createRadialGradient(width * 0.84, height * 0.64, 0, width * 0.84, height * 0.64, radius * 0.76);
      violet.addColorStop(0, "rgba(93,62,135,0.17)");
      violet.addColorStop(0.55, "rgba(48,38,89,0.08)");
      violet.addColorStop(1, "rgba(7,9,16,0)");
      context.fillStyle = violet;
      context.fillRect(0, 0, width, height);
    }

    function drawMeteor(time) {
      if (time < nextMeteorAt) return;
      if (!meteor) meteor = { start: nextMeteorAt, x: 0.68 + random() * 0.24, y: 0.06 + random() * 0.2, dx: -0.25 - random() * 0.14, dy: 0.2 + random() * 0.12 };
      const progress = (time - meteor.start) / 1.15;
      if (progress >= 1) {
        meteor = null;
        nextMeteorAt = time + 5 + random() * 5;
        return;
      }
      const x = (meteor.x + meteor.dx * progress) * width;
      const y = (meteor.y + meteor.dy * progress) * height;
      const angle = Math.atan2(meteor.dy * height, meteor.dx * width);
      const length = 120 + progress * 110;
      const tailX = x - Math.cos(angle) * length;
      const tailY = y - Math.sin(angle) * length;
      const alpha = Math.sin(progress * Math.PI) * 0.8;
      const trail = context.createLinearGradient(tailX, tailY, x, y);
      trail.addColorStop(0, "rgba(170,204,255,0)");
      trail.addColorStop(0.72, `rgba(194,220,255,${alpha * 0.55})`);
      trail.addColorStop(1, `rgba(255,255,255,${alpha})`);
      context.strokeStyle = trail;
      context.lineWidth = 1.6;
      context.lineCap = "round";
      context.beginPath();
      context.moveTo(tailX, tailY);
      context.lineTo(x, y);
      context.stroke();
    }

    function draw(time) {
      if (!width || !height) return;
      context.clearRect(0, 0, width, height);
      drawBackground();
      stars.forEach((star) => {
        const drift = (star.x + time * 0.0012 * star.depth) % 1;
        const x = drift * width + pointerX * star.depth;
        const y = (star.y + Math.sin(time * 0.08 + star.phase) * 0.004) * height + pointerY * star.depth;
        const shimmer = 0.22 + (Math.sin(time * (0.65 + star.depth * 0.65) + star.phase) * 0.5 + 0.5) * 0.68;
        context.fillStyle = star.blue ? `rgba(167,205,255,${shimmer})` : `rgba(244,247,255,${shimmer})`;
        context.beginPath();
        context.arc(x, y, star.radius * (0.75 + star.depth * 0.45), 0, Math.PI * 2);
        context.fill();
      });
      drawMeteor(time);
    }

    buildStars();
    canvas.dataset.renderer = "canvas";
    return { draw: draw, rebuild: buildStars, setTransform: (value) => context.setTransform(value, 0, 0, value, 0, 0) };
  }

  function selectRenderer() {
    try {
      renderer = createWebGLRenderer();
    } catch {
      renderer = null;
    }
    if (!renderer) renderer = createCanvasRenderer();
    if (!renderer && typeof canvas.cloneNode === "function") {
      const replacement = canvas.cloneNode(false);
      canvas.replaceWith(replacement);
      canvas = replacement;
      renderer = createCanvasRenderer();
    }
  }

  function active() {
    return pageActive && !document.hidden && inView;
  }

  function animated() {
    return active() && hasIntersection && !motion.matches && width > 0 && height > 0 && renderer;
  }

  function stop() {
    if (frame) window.cancelAnimationFrame(frame);
    frame = 0;
    lastPaint = null;
  }

  function tick(now) {
    frame = 0;
    if (!animated()) return;
    const delta = lastPaint === null ? 34 : now - lastPaint;
    if (delta >= 1000 / 30) {
      elapsed += Math.min(delta, 64) / 1000;
      pointerX += (targetX - pointerX) * 0.08;
      pointerY += (targetY - pointerY) * 0.08;
      renderer?.draw(elapsed);
      lastPaint = now;
    }
    frame = window.requestAnimationFrame(tick);
  }

  function sync() {
    if (!animated()) {
      stop();
      if (active()) {
        if (motion.matches) pointerX = pointerY = targetX = targetY = 0;
        renderer?.draw(motion.matches ? 0 : elapsed);
      }
    } else if (!frame) {
      frame = window.requestAnimationFrame(tick);
    }
  }

  function resize() {
    if (!renderer) return;
    const bounds = canvas.getBoundingClientRect();
    const nextWidth = Math.max(0, Math.round(bounds.width));
    const nextHeight = Math.max(0, Math.round(bounds.height));
    const nextRatio = Math.min(2, window.devicePixelRatio || 1);
    if (nextWidth === width && nextHeight === height && nextRatio === ratio) return;
    width = nextWidth;
    height = nextHeight;
    ratio = nextRatio;
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
    renderer?.setTransform?.(ratio);
    renderer?.rebuild?.();
    if (animated()) renderer?.draw(elapsed);
    sync();
  }

  function pointer(event) {
    if (motion.matches || !active() || event.pointerType === "touch") return;
    const bounds = section.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const x = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width));
    const y = Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height));
    targetX = (x - 0.5) * 12;
    targetY = (y - 0.5) * 8;
  }

  canvas.addEventListener?.("webglcontextlost", (event) => {
    event.preventDefault();
    const replacement = canvas.cloneNode(false);
    canvas.replaceWith(replacement);
    canvas = replacement;
    selectRenderer();
    resize();
  });
  document.addEventListener("visibilitychange", sync);
  window.addEventListener("resize", resize, { passive: true });
  window.addEventListener("pagehide", () => {
    pageActive = false;
    stop();
  });
  window.addEventListener("pageshow", () => {
    pageActive = true;
    resize();
    sync();
  });
  if (motion.addEventListener) motion.addEventListener("change", sync);
  else motion.addListener?.(sync);
  if (hasIntersection) {
    new IntersectionObserver(([entry]) => {
      inView = entry.isIntersecting;
      sync();
    }).observe(section);
  } else {
    inView = true;
  }
  if (typeof ResizeObserver === "function") new ResizeObserver(resize).observe(canvas.parentElement);
  selectRenderer();
  if (renderer) {
    section.addEventListener("pointermove", pointer, { passive: true });
    section.addEventListener("pointerleave", () => {
      targetX = targetY = 0;
    }, { passive: true });
  }
  resize();
})();
