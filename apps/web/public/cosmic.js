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
      attribute vec3 aPosition;
      attribute float aSize;
      attribute vec4 aColor;
      attribute float aLayer;
      uniform vec2 uResolution;
      uniform vec3 uCamera;
      uniform float uTime;
      uniform float uRatio;
      varying mediump vec4 vColor;
      varying mediump vec2 vTravel;
      varying mediump float vNear;
      void main() {
        vec3 position = aPosition;
        if (aLayer < 0.5) {
          float radius = length(position.xz);
          float initialAngle = atan(position.z, position.x);
          float angularVelocity = 0.22 / max(radius / 5.0, 0.35);
          float angle = initialAngle + uTime * angularVelocity;
          float arm = sin(2.0 * angle - 3.4 * log(max(radius, 0.4)) - uTime * 0.14);
          float armBoost = 0.18 + 0.82 * smoothstep(-0.35, 0.9, arm);
          float core = 1.0 - smoothstep(1.2, 10.0, radius);
          float ring = smoothstep(6.0, 10.0, radius) * (1.0 - smoothstep(11.0, 14.0, radius));
          float rim = 1.0 - 0.78 * smoothstep(11.5, 14.5, radius);
          float twinkle = 0.72 + 0.28 * sin(uTime * (0.55 + aSize * 4.0) + aPosition.x * 1.7 + aPosition.z * 2.1);
          float farBoost = 1.0 + 0.55 * smoothstep(0.0, 9.0, position.z);
          float cycle = 0.5 + 0.5 * sin(uTime * 0.65);
          float spread = 0.68 + cycle * 0.60;
          float density = 1.25 - cycle * 0.35;
          position = vec3(cos(angle) * radius * spread, position.y, sin(angle) * radius * spread);
          vColor = vec4(aColor.rgb, aColor.a * armBoost * twinkle * farBoost * rim * density * (0.55 + core * 0.35 + ring * 0.35));
        } else {
          float span = aLayer < 1.5 ? 44.0 : (aLayer < 2.5 ? 108.0 : 204.0);
          float speed = aLayer < 1.5 ? 18.0 : (aLayer < 2.5 ? 10.0 : 6.0);
          position.z = -4.0 - mod(-position.z - 4.0 - uTime * speed, span);
          vColor = aColor;
        }
        vec3 view = position - uCamera;
        float tilt = 0.50;
        float cy = view.y * cos(tilt) - view.z * sin(tilt);
        float cz = view.y * sin(tilt) + view.z * cos(tilt);
        float depth = max(0.5, -cz);
        float aspect = uResolution.x / uResolution.y;
        vec2 clip = vec2(view.x / aspect, cy) * 2.3;
        gl_Position = vec4(clip, 0.0, depth);
        vNear = aLayer > 0.5 ? 1.0 - smoothstep(8.0, 42.0, depth) : 0.0;
        vTravel = normalize(clip + vec2(0.0001));
        float pointScale = aLayer < 0.5 ? 1.75 : 1.3;
        float minSize = aLayer < 0.5 ? 0.85 : 0.6;
        float size = clamp(aSize * uResolution.y * pointScale / depth, minSize * uRatio, 14.0 * uRatio);
        gl_PointSize = size * (1.0 + vNear * 1.8);
        float fog = exp(-depth * 0.0055);
        float arrival = smoothstep(4.0, 12.0, depth);
        float shimmer = 0.78 + 0.22 * sin(uTime * 0.7 + aPosition.x * 0.31 + aPosition.z * 0.17);
        float luminance = aLayer < 0.5 ? 3.4 : 2.1;
        vColor.a = min(1.0, vColor.a * luminance) * fog * arrival * shimmer;
      }
    `;
    const starFragment = `
      precision mediump float;
      varying mediump vec4 vColor;
      varying mediump vec2 vTravel;
      varying mediump float vNear;
      void main() {
        vec2 point = gl_PointCoord - 0.5;
        float along = dot(point, vTravel);
        float across = dot(point, vec2(-vTravel.y, vTravel.x));
        float radius = length(vec2(along, across * (1.0 + vNear * 4.0)));
        float core = 1.0 - smoothstep(0.04, 0.27, radius);
        float halo = 1.0 - smoothstep(0.15, 0.5, radius);
        gl_FragColor = vec4(vColor.rgb, vColor.a * (core * 0.8 + halo * 0.2));
      }
    `;
    const quadVertex = `
      attribute vec2 aPosition;
      void main() { gl_Position = vec4(aPosition, 0.0, 1.0); }
    `;
    const nebulaFragment = `
      precision mediump float;
      uniform vec2 uResolution;
      uniform vec3 uCamera;
      uniform float uTime;
      float hash(vec2 p) {
        vec3 value = fract(vec3(p.xyx) * 0.1031);
        value += dot(value, value.yzx + 33.33);
        return fract((value.x + value.y) * value.z);
      }
      float noise(vec2 p) {
        vec2 cell = floor(p);
        vec2 fraction = fract(p);
        vec2 blend = fraction * fraction * (3.0 - 2.0 * fraction);
        return mix(mix(hash(cell), hash(cell + vec2(1.0, 0.0)), blend.x),
          mix(hash(cell + vec2(0.0, 1.0)), hash(cell + vec2(1.0, 1.0)), blend.x), blend.y);
      }
      float fbm(vec2 p) {
        float value = 0.0;
        float amplitude = 0.5;
        for (int i = 0; i < 3; i++) {
          value += amplitude * noise(p);
          p = p * 2.03 + 17.1;
          amplitude *= 0.5;
        }
        return value;
      }
      void main() {
        vec2 uv = gl_FragCoord.xy / uResolution;
        float aspect = uResolution.x / uResolution.y;
        vec3 ray = normalize(vec3((uv.x - 0.5) * aspect, uv.y - 0.5, -1.15));
        vec3 farPoint = uCamera + ray * ((170.0 + uCamera.z) / -ray.z);
        vec3 nearPoint = uCamera + ray * ((64.0 + uCamera.z) / -ray.z);
        float farCloud = fbm(farPoint.xy * 0.026 + vec2(uTime * 0.002, 0.0));
        float nearCloud = fbm(nearPoint.xy * 0.052 + fbm(nearPoint.xy * 0.03) * 1.4);
        vec3 color = vec3(0.002, 0.002, 0.009);
        color += vec3(0.07, 0.025, 0.18) * smoothstep(0.22, 0.72, farCloud);
        color += vec3(0.035, 0.014, 0.085) * smoothstep(0.28, 0.78, nearCloud) * 0.72;
        float vignette = 1.0 - smoothstep(0.4, 0.98, length((uv - 0.5) * vec2(1.0, 0.82)));
        gl_FragColor = vec4(color * vignette, 1.0);
      }
    `;

    function shader(type, source) {
      const item = gl.createShader(type);
      if (!item) return null;
      gl.shaderSource(item, source);
      gl.compileShader(item);
      return gl.getShaderParameter(item, gl.COMPILE_STATUS) ? item : null;
    }

    function program(vertexSource, fragmentSource) {
      const vertex = shader(gl.VERTEX_SHADER, vertexSource);
      const fragment = shader(gl.FRAGMENT_SHADER, fragmentSource);
      if (!vertex || !fragment) return null;
      const result = gl.createProgram();
      if (!result) return null;
      gl.attachShader(result, vertex);
      gl.attachShader(result, fragment);
      gl.linkProgram(result);
      return gl.getProgramParameter(result, gl.LINK_STATUS) ? result : null;
    }

    const starProgram = program(starVertex, starFragment);
    const nebulaProgram = program(quadVertex, nebulaFragment);
    if (!starProgram || !nebulaProgram) return null;
    const starBuffer = gl.createBuffer();
    const quadBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, quadBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
    const starAttributes = Object.fromEntries(["aPosition", "aSize", "aColor", "aLayer"].map((name) => [name, gl.getAttribLocation(starProgram, name)]));
    const quadAttribute = gl.getAttribLocation(nebulaProgram, "aPosition");
    const starUniforms = Object.fromEntries(["uResolution", "uCamera", "uTime", "uRatio"].map((name) => [name, gl.getUniformLocation(starProgram, name)]));
    const nebulaUniforms = Object.fromEntries(["uResolution", "uCamera", "uTime"].map((name) => [name, gl.getUniformLocation(nebulaProgram, name)]));
    let field = [];
    let meteor = null;
    let nextMeteorAt = 3.4;

    function colorFor(tone) {
      if (tone > 0.94) return [1.0, 0.97, 1.0];
      if (tone > 0.72) return [0.82, 0.72, 1.0];
      if (tone > 0.42) return [0.62, 0.46, 1.0];
      return [0.42, 0.30, 0.85];
    }

    function buildStars() {
      field = [];
      const aspect = width / Math.max(1, height);
      const fieldCount = Math.max(6000, Math.min(14000, Math.round(width * height / 100)));
      const galaxyCount = Math.max(12000, Math.min(28000, Math.round(width * height / 45)));
      for (let index = 0; index < fieldCount; index++) {
        const distribution = random();
        const layer = distribution < 0.14 ? 1 : distribution < 0.48 ? 2 : 3;
        const span = layer === 1 ? 44 : layer === 2 ? 108 : 204;
        const x = (random() - 0.5) * span * aspect * 1.5;
        const y = (random() - 0.5) * span * 1.15;
        const z = -4 - random() * span;
        const radius = 0.04 + random() * 0.07;
        const color = colorFor(random());
        field.push(x, y, z, radius, ...color, 0.45 + random() * 0.45, layer);
      }
      for (let index = 0; index < galaxyCount; index++) {
        const radius = 0.65 + Math.pow(random(), 0.72) * 13.2;
        const armAngle = (index % 2) * Math.PI + Math.log(radius) * 1.7 + Math.PI / 4;
        const angle = random() < 0.18 ? random() * Math.PI * 2 : armAngle + (random() - 0.5) * (0.55 + radius * 0.025);
        const thickness = (random() - 0.5) * (1.5 - Math.min(1.1, radius / 14));
        const size = 0.035 + random() * 0.065;
        const color = colorFor(random());
        const alpha = 0.52 + random() * 0.48;
        field.push(Math.cos(angle) * radius, thickness, Math.sin(angle) * radius, size, ...color, alpha, 0);
      }
    }

    function draw(time) {
      if (!width || !height) return;
      const camera = [pointerX * 0.35 + Math.sin(time * 0.07) * 1.4,
        7.8 + pointerY * 0.3 + Math.cos(time * 0.05) * 0.5, 18 + Math.sin(time * 0.035) * 0.8];
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.disable(gl.BLEND);
      gl.useProgram(nebulaProgram);
      gl.uniform2f(nebulaUniforms.uResolution, canvas.width, canvas.height);
      gl.uniform3f(nebulaUniforms.uCamera, ...camera);
      gl.uniform1f(nebulaUniforms.uTime, time);
      gl.bindBuffer(gl.ARRAY_BUFFER, quadBuffer);
      gl.vertexAttribPointer(quadAttribute, 2, gl.FLOAT, false, 0, 0);
      gl.enableVertexAttribArray(quadAttribute);
      gl.drawArrays(gl.TRIANGLES, 0, 6);

      const vertices = field.slice();
      if (time >= nextMeteorAt && !meteor) {
        meteor = { start: nextMeteorAt, x: 10 + random() * 9, y: 1 + random() * 5,
          z: -14 - random() * 8, vx: -18 - random() * 8, vy: -3 - random() * 4 };
      }
      if (meteor) {
        const progress = (time - meteor.start) / 1.3;
        if (progress >= 1) {
          meteor = null;
          nextMeteorAt = time + 6 + random() * 5;
        } else {
          for (let index = 0; index < 30; index++) {
            const t = progress - index * 0.018;
            if (t < 0) continue;
            vertices.push(meteor.x + meteor.vx * t, meteor.y + meteor.vy * t, meteor.z,
              0.09 * (1 - index / 34), 0.72, 0.58, 1.0,
              (1 - index / 30) * Math.sin(progress * Math.PI), -1);
          }
        }
      }

      gl.useProgram(starProgram);
      gl.uniform2f(starUniforms.uResolution, canvas.width, canvas.height);
      gl.uniform3f(starUniforms.uCamera, ...camera);
      gl.uniform1f(starUniforms.uTime, time);
      gl.uniform1f(starUniforms.uRatio, ratio);
      gl.bindBuffer(gl.ARRAY_BUFFER, starBuffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.DYNAMIC_DRAW);
      gl.vertexAttribPointer(starAttributes.aPosition, 3, gl.FLOAT, false, 36, 0);
      gl.vertexAttribPointer(starAttributes.aSize, 1, gl.FLOAT, false, 36, 12);
      gl.vertexAttribPointer(starAttributes.aColor, 4, gl.FLOAT, false, 36, 16);
      gl.vertexAttribPointer(starAttributes.aLayer, 1, gl.FLOAT, false, 36, 32);
      Object.values(starAttributes).forEach((attribute) => gl.enableVertexAttribArray(attribute));
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE);
      gl.drawArrays(gl.POINTS, 0, vertices.length / 9);
    }

    buildStars();
    canvas.dataset.renderer = "webgl";
    return { draw, rebuild: buildStars };
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

    function buildStars() {
      const count = Math.max(480, Math.min(1100, Math.round((width * height) / 1100)));
      stars = Array.from({ length: count }, (_, index) => ({
        radius: 0.5 + Math.pow(random(), 0.7) * 12.5,
        angle: random() * Math.PI * 2,
        arm: index % 2,
        scattered: random() < 0.18,
        spread: (random() - 0.5) * 0.9,
        depth: 0.2 + random() * 0.8,
        phase: random() * Math.PI * 2,
        size: 0.45 + random() * 1.65,
        tone: random()
      }));
    }

    function drawBackground() {
      context.fillStyle = "#020208";
      context.fillRect(0, 0, width, height);
      const radius = Math.max(width, height) * 0.82;
      const violet = context.createRadialGradient(width * 0.5, height * 0.52, 0, width * 0.5, height * 0.52, radius);
      violet.addColorStop(0, "rgba(88,54,173,0.22)");
      violet.addColorStop(0.52, "rgba(41,22,90,0.11)");
      violet.addColorStop(1, "rgba(2,2,8,0)");
      context.fillStyle = violet;
      context.fillRect(0, 0, width, height);
      const blue = context.createRadialGradient(width * 0.18, height * 0.18, 0, width * 0.18, height * 0.18, radius * 0.72);
      blue.addColorStop(0, "rgba(52,38,120,0.16)");
      blue.addColorStop(1, "rgba(2,2,8,0)");
      context.fillStyle = blue;
      context.fillRect(0, 0, width, height);
    }

    function draw(time) {
      if (!width || !height) return;
      context.clearRect(0, 0, width, height);
      drawBackground();
      const scale = Math.min(width, height) / 15;
      const cycle = 0.5 + 0.5 * Math.sin(time * 0.65);
      const spread = 0.68 + cycle * 0.60;
      stars.forEach((star) => {
        const speed = 0.22 / Math.max(star.radius / 5, 0.35);
        const armAngle = star.arm * Math.PI + Math.log(star.radius) * 1.7 + Math.PI / 4;
        const angle = (star.scattered ? star.angle : armAngle + star.spread) + time * speed;
        const x = width / 2 + Math.cos(angle) * star.radius * spread * scale + pointerX * star.depth;
        const y = height / 2 + Math.sin(angle) * star.radius * spread * scale * 0.42 + pointerY * star.depth;
        const twinkle = 0.55 + (Math.sin(time * 0.65 + star.phase) * 0.5 + 0.5) * 0.45;
        const rim = 1 - Math.min(0.78, star.radius / 18);
        const alpha = twinkle * rim * (1.25 - cycle * 0.35);
        if (star.tone > 0.72) context.fillStyle = `rgba(207,184,255,${alpha})`;
        else if (star.tone > 0.4) context.fillStyle = `rgba(151,113,250,${alpha})`;
        else context.fillStyle = `rgba(98,69,190,${alpha})`;
        context.beginPath();
        context.arc(x, y, star.size * (0.7 + star.depth * 0.45), 0, Math.PI * 2);
        context.fill();
      });
    }

    buildStars();
    canvas.dataset.renderer = "canvas";
    return { draw, rebuild: buildStars, setTransform: (value) => context.setTransform(value, 0, 0, value, 0, 0) };
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
    targetX = (x - 0.5) * 10;
    targetY = (y - 0.5) * 6;
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
