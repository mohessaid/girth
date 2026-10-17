# Helpers for the chalk film: scene setup, Line Art, chalk text, and baked animation.
# Everything is keyed frame by frame from plain Python functions, so the film is
# deterministic and every move is easy to read in one place.
import math
import os

import bpy

FPS = 24
W, H = 1920, 1080
HERE = os.path.dirname(os.path.abspath(__file__))
FONT = os.path.join(HERE, '..', 'fonts', 'Caveat.ttf')


def F(t):
    """Seconds to a frame number (frame 1 is t = 0)."""
    return int(round(t * FPS)) + 1


def clamp(x, a=0.0, b=1.0):
    return max(a, min(b, x))


def lerp(a, b, k):
    return a + (b - a) * k


def ease_out(k, p=3):
    k = clamp(k)
    return 1 - (1 - k) ** p


def ease_in(k, p=3):
    k = clamp(k)
    return k ** p


def ease_io(k, p=3):
    k = clamp(k)
    return 2 ** (p - 1) * k ** p if k < 0.5 else 1 - (-2 * k + 2) ** p / 2


# --- the edit ----------------------------------------------------------------
# The film is authored on the first cut's clock. EDL maps that clock onto the final
# edit, piecewise linearly, so a station can grow or shrink without rewriting it.

EDL = [(0.0, 0.0), (1e6, 1e6)]


def retime(points):
    """Set the edit: a list of (authored time, final time) breakpoints, both increasing."""
    EDL[:] = list(points)


def _map(t, src, dst):
    pts = EDL
    for (a0, b0), (a1, b1) in zip(pts, pts[1:]):
        lo, hi = (a0, a1) if src == 0 else (b0, b1)
        if t <= hi or (a1, b1) == pts[-1]:
            k = (t - lo) / (hi - lo)
            return (b0 + k * (b1 - b0)) if src == 0 else (a0 + k * (a1 - a0))
    return t


def at(t):
    """Authored time -> final time."""
    return _map(t, 0, 1)


def N(t):
    """Final time -> authored time (for beats placed on the final clock)."""
    return _map(t, 1, 0)


def prog(t, t0, t1):
    return clamp((t - t0) / (t1 - t0)) if t1 > t0 else float(t >= t1)


# --- scene -----------------------------------------------------------------

def setup(seconds):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    s = bpy.context.scene
    s.render.engine = 'BLENDER_EEVEE'
    s.eevee.taa_render_samples = 4
    s.render.resolution_x, s.render.resolution_y = W, H
    s.render.fps = FPS
    s.frame_start, s.frame_end = 1, F(seconds) - 1
    s.view_settings.view_transform = 'Standard'
    s.view_settings.look = 'None'
    world = bpy.data.worlds.new('black')
    world.color = (0, 0, 0)
    world.use_nodes = True
    world.node_tree.nodes['Background'].inputs['Color'].default_value = (0, 0, 0, 1)
    s.world = world
    s.render.image_settings.file_format = 'PNG'
    s.render.image_settings.color_mode = 'RGB'
    drawn = bpy.data.collections.new('Drawn')
    s.collection.children.link(drawn)
    text = bpy.data.collections.new('Text')
    s.collection.children.link(text)
    cam_data = bpy.data.cameras.new('cam')
    cam_data.lens = 35
    # Lift every subject a little so the bottom band stays free for captions.
    cam_data.shift_y = -0.08
    cam_data.clip_start, cam_data.clip_end = 0.05, 60
    cam = bpy.data.objects.new('cam', cam_data)
    s.collection.objects.link(cam)
    s.camera = cam
    return s, cam, drawn, text


def line_art(drawn, radius=0.006, wobble=0.0035):
    """One Line Art object draws every mesh in `drawn` as white chalk strokes."""
    gp_data = bpy.data.grease_pencils.new('chalk')
    gp = bpy.data.objects.new('chalk', gp_data)
    bpy.context.scene.collection.objects.link(gp)
    layer = gp_data.layers.new('lines')
    layer.frames.new(1)
    # Strokes are lit like surfaces by default; with no lights they render black.
    layer.use_lights = False
    gp.use_grease_pencil_lights = False
    mat = bpy.data.materials.new('chalk-stroke')
    bpy.data.materials.create_gpencil_data(mat)
    mat.grease_pencil.show_stroke = True
    mat.grease_pencil.show_fill = False
    mat.grease_pencil.color = (0.93, 0.93, 0.9, 1)
    gp_data.materials.append(mat)
    la = gp.modifiers.new('lineart', 'LINEART')
    la.source_type = 'COLLECTION'
    la.source_collection = drawn
    la.target_layer = 'lines'
    la.target_material = mat
    la.radius = radius
    la.use_crease = True
    la.crease_threshold = math.radians(140)
    la.use_intersection = False
    la.use_loose = True
    la.smooth_tolerance = 0.0
    la.use_edge_overlap = True
    # Subtle wobble that boils at 8 per second (house rule: wobble stays subtle).
    noise = gp.modifiers.new('wobble', 'GREASE_PENCIL_NOISE')
    noise.factor = wobble
    noise.factor_strength = 0.25
    noise.factor_thickness = 0.45
    noise.noise_scale = 0.6
    noise.use_random = True
    noise.step = 3
    return gp


_black = None


def black():
    """Faces are black so they hide what is behind them; only the lines show."""
    global _black
    if _black is None:
        _black = bpy.data.materials.new('ink-black')
        _black.use_nodes = True
        nt = _black.node_tree
        nt.nodes.clear()
        out = nt.nodes.new('ShaderNodeOutputMaterial')
        em = nt.nodes.new('ShaderNodeEmission')
        em.inputs['Color'].default_value = (0, 0, 0, 1)
        nt.links.new(em.outputs[0], out.inputs[0])
    return _black


_chalk = None


def chalk_material():
    """White chalk fill with a little grain; Object Info alpha fades it in and out."""
    global _chalk
    if _chalk is not None:
        return _chalk
    m = bpy.data.materials.new('chalk-fill')
    m.use_nodes = True
    m.surface_render_method = 'BLENDED'
    nt = m.node_tree
    nt.nodes.clear()
    out = nt.nodes.new('ShaderNodeOutputMaterial')
    info = nt.nodes.new('ShaderNodeObjectInfo')
    tc = nt.nodes.new('ShaderNodeTexCoord')
    noise = nt.nodes.new('ShaderNodeTexNoise')
    noise.inputs['Scale'].default_value = 900
    noise.inputs['Detail'].default_value = 2
    ramp = nt.nodes.new('ShaderNodeValToRGB')
    ramp.color_ramp.elements[0].position = 0.35
    ramp.color_ramp.elements[0].color = (0.62, 0.62, 0.6, 1)
    ramp.color_ramp.elements[1].position = 0.62
    ramp.color_ramp.elements[1].color = (0.95, 0.95, 0.92, 1)
    em = nt.nodes.new('ShaderNodeEmission')
    tr = nt.nodes.new('ShaderNodeBsdfTransparent')
    mix = nt.nodes.new('ShaderNodeMixShader')
    nt.links.new(tc.outputs['Window'], noise.inputs['Vector'])
    nt.links.new(noise.outputs['Fac'], ramp.inputs['Fac'])
    nt.links.new(ramp.outputs['Color'], em.inputs['Color'])
    nt.links.new(info.outputs['Alpha'], mix.inputs['Fac'])
    nt.links.new(tr.outputs[0], mix.inputs[1])
    nt.links.new(em.outputs[0], mix.inputs[2])
    nt.links.new(mix.outputs[0], out.inputs[0])
    _chalk = m
    return m


_solid = None


def solid():
    """Opaque chalk fill for shapes that should read as filled, not outlined."""
    global _solid
    if _solid is None:
        _solid = bpy.data.materials.new('chalk-solid')
        _solid.use_nodes = True
        nt = _solid.node_tree
        nt.nodes.clear()
        out = nt.nodes.new('ShaderNodeOutputMaterial')
        em = nt.nodes.new('ShaderNodeEmission')
        tc = nt.nodes.new('ShaderNodeTexCoord')
        noise = nt.nodes.new('ShaderNodeTexNoise')
        noise.inputs['Scale'].default_value = 700
        ramp = nt.nodes.new('ShaderNodeValToRGB')
        ramp.color_ramp.elements[0].position = 0.3
        ramp.color_ramp.elements[0].color = (0.45, 0.45, 0.44, 1)
        ramp.color_ramp.elements[1].position = 0.7
        ramp.color_ramp.elements[1].color = (0.85, 0.85, 0.82, 1)
        nt.links.new(tc.outputs['Window'], noise.inputs['Vector'])
        nt.links.new(noise.outputs['Fac'], ramp.inputs['Fac'])
        nt.links.new(ramp.outputs['Color'], em.inputs['Color'])
        nt.links.new(em.outputs[0], out.inputs[0])
    return _solid


_veil = None


def veil(cam):
    """A black card in front of the lens; its alpha (object colour) dips the film to black."""
    global _veil
    m = bpy.data.materials.new('veil')
    m.use_nodes = True
    m.surface_render_method = 'BLENDED'
    nt = m.node_tree
    nt.nodes.clear()
    out = nt.nodes.new('ShaderNodeOutputMaterial')
    info = nt.nodes.new('ShaderNodeObjectInfo')
    em = nt.nodes.new('ShaderNodeEmission')
    em.inputs['Color'].default_value = (0, 0, 0, 1)
    tr = nt.nodes.new('ShaderNodeBsdfTransparent')
    mix = nt.nodes.new('ShaderNodeMixShader')
    nt.links.new(info.outputs['Alpha'], mix.inputs['Fac'])
    nt.links.new(tr.outputs[0], mix.inputs[1])
    nt.links.new(em.outputs[0], mix.inputs[2])
    nt.links.new(mix.outputs[0], out.inputs[0])
    me = bpy.data.meshes.new('veil')
    me.from_pydata([(-2, -2, 0), (2, -2, 0), (2, 2, 0), (-2, 2, 0)], [], [(0, 1, 2, 3)])
    me.materials.append(m)
    o = bpy.data.objects.new('veil', me)
    bpy.data.collections['Text'].objects.link(o)
    o.parent = cam
    o.location = (0, 0, -0.08)
    o.color = (1, 1, 1, 0)
    # Line Art treats every mesh as an occluder; the veil must not hide the drawing.
    o.lineart.usage = 'EXCLUDE'
    _veil = o
    return o


# --- objects ---------------------------------------------------------------

def box(name, size, loc=(0, 0, 0), rot=(0, 0, 0), coll=None, parent=None, pivot=(0, 0, 0), mat=None):
    """A box mesh drawn by Line Art (black faces, chalk edges).

    `pivot` moves the origin to a face: (-1, 0, 0) puts it on the left face, (0, 0, -1) on the bottom.
    """
    import bmesh
    me = bpy.data.meshes.new(name)
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=1.0)
    for v in bm.verts:
        v.co.x *= size[0]
        v.co.y *= size[1]
        v.co.z *= size[2]
        v.co.x -= pivot[0] * size[0] / 2
        v.co.y -= pivot[1] * size[1] / 2
        v.co.z -= pivot[2] * size[2] / 2
    bm.to_mesh(me)
    bm.free()
    me.materials.append(mat or black())
    o = bpy.data.objects.new(name, me)
    (coll or bpy.data.collections['Drawn']).objects.link(o)
    o.location, o.rotation_euler = loc, rot
    if parent:
        o.parent = parent
    return o


def mesh_from(name, make, coll=None, parent=None):
    """Run a bpy.ops primitive and move the result into the drawn collection."""
    make()
    o = bpy.context.object
    o.name = name
    for c in list(o.users_collection):
        c.objects.unlink(o)
    (coll or bpy.data.collections['Drawn']).objects.link(o)
    o.data.materials.clear()
    o.data.materials.append(black())
    if parent:
        o.parent = parent
    return o


_font = None


_mono = None


def text(name, body, size=0.3, loc=(0, 0, 0), rot=(0, 0, 0), align='CENTER', parent=None, alpha=1.0, mono=False):
    """Filled chalk text. Faces the camera when parented to it with no rotation.

    `mono` uses Blender's built-in font, for commands and URLs.
    """
    global _font, _mono
    if _font is None:
        _font = bpy.data.fonts.load(FONT)
    if mono and _mono is None:
        _mono = bpy.data.fonts.load('<builtin>')
    cu = bpy.data.curves.new(name, 'FONT')
    cu.body = body
    cu.font = _mono if mono else _font
    cu.size = size
    cu.align_x = align
    cu.align_y = 'CENTER'
    cu.materials.append(chalk_material())
    o = bpy.data.objects.new(name, cu)
    bpy.data.collections['Text'].objects.link(o)
    o.location, o.rotation_euler = loc, rot
    o.color = (1, 1, 1, alpha)
    # Text never hides chalk lines behind it.
    o.lineart.usage = 'EXCLUDE'
    if parent:
        o.parent = parent
    return o


# --- animation -------------------------------------------------------------

def bake(obj, path, t0, t1, fn, index=-1):
    """Key `obj.<path>` on every frame from t0 to t1 with value fn(t), linearly between keys."""
    for f in range(F(at(t0)), F(at(t1)) + 1):
        v = fn(N((f - 1) / FPS))
        if index >= 0:
            getattr(obj, path)[index] = v
        else:
            setattr(obj, path, v)
        obj.keyframe_insert(path, index=index, frame=f)


def linearise(obj):
    ad = obj.animation_data
    if not ad or not ad.action:
        return
    for fc in fcurves(ad):
        for k in fc.keyframe_points:
            k.interpolation = 'LINEAR'


def fcurves(ad):
    action = ad.action
    if hasattr(action, 'fcurves') and len(getattr(action, 'fcurves', [])):
        return list(action.fcurves)
    out = []
    for layer in getattr(action, 'layers', []):
        for strip in layer.strips:
            for bag in strip.channelbags:
                out.extend(bag.fcurves)
    return out


def visible(obj, t0, t1):
    """Show `obj` (and its children) only between t0 and t1."""
    windows(obj, [(t0, t1)])


def windows(obj, spans, keep=False):
    """Show `obj` (and its children) only inside the given (t0, t1) spans.

    With keep=True, objects that already have their own window are left alone.
    """
    keys = [(1, True)]
    for t0, t1 in spans:
        keys += [(max(1, F(at(t0))), False), (F(at(t1)), True)]
    for o in [obj] + list(obj.children_recursive):
        if keep and o.animation_data and o.animation_data.action and \
                any(fc.data_path == 'hide_render' for fc in fcurves(o.animation_data)):
            continue
        for f, hide in keys:
            o.hide_render = hide
            o.hide_viewport = hide
            o.keyframe_insert('hide_render', frame=f)
            o.keyframe_insert('hide_viewport', frame=f)


def fade(obj, t_in, t_out=None, d_in=0.35, d_out=0.3, peak=1.0):
    """Alpha for chalk text: fade in at t_in, out at t_out (None keeps it)."""
    end = t_out + d_out if t_out is not None else t_in + d_in
    def a(t):
        k = ease_out(prog(t, t_in, t_in + d_in)) * peak
        if t_out is not None:
            k *= 1 - ease_in(prog(t, t_out, t_out + d_out), 2)
        return k
    bake(obj, 'color', t_in - 1 / FPS, end, a, index=3)
    visible(obj, t_in - 1 / FPS, end if t_out is not None else 999)
