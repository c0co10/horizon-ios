#!/usr/bin/env python3
"""Writes the content half of the Create port (`mods/ports/create/content`).

Every file under `content/` comes from this script: the block and item
declarations (vanilla templates, `docs/modpacks.md`), loot tables, recipes,
tags, block states, models, item models, names, and the textures, which are
drawn here pixel by pixel. Create's own assets are "All Rights Reserved"
(its LICENSE.md), so none of them is used or traced; the shapes follow what
the blocks are (a shaft, a cogwheel, a casing) and the colours the
materials (andesite, oak, iron, copper).

    python3 mods/ports/create/tools/generate.py

Run it again after changing it; it rewrites `content/` (except
`horizon-mod.toml`) and is deterministic.
"""
import json
import os
import random
import shutil

from PIL import Image

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "content")
NS = "create"

# ---------------------------------------------------------------------------
# Palette
# ---------------------------------------------------------------------------
ANDESITE = [(136, 140, 141), (150, 154, 155), (122, 126, 127), (163, 166, 165), (110, 114, 116)]
ALLOY = [(168, 172, 166), (186, 190, 182), (143, 148, 142), (204, 207, 198), (120, 125, 121)]
OAK = [(162, 130, 78), (150, 119, 70), (184, 148, 95), (126, 98, 56), (171, 139, 87)]
OAK_DARK = (98, 74, 42)
IRON = [(196, 196, 196), (216, 216, 216), (170, 170, 170), (232, 232, 232), (150, 150, 150)]
DARK = (54, 57, 60)
SHADOW = (82, 86, 88)
COPPER = [(192, 107, 79), (216, 132, 98), (160, 86, 62), (230, 160, 120)]
CANVAS = [(232, 228, 214), (222, 218, 203), (240, 237, 226), (210, 205, 190)]
RED = (196, 52, 40)
RED_ON = (255, 92, 64)
BLUE = (48, 92, 160)
BLUE_ON = (90, 160, 255)
NONE = (0, 0, 0, 0)


def rng(name):
    return random.Random(name)


def noise(name, colours, size=16):
    r = rng(name)
    img = Image.new("RGBA", (size, size))
    for y in range(size):
        for x in range(size):
            c = colours[0] if r.random() < 0.45 else r.choice(colours)
            img.putpixel((x, y), c + (255,))
    return img


def frame(img, colour, inset=0, width=1):
    w, h = img.size
    for i in range(width):
        a, b = inset + i, w - 1 - inset - i
        for t in range(a, b + 1):
            img.putpixel((t, a), colour + (255,))
            img.putpixel((t, b), colour + (255,))
            img.putpixel((a, t), colour + (255,))
            img.putpixel((b, t), colour + (255,))
    return img


def rect(img, x0, y0, x1, y1, colour):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            img.putpixel((x, y), colour + (255,) if len(colour) == 3 else colour)


def planks(name):
    """Oak boards running across, a dark seam every four rows."""
    r = rng(name)
    img = noise(name, OAK)
    for y in (3, 7, 11, 15):
        for x in range(16):
            img.putpixel((x, y), OAK_DARK + (255,))
    for y0 in (0, 4, 8, 12):
        x = r.randrange(2, 14)
        for y in range(y0, y0 + 3):
            img.putpixel((x, y), OAK[3] + (255,))
    return img


def casing_side(name, hole=False, indicator=None):
    """Andesite casing: oak boards inside a frame of andesite alloy."""
    img = planks(name)
    frame(img, ALLOY[0], 0, 2)
    frame(img, ALLOY[4], 1, 1)
    for x, y in ((0, 0), (15, 0), (0, 15), (15, 15)):
        img.putpixel((x, y), ALLOY[3] + (255,))
    if hole:
        rect(img, 5, 5, 10, 10, DARK)
        rect(img, 6, 6, 9, 9, ANDESITE[2])
        rect(img, 7, 7, 8, 8, ANDESITE[3])
    if indicator:
        rect(img, 6, 2, 9, 3, indicator)
        rect(img, 6, 12, 9, 13, indicator)
    return img


def casing_top(name):
    img = planks(name).rotate(90)
    frame(img, ALLOY[0], 0, 2)
    frame(img, ALLOY[4], 1, 1)
    return img


def shaft_side(phase=0, name="shaft"):
    """A turned andesite rod: light and dark bands that move with `phase`."""
    img = Image.new("RGBA", (16, 16), NONE)
    base = noise(name, ALLOY)
    for y in range(16):
        for x in range(16):
            band = (y + phase) % 4
            c = base.getpixel((x, y))[:3]
            if band == 0:
                c = ALLOY[3]
            elif band == 2:
                c = ALLOY[4]
            col = x % 4
            if col == 0:
                c = tuple(max(0, v - 18) for v in c)
            elif col == 3:
                c = tuple(min(255, v + 10) for v in c)
            img.putpixel((x, y), c + (255,))
    return img


def animated(frames):
    sheet = Image.new("RGBA", (16, 16 * len(frames)), NONE)
    for i, f in enumerate(frames):
        sheet.paste(f, (0, 16 * i))
    return sheet


def shaft_end():
    img = noise("shaft_end", ALLOY)
    frame(img, ALLOY[4], 0, 1)
    rect(img, 6, 6, 9, 9, ALLOY[3])
    rect(img, 7, 7, 8, 8, DARK)
    return img


def cog_wood(name, phase=0):
    """End grain of the cog's oak teeth, rings that drift when it turns."""
    img = noise(name, OAK)
    for y in range(16):
        for x in range(16):
            if (x + y + phase) % 5 == 0:
                img.putpixel((x, y), OAK[3] + (255,))
    return img


def gauge_face(kind):
    img = casing_side("gauge_" + kind)
    rect(img, 3, 3, 12, 12, (232, 230, 220))
    frame(img, DARK, 3, 1)
    colour = (60, 160, 70) if kind == "speed" else RED
    for x in range(4, 12):
        img.putpixel((x, 11), colour + (255,))
    # A needle from the dial's centre.
    for i in range(4):
        img.putpixel((8 + i // 2, 10 - i), DARK + (255,))
    return img


def drill_metal(phase=0):
    img = noise("drill", IRON)
    for y in range(16):
        for x in range(16):
            if (x - y + phase) % 4 == 0:
                img.putpixel((x, y), IRON[4] + (255,))
    return img


def saw_blade(phase=0):
    img = Image.new("RGBA", (16, 16), NONE)
    for y in range(16):
        for x in range(16):
            if y >= 3:
                img.putpixel((x, y), (IRON[0] if (x + y) % 3 else IRON[2]) + (255,))
            elif (x + phase) % 4 < 2 and y >= 3 - ((x + phase) % 4 + 1):
                img.putpixel((x, y), IRON[3] + (255,))
    rect(img, 0, 14, 15, 15, SHADOW)
    return img


def motor_side():
    img = noise("motor", COPPER)
    frame(img, DARK, 0, 1)
    for y in (4, 8, 12):
        for x in range(2, 14):
            img.putpixel((x, y), COPPER[2] + (255,))
    return img


def motor_front():
    img = motor_side()
    rect(img, 4, 4, 11, 11, DARK)
    rect(img, 6, 6, 9, 9, ALLOY[3])
    return img


def piston_top(name):
    img = planks(name)
    frame(img, ALLOY[4], 0, 1)
    return img


def sail():
    img = noise("sail", CANVAS)
    for y in range(16):
        if y % 4 == 0:
            for x in range(16):
                img.putpixel((x, y), CANVAS[3] + (255,))
    frame(img, OAK[3], 0, 1)
    return img


def sail_frame():
    img = Image.new("RGBA", (16, 16), NONE)
    frame(img, OAK[0], 0, 2)
    frame(img, OAK_DARK, 1, 1)
    for i in range(2, 14):
        img.putpixel((i, i), OAK[3] + (255,))
    return img


def alloy_item():
    img = Image.new("RGBA", (16, 16), NONE)
    r = rng("alloy_item")
    rows = [(4, 12, 5), (3, 13, 6), (2, 13, 7), (2, 13, 8), (2, 12, 9), (3, 11, 10)]
    for x0, x1, y in rows:
        for x in range(x0, x1 + 1):
            img.putpixel((x, y), r.choice(ALLOY) + (255,))
    for x in range(4, 12):
        img.putpixel((x, 5), ALLOY[3] + (255,))
    for x in range(3, 12):
        img.putpixel((x, 11), ALLOY[4] + (255,))
    return img


def crank_wood():
    img = noise("crank", OAK)
    frame(img, OAK_DARK, 0, 1)
    return img


# ---------------------------------------------------------------------------
# Models
# ---------------------------------------------------------------------------
ALL = ("north", "south", "east", "west", "up", "down")


def box(f, t, tex, faces=ALL, uv=None, rotation=None, end=None, side_uv=True):
    """A model element; `end` textures the faces along the box's long axis
    differently from its sides (a rod's ends)."""
    faces_out = {}
    for face in faces:
        texture = tex
        if end and face in ("up", "down"):
            texture = end
        entry = {"texture": texture}
        if uv:
            entry["uv"] = uv
        elif side_uv:
            entry["uv"] = face_uv(f, t, face)
        faces_out[face] = entry
    el = {"from": f, "to": t, "faces": faces_out}
    if rotation:
        el["rotation"] = rotation
    return el


def clampuv(v):
    return max(0, min(16, v))


def face_uv(f, t, face):
    x0, y0, z0 = f
    x1, y1, z1 = t
    if face in ("up", "down"):
        u = [x0, z0, x1, z1]
    elif face in ("north", "south"):
        u = [x0, 16 - y1, x1, 16 - y0]
    else:
        u = [z0, 16 - y1, z1, 16 - y0]
    u = [clampuv(v) for v in u]
    if u[0] == u[2]:
        u[2] = min(16, u[0] + 1)
    if u[1] == u[3]:
        u[3] = min(16, u[1] + 1)
    return u


def model(textures, elements, particle=None, parent="minecraft:block/block"):
    m = {"parent": parent, "textures": dict(textures)}
    m["textures"]["particle"] = particle or next(iter(textures.values()))
    m["elements"] = elements
    return m


def rod(tex, end, a=6, b=10, y0=0, y1=16):
    return box([a, y0, a], [b, y1, b], tex, end=end)


def shaft_model(spin):
    t = f"{NS}:block/shaft_spinning" if spin else f"{NS}:block/shaft"
    return model({"side": t, "end": f"{NS}:block/shaft_end"}, [rod("#side", "#end")])


def cog_elements(radius_teeth, radius_disc, y0, y1, prefix=""):
    lo, hi = 8 - radius_teeth, 8 + radius_teeth
    d0, d1 = 8 - radius_disc, 8 + radius_disc
    tw = 1.0 if radius_teeth <= 8 else 1.5
    els = [
        box([d0, y0, d0], [d1, y1, d1], "#disc", uv=[0, 0, 16, 16]),
        box([d0, y0, d0], [d1, y1, d1], "#disc", uv=[0, 0, 16, 16],
            rotation={"angle": 45, "axis": "y", "origin": [8, 8, 8]}),
    ]
    for rot in (None, {"angle": 45, "axis": "y", "origin": [8, 8, 8]}):
        els.append(box([lo, y0 + 0.5, 8 - tw], [hi, y1 - 0.5, 8 + tw], "#teeth", uv=[0, 6, 16, 10], rotation=rot))
        els.append(box([8 - tw, y0 + 0.5, lo], [8 + tw, y1 - 0.5, hi], "#teeth", uv=[0, 6, 16, 10], rotation=rot))
    return els


def cog_model(large, spin):
    sfx = "_spinning" if spin else ""
    tex = {
        "side": f"{NS}:block/shaft{sfx}",
        "end": f"{NS}:block/shaft_end",
        "disc": f"{NS}:block/cogwheel_disc{sfx}",
        "teeth": f"{NS}:block/cogwheel_teeth",
    }
    if large:
        els = [rod("#side", "#end")] + cog_elements(15, 11, 6, 10)
    else:
        els = [rod("#side", "#end")] + cog_elements(8, 6, 6, 10)
    return model(tex, els, particle=tex["teeth"])


def water_wheel_model(spin):
    sfx = "_spinning" if spin else ""
    tex = {
        "side": f"{NS}:block/shaft{sfx}",
        "end": f"{NS}:block/shaft_end",
        "disc": f"{NS}:block/cogwheel_disc{sfx}",
        "paddle": f"{NS}:block/water_wheel_paddle",
    }
    els = [rod("#side", "#end"), box([3, 5, 3], [13, 11, 13], "#disc", uv=[0, 0, 16, 16])]
    for rot in (None, {"angle": 45, "axis": "y", "origin": [8, 8, 8]}):
        els.append(box([-8, 4, 7], [24, 12, 9], "#paddle", uv=[0, 4, 16, 12], rotation=rot))
        els.append(box([7, 4, -8], [9, 12, 24], "#paddle", uv=[0, 4, 16, 12], rotation=rot))
    return model(tex, els, particle=tex["paddle"])


def cube(textures_by_face, particle):
    t = {}
    faces = {}
    for face, tex in textures_by_face.items():
        key = face
        t[key] = tex
        faces[face] = {"texture": "#" + key, "uv": [0, 0, 16, 16], "cullface": face}
    return {
        "parent": "minecraft:block/block",
        "textures": dict(t, particle=particle),
        "elements": [{"from": [0, 0, 0], "to": [16, 16, 16], "faces": faces}],
    }


def axis_cube(end, side):
    """A cube with `end` on its top and bottom (the Y axis) and `side` round."""
    return cube({"up": end, "down": end, "north": side, "south": side, "east": side, "west": side}, side)


def gearbox_model():
    hole = f"{NS}:block/gearbox_side"
    top = f"{NS}:block/andesite_casing_top"
    return cube({"up": top, "down": top, "north": hole, "south": hole, "east": hole, "west": hole}, hole)


def all_cube(tex):
    return cube({f: tex for f in ALL}, tex)


def crank_model(on):
    tex = {"wood": f"{NS}:block/hand_crank", "side": f"{NS}:block/shaft", "end": f"{NS}:block/shaft_end"}
    els = [
        box([6.5, 0, 6.5], [9.5, 5, 9.5], "#side", end="#end"),
        box([2, 5, 7], [14, 7, 9], "#wood"),
    ]
    if on:
        els.append(box([2, 7, 7], [4, 13, 9], "#wood"))
        els.append(box([12, 3, 7], [14, 5, 9], "#wood"))
    else:
        els.append(box([12, 7, 7], [14, 13, 9], "#wood"))
        els.append(box([2, 3, 7], [4, 5, 9], "#wood"))
    return model(tex, els, particle=tex["wood"])


def facing_body(top, side, bottom, height=12):
    return box([0, 0, 0], [16, height, 16], "#side", faces=("north", "south", "east", "west")), {
        "up": top,
        "down": bottom,
    }


def motor_model():
    tex = {
        "side": f"{NS}:block/creative_motor_side",
        "front": f"{NS}:block/creative_motor_front",
        "shaft": f"{NS}:block/shaft",
        "end": f"{NS}:block/shaft_end",
    }
    els = [
        {
            "from": [1, 0, 1],
            "to": [15, 12, 15],
            "faces": {
                "north": {"texture": "#side", "uv": [1, 4, 15, 16]},
                "south": {"texture": "#side", "uv": [1, 4, 15, 16]},
                "east": {"texture": "#side", "uv": [1, 4, 15, 16]},
                "west": {"texture": "#side", "uv": [1, 4, 15, 16]},
                "up": {"texture": "#front", "uv": [1, 1, 15, 15]},
                "down": {"texture": "#side", "uv": [1, 1, 15, 15], "cullface": "down"},
            },
        },
        rod("#shaft", "#end", y0=12, y1=16),
    ]
    return model(tex, els, particle=tex["side"])


def piston_model():
    tex = {
        "side": f"{NS}:block/andesite_casing",
        "top": f"{NS}:block/mechanical_piston_top",
        "bottom": f"{NS}:block/andesite_casing_top",
        "hole": f"{NS}:block/gearbox_side",
    }
    els = [
        {
            "from": [0, 0, 0],
            "to": [16, 16, 16],
            "faces": {
                "north": {"texture": "#hole", "cullface": "north"},
                "south": {"texture": "#hole", "cullface": "south"},
                "east": {"texture": "#hole", "cullface": "east"},
                "west": {"texture": "#hole", "cullface": "west"},
                "up": {"texture": "#top", "cullface": "up"},
                "down": {"texture": "#bottom", "cullface": "down"},
            },
        }
    ]
    return model(tex, els, particle=tex["side"])


def piston_head_model():
    tex = {"top": f"{NS}:block/mechanical_piston_top", "side": f"{NS}:block/shaft", "end": f"{NS}:block/shaft_end",
           "wood": f"{NS}:block/hand_crank"}
    els = [
        box([0, 12, 0], [16, 16, 16], "#wood", faces=("north", "south", "east", "west", "down")),
        box([0, 12, 0], [16, 16, 16], "#top", faces=("up",), uv=[0, 0, 16, 16]),
        rod("#side", "#end", y0=-4, y1=12),
    ]
    els[1]["faces"]["up"]["cullface"] = "up"
    return model(tex, els, particle=tex["wood"])


def pole_model():
    tex = {"side": f"{NS}:block/piston_extension_pole", "end": f"{NS}:block/shaft_end"}
    return model(tex, [rod("#side", "#end", 5, 11)], particle=tex["side"])


def drill_model(spin):
    sfx = "_spinning" if spin else ""
    tex = {"case": f"{NS}:block/andesite_casing", "metal": f"{NS}:block/drill_head{sfx}",
           "bottom": f"{NS}:block/gearbox_side"}
    els = [
        box([0, 0, 0], [16, 8, 16], "#case", faces=("north", "south", "east", "west", "up")),
        box([0, 0, 0], [16, 8, 16], "#bottom", faces=("down",), uv=[0, 0, 16, 16]),
        box([2, 8, 2], [14, 11, 14], "#metal"),
        box([4, 11, 4], [12, 14, 12], "#metal"),
        box([6, 14, 6], [10, 16, 10], "#metal"),
    ]
    els[1]["faces"]["down"]["cullface"] = "down"
    return model(tex, els, particle=tex["case"])


def saw_model(spin):
    sfx = "_spinning" if spin else ""
    tex = {"case": f"{NS}:block/andesite_casing", "blade": f"{NS}:block/saw_blade{sfx}",
           "bottom": f"{NS}:block/gearbox_side", "top": f"{NS}:block/andesite_casing_top"}
    els = [
        box([0, 0, 0], [16, 10, 16], "#case", faces=("north", "south", "east", "west")),
        box([0, 0, 0], [16, 10, 16], "#bottom", faces=("down",), uv=[0, 0, 16, 16]),
        box([0, 0, 0], [16, 10, 16], "#top", faces=("up",), uv=[0, 0, 16, 16]),
        box([0, 10, 7.5], [16, 16, 8.5], "#blade", faces=("north", "south"), uv=[0, 0, 16, 6]),
        box([0, 10, 7.5], [16, 16, 8.5], "#blade", faces=("east", "west", "up"), uv=[0, 3, 1, 4]),
    ]
    els[1]["faces"]["down"]["cullface"] = "down"
    return model(tex, els, particle=tex["case"])


def bearing_model():
    tex = {"case": f"{NS}:block/andesite_casing", "plate": f"{NS}:block/windmill_bearing_top",
           "bottom": f"{NS}:block/gearbox_side"}
    els = [
        box([0, 0, 0], [16, 12, 16], "#case", faces=("north", "south", "east", "west", "up")),
        box([0, 0, 0], [16, 12, 16], "#bottom", faces=("down",), uv=[0, 0, 16, 16]),
        box([2, 12, 2], [14, 16, 14], "#plate"),
    ]
    els[1]["faces"]["down"]["cullface"] = "down"
    return model(tex, els, particle=tex["case"])


def gauge_model(kind):
    face = f"{NS}:block/{kind}ometer_face"
    hole = f"{NS}:block/encased_shaft_end"
    return cube({"up": hole, "down": hole, "north": face, "south": face, "east": face, "west": face}, face)


def clutch_model(kind, on):
    side = f"{NS}:block/{kind}_side{'_on' if on else ''}"
    return all_cube(side)


# ---------------------------------------------------------------------------
# Block states
# ---------------------------------------------------------------------------
def axis_states(m):
    return {"variants": {
        "axis=x": {"model": m, "x": 90, "y": 90},
        "axis=y": {"model": m},
        "axis=z": {"model": m, "x": 90},
    }}


def facing_states(m):
    return {"variants": {
        "facing=up": {"model": m},
        "facing=down": {"model": m, "x": 180},
        "facing=north": {"model": m, "x": 90},
        "facing=south": {"model": m, "x": 90, "y": 180},
        "facing=west": {"model": m, "x": 90, "y": 270},
        "facing=east": {"model": m, "x": 90, "y": 90},
    }}


def lever_states(off, on):
    v = {}
    turns = {"north": 0, "east": 90, "south": 180, "west": 270}
    for powered, m in (("false", off), ("true", on)):
        for facing, y in turns.items():
            v[f"face=floor,facing={facing},powered={powered}"] = {"model": m, **({"y": y} if y else {})}
            v[f"face=ceiling,facing={facing},powered={powered}"] = {"model": m, "x": 180, **({"y": (y + 180) % 360} if (y + 180) % 360 else {})}
            v[f"face=wall,facing={facing},powered={powered}"] = {"model": m, "x": 90, **({"y": y} if y else {})}
    return {"variants": v}


def lit_states(off, on):
    return {"variants": {"lit=false": {"model": off}, "lit=true": {"model": on}}}


def single(m):
    return {"variants": {"": {"model": m}}}


# ---------------------------------------------------------------------------
# The blocks
# ---------------------------------------------------------------------------
METAL = "minecraft:iron_block"
WOOD = "minecraft:oak_planks"
STONE = "minecraft:stone"
WOOL = "minecraft:white_wool"
CHAIN = "minecraft:iron_chain"

# id: (template, properties, item, drops, tool, names (en, de))
BLOCKS = {}


def block(id, template, props, names, item=True, drops=None, tool="pickaxe", states=None, models=None):
    BLOCKS[id] = dict(template=template, props=props, item=item, drops=drops if drops is not None else (id if item else None),
                      tool=tool, names=names, states=states, models=models or {})


def kinetic_props(sounds, hardness=1.5):
    return {"hardness": hardness, "explosion_resistance": 6.0, "light": 0, "sounds": sounds}


M = lambda n: f"{NS}:block/{n}"

block("shaft", CHAIN, kinetic_props(METAL, 1.5), ("Shaft", "Welle"),
      states=axis_states(M("shaft")), models={"shaft": shaft_model(False)})
block("shaft_spinning", CHAIN, kinetic_props(METAL, 1.5), ("Shaft", "Welle"), item=False, drops="shaft",
      states=axis_states(M("shaft_spinning")), models={"shaft_spinning": shaft_model(True)})
block("cogwheel", CHAIN, kinetic_props(WOOD, 1.5), ("Cogwheel", "Zahnrad"), tool="axe",
      states=axis_states(M("cogwheel")), models={"cogwheel": cog_model(False, False)})
block("cogwheel_spinning", CHAIN, kinetic_props(WOOD, 1.5), ("Cogwheel", "Zahnrad"), item=False, drops="cogwheel", tool="axe",
      states=axis_states(M("cogwheel_spinning")), models={"cogwheel_spinning": cog_model(False, True)})
block("large_cogwheel", CHAIN, kinetic_props(WOOD, 1.5), ("Large Cogwheel", "Großes Zahnrad"), tool="axe",
      states=axis_states(M("large_cogwheel")), models={"large_cogwheel": cog_model(True, False)})
block("large_cogwheel_spinning", CHAIN, kinetic_props(WOOD, 1.5), ("Large Cogwheel", "Großes Zahnrad"), item=False,
      drops="large_cogwheel", tool="axe",
      states=axis_states(M("large_cogwheel_spinning")), models={"large_cogwheel_spinning": cog_model(True, True)})
block("andesite_casing", "minecraft:polished_andesite", {"hardness": 2.0, "sounds": WOOD}, ("Andesite Casing", "Andesitgehäuse"),
      tool="axe", states=single(M("andesite_casing")), models={"andesite_casing": all_cube(M("andesite_casing"))})
block("encased_shaft", "minecraft:bone_block", {"hardness": 2.0, "sounds": WOOD}, ("Encased Shaft", "Ummantelte Welle"),
      tool="axe", states=axis_states(M("encased_shaft")),
      models={"encased_shaft": axis_cube(M("encased_shaft_end"), M("andesite_casing"))})
block("gearbox", "minecraft:bone_block", {"hardness": 2.0, "sounds": WOOD}, ("Gearbox", "Getriebe"), tool="axe",
      states=axis_states(M("gearbox")), models={"gearbox": gearbox_model()})
block("clutch", "minecraft:redstone_lamp", {"hardness": 2.0, "light": 0, "sounds": WOOD}, ("Clutch", "Kupplung"), tool="axe",
      states=lit_states(M("clutch"), M("clutch_on")),
      models={"clutch": clutch_model("clutch", False), "clutch_on": clutch_model("clutch", True)})
block("gearshift", "minecraft:redstone_lamp", {"hardness": 2.0, "light": 0, "sounds": WOOD}, ("Gearshift", "Gangschaltung"),
      tool="axe", states=lit_states(M("gearshift"), M("gearshift_on")),
      models={"gearshift": clutch_model("gearshift", False), "gearshift_on": clutch_model("gearshift", True)})
block("hand_crank", "minecraft:lever", {"hardness": 1.0, "sounds": WOOD}, ("Hand Crank", "Handkurbel"), tool="axe",
      states=lever_states(M("hand_crank"), M("hand_crank_on")),
      models={"hand_crank": crank_model(False), "hand_crank_on": crank_model(True)})
block("water_wheel", CHAIN, kinetic_props(WOOD, 2.0), ("Water Wheel", "Wasserrad"), tool="axe",
      states=axis_states(M("water_wheel")), models={"water_wheel": water_wheel_model(False)})
block("water_wheel_spinning", CHAIN, kinetic_props(WOOD, 2.0), ("Water Wheel", "Wasserrad"), item=False, drops="water_wheel",
      tool="axe", states=axis_states(M("water_wheel_spinning")), models={"water_wheel_spinning": water_wheel_model(True)})
block("creative_motor", "minecraft:end_rod", {"hardness": -1.0, "explosion_resistance": 3600000.0, "light": 0, "sounds": METAL},
      ("Creative Motor", "Kreativ-Motor"), states=facing_states(M("creative_motor")),
      models={"creative_motor": motor_model()}, tool=None)
block("windmill_bearing", "minecraft:end_rod", {"hardness": 2.0, "light": 0, "sounds": WOOD},
      ("Windmill Bearing", "Windmühlenlager"), tool="axe", states=facing_states(M("windmill_bearing")),
      models={"windmill_bearing": bearing_model()})
block("white_sail", WOOL, {"hardness": 0.8}, ("Sail", "Segel"), tool="axe", states=single(M("white_sail")),
      models={"white_sail": all_cube(M("white_sail"))})
block("sail_frame", "minecraft:glass", {"hardness": 0.8, "sounds": WOOD}, ("Sail Frame", "Segelrahmen"), tool="axe",
      states=single(M("sail_frame")), models={"sail_frame": all_cube(M("sail_frame"))})
block("mechanical_piston", "minecraft:end_rod", {"hardness": 2.0, "light": 0, "sounds": WOOD},
      ("Mechanical Piston", "Mechanischer Kolben"), tool="axe", states=facing_states(M("mechanical_piston")),
      models={"mechanical_piston": piston_model()})
block("mechanical_piston_head", "minecraft:end_rod", {"hardness": 2.0, "light": 0, "sounds": WOOD},
      ("Mechanical Piston Head", "Mechanischer Kolbenkopf"), item=False, drops=None, tool="axe",
      states=facing_states(M("mechanical_piston_head")), models={"mechanical_piston_head": piston_head_model()})
block("piston_extension_pole", CHAIN, kinetic_props(WOOD, 1.0), ("Piston Extension Pole", "Kolbenverlängerung"), tool="axe",
      states=axis_states(M("piston_extension_pole")), models={"piston_extension_pole": pole_model()})
block("mechanical_drill", "minecraft:end_rod", {"hardness": 2.0, "light": 0, "sounds": METAL},
      ("Mechanical Drill", "Mechanischer Bohrer"), states=facing_states(M("mechanical_drill")),
      models={"mechanical_drill": drill_model(False)})
block("mechanical_drill_spinning", "minecraft:end_rod", {"hardness": 2.0, "light": 0, "sounds": METAL},
      ("Mechanical Drill", "Mechanischer Bohrer"), item=False, drops="mechanical_drill",
      states=facing_states(M("mechanical_drill_spinning")), models={"mechanical_drill_spinning": drill_model(True)})
block("mechanical_saw", "minecraft:end_rod", {"hardness": 2.0, "light": 0, "sounds": METAL},
      ("Mechanical Saw", "Mechanische Säge"), states=facing_states(M("mechanical_saw")),
      models={"mechanical_saw": saw_model(False)})
block("mechanical_saw_spinning", "minecraft:end_rod", {"hardness": 2.0, "light": 0, "sounds": METAL},
      ("Mechanical Saw", "Mechanische Säge"), item=False, drops="mechanical_saw",
      states=facing_states(M("mechanical_saw_spinning")), models={"mechanical_saw_spinning": saw_model(True)})
block("speedometer", "minecraft:bone_block", {"hardness": 2.0, "sounds": WOOD}, ("Speedometer", "Tachometer"), tool="axe",
      states=axis_states(M("speedometer")), models={"speedometer": gauge_model("speed")})
block("stressometer", "minecraft:bone_block", {"hardness": 2.0, "sounds": WOOD}, ("Stressometer", "Belastungsmesser"),
      tool="axe", states=axis_states(M("stressometer")), models={"stressometer": gauge_model("stress")})

ITEMS = {
    "andesite_alloy": ("minecraft:iron_ingot", ("Andesite Alloy", "Andesitlegierung")),
}

# The model an item shows: its block's (spinning variants have no item).
ITEM_MODEL = {
    "hand_crank": "hand_crank",
    "clutch": "clutch",
    "gearshift": "gearshift",
}


def tag(i):
    return "#" + i


RECIPES = {
    "andesite_alloy": {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "key": {"A": "minecraft:andesite", "N": "minecraft:iron_nugget"},
        "pattern": ["AN", "NA"], "result": {"id": f"{NS}:andesite_alloy", "count": 1},
    },
    "shaft": {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "key": {"A": f"{NS}:andesite_alloy"}, "pattern": ["A", "A"],
        "result": {"id": f"{NS}:shaft", "count": 8},
    },
    "cogwheel": {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": [f"{NS}:shaft", tag("minecraft:planks")],
        "result": {"id": f"{NS}:cogwheel", "count": 1},
    },
    "large_cogwheel": {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": [f"{NS}:shaft", tag("minecraft:planks"), tag("minecraft:planks")],
        "result": {"id": f"{NS}:large_cogwheel", "count": 1},
    },
    "large_cogwheel_from_cogwheel": {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": [f"{NS}:cogwheel", tag("minecraft:planks")],
        "result": {"id": f"{NS}:large_cogwheel", "count": 1},
    },
    "andesite_casing": {
        "type": "minecraft:crafting_shaped", "category": "building",
        "key": {"A": f"{NS}:andesite_alloy", "L": tag("minecraft:logs")},
        "pattern": ["A", "L"], "result": {"id": f"{NS}:andesite_casing", "count": 2},
    },
    "encased_shaft": {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": [f"{NS}:andesite_casing", f"{NS}:shaft"],
        "result": {"id": f"{NS}:encased_shaft", "count": 1},
    },
    "gearbox": {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": [f"{NS}:andesite_casing", f"{NS}:cogwheel"],
        "result": {"id": f"{NS}:gearbox", "count": 1},
    },
    "clutch": {
        "type": "minecraft:crafting_shapeless", "category": "redstone",
        "ingredients": [f"{NS}:andesite_casing", f"{NS}:shaft", "minecraft:redstone"],
        "result": {"id": f"{NS}:clutch", "count": 1},
    },
    "gearshift": {
        "type": "minecraft:crafting_shapeless", "category": "redstone",
        "ingredients": [f"{NS}:andesite_casing", f"{NS}:cogwheel", "minecraft:redstone"],
        "result": {"id": f"{NS}:gearshift", "count": 1},
    },
    "hand_crank": {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "key": {"P": tag("minecraft:planks"), "A": f"{NS}:andesite_alloy"},
        "pattern": ["PPP", "  A"], "result": {"id": f"{NS}:hand_crank", "count": 1},
    },
    "water_wheel": {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "key": {"P": tag("minecraft:planks"), "S": f"{NS}:shaft"},
        "pattern": ["PPP", "PSP", "PPP"], "result": {"id": f"{NS}:water_wheel", "count": 1},
    },
    "windmill_bearing": {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "key": {"W": tag("minecraft:wooden_slabs"), "C": f"{NS}:andesite_casing", "S": f"{NS}:shaft"},
        "pattern": ["W", "C", "S"], "result": {"id": f"{NS}:windmill_bearing", "count": 1},
    },
    "sail_frame": {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "key": {"S": "minecraft:stick", "A": f"{NS}:andesite_alloy"},
        "pattern": ["SSS", "SAS", "SSS"], "result": {"id": f"{NS}:sail_frame", "count": 8},
    },
    "white_sail": {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": [f"{NS}:sail_frame", "minecraft:white_wool"],
        "result": {"id": f"{NS}:white_sail", "count": 1},
    },
    "mechanical_piston": {
        "type": "minecraft:crafting_shaped", "category": "redstone",
        "key": {"P": "minecraft:piston", "C": f"{NS}:andesite_casing", "S": f"{NS}:shaft"},
        "pattern": ["P", "C", "S"], "result": {"id": f"{NS}:mechanical_piston", "count": 1},
    },
    "piston_extension_pole": {
        "type": "minecraft:crafting_shaped", "category": "redstone",
        "key": {"A": f"{NS}:andesite_alloy", "P": tag("minecraft:planks")},
        "pattern": ["A", "P", "A"], "result": {"id": f"{NS}:piston_extension_pole", "count": 8},
    },
    "mechanical_drill": {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "key": {"A": f"{NS}:andesite_alloy", "I": "minecraft:iron_ingot", "C": f"{NS}:andesite_casing"},
        "pattern": [" A ", "AIA", " C "], "result": {"id": f"{NS}:mechanical_drill", "count": 1},
    },
    "mechanical_saw": {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "key": {"I": "minecraft:iron_ingot", "A": f"{NS}:andesite_alloy", "C": f"{NS}:andesite_casing"},
        "pattern": ["III", "ACA"], "result": {"id": f"{NS}:mechanical_saw", "count": 1},
    },
    "speedometer": {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": [f"{NS}:andesite_casing", "minecraft:compass"],
        "result": {"id": f"{NS}:speedometer", "count": 1},
    },
    "stressometer": {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": [f"{NS}:speedometer", "minecraft:redstone"],
        "result": {"id": f"{NS}:stressometer", "count": 1},
    },
}


def textures():
    t = {
        "shaft": shaft_side(0),
        "shaft_end": shaft_end(),
        "cogwheel_disc": cog_wood("cog_disc"),
        "cogwheel_teeth": cog_wood("cog_teeth"),
        "water_wheel_paddle": planks("paddle"),
        "andesite_casing": casing_side("casing"),
        "andesite_casing_top": casing_top("casing_top"),
        "encased_shaft_end": casing_side("encased_end", hole=True),
        "gearbox_side": casing_side("gearbox", hole=True),
        "clutch_side": casing_side("clutch", hole=True, indicator=RED),
        "clutch_side_on": casing_side("clutch", hole=True, indicator=RED_ON),
        "gearshift_side": casing_side("gearshift", hole=True, indicator=BLUE),
        "gearshift_side_on": casing_side("gearshift", hole=True, indicator=BLUE_ON),
        "hand_crank": crank_wood(),
        "creative_motor_side": motor_side(),
        "creative_motor_front": motor_front(),
        "windmill_bearing_top": piston_top("bearing_top"),
        "white_sail": sail(),
        "sail_frame": sail_frame(),
        "mechanical_piston_top": piston_top("piston_top"),
        "piston_extension_pole": shaft_side(0, "pole"),
        "drill_head": drill_metal(0),
        "saw_blade": saw_blade(0),
        "speedometer_face": gauge_face("speed"),
        "stressometer_face": gauge_face("stress"),
    }
    anim = {
        "shaft_spinning": [shaft_side(p) for p in range(4)],
        "cogwheel_disc_spinning": [cog_wood("cog_disc", p) for p in range(5)],
        "drill_head_spinning": [drill_metal(p) for p in range(4)],
        "saw_blade_spinning": [saw_blade(p) for p in range(4)],
    }
    return t, anim


def write(path, data):
    full = os.path.join(ROOT, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    if isinstance(data, Image.Image):
        data.save(full, optimize=True)
        return
    with open(full, "w", encoding="utf-8") as f:
        if isinstance(data, str):
            f.write(data)
        else:
            json.dump(data, f, indent=2, ensure_ascii=False)
            f.write("\n")


def loot(id, drop):
    return {
        "type": "minecraft:block",
        "pools": [{
            "rolls": 1,
            "entries": [{"type": "minecraft:item", "name": f"{NS}:{drop}"}],
            "conditions": [{"condition": "minecraft:survives_explosion"}],
        }],
        "random_sequence": f"{NS}:blocks/{id}",
    }


def main():
    for sub in ("assets", "data"):
        shutil.rmtree(os.path.join(ROOT, sub), ignore_errors=True)
    en, de = {}, {}
    tools = {"pickaxe": [], "axe": []}
    for id, b in BLOCKS.items():
        decl = {"template": b["template"], "properties": b["props"]}
        if not b["item"]:
            decl["item"] = False
        write(f"data/{NS}/horizon/block/{id}.json", decl)
        if b["drops"]:
            write(f"data/{NS}/loot_table/blocks/{id}.json", loot(id, b["drops"]))
        if b["tool"]:
            tools[b["tool"]].append(f"{NS}:{id}")
        write(f"assets/{NS}/blockstates/{id}.json", b["states"])
        for name, m in b["models"].items():
            write(f"assets/{NS}/models/block/{name}.json", m)
        en[f"block.{NS}.{id}"], de[f"block.{NS}.{id}"] = b["names"]
        if b["item"]:
            m = ITEM_MODEL.get(id, id)
            write(f"assets/{NS}/items/{id}.json", {"model": {"type": "minecraft:model", "model": f"{NS}:block/{m}"}})
    for id, (template, names) in ITEMS.items():
        write(f"data/{NS}/horizon/item/{id}.json", {"template": template})
        write(f"assets/{NS}/items/{id}.json", {"model": {"type": "minecraft:model", "model": f"{NS}:item/{id}"}})
        write(f"assets/{NS}/models/item/{id}.json",
              {"parent": "minecraft:item/generated", "textures": {"layer0": f"{NS}:item/{id}"}})
        en[f"item.{NS}.{id}"], de[f"item.{NS}.{id}"] = names
    write(f"assets/{NS}/textures/item/andesite_alloy.png", alloy_item())
    for id, r in RECIPES.items():
        write(f"data/{NS}/recipe/{id}.json", r)
    write("data/minecraft/tags/block/mineable/pickaxe.json", {"values": sorted(tools["pickaxe"])})
    write("data/minecraft/tags/block/mineable/axe.json", {"values": sorted(tools["axe"])})
    still, anim = textures()
    for name, img in still.items():
        write(f"assets/{NS}/textures/block/{name}.png", img)
    for name, frames in anim.items():
        write(f"assets/{NS}/textures/block/{name}.png", animated(frames))
        write(f"assets/{NS}/textures/block/{name}.png.mcmeta", {"animation": {"frametime": 2}})
    write(f"assets/{NS}/lang/en_us.json", dict(sorted(en.items())))
    write(f"assets/{NS}/lang/de_de.json", dict(sorted(de.items())))


if __name__ == "__main__":
    main()
