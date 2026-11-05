//! Character rigging: armatures of bones with forward kinematics, a constrained
//! CCD inverse-kinematics solver, pose interpolation and element binding.

use animara_document::{Armature, Bone, Pose, fresh_id};
use animara_geometry::{Affine, Vec2, lerp_angle};

/// World-space state of one bone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneWorld {
    pub head: Vec2,
    pub tail: Vec2,
    /// Absolute angle, radians.
    pub angle: f64,
}

impl BoneWorld {
    /// Transform mapping bone-local space (x along the bone) into world space.
    pub fn matrix(&self) -> Affine {
        Affine::translate(self.head.x, self.head.y) * Affine::rotate(self.angle)
    }
}

/// Rest pose: the angles stored in the armature itself.
pub fn rest_pose(arm: &Armature) -> Pose {
    Pose { angles: arm.bones.iter().map(|b| b.angle).collect(), root_offset: Vec2::ZERO }
}

/// Forward kinematics: compute the world state of every bone for a pose.
/// Parents must precede children in `arm.bones`.
pub fn forward(arm: &Armature, pose: &Pose) -> Vec<BoneWorld> {
    let mut out: Vec<BoneWorld> = Vec::with_capacity(arm.bones.len());
    for (i, b) in arm.bones.iter().enumerate() {
        let local = pose.angles.get(i).copied().unwrap_or(b.angle);
        let w = match b.parent.and_then(|p| out.get(p).copied()) {
            Some(parent) => {
                let head = parent.tail + Vec2::new(b.head.x, b.head.y).rotate(parent.angle);
                let angle = parent.angle + local;
                BoneWorld { head, tail: head + Vec2::from_angle(angle) * b.length, angle }
            }
            None => {
                let head = b.head + pose.root_offset;
                BoneWorld { head, tail: head + Vec2::from_angle(local) * b.length, angle: local }
            }
        };
        out.push(w);
    }
    out
}

/// Interpolate two poses (shortest-arc angles).
pub fn lerp_pose(a: &Pose, b: &Pose, t: f64) -> Pose {
    let n = a.angles.len().max(b.angles.len());
    Pose {
        angles: (0..n)
            .map(|i| {
                let x = a.angles.get(i).or(b.angles.get(i)).copied().unwrap_or(0.0);
                let y = b.angles.get(i).copied().unwrap_or(x);
                lerp_angle(x, y, t)
            })
            .collect(),
        root_offset: a.root_offset.lerp(b.root_offset, t),
    }
}

/// Transform that moves something bound at rest pose to follow bone `i` in `pose`.
pub fn binding_transform(rest: &[BoneWorld], posed: &[BoneWorld], i: usize) -> Affine {
    match (rest.get(i), posed.get(i)) {
        (Some(r), Some(p)) => p.matrix() * r.matrix().invert().unwrap_or(Affine::IDENTITY),
        _ => Affine::IDENTITY,
    }
}

fn chain_to_root(arm: &Armature, end: usize) -> Vec<usize> {
    let mut chain = vec![end];
    let mut cur = arm.bones[end].parent;
    while let Some(p) = cur {
        if arm.bones[p].pinned {
            break;
        }
        chain.push(p);
        cur = arm.bones[p].parent;
        if chain.len() > arm.bones.len() {
            break;
        }
    }
    chain
}

/// Cyclic-coordinate-descent IK: rotate the chain ending at `end` so its tail reaches `target`.
/// Respects per-bone angle limits. Returns the solved pose.
pub fn solve_ik(arm: &Armature, pose: &Pose, end: usize, target: Vec2, iterations: usize) -> Pose {
    let mut pose = pose.clone();
    if end >= arm.bones.len() {
        return pose;
    }
    if pose.angles.len() < arm.bones.len() {
        let rest = rest_pose(arm);
        pose.angles.extend_from_slice(&rest.angles[pose.angles.len()..]);
    }
    let chain = chain_to_root(arm, end);
    for _ in 0..iterations {
        for &j in &chain {
            let world = forward(arm, &pose);
            let tip = world[end].tail;
            if tip.distance(target) < 0.01 {
                return pose;
            }
            let head = world[j].head;
            let a = (tip - head).angle();
            let b = (target - head).angle();
            let mut delta = b - a;
            while delta > std::f64::consts::PI {
                delta -= std::f64::consts::TAU;
            }
            while delta < -std::f64::consts::PI {
                delta += std::f64::consts::TAU;
            }
            let bone = &arm.bones[j];
            let mut angle = pose.angles[j] + delta;
            if let Some(min) = bone.min_angle {
                angle = angle.max(min);
            }
            if let Some(max) = bone.max_angle {
                angle = angle.min(max);
            }
            pose.angles[j] = angle;
        }
    }
    pose
}

/// Add a bone from `head` to `tail` (world coordinates) to the armature.
/// When `parent` is given, the new bone's head is expressed in the parent's tip space.
pub fn add_bone(arm: &mut Armature, parent: Option<usize>, head: Vec2, tail: Vec2, name: &str) -> usize {
    let world = forward(arm, &rest_pose(arm));
    let abs_angle = (tail - head).angle();
    let (local_head, local_angle) = match parent.and_then(|p| world.get(p).map(|w| (p, *w))) {
        Some((_, pw)) => ((head - pw.tail).rotate(-pw.angle), abs_angle - pw.angle),
        None => (head, abs_angle),
    };
    arm.bones.push(Bone {
        id: fresh_id(),
        name: name.to_string(),
        parent,
        head: local_head,
        length: head.distance(tail).max(1.0),
        angle: local_angle,
        bound_element: None,
        min_angle: None,
        max_angle: None,
        pinned: false,
    });
    arm.bones.len() - 1
}

/// Find the bone whose segment is closest to `p` (within `tol`).
pub fn pick_bone(world: &[BoneWorld], p: Vec2, tol: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, w) in world.iter().enumerate() {
        let ab = w.tail - w.head;
        let t = ((p - w.head).dot(ab) / ab.length_sq().max(1e-9)).clamp(0.0, 1.0);
        let d = p.distance(w.head + ab * t);
        if d <= tol && best.is_none_or(|b| d < b.1) {
            best = Some((i, d));
        }
    }
    best.map(|b| b.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm() -> Armature {
        let mut a = Armature::default();
        let root = add_bone(&mut a, None, Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0), "upper");
        add_bone(&mut a, Some(root), Vec2::new(100.0, 0.0), Vec2::new(200.0, 0.0), "lower");
        a
    }

    #[test]
    fn forward_kinematics_chain() {
        let a = arm();
        let w = forward(&a, &rest_pose(&a));
        assert!((w[1].tail - Vec2::new(200.0, 0.0)).length() < 1e-9);
        let mut p = rest_pose(&a);
        p.angles[0] = std::f64::consts::FRAC_PI_2;
        let w = forward(&a, &p);
        assert!((w[1].tail - Vec2::new(0.0, 200.0)).length() < 1e-9);
    }

    #[test]
    fn ik_reaches_target() {
        let a = arm();
        let target = Vec2::new(120.0, 80.0);
        let p = solve_ik(&a, &rest_pose(&a), 1, target, 50);
        let w = forward(&a, &p);
        assert!(w[1].tail.distance(target) < 0.5, "{:?}", w[1].tail);
    }

    #[test]
    fn ik_respects_limits() {
        let mut a = arm();
        a.bones[1].min_angle = Some(-0.1);
        a.bones[1].max_angle = Some(0.1);
        let p = solve_ik(&a, &rest_pose(&a), 1, Vec2::new(50.0, 50.0), 50);
        assert!(p.angles[1] >= -0.1 - 1e-9 && p.angles[1] <= 0.1 + 1e-9);
    }

    #[test]
    fn binding_follows_bone() {
        let a = arm();
        let rest = forward(&a, &rest_pose(&a));
        let mut p = rest_pose(&a);
        p.angles[0] = std::f64::consts::FRAC_PI_2;
        let posed = forward(&a, &p);
        let m = binding_transform(&rest, &posed, 0);
        let q = m.apply(Vec2::new(50.0, 0.0));
        assert!((q - Vec2::new(0.0, 50.0)).length() < 1e-9);
    }
}
