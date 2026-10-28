//! The kinetics of one dimension, apart from the host: networks found from
//! the blocks, solved for speed and stress, and the machines they drive.
//! The host is a [`World`] it reads and a list of [`Action`]s it returns,
//! so the whole of it runs in native tests.

use crate::blocks::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// Read access to a dimension's blocks.
pub trait World {
    /// The state at `p`; `None` where no chunk is loaded.
    fn get(&mut self, p: Pos) -> Option<String>;
}

/// What the mod asks of the server after a step.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Set(Pos, String),
    Destroy(Pos),
}

/// Fastest a network turns, as in Create.
pub const MAX_RPM: f64 = 256.0;
pub const CRANK_RPM: f64 = 32.0;
pub const CRANK_SU: f64 = 256.0;
pub const WATER_RPM: f64 = 8.0;
pub const WATER_SU: f64 = 256.0;
pub const WINDMILL_MAX_RPM: f64 = 16.0;
pub const WINDMILL_SU_PER_RPM: f64 = 512.0;
pub const MOTOR_DEFAULT_RPM: i32 = 16;
pub const MOTOR_SU: f64 = 1.0e9;
/// Sails a windmill needs to turn, and the most it counts.
pub const MIN_SAILS: usize = 8;
const MAX_SAILS: usize = 512;
/// A network bigger than this is not solved (it stays still).
pub const MAX_NODES: usize = 4096;
/// Ticks between solves: machines move by this many ticks each time.
pub const PERIOD: u64 = 4;
const WINDMILL_RECOUNT: u64 = 40;
/// Breaking progress per tick: `rpm / (DIG * hardness)`.
const DIG: f64 = 800.0;
/// Piston travel per tick, in blocks: `rpm / PISTON`.
const PISTON: f64 = 512.0;
/// Most logs and leaves a saw fells at once.
const TREE: usize = 256;

/// How a network is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    /// Turning, or still with nothing to turn it.
    Ok,
    /// More stress than its sources carry: stopped.
    Overstressed,
    /// Its gears disagree (a loop of mismatched ratios) or its sources turn
    /// against each other: jammed.
    Jammed,
    /// More than [`MAX_NODES`] parts.
    TooBig,
}

#[derive(Clone, Debug)]
pub struct Network {
    pub parts: usize,
    /// The fastest part, in RPM.
    pub speed: f64,
    pub stress: f64,
    pub capacity: f64,
    pub status: Status,
}

/// One part's result after a solve.
#[derive(Clone, Copy, Debug)]
pub struct Turning {
    /// RPM of the block's part 0 (its own frame: about the axis' positive
    /// direction for a shaft, about its outward port for a machine).
    pub rpm: f64,
    pub network: usize,
}

#[derive(Default)]
pub struct Dimension {
    /// Positions that held a Create block when last seen.
    pub tracked: BTreeSet<Pos>,
    /// Creative motors' speeds set with `/create motor`.
    pub motors: BTreeMap<Pos, i32>,
    pub turning: HashMap<Pos, Turning>,
    pub networks: Vec<Network>,
    drills: HashMap<Pos, (Pos, String, f64)>,
    pistons: HashMap<Pos, f64>,
    windmills: HashMap<Pos, (u64, usize)>,
}

/// Reads of one step, each position read once.
struct Reads<'w, W: World> {
    world: &'w mut W,
    seen: HashMap<Pos, Option<String>>,
}

impl<W: World> Reads<'_, W> {
    fn get(&mut self, p: Pos) -> Option<&str> {
        if !self.seen.contains_key(&p) {
            let s = self.world.get(p);
            self.seen.insert(p, s);
        }
        self.seen[&p].as_deref()
    }
    fn kind(&mut self, p: Pos) -> Option<Kind> {
        self.get(p).and_then(Kind::of)
    }
    /// After an action this step: later reads see it.
    fn put(&mut self, p: Pos, s: &str) {
        self.seen.insert(p, Some(s.to_string()));
    }
}

type Node = (Pos, u8);

impl Dimension {
    /// A block placed or seen: follow it from the next step.
    pub fn track(&mut self, p: Pos) {
        self.tracked.insert(p);
    }

    /// A creative motor's speed (`/create motor`).
    pub fn set_motor(&mut self, p: Pos, rpm: i32) {
        self.motors.insert(p, rpm);
        self.tracked.insert(p);
    }

    /// One step: networks solved, machines moved by [`PERIOD`] ticks.
    pub fn step(&mut self, world: &mut impl World, tick: u64) -> Vec<Action> {
        let mut reads = Reads {
            world,
            seen: HashMap::new(),
        };
        let mut actions = Vec::new();
        self.solve(&mut reads, tick);
        self.show(&mut reads, &mut actions);
        self.run_machines(&mut reads, &mut actions);
        actions
    }

    fn solve(&mut self, r: &mut Reads<impl World>, tick: u64) {
        self.turning.clear();
        self.networks.clear();
        let mut done: BTreeSet<Pos> = BTreeSet::new();
        let starts: Vec<Pos> = self.tracked.iter().copied().collect();
        for start in starts {
            if done.contains(&start) {
                continue;
            }
            match r.get(start) {
                None => continue,
                Some(s) if Kind::of(s).is_none() => {
                    self.tracked.remove(&start);
                    self.motors.remove(&start);
                    continue;
                }
                _ => {}
            }
            self.solve_network(r, start, &mut done, tick);
        }
        self.drills.retain(|p, _| done.contains(p));
        self.pistons.retain(|p, _| done.contains(p));
        self.windmills.retain(|p, _| done.contains(p));
    }

    /// The network holding `start`: its parts found, their ratios checked,
    /// its speed set by its sources, its stress weighed.
    fn solve_network(
        &mut self,
        r: &mut Reads<impl World>,
        start: Pos,
        done: &mut BTreeSet<Pos>,
        tick: u64,
    ) {
        let root: Node = (start, first_part(r.kind(start).unwrap()));
        // Each part's speed as a factor of the root's.
        let mut factor: HashMap<Node, f64> = HashMap::new();
        let mut kinds: BTreeMap<Pos, Kind> = BTreeMap::new();
        let mut queue = VecDeque::new();
        factor.insert(root, 1.0);
        queue.push_back(root);
        let mut jammed = false;
        let mut too_big = false;
        while let Some(node @ (p, part)) = queue.pop_front() {
            let Some(kind) = r.kind(p) else { continue };
            kinds.insert(p, kind);
            if kinds.len() > MAX_NODES {
                too_big = true;
                break;
            }
            let f = factor[&node];
            let mut links: Vec<(Node, f64)> = Vec::new();
            for (face, prt, k) in kind.ports() {
                if prt != part {
                    continue;
                }
                let q = face.step(p);
                let Some(other) = r.kind(q) else { continue };
                if let Some((_, qp, kq)) = other
                    .ports()
                    .into_iter()
                    .find(|(f2, _, _)| *f2 == face.opposite())
                {
                    links.push(((q, qp), -(k as f64) * (kq as f64)));
                }
            }
            if let Kind::Cog { axis, large } = kind {
                for (q, ratio) in meshes(r, p, axis, large) {
                    links.push(((q, 0), ratio));
                }
            }
            for (next, ratio) in links {
                let want = f * ratio;
                match factor.get(&next) {
                    Some(have) => {
                        if (have - want).abs() > 1e-9 {
                            jammed = true;
                        }
                    }
                    None => {
                        factor.insert(next, want);
                        queue.push_back(next);
                    }
                }
            }
        }
        for &p in kinds.keys() {
            done.insert(p);
            self.tracked.insert(p);
        }
        let id = self.networks.len();
        // Sources: the root speed each one asks for, and what it carries.
        let mut root_speed: f64 = 0.0;
        let mut capacity = 0.0;
        for (&node, &f) in &factor {
            let Some(&kind) = kinds.get(&node.0) else {
                continue;
            };
            let (rpm, su) = self.source(r, node.0, kind, tick);
            if rpm == 0.0 {
                continue;
            }
            capacity += su;
            let want = rpm / f;
            if root_speed != 0.0 && want.signum() != root_speed.signum() {
                jammed = true;
            }
            if want.abs() > root_speed.abs() {
                root_speed = want;
            }
        }
        let fastest = factor.values().fold(0.0f64, |m, f| m.max(f.abs()));
        if fastest * root_speed.abs() > MAX_RPM {
            root_speed = root_speed.signum() * MAX_RPM / fastest;
        }
        let mut stress = 0.0;
        for (&(p, _), &f) in &factor {
            if let Some(kind) = kinds.get(&p) {
                stress += kind.impact() * (f * root_speed).abs();
            }
        }
        let status = if too_big {
            Status::TooBig
        } else if jammed {
            Status::Jammed
        } else if stress > capacity + 1e-9 && root_speed != 0.0 {
            Status::Overstressed
        } else {
            Status::Ok
        };
        if status != Status::Ok {
            root_speed = 0.0;
        }
        for (&(p, part), &f) in &factor {
            if !kinds.contains_key(&p) {
                continue;
            }
            let rpm = f * root_speed;
            let entry = self.turning.entry(p).or_insert(Turning {
                rpm: 0.0,
                network: id,
            });
            if part == first_part(kinds[&p]) || entry.rpm == 0.0 {
                *entry = Turning { rpm, network: id };
            }
        }
        self.networks.push(Network {
            parts: kinds.len(),
            speed: fastest * root_speed.abs(),
            stress,
            capacity,
            status,
        });
    }

    /// The speed a source turns its part at (its own frame), and the stress
    /// it carries then; (0, 0) for a block that makes no rotation.
    fn source(&mut self, r: &mut Reads<impl World>, p: Pos, kind: Kind, tick: u64) -> (f64, f64) {
        match kind {
            Kind::Crank { on: true, .. } => (CRANK_RPM, CRANK_SU),
            Kind::Motor { .. } => {
                let rpm = *self.motors.get(&p).unwrap_or(&MOTOR_DEFAULT_RPM) as f64;
                (rpm, if rpm == 0.0 { 0.0 } else { MOTOR_SU })
            }
            Kind::WaterWheel { axis } => (water_wheel(r, p, axis) * WATER_RPM, WATER_SU),
            Kind::Bearing { facing } => {
                let sails = match self.windmills.get(&p) {
                    Some(&(at, n)) if tick < at + WINDMILL_RECOUNT => n,
                    _ => {
                        let n = count_sails(r, facing.step(p));
                        self.windmills.insert(p, (tick, n));
                        n
                    }
                };
                if sails < MIN_SAILS {
                    return (0.0, 0.0);
                }
                let rpm = ((sails / MIN_SAILS) as f64).min(WINDMILL_MAX_RPM);
                (rpm, rpm * WINDMILL_SU_PER_RPM)
            }
            _ => (0.0, 0.0),
        }
    }

    /// Turning blocks shown turning, still ones still.
    fn show(&mut self, r: &mut Reads<impl World>, actions: &mut Vec<Action>) {
        let mut todo: Vec<(Pos, bool)> = self
            .turning
            .iter()
            .map(|(&p, t)| (p, t.rpm != 0.0))
            .collect();
        todo.sort_by_key(|&(p, _)| p);
        for (p, spin) in todo {
            let Some(state) = r.get(p).map(str::to_string) else {
                continue;
            };
            let Some(kind) = Kind::of(&state) else {
                continue;
            };
            if kind.spins() && is_spinning(&state) != spin {
                let new = with_spin(&state, spin);
                r.put(p, &new);
                actions.push(Action::Set(p, new));
            }
        }
    }

    fn run_machines(&mut self, r: &mut Reads<impl World>, actions: &mut Vec<Action>) {
        let mut machines: Vec<(Pos, f64)> = self.turning.iter().map(|(&p, t)| (p, t.rpm)).collect();
        machines.sort_by_key(|&(p, _)| p);
        for (p, rpm) in machines {
            match r.kind(p) {
                Some(Kind::Drill { facing }) => self.drill(r, p, facing, rpm, false, actions),
                Some(Kind::Saw { facing }) => self.drill(r, p, facing, rpm, true, actions),
                Some(Kind::Piston { facing }) => self.piston(r, p, facing, rpm, actions),
                _ => {}
            }
        }
    }

    /// A drill digs the block in front of it; a saw fells the tree in front.
    fn drill(
        &mut self,
        r: &mut Reads<impl World>,
        p: Pos,
        facing: Dir,
        rpm: f64,
        saw: bool,
        actions: &mut Vec<Action>,
    ) {
        let target = facing.step(p);
        let Some(state) = r.get(target).map(str::to_string) else {
            return;
        };
        let cuts = if saw {
            is_log(&state) || is_leaves(&state) || is_stalk(&state) || is_gourd(&state)
        } else {
            true
        };
        let hard = if cuts { hardness(&state) } else { None };
        let (Some(hard), true) = (hard, rpm != 0.0) else {
            self.drills.remove(&p);
            return;
        };
        let entry = self.drills.entry(p).or_insert((target, state.clone(), 0.0));
        if entry.0 != target || entry.1 != state {
            *entry = (target, state.clone(), 0.0);
        }
        entry.2 += PERIOD as f64 * rpm.abs() / (DIG * hard.max(0.05));
        if entry.2 < 1.0 {
            return;
        }
        self.drills.remove(&p);
        let broken = if !saw {
            vec![target]
        } else if is_stalk(&state) {
            stalk(r, target, &state)
        } else if is_log(&state) {
            tree(r, target)
        } else {
            vec![target]
        };
        for b in broken {
            r.put(b, "minecraft:air");
            actions.push(Action::Destroy(b));
        }
    }

    /// A mechanical piston moves its head out (turning one way) or in (the
    /// other), its poles carried from behind it to in front and back.
    fn piston(
        &mut self,
        r: &mut Reads<impl World>,
        p: Pos,
        facing: Dir,
        rpm: f64,
        actions: &mut Vec<Action>,
    ) {
        if rpm == 0.0 {
            return;
        }
        let pole = |r: &mut Reads<_>, q: Pos| {
            r.get(q).is_some_and(|s| {
                let (name, props) = split(s);
                name == POLE && prop(&props, "axis") == Some(facing.axis().name())
            })
        };
        let mut back = 0;
        while back < 64 && pole(r, offset(p, facing.opposite(), back + 1)) {
            back += 1;
        }
        let mut front = 0;
        while front < 64 && pole(r, offset(p, facing, front + 1)) {
            front += 1;
        }
        let head_at = offset(p, facing, front + 1);
        let head = r.get(head_at).is_some_and(|s| {
            let (name, props) = split(s);
            name == HEAD && prop(&props, "facing") == Some(facing.name())
        });
        let extension = if head { front + 1 } else { 0 };
        let poles = back + if head { front } else { 0 };
        let progress = self.pistons.entry(p).or_insert(0.0);
        *progress += PERIOD as f64 * rpm.abs() / PISTON;
        if *progress < 1.0 {
            return;
        }
        *progress = (*progress - 1.0).min(0.99);
        let head_state = format!("{HEAD}[facing={}]", facing.name());
        let pole_state = format!("{POLE}[axis={},waterlogged=false]", facing.axis().name());
        let air = "minecraft:air".to_string();
        let moves: Vec<(Pos, String)> = if rpm > 0.0 {
            // Out: needs a pole left behind, room for the head (and for the
            // block it pushes).
            if extension >= poles {
                self.pistons.insert(p, 0.0);
                return;
            }
            let new_head = offset(p, facing, extension + 1);
            let Some(ahead) = r.get(new_head).map(str::to_string) else {
                return;
            };
            let mut m = Vec::new();
            if !replaceable(&ahead) {
                let beyond = offset(p, facing, extension + 2);
                let free = r.get(beyond).is_some_and(replaceable);
                if !movable(&ahead) || !free {
                    self.pistons.insert(p, 0.0);
                    return;
                }
                m.push((beyond, ahead));
            }
            m.push((new_head, head_state));
            if extension >= 1 {
                m.push((offset(p, facing, extension), pole_state));
                m.push((offset(p, facing.opposite(), back), air));
            }
            m
        } else {
            if extension == 0 {
                self.pistons.insert(p, 0.0);
                return;
            }
            let head_pos = offset(p, facing, extension);
            let held_at = offset(p, facing, extension + 1);
            let held = r.get(held_at).map(str::to_string).unwrap_or_default();
            let mut m = Vec::new();
            if extension >= 2 {
                let behind = offset(p, facing.opposite(), back + 1);
                if !r.get(behind).is_some_and(replaceable) {
                    self.pistons.insert(p, 0.0);
                    return;
                }
                m.push((behind, pole_state));
                m.push((offset(p, facing, extension - 1), head_state));
            }
            // The block stuck to the head comes along.
            if movable(&held) {
                m.push((head_pos, held));
                m.push((held_at, air));
            } else {
                m.push((head_pos, air));
            }
            m
        };
        for (q, s) in moves {
            r.put(q, &s);
            actions.push(Action::Set(q, s));
        }
    }

    /// A piston broken: its poles in front broken with it, its head gone.
    pub fn piston_broken(&mut self, world: &mut impl World, p: Pos, facing: Dir) -> Vec<Action> {
        let mut r = Reads {
            world,
            seen: HashMap::new(),
        };
        let mut out = Vec::new();
        let mut q = facing.step(p);
        for _ in 0..64 {
            let Some(s) = r.get(q).map(str::to_string) else {
                break;
            };
            let (name, props) = split(&s);
            if name == POLE && prop(&props, "axis") == Some(facing.axis().name()) {
                out.push(Action::Destroy(q));
                q = facing.step(q);
                continue;
            }
            if name == HEAD && prop(&props, "facing") == Some(facing.name()) {
                out.push(Action::Set(q, "minecraft:air".into()));
            }
            break;
        }
        self.pistons.remove(&p);
        out
    }
}

/// The part a block's speed is reported for.
fn first_part(kind: Kind) -> u8 {
    kind.ports().first().map_or(0, |&(_, part, _)| part)
}

/// The cogwheels a cogwheel meshes with, and their speed as a factor of
/// its own: a small one beside a small one, a large one's corner beside a
/// small one, both on the same axis.
fn meshes(r: &mut Reads<impl World>, p: Pos, axis: Axis, large: bool) -> Vec<(Pos, f64)> {
    let mut out = Vec::new();
    let [u, v] = axis.others();
    let small = |k: Option<Kind>| k == Some(Kind::Cog { axis, large: false });
    let big = |k: Option<Kind>| k == Some(Kind::Cog { axis, large: true });
    for a in [u, v] {
        for d in a.dirs() {
            let q = d.step(p);
            if !large && small(r.kind(q)) {
                out.push((q, -1.0));
            }
        }
    }
    for du in u.dirs() {
        for dv in v.dirs() {
            let q = dv.step(du.step(p));
            let k = r.kind(q);
            if large && small(k) {
                out.push((q, -2.0));
            } else if !large && big(k) {
                out.push((q, -0.5));
            }
        }
    }
    out
}

/// Which way water turns a wheel (+1, -1 about its axis), 0 for none: the
/// flow on its four faces round the axis, each pushing its side along.
fn water_wheel(r: &mut Reads<impl World>, p: Pos, axis: Axis) -> f64 {
    let mut torque = 0;
    for a in axis.others() {
        for d in a.dirs() {
            let w = d.step(p);
            let Some(level) = water_level(r.get(w)) else {
                continue;
            };
            let mut flow = [0; 3];
            if level >= 8 {
                flow = Dir::Down.vec();
            } else {
                for h in [Dir::North, Dir::South, Dir::West, Dir::East] {
                    let n = h.step(w);
                    let lower = match r.get(n) {
                        Some(s) if is_air(s) => level > 0,
                        s => water_level(s).is_some_and(|l| l > level && l < 8),
                    };
                    if lower && n != p {
                        let v = h.vec();
                        flow = [flow[0] + v[0], flow[1] + v[1], flow[2] + v[2]];
                    }
                }
                if flow == [0, 0, 0] && level > 0 {
                    flow = Dir::Down.vec();
                }
            }
            torque += cross(d.vec(), flow)[axis.index()];
        }
    }
    torque.signum() as f64
}

fn water_level(state: Option<&str>) -> Option<i32> {
    let (name, props) = split(state?);
    if name != "minecraft:water" {
        return None;
    }
    Some(
        prop(&props, "level")
            .and_then(|l| l.parse().ok())
            .unwrap_or(0),
    )
}

/// Sails and sail frames joined to the block in front of a bearing.
fn count_sails(r: &mut Reads<impl World>, start: Pos) -> usize {
    let is_sail = |s: Option<&str>| s.is_some_and(|s| SAILS.contains(&split(s).0));
    if !is_sail(r.get(start)) {
        return 0;
    }
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(q) = queue.pop_front() {
        for d in Dir::ALL {
            let n = d.step(q);
            if seen.len() >= MAX_SAILS {
                return seen.len();
            }
            if !seen.contains(&n) && is_sail(r.get(n)) {
                seen.insert(n);
                queue.push_back(n);
            }
        }
    }
    seen.len()
}

/// The logs and leaves of the tree a saw cut at `cut`, highest first: no
/// lower than the cut, logs joined at edges and corners, leaves touching.
fn tree(r: &mut Reads<impl World>, cut: Pos) -> Vec<Pos> {
    let mut seen = BTreeSet::from([cut]);
    let mut queue = VecDeque::from([cut]);
    let mut out = vec![cut];
    while let Some(q) = queue.pop_front() {
        let log = r.get(q).is_some_and(is_log);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if out.len() >= TREE {
                        break;
                    }
                    let n = (q.0 + dx, q.1 + dy, q.2 + dz);
                    if n.1 < cut.1 || seen.contains(&n) {
                        continue;
                    }
                    let face = (dx != 0) as i32 + (dy != 0) as i32 + (dz != 0) as i32 == 1;
                    let s = r.get(n).unwrap_or("");
                    let take = if is_log(s) {
                        log
                    } else {
                        is_leaves(s) && face && log
                    };
                    if take {
                        seen.insert(n);
                        queue.push_back(n);
                        out.push(n);
                    }
                }
            }
        }
    }
    out.sort_by_key(|&(_, y, _)| std::cmp::Reverse(y));
    out
}

/// A plant's column cut at `cut`, its lowest block kept to grow again.
fn stalk(r: &mut Reads<impl World>, cut: Pos, state: &str) -> Vec<Pos> {
    let plant = split(state).0.to_string();
    let same = |s: Option<&str>| s.is_some_and(|s| split(s).0 == plant);
    let mut from = cut;
    if !same(r.get(Dir::Down.step(cut))) {
        from = Dir::Up.step(cut);
    }
    let mut out = Vec::new();
    let mut q = from;
    while out.len() < TREE && same(r.get(q)) {
        out.push(q);
        q = Dir::Up.step(q);
    }
    out.reverse();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Map(HashMap<Pos, String>);
    impl World for Map {
        fn get(&mut self, p: Pos) -> Option<String> {
            Some(
                self.0
                    .get(&p)
                    .cloned()
                    .unwrap_or_else(|| "minecraft:air".into()),
            )
        }
    }
    impl Map {
        fn put(&mut self, p: Pos, s: &str) {
            self.0.insert(p, s.into());
        }
        fn apply(&mut self, actions: &[Action]) {
            for a in actions {
                match a {
                    Action::Set(p, s) => self.put(*p, s),
                    Action::Destroy(p) => self.put(*p, "minecraft:air"),
                }
            }
        }
    }

    fn shaft(axis: &str) -> String {
        format!("create:shaft[axis={axis},waterlogged=false]")
    }
    fn cog(axis: &str, large: bool) -> String {
        let n = if large { "large_cogwheel" } else { "cogwheel" };
        format!("create:{n}[axis={axis},waterlogged=false]")
    }
    const CRANK_ON: &str = "create:hand_crank[face=floor,facing=north,powered=true]";

    fn run(w: &mut Map, d: &mut Dimension) -> Vec<Action> {
        let a = d.step(w, 0);
        w.apply(&a);
        a
    }

    #[test]
    fn a_crank_turns_a_shaft_line() {
        let mut w = Map::default();
        let mut d = Dimension::default();
        w.put((0, 1, 0), CRANK_ON);
        for y in -3..=0 {
            w.put((0, y, 0), &shaft("y"));
        }
        d.track((0, 1, 0));
        let a = run(&mut w, &mut d);
        assert_eq!(d.networks.len(), 1);
        assert_eq!(d.networks[0].parts, 5);
        // The crank turns about up at 32, so the shafts turn about -y... the
        // crank's port faces down: about down at 32 is about up at -32.
        assert_eq!(d.turning[&(0, -3, 0)].rpm, -32.0);
        assert_eq!(a.len(), 4, "{a:?}");
        assert!(w.0[&(0, 0, 0)].starts_with("create:shaft_spinning"));
        // Off: the shafts stop and look still again.
        w.put(
            (0, 1, 0),
            "create:hand_crank[face=floor,facing=north,powered=false]",
        );
        run(&mut w, &mut d);
        assert_eq!(d.turning[&(0, 0, 0)].rpm, 0.0);
        assert_eq!(w.0[&(0, 0, 0)], shaft("y"));
    }

    #[test]
    fn cogs_reverse_and_large_ones_halve() {
        let mut w = Map::default();
        let mut d = Dimension::default();
        w.put((0, 0, 0), "create:creative_motor[facing=up]");
        w.put((0, 1, 0), &cog("y", true));
        w.put((1, 1, 1), &cog("y", false));
        w.put((2, 1, 1), &cog("y", false));
        d.track((0, 0, 0));
        run(&mut w, &mut d);
        let big = d.turning[&(0, 1, 0)].rpm;
        assert_eq!(big, 16.0);
        assert_eq!(d.turning[&(1, 1, 1)].rpm, -32.0);
        assert_eq!(d.turning[&(2, 1, 1)].rpm, 32.0);
        assert_eq!(d.networks[0].speed, 32.0);
    }

    #[test]
    fn a_loop_of_mismatched_gears_jams() {
        let mut w = Map::default();
        let mut d = Dimension::default();
        // Three small cogs in an L with a fourth closing the square: a loop
        // of four meshes is fine; an odd loop through a shaft is not.
        w.put((0, 0, 0), "create:creative_motor[facing=up]");
        w.put((0, 1, 0), &cog("y", false));
        w.put((1, 1, 0), &cog("y", false));
        w.put((1, 1, 1), &cog("y", false));
        w.put((0, 1, 1), &cog("y", false));
        d.track((0, 0, 0));
        run(&mut w, &mut d);
        assert_eq!(d.networks[0].status, Status::Ok);
        // Two motors face to face on one shaft: each turns its front the
        // same way, so they turn against each other.
        let mut w = Map::default();
        let mut d = Dimension::default();
        w.put((0, 0, 0), "create:creative_motor[facing=east]");
        w.put((1, 0, 0), &shaft("x"));
        w.put((2, 0, 0), "create:creative_motor[facing=west]");
        d.track((0, 0, 0));
        run(&mut w, &mut d);
        assert_eq!(d.networks[0].status, Status::Jammed);
        assert_eq!(d.turning[&(1, 0, 0)].rpm, 0.0);
        // Back to back through a gearbox they agree.
        w.put((1, 0, 0), "create:gearbox[axis=y]");
        run(&mut w, &mut d);
        assert_eq!(d.networks[0].status, Status::Ok);
    }

    #[test]
    fn gearbox_turns_the_corner_and_reverses_straight() {
        let mut w = Map::default();
        let mut d = Dimension::default();
        w.put((-1, 0, 0), "create:creative_motor[facing=east]");
        w.put((0, 0, 0), "create:gearbox[axis=y]");
        w.put((1, 0, 0), &shaft("x"));
        w.put((0, 0, 1), &shaft("z"));
        d.track((0, 0, 0));
        run(&mut w, &mut d);
        // The motor turns about east at 16; the shaft straight through turns
        // about west at 16 (reversed); the corner one about north.
        assert_eq!(d.turning[&(1, 0, 0)].rpm, -16.0);
        assert_eq!(d.turning[&(0, 0, 1)].rpm.abs(), 16.0);
    }

    #[test]
    fn clutch_cuts_and_gearshift_reverses() {
        for (block, want) in [
            ("create:clutch[lit=false]", 16.0),
            ("create:clutch[lit=true]", 0.0),
            ("create:gearshift[lit=false]", 16.0),
            ("create:gearshift[lit=true]", -16.0),
        ] {
            let mut w = Map::default();
            let mut d = Dimension::default();
            w.put((0, 0, 0), "create:creative_motor[facing=up]");
            w.put((0, 1, 0), block);
            w.put((0, 2, 0), &shaft("y"));
            d.track((0, 0, 0));
            d.track((0, 2, 0));
            run(&mut w, &mut d);
            assert_eq!(d.turning[&(0, 2, 0)].rpm, want, "{block}");
        }
    }

    #[test]
    fn stress_stops_an_overloaded_network() {
        let mut w = Map::default();
        let mut d = Dimension::default();
        // A crank carries 256 SU; at 32 RPM a drill takes 128 and two take 256.
        w.put((0, 2, 0), CRANK_ON);
        w.put((0, 1, 0), "create:gearbox[axis=x]");
        w.put((0, 1, 1), "create:mechanical_drill[facing=south]");
        w.put((0, 1, -1), "create:mechanical_drill[facing=north]");
        d.track((0, 2, 0));
        run(&mut w, &mut d);
        assert_eq!(d.networks[0].status, Status::Ok);
        assert_eq!(d.networks[0].stress, 256.0);
        w.put((0, 0, 0), "create:mechanical_saw[facing=down]");
        run(&mut w, &mut d);
        assert_eq!(d.networks[0].status, Status::Overstressed);
        assert_eq!(d.turning[&(0, 1, 1)].rpm, 0.0);
    }

    #[test]
    fn a_drill_digs_and_a_saw_fells() {
        let mut w = Map::default();
        let mut d = Dimension::default();
        w.put((0, 0, 0), "create:creative_motor[facing=east]");
        w.put((1, 0, 0), "create:mechanical_drill[facing=east]");
        w.put((2, 0, 0), "minecraft:stone");
        d.track((0, 0, 0));
        d.set_motor((0, 0, 0), 256);
        let mut broke = Vec::new();
        for tick in 0..10 {
            let a = d.step(&mut w, tick * PERIOD);
            w.apply(&a);
            broke.extend(a.into_iter().filter(|a| matches!(a, Action::Destroy(_))));
        }
        assert_eq!(broke, [Action::Destroy((2, 0, 0))]);
        // A saw: a trunk of three logs and a leaf, felled from the bottom.
        let mut w = Map::default();
        let mut d = Dimension::default();
        w.put((0, 0, 0), "create:creative_motor[facing=east]");
        w.put((1, 0, 0), "create:mechanical_saw[facing=east]");
        for y in 0..3 {
            w.put((2, y, 0), "minecraft:oak_log[axis=y]");
        }
        w.put(
            (2, 3, 0),
            "minecraft:oak_leaves[distance=1,persistent=false,waterlogged=false]",
        );
        w.put((2, -1, 0), "minecraft:dirt");
        d.track((0, 0, 0));
        d.set_motor((0, 0, 0), 256);
        let mut broke = Vec::new();
        for tick in 0..20 {
            let a = d.step(&mut w, tick * PERIOD);
            w.apply(&a);
            broke.extend(a.into_iter().filter(|a| matches!(a, Action::Destroy(_))));
        }
        assert_eq!(broke.len(), 4, "{broke:?}");
        assert_eq!(w.0[&(2, -1, 0)], "minecraft:dirt");
    }

    #[test]
    fn piston_moves_both_ways() {
        let mut w = Map::default();
        let mut d = Dimension::default();
        let pole = "create:piston_extension_pole[axis=x,waterlogged=false]";
        let head = "create:mechanical_piston_head[facing=east]";
        w.put((0, 1, 0), "create:creative_motor[facing=down]");
        w.put((0, 0, 0), "create:mechanical_piston[facing=east]");
        w.put((-1, 0, 0), pole);
        w.put((-2, 0, 0), pole);
        w.put((1, 0, 0), "minecraft:stone");
        d.track((0, 0, 0));
        // Pick the sign that extends.
        d.set_motor((0, 1, 0), 256);
        d.step(&mut Map(w.0.clone()), 0);
        let sign = d.turning[&(0, 0, 0)].rpm.signum() as i32;
        d.set_motor((0, 1, 0), 256 * sign);
        for t in 0..20 {
            let a = d.step(&mut w, t * PERIOD);
            w.apply(&a);
        }
        // Two poles: out by two, the stone pushed ahead of the head.
        assert_eq!(w.0[&(1, 0, 0)], pole);
        assert_eq!(w.0[&(2, 0, 0)], head);
        assert_eq!(w.0[&(3, 0, 0)], "minecraft:stone");
        assert_eq!(w.0[&(-1, 0, 0)], pole);
        assert_eq!(
            w.0.get(&(-2, 0, 0)).map(String::as_str),
            Some("minecraft:air")
        );
        // Back in: the poles go home, the stone comes along.
        d.set_motor((0, 1, 0), -256 * sign);
        for t in 20..40 {
            let a = d.step(&mut w, t * PERIOD);
            w.apply(&a);
        }
        assert_eq!(w.0[&(-1, 0, 0)], pole);
        assert_eq!(w.0[&(-2, 0, 0)], pole);
        assert_eq!(w.0[&(1, 0, 0)], "minecraft:stone");
        assert_eq!(w.0[&(2, 0, 0)], "minecraft:air");
        assert_eq!(w.0[&(3, 0, 0)], "minecraft:air");
    }

    #[test]
    fn water_and_wind() {
        let mut w = Map::default();
        let mut d = Dimension::default();
        // Water falling past the east side of a wheel on z.
        w.put((0, 0, 0), "create:water_wheel[axis=z,waterlogged=false]");
        w.put((1, 0, 0), "minecraft:water[level=8]");
        w.put((0, 0, 1), &shaft("z"));
        d.track((0, 0, 0));
        run(&mut w, &mut d);
        // Falling on the +x side: (x cross -y) = -z, so -8 about z.
        assert_eq!(d.turning[&(0, 0, 1)].rpm, -8.0);
        // A windmill: 16 sails in front of a bearing make 2 RPM.
        let mut w = Map::default();
        let mut d = Dimension::default();
        w.put((0, 0, 0), "create:windmill_bearing[facing=up]");
        w.put((0, -1, 0), &shaft("y"));
        for x in -2..2 {
            for z in -2..2 {
                w.put((x, 1, z), "create:white_sail");
            }
        }
        d.track((0, 0, 0));
        run(&mut w, &mut d);
        assert_eq!(d.turning[&(0, 0, 0)].rpm, 2.0);
        // About down at 2 is about up at -2.
        assert_eq!(d.turning[&(0, -1, 0)].rpm, -2.0);
        assert_eq!(d.networks[0].capacity, 1024.0);
    }
}
