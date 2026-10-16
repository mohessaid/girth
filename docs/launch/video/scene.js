// Girth intro: a hand-drawn look over real 3D.
// The scene renders normals, depth and light into one target; a post shader
// turns that into wobbling ink outlines, hatching and paper grain. Lines
// "boil" (redraw slightly) at 12 drawings a second, like hand animation.
// Deterministic: window.renderFrame(n) draws frame n at FPS.
import * as THREE from 'three'

const params = new URLSearchParams(location.search)
const STYLE = params.get('style') ?? 'ink'
const W = Number(params.get('w') ?? 1920)
const H = Number(params.get('h') ?? 1080)
const FPS = 30

// bg, ink, and how the shading is drawn, per style.
const STYLES = {
  ink: { bg: [0.96, 0.955, 0.94], ink: [0.05, 0.05, 0.05], hatch: 0, grain: 0.035, edge: 1.4, wobble: 1.0 },
  chalk: { bg: [0.05, 0.05, 0.05], ink: [0.92, 0.92, 0.91], hatch: 1, grain: 0.07, edge: 1.8, wobble: 1.3 },
  pencil: { bg: [0.98, 0.98, 0.98], ink: [0.32, 0.32, 0.33], hatch: 2, grain: 0.03, edge: 0.8, wobble: 2.2 },
}
const S = STYLES[STYLE]

const renderer = new THREE.WebGLRenderer({ antialias: false, preserveDrawingBuffer: true })
renderer.setPixelRatio(1)
renderer.setSize(W, H)
document.body.appendChild(renderer.domElement)

const target = new THREE.WebGLRenderTarget(W, H, { type: THREE.HalfFloatType, samples: 4 })

const scene = new THREE.Scene()
const camera = new THREE.PerspectiveCamera(32, W / H, 0.1, 100)

// Every object draws its normal (view space), its light and its depth.
const gbuffer = new THREE.ShaderMaterial({
  uniforms: { uLight: { value: new THREE.Vector3(0.5, 0.8, 0.6).normalize() } },
  vertexShader: `
    varying vec3 vN; varying float vZ; varying vec3 vWN;
    void main() {
      vec4 mv = modelViewMatrix * vec4(position, 1.0);
      vN = normalize(normalMatrix * normal);
      vWN = normalize(mat3(modelMatrix) * normal);
      vZ = -mv.z;
      gl_Position = projectionMatrix * mv;
    }`,
  fragmentShader: `
    uniform vec3 uLight; varying vec3 vN; varying float vZ; varying vec3 vWN;
    void main() {
      float lit = clamp(dot(normalize(vWN), uLight) * 0.6 + 0.45, 0.0, 1.0);
      // Normal in rg, light in b, depth in a; a = 0 is empty paper.
      gl_FragColor = vec4(normalize(vN).xy * 0.5 + 0.5, lit, vZ);
    }`,
})

// Skill cards: thin slabs. Most are noise for this project; a few are kept.
const CARDS = 36
const KEPT = new Set([5, 13, 22, 30])
const cardGeo = new THREE.BoxGeometry(1.6, 0.09, 1.0)
const cards = []
let rest = 0
for (let i = 0; i < CARDS; i++) {
  const m = new THREE.Mesh(cardGeo, gbuffer)
  const seed = Math.sin(i * 91.7) * 43758.5
  const r = seed - Math.floor(seed)
  const kept = KEPT.has(i)
  m.userData = {
    y: (i - CARDS / 2) * 0.12,
    twist: (r - 0.5) * 0.5,
    out: new THREE.Vector3(Math.cos(i * 2.4) * 9, (r - 0.5) * 6, Math.sin(i * 2.4) * 9),
    spin: (r - 0.5) * 6,
    kept,
    slot: [...KEPT].indexOf(i),
    line: kept ? -1 : rest++,
  }
  scene.add(m)
  cards.push(m)
}

// Meter: a row of bars, one per turn.
const BARS = [0.9, 1.6, 0.7, 2.4, 1.2, 3.1, 1.0, 1.9]
const bars = BARS.map((h, i) => {
  const m = new THREE.Mesh(new THREE.BoxGeometry(0.45, 1, 0.45), gbuffer)
  m.userData = { h, x: (i - (BARS.length - 1) / 2) * 0.7 }
  scene.add(m)
  return m
})

// Keeper: a clock face and its hand.
const ring = new THREE.Mesh(new THREE.TorusGeometry(1.6, 0.09, 16, 96), gbuffer)
const hand = new THREE.Mesh(new THREE.BoxGeometry(0.08, 1.2, 0.08), gbuffer)
hand.geometry.translate(0, 0.6, 0)
const pivot = new THREE.Mesh(new THREE.SphereGeometry(0.14, 24, 16), gbuffer)
scene.add(ring, hand, pivot)

// The call to action: a chalk star.
const starShape = new THREE.Shape()
for (let i = 0; i < 10; i++) {
  const r = i % 2 === 0 ? 1.2 : 0.5
  const a = Math.PI / 2 + (i * Math.PI) / 5
  if (i === 0) starShape.moveTo(Math.cos(a) * r, Math.sin(a) * r)
  else starShape.lineTo(Math.cos(a) * r, Math.sin(a) * r)
}
starShape.closePath()
const star = new THREE.Mesh(new THREE.ExtrudeGeometry(starShape, { depth: 0.3, bevelEnabled: false }), gbuffer)
star.geometry.center()
scene.add(star)

// The wink: a chalk face, because this video was made with Girth.
const face = new THREE.Group()
const eyeL = new THREE.Mesh(new THREE.SphereGeometry(0.2, 24, 16), gbuffer)
const eyeR = new THREE.Mesh(new THREE.SphereGeometry(0.2, 24, 16), gbuffer)
eyeL.position.set(-0.6, 0.55, 0)
eyeR.position.set(0.6, 0.55, 0)
const smile = new THREE.Mesh(new THREE.TorusGeometry(0.85, 0.07, 12, 64, Math.PI * 0.7), gbuffer)
smile.rotation.z = Math.PI + Math.PI * 0.15
smile.position.set(0, 0.15, 0)
face.add(eyeL, eyeR, smile)
scene.add(face)

// Idle compaction: a big block pressed down to a small one.
const block = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1, 1, 1, 1), gbuffer)
scene.add(block)

// Text, drawn on a 2D canvas and wobbled with the rest of the drawing.
const textCanvas = document.createElement('canvas')
textCanvas.width = W
textCanvas.height = H
const tctx = textCanvas.getContext('2d')
const textTex = new THREE.CanvasTexture(textCanvas)

const post = new THREE.ShaderMaterial({
  uniforms: {
    tScene: { value: target.texture },
    tText: { value: textTex },
    uRes: { value: new THREE.Vector2(W, H) },
    uSeed: { value: 0 },
    uBg: { value: new THREE.Vector3(...S.bg) },
    uInk: { value: new THREE.Vector3(...S.ink) },
    uHatch: { value: S.hatch },
    uGrain: { value: S.grain },
    uEdge: { value: S.edge },
    uWobble: { value: S.wobble },
  },
  vertexShader: `varying vec2 vUv; void main(){ vUv = uv; gl_Position = vec4(position.xy, 0.0, 1.0); }`,
  fragmentShader: `
    precision highp float;
    uniform sampler2D tScene, tText; uniform vec2 uRes; uniform float uSeed, uHatch, uGrain, uEdge, uWobble;
    uniform vec3 uBg, uInk; varying vec2 vUv;

    float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
    float noise(vec2 p){
      vec2 i = floor(p), f = fract(p); f = f*f*(3.0-2.0*f);
      return mix(mix(hash(i), hash(i+vec2(1,0)), f.x), mix(hash(i+vec2(0,1)), hash(i+vec2(1,1)), f.x), f.y);
    }
    vec2 wob(vec2 uv, float k, float amp){
      vec2 p = uv * uRes / k + uSeed * 17.0;
      return uv + (vec2(noise(p), noise(p + 31.7)) - 0.5) * amp / uRes;
    }
    vec4 g(vec2 uv){ return texture2D(tScene, uv); }
    float edgeAt(vec2 uv){
      vec2 px = uEdge / uRes;
      vec4 c = g(uv);
      float e = 0.0;
      for (int i = 0; i < 4; i++) {
        vec2 o = i == 0 ? vec2(px.x,0.) : i == 1 ? vec2(-px.x,0.) : i == 2 ? vec2(0.,px.y) : vec2(0.,-px.y);
        vec4 s = g(uv + o);
        float hasA = step(0.001, c.a), hasB = step(0.001, s.a);
        e += abs(hasA - hasB);
        e += hasA * hasB * (length(c.rg - s.rg) * 2.5 + abs(c.a - s.a) * 1.2);
      }
      return smoothstep(0.35, 0.7, e);
    }
    // Parallel strokes at an angle, broken up so they read as drawn.
    float hatch(vec2 frag, float ang, float freq, float width){
      vec2 d = vec2(cos(ang), sin(ang));
      float t = dot(frag, d) / freq;
      float jitter = noise(frag * 0.02 + uSeed) * 0.6 + noise(vec2(floor(t), uSeed)) * 0.3;
      float line = abs(fract(t + jitter) - 0.5);
      float gap = step(0.25, noise(vec2(floor(t) * 3.1, dot(frag, vec2(-d.y, d.x)) / 90.0 + uSeed)));
      return (1.0 - smoothstep(width * 0.5, width * 0.5 + 0.08, line)) * gap;
    }
    void main(){
      vec2 frag = vUv * uRes;
      vec2 a = wob(vUv, 90.0, 1.6 * uWobble);
      vec2 b = wob(vUv, 70.0, 1.2 * uWobble) + 0.8 / uRes;
      float ink = max(edgeAt(a), edgeAt(b) * 0.3);   // a doubled, slightly offset line
      vec4 c = g(a);
      float dark = c.a > 0.001 ? 1.0 - c.b : 0.0;
      float shade = 0.0;
      if (uHatch < 0.5) {
        shade += step(0.30, dark) * hatch(frag, 0.85, 7.0, 0.16);
        shade += step(0.55, dark) * hatch(frag, -0.75, 6.0, 0.14);
      } else if (uHatch < 1.5) {
        // Chalk: broad grainy strokes that thin out with light.
        float s = hatch(frag, 0.6, 9.0, 0.35) * smoothstep(0.2, 0.8, dark);
        shade = s * step(0.45, noise(frag * 0.35 + uSeed * 5.0));
      } else {
        // Pencil: soft tone made of fine strokes plus graphite smudge.
        shade += smoothstep(0.15, 0.9, dark) * (0.35 + 0.65 * hatch(frag, 1.05, 4.0, 0.3)) * 0.6;
      }
      vec2 ta = wob(vUv, 60.0, 0.6 * uWobble);
      float text = texture2D(tText, vec2(ta.x, ta.y)).a;
      float grain = (noise(frag * 0.9) - 0.5) * uGrain + (noise(frag * 0.08) - 0.5) * uGrain;
      float amount = clamp(max(max(ink, shade * 0.85), text), 0.0, 1.0);
      if (uHatch > 0.5 && uHatch < 1.5) amount *= 0.75 + 0.25 * noise(frag * 0.6 + uSeed * 3.0);
      vec3 col = mix(uBg, uInk, amount) + grain;
      gl_FragColor = vec4(col, 1.0);
    }`,
  depthTest: false,
})
const postScene = new THREE.Scene()
postScene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), post))
const postCam = new THREE.OrthographicCamera()

const ease = (x) => (x <= 0 ? 0 : x >= 1 ? 1 : x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2)
const span = (t, a, b) => ease((t - a) / (b - a))

function drawText(t) {
  tctx.clearRect(0, 0, W, H)
  tctx.fillStyle = '#000'
  tctx.textAlign = 'center'
  const k = H / 1080
  const line = (str, size, y, alpha, weight = 500) => {
    if (alpha <= 0) return
    tctx.globalAlpha = alpha
    tctx.font = `${weight} ${size * k}px "Ubuntu", "Noto Sans", sans-serif`
    tctx.fillText(str, W / 2, y * k)
  }
  // A caption fades in at a, holds, and fades out by b.
  const cap = (str, a, b, y = 140, size = 52, weight = 500) =>
    line(str, size, y, Math.min(span(t, a, a + 0.5), 1 - span(t, b - 0.4, b)), weight)
  cap('you install dozens of skills.', 0.5, 3.4)
  cap('every request carries all of them.', 3.6, 6.9)
  cap('a project uses a few.', 7.4, 11.7)
  cap('lens keeps those in full, and the rest as one line each.', 12.3, 15.6)
  cap('the skill listing gets 74% smaller.', 15.9, 18.9)
  cap('meter records what every turn costs.', 19.5, 22.8)
  cap('and what it bought: about $3.28 per commit.', 23.1, 26.8)
  cap('away for an hour? the prompt cache expires.', 27.4, 30.8)
  cap("keeper keeps it warm, so you don't pay twice.", 31.1, 34.8)
  cap('idle compaction (opt-in): 583K tokens come back as 21K.', 35.4, 40.7)
  cap('measured on one machine. yours will differ.', 16.3, 18.9, 1010, 30, 400)
  cap('measured on one machine. yours will differ.', 23.5, 26.8, 1010, 30, 400)
  const out = 1 - span(t, 46.0, 46.5)
  line('girth', 170, 230, span(t, 41.6, 42.3) * out, 700)
  line('local. measured. open source.', 46, 880, span(t, 42.6, 43.2) * out)
  line('/plugin marketplace add mohessaid/girth', 36, 960, span(t, 43.4, 44.0) * out, 400)
  cap("star it on github, so it's there when you need it.", 46.6, 49.0)
  cap("know someone who'd use it? share it with them.", 49.4, 52.4)
  line('github.com/mohessaid/girth', 36, 960, Math.min(span(t, 46.8, 47.3), 1 - span(t, 52.0, 52.4)), 400)
  line('p.s. this video was made with girth.', 52, 160, span(t, 52.8, 53.4))
  line('it remembered the taste: chalk, calm lines, room to breathe.', 34, 950, span(t, 54.0, 54.6), 400)
  textTex.needsUpdate = true
}

const v3 = (x, y, z) => new THREE.Vector3(x, y, z)
const show = (m, s) => {
  m.visible = s > 0.002
  m.scale.setScalar(Math.max(0.0001, s))
}

function layout(t) {
  const ang = 0.6 + t * 0.06
  const dist = 13
  camera.position.set(Math.sin(ang) * dist, 3.6, Math.cos(ang) * dist)
  camera.lookAt(0, 0.4, 0)
  // Left and right as the camera sees them, so side-by-side groups never overlap.
  const right = v3(Math.cos(ang), 0, -Math.sin(ang))

  // Cards: settle, scatter, return as one-line slivers beside the kept four, leave.
  const settle = span(t, 0, 1.2)
  const scatter = span(t, 7.2, 9.0)
  const gather = span(t, 7.6, 9.6)
  const lens = span(t, 12.2, 13.8)
  const leave = span(t, 18.8, 19.8)
  const finale = span(t, 40.8, 42.0)
  for (const m of cards) {
    const u = m.userData
    const stack = v3(0, u.y * (1 + (1 - settle) * 0.6), 0)
    if (u.kept) {
      const slot = right.clone().multiplyScalar(-1.4 * lens).add(v3(0, (u.slot - 1.5) * 0.36 + 0.2 * lens, 0))
      m.position.lerpVectors(stack, slot, gather)
      m.rotation.set(0, u.twist * (1 - gather), 0)
      const out = Math.min(leave, 1 - finale)
      const s = (1 + gather * 0.25) * (1 - (t < 40 ? leave : 1 - finale)) * (1 - span(t, 45.8, 46.4))
      show(m, s)
      if (t >= 40.8) m.position.set(0, (u.slot - 1.5) * 0.36 - 0.2, 0)
      void out
    } else {
      const sliver = right.clone().multiplyScalar(1.4).add(v3(0, (u.line - 15.5) * 0.1 + 0.2, 0))
      if (t < 12.2) {
        m.position.lerpVectors(stack, u.out, scatter)
        m.rotation.set(u.spin * scatter, u.twist + u.spin * scatter * 0.5, u.spin * scatter * 0.3)
        show(m, 1 - scatter)
      } else {
        m.position.lerpVectors(u.out, sliver, lens)
        m.rotation.set(u.spin * (1 - lens), u.spin * (1 - lens) * 0.5, 0)
        show(m, lens * (1 - leave))
        m.scale.set(m.scale.x * 0.75, m.scale.y * 0.3, m.scale.z * 0.45)
      }
    }
  }

  // Meter bars grow, then sink.
  const grow = span(t, 19.4, 21.4)
  const sink = span(t, 26.6, 27.4)
  bars.forEach((m, i) => {
    const g = span(t, 19.4 + i * 0.12, 20.6 + i * 0.12) * (1 - sink)
    const h = Math.max(0.0001, m.userData.h * g)
    m.visible = g > 0.002
    m.scale.set(1, h, 1)
    m.position.set(m.userData.x, h / 2 - 1.2, 0)
  })
  void grow

  // Keeper: the clock appears, its hand sweeps an hour; it leaves before compaction.
  const clock = Math.min(span(t, 27.2, 28.2), 1 - span(t, 34.8, 35.4))
  for (const m of [ring, hand, pivot]) {
    show(m, clock)
    m.position.set(0, 0.4, 0)
    m.lookAt(camera.position.x, 0.4, camera.position.z)
  }
  hand.rotateZ(-span(t, 28, 34.4) * Math.PI * 2)

  // The star spins in, and turns slowly while the call to action shows.
  const st = Math.min(span(t, 46.4, 47.2), 1 - span(t, 52.1, 52.5))
  show(star, st * 0.9)
  star.position.set(0, 0.5, 0)
  star.lookAt(camera.position.x, 0.5, camera.position.z)
  star.rotateY((1 - span(t, 46.4, 47.6)) * Math.PI * 2 + Math.sin(t * 1.2) * 0.25)

  // The wink: the face turns to the camera, and one eye closes.
  const f = span(t, 52.6, 53.4)
  face.visible = f > 0.002
  face.scale.setScalar(Math.max(0.0001, f * 1.4))
  face.position.set(0, 0.2, 0)
  face.lookAt(camera.position.x, 0.2, camera.position.z)
  const wink = Math.min(span(t, 55.2, 55.45), 1 - span(t, 56.3, 56.55))
  eyeR.scale.set(1.3, 1 - wink * 0.85, 1)

  // Compaction: a tall block pressed into a small one.
  const press = span(t, 36.6, 39.4)
  const block_in = Math.min(span(t, 35.2, 36.0), 1 - span(t, 40.4, 40.9))
  block.visible = block_in > 0.002
  const side = (2.4 - press * 1.6) * block_in
  const height = (3.2 - press * 2.6) * block_in
  block.scale.set(Math.max(0.0001, side), Math.max(0.0001, height), Math.max(0.0001, side))
  block.position.set(0, height / 2 - 1.2, 0)
  block.rotation.set(0, 0.5 + t * 0.1, 0)
}

window.renderFrame = (n) => {
  const t = n / FPS
  layout(t)
  drawText(t)
  // Lines redraw 8 times a second: alive, but calm.
  post.uniforms.uSeed.value = Math.floor(t * 8) * 0.137
  renderer.setRenderTarget(target)
  renderer.setClearColor(0x000000, 0)
  renderer.clear()
  renderer.render(scene, camera)
  renderer.setRenderTarget(null)
  renderer.render(postScene, postCam)
  return true
}
document.fonts.ready.then(() => {
  window.renderFrame(0)
  window.ready = true
})
