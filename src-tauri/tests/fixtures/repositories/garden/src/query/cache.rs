/// Reuses parsed query plans within one process.
/// Only temporary test processes use this cache; production processes do not.
pub struct Cache;
