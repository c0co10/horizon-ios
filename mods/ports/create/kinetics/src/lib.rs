//! Create for Horizon, the server half (`docs/mods.md`, "Server mods"):
//! the blocks of the content half (`create`) turned. It finds the Create
//! blocks near players and where they are placed, joins them into rotation
//! networks through their shafts and gears, sets each network's speed from
//! its sources (hand crank, water wheel, windmill, creative motor), weighs
//! its stress, and runs the machines it drives (drill, saw, mechanical
//! piston). Turning blocks are shown by their `_spinning` twins, whose
//! textures move. `/create help` lists the commands.
//!
//! The rules are in [`sim`], apart from the host, and tested natively.

pub mod blocks;
pub mod sim;

use blocks::{Kind, NS, Pos, split};
use horizon_mod_sdk::server::{
    Args, Argument, BlockChange, BlockPos, Player, Registration, ServerMod, players, world,
};
use horizon_mod_sdk::storage;
use sim::{Action, Dimension, PERIOD, Status};
use std::collections::BTreeMap;

/// Blocks read each tick looking for Create blocks round the players.
const SCAN_PER_TICK: u64 = 2048;
/// The box scanned round each player: this far out, and up and down.
const SCAN_REACH: i32 = 32;
const SCAN_HEIGHT: i32 = 24;
/// `/create info` without a position looks this far.
const INFO_REACH: i32 = 6;
const MOTORS_KEY: &str = "motors";

struct Kinetics {
    dims: BTreeMap<String, Dimension>,
    scan: u64,
}

/// The server's blocks of one dimension.
struct Host<'a>(&'a str);

impl sim::World for Host<'_> {
    fn get(&mut self, p: Pos) -> Option<String> {
        world::get_block(self.0, p.0, p.1, p.2)
    }
}

fn apply(dimension: &str, actions: Vec<Action>) {
    let mut sets = Vec::new();
    let mut breaks = Vec::new();
    // Sets and breaks keep their order: each run of one kind goes at once.
    let flush = |sets: &mut Vec<BlockChange>, breaks: &mut Vec<(i32, i32, i32)>| {
        for chunk in sets.chunks(4096) {
            if let Err(e) = world::set_blocks(dimension, chunk) {
                horizon_mod_sdk::log::write(
                    horizon_mod_sdk::log::Level::Warn,
                    &format!("create: setting blocks: {e}"),
                );
            }
        }
        for chunk in breaks.chunks(256) {
            if let Err(e) = world::destroy_blocks(dimension, chunk) {
                horizon_mod_sdk::log::write(
                    horizon_mod_sdk::log::Level::Warn,
                    &format!("create: breaking blocks: {e}"),
                );
            }
        }
        sets.clear();
        breaks.clear();
    };
    for a in actions {
        match a {
            Action::Set(p, state) => {
                if !breaks.is_empty() {
                    flush(&mut sets, &mut breaks);
                }
                sets.push(BlockChange {
                    x: p.0,
                    y: p.1,
                    z: p.2,
                    state,
                });
            }
            Action::Destroy(p) => {
                if !sets.is_empty() {
                    flush(&mut sets, &mut breaks);
                }
                breaks.push(p);
            }
        }
    }
    flush(&mut sets, &mut breaks);
}

fn feet(player: &Player) -> Pos {
    (
        player.x.floor() as i32,
        player.y.floor() as i32,
        player.z.floor() as i32,
    )
}

fn rpm(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.1}")
    }
}

impl Kinetics {
    fn dim(&mut self, name: &str) -> &mut Dimension {
        self.dims.entry(name.to_string()).or_default()
    }

    /// Reads the next stretch of each player's box, tracking what it finds.
    fn scan_round(&mut self, online: &[Player]) {
        if online.is_empty() {
            return;
        }
        let side = (2 * SCAN_REACH + 1) as u64;
        let tall = (2 * SCAN_HEIGHT + 1) as u64;
        let volume = side * side * tall;
        let each = (SCAN_PER_TICK / online.len() as u64).max(64);
        let start = self.scan;
        self.scan = (self.scan + each) % volume;
        for player in online {
            let (px, py, pz) = feet(player);
            let mut found = Vec::new();
            for i in start..start + each {
                let i = i % volume;
                let dy = (i / (side * side)) as i32 - SCAN_HEIGHT;
                let dz = ((i / side) % side) as i32 - SCAN_REACH;
                let dx = (i % side) as i32 - SCAN_REACH;
                let p = (px + dx, py + dy, pz + dz);
                let state = world::get_block(&player.dimension, p.0, p.1, p.2);
                if state.is_some_and(|s| s.starts_with(NS) && Kind::of(&s).is_some()) {
                    found.push(p);
                }
            }
            let dim = self.dim(&player.dimension);
            for p in found {
                dim.track(p);
            }
        }
    }

    fn save_motors(&self) {
        let mut text = String::new();
        for (dim, d) in &self.dims {
            for (p, rpm) in &d.motors {
                text.push_str(&format!("{dim} {} {} {} {rpm}\n", p.0, p.1, p.2));
            }
        }
        if let Err(e) = storage::set(MOTORS_KEY, text.as_bytes()) {
            horizon_mod_sdk::log::write(
                horizon_mod_sdk::log::Level::Warn,
                &format!("create: saving motors: {e}"),
            );
        }
    }

    fn load_motors(&mut self) {
        let Some(bytes) = storage::get(MOTORS_KEY) else {
            return;
        };
        for line in String::from_utf8_lossy(&bytes).lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let [dim, x, y, z, rpm] = f[..] else { continue };
            let (Ok(x), Ok(y), Ok(z), Ok(rpm)) = (x.parse(), y.parse(), z.parse(), rpm.parse())
            else {
                continue;
            };
            self.dim(dim).set_motor((x, y, z), rpm);
        }
    }

    /// What `/create info` says about the block at `p`.
    fn describe(&mut self, dimension: &str, p: Pos) -> Result<String, String> {
        let state = world::get_block(dimension, p.0, p.1, p.2).ok_or("That block isn't loaded")?;
        let kind = Kind::of(&state).ok_or_else(|| {
            format!(
                "{} {} {} is {}, not a Create machine",
                p.0,
                p.1,
                p.2,
                split(&state).0
            )
        })?;
        let dim = self.dim(dimension);
        dim.track(p);
        let at = format!("{} at {} {} {}", kind.label(), p.0, p.1, p.2);
        let Some(t) = dim.turning.get(&p).copied() else {
            return Ok(format!("{at}: not yet in a network, ask again in a moment"));
        };
        let net = &dim.networks[t.network];
        let state = match net.status {
            Status::Ok if net.speed == 0.0 => "still: nothing turns it".to_string(),
            Status::Ok => format!("{} RPM", rpm(t.rpm.abs())),
            Status::Overstressed => "stopped: overstressed".into(),
            Status::Jammed => "stopped: its gears or sources turn against each other".into(),
            Status::TooBig => format!("stopped: more than {} parts", sim::MAX_NODES),
        };
        let direction = if t.rpm > 0.0 {
            " (clockwise)"
        } else if t.rpm < 0.0 {
            " (counter-clockwise)"
        } else {
            ""
        };
        let motor = match kind {
            Kind::Motor { .. } => format!(
                ", set to {} RPM",
                dim.motors.get(&p).unwrap_or(&sim::MOTOR_DEFAULT_RPM)
            ),
            _ => String::new(),
        };
        let capacity = if net.capacity >= sim::MOTOR_SU {
            "unlimited".to_string()
        } else {
            format!("{}", net.capacity.round())
        };
        Ok(format!(
            "{at}: {state}{direction}{motor}. Network: {} parts, stress {} of {capacity} SU",
            net.parts,
            net.stress.round()
        ))
    }

    /// The Create block nearest `player`, within [`INFO_REACH`].
    fn nearest(&mut self, player: &Player) -> Option<Pos> {
        let (x, y, z) = feet(player);
        let eye = (x, y + 1, z);
        let dim = self.dim(&player.dimension);
        let mut best: Option<(i32, Pos)> = None;
        for &p in &dim.tracked {
            let d2 = (p.0 - eye.0).pow(2) + (p.1 - eye.1).pow(2) + (p.2 - eye.2).pow(2);
            if d2 <= INFO_REACH * INFO_REACH && best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, p));
            }
        }
        if best.is_none() {
            // Not seen yet: look round now.
            for dy in -INFO_REACH..=INFO_REACH {
                for dz in -INFO_REACH..=INFO_REACH {
                    for dx in -INFO_REACH..=INFO_REACH {
                        let d2 = dx * dx + dy * dy + dz * dz;
                        if d2 > INFO_REACH * INFO_REACH || best.is_some_and(|(b, _)| d2 >= b) {
                            continue;
                        }
                        let p = (eye.0 + dx, eye.1 + dy, eye.2 + dz);
                        let found = world::get_block(&player.dimension, p.0, p.1, p.2)
                            .is_some_and(|s| Kind::of(&s).is_some());
                        if found {
                            best = Some((d2, p));
                        }
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }
}

const HELP: &str = "/create info [pos]: speed and stress of the machine at pos, or the nearest one. \
/create motor <pos> <rpm>: a creative motor's speed (-256 to 256). \
/create networks: the rotation networks in this dimension.";

impl ServerMod for Kinetics {
    fn init() -> Result<(Self, Registration), String> {
        let mut me = Kinetics {
            dims: BTreeMap::new(),
            scan: 0,
        };
        me.load_motors();
        Ok((
            me,
            Registration::new().on_tick().on_blocks().command_with(
                "create",
                [
                    Argument::word("action"),
                    Argument::block_pos("pos").optional(),
                    Argument::integer("rpm", Some(-256), Some(256)).optional(),
                ],
            ),
        ))
    }

    fn tick(&mut self, tick: u64) {
        let online = players::online();
        self.scan_round(&online);
        if !tick.is_multiple_of(PERIOD) {
            return;
        }
        let names: Vec<String> = self.dims.keys().cloned().collect();
        for name in names {
            let dim = self.dims.get_mut(&name).unwrap();
            if dim.tracked.is_empty() {
                continue;
            }
            let actions = dim.step(&mut Host(&name), tick);
            apply(&name, actions);
        }
    }

    fn block_placed(&mut self, player: &Player, pos: BlockPos, state: &str) {
        if state.starts_with(NS) {
            self.dim(&player.dimension).track((pos.x, pos.y, pos.z));
        }
    }

    fn block_broken(&mut self, player: &Player, pos: BlockPos, state: &str) {
        let p = (pos.x, pos.y, pos.z);
        if let Some(Kind::Piston { facing }) = Kind::of(state) {
            let dimension = player.dimension.clone();
            let actions = self
                .dim(&dimension)
                .piston_broken(&mut Host(&dimension), p, facing);
            apply(&dimension, actions);
        }
        if let Some(Kind::Motor { .. }) = Kind::of(state)
            && self.dim(&player.dimension).motors.remove(&p).is_some()
        {
            self.save_motors();
        }
    }

    fn command(
        &mut self,
        player: &Player,
        _name: &str,
        args: &Args,
    ) -> Result<Option<String>, String> {
        let pos = args.block_pos("pos").map(|p| (p.x, p.y, p.z));
        match args.string("action").unwrap_or("help") {
            "info" => {
                let p = match pos {
                    Some(p) => p,
                    None => self
                        .nearest(player)
                        .ok_or("No Create machine within 6 blocks: give a position")?,
                };
                self.describe(&player.dimension, p).map(Some)
            }
            "motor" => {
                let p = pos.ok_or("Usage: /create motor <pos> <rpm>")?;
                let speed = args
                    .integer("rpm")
                    .ok_or("Usage: /create motor <pos> <rpm>")?;
                let state = world::get_block(&player.dimension, p.0, p.1, p.2)
                    .ok_or("That block isn't loaded")?;
                if !matches!(Kind::of(&state), Some(Kind::Motor { .. })) {
                    return Err(format!("{} {} {} is not a creative motor", p.0, p.1, p.2));
                }
                self.dim(&player.dimension).set_motor(p, speed);
                self.save_motors();
                Ok(Some(format!(
                    "Creative motor at {} {} {} set to {speed} RPM",
                    p.0, p.1, p.2
                )))
            }
            "networks" => {
                let dim = self.dim(&player.dimension);
                if dim.networks.is_empty() {
                    return Ok(Some("No rotation networks here yet".into()));
                }
                let turning = dim.networks.iter().filter(|n| n.speed > 0.0).count();
                let stopped = dim
                    .networks
                    .iter()
                    .filter(|n| n.status != Status::Ok)
                    .count();
                let parts: usize = dim.networks.iter().map(|n| n.parts).sum();
                Ok(Some(format!(
                    "{} networks ({turning} turning, {stopped} stopped), {parts} parts in all",
                    dim.networks.len()
                )))
            }
            "help" => Ok(Some(HELP.into())),
            other => Err(format!("Unknown action {other}: try /create help")),
        }
    }
}

horizon_mod_sdk::export_server_mod!(Kinetics);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_read_well() {
        assert_eq!(rpm(32.0), "32");
        assert_eq!(rpm(0.5), "0.5");
        assert_eq!(rpm(16.04), "16");
    }

    #[test]
    fn directions_name_their_faces() {
        use blocks::Dir;
        assert_eq!(Dir::parse("east").map(Dir::opposite), Some(Dir::West));
    }
}
