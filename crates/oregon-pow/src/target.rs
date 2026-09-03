use core::cmp::Ordering;

pub fn hash_meets_target(hash_le: [u8; 32], target_le: [u8; 32]) -> bool {
    for index in (0..32).rev() {
        match hash_le[index].cmp(&target_le[index]) {
            Ordering::Less => return true,
            Ordering::Greater => return false,
            Ordering::Equal => {}
        }
    }
    true
}
