# Create, ported to Horizon

[Create](https://github.com/Creators-of-Create/Create) is a NeoForge/Fabric mod about rotational
power: shafts and cogwheels carry rotation from water wheels, windmills and cranks to machines, and
every machine puts stress on its network. Horizon runs no Java mods (`docs/modpacks.md`), so this is a
port in Horizon's own two halves:

| Folder | Mod id | API | What it is |
|---|---|---|---|
| `content/` | `create` | 0.2, `registries` | the blocks and items: templates, models, textures, recipes, loot, tags, names (English and German). No code, so it loads everywhere |
| `kinetics/` | `create_kinetics` | 0.3, server, `players` `world` `storage` | the rules: rotation networks, speed, stress and the machines (Rust, WebAssembly) |
| `tools/generate.py` | | | writes every file under `content/` (run it after changing it; it is deterministic) |

Both halves run on iOS too: the content half needs no runtime, and the server half runs in wasmtime's
Pulley interpreter there (`docs/mods.md`, "Choosing the runtime").

## Installing

```sh
cd mods/ports/create/content && cargo horizon-mod build     # target/horizon-mod/create.hmod
cd ../kinetics && cargo horizon-mod build                    # target/horizon-mod/create_kinetics.hmod
```

Put both `.hmod` files in `horizon-mods/` beside `options.txt` (on an iPhone or iPad: `Documents/horizon-mods/`
through the Files app; on a dedicated server: the server folder's `horizon-mods/`) and turn both on in
Options > Mods, or with `mod.create:true` and `mod.create_kinetics:true` in `horizon-mods.txt`. A server
and its players need the content half; only the server (or the single-player game) runs the kinetics.

## What is in it

| Block | Recipe | Does |
|---|---|---|
| Andesite Alloy (item) | andesite and iron nuggets, crossed 2x2 | the material of everything below |
| Shaft | 2 andesite alloy in a column: 8 | carries rotation along its axis |
| Cogwheel, Large Cogwheel | shaft + planks; shaft + 2 planks, or cogwheel + planks | carry rotation along their axis and mesh: small beside small (reverses), large to small at a corner (2:1) |
| Andesite Casing, Encased Shaft | alloy over a log: 2; casing + shaft | a shaft in a casing |
| Gearbox | casing + cogwheel | turns rotation round corners; straight through it reverses |
| Clutch, Gearshift | casing + shaft + redstone; casing + cogwheel + redstone | powered: a clutch cuts its shafts, a gearshift reverses them |
| Hand Crank | 3 planks over alloy, shaped | toggled on (use it), turns at 32 RPM with 256 SU, into the block it is on |
| Water Wheel | 8 planks round a shaft | turned by flowing or falling water on its rim: 8 RPM, 256 SU |
| Windmill Bearing, Sail Frame, Sail | wooden slab, casing, shaft in a column; 8 sticks round alloy: 8; frame + white wool | a bearing with at least 8 sails or frames joined in front of it turns at 1 RPM per 8 sails up to 16, 512 SU per RPM, from its back |
| Creative Motor (no recipe) | | any speed: `/create motor <pos> <rpm>` (default 16, -256 to 256), unlimited stress |
| Mechanical Drill | alloy round an iron ingot over a casing | fed from the back, breaks the block in front (drops as mined), faster the faster it turns and the softer the block; 4 SU per RPM |
| Mechanical Saw | 3 iron ingots over alloy, casing, alloy | fells the whole tree in front (logs and its leaves), cuts cactus, sugar cane, bamboo and kelp above the lowest block, melons and pumpkins; 4 SU per RPM |
| Mechanical Piston, Piston Extension Pole | piston, casing, shaft in a column; alloy, planks, alloy: 8 | fed from a side; poles in line behind it give its reach. One way out, the other way in, pushing one block ahead and pulling it back (blocks with inventories and machines don't move); breaking the piston breaks its extended poles; 4 SU per RPM |
| Speedometer, Stressometer | casing + compass; speedometer + redstone | shafts that read out with `/create info` |

Every network turns at the speed of its fastest source, with the ratios of its gears, up to 256 RPM.
It stops if its stress (the sum of each machine's SU per RPM times its speed) is more than its sources
carry, or if it is jammed: gears that disagree in a loop, or sources turning against each other.
Turning blocks swap to `_spinning` twins whose textures move, and drop the block itself.

Commands (anyone may use them): `/create info [pos]` (the machine at `pos` or the nearest one:
speed, direction, network size, stress), `/create motor <pos> <rpm>`, `/create networks`, `/create help`.

## What is not (yet)

Create's moving parts are drawn by its own renderer, with block entities and contraptions that rotate
and move whole structures; Horizon's mods add blocks from vanilla templates and cannot draw models that
turn, so here a turning block shows its motion in its texture, a windmill's sails stand still while it
turns, and a piston moves its blocks step by step instead of gliding. Belts, deployers, fans, presses,
mixers, basins, encased fans, trains, fluids, the Ponder system, goggles and the item and stress
overlays are not ported. Speeds per machine and stress values follow the Create wiki's numbers for
1.21 at default config.

## Licence and sources

Create's code is MIT; its assets are "All Rights Reserved"
([LICENSE.md](https://github.com/Creators-of-Create/Create/blob/mc1.21.1/dev/LICENSE.md)). This port
uses neither: Horizon ports mods from what they do, as documented and as players see it, never from
their code (`docs/modpacks.md`), and every texture here is drawn by `tools/generate.py` from scratch. The
behaviour follows the [Create wiki](https://wiki.createmod.net/). "Create" is the Create team's; this port
is not made or endorsed by them.

## Tests

```sh
cargo test -p create-kinetics                                         # the rules, natively
cd mods/ports/create/kinetics && cargo horizon-mod test               # in the test server
HORIZON_MODS_INTERPRET=1 cargo horizon-mod test                       # as an iPhone runs it
cargo test -p horizon-client --no-default-features --lib create_port  # the content over 26.3's resources
```
