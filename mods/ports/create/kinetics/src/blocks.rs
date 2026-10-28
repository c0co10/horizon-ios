//! Block geometry and the Create blocks as the server spells them
//! (`create:shaft[axis=y,waterlogged=false]`): what each one is, and where
//! it takes and gives rotation.

pub type Pos = (i32, i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    pub fn parse(s: &str) -> Option<Axis> {
        match s {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
    /// The two directions along this axis, positive first.
    pub fn dirs(self) -> [Dir; 2] {
        match self {
            Axis::X => [Dir::East, Dir::West],
            Axis::Y => [Dir::Up, Dir::Down],
            Axis::Z => [Dir::South, Dir::North],
        }
    }
    /// The other two axes, in X, Y, Z order.
    pub fn others(self) -> [Axis; 2] {
        match self {
            Axis::X => [Axis::Y, Axis::Z],
            Axis::Y => [Axis::X, Axis::Z],
            Axis::Z => [Axis::X, Axis::Y],
        }
    }
    pub fn unit(self) -> [i32; 3] {
        let mut v = [0; 3];
        v[self.index()] = 1;
        v
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Dir {
    pub const ALL: [Dir; 6] = [
        Dir::Down,
        Dir::Up,
        Dir::North,
        Dir::South,
        Dir::West,
        Dir::East,
    ];

    pub fn parse(s: &str) -> Option<Dir> {
        match s {
            "down" => Some(Dir::Down),
            "up" => Some(Dir::Up),
            "north" => Some(Dir::North),
            "south" => Some(Dir::South),
            "west" => Some(Dir::West),
            "east" => Some(Dir::East),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Dir::Down => "down",
            Dir::Up => "up",
            Dir::North => "north",
            Dir::South => "south",
            Dir::West => "west",
            Dir::East => "east",
        }
    }
    pub fn vec(self) -> [i32; 3] {
        match self {
            Dir::Down => [0, -1, 0],
            Dir::Up => [0, 1, 0],
            Dir::North => [0, 0, -1],
            Dir::South => [0, 0, 1],
            Dir::West => [-1, 0, 0],
            Dir::East => [1, 0, 0],
        }
    }
    pub fn axis(self) -> Axis {
        match self {
            Dir::Down | Dir::Up => Axis::Y,
            Dir::North | Dir::South => Axis::Z,
            Dir::West | Dir::East => Axis::X,
        }
    }
    /// +1 for up, south, east; -1 for the others.
    pub fn sign(self) -> i32 {
        match self {
            Dir::Up | Dir::South | Dir::East => 1,
            _ => -1,
        }
    }
    pub fn opposite(self) -> Dir {
        match self {
            Dir::Down => Dir::Up,
            Dir::Up => Dir::Down,
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
            Dir::East => Dir::West,
        }
    }
    pub fn step(self, p: Pos) -> Pos {
        offset(p, self, 1)
    }
}

pub fn offset(p: Pos, d: Dir, n: i32) -> Pos {
    let v = d.vec();
    (p.0 + v[0] * n, p.1 + v[1] * n, p.2 + v[2] * n)
}

pub fn add(p: Pos, v: [i32; 3]) -> Pos {
    (p.0 + v[0], p.1 + v[1], p.2 + v[2])
}

pub fn cross(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A state string split: `minecraft:oak_log[axis=y]` is
/// (`minecraft:oak_log`, [(`axis`, `y`)]).
pub fn split(state: &str) -> (&str, Vec<(&str, &str)>) {
    let Some(open) = state.find('[') else {
        return (state, Vec::new());
    };
    let name = &state[..open];
    let inner = state[open + 1..].trim_end_matches(']');
    let props = inner
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .collect();
    (name, props)
}

pub fn prop<'a>(props: &[(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    props.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// `state` with its block renamed, its properties kept.
pub fn rename(state: &str, name: &str) -> String {
    match state.find('[') {
        Some(open) => format!("{name}{}", &state[open..]),
        None => name.to_string(),
    }
}

pub const NS: &str = "create:";
pub const SPINNING: &str = "_spinning";

/// The Create blocks that take part in rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Shafts and what behaves like one: an encased shaft, the gauges.
    Shaft {
        axis: Axis,
        gauge: Gauge,
    },
    Cog {
        axis: Axis,
        large: bool,
    },
    WaterWheel {
        axis: Axis,
    },
    Gearbox {
        axis: Axis,
    },
    /// Three straight shafts, one per axis; powered, none.
    Clutch {
        powered: bool,
    },
    /// Three straight shafts, one per axis; powered, each one reverses.
    Gearshift {
        powered: bool,
    },
    /// Attached to the block on its `attached` side; turned on, it turns.
    Crank {
        attached: Dir,
        on: bool,
    },
    Motor {
        facing: Dir,
    },
    Bearing {
        facing: Dir,
    },
    Drill {
        facing: Dir,
    },
    Saw {
        facing: Dir,
    },
    Piston {
        facing: Dir,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gauge {
    None,
    Speed,
    Stress,
}

impl Kind {
    /// The Create block a state is, `None` for any other block.
    pub fn of(state: &str) -> Option<Kind> {
        let (name, props) = split(state);
        let id = name.strip_prefix(NS)?;
        let id = id.strip_suffix(SPINNING).unwrap_or(id);
        let axis = || {
            prop(&props, "axis")
                .and_then(Axis::parse)
                .unwrap_or(Axis::Y)
        };
        let facing = || {
            prop(&props, "facing")
                .and_then(Dir::parse)
                .unwrap_or(Dir::North)
        };
        let lit = || prop(&props, "lit") == Some("true");
        Some(match id {
            "shaft" | "encased_shaft" => Kind::Shaft {
                axis: axis(),
                gauge: Gauge::None,
            },
            "speedometer" => Kind::Shaft {
                axis: axis(),
                gauge: Gauge::Speed,
            },
            "stressometer" => Kind::Shaft {
                axis: axis(),
                gauge: Gauge::Stress,
            },
            "cogwheel" => Kind::Cog {
                axis: axis(),
                large: false,
            },
            "large_cogwheel" => Kind::Cog {
                axis: axis(),
                large: true,
            },
            "water_wheel" => Kind::WaterWheel { axis: axis() },
            "gearbox" => Kind::Gearbox { axis: axis() },
            "clutch" => Kind::Clutch { powered: lit() },
            "gearshift" => Kind::Gearshift { powered: lit() },
            "hand_crank" => {
                let attached = match prop(&props, "face") {
                    Some("floor") => Dir::Down,
                    Some("ceiling") => Dir::Up,
                    _ => facing().opposite(),
                };
                Kind::Crank {
                    attached,
                    on: prop(&props, "powered") == Some("true"),
                }
            }
            "creative_motor" => Kind::Motor { facing: facing() },
            "windmill_bearing" => Kind::Bearing { facing: facing() },
            "mechanical_drill" => Kind::Drill { facing: facing() },
            "mechanical_saw" => Kind::Saw { facing: facing() },
            "mechanical_piston" => Kind::Piston { facing: facing() },
            _ => return None,
        })
    }

    /// Where the block meets its neighbours' rotation: (face, part, sign).
    /// The part is one of the block's independent rotating parts (a clutch
    /// has one per axis); the sign `k` says how the part's speed `x` reads
    /// from that face: `k * x` is the turn about the face's outward normal.
    /// Two faces touching turn alike about their shared normal, so across
    /// a link `kA * xA = -kB * xB`.
    pub fn ports(self) -> Vec<(Dir, u8, i8)> {
        let shaft = |axis: Axis, part: u8| {
            let [pos, neg] = axis.dirs();
            [(pos, part, 1), (neg, part, -1)]
        };
        let gearbox = |axis: Axis| {
            // Bevel gears round the axis: opposite faces alike about their
            // outward normal (so a straight path reverses), neighbours not.
            let [u, v] = axis.others();
            let [u1, u2] = u.dirs();
            let [v1, v2] = v.dirs();
            vec![(u1, 0, 1), (u2, 0, 1), (v1, 0, -1), (v2, 0, -1)]
        };
        match self {
            Kind::Shaft { axis, .. } | Kind::Cog { axis, .. } | Kind::WaterWheel { axis } => {
                shaft(axis, 0).to_vec()
            }
            Kind::Gearbox { axis } => gearbox(axis),
            Kind::Piston { facing } => gearbox(facing.axis()),
            Kind::Clutch { powered: true } => Vec::new(),
            Kind::Clutch { powered: false } | Kind::Gearshift { powered: false } => Axis::ALL
                .iter()
                .flat_map(|&a| shaft(a, a.index() as u8))
                .collect(),
            Kind::Gearshift { powered: true } => Axis::ALL
                .iter()
                .flat_map(|&a| {
                    let [pos, neg] = a.dirs();
                    [(pos, a.index() as u8, 1), (neg, a.index() as u8, 1)]
                })
                .collect(),
            Kind::Crank { attached, .. } => vec![(attached, 0, 1)],
            Kind::Motor { facing } => vec![(facing, 0, 1)],
            Kind::Bearing { facing } | Kind::Drill { facing } | Kind::Saw { facing } => {
                vec![(facing.opposite(), 0, 1)]
            }
        }
    }

    /// Stress per RPM the block puts on its network.
    pub fn impact(self) -> f64 {
        match self {
            Kind::Drill { .. } | Kind::Saw { .. } | Kind::Piston { .. } => 4.0,
            _ => 0.0,
        }
    }

    /// The block has a `_spinning` twin shown while it turns.
    pub fn spins(self) -> bool {
        matches!(
            self,
            Kind::Shaft {
                gauge: Gauge::None,
                ..
            } | Kind::Cog { .. }
                | Kind::WaterWheel { .. }
                | Kind::Drill { .. }
                | Kind::Saw { .. }
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Shaft {
                gauge: Gauge::Speed,
                ..
            } => "Speedometer",
            Kind::Shaft {
                gauge: Gauge::Stress,
                ..
            } => "Stressometer",
            Kind::Shaft { .. } => "Shaft",
            Kind::Cog { large: true, .. } => "Large Cogwheel",
            Kind::Cog { .. } => "Cogwheel",
            Kind::WaterWheel { .. } => "Water Wheel",
            Kind::Gearbox { .. } => "Gearbox",
            Kind::Clutch { .. } => "Clutch",
            Kind::Gearshift { .. } => "Gearshift",
            Kind::Crank { .. } => "Hand Crank",
            Kind::Motor { .. } => "Creative Motor",
            Kind::Bearing { .. } => "Windmill Bearing",
            Kind::Drill { .. } => "Mechanical Drill",
            Kind::Saw { .. } => "Mechanical Saw",
            Kind::Piston { .. } => "Mechanical Piston",
        }
    }
}

/// The name of a block, its `_spinning` twin taken back to the block.
pub fn base_name(state: &str) -> &str {
    let (name, _) = split(state);
    name.strip_suffix(SPINNING).unwrap_or(name)
}

pub fn is_spinning(state: &str) -> bool {
    split(state).0.ends_with(SPINNING)
}

/// `state` shown turning or still.
pub fn with_spin(state: &str, spinning: bool) -> String {
    let base = base_name(state).to_string();
    if spinning {
        rename(state, &format!("{base}{SPINNING}"))
    } else {
        rename(state, &base)
    }
}

pub const POLE: &str = "create:piston_extension_pole";
pub const HEAD: &str = "create:mechanical_piston_head";
pub const SAILS: [&str; 2] = ["create:white_sail", "create:sail_frame"];

pub fn is_air(state: &str) -> bool {
    matches!(
        split(state).0,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// Water and lava, which break nothing and are not broken.
pub fn is_fluid(state: &str) -> bool {
    matches!(
        split(state).0,
        "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column"
    )
}

/// A block a piston head or a pushed block may take the place of.
pub fn replaceable(state: &str) -> bool {
    is_air(state)
        || is_fluid(state)
        || matches!(
            split(state).0,
            "minecraft:short_grass"
                | "minecraft:fern"
                | "minecraft:dead_bush"
                | "minecraft:vine"
                | "minecraft:fire"
                | "minecraft:soul_fire"
                | "minecraft:snow"
                | "minecraft:short_dry_grass"
                | "minecraft:bush"
        )
}

/// How long a block takes to break by hand, as vanilla's hardness, by name:
/// `None` for a block no machine breaks.
pub fn hardness(state: &str) -> Option<f64> {
    let name = split(state).0;
    if is_air(name) || is_fluid(name) {
        return None;
    }
    let id = name.split_once(':').map_or(name, |(_, id)| id);
    const NEVER: [&str; 16] = [
        "bedrock",
        "barrier",
        "light",
        "structure_void",
        "end_portal",
        "end_portal_frame",
        "end_gateway",
        "nether_portal",
        "command_block",
        "chain_command_block",
        "repeating_command_block",
        "structure_block",
        "jigsaw",
        "reinforced_deepslate",
        "test_block",
        "test_instance_block",
    ];
    if NEVER.contains(&id) || id.starts_with("moving_piston") {
        return None;
    }
    if name.starts_with(NS) {
        return Some(1.5);
    }
    let h = if id.contains("obsidian") {
        50.0
    } else if id == "ancient_debris" {
        30.0
    } else if id.contains("deepslate") && id.contains("ore") {
        4.5
    } else if id.ends_with("_ore")
        || id.contains("netherite_block")
        || id.starts_with("deepslate")
        || id == "cobbled_deepslate"
    {
        3.0
    } else if id.ends_with("_block")
        && ["iron", "gold", "diamond", "emerald", "copper", "lapis"]
            .iter()
            .any(|m| id.contains(m))
    {
        5.0
    } else if id.contains("leaves") {
        0.2
    } else if id.contains("glass") {
        0.3
    } else if id.contains("wool") || id.contains("carpet") {
        0.8
    } else if [
        "dirt",
        "grass_block",
        "sand",
        "red_sand",
        "farmland",
        "dirt_path",
        "coarse_dirt",
        "rooted_dirt",
        "podzol",
        "mycelium",
        "mud",
        "clay",
        "soul_sand",
        "soul_soil",
        "gravel",
    ]
    .contains(&id)
    {
        0.6
    } else if id.contains("netherrack") {
        0.4
    } else if id.ends_with("_log")
        || id.ends_with("_wood")
        || id.ends_with("_planks")
        || id.ends_with("_stem")
        || id.ends_with("_hyphae")
    {
        2.0
    } else if id.contains("stone")
        || id.contains("andesite")
        || id.contains("diorite")
        || id.contains("granite")
        || id.contains("tuff")
        || id.contains("calcite")
        || id.contains("basalt")
        || id.contains("brick")
    {
        1.5
    } else if id.contains("flower")
        || id.contains("grass")
        || id.contains("fern")
        || id.contains("sapling")
        || id.contains("torch")
        || id.contains("mushroom")
    {
        0.0
    } else {
        1.0
    };
    Some(h)
}

/// A tree's block a saw fells.
pub fn is_log(state: &str) -> bool {
    let name = split(state).0;
    name.starts_with("minecraft:")
        && !name.contains("stripped")
        && (name.ends_with("_log")
            || name.ends_with("_wood")
            || name.ends_with("_stem")
            || name.ends_with("_hyphae"))
}

pub fn is_leaves(state: &str) -> bool {
    let name = split(state).0;
    name.ends_with("_leaves") || name.ends_with("_wart_block") || name == "minecraft:shroomlight"
}

/// Plants a saw cuts, leaving the lowest block to grow again.
pub fn is_stalk(state: &str) -> bool {
    matches!(
        split(state).0,
        "minecraft:cactus" | "minecraft:sugar_cane" | "minecraft:bamboo" | "minecraft:kelp_plant"
    )
}

pub fn is_gourd(state: &str) -> bool {
    matches!(split(state).0, "minecraft:melon" | "minecraft:pumpkin")
}

/// A block a mechanical piston may push or pull: whole, without a block
/// entity, not part of a machine.
pub fn movable(state: &str) -> bool {
    let name = split(state).0;
    if is_air(name) || hardness(name).is_none() || name.starts_with(NS) {
        return false;
    }
    let id = name.split_once(':').map_or(name, |(_, id)| id);
    const KEEP: [&str; 24] = [
        "obsidian",
        "crying_obsidian",
        "respawn_anchor",
        "chest",
        "trapped_chest",
        "ender_chest",
        "barrel",
        "furnace",
        "blast_furnace",
        "smoker",
        "hopper",
        "dispenser",
        "dropper",
        "beacon",
        "spawner",
        "trial_spawner",
        "vault",
        "lectern",
        "enchanting_table",
        "brewing_stand",
        "jukebox",
        "piston_head",
        "moving_piston",
        "crafter",
    ];
    !(KEEP.contains(&id)
        || id.ends_with("_door")
        || id.ends_with("_bed")
        || id.ends_with("shulker_box")
        || id.ends_with("_sign")
        || id.ends_with("_banner")
        || id.contains("piston")
        || id.contains("campfire")
        || id.contains("skull")
        || id.contains("head")
        || id.starts_with("tall_")
        || id.ends_with("bush")
        || matches!(
            id,
            "sunflower" | "lilac" | "rose_bush" | "peony" | "large_fern"
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_split_and_rename() {
        let s = "create:shaft[axis=x,waterlogged=false]";
        assert_eq!(split(s).0, "create:shaft");
        assert_eq!(prop(&split(s).1, "axis"), Some("x"));
        assert_eq!(
            with_spin(s, true),
            "create:shaft_spinning[axis=x,waterlogged=false]"
        );
        assert_eq!(with_spin(&with_spin(s, true), false), s);
        assert_eq!(with_spin("create:gearbox", true), "create:gearbox_spinning");
    }

    #[test]
    fn blocks_are_read() {
        assert_eq!(
            Kind::of("create:cogwheel_spinning[axis=z,waterlogged=false]"),
            Some(Kind::Cog {
                axis: Axis::Z,
                large: false
            })
        );
        assert_eq!(
            Kind::of("create:hand_crank[face=wall,facing=north,powered=true]"),
            Some(Kind::Crank {
                attached: Dir::South,
                on: true
            })
        );
        assert_eq!(Kind::of("minecraft:stone"), None);
        assert_eq!(Kind::of("create:andesite_casing"), None);
    }

    #[test]
    fn hardness_by_name() {
        assert_eq!(hardness("minecraft:bedrock"), None);
        assert_eq!(hardness("minecraft:water[level=0]"), None);
        assert_eq!(hardness("minecraft:stone"), Some(1.5));
        assert_eq!(hardness("minecraft:dirt"), Some(0.6));
        assert_eq!(hardness("minecraft:oak_log[axis=y]"), Some(2.0));
        assert!(movable("minecraft:stone"));
        assert!(!movable(
            "minecraft:chest[facing=north,type=single,waterlogged=false]"
        ));
        assert!(!movable("create:shaft[axis=y,waterlogged=false]"));
    }
}
