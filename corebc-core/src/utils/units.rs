use super::ConversionError;
use std::{convert::TryFrom, fmt, str::FromStr};

/// Common Core unit types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Units {
    /// Wei is equivalent to 1 wei.
    Ore,
    /// Kwei is equivalent to 1e3 wei.
    Wav,
    /// Mwei is equivalent to 1e6 wei.
    Grav,
    /// Gwei is equivalent to 1e9 wei.
    Nucle,
    /// Twei is equivalent to 1e12 wei.
    Atom,
    /// Pwei is equivalent to 1e15 wei.
    Moli,
    /// Ether is equivalent to 1e18 wei.
    Core,
    /// Other less frequent unit sizes, equivalent to 1e{0} wei.
    Other(u32),
}

impl fmt::Display for Units {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_num().to_string().as_str())
    }
}

impl TryFrom<u32> for Units {
    type Error = ConversionError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(Units::Other(value))
    }
}

impl TryFrom<i32> for Units {
    type Error = ConversionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(Units::Other(value as u32))
    }
}

impl TryFrom<usize> for Units {
    type Error = ConversionError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Units::Other(value as u32))
    }
}

impl TryFrom<String> for Units {
    type Error = ConversionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl<'a> TryFrom<&'a String> for Units {
    type Error = ConversionError;

    fn try_from(value: &'a String) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

impl TryFrom<&str> for Units {
    type Error = ConversionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

impl FromStr for Units {
    type Err = ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "xcb" | "core" => Units::Core,
            "moli" | "milli" | "milliether" | "finney" => Units::Moli,
            "atom" | "micro" | "microether" | "szabo" => Units::Atom,
            "nucle" | "nano" | "nanoether" | "shannon" => Units::Nucle,
            "grav" | "pico" | "picoether" | "lovelace" => Units::Grav,
            "wav" | "femto" | "femtoether" | "babbage" => Units::Wav,
            "ore" => Units::Ore,
            _ => return Err(ConversionError::UnrecognizedUnits(s.to_string())),
        })
    }
}

impl From<Units> for u32 {
    fn from(units: Units) -> Self {
        units.as_num()
    }
}

impl From<Units> for i32 {
    fn from(units: Units) -> Self {
        units.as_num() as i32
    }
}

impl From<Units> for usize {
    fn from(units: Units) -> Self {
        units.as_num() as usize
    }
}

impl Units {
    pub fn as_num(&self) -> u32 {
        match self {
            Units::Ore => 0,
            Units::Wav => 3,
            Units::Grav => 6,
            Units::Nucle => 9,
            Units::Atom => 12,
            Units::Moli => 15,
            Units::Core => 18,
            Units::Other(inner) => *inner,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Units::*;

    #[test]
    fn test_units() {
        assert_eq!(Ore.as_num(), 0);
        assert_eq!(Wav.as_num(), 3);
        assert_eq!(Grav.as_num(), 6);
        assert_eq!(Nucle.as_num(), 9);
        assert_eq!(Atom.as_num(), 12);
        assert_eq!(Moli.as_num(), 15);
        assert_eq!(Core.as_num(), 18);
        assert_eq!(Other(10).as_num(), 10);
        assert_eq!(Other(20).as_num(), 20);
    }

    #[test]
    fn test_into() {
        assert_eq!(Units::try_from("ore").unwrap(), Ore);
        assert_eq!(Units::try_from("wav").unwrap(), Wav);
        assert_eq!(Units::try_from("grav").unwrap(), Grav);
        assert_eq!(Units::try_from("nucle").unwrap(), Nucle);
        assert_eq!(Units::try_from("atom").unwrap(), Atom);
        assert_eq!(Units::try_from("moli").unwrap(), Moli);
        assert_eq!(Units::try_from("core").unwrap(), Core);

        assert_eq!(Units::try_from("ore".to_string()).unwrap(), Ore);
        assert_eq!(Units::try_from("wav".to_string()).unwrap(), Wav);
        assert_eq!(Units::try_from("grav".to_string()).unwrap(), Grav);
        assert_eq!(Units::try_from("nucle".to_string()).unwrap(), Nucle);
        assert_eq!(Units::try_from("atom".to_string()).unwrap(), Atom);
        assert_eq!(Units::try_from("moli".to_string()).unwrap(), Moli);
        assert_eq!(Units::try_from("core".to_string()).unwrap(), Core);

        assert_eq!(Units::try_from(&"ore".to_string()).unwrap(), Ore);
        assert_eq!(Units::try_from(&"wav".to_string()).unwrap(), Wav);
        assert_eq!(Units::try_from(&"grav".to_string()).unwrap(), Grav);
        assert_eq!(Units::try_from(&"nucle".to_string()).unwrap(), Nucle);
        assert_eq!(Units::try_from(&"atom".to_string()).unwrap(), Atom);
        assert_eq!(Units::try_from(&"moli".to_string()).unwrap(), Moli);
        assert_eq!(Units::try_from(&"core".to_string()).unwrap(), Core);
    }
}
