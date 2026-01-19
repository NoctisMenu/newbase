/// Spine
/// Left Arm
/// Right Arm
/// Left Leg
/// Right Leg
pub struct Skeleton {
    pub bone_links: [Vec<usize>; 5],
}

impl Skeleton {
    ///Construction semantics:
    /// Arms should always meet at the first link
    /// eg:
    /// [6,7,8,9] and [6,10,11,12]
    /// instead of
    /// [9,8,7,6] and [12,11,10,6]
    /// Same for legs.
    /// The intersection point should always be part of the spine bone list
    /// The legs intersection should be the first link.
    pub fn new(bone_links: [Vec<usize>; 5]) -> Self {
        Self { bone_links }
    }
    //iterator should go through each Vector, then return the first index + second index
    pub fn bone_paths(&self) -> Vec<(usize, usize)> {
        let mut bone_paths = Vec::new();
        for limb in &self.bone_links {
            for index in 0..limb.len() - 1 {
                bone_paths.push((limb[index], limb[index + 1]));
            }
        }
        bone_paths
    }
}
