(() => {
  "use strict";

  const stage = document.querySelector("[data-mist-stage]");
  const canvas = document.querySelector("[data-mist-canvas]");
  const context = canvas?.getContext("2d", { alpha: true, desynchronized: true });
  if (!stage || !canvas || !context) return;

  const TAU = Math.PI * 2;
  const MAX_PIXEL_RATIO = 1.5;
  const MIN_PARTICLES = 34;
  const MAX_PARTICLES = 88;
  const motionPreference = window.matchMedia("(prefers-reduced-motion: reduce)");
  const depthBands = [
    { depth: 0.48, opacity: 0.052, scale: 0.67, phase: 5.4 },
    { depth: 0.62, opacity: 0.064, scale: 0.78, phase: 3.7 },
    { depth: 0.8, opacity: 0.076, scale: 0.9, phase: 1.8 },
    { depth: 1, opacity: 0.088, scale: 1, phase: 0 },
  ];

  let randomState = 0x5f3759df;
  let width = 0;
  let height = 0;
  let pixelRatio = 1;
  let particles = [];
  let sprites = [];
  let animationFrame = 0;
  let previousTime = 0;
  let elapsedTime = 0;
  let isIntersecting = true;

  function random() {
    randomState ^= randomState << 13;
    randomState ^= randomState >>> 17;
    randomState ^= randomState << 5;
    return (randomState >>> 0) / 4294967296;
  }

  function clamp(value, minimum, maximum) {
    return Math.min(Math.max(value, minimum), maximum);
  }

  function smoothstep(edgeStart, edgeEnd, value) {
    const progress = clamp((value - edgeStart) / (edgeEnd - edgeStart), 0, 1);
    return progress * progress * (3 - 2 * progress);
  }

  function addLobe(spriteContext, lobe, color) {
    const gradient = spriteContext.createRadialGradient(
      lobe.x,
      lobe.y,
      0,
      lobe.x,
      lobe.y,
      lobe.radius,
    );
    gradient.addColorStop(0, `rgba(${color}, ${lobe.opacity})`);
    gradient.addColorStop(0.32, `rgba(${color}, ${lobe.opacity * 0.72})`);
    gradient.addColorStop(0.72, `rgba(${color}, ${lobe.opacity * 0.16})`);
    gradient.addColorStop(1, `rgba(${color}, 0)`);

    spriteContext.save();
    spriteContext.translate(lobe.x, lobe.y);
    spriteContext.rotate(lobe.rotation);
    spriteContext.scale(lobe.stretch, 1);
    spriteContext.translate(-lobe.x, -lobe.y);
    spriteContext.fillStyle = gradient;
    spriteContext.beginPath();
    spriteContext.arc(lobe.x, lobe.y, lobe.radius, 0, TAU);
    spriteContext.fill();
    spriteContext.restore();
  }

  function createWispSprite(variation) {
    const sprite = document.createElement("canvas");
    sprite.width = 320;
    sprite.height = 128;
    const spriteContext = sprite.getContext("2d");
    const colors = ["186, 207, 244", "202, 214, 244", "166, 198, 238"];
    const shift = variation * 8;
    const lobes = [
      { x: 210, y: 62 + shift * 0.25, radius: 48, stretch: 1.36, rotation: -0.06, opacity: 0.76 },
      { x: 151, y: 59 - shift * 0.18, radius: 38, stretch: 1.52, rotation: 0.08, opacity: 0.52 },
      { x: 94, y: 68 + shift * 0.12, radius: 28, stretch: 1.7, rotation: -0.1, opacity: 0.3 },
      { x: 255, y: 52 - shift * 0.2, radius: 29, stretch: 1.12, rotation: 0.16, opacity: 0.42 },
      { x: 183, y: 88 - shift * 0.12, radius: 23, stretch: 1.4, rotation: -0.18, opacity: 0.28 },
    ];

    for (const [index, lobe] of lobes.entries()) {
      addLobe(spriteContext, lobe, colors[(index + variation) % colors.length]);
    }
    return sprite;
  }

  function createParticle(spawnAtLeft = false) {
    const bandIndex = Math.min(
      Math.floor(random() * depthBands.length),
      depthBands.length - 1,
    );
    const band = depthBands[bandIndex];
    const size = (0.07 + random() * 0.095) * band.scale;
    const particle = {
      bandIndex,
      x: spawnAtLeft ? -size * (0.25 + random() * 0.7) : random() * (1 + size * 2) - size,
      y: 0.2 + random() * 0.6,
      anchorY: 0.2 + random() * 0.6,
      verticalVelocity: 0,
      speed: (0.015 + random() * 0.025) * (0.82 + band.depth * 0.24),
      lift: 0.001 + random() * 0.0035,
      size,
      aspect: 0.28 + random() * 0.16,
      phase: random() * TAU + band.phase,
      secondaryPhase: random() * TAU,
      breathOffset: (random() - 0.5) * 0.9,
      opacity: band.opacity * (0.78 + random() * 0.46),
      spriteIndex: Math.floor(random() * sprites.length),
    };
    particle.y = particle.anchorY + (random() - 0.5) * 0.08;
    return particle;
  }

  function desiredParticleCount() {
    return clamp(Math.round((width * height) / 6100), MIN_PARTICLES, MAX_PARTICLES);
  }

  function reconcileParticles() {
    const count = desiredParticleCount();
    while (particles.length < count) particles.push(createParticle());
    if (particles.length > count) particles.length = count;
    stage.dataset.particleCount = String(count);
  }

  function resizeCanvas() {
    const bounds = canvas.getBoundingClientRect();
    const nextWidth = Math.max(1, Math.round(bounds.width));
    const nextHeight = Math.max(1, Math.round(bounds.height));
    const nextPixelRatio = Math.min(window.devicePixelRatio || 1, MAX_PIXEL_RATIO);
    if (nextWidth === width && nextHeight === height && nextPixelRatio === pixelRatio) return;

    width = nextWidth;
    height = nextHeight;
    pixelRatio = nextPixelRatio;
    canvas.width = Math.round(width * pixelRatio);
    canvas.height = Math.round(height * pixelRatio);
    context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
    reconcileParticles();
  }

  function recycleParticle(particle) {
    const replacement = createParticle(true);
    Object.assign(particle, replacement);
  }

  function advanceParticle(particle, deltaTime) {
    const band = depthBands[particle.bandIndex];
    const coherentCurl = Math.sin(
      particle.x * TAU * 1.35 + elapsedTime * 0.24 + band.phase,
    );
    const localCurl = Math.cos(
      (particle.x * 1.9 + particle.y * 0.8) * TAU - elapsedTime * 0.17 + particle.phase,
    );
    const targetVerticalVelocity =
      (coherentCurl * 0.009 + localCurl * 0.006 - particle.lift) * band.depth;
    const easing = 1 - Math.exp(-1.8 * deltaTime);

    particle.verticalVelocity +=
      (targetVerticalVelocity - particle.verticalVelocity) * easing;
    particle.y += particle.verticalVelocity * deltaTime;
    particle.y += (particle.anchorY - particle.y) * (1 - Math.exp(-0.22 * deltaTime));

    const gust =
      0.91 +
      Math.sin(elapsedTime * 0.31 + particle.secondaryPhase) * 0.11 +
      Math.cos(particle.y * TAU * 2.2 - elapsedTime * 0.19 + band.phase) * 0.06;
    particle.x += particle.speed * Math.max(0.68, gust) * deltaTime;

    if (particle.x - particle.size > 1.08) recycleParticle(particle);
  }

  function drawParticle(particle) {
    const band = depthBands[particle.bandIndex];
    const edgeFade =
      smoothstep(-particle.size, 0.1, particle.x) *
      (1 - smoothstep(0.8, 1 + particle.size, particle.x));
    const verticalFade =
      smoothstep(0.08, 0.24, particle.y) *
      (1 - smoothstep(0.76, 0.94, particle.y));
    const shimmer = 0.84 + Math.sin(elapsedTime * 0.3 + particle.secondaryPhase) * 0.16;
    const opacity = particle.opacity * edgeFade * verticalFade * shimmer;
    if (opacity <= 0.001) return;

    const breath =
      1 + Math.sin(elapsedTime * 0.249 + particle.breathOffset) * (0.028 + band.depth * 0.025);
    const drawWidth = clamp(particle.size * width * breath, 52, Math.min(250, width * 0.32));
    const drawHeight = drawWidth * particle.aspect;
    const angle = clamp(
      Math.atan2(particle.verticalVelocity * height, particle.speed * width) * 0.45,
      -0.18,
      0.18,
    );

    context.save();
    context.translate(particle.x * width, particle.y * height);
    context.rotate(angle);
    context.globalAlpha = opacity;
    context.drawImage(
      sprites[particle.spriteIndex],
      -drawWidth * 0.56,
      -drawHeight * 0.5,
      drawWidth,
      drawHeight,
    );
    context.restore();
  }

  function renderFrame(currentTime) {
    animationFrame = 0;
    if (!shouldAnimate()) {
      syncAnimation();
      return;
    }

    resizeCanvas();
    const deltaTime = previousTime
      ? Math.min((currentTime - previousTime) / 1000, 0.05)
      : 0;
    previousTime = currentTime;
    elapsedTime += deltaTime;

    context.clearRect(0, 0, width, height);
    context.globalCompositeOperation = "screen";
    for (const particle of particles) {
      advanceParticle(particle, deltaTime);
      drawParticle(particle);
    }
    context.globalAlpha = 1;
    context.globalCompositeOperation = "source-over";

    animationFrame = window.requestAnimationFrame(renderFrame);
  }

  function shouldAnimate() {
    return !motionPreference.matches && isIntersecting && !document.hidden;
  }

  function syncAnimation() {
    const animate = shouldAnimate();
    stage.classList.toggle("is-static", motionPreference.matches);
    stage.dataset.motion = motionPreference.matches ? "static" : "flowing";

    if (!animate) {
      if (animationFrame) window.cancelAnimationFrame(animationFrame);
      animationFrame = 0;
      previousTime = 0;
      if (motionPreference.matches) context.clearRect(0, 0, width, height);
      return;
    }

    if (!animationFrame) {
      previousTime = 0;
      animationFrame = window.requestAnimationFrame(renderFrame);
    }
  }

  sprites = [0, 1, 2].map(createWispSprite);
  resizeCanvas();

  if ("ResizeObserver" in window) {
    new ResizeObserver(resizeCanvas).observe(canvas);
  } else {
    window.addEventListener("resize", resizeCanvas, { passive: true });
  }

  if ("IntersectionObserver" in window) {
    new IntersectionObserver(([entry]) => {
      isIntersecting = entry.isIntersecting;
      syncAnimation();
    }).observe(stage);
  }

  document.addEventListener("visibilitychange", syncAnimation);
  if ("addEventListener" in motionPreference) {
    motionPreference.addEventListener("change", syncAnimation);
  } else {
    motionPreference.addListener(syncAnimation);
  }
  syncAnimation();
})();
