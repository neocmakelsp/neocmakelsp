use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, Default)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub extra: Option<Extra>,
}

const EXTRRA_REGEX_RULES: &str = r"((?<version>[0-9]+)(?<extra>[a-z]+)(?<extraVersion>[0-9]+))";
static EXTRRA_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(EXTRRA_REGEX_RULES).unwrap());

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Extra {
    Alpha(u32),
    Beta(u32),
    Rc(u32),
}

impl Version {
    pub const fn invalid() -> Self {
        Self {
            major: 0,
            minor: 0,
            patch: 0,
            extra: None,
        }
    }
}

impl From<&str> for Version {
    fn from(value: &str) -> Self {
        let mut version = Self::default();
        let versions: Vec<&str> = value.split(".").collect();
        for (index, ver) in versions.iter().enumerate() {
            let ver: u32 = if index != 2 {
                let Ok(ver) = ver.parse() else {
                    return Self::default();
                };
                ver
            } else {
                match ver.parse() {
                    Ok(ver) => ver,
                    Err(_) if let Some(caps) = EXTRRA_REGEX.captures(ver) => {
                        let ver = &caps["version"];
                        let extra = &caps["extra"];

                        let extra_version = &caps["extraVersion"];
                        let extra_version: u32 = extra_version.parse().unwrap();
                        let extra = match extra {
                            "alpha" => Extra::Alpha(extra_version),
                            "beta" => Extra::Beta(extra_version),
                            "rc" => Extra::Rc(extra_version),
                            _ => return Self::default(),
                        };
                        version.extra = Some(extra);

                        ver.parse().unwrap()
                    }
                    _ => return Self::default(),
                }
            };

            match index {
                0 => version.major = ver,
                1 => version.minor = ver,
                2 => version.patch = ver,
                _ => {
                    break;
                }
            }
        }
        version
    }
}

impl From<String> for Version {
    fn from(value: String) -> Self {
        let mut version = Self::default();
        let versions: Vec<&str> = value.split(".").collect();
        for (index, ver) in versions.iter().enumerate() {
            let Ok(ver) = ver.parse() else {
                return Self::default();
            };
            match index {
                0 => version.major = ver,
                1 => version.minor = ver,
                2 => version.patch = ver,
                _ => {
                    break;
                }
            }
        }
        version
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        // NOTE: we should keep patch won't break anything
        self.major == other.major && self.minor == other.minor
    }
}
impl Eq for Version {}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        if self.major > other.major {
            return Ordering::Greater;
        } else if self.major < other.major {
            return Ordering::Less;
        }
        if self.minor > other.minor {
            return Ordering::Greater;
        } else if self.minor < other.minor {
            return Ordering::Less;
        }
        Ordering::Equal
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod test {
    use super::Extra;
    use super::Version;

    const INVALAD_VERSION: Version = Version::invalid();
    #[test]
    fn invalid() {
        let version = "abcd";
        let version: Version = version.into();
        assert_eq!(version, INVALAD_VERSION);
        let version = "1.3.ab";
        let version: Version = version.into();
        assert_eq!(version, INVALAD_VERSION);
    }
    #[test]
    fn valid() {
        let version = "1.2.3";
        let version: Version = version.into();
        assert_eq!(
            version,
            Version {
                major: 1,
                minor: 2,
                patch: 3,
                extra: None
            }
        );
        let version = "1.3.3rc1";
        let version: Version = version.into();
        assert_eq!(
            version,
            Version {
                major: 1,
                minor: 3,
                patch: 3,
                extra: Some(Extra::Rc(1))
            }
        );
    }

    #[test]
    fn compare() {
        let version = "1.2.3";
        let version1: Version = version.into();
        let version = "1.2.4";
        let version2: Version = version.into();
        let version = "1.2.4rc1";
        let version3: Version = version.into();
        let version = "1.1.4";
        let version4: Version = version.into();
        let version = "2.1.4";
        let version5: Version = version.into();
        assert!(version1 == version2);
        assert!(version1 == version3);
        assert!(version1 > version4);
        assert!(version1 < version5);
    }
}
