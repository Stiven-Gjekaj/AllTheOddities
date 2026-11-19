//! The version and the codename of this build.
//!
//! The version is `YYYY.MM.N`: the year and the month of the release, and a
//! count of the releases in that month. Each month with a release takes the
//! next codename of the list. docs/releasing.md gives the rules.

/// The codenames, in the order that releases take them. The theme is a
/// cabinet of curiosities.
pub const CODENAMES: [&str; 26] = [
    "Astrolabe",
    "Barometer",
    "Chronometer",
    "Diorama",
    "Ephemeris",
    "Fossil",
    "Gyroscope",
    "Hourglass",
    "Inclinometer",
    "Jacquard",
    "Kaleidoscope",
    "Lodestone",
    "Microscope",
    "Narwhal",
    "Orrery",
    "Planisphere",
    "Quadrant",
    "Reliquary",
    "Sextant",
    "Telescope",
    "Unicorn",
    "Vellum",
    "Wunderkammer",
    "Xylograph",
    "Yardstick",
    "Zoetrope",
];

/// The codename of this build, as a literal, so `concat!` can use it. The
/// release commit changes it.
macro_rules! codename {
    () => {
        "Astrolabe"
    };
}

/// The codename of this build.
pub const CODENAME: &str = codename!();

/// The version and the codename, as `odd --version` prints them.
pub const LONG: &str = concat!(env!("CARGO_PKG_VERSION"), " (", codename!(), ")");

/// The version and the codename, as `odd --version` prints them.
pub fn long() -> String {
    LONG.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codenames_go_from_a_to_z() {
        assert_eq!(CODENAMES.len(), 26);
        for (i, name) in CODENAMES.iter().enumerate() {
            assert_eq!(name.as_bytes()[0], b'A' + i as u8, "{name}");
        }
    }

    #[test]
    fn the_long_version_names_the_codename() {
        assert_eq!(
            long(),
            format!("{} ({})", env!("CARGO_PKG_VERSION"), CODENAME)
        );
        assert!(CODENAMES.contains(&CODENAME));
    }
}
