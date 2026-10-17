# The Girth intro film: 75 s, chalk on black, drawn by Line Art. See ../STORYBOARD.md.
#   blender -b -P film.py -- --out DIR [--frames 1,120,...] [--save film.blend]
# Without --frames it renders the whole film as a PNG sequence into DIR (resumable).
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bpy  # noqa: E402
from mathutils import Euler, Vector  # noqa: E402

from lib import (FPS, F, N, at, retime, bake, box, clamp, ease_in, ease_io, ease_out, fade, lerp,  # noqa: E402
                 fcurves, line_art, mesh_from, prog, setup, solid, text, veil, visible, windows)

LENGTH = 75.0
STATIONS = []
s, cam, drawn, _ = setup(LENGTH)
# The director's edit (see STORYBOARD.md): each station's authored span -> its span in the final cut.
retime([(0, 0), (7, 7.5), (14, 14.5), (18, 19.5), (24.5, 25.5), (28, 29), (32, 33), (38, 38.5),
        (44, 42.3), (50, 49), (53, 52.4), (56, 55.4), (59, 58.2), (64, 63.5), (70, 69.5), (75, 75)])
# Fast moves strobe on thin chalk lines; a short shutter smears what motion is left.
s.render.use_motion_blur = True
s.render.motion_blur_shutter = 0.3
s.eevee.taa_render_samples = 12
line_art(drawn)
VEIL = veil(cam)

SKILLS = sorted(n.rstrip('/') for n in os.listdir(os.path.expanduser('~/.claude/skills')))
SOURCES = ['explainer-beat-sheet', 'faceless-explainer', 'hyperframes-animation', 'product-launch-video']
HEADINGS = {
    'explainer-beat-sheet': '## each station',
    'faceless-explainer': '## transitions',
    'hyperframes-animation': '## scene transitions',
    'product-launch-video': '## cut the curve',
}


def root(name, origin):
    e = bpy.data.objects.new(name, None)
    s.collection.objects.link(e)
    e.location = origin
    return e


def bump(k):
    """0 → 1 → 0 over k in [0, 1]."""
    return math.sin(math.pi * clamp(k)) if 0 < k < 1 else 0.0


def oi(k):
    """Fast, slow, fast: a shot that enters and leaves on a cut."""
    k = clamp(k)
    return 0.5 * ease_out(2 * k, 2) if k < 0.5 else 0.5 + 0.5 * ease_in(2 * k - 1, 2)


# --- captions (parented to the camera, 1 unit in front of the lens) ----------

TITLE_Y, VERDICT_Y, FOOT_Y = -0.255, -0.315, -0.352


def caption(body, t_in, t_out, y=TITLE_Y, size=0.052, slide=1, slam=False, alpha=1.0, mono=False, x=0.0):
    """Text that enters with a short slide in the camera's direction; the cut is its exit."""
    o = text('cap', body, size=size, loc=(x, y, -1), parent=cam, alpha=alpha, mono=mono)
    if slam:
        bake(o, 'scale', t_in, t_in + 0.25, lambda t: lerp(1.35, 1.0, ease_out(prog(t, t_in, t_in + 0.22))), index=0)
        bake(o, 'scale', t_in, t_in + 0.25, lambda t: lerp(1.35, 1.0, ease_out(prog(t, t_in, t_in + 0.22))), index=1)
        fade(o, t_in, t_out - 0.2, d_in=0.12, d_out=0.2, peak=alpha)
    else:
        bake(o, 'location', t_in, t_in + 0.5, lambda t: x + slide * 0.03 * (1 - ease_out(prog(t, t_in, t_in + 0.5))), index=0)
        fade(o, t_in, t_out - 0.22, d_in=0.4, d_out=0.22, peak=alpha)
    return o


def dip(t0, t1, to_black=True):
    a, b = (0.0, 1.0) if to_black else (1.0, 0.0)
    bake(VEIL, 'color', t0, t1, lambda t: lerp(a, b, ease_io(prog(t, t0, t1), 2)), index=3)


# --- camera -----------------------------------------------------------------

SHOTS = []


def shot(t0, t1, path):
    """path(t) -> (location, target) for t in [t0, t1)."""
    SHOTS.append((t0, t1, path))


def move(a, b, k):
    return tuple(lerp(a[i], b[i], k) for i in range(3))


def bake_camera():
    prev = None
    for f in range(1, F(LENGTH)):
        t = N((f - 1) / FPS)
        for t0, t1, path in SHOTS:
            if t0 <= t < t1:
                loc, tgt = path(t)
                break
        else:
            continue
        cam.location = loc
        rot = (Vector(tgt) - Vector(loc)).to_track_quat('-Z', 'Y').to_euler('XYZ', prev) if prev else \
            (Vector(tgt) - Vector(loc)).to_track_quat('-Z', 'Y').to_euler('XYZ')
        cam.rotation_euler = rot
        prev = Euler(rot)
        cam.keyframe_insert('location', frame=f)
        cam.keyframe_insert('rotation_euler', frame=f)


# --- reusable props -----------------------------------------------------------

def book(name, x, w, h, label, parent, depth=1.0, size=0.2):
    b = box(name, (w, depth, h), loc=(x + w / 2, 0, 0), parent=parent, pivot=(0, 0, -1))
    text(name + '-label', label, size=size, loc=(0, -depth / 2 - 0.01, 0.14),
         rot=(math.radians(90), math.radians(-90), 0), align='LEFT', parent=b)
    return b


def shelf(name, origin, names, seed=0):
    r = root(name, origin)
    x, books = 0.0, {}
    for i, n in enumerate(names):
        w = 0.34 + 0.08 * ((i * 5 + seed) % 3)
        h = 1.5 + 0.18 * ((i * 7 + seed) % 4)
        books[n] = book(f'{name}-{i}', x, w, h, n, r)
        x += w + 0.03
    box(name + '-plank', (x + 0.6, 1.3, 0.08), loc=(x / 2, 0, -0.04), parent=r)
    return r, books, x


def card(name, parent, loc=(0, 0, 0)):
    """The crafted skill: one small card with its real name and size."""
    c = box(name, (0.9, 0.05, 1.2), loc=loc, parent=parent)
    text(name + '-t1', 'girth-video', size=0.17, loc=(0, -0.03, 0.18), rot=(math.radians(90), 0, 0), parent=c)
    text(name + '-t2', 'SKILL.md · 3 KB', size=0.1, loc=(0, -0.03, -0.12), rot=(math.radians(90), 0, 0), parent=c, mono=True)
    for i in range(3):
        box(f'{name}-line{i}', (0.6 - 0.12 * i, 0.06, 0.012), loc=(0, -0.005, -0.3 - 0.09 * i), parent=c)
    return c


BOX_H = 1.45
CARD_REST = 0.05 + 0.6 + 0.03     # card centre when it stands on the box floor
CARD_CLEAR = BOX_H + 0.6 + 0.15   # card centre that clears the walls


def folder(name, parent, loc, tag):
    f = root(name, (0, 0, 0))
    f.parent = parent
    f.location = loc
    # Taller than the card (1.2) so the lid closes over it with room to spare.
    w, d, h = 1.6, 1.1, BOX_H
    box(name + '-bottom', (w, d, 0.05), loc=(0, 0, 0.025), parent=f)
    box(name + '-back', (w, 0.05, h), loc=(0, d / 2, h / 2), parent=f)
    box(name + '-front', (w, 0.05, h * 0.75), loc=(0, -d / 2, h * 0.375), parent=f)
    box(name + '-left', (0.05, d, h), loc=(-w / 2, 0, h / 2), parent=f)
    box(name + '-right', (0.05, d, h), loc=(w / 2, 0, h / 2), parent=f)
    lid = box(name + '-lid', (w + 0.06, d + 0.06, 0.05), loc=(0, d / 2, h), parent=f, pivot=(0, 1, 0))
    text(name + '-tag', tag, size=0.17, loc=(0, -d / 2 - 0.04, h * 0.37), rot=(math.radians(90), 0, 0), parent=f, mono=True)
    return f, lid


# =============================================================================
# 1 · Hook (0–7): the shelf, then every book lifts on every turn
# =============================================================================
S1 = (0, 0, 0)
shelf1, books1, shelf_len = shelf('shelf', S1, SKILLS)
STATIONS.append((shelf1, [(0, 7.0), (18.0, 24.5)]))

def glide(t):
    return 0.7 * prog(t, 0, 3.5) + 0.3 * ease_out(prog(t, 0, 3.5), 2)


shot(0.0, 3.5, lambda t: (move((1.8, -9.6, 2.3), (4.0, -9.6, 2.3), glide(t)),
                          move((3.8, 0, 1.45), (6.0, 0, 1.45), glide(t))))


def s1b(t):
    k = prog(t, 3.5, 7.0)
    side = 0.6 * prog(t, 3.5, 6.3) + 0.4 * ease_out(prog(t, 3.5, 6.3), 2)   # enters at shot 1's speed
    push = ease_in(prog(t, 6.3, 7.0), 3)
    loc = move((8.4, -8.6, 1.5), (9.6, -8.6, 1.5), side)
    loc = (loc[0], loc[1] + 1.6 * push, loc[2])
    return loc, (loc[0] + 1.4 + 0.2 * k, 0, 1.6)


shot(3.5, 7.0, s1b)

caption('you installed dozens of skills.', N(0.5), N(3.3))
prompt = '› make a title card'
for i in range(1, len(prompt) + 1):
    o = text('prompt', prompt[:i], size=0.04, loc=(-0.42, 0.14, -1), parent=cam, align='LEFT', mono=True)
    t_on = 3.7 + i * 0.035
    t_off = 3.7 + (i + 1) * 0.035 if i < len(prompt) else 6.8
    visible(o, t_on, t_off)
caption('your AI carries all of them.', N(3.8), N(7.3))
caption('every turn.', N(6.3), N(7.3), y=VERDICT_Y, size=0.07, slam=True)

for i, n in enumerate(SKILLS):
    b = books1[n]
    def lift(t, i=i):
        return 0.28 * (bump((t - 4.75 - i * 0.018) / 0.42) + bump((t - 6.0 - i * 0.018) / 0.42))
    bake(b, 'location', 4.6, 6.9, lift, index=2)

# =============================================================================
# 2 · The weight (7–14): one manual unfolds, then does it again next session
# =============================================================================
S2 = Vector((100, 0, 0))
r2 = root('s2', S2)
STATIONS.append((r2, [(6.9, 14.1)]))
PAGES, PW, PH = 26, 0.9, 1.9
pages = []
for i in range(PAGES):
    p = box(f'page{i}', (PW, 0.025, PH), parent=r2, pivot=(-1, 0, -1))
    for j in range(3):
        box(f'page{i}-l{j}', (PW * (0.7 - 0.12 * j), 0.03, 0.012), loc=(PW * 0.45, -0.005, PH * (0.75 - 0.08 * j)), parent=p)
    pages.append(p)
text('cover', 'hyperframes', size=0.2, loc=(PW / 2, -0.03, PH * 0.55), rot=(math.radians(90), 0, 0), parent=pages[0])


def fold_angle(t):
    """90° folded, 22° open. Opens at 7.6, snaps shut at 10.7, opens again at 11.5."""
    a = lerp(90, 22, ease_io(prog(t, 7.6, 9.8), 3))
    a = lerp(a, 90, ease_in(prog(t, 10.6, 11.15), 2))
    a = lerp(a, 22, ease_out(prog(t, 11.5, 12.9), 3))
    return math.radians(a)


def page_pose(i, t):
    a = fold_angle(t)
    x, y = 0.0, 0.0
    for j in range(i):
        sign = 1 if j % 2 == 0 else -1
        x += PW * math.cos(a) + 0.02
        y += sign * PW * math.sin(a)
    sign = 1 if i % 2 == 0 else -1
    return (x, y), sign * a


for i, p in enumerate(pages):
    bake(p, 'location', 7.0, 14.0, lambda t, i=i: page_pose(i, t)[0][0], index=0)
    bake(p, 'location', 7.0, 14.0, lambda t, i=i: page_pose(i, t)[0][1], index=1)
    bake(p, 'rotation_euler', 7.0, 14.0, lambda t, i=i: page_pose(i, t)[1], index=2)


def s2a(t):
    k = prog(t, 7.0, 10.5)
    fwd = ease_out(prog(t, 7.0, 8.4), 3)
    pan = ease_io(prog(t, 8.0, 10.5), 2)
    loc = (S2.x - 0.6 + 3.0 * pan, -8.6 + 2.2 * fwd - 0.8 * pan, 1.5 + 0.6 * pan)   # pull back, not whip along
    return loc, (S2.x + 0.9 + 4.2 * pan, 0, 1.45 + 0.1 * k)


def s2b(t):
    fwd = ease_out(prog(t, 10.5, 12.0), 3)
    loc = (S2.x - 4.5 + 1.4 * fwd, -7.0 + 1.8 * fwd, 0.8)
    return loc, (S2.x + 4.0, 0.3, 1.55)


shot(7.0, 10.5, s2a)
shot(10.5, 14.0, s2b)
caption('the video skills alone:', N(8.0), N(11.0))
caption('1.7 MB of manuals.', N(9.6), N(11.0), y=VERDICT_Y, size=0.07, slam=True)
tag = text('next-session', 'next session', size=0.24, loc=(1.6, -0.6, 2.25), rot=(math.radians(90), 0, math.radians(-30)), parent=r2, mono=True)
visible(tag, 10.5, 14.0)
caption('next session, it reads them again.', N(11.3), N(14.3))
caption('and again.', N(13.2), N(14.3), y=VERDICT_Y, size=0.07, slam=True)
dip(13.5, 14.0, to_black=True)

# =============================================================================
# 3 · The turn (14–18): girth, written by hand
# =============================================================================
S3 = Vector((200, 0, 0))
r3 = root('s3', S3)
STATIONS.append((r3, [(13.9, 18.1)]))
word = text('girth', 'girth', size=1.7, loc=(0, 0, 1.55), rot=(math.radians(90), 0, 0), parent=r3)
# The write-on: a black card slides off the word from left to right.
mask = box('girth-mask', (3.4, 0.02, 1.6), loc=(0, -0.25, 1.65), parent=r3, mat=None, pivot=(-1, 0, 0))
drawn.objects.unlink(mask)
mask.lineart.usage = 'EXCLUDE'
bpy.data.collections['Text'].objects.link(mask)
bake(mask, 'location', 14.3, 15.6, lambda t: lerp(-1.75, 1.75, ease_io(prog(t, 14.35, 15.5), 2)), index=0)
underline = box('underline', (2.6, 0.04, 0.03), loc=(-1.3, -0.1, 0.62), parent=r3, pivot=(-1, 0, 0))
windows(underline, [(15.5, 18.1)])
bake(underline, 'scale', 15.5, 16.1, lambda t: max(0.001, ease_out(prog(t, 15.55, 16.05), 3)), index=0)
dip(14.0, 14.5, to_black=False)


def s3(t):
    push = 0.6 * prog(t, 14.0, 17.45) + 0.4 * ease_out(prog(t, 14.0, 17.45), 2)
    leave = ease_in(prog(t, 17.45, 18.0), 3)
    loc = (S3.x - 2.2 * leave, -7.2 + 1.1 * push, 1.25)
    return loc, (S3.x - 2.2 * leave, 0, 1.15)


shot(14.0, 18.0, s3)
caption('read once.', N(16.0), N(19.5), y=TITLE_Y + 0.01, size=0.07, slam=True)
caption('keep only the slice you need.', N(16.5), N(19.5), y=VERDICT_Y + 0.005, slide=-1)

# =============================================================================
# 4 · Search (18–24.5): a scan line passes the shelf; four sources step out
# =============================================================================
xs = {n: books1[n].location.x + books1[n].dimensions.x / 2 for n in SKILLS}
scan = box('scan', (0.03, 1.7, 2.8), loc=(0, -0.3, 1.4), parent=shelf1)
windows(scan, [(18.0, 21.0)])
bake(scan, 'location', 18.0, 21.0, lambda t: lerp(shelf_len + 0.5, -0.5, ease_io(prog(t, 18.2, 20.7), 2)), index=0)


def scan_x(t):
    return lerp(shelf_len + 0.5, -0.5, ease_io(prog(t, 18.2, 20.7), 2))


def passes(n):
    """When the scan line reaches book n."""
    lo, hi = 18.2, 20.7
    for f in range(F(lo), F(hi) + 1):
        if scan_x((f - 1) / FPS) <= xs[n]:
            return (f - 1) / FPS
    return hi


for n in SOURCES:
    tp = passes(n)
    bake(books1[n], 'location', 18.0, 24.5, lambda t, tp=tp: -0.9 * ease_out(prog(t, tp, tp + 0.4), 3), index=1)

lift_pages = []
MID = sum(xs[n] for n in SOURCES) / 4
for n in SOURCES:
    pg = box('lift-' + n, (1.5, 0.02, 1.25), loc=(xs[n], -0.95, 1.0), parent=shelf1)
    text('lift-' + n + '-h', HEADINGS[n], size=0.2, loc=(0, -0.02, 0.3), rot=(math.radians(90), 0, 0), parent=pg, mono=True)
    for j in range(3):
        box(f'lift-{n}-l{j}', (1.05 - 0.15 * j, 0.03, 0.015), loc=(0, -0.01, 0.0 - 0.14 * j), parent=pg)
    windows(pg, [(21.0, 24.5)])
    k0 = 21.4 + 0.18 * SOURCES.index(n)
    bake(pg, 'location', 21.0, 24.5, lambda t, k0=k0: lerp(1.0, 2.9, ease_out(prog(t, k0, k0 + 0.8), 3)), index=2)
    spread = (SOURCES.index(n) - 1.5) * 1.85
    bake(pg, 'location', 21.0, 24.5, lambda t, n=n, k0=k0, spread=spread: lerp(xs[n], MID + spread, ease_io(prog(t, k0, k0 + 0.8), 3)) + 4.0 * ease_in(prog(t, 23.95, 24.5), 3), index=0)
    lift_pages.append(pg)


def s4a(t):
    k = oi(prog(t, 18.0, 21.0))
    loc = (lerp(shelf_len * 0.62, shelf_len * 0.48, k), -11.5, 2.6)   # the scan line crosses; the camera barely drifts
    return loc, (loc[0] - 1.0, 0, 1.0)


mid = sum(xs[n] for n in SOURCES) / 4
span = max(xs[n] for n in SOURCES) - min(xs[n] for n in SOURCES)


def s4b(t):
    settle = ease_out(prog(t, 21.0, 22.4), 2)
    loc = (mid - 0.5 + 0.5 * settle, -12.5, 2.8)
    return loc, (mid - 0.1 + 0.5 * settle, 0, 1.95)


shot(18.0, 21.0, s4a)
shot(21.0, 24.5, s4b)
caption('it searches the skills you have.', N(19.8), N(22.8), slide=-1)
caption('only the slice.', N(23.7), N(25.5), y=VERDICT_Y, size=0.07, slam=True)

# =============================================================================
# 5 · Distil (24.5–32): four pages become one small card, kept in the project
# =============================================================================
S5 = Vector((300, 0, 0))
r5 = root('s5', S5)
STATIONS.append((r5, [(24.4, 32.1)]))
sheets = []
for j, n in enumerate(SOURCES):
    pg = box('d-' + n, (0.85, 0.02, 1.0), parent=r5)
    text('d-' + n + '-h', HEADINGS[n], size=0.12, loc=(0, -0.02, 0.25), rot=(math.radians(90), 0, 0), parent=pg, mono=True)
    for q in range(3):
        box(f'd-{n}-l{q}', (0.6 - 0.1 * q, 0.03, 0.012), loc=(0, -0.01, 0.0 - 0.1 * q), parent=pg)
    fan_x = (j - 1.5) * 1.05
    def px(t, fan_x=fan_x, j=j):
        enter = lerp(fan_x - 4.0, fan_x, ease_out(prog(t, 24.5, 25.3), 3))
        return lerp(enter, j * 0.01, ease_io(prog(t, 25.7, 26.5), 3))
    bake(pg, 'location', 24.5, 27.0, px, index=0)
    bake(pg, 'location', 24.5, 27.0, lambda t, j=j: 1.4 - 0.02 * j * ease_io(prog(t, 25.7, 26.5)), index=1)
    bake(pg, 'location', 24.5, 27.0, lambda t: 1.5, index=2)
    bake(pg, 'rotation_euler', 24.5, 27.0, lambda t, j=j: math.radians((j - 1.5) * 4) * (1 - ease_io(prog(t, 25.7, 26.5))), index=1)
    for ax in range(3):
        bake(pg, 'scale', 26.4, 26.9, lambda t: lerp(1.0, 0.001, ease_in(prog(t, 26.5, 26.85), 2)), index=ax)
    sheets.append(pg)
    windows(pg, [(24.4, 26.9)])
card5 = card('card5', r5, loc=(0, 1.4, 1.5))
for ax in range(3):
    bake(card5, 'scale', 26.5, 27.2, lambda t: max(0.001, ease_out(prog(t, 26.55, 27.1), 3)), index=ax)
windows(card5, [(26.5, 32.1)])
folder5, lid5 = folder('folder5', r5, (2.4, 1.4, 0), 'your-project/')
windows(folder5, [(28.0, 32.1)])


def card5_path(t):
    """Lift above the walls, travel over the open box, then drop straight in."""
    x = lerp(0.0, 2.4, ease_io(prog(t, 28.3, 29.1), 3))
    z = lerp(1.5, CARD_CLEAR, ease_out(prog(t, 28.3, 28.75), 2))
    z = lerp(z, CARD_REST, ease_io(prog(t, 29.1, 29.7), 3))
    return x, z


bake(card5, 'location', 28.0, 30.0, lambda t: card5_path(t)[0], index=0)
bake(card5, 'location', 28.0, 30.0, lambda t: card5_path(t)[1], index=2)
bake(lid5, 'rotation_euler', 28.0, 30.6, lambda t: math.radians(-100) * (1 - ease_io(prog(t, 29.75, 30.45), 3)), index=0)


def s5a(t):
    k = ease_out(prog(t, 24.5, 25.6), 3)
    loc = (S5.x - 1.6 + 1.6 * k, -4.6, 1.7)
    return loc, (S5.x, 1.4, 1.45)


def s5b(t):
    settle = ease_out(prog(t, 28.0, 29.0), 3)
    dive = ease_in(prog(t, 31.2, 32.0), 3)
    # Wide enough for the card's arc over the open lid (its top reaches z ≈ 2.8).
    loc = Vector((S5.x + 0.6 + 0.4 * settle, -7.0 + 0.4 * settle, 2.4))
    tgt = Vector((S5.x + 1.2, 1.2, 1.5))
    loc = loc.lerp(Vector((S5.x + 2.4, 0.4, 0.45)), dive)
    tgt = tgt.lerp(Vector((S5.x + 2.4, 1.6, 0.4)), dive)
    return tuple(loc), tuple(tgt)


shot(24.5, 28.0, s5a)
shot(28.0, 32.0, s5b)
caption('distilled into one small skill.', 24.9, 28.0)
caption('3 KB.', 26.9, 28.0, y=VERDICT_Y + 0.008, size=0.075, slam=True)
caption('four skills, 1.1 MB of docs', 25.4, 28.0, y=FOOT_Y + 0.006, size=0.034, alpha=0.75)
caption('it lives in your project.', N(29.4), N(32.4))
dip(N(32.7), N(33.0), to_black=True)
dip(N(33.0), N(33.3), to_black=False)

# =============================================================================
# 6 · Reuse (32–38): next session, it's already there
# =============================================================================
S6 = Vector((400, 0, 0))
r6 = root('s6', S6)
STATIONS.append((r6, [(31.9, 38.1)]))
folder6, lid6 = folder('folder6', r6, (0, 1.4, 0), 'your-project/')
text('ns6', 'next session', size=0.22, loc=(0, 1.4, CARD_CLEAR + 0.95), rot=(math.radians(90), 0, 0), parent=r6, mono=True)
card6 = card('card6', r6, loc=(0, 1.4, CARD_REST))
bake(lid6, 'rotation_euler', 32.0, 33.6, lambda t: math.radians(-100) * ease_io(prog(t, 32.8, 33.5), 3), index=0)
bake(card6, 'location', 32.0, 34.6, lambda t: lerp(CARD_REST, CARD_CLEAR, ease_out(prog(t, 33.4, 34.4), 3)), index=2)
bg6, bg_books, bg_len = shelf('shelf6', S6 + Vector((-5.0, 7.0, 0)), SKILLS[:16], seed=1)
STATIONS.append((bg6, [(31.9, 38.1)]))


def s6a(t):
    fwd = ease_out(prog(t, 32.0, 32.9), 3)
    loc = (S6.x, -4.4 - 4.4 * fwd, 0.9 + 1.4 * fwd)
    return loc, (S6.x, 1.4, 1.2 + 0.45 * fwd)


def s6b(t):
    k = oi(prog(t, 35.0, 38.0))
    loc = (S6.x - 3.6 + 2.2 * k, -7.0, 1.6)
    return loc, (S6.x - 0.6 + 2.2 * k, 3.0, 1.55)


shot(32.0, 35.0, s6a)
shot(35.0, 38.0, s6b)
caption("next session, it's already there.", N(33.3), N(36.2))
caption('nothing to reread.', N(36.4), N(38.5), y=VERDICT_Y, size=0.07, slam=True)

# =============================================================================
# 7 · Proof (38–50): five sessions, then two towers
# =============================================================================
S7 = Vector((500, 0, 0))
r7 = root('s7', S7)
STATIONS.append((r7, [(37.9, 50.1)]))
BULK = [0.178, 0.186, 0.172, 0.209, 0.164]
GIRTH = [0.222, 0.101, 0.090, 0.089, 0.098]
SCALE = 4.0
box('ground', (16.0, 0.6, 0.02), loc=(5.0, 0, 0), parent=r7)
bars = []
for i in range(5):
    x = i * 1.6
    for kind, vals, dx in (('bulk', BULK, -0.3), ('girth', GIRTH, 0.3)):
        h = vals[i] * SCALE
        b = box(f'{kind}{i}', (0.45, 0.45, h), loc=(x + dx, 0, 0), parent=r7, pivot=(0, 0, -1),
                mat=solid() if kind == 'girth' else None)
        bars.append((kind, i, b, h))
        t0 = 38.25 + i * 0.6
        windows(b, [(t0, 50.1)])
        bake(b, 'scale', t0 - 0.05, t0 + 0.55, lambda t, t0=t0: max(0.001, ease_out(prog(t, t0, t0 + 0.5), 3)), index=2)
        v = text(f'v-{kind}{i}', f'${vals[i]:.2f}', size=0.17, loc=(x + dx, -0.3, h + 0.16), rot=(math.radians(90), 0, 0), parent=r7)
        fade(v, t0 + 0.4, 44.0, d_in=0.3, d_out=0.2)
    lab = text(f'sess{i}', f'session {i + 1}', size=0.18, loc=(x, -0.35, -0.22), rot=(math.radians(90), 0, 0), parent=r7)
    fade(lab, 38.6 + i * 0.6, 44.0, d_in=0.3, d_out=0.2)
for kind, label, y in (('bulk', 'every skill installed', 2.0), ('girth', 'girth', 1.68)):
    sw = box('sw-' + kind, (0.2, 0.2, 0.2), loc=(-0.4, 0, y), parent=r7, mat=solid() if kind == 'girth' else None)
    lt = text('lg-' + kind, label, size=0.2, loc=(-0.15, -0.12, y - 0.02), rot=(math.radians(90), 0, 0), parent=r7, align='LEFT')
    visible(sw, 38.4, 44.0)
    fade(lt, 38.4, 43.8, d_in=0.3, d_out=0.2)
note1 = text('note1', 'crafts the slice', size=0.17, loc=(0.3, -0.3, GIRTH[0] * SCALE + 0.42), rot=(math.radians(90), 0, 0), parent=r7)
fade(note1, 40.0, 44.0, d_in=0.35, d_out=0.2)
note2 = text('note2', 'then about half', size=0.2, loc=(4.0 * 1.6 / 2 + 1.6 * 0.5 + 0.3, -0.3, 1.45), rot=(math.radians(90), 0, 0), parent=r7)
fade(note2, 41.8, 44.0, d_in=0.35, d_out=0.2)

TOWER = {'bulk': 9.0, 'girth': 10.4}
for kind in ('bulk', 'girth'):
    acc = 0.0
    for k2, i, b, h in [r for r in bars if r[0] == kind]:
        z0 = acc
        acc += h
        t0 = 44.3 + i * 0.16 + (0.08 if kind == 'girth' else 0)
        x0 = b.location.x
        bake(b, 'location', 44.2, 46.2, lambda t, x0=x0, t0=t0, kind=kind: lerp(x0, TOWER[kind], ease_io(prog(t, t0, t0 + 0.75), 3)), index=0)
        bake(b, 'location', 44.2, 46.2, lambda t, z0=z0, t0=t0: lerp(0, z0, ease_io(prog(t, t0, t0 + 0.75), 3)) + 0.5 * bump(prog(t, t0, t0 + 0.75)), index=2)
    tot = text('tot-' + kind, '$0.91' if kind == 'bulk' else '$0.60', size=0.42, loc=(TOWER[kind], -0.3, acc + 0.4),
               rot=(math.radians(90), 0, 0), parent=r7)
    fade(tot, 45.9 + (0.25 if kind == 'girth' else 0), None, d_in=0.35)
    lt2 = text('tl-' + kind, 'every skill' if kind == 'bulk' else 'girth', size=0.2, loc=(TOWER[kind], -0.35, -0.24), rot=(math.radians(90), 0, 0), parent=r7)
    fade(lt2, 45.6, None)


def s7a(t):
    settle = ease_out(prog(t, 38.0, 39.2), 3)
    drift = prog(t, 39.0, 44.0)
    leave = ease_in(prog(t, 43.6, 44.0), 3)
    loc = (S7.x + 2.2 + 0.6 * settle + 0.5 * drift + 1.4 * leave, -8.4, 2.1)
    return loc, (loc[0] + 0.4, 0, 0.75)


def s7b(t):
    settle = ease_out(prog(t, 44.0, 45.4), 3)
    leave = ease_in(prog(t, 49.5, 50.0), 3)
    loc = (S7.x + 8.6 + 0.8 * settle + 1.5 * leave, -12.2 + 0.5 * prog(t, 45.4, 50.0), 2.6)
    return loc, (loc[0] + 0.3, 0, 2.1)


shot(38.0, 44.0, s7a)
shot(44.0, 50.0, s7b)
caption('same tasks, five sessions.', 38.4, 44.0)
caption('–34%', N(45.2), N(49.0), y=VERDICT_Y + 0.02, size=0.085, slam=True)
caption('measured on one machine. yours will differ.', N(45.8), N(49.0), y=FOOT_Y + 0.006, size=0.034, alpha=0.75)

# =============================================================================
# 8 · The rest of the bill (50–59): lens, meter, keeper
# =============================================================================
S8a, S8b, S8c = Vector((600, 0, 0)), Vector((700, 0, 0)), Vector((800, 0, 0))
la_shelf, la_books, la_len = shelf('lens', S8a, SKILLS[16:28], seed=2)
STATIONS.append((la_shelf, [(49.9, 53.1)]))
KEEP = set(SKILLS[16:28][k] for k in (2, 6, 9))
for j, (n, b) in enumerate(la_books.items()):
    if n in KEEP:
        continue
    t0 = 50.9 + 0.05 * j
    bake(b, 'scale', 50.8, 52.0, lambda t, t0=t0: lerp(1.0, 0.06, ease_io(prog(t, t0, t0 + 0.45), 3)), index=2)
    for ch in b.children:
        bake(ch, 'scale', 50.8, 52.0, lambda t, t0=t0: lerp(1.0, 0.0001, ease_in(prog(t, t0, t0 + 0.2), 2)), index=0)


def lane(origin, t0, t1, z=1.4, dist=7.0, look_dz=-0.3, width=2.0):
    def path(t):
        k = oi(prog(t, t0, t1))
        x = origin.x + lerp(-width / 2, width / 2, k)
        return (x, -dist, z), (x + 0.3, 0, z + look_dz)
    return path


shot(50.0, 53.0, lane(S8a + Vector((la_len / 2, 0, 0)), 50.0, 53.0, z=1.6, dist=7.6, width=1.6))
caption('lens: unused skills become one line.', 50.3, 53.0)

r8b = root('meter', S8b)
STATIONS.append((r8b, [(52.9, 56.1)]))
box('printer', (2.0, 0.8, 0.5), loc=(0, 0, 0.25), parent=r8b)
paper = box('receipt', (1.4, 0.02, 3.0), loc=(0, -0.05, 0.5), parent=r8b, pivot=(0, 0, -1))
for j in range(7):
    box(f'rline{j}', (0.9 - 0.1 * (j % 3), 0.03, 0.012), loc=(-0.1, -0.01, 2.7 - 0.16 * j), parent=paper)
text('per-commit', '$3.28', size=0.45, loc=(0, -0.03, 1.15), rot=(math.radians(90), 0, 0), parent=paper)
text('per-commit-l', 'per commit', size=0.16, loc=(0, -0.03, 0.82), rot=(math.radians(90), 0, 0), parent=paper)
bake(paper, 'location', 53.0, 55.0, lambda t: lerp(-2.5, 0.5, ease_out(prog(t, 53.1, 54.6), 2)), index=2)
shot(53.0, 56.0, lane(S8b, 53.0, 56.0, z=2.0, dist=8.8, look_dz=-0.25, width=2.0))
caption('meter: what each commit cost.', 53.3, 56.0)

r8c = root('keeper', S8c)
STATIONS.append((r8c, [(55.9, 59.1)]))
clock = root('clock', (0, 0, 1.6))
clock.parent = r8c
ring = mesh_from('ring', lambda: bpy.ops.mesh.primitive_torus_add(major_radius=1.1, minor_radius=0.05, major_segments=64, minor_segments=8, rotation=(math.radians(90), 0, 0)), parent=clock)
for h in range(12):
    a = h * math.pi / 6
    box(f'tick{h}', (0.04, 0.04, 0.16), loc=(0.9 * math.sin(a), 0, 0.9 * math.cos(a)), rot=(0, a, 0), parent=clock)
hand = box('hand', (0.05, 0.05, 0.75), loc=(0, -0.06, 0), parent=clock, pivot=(0, 0, -1))
bake(hand, 'rotation_euler', 56.0, 58.6, lambda t: math.tau * ease_io(prog(t, 56.2, 58.4), 2), index=1)
text('hour', '1 hour', size=0.2, loc=(0, -0.05, 0.45), rot=(math.radians(90), 0, 0), parent=clock, mono=True)


def s8c(t):
    k = ease_out(prog(t, 56.0, 57.0), 3)
    up = ease_in(prog(t, 58.55, 59.0), 3)
    loc = (S8c.x + 0.4 - 0.4 * k, -6.4, 1.8 + 1.8 * up)
    return loc, (S8c.x, 0, 1.55 + 1.8 * up)


shot(56.0, 59.0, s8c)
caption('keeper: your cache stays warm.', 56.3, 59.0)

# =============================================================================
# 9 · Close (59–64)
# =============================================================================
S9 = Vector((900, 0, 0))
r9 = root('s9', S9)
STATIONS.append((r9, [(58.9, 64.1)]))
text('girth9', 'girth', size=1.5, loc=(0, 0, 1.75), rot=(math.radians(90), 0, 0), parent=r9)
mask9 = box('mask9', (3.2, 0.02, 1.5), loc=(-1.6, -0.25, 1.85), parent=r9, pivot=(-1, 0, 0))
drawn.objects.unlink(mask9)
mask9.lineart.usage = 'EXCLUDE'
bpy.data.collections['Text'].objects.link(mask9)
bake(mask9, 'location', 59.0, 60.2, lambda t: lerp(-1.6, 1.7, ease_io(prog(t, 59.05, 59.95), 2)), index=0)
tag9 = text('tag9', 'local. measured. open source.', size=0.26, loc=(0, -0.05, 0.62), rot=(math.radians(90), 0, 0), parent=r9)
fade(tag9, 60.6, None)
cmd9 = text('cmd9', '/plugin marketplace add mohessaid/girth', size=0.17, loc=(0, -0.05, 0.12), rot=(math.radians(90), 0, 0), parent=r9, mono=True)
fade(cmd9, 61.4, None)


def s9(t):
    k = ease_out(prog(t, 59.0, 60.0), 3)
    up = ease_in(prog(t, 63.55, 64.0), 3)
    loc = (S9.x, -6.2 + 0.6 * prog(t, 60.0, 63.55), 1.2 - 1.6 * (1 - k) + 1.6 * up)
    return loc, (S9.x, 0, 1.1 - 1.6 * (1 - k) + 1.6 * up)


shot(59.0, 64.0, s9)

# =============================================================================
# 10 · Call to action (64–70): star it, share it
# =============================================================================
S10 = Vector((1000, 0, 0))
r10 = root('s10', S10)
STATIONS.append((r10, [(63.9, 70.1)]))


def prism(name, pts, depth):
    """A flat outline (x, z) extruded toward -y: icons drawn by Line Art."""
    import bmesh
    me = bpy.data.meshes.new(name)
    bm = bmesh.new()
    face = bm.faces.new([bm.verts.new((x, 0, z)) for x, z in pts])
    ext = bmesh.ops.extrude_face_region(bm, geom=[face])
    for v in ext['geom']:
        if isinstance(v, bmesh.types.BMVert):
            v.co.y -= depth
    bm.to_mesh(me)
    bm.free()
    return me


STAR = [((1.0 if i % 2 == 0 else 0.42) * math.cos(math.pi / 2 + i * math.pi / 5),
         (1.0 if i % 2 == 0 else 0.42) * math.sin(math.pi / 2 + i * math.pi / 5)) for i in range(10)]
ARROW = [(0, 0), (0, -0.62), (0.15, -0.48), (0.27, -0.74), (0.37, -0.69), (0.25, -0.44), (0.45, -0.42)]


def icon(name, pts, depth, loc, scale, mat):
    o = bpy.data.objects.new(name, prism(name, pts, depth))
    drawn.objects.link(o)
    o.data.materials.append(mat)
    o.parent = r10
    o.location = loc
    o.scale = (scale, scale, scale)
    return o


# A chalk sketch of the repo's GitHub header: the address, the repo name, and its Star button.
T10 = 64.0
CLICK = T10 + 1.3
box('repo-panel', (5.6, 0.05, 2.1), loc=(0, 0.1, 1.75), parent=r10)
text('repo-url', 'github.com/mohessaid/girth', size=0.17, loc=(-2.55, 0.0, 2.48), align='LEFT', rot=(math.radians(90), 0, 0), parent=r10, mono=True)
box('repo-rule', (5.2, 0.03, 0.012), loc=(0, 0.06, 2.3), parent=r10)
text('repo-name', 'mohessaid / girth', size=0.36, loc=(-2.55, 0.0, 1.62), align='LEFT', rot=(math.radians(90), 0, 0), parent=r10)
text('repo-about', 'Just-in-time, project-scoped agent skills.', size=0.16, loc=(-2.55, 0.0, 1.2), align='LEFT', rot=(math.radians(90), 0, 0), parent=r10)
BTN = Vector((1.65, -0.05, 1.72))
box('star-button', (1.5, 0.12, 0.5), loc=tuple(BTN), parent=r10)
icon_at = (BTN.x - 0.45, BTN.y - 0.07, BTN.z)
icon('star-empty', STAR, 0.04, icon_at, 0.15, bpy.data.materials['ink-black'])
full = icon('star-full', STAR, 0.05, (icon_at[0], icon_at[1] - 0.01, icon_at[2]), 0.15, solid())
windows(full, [(CLICK + 0.05, 70.1)])
for ax in range(3):
    bake(full, 'scale', CLICK, CLICK + 0.6, lambda t: 0.15 * (1 + 0.6 * bump(prog(t, CLICK + 0.05, CLICK + 0.5))), index=ax)
lbl = text('star-label', 'Star', size=0.24, loc=(BTN.x + 0.15, BTN.y - 0.07, BTN.z - 0.08), rot=(math.radians(90), 0, 0), parent=r10)
visible(lbl, T10, CLICK + 0.05)
lbl2 = text('star-label2', 'Starred', size=0.24, loc=(BTN.x + 0.15, BTN.y - 0.07, BTN.z - 0.08), rot=(math.radians(90), 0, 0), parent=r10)
visible(lbl2, CLICK + 0.05, 70.1)

cursor = icon('cursor', ARROW, 0.04, (0, 0, 0), 0.55, solid())
TIP = Vector((icon_at[0] + 0.05, icon_at[1] - 0.12, icon_at[2] - 0.05))
START = Vector((3.4, -0.3, 0.2))


def cursor_at(t):
    k = ease_io(prog(t, T10 + 0.3, CLICK - 0.15), 3)
    return START.lerp(TIP, k) + Vector((0, 0, 0.25 * bump(k)))


for ax in range(3):
    bake(cursor, 'location', T10, 70.0, lambda t, ax=ax: cursor_at(t)[ax], index=ax)
    bake(cursor, 'scale', T10, 70.0, lambda t: 0.55 * (1 - 0.18 * bump(prog(t, CLICK - 0.08, CLICK + 0.18))), index=ax)
windows(cursor, [(T10 + 0.2, 70.1)])


def s10(t):
    k = ease_out(prog(t, T10, T10 + 1.2), 3)
    push = ease_io(prog(t, CLICK - 0.6, CLICK + 0.4), 2)   # lean into the click
    back = ease_io(prog(t, 67.0, 69.6), 2)                 # then give the share line room
    d = -7.4 + 0.6 * k + 0.5 * push * (1 - back) - 0.3 * back
    x = S10.x - 0.22 + 0.12 * push * (1 - back)            # panel centred; lean only slightly right
    return (x, d, 1.55 - 1.0 * (1 - k)), (x, 0, 1.6 - 1.0 * (1 - k))


shot(64.0, 70.0, s10)
caption('star it on GitHub,', 64.5, CLICK + 1.4)
caption("so it's there later.", CLICK + 0.15, CLICK + 1.4, y=VERDICT_Y, size=0.07, slam=True)
caption('know someone paying for this? share it.', 67.1, 70.1)

# =============================================================================
# 11 · The wink (70–75)
# =============================================================================
S11 = Vector((1100, 0, 0))
r11 = root('s11', S11)
STATIONS.append((r11, [(69.9, 75.1)]))
card11 = card('card11', r11, loc=(-0.9, 0, 1.55))
bake(card11, 'rotation_euler', 70.0, 71.2, lambda t: math.tau * 1.5 * (1 - ease_out(prog(t, 70.0, 71.1), 3)), index=2)
face = root('face', (1.0, 0, 1.6))
face.parent = r11
eyes = []
for dx in (-0.24, 0.24):
    e = mesh_from('eye', lambda dx=dx: bpy.ops.mesh.primitive_uv_sphere_add(radius=0.08, segments=16, ring_count=8, location=(dx, 0, 0.2)), parent=face)
    eyes.append(e)
smile = mesh_from('smile', lambda: bpy.ops.mesh.primitive_torus_add(major_radius=0.36, minor_radius=0.035, major_segments=48, minor_segments=6, rotation=(math.radians(90), 0, 0)), parent=face)
import bmesh  # noqa: E402
bm = bmesh.new()
bm.from_mesh(smile.data)
bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.co.y > -0.12], context='VERTS')
bm.to_mesh(smile.data)
bm.free()
smile.location = (0, 0, 0.05)
windows(face, [(72.45, 75.1)])
for ax in range(3):
    bake(face, 'scale', 72.4, 73.0, lambda t: max(0.001, ease_out(prog(t, 72.45, 72.95), 3)), index=ax)
bake(eyes[1], 'scale', 73.2, 73.9, lambda t: 1.0 - 0.88 * bump(prog(t, 73.3, 73.75)), index=2)


def s11(t):
    k = ease_out(prog(t, 70.0, 71.0), 3)
    loc = (S11.x + 0.05, -5.2 - 0.4 * k, 1.55)
    return loc, (S11.x + 0.05, 0, 1.5)


shot(70.0, 75.0, s11)
caption('p.s. this video was made with girth.', 70.6, 74.6)
caption('it remembered: chalk, calm lines, room to breathe.', 71.3, 74.6, y=VERDICT_Y, size=0.042)
dip(74.2, 74.95, to_black=True)

bake_camera()
for r, spans in STATIONS:
    windows(r, spans, keep=True)

# --- output ------------------------------------------------------------------
args = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
opt = {args[i].lstrip('-'): args[i + 1] for i in range(0, len(args) - 1, 2)}
out = opt.get('out', os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'frames'))
os.makedirs(out, exist_ok=True)
if 'save' in opt:
    bpy.ops.wm.save_as_mainfile(filepath=os.path.abspath(opt['save']))
if 'frames' in opt:
    for f in [int(x) for x in opt['frames'].split(',')]:
        s.frame_set(f)
        s.render.filepath = os.path.join(out, f'{f:05d}.png')
        bpy.ops.render.render(write_still=True)
elif 'render' in opt:
    s.render.filepath = os.path.join(out, '')
    s.render.use_overwrite = False
    s.render.use_placeholder = True
    s.frame_start = int(opt.get('start', s.frame_start))
    s.frame_end = int(opt.get('end', s.frame_end))
    bpy.ops.render.render(animation=True)
